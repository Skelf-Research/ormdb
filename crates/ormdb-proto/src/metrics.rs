//! Server metrics types.
//!
//! These types are returned by the GetMetrics operation to provide
//! server statistics and performance information.

use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Server metrics response.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct MetricsResult {
    /// Server uptime in seconds.
    pub uptime_secs: u64,
    /// Query metrics.
    pub queries: QueryMetrics,
    /// Mutation metrics.
    pub mutations: MutationMetrics,
    /// Plan cache metrics.
    pub cache: CacheMetrics,
    /// Storage metrics.
    pub storage: StorageMetrics,
    /// Transport layer metrics.
    pub transport: TransportMetrics,
}

/// Query-related metrics.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct QueryMetrics {
    /// Total queries executed.
    pub total_count: u64,
    /// Average query time in microseconds.
    pub avg_duration_us: u64,
    /// P50 (median) query time in microseconds.
    pub p50_duration_us: u64,
    /// P99 query time in microseconds.
    pub p99_duration_us: u64,
    /// Maximum query time in microseconds.
    pub max_duration_us: u64,
    /// Queries per entity type.
    pub by_entity: Vec<EntityQueryCount>,
}

/// Query count for a specific entity.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct EntityQueryCount {
    /// Entity name.
    pub entity: String,
    /// Number of queries for this entity.
    pub count: u64,
}

/// Mutation-related metrics.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct MutationMetrics {
    /// Total mutations executed.
    pub total_count: u64,
    /// Total insert operations.
    pub inserts: u64,
    /// Total update operations.
    pub updates: u64,
    /// Total delete operations.
    pub deletes: u64,
    /// Total upsert operations.
    pub upserts: u64,
    /// Total rows affected by mutations.
    pub rows_affected: u64,
}

/// Plan cache metrics.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct CacheMetrics {
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
    /// Hit rate (0.0 - 1.0).
    pub hit_rate: f64,
    /// Current number of cached plans.
    pub size: u64,
    /// Maximum cache capacity.
    pub capacity: u64,
    /// Number of evictions.
    pub evictions: u64,
}

/// Storage metrics.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct StorageMetrics {
    /// Entity counts by type.
    pub entity_counts: Vec<EntityCount>,
    /// Total entities across all types.
    pub total_entities: u64,
    /// Storage size in bytes (if available).
    pub size_bytes: Option<u64>,
    /// Number of active transactions.
    pub active_transactions: u64,
}

/// Entity count for a specific type.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct EntityCount {
    /// Entity type name.
    pub entity: String,
    /// Number of records.
    pub count: u64,
}

/// Transport layer metrics.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct TransportMetrics {
    /// Total requests received.
    pub requests_total: u64,
    /// Successful requests.
    pub requests_success: u64,
    /// Failed requests.
    pub requests_failed: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Current active connections.
    pub active_connections: u64,
}

impl MetricsResult {
    /// Create a new metrics result.
    pub fn new(
        uptime_secs: u64,
        queries: QueryMetrics,
        mutations: MutationMetrics,
        cache: CacheMetrics,
        storage: StorageMetrics,
        transport: TransportMetrics,
    ) -> Self {
        Self {
            uptime_secs,
            queries,
            mutations,
            cache,
            storage,
            transport,
        }
    }
}

impl Default for QueryMetrics {
    fn default() -> Self {
        Self {
            total_count: 0,
            avg_duration_us: 0,
            p50_duration_us: 0,
            p99_duration_us: 0,
            max_duration_us: 0,
            by_entity: Vec::new(),
        }
    }
}

impl Default for MutationMetrics {
    fn default() -> Self {
        Self {
            total_count: 0,
            inserts: 0,
            updates: 0,
            deletes: 0,
            upserts: 0,
            rows_affected: 0,
        }
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
            size: 0,
            capacity: 0,
            evictions: 0,
        }
    }
}

impl Default for StorageMetrics {
    fn default() -> Self {
        Self {
            entity_counts: Vec::new(),
            total_entities: 0,
            size_bytes: None,
            active_transactions: 0,
        }
    }
}

impl Default for TransportMetrics {
    fn default() -> Self {
        Self {
            requests_total: 0,
            requests_success: 0,
            requests_failed: 0,
            bytes_received: 0,
            bytes_sent: 0,
            active_connections: 0,
        }
    }
}

impl QueryMetrics {
    /// Create new query metrics.
    pub fn new(
        total_count: u64,
        avg_duration_us: u64,
        p50_duration_us: u64,
        p99_duration_us: u64,
        max_duration_us: u64,
    ) -> Self {
        Self {
            total_count,
            avg_duration_us,
            p50_duration_us,
            p99_duration_us,
            max_duration_us,
            by_entity: Vec::new(),
        }
    }

    /// Add entity query count.
    pub fn with_entity_count(mut self, entity: impl Into<String>, count: u64) -> Self {
        self.by_entity.push(EntityQueryCount {
            entity: entity.into(),
            count,
        });
        self
    }
}

impl CacheMetrics {
    /// Create new cache metrics.
    pub fn new(hits: u64, misses: u64, size: u64, capacity: u64, evictions: u64) -> Self {
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };
        Self {
            hits,
            misses,
            hit_rate,
            size,
            capacity,
            evictions,
        }
    }
}

impl MutationMetrics {
    /// Create new mutation metrics.
    pub fn new(inserts: u64, updates: u64, deletes: u64, upserts: u64, rows_affected: u64) -> Self {
        Self {
            total_count: inserts + updates + deletes + upserts,
            inserts,
            updates,
            deletes,
            upserts,
            rows_affected,
        }
    }
}

impl TransportMetrics {
    /// Create new transport metrics.
    pub fn new(
        requests_total: u64,
        requests_success: u64,
        requests_failed: u64,
        bytes_received: u64,
        bytes_sent: u64,
        active_connections: u64,
    ) -> Self {
        Self {
            requests_total,
            requests_success,
            requests_failed,
            bytes_received,
            bytes_sent,
            active_connections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_result_serialization() {
        let result = MetricsResult::new(
            3600,
            QueryMetrics::new(1000, 2500, 1500, 15000, 50000)
                .with_entity_count("User", 500)
                .with_entity_count("Post", 300),
            MutationMetrics::new(100, 50, 20, 10, 180),
            CacheMetrics::new(800, 200, 128, 1000, 50),
            StorageMetrics::default(),
            TransportMetrics::new(1200, 1150, 50, 1024 * 1024, 2 * 1024 * 1024, 5),
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
        let archived =
            rkyv::access::<ArchivedMetricsResult, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: MetricsResult =
            rkyv::deserialize::<MetricsResult, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_cache_hit_rate() {
        let cache = CacheMetrics::new(80, 20, 50, 100, 5);
        assert!((cache.hit_rate - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_mutation_total_count() {
        let mutations = MutationMetrics::new(10, 5, 3, 2, 20);
        assert_eq!(mutations.total_count, 20); // 10 + 5 + 3 + 2
    }
}
