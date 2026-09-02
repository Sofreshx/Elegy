use std::sync::OnceLock;

use thiserror::Error;
use tokio::runtime::{Builder, Handle, Runtime};

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to build shared tokio runtime: {0}")]
    Build(#[from] std::io::Error),
    #[error("shared runtime worker thread panicked")]
    WorkerPanicked,
}

impl Clone for RuntimeError {
    fn clone(&self) -> Self {
        match self {
            Self::Build(io) => Self::Build(std::io::Error::new(io.kind(), io.to_string())),
            Self::WorkerPanicked => Self::WorkerPanicked,
        }
    }
}

// OnceLock<Result<..>>: build failure does not poison the lock; next call retries.
static SHARED: OnceLock<Result<Runtime, RuntimeError>> = OnceLock::new();

fn default_worker_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(4)
}

fn parse_worker_threads() -> usize {
    match std::env::var("ELEGY_RUNTIME_WORKER_THREADS") {
        Ok(val) => match val.trim().parse::<usize>() {
            Ok(n) => n.clamp(1, 32),
            Err(_) => default_worker_threads(),
        },
        Err(_) => default_worker_threads(),
    }
}

fn build_runtime() -> Result<Runtime, RuntimeError> {
    let workers = parse_worker_threads();
    Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .thread_name("elegy-memory")
        .build()
        .map_err(RuntimeError::Build)
}

pub fn shared() -> Result<&'static Runtime, RuntimeError> {
    SHARED
        .get_or_init(build_runtime)
        .as_ref()
        .map_err(Clone::clone)
}

pub fn handle() -> Result<Handle, RuntimeError> {
    shared().map(|rt| rt.handle().clone())
}

pub fn block_on<F>(future: F) -> Result<F::Output, RuntimeError>
where
    F: std::future::Future + Send,
    F::Output: Send,
{
    if let Ok(current) = Handle::try_current() {
        // Cannot call .block_on() inside a runtime — hop to a scoped OS thread.
        std::thread::scope(|s| {
            let result = s.spawn(|| current.block_on(future)).join();
            result.map_err(|_| RuntimeError::WorkerPanicked)
        })
    } else {
        Ok(shared()?.handle().block_on(future))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // cargo test runs tests in parallel threads by default and
    // ELEGY_RUNTIME_WORKER_THREADS is process-global; serialize the tests below so
    // they don't observe each other's temporary env mutation.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn block_on_returns_value() {
        let val = block_on(async { 42 }).expect("block_on failed");
        assert_eq!(val, 42);
    }

    #[test]
    fn block_on_from_inside_runtime() {
        let rt = Runtime::new().expect("runtime build failed");
        let val = rt
            .block_on(async { block_on(async { 7 }) })
            .expect("nested block_on failed");
        assert_eq!(val, 7);
    }

    #[test]
    fn worker_threads_default() {
        let _guard = env_lock();
        assert!(parse_worker_threads() >= 1);
        assert!(parse_worker_threads() <= 4);
    }

    #[test]
    fn worker_threads_clamps_high() {
        let _guard = env_lock();
        std::env::set_var("ELEGY_RUNTIME_WORKER_THREADS", "999");
        assert_eq!(parse_worker_threads(), 32);
        std::env::remove_var("ELEGY_RUNTIME_WORKER_THREADS");
    }

    #[test]
    fn worker_threads_clamps_zero_up_to_one() {
        let _guard = env_lock();
        std::env::set_var("ELEGY_RUNTIME_WORKER_THREADS", "0");
        assert_eq!(parse_worker_threads(), 1);
        std::env::remove_var("ELEGY_RUNTIME_WORKER_THREADS");
    }

    #[test]
    fn worker_threads_parses_garbage() {
        let _guard = env_lock();
        let expected = default_worker_threads();
        std::env::set_var("ELEGY_RUNTIME_WORKER_THREADS", "not-a-number");
        assert_eq!(parse_worker_threads(), expected);
        std::env::remove_var("ELEGY_RUNTIME_WORKER_THREADS");
    }

    #[test]
    fn runtime_error_display() {
        let err = RuntimeError::WorkerPanicked;
        assert_eq!(err.to_string(), "shared runtime worker thread panicked");
    }
}
