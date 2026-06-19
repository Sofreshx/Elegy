use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{LlmError, LlmProvider};

pub const DEFAULT_OPENAI_LLM_BASE_URL: &str = "https://api.openai.com";
pub const DEFAULT_OPENAI_LLM_MODEL: &str = "gpt-4.1-mini";
pub const DEFAULT_OPENAI_LLM_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_OPENAI_LLM_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// OpenAI-compatible [`LlmProvider`] for chat-completions text generation.
#[derive(Debug, Clone)]
pub struct OpenAiLlmProvider {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl OpenAiLlmProvider {
    /// Create a provider with the default base URL, model, and timeouts using the given API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self, LlmError> {
        Self::new_with_config(
            DEFAULT_OPENAI_LLM_BASE_URL,
            DEFAULT_OPENAI_LLM_MODEL,
            api_key,
        )
    }

    /// Create a provider with an explicit base URL, model, and API key.
    pub fn new_with_config(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, LlmError> {
        Self::new_with_timeouts(
            base_url,
            model,
            api_key,
            DEFAULT_OPENAI_LLM_CONNECT_TIMEOUT,
            DEFAULT_OPENAI_LLM_REQUEST_TIMEOUT,
        )
    }

    /// Create a provider with full explicit configuration including timeouts.
    pub fn new_with_timeouts(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, LlmError> {
        validate_timeout("openai llm connect timeout", connect_timeout)?;
        validate_timeout("openai llm request timeout", request_timeout)?;

        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|error| {
                LlmError::Provider(format!(
                    "failed to build OpenAI LLM HTTP client with configured timeouts: {error}"
                ))
            })?;

        let base_url = normalize_base_url(base_url.into())?;
        let model = normalize_model_id(model.into())?;
        let api_key = normalize_api_key(api_key.into())?;

        Ok(Self {
            client,
            base_url,
            model,
            api_key,
            connect_timeout,
            request_timeout,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    fn map_request_error(&self, error: &reqwest::Error) -> LlmError {
        if error.is_timeout() {
            return LlmError::Provider(format!(
                "openai llm not reachable at {}: request timed out after {}",
                self.base_url,
                format_timeout(self.request_timeout)
            ));
        }

        if error.is_connect() {
            return LlmError::Provider(format!(
                "openai llm not reachable at {}: connection failed within {} ({error})",
                self.base_url,
                format_timeout(self.connect_timeout)
            ));
        }

        LlmError::Provider(format!("openai llm request failed: {error}"))
    }

    async fn map_status_error(
        &self,
        status: reqwest::StatusCode,
        response: reqwest::Response,
    ) -> LlmError {
        let body = match response.text().await {
            Ok(body) if !body.trim().is_empty() => body,
            Ok(_) => "<empty response body>".to_string(),
            Err(error) => format!("<failed to read error body: {error}>"),
        };

        match status.as_u16() {
            401 => LlmError::Provider(format!(
                "openai llm returned 401 Unauthorized: invalid API key ({body})"
            )),
            429 => LlmError::Provider(format!(
                "openai llm returned 429 Too Many Requests: rate limited, try again later ({body})"
            )),
            _ => LlmError::Provider(format!("openai llm request returned {status}: {body}")),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiLlmProvider {
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
            .bearer_auth(&self.api_key)
            .json(&OpenAiChatRequest {
                model: &self.model,
                messages: vec![OpenAiChatMessage {
                    role: "user",
                    content: prompt,
                }],
            })
            .send()
            .await
            .map_err(|error| self.map_request_error(&error))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.map_status_error(status, response).await);
        }

        let payload: OpenAiChatResponse = response.json().await.map_err(|error| {
            LlmError::InvalidResponse(format!("openai llm response decode failed: {error}"))
        })?;
        let completion = payload
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content.trim().to_string())
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                LlmError::InvalidResponse("openai llm returned no completion choice".to_string())
            })?;

        Ok(completion)
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

fn normalize_base_url(base_url: String) -> Result<String, LlmError> {
    let normalized = base_url.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return Err(LlmError::Provider(
            "openai llm base URL must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_model_id(model: String) -> Result<String, LlmError> {
    let normalized = model.trim().to_string();
    if normalized.is_empty() {
        return Err(LlmError::Provider(
            "openai llm model must not be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_api_key(api_key: String) -> Result<String, LlmError> {
    let normalized = api_key.trim().to_string();
    if normalized.is_empty() {
        return Err(LlmError::Provider(
            "openai llm API key must not be empty".to_string(),
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
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiChatMessage<'a>>,
}

#[derive(Debug, Serialize)]
struct OpenAiChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoiceMessage {
    content: String,
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
        OpenAiLlmProvider, DEFAULT_OPENAI_LLM_BASE_URL, DEFAULT_OPENAI_LLM_CONNECT_TIMEOUT,
        DEFAULT_OPENAI_LLM_MODEL, DEFAULT_OPENAI_LLM_REQUEST_TIMEOUT,
    };
    use crate::LlmProvider;

    #[test]
    fn new_provider_uses_openai_llm_defaults() {
        let provider = OpenAiLlmProvider::new("sk-test").expect("provider");
        assert_eq!(provider.base_url, DEFAULT_OPENAI_LLM_BASE_URL);
        assert_eq!(provider.model(), DEFAULT_OPENAI_LLM_MODEL);
        assert_eq!(provider.connect_timeout, DEFAULT_OPENAI_LLM_CONNECT_TIMEOUT);
        assert_eq!(provider.request_timeout, DEFAULT_OPENAI_LLM_REQUEST_TIMEOUT);
    }

    #[test]
    fn new_provider_normalizes_base_url_trailing_slash() {
        let provider = OpenAiLlmProvider::new_with_config(
            " https://api.openai.com/ ",
            " gpt-4.1-mini ",
            " sk-test ",
        )
        .expect("provider");
        assert_eq!(provider.base_url, DEFAULT_OPENAI_LLM_BASE_URL);
        assert_eq!(provider.model(), DEFAULT_OPENAI_LLM_MODEL);
    }

    #[tokio::test]
    async fn complete_parses_valid_chat_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 2048];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            assert!(request.contains("POST /v1/chat/completions "));
            let body = "{\"choices\":[{\"message\":{\"content\":\"AGREE\"}}]}";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let provider = OpenAiLlmProvider::new_with_config(
            format!("http://{address}"),
            "gpt-4.1-mini",
            "sk-test",
        )
        .expect("provider");
        let completion = provider
            .complete("check contradiction")
            .await
            .expect("completion");
        assert_eq!(completion, "AGREE");
    }

    #[tokio::test]
    async fn complete_invalid_api_key_yields_clear_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer).expect("read request");
            let body = "{\"error\":{\"message\":\"bad key\"}}";
            let response = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let provider = OpenAiLlmProvider::new_with_config(
            format!("http://{address}"),
            "gpt-4.1-mini",
            "sk-test",
        )
        .expect("provider");
        let error = provider
            .complete("check contradiction")
            .await
            .expect_err("401");
        assert!(error.to_string().contains("invalid API key"));
    }

    #[tokio::test]
    async fn complete_connection_errors_are_clear() {
        let closed_port = TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("ephemeral address")
            .port();
        let provider = OpenAiLlmProvider::new_with_config(
            format!("http://127.0.0.1:{closed_port}"),
            "gpt-4.1-mini",
            "sk-test",
        )
        .expect("provider");

        let error = provider
            .complete("merge these")
            .await
            .expect_err("connection failure");
        assert!(error.to_string().contains("openai llm not reachable at"));
    }

    #[tokio::test]
    async fn complete_timeout_errors_are_clear() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept request");
            thread::sleep(Duration::from_millis(200));
        });

        let provider = OpenAiLlmProvider::new_with_timeouts(
            format!("http://{address}"),
            "gpt-4.1-mini",
            "sk-test",
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
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer).expect("read request");
            let body = "{\"error\":{\"message\":\"slow down\"}}";
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let provider = OpenAiLlmProvider::new_with_config(
            format!("http://{address}"),
            "gpt-4.1-mini",
            "sk-test",
        )
        .expect("provider");
        let error = provider
            .complete("merge these")
            .await
            .expect_err("rate limit should surface");

        assert!(error.to_string().contains("rate limited"));
    }
}
