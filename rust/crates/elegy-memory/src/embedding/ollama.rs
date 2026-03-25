use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{EmbeddingError, EmbeddingProvider};

pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "nomic-embed-text";
pub const DEFAULT_OLLAMA_DIMENSIONS: usize = 768;

/// Ollama-backed [`EmbeddingProvider`] for the MVP local embedding flow.
#[derive(Debug, Clone)]
pub struct OllamaEmbeddingProvider {
    client: Client,
    base_url: String,
    model: String,
    dimensions: usize,
}

impl Default for OllamaEmbeddingProvider {
    fn default() -> Self {
        Self::new(DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_MODEL)
            .expect("default Ollama embedding provider configuration must be valid")
    }
}

impl OllamaEmbeddingProvider {
    /// Create a provider with explicit Ollama base URL and model configuration.
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, EmbeddingError> {
        Self::with_client(Client::new(), base_url, model)
    }

    fn with_client(
        client: Client,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, EmbeddingError> {
        let base_url = normalize_base_url(base_url.into())?;
        let model = normalize_model_id(model.into())?;

        Ok(Self {
            client,
            base_url,
            model,
            dimensions: DEFAULT_OLLAMA_DIMENSIONS,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/api/embeddings", self.base_url)
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let prompt = text.trim();
        if prompt.is_empty() {
            return Err(EmbeddingError::Provider(
                "embedding input must not be empty".to_string(),
            ));
        }

        let response = self
            .client
            .post(self.endpoint())
            .json(&OllamaEmbeddingRequest {
                model: &self.model,
                prompt,
            })
            .send()
            .await
            .map_err(|error| {
                EmbeddingError::Provider(format!("ollama embeddings request failed: {error}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            let response_body = match response.text().await {
                Ok(body) if !body.trim().is_empty() => body,
                Ok(_) => "<empty response body>".to_string(),
                Err(error) => format!("<failed to read error body: {error}>"),
            };
            return Err(EmbeddingError::Provider(format!(
                "ollama embeddings request returned {status}: {response_body}"
            )));
        }

        let payload: OllamaEmbeddingResponse = response.json().await.map_err(|error| {
            EmbeddingError::Provider(format!("ollama embeddings response decode failed: {error}"))
        })?;

        if payload.embedding.len() != self.dimensions {
            return Err(EmbeddingError::DimensionMismatch {
                expected: self.dimensions,
                actual: payload.embedding.len(),
            });
        }

        Ok(payload.embedding)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}

fn normalize_base_url(base_url: String) -> Result<String, EmbeddingError> {
    let normalized = base_url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return Err(EmbeddingError::Provider(
            "ollama base URL must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_model_id(model: String) -> Result<String, EmbeddingError> {
    let normalized = model.trim().to_string();
    if normalized.is_empty() {
        return Err(EmbeddingError::Provider(
            "ollama model must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::{
        OllamaEmbeddingProvider, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_DIMENSIONS,
        DEFAULT_OLLAMA_MODEL,
    };
    use crate::EmbeddingProvider;

    #[test]
    fn default_provider_uses_local_ollama_defaults() {
        let provider = OllamaEmbeddingProvider::default();

        assert_eq!(provider.base_url, DEFAULT_OLLAMA_BASE_URL);
        assert_eq!(provider.model_id(), DEFAULT_OLLAMA_MODEL);
        assert_eq!(provider.dimensions(), DEFAULT_OLLAMA_DIMENSIONS);
    }

    #[test]
    fn new_provider_normalizes_configured_base_url_and_model() {
        let provider =
            OllamaEmbeddingProvider::new(" http://localhost:11434/ ", " custom-embed-model ")
                .expect("custom Ollama provider should be created");

        assert_eq!(provider.base_url, DEFAULT_OLLAMA_BASE_URL);
        assert_eq!(provider.model_id(), "custom-embed-model");
        assert_eq!(provider.dimensions(), DEFAULT_OLLAMA_DIMENSIONS);
    }

    #[test]
    fn new_provider_rejects_empty_configuration() {
        let empty_base_url = OllamaEmbeddingProvider::new("   ", DEFAULT_OLLAMA_MODEL);
        let empty_model = OllamaEmbeddingProvider::new(DEFAULT_OLLAMA_BASE_URL, "   ");

        assert!(empty_base_url.is_err());
        assert!(empty_model.is_err());
    }
}
