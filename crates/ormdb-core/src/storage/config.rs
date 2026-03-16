//! Storage configuration.

use std::path::PathBuf;

/// Configuration for the storage engine.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Path to the database directory.
    pub path: PathBuf,

    /// Page cache capacity in bytes.
    pub cache_capacity: u64,

    /// Flush interval in milliseconds. None means flush on every write.
    pub flush_every_ms: Option<u64>,

    /// Enable zstd compression.
    pub compression: bool,

    /// Temporary database (deleted on drop).
    pub temporary: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./ormdb_data"),
            cache_capacity: 1024 * 1024 * 1024, // 1GB
            flush_every_ms: Some(1000),         // Flush every second
            compression: true,
            temporary: false,
        }
    }
}

impl StorageConfig {
    /// Create a new configuration with the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            ..Default::default()
        }
    }

    /// Create a temporary in-memory configuration for testing.
    pub fn temporary() -> Self {
        Self {
            path: PathBuf::from(""),
            temporary: true,
            ..Default::default()
        }
    }

    /// Convert to sled configuration.
    pub(crate) fn to_sled_config(&self) -> sled::Config {
        let mut config = sled::Config::new()
            .cache_capacity(self.cache_capacity)
            .use_compression(self.compression);

        if self.temporary {
            config = config.temporary(true);
        } else {
            config = config.path(&self.path);
        }

        if let Some(ms) = self.flush_every_ms {
            config = config.flush_every_ms(Some(ms));
        }

        config
    }
}
