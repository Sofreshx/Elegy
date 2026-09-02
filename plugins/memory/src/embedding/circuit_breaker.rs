use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::error::EmbeddingError;
use crate::traits::EmbeddingProvider;

enum BreakerState {
    Closed { consecutive_failures: u32 },
    Open { until: std::time::Instant },
    HalfOpen,
}

pub struct CircuitBreakerEmbeddingProvider {
    inner: Arc<dyn EmbeddingProvider>,
    state: std::sync::Mutex<BreakerState>,
    threshold: u32,
    cooldown: Duration,
}

impl CircuitBreakerEmbeddingProvider {
    pub fn new(inner: Arc<dyn EmbeddingProvider>, threshold: u32, cooldown: Duration) -> Self {
        Self {
            inner,
            state: std::sync::Mutex::new(BreakerState::Closed {
                consecutive_failures: 0,
            }),
            threshold,
            cooldown,
        }
    }

    pub fn from_env(inner: Arc<dyn EmbeddingProvider>) -> Self {
        let threshold = std::env::var("ELEGY_EMBEDDING_BREAKER_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let cooldown_secs = std::env::var("ELEGY_EMBEDDING_BREAKER_COOLDOWN_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        Self::new(inner, threshold, Duration::from_secs(cooldown_secs))
    }

    fn should_short_circuit(&self) -> Option<EmbeddingError> {
        let mut state = self.state.lock().expect("circuit breaker lock poisoned");
        match &*state {
            BreakerState::Open { until } => {
                let now = std::time::Instant::now();
                if now < *until {
                    let remaining = (*until - now).as_secs();
                    Some(EmbeddingError::Provider(format!(
                        "embedding provider circuit is open after {} consecutive failures; retrying in {remaining}s",
                        self.threshold,
                    )))
                } else {
                    *state = BreakerState::HalfOpen;
                    None
                }
            }
            BreakerState::Closed { .. } | BreakerState::HalfOpen => None,
        }
    }

    fn record_success(&self) {
        let mut state = self.state.lock().expect("circuit breaker lock poisoned");
        *state = BreakerState::Closed {
            consecutive_failures: 0,
        };
    }

    fn record_failure(&self) {
        if self.threshold == 0 {
            // threshold == 0 disables the breaker entirely; never open.
            return;
        }
        let mut state = self.state.lock().expect("circuit breaker lock poisoned");
        match &*state {
            BreakerState::Closed {
                consecutive_failures,
            } => {
                let new_count = consecutive_failures + 1;
                if new_count >= self.threshold {
                    *state = BreakerState::Open {
                        until: std::time::Instant::now() + self.cooldown,
                    };
                } else {
                    *state = BreakerState::Closed {
                        consecutive_failures: new_count,
                    };
                }
            }
            BreakerState::HalfOpen => {
                *state = BreakerState::Open {
                    until: std::time::Instant::now() + self.cooldown,
                };
            }
            BreakerState::Open { .. } => {}
        }
    }
}

#[async_trait]
impl EmbeddingProvider for CircuitBreakerEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if let Some(err) = self.should_short_circuit() {
            return Err(err);
        }

        let result = self.inner.embed(text).await;
        match &result {
            Ok(_) => self.record_success(),
            Err(_) => self.record_failure(),
        }
        result
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }

    fn model_id(&self) -> &str {
        self.inner.model_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct StubProvider {
        call_count: AtomicU32,
        fail_count: AtomicU32,
        always_fail: bool,
    }

    impl StubProvider {
        fn always_succeed() -> Arc<Self> {
            Arc::new(Self {
                call_count: AtomicU32::new(0),
                fail_count: AtomicU32::new(0),
                always_fail: false,
            })
        }

        fn always_fail() -> Arc<Self> {
            Arc::new(Self {
                call_count: AtomicU32::new(0),
                fail_count: AtomicU32::new(0),
                always_fail: true,
            })
        }

        fn fail_then_succeed(fail_n: u32) -> Arc<Self> {
            Arc::new(Self {
                call_count: AtomicU32::new(0),
                fail_count: AtomicU32::new(fail_n),
                always_fail: false,
            })
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    /// A provider that replays a fixed, ordered sequence of outcomes (true = success).
    /// Panics if called more times than the script provides — that indicates the test's
    /// own arithmetic is wrong, not the breaker.
    struct ScriptedProvider {
        script: std::sync::Mutex<std::collections::VecDeque<bool>>,
        call_count: AtomicU32,
    }

    impl ScriptedProvider {
        fn new(script: &[bool]) -> Arc<Self> {
            Arc::new(Self {
                script: std::sync::Mutex::new(script.iter().copied().collect()),
                call_count: AtomicU32::new(0),
            })
        }

        fn call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl EmbeddingProvider for ScriptedProvider {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let succeed = self
                .script
                .lock()
                .expect("script lock poisoned")
                .pop_front()
                .expect("scripted provider called more times than the test scripted");
            if succeed {
                Ok(vec![1.0; 3])
            } else {
                Err(EmbeddingError::Provider("stub failure".into()))
            }
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn model_id(&self) -> &str {
            "scripted-model"
        }
    }

    #[async_trait]
    impl EmbeddingProvider for StubProvider {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if self.always_fail || self.fail_count.load(Ordering::SeqCst) > 0 {
                self.fail_count.fetch_sub(1, Ordering::SeqCst);
                Err(EmbeddingError::Provider("stub failure".into()))
            } else {
                Ok(vec![1.0; 3])
            }
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn model_id(&self) -> &str {
            "stub-model"
        }
    }

    #[tokio::test]
    async fn opens_after_threshold_consecutive_failures() {
        let inner = StubProvider::always_fail();
        let cb = CircuitBreakerEmbeddingProvider::new(
            Arc::clone(&inner) as Arc<dyn EmbeddingProvider>,
            3,
            Duration::from_secs(60),
        );

        for _ in 0..3 {
            let _ = cb.embed("test").await;
        }

        assert_eq!(inner.call_count(), 3);

        let result = cb.embed("test").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("circuit is open"));

        assert_eq!(inner.call_count(), 3, "inner should not be called again");
    }

    #[tokio::test]
    async fn half_open_retries_after_cooldown() {
        let inner = StubProvider::fail_then_succeed(3);
        let cb = CircuitBreakerEmbeddingProvider::new(
            Arc::clone(&inner) as Arc<dyn EmbeddingProvider>,
            3,
            Duration::from_millis(50),
        );

        for _ in 0..3 {
            let _ = cb.embed("test").await;
        }
        assert_eq!(inner.call_count(), 3);

        let result = cb.embed("test").await;
        assert!(result.is_err(), "half-open trial should fail");

        let result = cb.embed("test").await;
        assert!(result.is_err(), "should still be open after failed trial");

        tokio::time::sleep(Duration::from_millis(60)).await;

        let result = cb.embed("test").await;
        assert!(
            result.is_ok(),
            "should succeed after cooldown with remaining failures exhausted"
        );
    }

    #[tokio::test]
    async fn single_success_resets_failure_counter() {
        // threshold = 3: 2 fails, a success (must reset the counter), then only 2 more
        // fails (must NOT open - if the reset did not happen, the cumulative count would
        // already be 4 and it would have opened one call early), then a 3rd fail that
        // does open it.
        let inner = ScriptedProvider::new(&[false, false, true, false, false, false]);
        let cb = CircuitBreakerEmbeddingProvider::new(
            Arc::clone(&inner) as Arc<dyn EmbeddingProvider>,
            3,
            Duration::from_secs(60),
        );

        assert!(cb.embed("fail1").await.is_err());
        assert!(cb.embed("fail2").await.is_err());
        assert!(
            cb.embed("ok").await.is_ok(),
            "success should reset the counter"
        );

        assert!(cb.embed("fail3").await.is_err());
        assert!(cb.embed("fail4").await.is_err());
        assert_eq!(
            inner.call_count(),
            5,
            "only 2 consecutive fails since the reset; must not have opened yet"
        );

        assert!(
            cb.embed("fail5").await.is_err(),
            "3rd consecutive fail since reset"
        );
        assert_eq!(inner.call_count(), 6);

        let result = cb.embed("test").await;
        assert!(result.is_err(), "should be open now");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("circuit is open"));
        assert_eq!(
            inner.call_count(),
            6,
            "short-circuited call must not reach inner"
        );
    }

    #[tokio::test]
    async fn threshold_zero_never_opens() {
        let inner = StubProvider::always_fail();
        let cb = CircuitBreakerEmbeddingProvider::new(
            Arc::clone(&inner) as Arc<dyn EmbeddingProvider>,
            0,
            Duration::from_secs(60),
        );

        for _ in 0..20 {
            let result = cb.embed("test").await;
            assert!(result.is_err(), "should fail through to inner every time");
        }

        assert_eq!(inner.call_count(), 20);
    }

    #[tokio::test]
    async fn embed_batch_short_circuits_on_first_failure() {
        let inner = StubProvider::fail_then_succeed(1);
        let cb = CircuitBreakerEmbeddingProvider::new(
            Arc::clone(&inner) as Arc<dyn EmbeddingProvider>,
            1,
            Duration::from_secs(60),
        );

        let result = cb.embed_batch(&["a", "b", "c"]).await;
        assert!(result.is_err());
        assert_eq!(inner.call_count(), 1, "only first call should go through");
    }

    #[tokio::test]
    async fn delegates_dimensions_and_model_id() {
        let inner = StubProvider::always_succeed();
        let cb = CircuitBreakerEmbeddingProvider::new(
            Arc::clone(&inner) as Arc<dyn EmbeddingProvider>,
            5,
            Duration::from_secs(60),
        );

        assert_eq!(cb.dimensions(), 3);
        assert_eq!(cb.model_id(), "stub-model");
    }
}
