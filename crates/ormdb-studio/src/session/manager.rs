use crate::config::StudioConfig;
use crate::error::{Result, StudioError};
use crate::session::SessionDatabase;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A user session with its own isolated database
pub struct Session {
    pub id: String,
    pub created_at: Instant,
    last_activity: AtomicU64,
    pub database: SessionDatabase,
}

impl Session {
    fn new(id: String, database: SessionDatabase) -> Self {
        Self {
            id,
            created_at: Instant::now(),
            last_activity: AtomicU64::new(Self::now_timestamp()),
            database,
        }
    }

    fn now_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn touch(&self) {
        self.last_activity.store(Self::now_timestamp(), Ordering::SeqCst);
    }

    pub fn last_activity_secs(&self) -> u64 {
        self.last_activity.load(Ordering::SeqCst)
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        let last = self.last_activity.load(Ordering::SeqCst);
        let now = Self::now_timestamp();
        now.saturating_sub(last) > timeout.as_secs()
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Manages all active sessions
pub struct SessionManager {
    sessions: DashMap<String, Arc<Session>>,
    config: StudioConfig,
}

impl SessionManager {
    pub fn new(config: StudioConfig) -> Self {
        Self {
            sessions: DashMap::new(),
            config,
        }
    }

    /// Create a new session with a temporary database
    pub fn create_session(&self) -> Result<Arc<Session>> {
        if self.sessions.len() >= self.config.max_sessions {
            return Err(StudioError::TooManySessions(self.config.max_sessions));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let database = SessionDatabase::new_temporary()?;

        let session = Arc::new(Session::new(session_id.clone(), database));
        self.sessions.insert(session_id, session.clone());

        Ok(session)
    }

    /// Get a session by ID, updating its last activity time
    pub fn get_session(&self, id: &str) -> Option<Arc<Session>> {
        self.sessions.get(id).map(|entry| {
            let session = entry.clone();
            session.touch();
            session
        })
    }

    /// Check if a session exists
    pub fn has_session(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    /// Delete a session
    pub fn delete_session(&self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    /// Get the number of active sessions
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let timeout = self.config.session_timeout;
        let before = self.sessions.len();

        self.sessions.retain(|_, session| !session.is_expired(timeout));

        before - self.sessions.len()
    }

    /// Get info about all sessions (for debugging/admin)
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .iter()
            .map(|entry| {
                let session = entry.value();
                SessionInfo {
                    id: session.id.clone(),
                    age_secs: session.age().as_secs(),
                    last_activity_secs: Session::now_timestamp() - session.last_activity_secs(),
                }
            })
            .collect()
    }

    /// Get the configuration
    pub fn config(&self) -> &StudioConfig {
        &self.config
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub age_secs: u64,
    pub last_activity_secs: u64,
}

/// Background task to periodically clean up expired sessions
pub async fn cleanup_task(manager: Arc<SessionManager>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let cleaned = manager.cleanup_expired();
        if cleaned > 0 {
            tracing::info!("Cleaned up {} expired sessions", cleaned);
        }
    }
}
