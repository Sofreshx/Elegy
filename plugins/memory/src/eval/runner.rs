//! Orchestrates the eval harness: fresh store per run, inject corpus, query,
//! measure, compute every metric, compare against gate thresholds.
//!
//! No metric here is optional — `run_eval` always computes and gates all
//! twelve entries in `gates::REQUIRED_METRIC_NAMES` (the spec's nine metrics,
//! where write and retrieval latency each split into p50/p95, plus
//! `gate_accuracy_full`), matching the spec's Acceptance Criteria ("no metric
//! is computed without a gate").

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::eval::{
    corpus::{
        self, CorpusMemory, EvalCorpusError, ExpectedGateOutcome, GateCorpus, RetrievalCorpus,
    },
    embedding::{axis_vector, blended_vector, vector_at_similarity, EVAL_EMBEDDING_DIMENSIONS},
    gates::{self, EvalGateError, EvalGateThresholds},
    metrics, synthetic,
};
use crate::{
    runtime::block_on, DefaultSalienceGate, GateDecision, Memory, MemoryCandidate, MemoryId,
    MemoryScope, MemoryState, MemoryStore, MemoryType, ProvenanceLevel, SalienceGate, ScopeConfig,
    ScoredMemory, SearchQuery, SensitivityLevel, SqliteMemoryStore, StoreError,
};

/// Number of retrieval queries in the isolated write-latency timing pass. Kept
/// separate from the golden corpus so a write-latency measurement never risks
/// perturbing the golden corpus's ground truth via gate decisions.
const WRITE_LATENCY_SAMPLE_COUNT: usize = 30;

/// Memory count for the retrieval-at-scale latency pass. The spec targets 10k;
/// Phase A uses a smaller count so `eval run --ci` stays within the CI job's
/// time budget (documented deviation, see `docs/specs/eval-harness-v1/spec.md`).
const RETRIEVAL_SCALE_MEMORY_COUNT: usize = 2_000;
const RETRIEVAL_SCALE_QUERY_COUNT: usize = 20;
const RETRIEVAL_TOP_K: usize = 10;

#[derive(Debug, Error)]
pub(crate) enum EvalError {
    #[error("{0}")]
    Corpus(#[from] EvalCorpusError),
    #[error("{0}")]
    Threshold(#[from] EvalGateError),
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    SalienceGate(#[from] crate::GateError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("unknown sweep parameter `{0}`")]
    UnknownSweepParam(String),
    #[error("--range must be `start..end` with start < end, got `{0}`")]
    InvalidRange(String),
}

fn run_async<F, T, E>(future: F) -> Result<T, EvalError>
where
    F: std::future::Future<Output = Result<T, E>> + Send,
    T: Send,
    E: Send,
    EvalError: From<E>,
{
    block_on(future)
        .map_err(|error| EvalError::Runtime(error.to_string()))?
        .map_err(EvalError::from)
}

/// A self-cleaning temp directory for one eval store, matching the
/// `unique_temp_dir` idiom in `tests/cli.rs` but with `Drop`-based cleanup since
/// this runs as part of the production CLI, not a throwaway test process.
struct EvalTempDir(PathBuf);

impl EvalTempDir {
    fn new(prefix: &str) -> Result<Self, EvalError> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| EvalError::Runtime(error.to_string()))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "elegy-memory-eval-{prefix}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }

    fn db_path(&self) -> PathBuf {
        self.0.join("memory.sqlite3")
    }
}

impl Drop for EvalTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fresh_store(prefix: &str) -> Result<(EvalTempDir, SqliteMemoryStore), EvalError> {
    let temp_dir = EvalTempDir::new(prefix)?;
    let store = SqliteMemoryStore::new(temp_dir.db_path(), MemoryScope::Workspace)?;
    Ok((temp_dir, store))
}

fn build_memory(spec: &CorpusMemory, scope: MemoryScope) -> Memory {
    let now = chrono::Utc::now();
    Memory {
        id: Uuid::new_v4(),
        content: spec.content.clone(),
        summary: None,
        scope,
        memory_type: spec.memory_type,
        provenance: spec.provenance,
        importance_score: spec.importance,
        reliability_score: spec.provenance.base_reliability(),
        sensitivity: SensitivityLevel::Low,
        state: MemoryState::Active,
        tags: Vec::new(),
        status: None,
        custom_metadata: HashMap::new(),
        access_count: 0,
        corroboration_count: 0,
        embedding_stale: true,
        created_at: now,
        updated_at: now,
        last_accessed_at: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
    }
}

/// Assigns each memory in `memories` a home-axis embedding by position, except
/// memories with an `anchor`, which get a vector at the anchor's exact cosine
/// similarity to their target's home axis (see `eval::embedding`).
///
/// Anchors are not chained: an anchor always resolves against its target's home
/// axis, even if the target is itself anchored. Neither this corpus schema nor
/// any fixture in this crate constructs a chained anchor.
fn assign_axis_embeddings(memories: &[CorpusMemory]) -> HashMap<String, Vec<f32>> {
    let mut result = HashMap::with_capacity(memories.len());
    for (index, memory) in memories.iter().enumerate() {
        if memory.anchor.is_none() {
            result.insert(memory.key.clone(), axis_vector(index));
        }
    }
    for (index, memory) in memories.iter().enumerate() {
        if let Some(anchor) = &memory.anchor {
            let target_axis = memories
                .iter()
                .position(|candidate| candidate.key == anchor.key)
                .expect("corpus validation guarantees an anchor's target key exists");
            result.insert(
                memory.key.clone(),
                vector_at_similarity(target_axis, index, anchor.target_cosine),
            );
        }
    }
    result
}

struct InjectedRetrievalCorpus {
    key_to_id: HashMap<String, MemoryId>,
}

/// Injects every memory in `corpus` directly (bypassing the write-time gate) so
/// the golden corpus's ground truth is never perturbed by a gate decision such
/// as an unexpected merge between two similar golden entries. The last memory
/// in the corpus is deliberately left with a stale embedding, giving the stale
/// embedding ratio metric a known non-zero value to exercise.
fn inject_retrieval_corpus(
    store: &SqliteMemoryStore,
    corpus: &RetrievalCorpus,
) -> Result<InjectedRetrievalCorpus, EvalError> {
    let embeddings = assign_axis_embeddings(&corpus.memories);
    let mut key_to_id = HashMap::with_capacity(corpus.memories.len());
    let last_index = corpus.memories.len().saturating_sub(1);

    for (index, memory) in corpus.memories.iter().enumerate() {
        let candidate_memory = build_memory(memory, store.scope());
        let id = run_async(store.store(candidate_memory))?;
        if index != last_index {
            let embedding = embeddings
                .get(&memory.key)
                .expect("axis embedding assigned for every corpus memory");
            run_async(store.store_embedding(&id, embedding))?;
        }
        key_to_id.insert(memory.key.clone(), id);
    }

    Ok(InjectedRetrievalCorpus { key_to_id })
}

struct RetrievalMetrics {
    recall_at_k: f64,
    precision_at_k: f64,
    ndcg_at_k: f64,
    hallucination_rate: f64,
}

fn evaluate_retrieval_corpus(
    store: &SqliteMemoryStore,
    corpus: &RetrievalCorpus,
    injected: &InjectedRetrievalCorpus,
) -> Result<RetrievalMetrics, EvalError> {
    let mut recalls = Vec::with_capacity(corpus.queries.len());
    let mut precisions = Vec::with_capacity(corpus.queries.len());
    let mut ndcgs = Vec::with_capacity(corpus.queries.len());
    let mut retrieved_per_query: Vec<Vec<MemoryId>> = Vec::with_capacity(corpus.queries.len());
    let mut relevant_per_query: Vec<HashSet<MemoryId>> = Vec::with_capacity(corpus.queries.len());
    let mut contradicting_per_query: Vec<HashSet<MemoryId>> =
        Vec::with_capacity(corpus.queries.len());

    for query in &corpus.queries {
        let relevant_axes: Vec<usize> = query
            .relevant_keys
            .iter()
            .filter_map(|key| corpus.memory_index(key))
            .collect();
        let query_embedding = blended_vector(&relevant_axes);

        let search_query = SearchQuery {
            text: query.text.clone(),
            embedding: Some(query_embedding),
            scope: store.scope(),
            state_filter: Some(MemoryState::Active),
            type_filter: None,
            max_results: RETRIEVAL_TOP_K,
            context_config: None,
            session_id: None,
            agent_id: None,
        };

        let results: Vec<ScoredMemory> = run_async(store.search(search_query))?;
        let retrieved_ids: Vec<MemoryId> = results.iter().map(|scored| scored.memory.id).collect();

        let relevant_ids: HashSet<MemoryId> = query
            .relevant_keys
            .iter()
            .filter_map(|key| injected.key_to_id.get(key).copied())
            .collect();

        let mut grades: HashMap<MemoryId, u8> = HashMap::new();
        for memory in &corpus.memories {
            let grade = RetrievalCorpus::relevance_grade(query, &memory.key);
            if grade > 0 {
                if let Some(id) = injected.key_to_id.get(&memory.key) {
                    grades.insert(*id, grade);
                }
            }
        }

        let contradicting_ids: HashSet<MemoryId> = query
            .contradicts
            .iter()
            .filter_map(|key| injected.key_to_id.get(key).copied())
            .collect();

        recalls.push(metrics::recall_at_k(&retrieved_ids, &relevant_ids));
        precisions.push(metrics::precision_at_k(&retrieved_ids, &relevant_ids));
        ndcgs.push(metrics::ndcg_at_k(&retrieved_ids, &grades, RETRIEVAL_TOP_K));
        retrieved_per_query.push(retrieved_ids);
        relevant_per_query.push(relevant_ids);
        contradicting_per_query.push(contradicting_ids);
    }

    Ok(RetrievalMetrics {
        recall_at_k: metrics::mean(&recalls),
        precision_at_k: metrics::mean(&precisions),
        ndcg_at_k: metrics::mean(&ndcgs),
        hallucination_rate: metrics::hallucination_rate(
            &retrieved_per_query,
            &relevant_per_query,
            &contradicting_per_query,
        ),
    })
}

fn outcome_of(decision: &GateDecision) -> ExpectedGateOutcome {
    match decision {
        GateDecision::Accept { .. } => ExpectedGateOutcome::Accept,
        GateDecision::Archive => ExpectedGateOutcome::Archive,
        GateDecision::Merge { .. } => ExpectedGateOutcome::Merge,
        GateDecision::Contradiction { .. } => ExpectedGateOutcome::Contradiction,
        GateDecision::Reject { .. } => ExpectedGateOutcome::Reject,
    }
}

/// Runs every case in `gate_corpus` against a fresh database (one per case, so
/// cases never see each other's writes) and returns, per case, whether the
/// gate's actual decision matched the expected outcome.
///
/// Each case's candidate is evaluated through a store opened at the
/// candidate's own scope. An existing memory whose `scope` differs from the
/// candidate's is written through a *separate* `SqliteMemoryStore` instance
/// opened at that memory's scope but pointed at the same database file —
/// `SqliteMemoryStore::store()` rejects a memory whose scope doesn't match the
/// store's configured scope, so a single store instance cannot write into two
/// scopes. This is what makes `GateDecision::Reject` reachable: the gate's
/// novelty check sees existing memories through `visible_scopes()`, which
/// includes broader scopes, so a near-duplicate in a broader scope than the
/// candidate is exactly the scenario the gate's Reject branch exists for.
fn evaluate_gate_corpus(gate_corpus: &GateCorpus) -> Result<Vec<bool>, EvalError> {
    let mut correctness = Vec::with_capacity(gate_corpus.cases.len());

    for case in &gate_corpus.cases {
        let temp_dir = EvalTempDir::new("gate-case")?;
        let candidate_scope: MemoryScope = case.candidate.scope.into();
        let store = SqliteMemoryStore::new(temp_dir.db_path(), candidate_scope)?;

        let mut case_memories: Vec<CorpusMemory> = case.existing.clone();
        case_memories.push(case.candidate.clone());
        let embeddings = assign_axis_embeddings(&case_memories);

        for existing in &case.existing {
            let existing_scope: MemoryScope = existing.scope.into();
            let memory = build_memory(existing, existing_scope);
            let embedding = embeddings
                .get(&existing.key)
                .expect("axis embedding assigned for every existing memory")
                .clone();
            if existing_scope == candidate_scope {
                let id = run_async(store.store(memory))?;
                run_async(store.store_embedding(&id, &embedding))?;
            } else {
                let scoped_store = SqliteMemoryStore::new(temp_dir.db_path(), existing_scope)?;
                let id = run_async(scoped_store.store(memory))?;
                run_async(scoped_store.store_embedding(&id, &embedding))?;
            }
        }

        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let candidate_embedding = embeddings
            .get(&case.candidate.key)
            .expect("axis embedding assigned for the candidate")
            .clone();
        let candidate = MemoryCandidate {
            content: case.candidate.content.clone(),
            summary: None,
            memory_type: case.candidate.memory_type,
            provenance: case.candidate.provenance,
            importance_score: case.candidate.importance,
            sensitivity: SensitivityLevel::Low,
            tags: Vec::new(),
            custom_metadata: HashMap::new(),
            embedding: Some(candidate_embedding),
        };

        let decision = run_async(gate.evaluate(&candidate, &store))?;
        correctness.push(outcome_of(&decision) == case.expected);
    }

    Ok(correctness)
}

/// Times an isolated, semantically trivial write path (gate evaluation, store,
/// then embed) `WRITE_LATENCY_SAMPLE_COUNT` times against a fresh store. Kept
/// separate from golden-corpus injection so a latency measurement never risks
/// perturbing retrieval ground truth.
fn measure_write_latencies_ms() -> Result<Vec<f64>, EvalError> {
    let (_temp_dir, store) = fresh_store("write-latency")?;
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let mut samples = Vec::with_capacity(WRITE_LATENCY_SAMPLE_COUNT);

    for index in 0..WRITE_LATENCY_SAMPLE_COUNT {
        let embedding = axis_vector(index);
        let candidate = MemoryCandidate {
            content: format!("write latency probe memory number {index}"),
            summary: None,
            memory_type: MemoryType::Fact,
            provenance: ProvenanceLevel::UserStated,
            importance_score: 0.8,
            sensitivity: SensitivityLevel::Low,
            tags: Vec::new(),
            custom_metadata: HashMap::new(),
            embedding: Some(embedding.clone()),
        };

        let started = Instant::now();
        let decision = run_async(gate.evaluate(&candidate, &store))?;
        if let GateDecision::Accept { .. } = decision {
            let memory = Memory {
                id: Uuid::new_v4(),
                content: candidate.content.clone(),
                summary: None,
                scope: store.scope(),
                memory_type: candidate.memory_type,
                provenance: candidate.provenance,
                importance_score: candidate.importance_score,
                reliability_score: candidate.provenance.base_reliability(),
                sensitivity: candidate.sensitivity,
                state: MemoryState::Active,
                tags: Vec::new(),
                status: None,
                custom_metadata: HashMap::new(),
                access_count: 0,
                corroboration_count: 0,
                embedding_stale: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                last_accessed_at: None,
                tenant_id: None,
                user_id: None,
                agent_id: None,
            };
            let id = run_async(store.store(memory))?;
            run_async(store.store_embedding(&id, &embedding))?;
        }
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    Ok(samples)
}

/// Output of `measure_retrieval_latencies_at_scale_ms`.
struct ScaleMeasurement {
    retrieval_latencies_ms: Vec<f64>,
    /// Marginal bytes per memory — see the doc comment below for why this is
    /// measured here rather than against the tiny golden corpus.
    storage_efficiency_bytes_per_memory: f64,
}

/// Bulk-injects `RETRIEVAL_SCALE_MEMORY_COUNT` trivial memories (embeddings reused
/// cyclically across a small set of axes — exact discrimination does not matter
/// for a raw scale/latency measurement), times `RETRIEVAL_SCALE_QUERY_COUNT`
/// searches against them, and measures marginal storage cost across the same
/// bulk insert.
///
/// Storage efficiency is measured here, not against the ~28-memory golden
/// corpus, because `vec0`'s chunked storage allocates space for a batch of
/// rows at once — the first insert into an empty `vec_memories` table pays a
/// one-time chunk-allocation cost that a tiny corpus divides by too few
/// memories to be representative (confirmed empirically: the same
/// baseline-subtraction technique that correctly isolated marginal cost
/// against the plain-table fallback still measured >100 KB/memory against a
/// real vec0 table at N=28, because one chunk's allocation dominates a
/// 28-row sample). At `RETRIEVAL_SCALE_MEMORY_COUNT`, the same one-time cost
/// amortizes across enough rows to reflect steady-state marginal cost.
fn measure_retrieval_latencies_at_scale_ms() -> Result<ScaleMeasurement, EvalError> {
    let (_temp_dir, store) = fresh_store("retrieval-scale")?;
    let usable_axes = EVAL_EMBEDDING_DIMENSIONS - 1;
    let baseline_storage_bytes = run_async(store.health_report())?.total_storage_bytes;

    for index in 0..RETRIEVAL_SCALE_MEMORY_COUNT {
        let memory = Memory {
            id: Uuid::new_v4(),
            content: format!("bulk scale memory number {index}"),
            summary: None,
            scope: store.scope(),
            memory_type: MemoryType::Observation,
            provenance: ProvenanceLevel::AgentObserved,
            importance_score: 0.5,
            reliability_score: ProvenanceLevel::AgentObserved.base_reliability(),
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: Vec::new(),
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_accessed_at: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
        };
        let id = run_async(store.store(memory))?;
        let embedding = axis_vector(index % usable_axes);
        run_async(store.store_embedding(&id, &embedding))?;
    }

    let health_after_insert = run_async(store.health_report())?;
    let marginal_storage_bytes = health_after_insert
        .total_storage_bytes
        .saturating_sub(baseline_storage_bytes);
    let storage_efficiency_bytes_per_memory = metrics::ratio(
        marginal_storage_bytes,
        health_after_insert.active_count.max(1),
    );

    let mut samples = Vec::with_capacity(RETRIEVAL_SCALE_QUERY_COUNT);
    for query_index in 0..RETRIEVAL_SCALE_QUERY_COUNT {
        let search_query = SearchQuery {
            text: format!("bulk scale query {query_index}"),
            embedding: Some(axis_vector(query_index % usable_axes)),
            scope: store.scope(),
            state_filter: Some(MemoryState::Active),
            type_filter: None,
            max_results: RETRIEVAL_TOP_K,
            context_config: None,
            session_id: None,
            agent_id: None,
        };
        let started = Instant::now();
        let _results: Vec<ScoredMemory> = run_async(store.search(search_query))?;
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    Ok(ScaleMeasurement {
        retrieval_latencies_ms: samples,
        storage_efficiency_bytes_per_memory,
    })
}

/// One computed metric, compared against its configured gate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvalMetricResult {
    pub name: String,
    pub value: f64,
    pub threshold: f64,
    pub comparison: String,
    pub passed: bool,
}

/// Full result of one `eval run`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvalReport {
    pub metrics: Vec<EvalMetricResult>,
    pub passed: bool,
    pub duration_ms: u64,
}

fn gate_metric(
    thresholds: &EvalGateThresholds,
    name: &str,
    value: f64,
) -> Result<EvalMetricResult, EvalError> {
    let (spec, passed) = thresholds
        .evaluate(name, value)
        .ok_or_else(|| EvalError::Threshold(EvalGateError::MissingMetric(name.to_string())))?;
    Ok(EvalMetricResult {
        name: name.to_string(),
        value,
        threshold: spec.value,
        comparison: match spec.comparison {
            gates::Comparison::Gte => "gte".to_string(),
            gates::Comparison::Lte => "lte".to_string(),
        },
        passed,
    })
}

/// Options for `eval run`.
pub(crate) struct EvalRunOptions {
    pub corpus_path: Option<PathBuf>,
    pub thresholds_path: Option<PathBuf>,
}

/// Runs the full harness: golden retrieval corpus, 8:1 synthetic gate-accuracy
/// corpus, the hand-authored full-coverage gate-decision corpus, an isolated
/// write-latency pass, and a bulk retrieval-at-scale pass. Computes and gates
/// every metric in `gates::REQUIRED_METRIC_NAMES`.
pub(crate) fn run_eval(options: &EvalRunOptions) -> Result<EvalReport, EvalError> {
    let started = Instant::now();

    let thresholds = match &options.thresholds_path {
        Some(path) => gates::load_thresholds(path)?,
        None => gates::embedded_default_thresholds(),
    };

    let retrieval_corpus = match &options.corpus_path {
        Some(path) => corpus::load_retrieval_corpus(path)?,
        None => corpus::embedded_golden_corpus(),
    };

    let (_temp_dir, store) = fresh_store("run")?;
    let injected = inject_retrieval_corpus(&store, &retrieval_corpus)?;
    let retrieval_metrics = evaluate_retrieval_corpus(&store, &retrieval_corpus, &injected)?;

    let health = run_async(store.health_report())?;
    let stale_embedding_ratio =
        metrics::ratio(health.stale_embeddings_count, health.active_count.max(1));

    let gate_corpus = synthetic::generate_gate_corpus(synthetic::DistractorRatio::EightToOne);
    let gate_correctness = evaluate_gate_corpus(&gate_corpus)?;
    let gate_accuracy = metrics::accuracy(&gate_correctness);

    // The hand-authored corpus, unlike the synthetic 8:1 corpus, exercises all
    // five GateDecision variants (Accept/Archive/Merge/Contradiction/Reject) —
    // the synthetic generator deliberately never reaches Merge/Contradiction/
    // Reject (see synthetic.rs's module docs on the 0.75 cosine ceiling).
    let full_gate_corpus = corpus::embedded_gate_corpus();
    let full_gate_correctness = evaluate_gate_corpus(&full_gate_corpus)?;
    let full_gate_accuracy = metrics::accuracy(&full_gate_correctness);

    let write_latencies_ms = measure_write_latencies_ms()?;
    let scale_measurement = measure_retrieval_latencies_at_scale_ms()?;
    let retrieval_latencies_ms = &scale_measurement.retrieval_latencies_ms;

    let metric_results = vec![
        gate_metric(&thresholds, "recall_at_10", retrieval_metrics.recall_at_k)?,
        gate_metric(
            &thresholds,
            "precision_at_10",
            retrieval_metrics.precision_at_k,
        )?,
        gate_metric(&thresholds, "ndcg_at_10", retrieval_metrics.ndcg_at_k)?,
        // Rank-sensitive by definition (metrics::hallucination_rate): a
        // contradicting memory merely co-occurring in top-k does not count,
        // only one that outranks the correct answer or stands in for a
        // missing one does. See the doc comment on that function.
        gate_metric(
            &thresholds,
            "hallucination_rate",
            retrieval_metrics.hallucination_rate,
        )?,
        gate_metric(&thresholds, "gate_accuracy_8to1", gate_accuracy)?,
        gate_metric(&thresholds, "gate_accuracy_full", full_gate_accuracy)?,
        gate_metric(
            &thresholds,
            "write_latency_p50_ms",
            metrics::percentile_ms(&write_latencies_ms, 50.0),
        )?,
        gate_metric(
            &thresholds,
            "write_latency_p95_ms",
            metrics::percentile_ms(&write_latencies_ms, 95.0),
        )?,
        // load_vector_similarity_scores now issues a real vec0 KNN query
        // (MATCH + k = ?) instead of decoding every candidate embedding and
        // computing cosine similarity in Rust — see
        // docs/specs/eval-harness-v1/spec.md for the measured before/after.
        // This meets the spec's original aspirational 100/200ms target, so
        // unlike storage efficiency below there is nothing calibrated here.
        gate_metric(
            &thresholds,
            "retrieval_latency_p50_ms",
            metrics::percentile_ms(retrieval_latencies_ms, 50.0),
        )?,
        gate_metric(
            &thresholds,
            "retrieval_latency_p95_ms",
            metrics::percentile_ms(retrieval_latencies_ms, 95.0),
        )?,
        // Marginal cost, measured at RETRIEVAL_SCALE_MEMORY_COUNT rather than
        // the golden corpus — see measure_retrieval_latencies_at_scale_ms's
        // doc comment for why: vec0's chunked storage allocates space for a
        // batch of rows on the first insert, and that one-time cost needs
        // enough rows to amortize before the ratio is representative. Still
        // calibrated well above the spec's aspirational 2 KB — real vec0
        // storage overhead is substantially higher than the plain-table
        // fallback's ~4 KB/memory floor, and reaching 2 KB needs quantizing
        // the stored embedding regardless (tracked separately).
        gate_metric(
            &thresholds,
            "storage_efficiency_bytes_per_memory",
            scale_measurement.storage_efficiency_bytes_per_memory,
        )?,
        gate_metric(&thresholds, "stale_embedding_ratio", stale_embedding_ratio)?,
    ];

    let passed = metric_results.iter().all(|metric| metric.passed);

    Ok(EvalReport {
        metrics: metric_results,
        passed,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// A `--param` name accepted by `eval sweep-threshold`, mapped to the
/// corresponding `ScopeConfig` field. Values are applied to a fresh store
/// *after* construction, via a direct SQL update to the persisted
/// `scope_config` table — `storage::schema::initialize_scope_config` rewrites a
/// handful of legacy keys (including `merge_similarity_threshold` and
/// `novelty_doubt_threshold`) on every `init_database` call, so any override
/// applied before store construction would be silently discarded.
fn known_sweep_params() -> &'static [&'static str] {
    &[
        "merge_similarity_threshold",
        "novelty_doubt_threshold",
        "duplicate_similarity_threshold",
        "salience_threshold",
        "agent_inferred_importance_threshold",
        "decay_lambda_base",
    ]
}

/// Writes `value` for `key` directly into the `scope_config` table via a second
/// connection to the same database file — the same technique `cli.rs`'s
/// `execute_reembed_command` already uses for read-only diagnostic queries
/// against the store's persisted tables.
fn override_scope_config(db_path: &Path, key: &str, value: f32) -> Result<(), EvalError> {
    let connection = rusqlite::Connection::open(db_path)?;
    connection.execute(
        "UPDATE scope_config SET value = ?2 WHERE key = ?1",
        rusqlite::params![key, value.to_string()],
    )?;
    Ok(())
}

/// One point in a `--range start..end` sweep.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SweepPoint {
    pub param_value: f32,
    pub gate_accuracy_8to1: f64,
    pub recall_at_10: f64,
    pub precision_at_10: f64,
}

/// Result of `eval sweep-threshold`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SweepReport {
    pub param: String,
    pub points: Vec<SweepPoint>,
}

fn parse_range(range: &str) -> Result<(f32, f32), EvalError> {
    let (start, end) = range
        .split_once("..")
        .ok_or_else(|| EvalError::InvalidRange(range.to_string()))?;
    let start: f32 = start
        .trim()
        .parse()
        .map_err(|_| EvalError::InvalidRange(range.to_string()))?;
    let end: f32 = end
        .trim()
        .parse()
        .map_err(|_| EvalError::InvalidRange(range.to_string()))?;
    if !(start.is_finite() && end.is_finite()) || start >= end {
        return Err(EvalError::InvalidRange(range.to_string()));
    }
    Ok((start, end))
}

const SWEEP_STEPS: usize = 5;

/// Sweeps one `ScopeConfig` threshold parameter across `range` (`"start..end"`),
/// reporting gate accuracy and retrieval quality at each point, against the
/// embedded corpora — never against the checked-in threshold file, satisfying
/// the spec's "runs against a changed threshold without modifying the checked-in
/// corpus."
pub(crate) fn sweep_threshold(param: &str, range: &str) -> Result<SweepReport, EvalError> {
    if !known_sweep_params().contains(&param) {
        return Err(EvalError::UnknownSweepParam(param.to_string()));
    }
    let (start, end) = parse_range(range)?;

    let retrieval_corpus = corpus::embedded_golden_corpus();
    let mut points = Vec::with_capacity(SWEEP_STEPS);

    for step in 0..SWEEP_STEPS {
        let fraction = step as f32 / (SWEEP_STEPS - 1) as f32;
        let param_value = start + fraction * (end - start);

        let (temp_dir, store) = fresh_store("sweep")?;
        override_scope_config(&temp_dir.db_path(), param, param_value)?;

        let injected = inject_retrieval_corpus(&store, &retrieval_corpus)?;
        let retrieval_metrics = evaluate_retrieval_corpus(&store, &retrieval_corpus, &injected)?;

        let gate_corpus = synthetic::generate_gate_corpus(synthetic::DistractorRatio::EightToOne);
        let gate_correctness = evaluate_gate_corpus(&gate_corpus)?;

        points.push(SweepPoint {
            param_value,
            gate_accuracy_8to1: metrics::accuracy(&gate_correctness),
            recall_at_10: retrieval_metrics.recall_at_k,
            precision_at_10: retrieval_metrics.precision_at_k,
        });
    }

    Ok(SweepReport {
        param: param.to_string(),
        points,
    })
}

/// Descriptor for one corpus reachable through `eval list-corpora`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CorpusDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub item_count: usize,
}

/// Lists every corpus the harness can run against: the two embedded, hand-authored
/// corpora, and the three generated synthetic distractor ratios.
pub(crate) fn list_corpora() -> Vec<CorpusDescriptor> {
    let golden = corpus::embedded_golden_corpus();
    let gate = corpus::embedded_gate_corpus();

    let mut descriptors = vec![
        CorpusDescriptor {
            name: "golden-v1",
            description: "Hand-authored recall/precision/NDCG/hallucination retrieval corpus",
            source: "embedded",
            item_count: golden.queries.len(),
        },
        CorpusDescriptor {
            name: "gate-decisions-v1",
            description: "Hand-authored write-time gate decision corpus",
            source: "embedded",
            item_count: gate.cases.len(),
        },
    ];

    for ratio in [
        synthetic::DistractorRatio::OneToOne,
        synthetic::DistractorRatio::FourToOne,
        synthetic::DistractorRatio::EightToOne,
    ] {
        descriptors.push(CorpusDescriptor {
            name: match ratio {
                synthetic::DistractorRatio::OneToOne => "synthetic-distractors-1to1",
                synthetic::DistractorRatio::FourToOne => "synthetic-distractors-4to1",
                synthetic::DistractorRatio::EightToOne => "synthetic-distractors-8to1",
            },
            description: "Generated Write-Time Gating-adapted synthetic distractor corpus (locally adapted, not benchmark parity)",
            source: "generated",
            item_count: synthetic::generate_gate_corpus(ratio).cases.len(),
        });
    }

    descriptors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_gate_corpus_reaches_every_expected_decision() {
        let gate_corpus = corpus::embedded_gate_corpus();
        let correctness =
            evaluate_gate_corpus(&gate_corpus).expect("evaluate the embedded gate corpus");
        for (case, correct) in gate_corpus.cases.iter().zip(&correctness) {
            assert!(
                *correct,
                "case `{}` expected {:?} but the gate reached a different decision",
                case.key, case.expected
            );
        }
        // All five GateDecision variants must actually be exercised at least
        // once, including Reject — otherwise this fixture would silently stop
        // covering a disposition without anyone noticing.
        for expected in [
            ExpectedGateOutcome::Accept,
            ExpectedGateOutcome::Archive,
            ExpectedGateOutcome::Merge,
            ExpectedGateOutcome::Contradiction,
            ExpectedGateOutcome::Reject,
        ] {
            assert!(
                gate_corpus
                    .cases
                    .iter()
                    .any(|case| case.expected == expected),
                "embedded gate corpus never exercises {expected:?}"
            );
        }
    }

    #[test]
    fn run_eval_against_the_embedded_corpus_computes_every_required_metric() {
        let options = EvalRunOptions {
            corpus_path: None,
            thresholds_path: None,
        };
        let report = run_eval(&options).expect("eval run must succeed against the embedded corpus");
        assert_eq!(report.metrics.len(), gates::REQUIRED_METRIC_NAMES.len());
        for required in gates::REQUIRED_METRIC_NAMES {
            assert!(
                report.metrics.iter().any(|metric| metric.name == *required),
                "missing computed metric {required}"
            );
        }

        let metric_value = |name: &str| {
            report
                .metrics
                .iter()
                .find(|metric| metric.name == name)
                .unwrap_or_else(|| panic!("metric {name} must be present"))
                .value
        };
        assert_eq!(
            metric_value("hallucination_rate"),
            0.0,
            "the golden corpus's two contradicting memories are anchored at 0.1 cosine \
             similarity, well below their correct counterparts' 1.0 — none should ever \
             outrank the correct answer"
        );
        // Real vec0 KNN, not the old Rust-side brute-force scan: comfortably
        // under the spec's original aspirational target (100/200ms), not just
        // a calibrated compromise. Bounded rather than pinned to an exact
        // value, unlike storage efficiency below — this is a real timing
        // measurement, not deterministic arithmetic.
        assert!(
            metric_value("retrieval_latency_p50_ms") < 100.0,
            "measured {}",
            metric_value("retrieval_latency_p50_ms")
        );
        assert!(
            metric_value("retrieval_latency_p95_ms") < 200.0,
            "measured {}",
            metric_value("retrieval_latency_p95_ms")
        );

        assert!(
            report.passed,
            "eval run against the embedded corpus and calibrated thresholds must pass: {report:#?}"
        );
    }

    #[test]
    fn sweep_threshold_rejects_an_unknown_param() {
        let error = sweep_threshold("not_a_real_param", "0.1..0.2").expect_err("must reject");
        assert!(matches!(error, EvalError::UnknownSweepParam(_)));
    }

    #[test]
    fn sweep_threshold_rejects_a_malformed_range() {
        let error =
            sweep_threshold("merge_similarity_threshold", "0.9-0.8").expect_err("must reject");
        assert!(matches!(error, EvalError::InvalidRange(_)));
        let error = sweep_threshold("merge_similarity_threshold", "0.9..0.2")
            .expect_err("must reject reversed range");
        assert!(matches!(error, EvalError::InvalidRange(_)));
    }

    #[test]
    fn list_corpora_reports_all_five_corpora() {
        let descriptors = list_corpora();
        assert_eq!(descriptors.len(), 5);
        assert!(descriptors
            .iter()
            .all(|descriptor| descriptor.item_count > 0));
    }
}
