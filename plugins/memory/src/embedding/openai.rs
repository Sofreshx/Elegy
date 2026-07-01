use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{EmbeddingError, EmbeddingProvider};

pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com";
pub const DEFAULT_OPENAI_MODEL: &str = "text-embedding-3-small";
pub const DEFAULT_OPENAI_DIMENSIONS: usize = 1536;
pub const DEFAULT_OPENAI_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_OPENAI_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// OpenAI-compatible [`EmbeddingProvider`] for cloud and self-hosted embedding endpoints.
///
/// Supports OpenAI's `/v1/embeddings` API as well as compatible servers such as
/// LM Studio and vLLM. When the remote endpoint is unreachable the provider surfaces
/// a well-structured error that the store's degradation-warning path can detect and
/// present as a graceful fallback message.
#[derive(Debug, Clone)]
pub struct OpenAiEmbeddingProvider {
    client: Client,
    base_url: String,
    model: String,
    dimensions: usize,
    api_key: String,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl OpenAiEmbeddingProvider {
    /// Create a provider with the default base URL, model, and timeouts using the given API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self, EmbeddingError> {
        Self::new_with_config(
            DEFAULT_OPENAI_BASE_URL,
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            api_key,
        )
    }

    /// Create a provider with an explicit base URL, model, expected dimensions, and API key.
    ///
    /// Set `base_url` to a custom URL (e.g. `http://localhost:1234`) for LM Studio / vLLM
    /// compatibility.
    pub fn new_with_config(
        base_url: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
        api_key: impl Into<String>,
    ) -> Result<Self, EmbeddingError> {
        Self::new_with_timeouts(
            base_url,
            model,
            dimensions,
            api_key,
            DEFAULT_OPENAI_CONNECT_TIMEOUT,
            DEFAULT_OPENAI_REQUEST_TIMEOUT,
        )
    }

    /// Create a provider with full explicit configuration including timeouts.
    pub fn new_with_timeouts(
        base_url: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
        api_key: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, EmbeddingError> {
        validate_timeout("openai connect timeout", connect_timeout)?;
        validate_timeout("openai request timeout", request_timeout)?;

        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|error| {
                EmbeddingError::Provider(format!(
                    "failed to build OpenAI HTTP client with configured timeouts: {error}"
                ))
            })?;

        Self::with_client(
            client,
            base_url,
            model,
            dimensions,
            api_key,
            connect_timeout,
            request_timeout,
        )
    }

    fn with_client(
        client: Client,
        base_url: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
        api_key: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, EmbeddingError> {
        let base_url = normalize_base_url(base_url.into())?;
        let model = normalize_model_id(model.into())?;
        let api_key = normalize_api_key(api_key.into())?;

        if dimensions == 0 {
            return Err(EmbeddingError::Provider(
                "openai dimensions must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            client,
            base_url,
            model,
            dimensions,
            api_key,
            connect_timeout,
            request_timeout,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/embeddings", self.base_url)
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let input = text.trim();
        if input.is_empty() {
            return Err(EmbeddingError::Provider(
                "embedding input must not be empty".to_string(),
            ));
        }

        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&OpenAiEmbeddingRequest {
                input,
                model: &self.model,
            })
            .send()
            .await
            .map_err(|error| self.map_request_error(&error))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.map_status_error(status, response).await);
        }

        let payload: OpenAiEmbeddingResponse = response.json().await.map_err(|error| {
            EmbeddingError::Provider(format!("openai embeddings response decode failed: {error}"))
        })?;

        let embedding = payload
            .data
            .into_iter()
            .next()
            .map(|entry| entry.embedding)
            .ok_or_else(|| {
                EmbeddingError::Provider("openai returned no embedding in response".to_string())
            })?;

        if embedding.is_empty() {
            return Err(EmbeddingError::Provider(
                "openai returned an empty embedding vector".to_string(),
            ));
        }

        if embedding.len() != self.dimensions {
            return Err(EmbeddingError::DimensionMismatch {
                expected: self.dimensions,
                actual: embedding.len(),
            });
        }

        Ok(embedding)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}

impl OpenAiEmbeddingProvider {
    fn map_request_error(&self, error: &reqwest::Error) -> EmbeddingError {
        if error.is_timeout() {
            return EmbeddingError::Provider(format!(
                "openai not reachable at {}: request timed out after {}",
                self.base_url,
                format_timeout(self.request_timeout)
            ));
        }

        if error.is_connect() {
            return EmbeddingError::Provider(format!(
                "openai not reachable at {}: connection failed within {} ({error})",
                self.base_url,
                format_timeout(self.connect_timeout)
            ));
        }

        EmbeddingError::Provider(format!("openai embeddings request failed: {error}"))
    }

    async fn map_status_error(
        &self,
        status: reqwest::StatusCode,
        response: reqwest::Response,
    ) -> EmbeddingError {
        let body = match response.text().await {
            Ok(body) if !body.trim().is_empty() => body,
            Ok(_) => "<empty response body>".to_string(),
            Err(error) => format!("<failed to read error body: {error}>"),
        };

        match status.as_u16() {
            401 => EmbeddingError::Provider(format!(
                "openai returned 401 Unauthorized: invalid API key ({body})"
            )),
            429 => EmbeddingError::Provider(format!(
                "openai returned 429 Too Many Requests: rate limited, try again later ({body})"
            )),
            _ => EmbeddingError::Provider(format!(
                "openai embeddings request returned {status}: {body}"
            )),
        }
    }
}

fn normalize_base_url(base_url: String) -> Result<String, EmbeddingError> {
    let normalized = base_url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return Err(EmbeddingError::Provider(
            "openai base URL must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_model_id(model: String) -> Result<String, EmbeddingError> {
    let normalized = model.trim().to_string();
    if normalized.is_empty() {
        return Err(EmbeddingError::Provider(
            "openai model must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_api_key(api_key: String) -> Result<String, EmbeddingError> {
    let normalized = api_key.trim().to_string();
    if normalized.is_empty() {
        return Err(EmbeddingError::Provider(
            "openai API key must not be empty".to_string(),
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
struct OpenAiEmbeddingRequest<'a> {
    input: &'a str,
    model: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingEntry {
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
        OpenAiEmbeddingProvider, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_CONNECT_TIMEOUT,
        DEFAULT_OPENAI_DIMENSIONS, DEFAULT_OPENAI_MODEL, DEFAULT_OPENAI_REQUEST_TIMEOUT,
    };
    use crate::EmbeddingProvider;

    #[test]
    fn new_provider_uses_openai_defaults() {
        let provider =
            OpenAiEmbeddingProvider::new("sk-test-key").expect("provider should be created");

        assert_eq!(provider.base_url, DEFAULT_OPENAI_BASE_URL);
        assert_eq!(provider.model_id(), DEFAULT_OPENAI_MODEL);
        assert_eq!(provider.dimensions(), DEFAULT_OPENAI_DIMENSIONS);
        assert_eq!(provider.connect_timeout, DEFAULT_OPENAI_CONNECT_TIMEOUT);
        assert_eq!(provider.request_timeout, DEFAULT_OPENAI_REQUEST_TIMEOUT);
    }

    #[test]
    fn new_provider_normalizes_base_url_trailing_slash() {
        let provider = OpenAiEmbeddingProvider::new_with_config(
            "https://api.openai.com/",
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
        )
        .expect("provider should be created");

        assert_eq!(provider.base_url, DEFAULT_OPENAI_BASE_URL);
    }

    #[test]
    fn new_provider_accepts_custom_base_url_for_lm_studio_compatibility() {
        let custom_url = "http://localhost:1234";
        let provider = OpenAiEmbeddingProvider::new_with_config(
            custom_url,
            "custom-embed-model",
            1536,
            "not-a-real-key",
        )
        .expect("custom base URL should be accepted");

        assert_eq!(provider.base_url, custom_url);
        assert_eq!(provider.model_id(), "custom-embed-model");
    }

    #[test]
    fn new_provider_rejects_invalid_configuration() {
        let empty_key = OpenAiEmbeddingProvider::new("   ");
        let empty_base_url = OpenAiEmbeddingProvider::new_with_config(
            "   ",
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
        );
        let empty_model = OpenAiEmbeddingProvider::new_with_config(
            DEFAULT_OPENAI_BASE_URL,
            "   ",
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
        );
        let zero_dimensions = OpenAiEmbeddingProvider::new_with_config(
            DEFAULT_OPENAI_BASE_URL,
            DEFAULT_OPENAI_MODEL,
            0,
            "sk-test-key",
        );
        let zero_connect_timeout = OpenAiEmbeddingProvider::new_with_timeouts(
            DEFAULT_OPENAI_BASE_URL,
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
            Duration::ZERO,
            Duration::from_secs(1),
        );
        let zero_request_timeout = OpenAiEmbeddingProvider::new_with_timeouts(
            DEFAULT_OPENAI_BASE_URL,
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
            Duration::from_secs(1),
            Duration::ZERO,
        );

        assert!(empty_key.is_err());
        assert!(empty_base_url.is_err());
        assert!(empty_model.is_err());
        assert!(zero_dimensions.is_err());
        assert!(zero_connect_timeout.is_err());
        assert!(zero_request_timeout.is_err());
    }

    #[tokio::test]
    async fn embed_parses_valid_json_response() {
        // Use a small dimension count for the test to keep the response compact.
        let test_dims: usize = 3;
        let expected_embedding = vec![0.1_f32, 0.5, 0.9];
        let embedding_json =
            serde_json::to_string(&expected_embedding).expect("serialize expected embedding");
        let response_body = format!(
            r#"{{"object":"list","data":[{{"object":"embedding","embedding":{embedding_json},"index":0}}],"model":"text-embedding-3-small","usage":{{"prompt_tokens":5,"total_tokens":5}}}}"#
        );

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report address");

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body,
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let provider = OpenAiEmbeddingProvider::new_with_config(
            format!("http://{address}"),
            DEFAULT_OPENAI_MODEL,
            test_dims,
            "sk-test-key",
        )
        .expect("provider should be created");

        let embedding = provider
            .embed("hello world")
            .await
            .expect("valid response should parse successfully");

        assert_eq!(embedding, expected_embedding);
    }

    #[tokio::test]
    async fn embed_invalid_api_key_yields_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report address");

        let body = r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error","code":"invalid_api_key"}}"#;
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body,
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let provider = OpenAiEmbeddingProvider::new_with_config(
            format!("http://{address}"),
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-invalid-key",
        )
        .expect("provider should be created");

        let error = provider
            .embed("some text")
            .await
            .expect_err("401 should surface as an error");

        let message = error.to_string();
        assert!(
            message.contains("401") || message.contains("invalid API key"),
            "expected invalid API key message, got: {message}"
        );
    }

    #[tokio::test]
    async fn embed_rate_limited_yields_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report address");

        let body = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body,
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let provider = OpenAiEmbeddingProvider::new_with_config(
            format!("http://{address}"),
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
        )
        .expect("provider should be created");

        let error = provider
            .embed("some text")
            .await
            .expect_err("429 should surface as an error");

        let message = error.to_string();
        assert!(
            message.contains("429") || message.contains("rate limited"),
            "expected rate-limited message, got: {message}"
        );
    }

    #[tokio::test]
    async fn embed_offline_connection_refused_returns_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report address");
        drop(listener);

        let base_url = format!("http://{address}");
        let provider = OpenAiEmbeddingProvider::new_with_timeouts(
            base_url.clone(),
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
            Duration::from_millis(200),
            Duration::from_millis(200),
        )
        .expect("provider should be created");

        let error = provider
            .embed("offline openai")
            .await
            .expect_err("connection refusal should surface as an error");

        let message = error.to_string();
        assert!(
            message.contains(&format!("openai not reachable at {base_url}")),
            "expected offline OpenAI message, got: {message}"
        );
        assert!(
            message.contains("connection failed")
                || message.contains("Connection refused")
                || message.contains("actively refused")
                || message.contains("timed out"),
            "expected an offline message, got: {message}"
        );
    }

    #[tokio::test]
    async fn embed_offline_timeout_returns_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
        let address = listener
            .local_addr()
            .expect("listener should report address");

        let server = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                thread::sleep(Duration::from_millis(250));
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let base_url = format!("http://{address}");
        let provider = OpenAiEmbeddingProvider::new_with_timeouts(
            base_url.clone(),
            DEFAULT_OPENAI_MODEL,
            DEFAULT_OPENAI_DIMENSIONS,
            "sk-test-key",
            Duration::from_millis(50),
            Duration::from_millis(50),
        )
        .expect("provider should be created");

        let error = provider
            .embed("slow openai")
            .await
            .expect_err("request timeout should surface as an error");
        server.join().expect("timeout server should finish cleanly");

        let message = error.to_string();
        assert!(
            message.contains(&format!("openai not reachable at {base_url}")),
            "expected offline OpenAI message, got: {message}"
        );
        assert!(
            message.contains("timed out"),
            "expected timeout message, got: {message}"
        );
    }
}
