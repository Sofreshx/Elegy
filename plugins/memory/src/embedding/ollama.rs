use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{EmbeddingError, EmbeddingProvider};

pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "nomic-embed-text";
pub const DEFAULT_OLLAMA_DIMENSIONS: usize = 768;
pub const DEFAULT_OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_OLLAMA_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Ollama-backed [`EmbeddingProvider`] for the MVP local embedding flow.
#[derive(Debug, Clone)]
pub struct OllamaEmbeddingProvider {
    client: Client,
    base_url: String,
    model: String,
    dimensions: usize,
    connect_timeout: Duration,
    request_timeout: Duration,
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
        Self::new_with_timeouts(
            base_url,
            model,
            DEFAULT_OLLAMA_CONNECT_TIMEOUT,
            DEFAULT_OLLAMA_REQUEST_TIMEOUT,
        )
    }

    /// Create a provider with explicit Ollama base URL, model, and timeout configuration.
    pub fn new_with_timeouts(
        base_url: impl Into<String>,
        model: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, EmbeddingError> {
        validate_timeout("ollama connect timeout", connect_timeout)?;
        validate_timeout("ollama request timeout", request_timeout)?;

        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|error| {
                EmbeddingError::Provider(format!(
                    "failed to build Ollama HTTP client with configured timeouts: {error}"
                ))
            })?;

        Self::with_client(client, base_url, model, connect_timeout, request_timeout)
    }

    fn with_client(
        client: Client,
        base_url: impl Into<String>,
        model: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, EmbeddingError> {
        let base_url = normalize_base_url(base_url.into())?;
        let model = normalize_model_id(model.into())?;

        Ok(Self {
            client,
            base_url,
            model,
            dimensions: DEFAULT_OLLAMA_DIMENSIONS,
            connect_timeout,
            request_timeout,
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
            .map_err(|error| self.map_request_error(&error))?;

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

impl OllamaEmbeddingProvider {
    fn map_request_error(&self, error: &reqwest::Error) -> EmbeddingError {
        if error.is_timeout() {
            return EmbeddingError::Provider(format!(
                "ollama not reachable at {}: request timed out after {}",
                self.base_url,
                format_timeout(self.request_timeout)
            ));
        }

        if error.is_connect() {
            return EmbeddingError::Provider(format!(
                "ollama not reachable at {}: connection failed within {} ({error})",
                self.base_url,
                format_timeout(self.connect_timeout)
            ));
        }

        EmbeddingError::Provider(format!("ollama embeddings request failed: {error}"))
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

fn validate_timeout(label: &str, timeout: Duration) -> Result<(), EmbeddingError> {
    if timeout.is_zero() {
        return Err(EmbeddingError::Provider(format!(
            "{label} must be greater than zero"
        )));
    }

    Ok(())
}

fn format_timeout(timeout: Duration) -> String {
    if timeout.subsec_nanos() == 0 {
        return format!("{}s", timeout.as_secs());
    }

    if timeout.as_secs() == 0 {
        return format!("{}ms", timeout.as_millis());
    }

    format!("{}.{:03}s", timeout.as_secs(), timeout.subsec_millis())
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
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    use super::{
        OllamaEmbeddingProvider, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_CONNECT_TIMEOUT,
        DEFAULT_OLLAMA_DIMENSIONS, DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_REQUEST_TIMEOUT,
    };
    use crate::EmbeddingProvider;

    #[test]
    fn default_provider_uses_local_ollama_defaults() {
        let provider = OllamaEmbeddingProvider::default();

        assert_eq!(provider.base_url, DEFAULT_OLLAMA_BASE_URL);
        assert_eq!(provider.model_id(), DEFAULT_OLLAMA_MODEL);
        assert_eq!(provider.dimensions(), DEFAULT_OLLAMA_DIMENSIONS);
        assert_eq!(provider.connect_timeout, DEFAULT_OLLAMA_CONNECT_TIMEOUT);
        assert_eq!(provider.request_timeout, DEFAULT_OLLAMA_REQUEST_TIMEOUT);
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
    fn new_provider_accepts_custom_timeouts() {
        let provider = OllamaEmbeddingProvider::new_with_timeouts(
            DEFAULT_OLLAMA_BASE_URL,
            DEFAULT_OLLAMA_MODEL,
            Duration::from_secs(2),
            Duration::from_secs(7),
        )
        .expect("custom timeout configuration should be accepted");

        assert_eq!(provider.connect_timeout, Duration::from_secs(2));
        assert_eq!(provider.request_timeout, Duration::from_secs(7));
    }

    #[test]
    fn new_provider_rejects_empty_configuration() {
        let empty_base_url = OllamaEmbeddingProvider::new("   ", DEFAULT_OLLAMA_MODEL);
        let empty_model = OllamaEmbeddingProvider::new(DEFAULT_OLLAMA_BASE_URL, "   ");
        let zero_connect_timeout = OllamaEmbeddingProvider::new_with_timeouts(
            DEFAULT_OLLAMA_BASE_URL,
            DEFAULT_OLLAMA_MODEL,
            Duration::ZERO,
            Duration::from_secs(1),
        );
        let zero_request_timeout = OllamaEmbeddingProvider::new_with_timeouts(
            DEFAULT_OLLAMA_BASE_URL,
            DEFAULT_OLLAMA_MODEL,
            Duration::from_secs(1),
            Duration::ZERO,
        );

        assert!(empty_base_url.is_err());
        assert!(empty_model.is_err());
        assert!(zero_connect_timeout.is_err());
        assert!(zero_request_timeout.is_err());
    }

    #[tokio::test]
    async fn ollama_offline_connection_refused_returns_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report a local address");
        drop(listener);

        let base_url = format!("http://{address}");
        let provider = OllamaEmbeddingProvider::new_with_timeouts(
            base_url.clone(),
            DEFAULT_OLLAMA_MODEL,
            Duration::from_millis(200),
            Duration::from_millis(200),
        )
        .expect("provider should be created");

        let error = provider
            .embed("offline ollama")
            .await
            .expect_err("connection refusal should surface as an error");

        let message = error.to_string();
        assert!(message.contains(&format!("ollama not reachable at {base_url}")));
        assert!(
            message.contains("connection failed")
                || message.contains("Connection refused")
                || message.contains("actively refused")
                || message.contains("timed out"),
            "expected an offline Ollama message, got: {message}"
        );
    }

    #[tokio::test]
    async fn ollama_offline_timeout_returns_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report a local address");
        let server = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                thread::sleep(Duration::from_millis(250));
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let base_url = format!("http://{address}");
        let provider = OllamaEmbeddingProvider::new_with_timeouts(
            base_url.clone(),
            DEFAULT_OLLAMA_MODEL,
            Duration::from_millis(50),
            Duration::from_millis(50),
        )
        .expect("provider should be created");

        let error = provider
            .embed("slow ollama")
            .await
            .expect_err("request timeout should surface as an error");
        server.join().expect("timeout server should finish cleanly");

        let message = error.to_string();
        assert!(message.contains(&format!("ollama not reachable at {base_url}")));
        assert!(
            message.contains("timed out"),
            "expected timeout message, got: {message}"
        );
    }
}
