mod ollama;
mod openai;

pub use ollama::{
    OllamaLlmProvider, DEFAULT_OLLAMA_LLM_BASE_URL, DEFAULT_OLLAMA_LLM_CONNECT_TIMEOUT,
    DEFAULT_OLLAMA_LLM_MODEL, DEFAULT_OLLAMA_LLM_REQUEST_TIMEOUT,
};
pub use openai::{
    OpenAiLlmProvider, DEFAULT_OPENAI_LLM_BASE_URL, DEFAULT_OPENAI_LLM_CONNECT_TIMEOUT,
    DEFAULT_OPENAI_LLM_MODEL, DEFAULT_OPENAI_LLM_REQUEST_TIMEOUT,
};

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::{LlmError, LlmProvider};

    #[derive(Debug)]
    struct StubLlmProvider;

    #[async_trait]
    impl LlmProvider for StubLlmProvider {
        async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
            Ok(format!("stub::{prompt}"))
        }

        fn name(&self) -> &str {
            "stub"
        }

        fn model(&self) -> &str {
            "stub-model"
        }
    }

    async fn invoke(provider: &dyn LlmProvider, prompt: &str) -> Result<String, LlmError> {
        provider.complete(prompt).await
    }

    #[tokio::test]
    async fn llm_trait_objects_support_mock_providers() {
        let provider = StubLlmProvider;

        let completion = invoke(&provider, "merge me")
            .await
            .expect("mock completion");

        assert_eq!(completion, "stub::merge me");
        assert_eq!(provider.name(), "stub");
        assert_eq!(provider.model(), "stub-model");
    }
}
