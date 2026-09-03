//! Corpus schema and loaders for the eval harness.
//!
//! Two hand-authored, embedded corpora ship with the crate (`fixtures/eval/`):
//! a retrieval corpus (recall/precision/NDCG/hallucination) and a gate-decision
//! corpus (write-time gate accuracy). Both are plain JSON, matching the
//! repo-wide convention (`docs/repo-layout.md`) that no `.jsonl` fixtures exist.

use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{MemoryType, ProvenanceLevel};

/// Errors produced while loading or validating an eval corpus file.
#[derive(Debug, Error)]
pub(crate) enum EvalCorpusError {
    #[error("failed to read corpus file {0}: {1}")]
    Io(String, String),
    #[error("failed to parse corpus file {0}: {1}")]
    Parse(String, String),
    #[error("corpus is internally inconsistent: {0}")]
    Inconsistent(String),
}

/// A retrieval-quality corpus: memories to inject plus queries with ground-truth
/// relevance, used for recall@k, precision@k, NDCG@k, and hallucination rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RetrievalCorpus {
    pub version: String,
    pub memories: Vec<CorpusMemory>,
    pub queries: Vec<CorpusQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CorpusMemory {
    pub key: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub provenance: ProvenanceLevel,
    pub importance: f32,
    /// When set, this memory's embedding is engineered to sit at an exact cosine
    /// similarity to another memory's embedding (see `eval::embedding::vector_at_similarity`)
    /// instead of getting its own independent home axis. Used for hallucination-rate
    /// near-duplicates (`target_cosine` ~0.9) and for the synthetic distractor
    /// generator's uniform similarity spread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<MemoryAnchor>,
}

/// Engineers a memory's embedding at `target_cosine` similarity to the memory
/// identified by `key`, within the same corpus or gate case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryAnchor {
    pub key: String,
    pub target_cosine: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CorpusQuery {
    pub key: String,
    pub text: String,
    /// Ground-truth relevant memory keys for this query (order does not matter;
    /// use `relevance_grades` to express graded relevance for NDCG).
    pub relevant_keys: Vec<String>,
    /// Graded relevance (0-3) per memory key, for NDCG@k. A key in `relevant_keys`
    /// with no entry here defaults to grade 1.
    #[serde(default)]
    pub relevance_grades: HashMap<String, u8>,
    /// Memory keys that represent a hallucinated (contradictory) retrieval if they
    /// appear in this query's top-k results.
    #[serde(default)]
    pub contradicts: Vec<String>,
}

impl RetrievalCorpus {
    /// Returns the 0-based index of the memory with the given key, used as its
    /// embedding axis (see `eval::embedding`).
    pub fn memory_index(&self, key: &str) -> Option<usize> {
        self.memories.iter().position(|memory| memory.key == key)
    }

    /// The effective NDCG relevance grade for `key` against `query`: an explicit
    /// grade if present, else 1 if the key is in `relevant_keys`, else 0.
    pub fn relevance_grade(query: &CorpusQuery, key: &str) -> u8 {
        if let Some(grade) = query.relevance_grades.get(key) {
            return *grade;
        }
        u8::from(query.relevant_keys.iter().any(|relevant| relevant == key))
    }
}

fn validate_retrieval_corpus(corpus: &RetrievalCorpus) -> Result<(), EvalCorpusError> {
    for query in &corpus.queries {
        for key in &query.relevant_keys {
            if corpus.memory_index(key).is_none() {
                return Err(EvalCorpusError::Inconsistent(format!(
                    "query `{}` references unknown relevant memory key `{key}`",
                    query.key
                )));
            }
        }
        for key in &query.contradicts {
            if corpus.memory_index(key).is_none() {
                return Err(EvalCorpusError::Inconsistent(format!(
                    "query `{}` references unknown contradicts memory key `{key}`",
                    query.key
                )));
            }
        }
        for key in query.relevance_grades.keys() {
            if corpus.memory_index(key).is_none() {
                return Err(EvalCorpusError::Inconsistent(format!(
                    "query `{}` has a relevance grade for unknown memory key `{key}`",
                    query.key
                )));
            }
        }
    }
    for memory in &corpus.memories {
        if let Some(anchor) = &memory.anchor {
            if corpus.memory_index(&anchor.key).is_none() {
                return Err(EvalCorpusError::Inconsistent(format!(
                    "memory `{}` is anchored to unknown memory key `{}`",
                    memory.key, anchor.key
                )));
            }
        }
    }
    Ok(())
}

/// Loads the embedded golden retrieval corpus shipped with the crate.
///
/// # Panics
/// Panics if the embedded fixture fails to parse or is internally inconsistent —
/// that is a fixture-authoring bug caught at first use, not a runtime input error.
pub(crate) fn embedded_golden_corpus() -> RetrievalCorpus {
    let corpus: RetrievalCorpus =
        serde_json::from_str(include_str!("../../fixtures/eval/golden-v1.json"))
            .expect("embedded golden-v1.json corpus must deserialize");
    validate_retrieval_corpus(&corpus).expect("embedded golden-v1.json corpus must be internally consistent");
    corpus
}

/// Loads a retrieval corpus from a caller-supplied file path (`eval run --corpus <path>`).
pub(crate) fn load_retrieval_corpus(path: &Path) -> Result<RetrievalCorpus, EvalCorpusError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| EvalCorpusError::Io(path.display().to_string(), error.to_string()))?;
    let corpus: RetrievalCorpus = serde_json::from_str(&raw)
        .map_err(|error| EvalCorpusError::Parse(path.display().to_string(), error.to_string()))?;
    validate_retrieval_corpus(&corpus)?;
    Ok(corpus)
}

/// A write-time gate-accuracy corpus: for each case, a set of pre-seeded existing
/// memories, one candidate, and the `GateDecision` variant the gate is expected to
/// produce.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GateCorpus {
    pub version: String,
    pub cases: Vec<GateCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GateCase {
    pub key: String,
    /// Memories seeded into a fresh store before the candidate is evaluated.
    #[serde(default)]
    pub existing: Vec<CorpusMemory>,
    pub candidate: CorpusMemory,
    pub expected: ExpectedGateOutcome,
}

/// The `GateDecision` variant kind expected for a `GateCase`. Compared by
/// discriminant only — `runner.rs` does not assert on decision payload fields
/// (e.g. which existing memory a `Merge` targets), only on which of the five
/// dispositions the gate reached.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedGateOutcome {
    Accept,
    Archive,
    Merge,
    Contradiction,
    Reject,
}

fn validate_gate_corpus(corpus: &GateCorpus) -> Result<(), EvalCorpusError> {
    for case in &corpus.cases {
        if let Some(anchor) = &case.candidate.anchor {
            let known = case.existing.iter().any(|memory| memory.key == anchor.key)
                || case.candidate.key == anchor.key;
            if !known {
                return Err(EvalCorpusError::Inconsistent(format!(
                    "gate case `{}` candidate is anchored to unknown key `{}`",
                    case.key, anchor.key
                )));
            }
        }
        for memory in &case.existing {
            if let Some(anchor) = &memory.anchor {
                let known = case.existing.iter().any(|other| other.key == anchor.key);
                if !known {
                    return Err(EvalCorpusError::Inconsistent(format!(
                        "gate case `{}` existing memory `{}` is anchored to unknown key `{}`",
                        case.key, memory.key, anchor.key
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Loads the embedded golden gate-decision corpus shipped with the crate.
///
/// # Panics
/// Panics if the embedded fixture fails to parse or is internally inconsistent.
pub(crate) fn embedded_gate_corpus() -> GateCorpus {
    let corpus: GateCorpus =
        serde_json::from_str(include_str!("../../fixtures/eval/gate-decisions-v1.json"))
            .expect("embedded gate-decisions-v1.json corpus must deserialize");
    validate_gate_corpus(&corpus).expect("embedded gate-decisions-v1.json corpus must be internally consistent");
    corpus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_golden_corpus_loads_and_validates() {
        let corpus = embedded_golden_corpus();
        assert!(!corpus.memories.is_empty());
        assert!(!corpus.queries.is_empty());
    }

    #[test]
    fn embedded_gate_corpus_loads_and_validates() {
        let corpus = embedded_gate_corpus();
        assert!(!corpus.cases.is_empty());
    }

    #[test]
    fn relevance_grade_defaults_to_one_for_relevant_keys_without_explicit_grade() {
        let query = CorpusQuery {
            key: "q1".to_string(),
            text: "test".to_string(),
            relevant_keys: vec!["m1".to_string()],
            relevance_grades: HashMap::new(),
            contradicts: Vec::new(),
        };
        assert_eq!(RetrievalCorpus::relevance_grade(&query, "m1"), 1);
        assert_eq!(RetrievalCorpus::relevance_grade(&query, "m2"), 0);
    }

    #[test]
    fn load_retrieval_corpus_rejects_dangling_relevant_key() {
        let dir = std::env::temp_dir().join(format!(
            "elegy-memory-eval-corpus-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("bad-corpus.json");
        std::fs::write(
            &path,
            r#"{"version":"t1","memories":[],"queries":[{"key":"q1","text":"x","relevantKeys":["missing"]}]}"#,
        )
        .expect("write bad corpus fixture");
        let result = load_retrieval_corpus(&path);
        assert!(matches!(result, Err(EvalCorpusError::Inconsistent(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
