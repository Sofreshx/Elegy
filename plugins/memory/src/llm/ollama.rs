use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{LlmError, LlmProvider};

pub const DEFAULT_OLLAMA_LLM_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_OLLAMA_LLM_MODEL: &str = "qwen3:8b";
pub const DEFAULT_OLLAMA_LLM_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_OLLAMA_LLM_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Ollama-backed [`LlmProvider`] for local text-generation completions.
#[derive(Debug, Clone)]
pub struct OllamaLlmProvider {
    client: Client,
    base_url: String,
    model: String,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl Default for OllamaLlmProvider {
    fn default() -> Self {
        Self::new(DEFAULT_OLLAMA_LLM_BASE_URL, DEFAULT_OLLAMA_LLM_MODEL)
            .expect("default Ollama LLM provider configuration must be valid")
    }
}

impl OllamaLlmProvider {
    /// Create a provider with explicit Ollama base URL and model configuration.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Result<Self, LlmError> {
        Self::new_with_timeouts(
            base_url,
            model,
            DEFAULT_OLLAMA_LLM_CONNECT_TIMEOUT,
            DEFAULT_OLLAMA_LLM_REQUEST_TIMEOUT,
        )
    }

    /// Create a provider with explicit Ollama base URL, model, and timeout configuration.
    pub fn new_with_timeouts(
        base_url: impl Into<String>,
        model: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, LlmError> {
        validate_timeout("ollama llm connect timeout", connect_timeout)?;
        validate_timeout("ollama llm request timeout", request_timeout)?;

        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|error| {
                LlmError::Provider(format!(
                    "failed to build Ollama LLM HTTP client with configured timeouts: {error}"
                ))
            })?;

        let base_url = normalize_base_url(base_url.into())?;
        let model = normalize_model_id(model.into())?;

        Ok(Self {
            client,
            base_url,
            model,
            connect_timeout,
            request_timeout,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/api/generate", self.base_url)
    }

    fn map_request_error(&self, error: &reqwest::Error) -> LlmError {
        if error.is_timeout() {
            return LlmError::Provider(format!(
                "ollama llm not reachable at {}: request timed out after {}",
                self.base_url,
                format_timeout(self.request_timeout)
            ));
        }

        if error.is_connect() {
            return LlmError::Provider(format!(
                "ollama llm not reachable at {}: connection failed within {} ({error})",
                self.base_url,
                format_timeout(self.connect_timeout)
            ));
        }

        LlmError::Provider(format!("ollama llm request failed: {error}"))
    }
}

#[async_trait]
impl LlmProvider for OllamaLlmProvider {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(LlmError::Provider(
                "llm prompt must not be empty".to_string(),
            ));
        }

        let response = self
            .client
            .post(self.endpoint())
            .json(&OllamaGenerateRequest {
                model: &self.model,
                prompt,
                stream: false,
            })
            .send()
            .await
            .map_err(|error| self.map_request_error(&error))?;

        let status = response.status();
        if !status.is_success() {
            let body = match response.text().await {
                Ok(body) if !body.trim().is_empty() => body,
                Ok(_) => "<empty response body>".to_string(),
                Err(error) => format!("<failed to read error body: {error}>"),
            };
            return Err(match status.as_u16() {
                429 => LlmError::Provider(format!(
                    "ollama llm returned 429 Too Many Requests: rate limited, try again later ({body})"
                )),
                _ => LlmError::Provider(format!(
                    "ollama llm request returned {status}: {body}"
                )),
            });
        }

        let payload: OllamaGenerateResponse = response.json().await.map_err(|error| {
            LlmError::InvalidResponse(format!("ollama llm response decode failed: {error}"))
        })?;
        let completion = payload.response.trim();
        if completion.is_empty() {
            return Err(LlmError::InvalidResponse(
                "ollama llm returned an empty response".to_string(),
            ));
        }
        Ok(completion.to_string())
    }

    fn name(&self) -> &str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

fn normalize_base_url(base_url: String) -> Result<String, LlmError> {
    let normalized = base_url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return Err(LlmError::Provider(
            "ollama llm base URL must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_model_id(model: String) -> Result<String, LlmError> {
    let normalized = model.trim().to_string();
    if normalized.is_empty() {
        return Err(LlmError::Provider(
            "ollama llm model must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn validate_timeout(label: &str, timeout: Duration) -> Result<(), LlmError> {
    if timeout.is_zero() {
        return Err(LlmError::Provider(format!(
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
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
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
        OllamaLlmProvider, DEFAULT_OLLAMA_LLM_BASE_URL, DEFAULT_OLLAMA_LLM_CONNECT_TIMEOUT,
        DEFAULT_OLLAMA_LLM_MODEL, DEFAULT_OLLAMA_LLM_REQUEST_TIMEOUT,
    };
    use crate::LlmProvider;

    #[test]
    fn default_provider_uses_ollama_llm_defaults() {
        let provider = OllamaLlmProvider::default();
        assert_eq!(provider.base_url, DEFAULT_OLLAMA_LLM_BASE_URL);
        assert_eq!(provider.model(), DEFAULT_OLLAMA_LLM_MODEL);
        assert_eq!(provider.connect_timeout, DEFAULT_OLLAMA_LLM_CONNECT_TIMEOUT);
        assert_eq!(provider.request_timeout, DEFAULT_OLLAMA_LLM_REQUEST_TIMEOUT);
    }

    #[test]
    fn new_provider_normalizes_base_url_and_model() {
        let provider =
            OllamaLlmProvider::new(" http://localhost:11434/ ", " qwen3:8b ").expect("provider");
        assert_eq!(provider.base_url, DEFAULT_OLLAMA_LLM_BASE_URL);
        assert_eq!(provider.model(), DEFAULT_OLLAMA_LLM_MODEL);
    }

    #[tokio::test]
    async fn complete_parses_valid_generate_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 2048];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            assert!(request.contains("POST /api/generate "));
            let body = "{\"response\":\"merged answer\",\"done\":true}";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let provider =
            OllamaLlmProvider::new(format!("http://{address}"), "qwen3:8b").expect("provider");
        let completion = provider.complete("merge these").await.expect("completion");
        assert_eq!(completion, "merged answer");
    }

    #[tokio::test]
    async fn complete_connection_errors_are_clear() {
        let closed_port = TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("ephemeral address")
            .port();
        let provider =
            OllamaLlmProvider::new(format!("http://127.0.0.1:{closed_port}"), "qwen3:8b")
                .expect("provider");

        let error = provider
            .complete("merge these")
            .await
            .expect_err("connection failure");
        assert!(error.to_string().contains("ollama llm not reachable at"));
    }

    #[tokio::test]
    async fn complete_timeout_errors_are_clear() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept request");
            thread::sleep(Duration::from_millis(200));
        });

        let provider = OllamaLlmProvider::new_with_timeouts(
            format!("http://{address}"),
            "qwen3:8b",
            Duration::from_millis(50),
            Duration::from_millis(50),
        )
        .expect("provider");

        let error = provider.complete("merge these").await.expect_err("timeout");
        assert!(error.to_string().contains("request timed out"));
    }

    #[tokio::test]
    async fn complete_rate_limit_errors_are_clear() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            let body = "{\"error\":\"rate limited\"}";
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let provider =
            OllamaLlmProvider::new(format!("http://{address}"), "qwen3:8b").expect("provider");
        let error = provider
            .complete("merge these")
            .await
            .expect_err("rate limit should surface");

        assert!(error.to_string().contains("rate limited"));
    }
}
