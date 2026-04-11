//! Tokio <-> GPUI bridge: owns a multi-threaded tokio runtime for backend async.

use std::sync::Arc;

#[derive(Clone)]
pub struct TokioBridge {
    rt: Arc<tokio::runtime::Runtime>,
}

impl TokioBridge {
    pub fn new() -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");
        Self { rt: Arc::new(rt) }
    }

    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.rt
    }

    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.rt.spawn(future)
    }

    pub fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.rt.block_on(future)
    }
}

impl Default for TokioBridge {
    fn default() -> Self {
        Self::new()
    }
}
