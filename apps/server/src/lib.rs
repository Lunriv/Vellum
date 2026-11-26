//! Vellum backend crate root.
//! Modules follow the strict dependency directions defined in docs/ARCHITECTURE_PLAN.md.

pub mod api;
pub mod kernel;
pub mod models;
pub mod modules;
pub mod narration;
pub mod persistence;
pub mod runtime;
pub mod stream;

use std::sync::{Arc, Mutex};

use axum::Router;
use persistence::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub locks: runtime::lock::SessionCommandLocks,
    pub buffers: Arc<stream::buffer::BufferRegistry>,
    pub provider: Arc<dyn models::LlmProvider>,
    pub modules: Arc<modules::ModuleRegistry>,
    pub rng: Arc<Mutex<Box<dyn kernel::DiceRng>>>,
    pub llm_mode: String,
}

impl AppState {
    pub fn new(
        db: Database,
        provider: Arc<dyn models::LlmProvider>,
        modules: Arc<modules::ModuleRegistry>,
        llm_mode: impl Into<String>,
    ) -> Self {
        Self {
            db,
            locks: Default::default(),
            buffers: Arc::new(Default::default()),
            provider,
            modules,
            rng: Arc::new(Mutex::new(Box::new(kernel::OsDiceRng::default()))),
            llm_mode: llm_mode.into(),
        }
    }

    pub fn with_rng(mut self, rng: Box<dyn kernel::DiceRng>) -> Self {
        self.rng = Arc::new(Mutex::new(rng));
        self
    }

    pub fn with_buffers(mut self, buffers: Arc<stream::buffer::BufferRegistry>) -> Self {
        self.buffers = buffers;
        self
    }
}

pub fn build_app(state: AppState) -> Router {
    api::router(state)
}

pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
