//! Storage configuration.

use std::path::PathBuf;
use std::time::Duration;

/// Configuration for version retention and garbage collection.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// Keep versions for this duration. None means keep forever.
    pub ttl: Option<Duration>,

    /// Keep at most N versions per entity. None means unlimited.
    /// The most recent versions are always kept.
    pub max_versions: Option<usize>,

    /// Minimum age before a version is eligible for cleanup.
    /// This prevents cleanup of very recent versions even if max_versions is exceeded.
    pub min_age: Duration,

    /// Whether to clean up tombstones after the retention period.
    pub cleanup_tombstones: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            ttl: None,                          // Keep forever by default
            max_versions: Some(100),            // Keep last 100 versions
            min_age: Duration::from_secs(3600), // At least 1 hour old
            cleanup_tombstones: true,
        }
    }
}

impl RetentionPolicy {
    /// Create a policy that keeps all versions forever.
    pub fn keep_all() -> Self {
        Self {
            ttl: None,
            max_versions: None,
            min_age: Duration::from_secs(0),
            cleanup_tombstones: false,
        }
    }

    /// Create a policy that keeps versions for a specific duration.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl: Some(ttl),
            ..Default::default()
        }
    }

    /// Create a policy that keeps a specific number of versions.
    pub fn with_max_versions(max: usize) -> Self {
        Self {
            max_versions: Some(max),
            ..Default::default()
        }
    }

    /// Set the TTL.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the maximum versions.
    pub fn max_versions(mut self, max: usize) -> Self {
        self.max_versions = Some(max);
        self
    }

    /// Set the minimum age.
    pub fn min_age(mut self, age: Duration) -> Self {
        self.min_age = age;
        self
    }

    /// Set whether to cleanup tombstones.
    pub fn cleanup_tombstones(mut self, cleanup: bool) -> Self {
        self.cleanup_tombstones = cleanup;
        self
    }
}

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

    /// Version retention and GC policy.
    pub retention: RetentionPolicy,

    /// Interval between compaction runs. None disables automatic compaction.
    pub compaction_interval: Option<Duration>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./ormdb_data"),
            cache_capacity: 1024 * 1024 * 1024, // 1GB
            flush_every_ms: Some(1000),         // Flush every second
            compression: true,
            temporary: false,
            retention: RetentionPolicy::default(),
            compaction_interval: Some(Duration::from_secs(3600)), // Every hour
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

    /// Set the retention policy.
    pub fn with_retention(mut self, retention: RetentionPolicy) -> Self {
        self.retention = retention;
        self
    }

    /// Set the compaction interval.
    pub fn with_compaction_interval(mut self, interval: Duration) -> Self {
        self.compaction_interval = Some(interval);
        self
    }

    /// Disable automatic compaction.
    pub fn without_compaction(mut self) -> Self {
        self.compaction_interval = None;
        self
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
