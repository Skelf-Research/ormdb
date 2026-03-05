//! Connection mode detection.
//!
//! Auto-detects whether to use embedded mode or client mode based on the target string.

use std::path::PathBuf;

/// The connection mode for the CLI.
#[derive(Debug, Clone)]
pub enum ConnectionMode {
    /// Embedded mode - opens a local database file or in-memory database.
    Embedded {
        /// Path to the database directory, or None for in-memory.
        path: Option<PathBuf>,
    },
    /// Client mode - connects to a remote ORMDB server.
    Client {
        /// Server URL (tcp:// or ipc://).
        url: String,
    },
}

impl ConnectionMode {
    /// Detect the connection mode from a target string.
    ///
    /// Rules:
    /// - `:memory:` → Embedded in-memory
    /// - `tcp://...` or `ipc://...` → Client mode
    /// - Anything else → Embedded with file path
    pub fn detect(target: &str) -> Self {
        let target = target.trim();

        if target == ":memory:" {
            ConnectionMode::Embedded { path: None }
        } else if target.starts_with("tcp://") || target.starts_with("ipc://") {
            ConnectionMode::Client {
                url: target.to_string(),
            }
        } else {
            // Assume it's a file path for embedded mode
            ConnectionMode::Embedded {
                path: Some(PathBuf::from(target)),
            }
        }
    }

    /// Check if this is embedded mode.
    pub fn is_embedded(&self) -> bool {
        matches!(self, ConnectionMode::Embedded { .. })
    }

    /// Check if this is client mode.
    pub fn is_client(&self) -> bool {
        matches!(self, ConnectionMode::Client { .. })
    }

    /// Get a display name for the connection.
    pub fn display_name(&self) -> String {
        match self {
            ConnectionMode::Embedded { path: None } => ":memory:".to_string(),
            ConnectionMode::Embedded { path: Some(p) } => p.display().to_string(),
            ConnectionMode::Client { url } => url.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_memory() {
        let mode = ConnectionMode::detect(":memory:");
        assert!(matches!(mode, ConnectionMode::Embedded { path: None }));
    }

    #[test]
    fn test_detect_tcp() {
        let mode = ConnectionMode::detect("tcp://localhost:9000");
        assert!(matches!(mode, ConnectionMode::Client { url } if url == "tcp://localhost:9000"));
    }

    #[test]
    fn test_detect_ipc() {
        let mode = ConnectionMode::detect("ipc:///tmp/ormdb.sock");
        assert!(matches!(mode, ConnectionMode::Client { url } if url == "ipc:///tmp/ormdb.sock"));
    }

    #[test]
    fn test_detect_path() {
        let mode = ConnectionMode::detect("./my_data");
        assert!(matches!(mode, ConnectionMode::Embedded { path: Some(p) } if p == PathBuf::from("./my_data")));
    }

    #[test]
    fn test_detect_absolute_path() {
        let mode = ConnectionMode::detect("/var/lib/ormdb/data");
        assert!(matches!(mode, ConnectionMode::Embedded { path: Some(p) } if p == PathBuf::from("/var/lib/ormdb/data")));
    }
}
