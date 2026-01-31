//! Central metrics registry.
//!
//! This module provides a centralized registry for collecting and exporting
//! server metrics including query statistics, cache metrics, and mutation counts.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::histogram::Histogram;

/// Central registry for all metrics.
///
/// This struct collects metrics from all parts of the system and provides
/// methods to query current values and export to Prometheus format.
pub struct MetricsRegistry {
    /// Server start time.
    started_at: Instant,

    // Query metrics
    query_count: AtomicU64,
    query_latency: Histogram,
    queries_by_entity: RwLock<HashMap<String, AtomicU64>>,

    // Mutation metrics
    mutation_count: AtomicU64,
    insert_count: AtomicU64,
    update_count: AtomicU64,
    delete_count: AtomicU64,
    upsert_count: AtomicU64,
    rows_affected: AtomicU64,

    // Cache metrics
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    cache_evictions: AtomicU64,

    // Fanout metrics
    fanout_total: AtomicU64,
    fanout_budget_exceeded: AtomicU64,
}

/// Mutation type for metrics tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationType {
    /// Insert operation.
    Insert,
    /// Update operation.
    Update,
    /// Delete operation.
    Delete,
    /// Upsert operation.
    Upsert,
}

impl MetricsRegistry {
    /// Create a new metrics registry.
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            query_count: AtomicU64::new(0),
            query_latency: Histogram::latency(),
            queries_by_entity: RwLock::new(HashMap::new()),
            mutation_count: AtomicU64::new(0),
            insert_count: AtomicU64::new(0),
            update_count: AtomicU64::new(0),
            delete_count: AtomicU64::new(0),
            upsert_count: AtomicU64::new(0),
            rows_affected: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            fanout_total: AtomicU64::new(0),
            fanout_budget_exceeded: AtomicU64::new(0),
        }
    }

    /// Record a query execution.
    pub fn record_query(&self, entity: &str, duration_us: u64) {
        self.query_count.fetch_add(1, Ordering::Relaxed);
        self.query_latency.observe(duration_us);

        if let Ok(mut map) = self.queries_by_entity.write() {
            map.entry(entity.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record a mutation.
    pub fn record_mutation(&self, op_type: MutationType, affected_rows: u64) {
        self.mutation_count.fetch_add(1, Ordering::Relaxed);
        self.rows_affected.fetch_add(affected_rows, Ordering::Relaxed);

        match op_type {
            MutationType::Insert => {
                self.insert_count.fetch_add(1, Ordering::Relaxed);
            }
            MutationType::Update => {
                self.update_count.fetch_add(1, Ordering::Relaxed);
            }
            MutationType::Delete => {
                self.delete_count.fetch_add(1, Ordering::Relaxed);
            }
            MutationType::Upsert => {
                self.upsert_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss.
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache eviction.
    pub fn record_cache_eviction(&self) {
        self.cache_evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record fanout usage.
    pub fn record_fanout(&self, entities: u64) {
        self.fanout_total.fetch_add(entities, Ordering::Relaxed);
    }

    /// Record a fanout budget exceeded error.
    pub fn record_budget_exceeded(&self) {
        self.fanout_budget_exceeded.fetch_add(1, Ordering::Relaxed);
    }

    // Getters

    /// Get uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Get total query count.
    pub fn query_count(&self) -> u64 {
        self.query_count.load(Ordering::Relaxed)
    }

    /// Get average query latency in microseconds.
    pub fn avg_query_latency_us(&self) -> u64 {
        self.query_latency.avg()
    }

    /// Get P50 query latency in microseconds.
    pub fn p50_query_latency_us(&self) -> u64 {
        self.query_latency.p50()
    }

    /// Get P99 query latency in microseconds.
    pub fn p99_query_latency_us(&self) -> u64 {
        self.query_latency.p99()
    }

    /// Get max query latency in microseconds.
    pub fn max_query_latency_us(&self) -> u64 {
        self.query_latency.max()
    }

    /// Get query counts by entity.
    pub fn queries_by_entity(&self) -> HashMap<String, u64> {
        if let Ok(map) = self.queries_by_entity.read() {
            map.iter()
                .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Get total mutation count.
    pub fn mutation_count(&self) -> u64 {
        self.mutation_count.load(Ordering::Relaxed)
    }

    /// Get insert count.
    pub fn insert_count(&self) -> u64 {
        self.insert_count.load(Ordering::Relaxed)
    }

    /// Get update count.
    pub fn update_count(&self) -> u64 {
        self.update_count.load(Ordering::Relaxed)
    }

    /// Get delete count.
    pub fn delete_count(&self) -> u64 {
        self.delete_count.load(Ordering::Relaxed)
    }

    /// Get upsert count.
    pub fn upsert_count(&self) -> u64 {
        self.upsert_count.load(Ordering::Relaxed)
    }

    /// Get total rows affected by mutations.
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected.load(Ordering::Relaxed)
    }

    /// Get cache hits.
    pub fn cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }

    /// Get cache misses.
    pub fn cache_misses(&self) -> u64 {
        self.cache_misses.load(Ordering::Relaxed)
    }

    /// Get cache evictions.
    pub fn cache_evictions(&self) -> u64 {
        self.cache_evictions.load(Ordering::Relaxed)
    }

    /// Get cache hit rate (0.0 - 1.0).
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.cache_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }

    /// Get total fanout.
    pub fn fanout_total(&self) -> u64 {
        self.fanout_total.load(Ordering::Relaxed)
    }

    /// Get budget exceeded count.
    pub fn budget_exceeded_count(&self) -> u64 {
        self.fanout_budget_exceeded.load(Ordering::Relaxed)
    }

    /// Export to Prometheus text format.
    pub fn to_prometheus(&self) -> String {
        let mut out = String::new();

        // Uptime
        out.push_str("# HELP ormdb_uptime_seconds Server uptime in seconds\n");
        out.push_str("# TYPE ormdb_uptime_seconds gauge\n");
        out.push_str(&format!("ormdb_uptime_seconds {}\n\n", self.uptime_secs()));

        // Query metrics
        out.push_str("# HELP ormdb_queries_total Total queries executed\n");
        out.push_str("# TYPE ormdb_queries_total counter\n");
        out.push_str(&format!("ormdb_queries_total {}\n\n", self.query_count()));

        out.push_str("# HELP ormdb_query_duration_us_avg Average query duration in microseconds\n");
        out.push_str("# TYPE ormdb_query_duration_us_avg gauge\n");
        out.push_str(&format!(
            "ormdb_query_duration_us_avg {}\n\n",
            self.avg_query_latency_us()
        ));

        out.push_str("# HELP ormdb_query_duration_us_p50 P50 query duration in microseconds\n");
        out.push_str("# TYPE ormdb_query_duration_us_p50 gauge\n");
        out.push_str(&format!(
            "ormdb_query_duration_us_p50 {}\n\n",
            self.p50_query_latency_us()
        ));

        out.push_str("# HELP ormdb_query_duration_us_p99 P99 query duration in microseconds\n");
        out.push_str("# TYPE ormdb_query_duration_us_p99 gauge\n");
        out.push_str(&format!(
            "ormdb_query_duration_us_p99 {}\n\n",
            self.p99_query_latency_us()
        ));

        // Mutation metrics
        out.push_str("# HELP ormdb_mutations_total Total mutations executed\n");
        out.push_str("# TYPE ormdb_mutations_total counter\n");
        out.push_str(&format!("ormdb_mutations_total {}\n\n", self.mutation_count()));

        out.push_str("# HELP ormdb_inserts_total Total insert operations\n");
        out.push_str("# TYPE ormdb_inserts_total counter\n");
        out.push_str(&format!("ormdb_inserts_total {}\n\n", self.insert_count()));

        out.push_str("# HELP ormdb_updates_total Total update operations\n");
        out.push_str("# TYPE ormdb_updates_total counter\n");
        out.push_str(&format!("ormdb_updates_total {}\n\n", self.update_count()));

        out.push_str("# HELP ormdb_deletes_total Total delete operations\n");
        out.push_str("# TYPE ormdb_deletes_total counter\n");
        out.push_str(&format!("ormdb_deletes_total {}\n\n", self.delete_count()));

        // Cache metrics
        out.push_str("# HELP ormdb_cache_hits_total Plan cache hits\n");
        out.push_str("# TYPE ormdb_cache_hits_total counter\n");
        out.push_str(&format!("ormdb_cache_hits_total {}\n\n", self.cache_hits()));

        out.push_str("# HELP ormdb_cache_misses_total Plan cache misses\n");
        out.push_str("# TYPE ormdb_cache_misses_total counter\n");
        out.push_str(&format!("ormdb_cache_misses_total {}\n\n", self.cache_misses()));

        out.push_str("# HELP ormdb_cache_hit_rate Plan cache hit rate\n");
        out.push_str("# TYPE ormdb_cache_hit_rate gauge\n");
        out.push_str(&format!("ormdb_cache_hit_rate {:.4}\n\n", self.cache_hit_rate()));

        // Fanout metrics
        out.push_str("# HELP ormdb_budget_exceeded_total Fanout budget exceeded count\n");
        out.push_str("# TYPE ormdb_budget_exceeded_total counter\n");
        out.push_str(&format!(
            "ormdb_budget_exceeded_total {}\n",
            self.budget_exceeded_count()
        ));

        out
    }

    /// Reset all metrics (for testing).
    pub fn reset(&self) {
        self.query_latency.reset();
        self.query_count.store(0, Ordering::Relaxed);
        if let Ok(mut map) = self.queries_by_entity.write() {
            map.clear();
        }
        self.mutation_count.store(0, Ordering::Relaxed);
        self.insert_count.store(0, Ordering::Relaxed);
        self.update_count.store(0, Ordering::Relaxed);
        self.delete_count.store(0, Ordering::Relaxed);
        self.upsert_count.store(0, Ordering::Relaxed);
        self.rows_affected.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.cache_evictions.store(0, Ordering::Relaxed);
        self.fanout_total.store(0, Ordering::Relaxed);
        self.fanout_budget_exceeded.store(0, Ordering::Relaxed);
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared metrics registry handle.
pub type SharedMetricsRegistry = Arc<MetricsRegistry>;

/// Create a new shared metrics registry.
pub fn new_shared_registry() -> SharedMetricsRegistry {
    Arc::new(MetricsRegistry::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_metrics() {
        let registry = MetricsRegistry::new();

        registry.record_query("User", 1000);
        registry.record_query("User", 2000);
        registry.record_query("Post", 500);

        assert_eq!(registry.query_count(), 3);
        assert_eq!(registry.avg_query_latency_us(), 1166); // (1000+2000+500)/3

        let by_entity = registry.queries_by_entity();
        assert_eq!(by_entity.get("User"), Some(&2));
        assert_eq!(by_entity.get("Post"), Some(&1));
    }

    #[test]
    fn test_mutation_metrics() {
        let registry = MetricsRegistry::new();

        registry.record_mutation(MutationType::Insert, 1);
        registry.record_mutation(MutationType::Insert, 5);
        registry.record_mutation(MutationType::Update, 3);
        registry.record_mutation(MutationType::Delete, 2);

        assert_eq!(registry.mutation_count(), 4);
        assert_eq!(registry.insert_count(), 2);
        assert_eq!(registry.update_count(), 1);
        assert_eq!(registry.delete_count(), 1);
        assert_eq!(registry.rows_affected(), 11);
    }

    #[test]
    fn test_cache_metrics() {
        let registry = MetricsRegistry::new();

        registry.record_cache_hit();
        registry.record_cache_hit();
        registry.record_cache_hit();
        registry.record_cache_miss();

        assert_eq!(registry.cache_hits(), 3);
        assert_eq!(registry.cache_misses(), 1);
        assert!((registry.cache_hit_rate() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_prometheus_format() {
        let registry = MetricsRegistry::new();
        registry.record_query("User", 1000);
        registry.record_cache_hit();

        let prometheus = registry.to_prometheus();

        assert!(prometheus.contains("ormdb_queries_total 1"));
        assert!(prometheus.contains("ormdb_cache_hits_total 1"));
        assert!(prometheus.contains("# TYPE ormdb_queries_total counter"));
    }

    #[test]
    fn test_reset() {
        let registry = MetricsRegistry::new();
        registry.record_query("User", 1000);
        registry.record_mutation(MutationType::Insert, 1);
        registry.record_cache_hit();

        assert_eq!(registry.query_count(), 1);

        registry.reset();

        assert_eq!(registry.query_count(), 0);
        assert_eq!(registry.mutation_count(), 0);
        assert_eq!(registry.cache_hits(), 0);
    }

    #[test]
    fn test_shared_registry() {
        let registry = new_shared_registry();

        registry.record_query("User", 100);
        assert_eq!(registry.query_count(), 1);

        let registry2 = Arc::clone(&registry);
        registry2.record_query("User", 200);
        assert_eq!(registry.query_count(), 2);
    }
}
