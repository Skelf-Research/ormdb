use crate::config::StudioConfig;
use crate::session::SessionManager;
use std::sync::Arc;

/// Application state shared across all routes
#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub config: StudioConfig,
}

impl AppState {
    pub fn new(config: StudioConfig) -> Self {
        Self {
            sessions: Arc::new(SessionManager::new(config.clone())),
            config,
        }
    }
}
