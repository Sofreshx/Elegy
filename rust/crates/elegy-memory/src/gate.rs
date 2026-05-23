use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;

use crate::{
    embedding::{prepare_embedding_input, EmbeddingTask},
    EmbeddingProvider, GateDecision, GateError, LlmProvider, MemoryCandidate, MemoryId,
    MemoryScope, MemoryStore, ProvenanceLevel, SalienceGate, ScopeConfig,
};

/// Default MVP salience gate using scope-configured novelty and salience thresholds.
#[derive(Clone)]
pub struct DefaultSalienceGate {
    scope_config: ScopeConfig,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    llm_provider: Option<Arc<dyn LlmProvider>>,
}

impl std::fmt::Debug for DefaultSalienceGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultSalienceGate")
            .field("scope_config", &self.scope_config)
            .field("has_embedding_provider", &self.embedding_provider.is_some())
            .field("has_llm_provider", &self.llm_provider.is_some())
            .finish()
    }
}

impl DefaultSalienceGate {
    const HIGH_SIMILARITY_REPLACE_THRESHOLD: f32 = 0.95;

    /// Create a new salience gate from an already-loaded scope configuration.
    #[must_use]
    pub fn new(scope_config: ScopeConfig) -> Self {
        Self::new_with_optional_providers(scope_config, None, None)
    }

    /// Create a new salience gate with an embedding provider used when candidates omit embeddings.
    #[must_use]
    pub fn new_with_embedding_provider(
        scope_config: ScopeConfig,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self::new_with_optional_providers(scope_config, Some(embedding_provider), None)
    }

    /// Create a new salience gate with an LLM provider used for contradiction classification.
    #[must_use]
    pub fn new_with_llm_provider(
        scope_config: ScopeConfig,
        llm_provider: Arc<dyn LlmProvider>,
    ) -> Self {
        Self::new_with_optional_providers(scope_config, None, Some(llm_provider))
    }

    /// Create a new salience gate with explicit embedding and LLM providers.
    #[must_use]
    pub fn new_with_providers(
        scope_config: ScopeConfig,
        embedding_provider: Arc<dyn EmbeddingProvider>,
        llm_provider: Arc<dyn LlmProvider>,
    ) -> Self {
        Self::new_with_optional_providers(
            scope_config,
            Some(embedding_provider),
            Some(llm_provider),
        )
    }

    /// Create a new salience gate with an optional embedding provider.
    #[must_use]
    pub fn new_with_optional_embedding_provider(
        scope_config: ScopeConfig,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Self {
        Self::new_with_optional_providers(scope_config, embedding_provider, None)
    }

    /// Create a new salience gate with optional embedding and LLM providers.
    #[must_use]
    pub fn new_with_optional_providers(
        scope_config: ScopeConfig,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
        llm_provider: Option<Arc<dyn LlmProvider>>,
    ) -> Self {
        Self {
            scope_config,
            embedding_provider,
            llm_provider,
        }
    }

    fn validate_candidate(&self, candidate: &MemoryCandidate) -> Result<(), GateError> {
        if candidate.content.trim().is_empty() {
            return Err(GateError::InvalidCandidate(
                "candidate content must not be empty".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&candidate.importance_score)
            || !candidate.importance_score.is_finite()
        {
            return Err(GateError::InvalidCandidate(
                "candidate importance_score must be finite and within 0.0..=1.0".to_string(),
            ));
        }
        if candidate.embedding.as_ref().is_some_and(Vec::is_empty) {
            return Err(GateError::InvalidCandidate(
                "candidate embedding must not be empty when provided".to_string(),
            ));
        }

        Ok(())
    }

    fn novelty_floor(&self) -> f32 {
        self.scope_config
            .novelty_doubt_threshold
            .clamp(0.0, 1.0)
            .min(self.scope_config.merge_similarity_threshold.clamp(0.0, 1.0))
    }

    fn should_merge(&self, similarity: f32) -> bool {
        similarity >= self.scope_config.merge_similarity_threshold.clamp(0.0, 1.0)
    }

    fn is_likely_duplicate(&self, similarity: f32) -> bool {
        let novelty_floor = self.novelty_floor();
        let merge_threshold = self.scope_config.merge_similarity_threshold.clamp(0.0, 1.0);

        similarity >= novelty_floor && similarity < merge_threshold
    }

    fn merge_content(existing_content: &str, candidate_content: &str, similarity: f32) -> String {
        let existing_content = existing_content.trim();
        let candidate_content = candidate_content.trim();
        let normalized_existing = normalize_for_merge(existing_content);
        let normalized_candidate = normalize_for_merge(candidate_content);

        if similarity >= Self::HIGH_SIMILARITY_REPLACE_THRESHOLD {
            return candidate_content.to_string();
        }
        if normalized_existing == normalized_candidate {
            return existing_content.to_string();
        }
        if normalized_candidate.contains(&normalized_existing)
            || candidate_is_clearly_more_detailed(existing_content, candidate_content)
            || candidate_adds_material_search_terms(existing_content, candidate_content)
        {
            return candidate_content.to_string();
        }
        existing_content.to_string()
    }

    fn contradiction_description(
        existing_content: &str,
        candidate_content: &str,
    ) -> Option<String> {
        detect_technology_contradiction(existing_content, candidate_content)
            .or_else(|| detect_numeric_contradiction(existing_content, candidate_content))
    }

    async fn llm_contradiction_verdict(
        &self,
        existing_content: &str,
        candidate_content: &str,
    ) -> Option<LlmContradictionVerdict> {
        let provider = self.llm_provider.as_ref()?;
        let prompt = build_contradiction_prompt(existing_content, candidate_content);
        match provider.complete(&prompt).await {
            Ok(response) => match parse_contradiction_response(&response) {
                Some(verdict) => Some(verdict),
                None => {
                    eprintln!(
                        "warning: {} ({}) returned an unusable contradiction verdict; falling back to heuristic contradiction detection",
                        provider.name(),
                        provider.model()
                    );
                    None
                }
            },
            Err(error) => {
                eprintln!(
                    "warning: {} ({}) contradiction check failed: {error}. Falling back to heuristic contradiction detection.",
                    provider.name(),
                    provider.model()
                );
                None
            }
        }
    }

    async fn novelty_embedding<'a>(
        &'a self,
        candidate: &'a MemoryCandidate,
    ) -> Option<Cow<'a, [f32]>> {
        if let Some(embedding) = candidate.embedding.as_deref() {
            return Some(Cow::Borrowed(embedding));
        }

        let trimmed_content = candidate.content.trim();
        if trimmed_content.is_empty() {
            return None;
        }

        let provider = self.embedding_provider.as_ref()?;
        let prepared_input =
            prepare_embedding_input(provider.as_ref(), EmbeddingTask::Document, trimmed_content);
        match provider.embed(prepared_input.as_ref()).await {
            Ok(embedding) if !embedding.is_empty() => Some(Cow::Owned(embedding)),
            Ok(_) | Err(_) => None,
        }
    }
}

#[async_trait]
impl SalienceGate for DefaultSalienceGate {
    async fn evaluate(
        &self,
        candidate: &MemoryCandidate,
        store: &dyn MemoryStore,
    ) -> Result<GateDecision, GateError> {
        self.evaluate_internal(candidate, store, None).await
    }
}

impl DefaultSalienceGate {
    /// Evaluate a candidate while excluding one existing memory from duplicate checks.
    pub async fn evaluate_excluding(
        &self,
        candidate: &MemoryCandidate,
        store: &dyn MemoryStore,
        excluded_id: MemoryId,
    ) -> Result<GateDecision, GateError> {
        self.evaluate_internal(candidate, store, Some(excluded_id))
            .await
    }

    async fn evaluate_internal(
        &self,
        candidate: &MemoryCandidate,
        store: &dyn MemoryStore,
        excluded_id: Option<MemoryId>,
    ) -> Result<GateDecision, GateError> {
        self.validate_candidate(candidate)?;

        let mut likely_duplicate = None;
        let candidate_scope = store.scope();

        if let Some(embedding) = self.novelty_embedding(candidate).await {
            let matches = store
                .find_similar(
                    embedding.as_ref(),
                    self.novelty_floor(),
                    if excluded_id.is_some() { 2 } else { 1 },
                )
                .await?;
            if let Some(best_match) = matches
                .into_iter()
                .find(|scored| excluded_id != Some(scored.memory.id))
            {
                if best_match.memory.scope.rank() > candidate_scope.rank() {
                    return Ok(GateDecision::Reject {
                        reason: format!(
                            "near-duplicate already exists in higher scope {} ({})",
                            display_scope(best_match.memory.scope),
                            best_match.memory.id
                        ),
                    });
                }

                if self.should_merge(best_match.similarity) {
                    if let Some(verdict) = self
                        .llm_contradiction_verdict(&best_match.memory.content, &candidate.content)
                        .await
                    {
                        match verdict {
                            LlmContradictionVerdict::Agree => {
                                return Ok(GateDecision::Merge {
                                    target_id: best_match.memory.id,
                                    enriched_content: Self::merge_content(
                                        &best_match.memory.content,
                                        &candidate.content,
                                        best_match.similarity,
                                    ),
                                    promote_to: (best_match.memory.scope.rank()
                                        < candidate_scope.rank())
                                    .then_some(candidate_scope),
                                });
                            }
                            LlmContradictionVerdict::Contradict(description) => {
                                return Ok(GateDecision::Contradiction {
                                    conflicting_id: best_match.memory.id,
                                    description,
                                });
                            }
                            LlmContradictionVerdict::Unrelated => {
                                return Ok(GateDecision::Accept {
                                    similar_to: None,
                                    similarity: None,
                                });
                            }
                        }
                    }

                    if let Some(description) = Self::contradiction_description(
                        &best_match.memory.content,
                        &candidate.content,
                    ) {
                        return Ok(GateDecision::Contradiction {
                            conflicting_id: best_match.memory.id,
                            description,
                        });
                    }
                    return Ok(GateDecision::Merge {
                        target_id: best_match.memory.id,
                        enriched_content: Self::merge_content(
                            &best_match.memory.content,
                            &candidate.content,
                            best_match.similarity,
                        ),
                        promote_to: (best_match.memory.scope.rank() < candidate_scope.rank())
                            .then_some(candidate_scope),
                    });
                }

                if self.is_likely_duplicate(best_match.similarity) {
                    likely_duplicate = Some((best_match.memory.id, best_match.similarity));
                }
            }
        }

        if candidate.importance_score < self.scope_config.salience_threshold {
            return Ok(GateDecision::Archive);
        }

        if candidate.provenance == ProvenanceLevel::AgentInferred
            && candidate.importance_score < self.scope_config.agent_inferred_importance_threshold
        {
            return Ok(GateDecision::Archive);
        }

        Ok(GateDecision::Accept {
            similar_to: likely_duplicate.map(|(memory_id, _)| memory_id),
            similarity: likely_duplicate.map(|(_, similarity)| similarity),
        })
    }
}

fn display_scope(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Session => "session",
        MemoryScope::Workspace => "workspace",
        MemoryScope::User => "user",
        MemoryScope::Agent => "agent",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LlmContradictionVerdict {
    Agree,
    Contradict(String),
    Unrelated,
}

fn build_contradiction_prompt(existing_content: &str, candidate_content: &str) -> String {
    format!(
        "You are a fact-checking agent. Determine if these two memories contradict each other.\n\nRules:\n- A contradiction means they make incompatible claims about the same subject.\n- Additive information (one has more detail) is NOT a contradiction.\n- Rephrasing the same fact is NOT a contradiction.\n- Respond ONLY with one of: AGREE, CONTRADICT, or UNRELATED.\n- If CONTRADICT, add a brief explanation after a colon.\n\nMemory A:\n{existing_content}\n\nMemory B:\n{candidate_content}\n\nVerdict:"
    )
}

fn parse_contradiction_response(response: &str) -> Option<LlmContradictionVerdict> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return None;
    }

    let uppercase = trimmed.to_ascii_uppercase();
    if uppercase == "AGREE" {
        return Some(LlmContradictionVerdict::Agree);
    }
    if uppercase == "UNRELATED" {
        return Some(LlmContradictionVerdict::Unrelated);
    }
    if uppercase.starts_with("CONTRADICT") {
        let description = trimmed.split_once(':').map_or(
            "llm contradiction check flagged incompatible claims",
            |(_, detail)| detail.trim(),
        );
        let description = if description.is_empty() {
            "llm contradiction check flagged incompatible claims"
        } else {
            description
        };
        return Some(LlmContradictionVerdict::Contradict(description.to_string()));
    }

    None
}

fn normalize_for_merge(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn candidate_is_clearly_more_detailed(existing_content: &str, candidate_content: &str) -> bool {
    candidate_content.chars().count() > (existing_content.chars().count() * 6 / 5)
}

fn candidate_adds_material_search_terms(existing_content: &str, candidate_content: &str) -> bool {
    let existing_terms = searchable_terms(existing_content);
    let candidate_terms = searchable_terms(candidate_content);

    if existing_terms.is_empty() || candidate_terms.len() <= existing_terms.len() {
        return false;
    }

    let overlap_count = existing_terms.intersection(&candidate_terms).count();
    let added_count = candidate_terms.difference(&existing_terms).count();
    let removed_count = existing_terms.difference(&candidate_terms).count();

    added_count > 0 && added_count > removed_count && overlap_count * 2 >= existing_terms.len()
}

fn searchable_terms(content: &str) -> BTreeSet<String> {
    let mut searchable_terms = BTreeSet::new();
    let mut token = String::new();

    for character in content.chars() {
        if character.is_alphanumeric() || character == '_' {
            token.push(character);
        } else {
            collect_searchable_terms(&token, &mut searchable_terms);
            token.clear();
        }
    }

    collect_searchable_terms(&token, &mut searchable_terms);
    searchable_terms
}

fn collect_searchable_terms(token: &str, searchable_terms: &mut BTreeSet<String>) {
    let Some(normalized) = normalize_searchable_term(token) else {
        return;
    };
    searchable_terms.insert(normalized);

    for part in split_compound_search_term(token) {
        if let Some(normalized_part) = normalize_searchable_term(&part) {
            searchable_terms.insert(normalized_part);
        }
    }
}

fn normalize_searchable_term(token: &str) -> Option<String> {
    let normalized = token.trim_matches('_').to_lowercase();
    if normalized.len() < 2 || is_searchable_term_filler(&normalized) {
        return None;
    }

    Some(normalized)
}

fn is_searchable_term_filler(token: &str) -> bool {
    is_subject_filler(token) || matches!(token, "tout")
}

fn split_compound_search_term(token: &str) -> Vec<String> {
    let characters = token.chars().collect::<Vec<_>>();
    if characters.len() < 2 {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut current_part = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if index > 0 {
            let previous = characters[index - 1];
            let next = characters.get(index + 1).copied();
            let has_boundary = (previous.is_lowercase() && character.is_uppercase())
                || (previous.is_uppercase()
                    && character.is_uppercase()
                    && next.is_some_and(|next_character| next_character.is_lowercase()))
                || (previous.is_ascii_digit() && character.is_alphabetic())
                || (previous.is_alphabetic() && character.is_ascii_digit());

            if has_boundary && !current_part.is_empty() {
                parts.push(std::mem::take(&mut current_part));
            }
        }

        current_part.push(character);
    }

    if !current_part.is_empty() {
        parts.push(current_part);
    }

    if parts.len() > 1 {
        parts
    } else {
        Vec::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TechnologyFact {
    subject: String,
    category: &'static str,
    values: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumericFact {
    subject: String,
    value: String,
}

fn detect_technology_contradiction(
    existing_content: &str,
    candidate_content: &str,
) -> Option<String> {
    let existing_facts = extract_technology_facts(existing_content);
    let candidate_facts = extract_technology_facts(candidate_content);

    for existing_fact in &existing_facts {
        for candidate_fact in &candidate_facts {
            if existing_fact.subject != candidate_fact.subject
                || existing_fact.category != candidate_fact.category
            {
                continue;
            }
            if existing_fact.values.is_subset(&candidate_fact.values)
                || candidate_fact.values.is_subset(&existing_fact.values)
            {
                continue;
            }
            if existing_fact.values.is_disjoint(&candidate_fact.values) {
                let existing_values =
                    technology_values_for_subject(&existing_facts, &existing_fact.subject);
                let candidate_values =
                    technology_values_for_subject(&candidate_facts, &candidate_fact.subject);
                return Some(format!(
                    "Conflicting technology values detected for {}: {} vs {}",
                    existing_fact.subject,
                    join_values(&existing_values),
                    join_values(&candidate_values)
                ));
            }
        }
    }

    None
}

fn detect_numeric_contradiction(existing_content: &str, candidate_content: &str) -> Option<String> {
    let existing_facts = extract_numeric_facts(existing_content);
    let candidate_facts = extract_numeric_facts(candidate_content);

    for existing_fact in &existing_facts {
        for candidate_fact in &candidate_facts {
            if existing_fact.subject == candidate_fact.subject
                && existing_fact.value != candidate_fact.value
            {
                return Some(format!(
                    "Conflicting numeric values detected for {}: {} vs {}",
                    existing_fact.subject, existing_fact.value, candidate_fact.value
                ));
            }
        }
    }

    None
}

fn extract_technology_facts(content: &str) -> Vec<TechnologyFact> {
    let tokens = heuristic_tokens(content);
    let mut facts_by_key: BTreeMap<(String, &'static str), BTreeSet<String>> = BTreeMap::new();

    for (index, token) in tokens.iter().enumerate() {
        if !matches!(
            token.as_str(),
            "is" | "are" | "est" | "uses" | "use" | "using"
        ) {
            continue;
        }
        let Some(subject) = subject_from_prefix(&tokens, index) else {
            continue;
        };

        for value in tokens
            .iter()
            .skip(index + 1)
            .take(8)
            .filter_map(|token| canonical_technology_value(token))
        {
            facts_by_key
                .entry((subject.clone(), value.1))
                .or_default()
                .insert(value.0.to_string());
        }
    }

    facts_by_key
        .into_iter()
        .map(|((subject, category), values)| TechnologyFact {
            subject,
            category,
            values,
        })
        .collect()
}

fn extract_numeric_facts(content: &str) -> Vec<NumericFact> {
    let tokens = heuristic_tokens(content);
    let mut facts = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        let Some((value, consumed)) = normalize_numeric_value(&tokens, index) else {
            index += 1;
            continue;
        };
        if let Some(subject) = subject_from_prefix(&tokens, index) {
            facts.push(NumericFact { subject, value });
        }
        index += consumed;
    }

    facts
}

fn heuristic_tokens(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in content.chars() {
        if character.is_alphanumeric() || matches!(character, '#' | '+' | '.' | '%' | '-') {
            current.push(character.to_ascii_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn subject_from_prefix(tokens: &[String], end_index: usize) -> Option<String> {
    let subject_tokens = tokens[..end_index]
        .iter()
        .rev()
        .filter(|token| !is_subject_filler(token))
        .take(3)
        .cloned()
        .collect::<Vec<_>>();

    if subject_tokens.is_empty() {
        None
    } else {
        Some(
            subject_tokens
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

fn is_subject_filler(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "avec"
            | "de"
            | "des"
            | "du"
            | "en"
            | "est"
            | "et"
            | "in"
            | "is"
            | "la"
            | "le"
            | "les"
            | "the"
            | "use"
            | "uses"
            | "using"
            | "with"
    )
}

fn canonical_technology_value(token: &str) -> Option<(&'static str, &'static str)> {
    match token {
        ".net" => Some((".net", "language")),
        "asp.net" => Some(("asp.net", "language")),
        "c#" | "csharp" => Some(("c#", "language")),
        "f#" => Some(("f#", "language")),
        "go" | "golang" => Some(("go", "language")),
        "java" => Some(("java", "language")),
        "javascript" => Some(("javascript", "language")),
        "kotlin" => Some(("kotlin", "language")),
        "php" => Some(("php", "language")),
        "python" => Some(("python", "language")),
        "ruby" => Some(("ruby", "language")),
        "rust" => Some(("rust", "language")),
        "swift" => Some(("swift", "language")),
        "typescript" => Some(("typescript", "language")),
        "actix" => Some(("actix", "framework")),
        "angular" => Some(("angular", "framework")),
        "axum" => Some(("axum", "framework")),
        "django" => Some(("django", "framework")),
        "fastapi" => Some(("fastapi", "framework")),
        "flask" => Some(("flask", "framework")),
        "grpc" => Some(("grpc", "framework")),
        "react" => Some(("react", "framework")),
        "tauri" => Some(("tauri", "framework")),
        "vue" => Some(("vue", "framework")),
        "mssql" => Some(("mssql", "storage")),
        "mysql" => Some(("mysql", "storage")),
        "postgres" | "postgresql" => Some(("postgres", "storage")),
        "redis" => Some(("redis", "storage")),
        "sqlite" => Some(("sqlite", "storage")),
        _ => None,
    }
}

fn normalize_numeric_value(tokens: &[String], index: usize) -> Option<(String, usize)> {
    let token = tokens.get(index)?;

    if let Some((number, unit)) = split_inline_numeric_value(token) {
        return Some((format!("{number}{unit}"), 1));
    }

    if is_plain_numeric_token(token) {
        let unit = tokens.get(index + 1)?;
        if is_unit_token(unit) {
            return Some((format!("{token}{unit}"), 2));
        }
    }

    None
}

fn split_inline_numeric_value(token: &str) -> Option<(&str, &str)> {
    let split_index =
        token.find(|character: char| !character.is_ascii_digit() && character != '.')?;
    let (number, unit) = token.split_at(split_index);
    if number.is_empty() || !is_plain_numeric_token(number) || !is_unit_token(unit) {
        return None;
    }
    Some((number, unit))
}

fn is_plain_numeric_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
}

fn is_unit_token(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '%')
}

fn join_values(values: &BTreeSet<String>) -> String {
    values.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn technology_values_for_subject(facts: &[TechnologyFact], subject: &str) -> BTreeSet<String> {
    facts
        .iter()
        .filter(|fact| fact.subject == subject)
        .flat_map(|fact| fact.values.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use uuid::Uuid;

    use super::{searchable_terms, DefaultSalienceGate};
    use crate::{
        ContradictionEntry, EmbeddingError, EmbeddingProvider, GateDecision, LlmError, LlmProvider,
        Memory, MemoryCandidate, MemoryFilter, MemoryHealthReport, MemoryId, MemoryScope,
        MemoryState, MemoryStore, MemoryType, MetadataUpdate, ProvenanceLevel, PurgeReport,
        ResolutionStatus, SalienceGate, ScopeConfig, ScoredMemory, SearchQuery, SensitivityLevel,
        StoreError,
    };

    #[tokio::test]
    async fn merges_when_similarity_exceeds_merge_threshold() {
        let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Launch plan with contingency checklist",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate candidate");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(enriched_content, "Launch plan with contingency checklist");
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn detects_contradiction_for_different_technology_values() {
        let target = sample_memory("Backend is C# with gRPC", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Backend is Python with Flask",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate contradictory candidate");

        match decision {
            GateDecision::Contradiction {
                conflicting_id,
                description,
            } => {
                assert_eq!(conflicting_id, target.id);
                assert!(description.contains("backend"));
                assert!(description.contains("c#"));
                assert!(description.contains("python"));
            }
            other => panic!("expected contradiction decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn detects_contradiction_for_different_numeric_values() {
        let target = sample_memory("Cap RTSS 120fps", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Cap RTSS 60fps",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate contradictory numeric candidate");

        match decision {
            GateDecision::Contradiction {
                conflicting_id,
                description,
            } => {
                assert_eq!(conflicting_id, target.id);
                assert!(description.contains("cap rtss"));
                assert!(description.contains("120fps"));
                assert!(description.contains("60fps"));
            }
            other => panic!("expected contradiction decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn llm_agree_verdict_keeps_merge_path() {
        let target = sample_memory("Project uses Rust", ProvenanceLevel::UserStated);
        let llm = Arc::new(StubLlmProvider::new([(
            "Project uses Rust\n---\nProject uses Rust and Tauri",
            Ok("AGREE".to_string()),
        )]));
        let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Project uses Rust and Tauri",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate llm agree candidate");

        match decision {
            GateDecision::Merge { target_id, .. } => assert_eq!(target_id, target.id),
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn llm_contradict_verdict_records_contradiction() {
        let target = sample_memory("Backend is C# with gRPC", ProvenanceLevel::UserStated);
        let llm = Arc::new(StubLlmProvider::new([(
            "Backend is C# with gRPC\n---\nBackend is Python with Flask",
            Ok("CONTRADICT: incompatible backend stack".to_string()),
        )]));
        let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Backend is Python with Flask",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate llm contradiction candidate");

        match decision {
            GateDecision::Contradiction {
                conflicting_id,
                description,
            } => {
                assert_eq!(conflicting_id, target.id);
                assert_eq!(description, "incompatible backend stack");
            }
            other => panic!("expected contradiction decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn llm_unrelated_verdict_accepts_new_memory() {
        let target = sample_memory("Project uses Rust", ProvenanceLevel::UserStated);
        let llm = Arc::new(StubLlmProvider::new([(
            "Project uses Rust\n---\nTeam meets on Fridays",
            Ok("UNRELATED".to_string()),
        )]));
        let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target,
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Team meets on Fridays",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate llm unrelated candidate");

        assert_eq!(
            decision,
            GateDecision::Accept {
                similar_to: None,
                similarity: None,
            }
        );
    }

    #[tokio::test]
    async fn llm_failure_falls_back_to_heuristic_contradiction_detection() {
        let target = sample_memory("Cap RTSS 120fps", ProvenanceLevel::UserStated);
        let llm = Arc::new(StubLlmProvider::new([(
            "Cap RTSS 120fps\n---\nCap RTSS 60fps",
            Err(LlmError::Provider("offline".to_string())),
        )]));
        let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Cap RTSS 60fps",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate llm failure candidate");

        match decision {
            GateDecision::Contradiction { conflicting_id, .. } => {
                assert_eq!(conflicting_id, target.id);
            }
            other => panic!("expected heuristic contradiction decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn additive_information_still_merges_instead_of_flagging_contradiction() {
        let target = sample_memory("Project uses Rust", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Project uses Rust and Tauri",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate additive candidate");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(enriched_content, "Project uses Rust and Tauri");
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rephrased_content_still_merges_instead_of_flagging_contradiction() {
        let target = sample_memory("Elegy is a memory system", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Elegy is a standalone memory system",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate rephrased candidate");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(enriched_content, "Elegy is a standalone memory system");
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn moderate_similarity_keeps_existing_content_instead_of_concatenating() {
        let target = sample_memory(
            "Launch plan with rollback checklist",
            ProvenanceLevel::UserStated,
        );
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.94,
            similarity: 0.94,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Launch plan with fallback checklist",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate candidate");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(enriched_content, target.content);
                assert!(!enriched_content.contains("\n\n"));
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn moderate_similarity_replaces_with_more_detailed_candidate() {
        let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.94,
            similarity: 0.94,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Launch plan with contingency checklist and rollback owner",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate candidate");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(
                    enriched_content,
                    "Launch plan with contingency checklist and rollback owner"
                );
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[test]
    fn searchable_term_extraction_keeps_compound_word_expansions_for_merge_enrichment() {
        let terms = searchable_terms("ProtonVPN avec WireGuard et JavaScript");

        assert!(terms.contains("protonvpn"));
        assert!(terms.contains("vpn"));
        assert!(terms.contains("wireguard"));
        assert!(terms.contains("javascript"));
        assert!(terms.contains("script"));
    }

    #[tokio::test]
    async fn moderate_similarity_replaces_when_candidate_adds_material_search_terms() {
        let target = sample_memory(
            "ProtonVPN avec WireGuard protege tout le trafic reseau",
            ProvenanceLevel::UserStated,
        );
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.94,
            similarity: 0.94,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "ProtonVPN avec WireGuard et JavaScript protegent le reseau",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate candidate");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(
                    enriched_content,
                    "ProtonVPN avec WireGuard et JavaScript protegent le reseau"
                );
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn accepts_candidates_below_the_likely_duplicate_floor_without_warning() {
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: sample_memory("Existing plan", ProvenanceLevel::UserStated),
            score: 0.79,
            similarity: 0.79,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Existing plan with a distinct rollback path",
                    0.8,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate candidate");

        assert_eq!(
            decision,
            GateDecision::Accept {
                similar_to: None,
                similarity: None,
            }
        );
        assert_eq!(store.find_similar_call_count(), 1);
        let calls = store.find_similar_calls();
        assert_eq!(calls[0].0, 4);
        assert!((calls[0].1 - 0.80).abs() < f32::EPSILON);
        assert_eq!(calls[0].2, 1);
    }

    #[tokio::test]
    async fn accepts_candidates_in_the_likely_duplicate_warning_band() {
        let existing = sample_memory("Uses Rust", ProvenanceLevel::UserStated);
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: existing.clone(),
            score: 0.82,
            similarity: 0.82,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Uses Rust and Tauri",
                    0.8,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate likely-duplicate candidate");

        assert_eq!(
            decision,
            GateDecision::Accept {
                similar_to: Some(existing.id),
                similarity: Some(0.82),
            }
        );
    }

    #[tokio::test]
    async fn archives_low_salience_candidates() {
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::default();

        let decision = gate
            .evaluate(
                &sample_candidate("Minor aside", 0.1, ProvenanceLevel::UserStated, None),
                &store,
            )
            .await
            .expect("evaluate low-salience candidate");

        assert_eq!(decision, GateDecision::Archive);
    }

    #[tokio::test]
    async fn archives_low_confidence_inferences_using_architecture_threshold() {
        let gate = DefaultSalienceGate::new(ScopeConfig {
            salience_threshold: 0.2,
            agent_inferred_importance_threshold: 0.5,
            ..ScopeConfig::default()
        });
        let store = MockStore::default();

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "The user might prefer morning standups",
                    0.45,
                    ProvenanceLevel::AgentInferred,
                    None,
                ),
                &store,
            )
            .await
            .expect("evaluate inferred candidate");

        assert_eq!(decision, GateDecision::Archive);
    }

    #[tokio::test]
    async fn missing_embedding_skips_novelty_lookup() {
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: sample_memory("Should not be consulted", ProvenanceLevel::UserStated),
            score: 0.99,
            similarity: 0.99,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Important user preference",
                    0.9,
                    ProvenanceLevel::UserStated,
                    None,
                ),
                &store,
            )
            .await
            .expect("evaluate candidate without embedding");

        assert_eq!(
            decision,
            GateDecision::Accept {
                similar_to: None,
                similarity: None,
            }
        );
        assert_eq!(store.find_similar_call_count(), 0);
    }

    #[tokio::test]
    async fn provider_backed_gate_merges_when_candidate_embedding_is_missing() {
        let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
        let provider = Arc::new(StubEmbeddingProvider::new([(
            "Launch plan with contingency checklist",
            StubEmbeddingResponse::Embedding(vec![0.1, 0.2, 0.3, 0.4]),
        )]));
        let gate = DefaultSalienceGate::new_with_embedding_provider(
            ScopeConfig::default(),
            provider.clone(),
        );
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Launch plan with contingency checklist",
                    0.9,
                    ProvenanceLevel::UserStated,
                    None,
                ),
                &store,
            )
            .await
            .expect("evaluate candidate with provider-backed novelty lookup");

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                ..
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(enriched_content, "Launch plan with contingency checklist");
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
        assert_eq!(
            provider.calls(),
            vec!["Launch plan with contingency checklist".to_string()]
        );
        assert_eq!(store.find_similar_call_count(), 1);
        let calls = store.find_similar_calls();
        assert_eq!(calls[0].0, 4);
    }

    #[tokio::test]
    async fn provider_failure_gracefully_falls_back_to_archive_logic() {
        let provider = Arc::new(StubEmbeddingProvider::new([(
            "Minor aside",
            StubEmbeddingResponse::Failure("provider offline".to_string()),
        )]));
        let gate = DefaultSalienceGate::new_with_embedding_provider(
            ScopeConfig::default(),
            provider.clone(),
        );
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: sample_memory("Should not be consulted", ProvenanceLevel::UserStated),
            score: 0.99,
            similarity: 0.99,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate("Minor aside", 0.1, ProvenanceLevel::UserStated, None),
                &store,
            )
            .await
            .expect("evaluate candidate when provider embedding fails");

        assert_eq!(decision, GateDecision::Archive);
        assert_eq!(provider.calls(), vec!["Minor aside".to_string()]);
        assert_eq!(store.find_similar_call_count(), 0);
    }

    #[tokio::test]
    async fn rejects_near_duplicate_when_match_exists_in_higher_scope() {
        let existing = sample_memory("Shared preference", ProvenanceLevel::UserStated);
        let mut higher = existing.clone();
        higher.scope = MemoryScope::User;
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_scope_and_similar_results(
            MemoryScope::Workspace,
            vec![ScoredMemory {
                memory: higher.clone(),
                score: 0.95,
                similarity: 0.95,
            }],
        );

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Shared preference with tiny wording change",
                    0.8,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate higher-scope duplicate");

        assert!(matches!(decision, GateDecision::Reject { .. }));
    }

    #[tokio::test]
    async fn lower_scope_duplicate_requests_merge_and_promotion_to_current_scope() {
        let mut lower = sample_memory("Team procedure", ProvenanceLevel::UserStated);
        lower.scope = MemoryScope::Workspace;
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_scope_and_similar_results(
            MemoryScope::User,
            vec![ScoredMemory {
                memory: lower.clone(),
                score: 0.95,
                similarity: 0.95,
            }],
        );

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Team procedure with more detail",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate lower-scope duplicate");

        assert_eq!(
            decision,
            GateDecision::Merge {
                target_id: lower.id,
                enriched_content: "Team procedure with more detail".to_string(),
                promote_to: Some(MemoryScope::User),
            }
        );
    }

    #[tokio::test]
    async fn explicit_candidate_embedding_still_takes_precedence_over_provider() {
        let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
        let provider = Arc::new(StubEmbeddingProvider::new([(
            "Launch plan with contingency checklist",
            StubEmbeddingResponse::Embedding(vec![9.0; 4]),
        )]));
        let gate = DefaultSalienceGate::new_with_embedding_provider(
            ScopeConfig::default(),
            provider.clone(),
        );
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: target.clone(),
            score: 0.95,
            similarity: 0.95,
        }]);

        let decision = gate
            .evaluate(
                &sample_candidate(
                    "Launch plan with contingency checklist",
                    0.9,
                    ProvenanceLevel::UserStated,
                    Some(vec![1.0; 4]),
                ),
                &store,
            )
            .await
            .expect("evaluate candidate with explicit embedding");

        match decision {
            GateDecision::Merge { target_id, .. } => assert_eq!(target_id, target.id),
            other => panic!("expected merge decision, got {other:?}"),
        }
        assert!(provider.calls().is_empty());
        assert_eq!(store.find_similar_call_count(), 1);
        let calls = store.find_similar_calls();
        assert_eq!(calls[0].0, 4);
    }

    fn sample_candidate(
        content: &str,
        importance_score: f32,
        provenance: ProvenanceLevel,
        embedding: Option<Vec<f32>>,
    ) -> MemoryCandidate {
        MemoryCandidate {
            content: content.to_string(),
            summary: None,
            memory_type: MemoryType::Observation,
            provenance,
            importance_score,
            sensitivity: SensitivityLevel::Low,
            tags: Vec::new(),
            custom_metadata: HashMap::new(),
            embedding,
        }
    }

    fn sample_memory(content: &str, provenance: ProvenanceLevel) -> Memory {
        let now = Utc::now();
        Memory {
            id: Uuid::new_v4(),
            content: content.to_string(),
            summary: None,
            scope: MemoryScope::Workspace,
            memory_type: MemoryType::Observation,
            provenance,
            importance_score: 0.8,
            reliability_score: provenance.base_reliability(),
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: Vec::new(),
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: false,
            created_at: now,
            updated_at: now,
            last_accessed_at: Some(now),
            tenant_id: None,
            user_id: None,
            agent_id: None,
        }
    }

    #[derive(Debug)]
    struct StubLlmProvider {
        responses: HashMap<String, Result<String, LlmError>>,
    }

    impl StubLlmProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, Result<String, LlmError>)>,
            S: Into<String>,
        {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(pair, response)| (pair.into(), response))
                    .collect(),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for StubLlmProvider {
        async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
            let memory_a = prompt
                .split("\n\nMemory A:\n")
                .nth(1)
                .and_then(|tail| tail.split("\n\nMemory B:\n").next())
                .unwrap_or("")
                .trim();
            let memory_b = prompt
                .split("\n\nMemory B:\n")
                .nth(1)
                .and_then(|tail| tail.split("\n\nVerdict:").next())
                .unwrap_or("")
                .trim();
            let key = format!("{memory_a}\n---\n{memory_b}");
            match self.responses.get(&key) {
                Some(Ok(response)) => Ok(response.clone()),
                Some(Err(error)) => Err(LlmError::Provider(error.to_string())),
                None => Err(LlmError::Provider(format!(
                    "missing llm response for `{key}`"
                ))),
            }
        }

        fn name(&self) -> &str {
            "stub-llm"
        }

        fn model(&self) -> &str {
            "stub-llm-model"
        }
    }

    #[derive(Debug, Clone)]
    enum StubEmbeddingResponse {
        Embedding(Vec<f32>),
        Failure(String),
    }

    #[derive(Debug)]
    struct StubEmbeddingProvider {
        responses: HashMap<String, StubEmbeddingResponse>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
            S: Into<String>,
        {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(text, response)| (text.into(), response))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("stub provider calls lock").clone()
        }
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let trimmed = text.trim().to_string();
            self.calls
                .lock()
                .expect("stub provider calls lock")
                .push(trimmed.clone());

            match self.responses.get(&trimmed) {
                Some(StubEmbeddingResponse::Embedding(embedding)) => Ok(embedding.clone()),
                Some(StubEmbeddingResponse::Failure(message)) => {
                    Err(EmbeddingError::Provider(message.clone()))
                }
                None => Err(EmbeddingError::Provider(format!(
                    "missing stub embedding for `{trimmed}`"
                ))),
            }
        }

        fn dimensions(&self) -> usize {
            768
        }

        fn model_id(&self) -> &str {
            "stub-embedding-provider"
        }
    }

    #[derive(Clone)]
    struct MockStore {
        scope: MemoryScope,
        similar_results: Vec<ScoredMemory>,
        find_similar_calls: Arc<Mutex<Vec<(usize, f32, usize)>>>,
    }

    impl MockStore {
        fn with_similar_results(similar_results: Vec<ScoredMemory>) -> Self {
            Self::with_scope_and_similar_results(MemoryScope::Workspace, similar_results)
        }

        fn with_scope_and_similar_results(
            scope: MemoryScope,
            similar_results: Vec<ScoredMemory>,
        ) -> Self {
            Self {
                scope,
                similar_results,
                find_similar_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn find_similar_call_count(&self) -> usize {
            self.find_similar_calls.lock().expect("lock call log").len()
        }

        fn find_similar_calls(&self) -> Vec<(usize, f32, usize)> {
            self.find_similar_calls
                .lock()
                .expect("lock call log")
                .clone()
        }
    }

    impl Default for MockStore {
        fn default() -> Self {
            Self::with_scope_and_similar_results(MemoryScope::Workspace, Vec::new())
        }
    }

    #[async_trait]
    impl MemoryStore for MockStore {
        fn scope(&self) -> MemoryScope {
            self.scope
        }

        async fn store(&self, _memory: Memory) -> Result<MemoryId, StoreError> {
            Err(unused_store_error())
        }

        async fn update_content(
            &self,
            _id: &MemoryId,
            _new_content: &str,
            _changed_by: &str,
            _reason: &str,
        ) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn update_metadata(
            &self,
            _id: &MemoryId,
            _updates: MetadataUpdate,
        ) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn get(&self, _id: &MemoryId) -> Result<Option<Memory>, StoreError> {
            Err(unused_store_error())
        }

        async fn get_raw(&self, _id: &MemoryId) -> Result<Option<Memory>, StoreError> {
            Err(unused_store_error())
        }

        async fn list(&self, _filter: MemoryFilter) -> Result<Vec<Memory>, StoreError> {
            Err(unused_store_error())
        }

        async fn search(&self, _query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError> {
            Err(unused_store_error())
        }

        async fn find_similar(
            &self,
            embedding: &[f32],
            threshold: f32,
            limit: usize,
        ) -> Result<Vec<ScoredMemory>, StoreError> {
            self.find_similar_calls
                .lock()
                .expect("lock call log")
                .push((embedding.len(), threshold, limit));
            Ok(self.similar_results.iter().take(limit).cloned().collect())
        }

        async fn store_embedding(
            &self,
            _id: &MemoryId,
            _embedding: &[f32],
        ) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn get_stale_embeddings(&self, _limit: usize) -> Result<Vec<MemoryId>, StoreError> {
            Err(unused_store_error())
        }

        async fn make_dormant(&self, _id: &MemoryId) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn reactivate(&self, _id: &MemoryId) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn hard_delete(&self, _id: &MemoryId) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn purge_user(&self, _user_id: &str) -> Result<PurgeReport, StoreError> {
            Err(unused_store_error())
        }

        async fn purge_all(&self) -> Result<PurgeReport, StoreError> {
            Err(unused_store_error())
        }

        async fn health_report(&self) -> Result<MemoryHealthReport, StoreError> {
            Err(unused_store_error())
        }

        async fn list_contradictions(
            &self,
            _status: Option<ResolutionStatus>,
        ) -> Result<Vec<ContradictionEntry>, StoreError> {
            Err(unused_store_error())
        }

        async fn record_contradiction(
            &self,
            _a_id: &MemoryId,
            _b_id: &MemoryId,
            _description: &str,
        ) -> Result<(), StoreError> {
            Err(unused_store_error())
        }

        async fn update_contradiction_status(
            &self,
            _contradiction_id: &str,
            _status: ResolutionStatus,
            _note: Option<&str>,
        ) -> Result<(), StoreError> {
            Err(unused_store_error())
        }
    }

    fn unused_store_error() -> StoreError {
        StoreError::Validation("unused mock store method".to_string())
    }
}
