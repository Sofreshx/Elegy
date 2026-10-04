//! Salience gating and candidate acceptance decisions.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use tracing::warn;

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
                    warn!(
                        provider = %provider.name(),
                        model = %provider.model(),
                        "unusable contradiction verdict; falling back to heuristic contradiction detection"
                    );
                    None
                }
            },
            Err(error) => {
                warn!(
                    provider = %provider.name(),
                    model = %provider.model(),
                    "contradiction check failed: {error}. Falling back to heuristic contradiction detection."
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
mod tests;
