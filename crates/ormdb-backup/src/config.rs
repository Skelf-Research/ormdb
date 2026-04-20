//! Backup configuration.

use crate::error::{BackupError, Result};

/// Configuration for backup operations.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// S3 endpoint URL (None for AWS default).
    pub endpoint: Option<String>,
    /// AWS region.
    pub region: String,
    /// S3 bucket name.
    pub bucket: String,
    /// Prefix for all backup objects.
    pub prefix: String,
    /// Compression level (1-22, default 3).
    pub compression_level: i32,
    /// Chunk size in bytes (default 64MB).
    pub chunk_size_bytes: usize,
    /// Verify checksums after backup.
    pub verify_after_backup: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            region: "us-east-1".to_string(),
            bucket: String::new(),
            prefix: "ormdb-backups/".to_string(),
            compression_level: 3,
            chunk_size_bytes: 64 * 1024 * 1024, // 64MB
            verify_after_backup: true,
        }
    }
}

impl BackupConfig {
    /// Create a new builder for BackupConfig.
    pub fn builder() -> BackupConfigBuilder {
        BackupConfigBuilder::default()
    }

    /// Parse from S3 URL (s3://bucket/prefix).
    pub fn from_url(url: &str) -> Result<Self> {
        if !url.starts_with("s3://") {
            return Err(BackupError::InvalidConfig(format!(
                "URL must start with s3://, got: {}",
                url
            )));
        }

        let rest = &url[5..]; // Remove "s3://"
        let parts: Vec<&str> = rest.splitn(2, '/').collect();

        let bucket = parts
            .first()
            .ok_or_else(|| BackupError::InvalidConfig("missing bucket name".to_string()))?
            .to_string();

        let prefix = parts.get(1).map(|s| s.to_string()).unwrap_or_default();

        Ok(Self {
            bucket,
            prefix,
            ..Default::default()
        })
    }

    /// Get the full path for snapshots.
    pub fn snapshots_path(&self) -> String {
        format!("{}snapshots/", self.prefix)
    }

    /// Get the full path for incremental backups.
    pub fn incremental_path(&self) -> String {
        format!("{}incremental/", self.prefix)
    }

    /// Get the full path for metadata.
    pub fn metadata_path(&self) -> String {
        format!("{}metadata/", self.prefix)
    }
}

/// Builder for BackupConfig.
#[derive(Debug, Default)]
pub struct BackupConfigBuilder {
    endpoint: Option<String>,
    region: Option<String>,
    bucket: Option<String>,
    prefix: Option<String>,
    compression_level: Option<i32>,
    chunk_size_bytes: Option<usize>,
    verify_after_backup: Option<bool>,
}

impl BackupConfigBuilder {
    /// Set the S3 endpoint (for S3-compatible services).
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the AWS region.
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set the S3 bucket name.
    pub fn bucket(mut self, bucket: impl Into<String>) -> Self {
        self.bucket = Some(bucket.into());
        self
    }

    /// Set the prefix for backup objects.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the compression level (1-22).
    pub fn compression_level(mut self, level: i32) -> Self {
        self.compression_level = Some(level);
        self
    }

    /// Set the chunk size in bytes.
    pub fn chunk_size_bytes(mut self, size: usize) -> Self {
        self.chunk_size_bytes = Some(size);
        self
    }

    /// Set whether to verify after backup.
    pub fn verify_after_backup(mut self, verify: bool) -> Self {
        self.verify_after_backup = Some(verify);
        self
    }

    /// Build the configuration.
    pub fn build(self) -> Result<BackupConfig> {
        let bucket = self
            .bucket
            .ok_or_else(|| BackupError::InvalidConfig("bucket is required".to_string()))?;

        Ok(BackupConfig {
            endpoint: self.endpoint,
            region: self.region.unwrap_or_else(|| "us-east-1".to_string()),
            bucket,
            prefix: self
                .prefix
                .unwrap_or_else(|| "ormdb-backups/".to_string()),
            compression_level: self.compression_level.unwrap_or(3),
            chunk_size_bytes: self.chunk_size_bytes.unwrap_or(64 * 1024 * 1024),
            verify_after_backup: self.verify_after_backup.unwrap_or(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_url() {
        let config = BackupConfig::from_url("s3://my-bucket/backups/").unwrap();
        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.prefix, "backups/");
    }

    #[test]
    fn test_builder() {
        let config = BackupConfig::builder()
            .bucket("test-bucket")
            .region("eu-west-1")
            .prefix("test/")
            .compression_level(5)
            .build()
            .unwrap();

        assert_eq!(config.bucket, "test-bucket");
        assert_eq!(config.region, "eu-west-1");
        assert_eq!(config.prefix, "test/");
        assert_eq!(config.compression_level, 5);
    }

    #[test]
    fn test_builder_requires_bucket() {
        let result = BackupConfig::builder().region("us-east-1").build();
        assert!(result.is_err());
    }
}
