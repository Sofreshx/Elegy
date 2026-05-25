use std::borrow::Cow;

use crate::EmbeddingProvider;

mod ollama;
mod openai;

pub use ollama::{
    OllamaEmbeddingProvider, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_CONNECT_TIMEOUT,
    DEFAULT_OLLAMA_DIMENSIONS, DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_REQUEST_TIMEOUT,
};
pub use openai::{
    OpenAiEmbeddingProvider, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_CONNECT_TIMEOUT,
    DEFAULT_OPENAI_DIMENSIONS, DEFAULT_OPENAI_MODEL, DEFAULT_OPENAI_REQUEST_TIMEOUT,
};

const NOMIC_EMBED_TEXT_MODEL_PREFIX: &str = "nomic-embed-text";
const NOMIC_SEARCH_DOCUMENT_PREFIX: &str = "search_document: ";
const NOMIC_SEARCH_QUERY_PREFIX: &str = "search_query: ";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EmbeddingTask {
    Document,
    Query,
}

impl EmbeddingTask {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Document => NOMIC_SEARCH_DOCUMENT_PREFIX,
            Self::Query => NOMIC_SEARCH_QUERY_PREFIX,
        }
    }
}

pub(crate) fn prepare_embedding_input<'a>(
    provider: &dyn EmbeddingProvider,
    task: EmbeddingTask,
    text: &'a str,
) -> Cow<'a, str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed(trimmed);
    }

    if model_requires_nomic_task_prefix(provider.model_id()) {
        let mut prefixed = String::with_capacity(task.prefix().len() + trimmed.len());
        prefixed.push_str(task.prefix());
        prefixed.push_str(trimmed);
        Cow::Owned(prefixed)
    } else {
        Cow::Borrowed(trimmed)
    }
}

fn model_requires_nomic_task_prefix(model_id: &str) -> bool {
    let normalized = model_id.trim().to_ascii_lowercase();
    let Some(suffix) = normalized.strip_prefix(NOMIC_EMBED_TEXT_MODEL_PREFIX) else {
        return false;
    };

    suffix.is_empty() || suffix.starts_with(':')
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::{prepare_embedding_input, EmbeddingTask};
    use crate::{EmbeddingError, EmbeddingProvider};

    struct StubEmbeddingProvider {
        model_id: &'static str,
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(vec![0.0; 3])
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn model_id(&self) -> &str {
            self.model_id
        }
    }

    #[test]
    fn nomic_document_embeddings_receive_task_prefix() {
        let provider = StubEmbeddingProvider {
            model_id: "nomic-embed-text:latest",
        };

        let prepared = prepare_embedding_input(
            &provider,
            EmbeddingTask::Document,
            "  durable memory content  ",
        );

        assert_eq!(prepared, "search_document: durable memory content");
    }

    #[test]
    fn nomic_query_embeddings_receive_task_prefix() {
        let provider = StubEmbeddingProvider {
            model_id: "nomic-embed-text",
        };

        let prepared =
            prepare_embedding_input(&provider, EmbeddingTask::Query, " concept-only query ");

        assert_eq!(prepared, "search_query: concept-only query");
    }

    #[test]
    fn non_nomic_models_keep_trimmed_input_unchanged() {
        let provider = StubEmbeddingProvider {
            model_id: "text-embedding-3-small",
        };

        let prepared = prepare_embedding_input(
            &provider,
            EmbeddingTask::Document,
            "  no task prefix here  ",
        );

        assert_eq!(prepared, "no task prefix here");
    }
}
