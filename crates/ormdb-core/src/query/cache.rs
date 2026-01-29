//! Plan cache for query optimization.
//!
//! This module caches compiled query plans to avoid repeated planning overhead
//! for queries with the same structure.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use ormdb_proto::{FilterExpr, GraphQuery, SimpleFilter};

use super::planner::QueryPlan;

/// Query fingerprint for cache lookup.
///
/// The fingerprint is computed from the query structure, not the literal values.
/// This means queries with the same shape but different filter values will share
/// the same plan (the plan structure doesn't depend on specific values).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct QueryFingerprint {
    hash: u64,
}

impl QueryFingerprint {
    /// Create a fingerprint from a GraphQuery.
    ///
    /// The fingerprint captures:
    /// - Root entity name
    /// - Field names (sorted for canonicalization)
    /// - Include paths (sorted)
    /// - Filter structure (operators and field names, not values)
    /// - Ordering specification
    pub fn from_query(query: &GraphQuery) -> Self {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();

        // Hash root entity
        query.root_entity.hash(&mut hasher);

        // Hash fields in sorted order for canonicalization
        let mut sorted_fields = query.fields.clone();
        sorted_fields.sort();
        sorted_fields.hash(&mut hasher);

        // Hash includes in sorted order by path
        let mut sorted_include_paths: Vec<&str> =
            query.includes.iter().map(|i| i.path.as_str()).collect();
        sorted_include_paths.sort();
        for path in sorted_include_paths {
            path.hash(&mut hasher);
        }

        // Hash filter structure (not values)
        if let Some(filter) = &query.filter {
            Self::hash_filter_structure(&filter.expression, &mut hasher);
        }

        // Hash ordering
        for order in &query.order_by {
            order.field.hash(&mut hasher);
            std::mem::discriminant(&order.direction).hash(&mut hasher);
        }

        // Hash pagination presence (not values)
        query.pagination.is_some().hash(&mut hasher);

        Self {
            hash: hasher.finish(),
        }
    }

    /// Hash the structure of a filter expression (not the values).
    fn hash_filter_structure<H: Hasher>(filter: &FilterExpr, hasher: &mut H) {
        // Hash the discriminant to identify the filter type
        std::mem::discriminant(filter).hash(hasher);

        match filter {
            FilterExpr::Eq { field, .. }
            | FilterExpr::Ne { field, .. }
            | FilterExpr::Lt { field, .. }
            | FilterExpr::Le { field, .. }
            | FilterExpr::Gt { field, .. }
            | FilterExpr::Ge { field, .. }
            | FilterExpr::IsNull { field }
            | FilterExpr::IsNotNull { field }
            | FilterExpr::Like { field, .. }
            | FilterExpr::NotLike { field, .. } => {
                field.hash(hasher);
            }
            FilterExpr::In { field, values }
            | FilterExpr::NotIn { field, values } => {
                field.hash(hasher);
                // Hash cardinality, not actual values
                values.len().hash(hasher);
            }
            FilterExpr::And(filters) | FilterExpr::Or(filters) => {
                filters.len().hash(hasher);
                for f in filters {
                    Self::hash_simple_filter_structure(f, hasher);
                }
            }
        }
    }

    /// Hash the structure of a SimpleFilter.
    fn hash_simple_filter_structure<H: Hasher>(filter: &SimpleFilter, hasher: &mut H) {
        std::mem::discriminant(filter).hash(hasher);

        match filter {
            SimpleFilter::Eq { field, .. }
            | SimpleFilter::Ne { field, .. }
            | SimpleFilter::Lt { field, .. }
            | SimpleFilter::Le { field, .. }
            | SimpleFilter::Gt { field, .. }
            | SimpleFilter::Ge { field, .. }
            | SimpleFilter::IsNull { field }
            | SimpleFilter::IsNotNull { field }
            | SimpleFilter::Like { field, .. }
            | SimpleFilter::NotLike { field, .. } => {
                field.hash(hasher);
            }
            SimpleFilter::In { field, values }
            | SimpleFilter::NotIn { field, values } => {
                field.hash(hasher);
                values.len().hash(hasher);
            }
        }
    }
}

/// Cached query plan with metadata.
#[derive(Debug)]
pub struct CachedPlan {
    /// The compiled query plan.
    pub plan: QueryPlan,
    /// Schema version when this plan was created.
    pub schema_version: u64,
    /// When this plan was created (microseconds since Unix epoch).
    pub created_at: u64,
    /// Number of cache hits for this plan.
    pub hit_count: AtomicU64,
}

impl CachedPlan {
    /// Create a new cached plan.
    pub fn new(plan: QueryPlan, schema_version: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        Self {
            plan,
            schema_version,
            created_at: now,
            hit_count: AtomicU64::new(0),
        }
    }

    /// Increment the hit count and return the new value.
    pub fn record_hit(&self) -> u64 {
        self.hit_count.fetch_add(1, AtomicOrdering::Relaxed) + 1
    }

    /// Get the current hit count.
    pub fn hits(&self) -> u64 {
        self.hit_count.load(AtomicOrdering::Relaxed)
    }
}

impl Clone for CachedPlan {
    fn clone(&self) -> Self {
        Self {
            plan: self.plan.clone(),
            schema_version: self.schema_version,
            created_at: self.created_at,
            hit_count: AtomicU64::new(self.hit_count.load(AtomicOrdering::Relaxed)),
        }
    }
}

/// Plan cache with LRU eviction.
///
/// Thread-safe cache for compiled query plans, keyed by query fingerprint.
/// Plans are invalidated when the schema version changes.
pub struct PlanCache {
    /// The cache storage.
    cache: RwLock<HashMap<QueryFingerprint, CachedPlan>>,
    /// Maximum number of entries.
    max_entries: usize,
    /// Current schema version (for invalidation).
    current_schema_version: AtomicU64,
    /// Cache statistics.
    stats: CacheStats,
}

/// Cache statistics.
#[derive(Debug, Default)]
pub struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl CacheStats {
    /// Get hit count.
    pub fn hits(&self) -> u64 {
        self.hits.load(AtomicOrdering::Relaxed)
    }

    /// Get miss count.
    pub fn misses(&self) -> u64 {
        self.misses.load(AtomicOrdering::Relaxed)
    }

    /// Get eviction count.
    pub fn evictions(&self) -> u64 {
        self.evictions.load(AtomicOrdering::Relaxed)
    }

    /// Calculate hit rate (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits() as f64;
        let total = hits + self.misses() as f64;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }
}

impl PlanCache {
    /// Create a new plan cache with the specified maximum size.
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_entries,
            current_schema_version: AtomicU64::new(0),
            stats: CacheStats::default(),
        }
    }

    /// Get a cached plan if valid.
    ///
    /// Returns None if:
    /// - No plan is cached for this fingerprint
    /// - The cached plan's schema version doesn't match current
    pub fn get(&self, fingerprint: &QueryFingerprint) -> Option<QueryPlan> {
        let current_version = self.current_schema_version.load(AtomicOrdering::SeqCst);

        let guard = self.cache.read().ok()?;

        if let Some(cached) = guard.get(fingerprint) {
            if cached.schema_version == current_version {
                cached.record_hit();
                self.stats.hits.fetch_add(1, AtomicOrdering::Relaxed);
                return Some(cached.plan.clone());
            }
        }

        self.stats.misses.fetch_add(1, AtomicOrdering::Relaxed);
        None
    }

    /// Insert a plan into the cache.
    ///
    /// If the cache is full, evicts the least-used entry.
    pub fn insert(&self, fingerprint: QueryFingerprint, plan: QueryPlan, schema_version: u64) {
        // Update schema version if newer
        let current = self.current_schema_version.load(AtomicOrdering::SeqCst);
        if schema_version > current {
            self.current_schema_version
                .store(schema_version, AtomicOrdering::SeqCst);
        }

        let cached = CachedPlan::new(plan, schema_version);

        let mut guard = match self.cache.write() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Evict if at capacity
        if guard.len() >= self.max_entries && !guard.contains_key(&fingerprint) {
            self.evict_lru(&mut guard);
        }

        guard.insert(fingerprint, cached);
    }

    /// Invalidate all plans (on schema change).
    pub fn invalidate(&self, new_schema_version: u64) {
        self.current_schema_version
            .store(new_schema_version, AtomicOrdering::SeqCst);

        // Clear the cache
        if let Ok(mut guard) = self.cache.write() {
            guard.clear();
        }
    }

    /// Evict the least recently used entry.
    fn evict_lru(&self, cache: &mut HashMap<QueryFingerprint, CachedPlan>) {
        // Find entry with lowest hit count
        let evict_key = cache
            .iter()
            .min_by_key(|(_, v)| v.hits())
            .map(|(k, _)| k.clone());

        if let Some(key) = evict_key {
            cache.remove(&key);
            self.stats.evictions.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get the current number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.cache.write() {
            guard.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{EntityDef, FieldDef, FieldType, ScalarType};
    use crate::query::planner::FanoutBudget;
    use ormdb_proto::{RelationInclude, Value};

    fn create_test_plan(entity: &str) -> QueryPlan {
        QueryPlan {
            root_entity: entity.to_string(),
            root_entity_def: EntityDef::new(entity, "id")
                .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid))),
            fields: vec![],
            filter: None,
            order_by: vec![],
            pagination: None,
            includes: vec![],
            budget: FanoutBudget::default(),
        }
    }

    #[test]
    fn test_fingerprint_same_query() {
        let query = GraphQuery::new("User").with_fields(vec!["id".into(), "name".into()]);

        let fp1 = QueryFingerprint::from_query(&query);
        let fp2 = QueryFingerprint::from_query(&query);

        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_different_entity() {
        let query1 = GraphQuery::new("User");
        let query2 = GraphQuery::new("Post");

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_field_order_normalized() {
        // Fields in different order should produce same fingerprint
        let query1 = GraphQuery::new("User").with_fields(vec!["id".into(), "name".into()]);
        let query2 = GraphQuery::new("User").with_fields(vec!["name".into(), "id".into()]);

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_different_filter_values_same_structure() {
        // Same filter structure with different values should produce same fingerprint
        let query1 = GraphQuery::new("User").with_filter(
            FilterExpr::eq("name", Value::String("Alice".to_string())).into(),
        );
        let query2 = GraphQuery::new("User").with_filter(
            FilterExpr::eq("name", Value::String("Bob".to_string())).into(),
        );

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_different_filter_field() {
        let query1 = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("name", Value::String("Alice".to_string())).into());
        let query2 = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("email", Value::String("alice@test.com".to_string())).into());

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_different_filter_operator() {
        let query1 = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("age", Value::Int32(30)).into());
        let query2 = GraphQuery::new("User")
            .with_filter(FilterExpr::gt("age", Value::Int32(30)).into());

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = PlanCache::new(100);
        let query = GraphQuery::new("User");
        let fingerprint = QueryFingerprint::from_query(&query);
        let plan = create_test_plan("User");

        cache.insert(fingerprint.clone(), plan.clone(), 1);

        // Update schema version to match
        cache.current_schema_version.store(1, AtomicOrdering::SeqCst);

        let cached = cache.get(&fingerprint);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().root_entity, "User");
    }

    #[test]
    fn test_cache_miss() {
        let cache = PlanCache::new(100);
        let query = GraphQuery::new("User");
        let fingerprint = QueryFingerprint::from_query(&query);

        let cached = cache.get(&fingerprint);
        assert!(cached.is_none());
        assert_eq!(cache.stats().misses(), 1);
    }

    #[test]
    fn test_cache_schema_invalidation() {
        let cache = PlanCache::new(100);
        let query = GraphQuery::new("User");
        let fingerprint = QueryFingerprint::from_query(&query);
        let plan = create_test_plan("User");

        cache.insert(fingerprint.clone(), plan, 1);
        cache.current_schema_version.store(1, AtomicOrdering::SeqCst);

        // Should get the plan
        assert!(cache.get(&fingerprint).is_some());

        // Invalidate with new schema version
        cache.invalidate(2);

        // Should miss now
        assert!(cache.get(&fingerprint).is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = PlanCache::new(2);

        let query1 = GraphQuery::new("User");
        let query2 = GraphQuery::new("Post");
        let query3 = GraphQuery::new("Comment");

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);
        let fp3 = QueryFingerprint::from_query(&query3);

        cache.insert(fp1.clone(), create_test_plan("User"), 1);
        cache.insert(fp2.clone(), create_test_plan("Post"), 1);
        cache.current_schema_version.store(1, AtomicOrdering::SeqCst);

        // Access fp2 to increase its hit count
        cache.get(&fp2);
        cache.get(&fp2);

        // Insert third, should evict fp1 (lowest hit count)
        cache.insert(fp3.clone(), create_test_plan("Comment"), 1);

        assert_eq!(cache.len(), 2);
        assert!(cache.get(&fp2).is_some());
        assert!(cache.get(&fp3).is_some());
        // fp1 should be evicted
        assert_eq!(cache.stats().evictions(), 1);
    }

    #[test]
    fn test_cache_stats() {
        let cache = PlanCache::new(100);
        let query = GraphQuery::new("User");
        let fingerprint = QueryFingerprint::from_query(&query);
        let plan = create_test_plan("User");

        // Miss
        cache.get(&fingerprint);
        assert_eq!(cache.stats().misses(), 1);
        assert_eq!(cache.stats().hits(), 0);

        // Insert and hit
        cache.insert(fingerprint.clone(), plan, 1);
        cache.current_schema_version.store(1, AtomicOrdering::SeqCst);
        cache.get(&fingerprint);
        cache.get(&fingerprint);

        assert_eq!(cache.stats().hits(), 2);
        assert_eq!(cache.stats().misses(), 1);

        // Hit rate = 2 / (2 + 1) = 0.666...
        assert!((cache.stats().hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_cached_plan_hit_tracking() {
        let plan = create_test_plan("User");
        let cached = CachedPlan::new(plan, 1);

        assert_eq!(cached.hits(), 0);
        assert_eq!(cached.record_hit(), 1);
        assert_eq!(cached.record_hit(), 2);
        assert_eq!(cached.hits(), 2);
    }

    #[test]
    fn test_fingerprint_with_includes() {
        let query1 = GraphQuery::new("User")
            .include(RelationInclude::new("posts"));
        let query2 = GraphQuery::new("User")
            .include(RelationInclude::new("comments"));

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_include_order_normalized() {
        let query1 = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .include(RelationInclude::new("comments"));
        let query2 = GraphQuery::new("User")
            .include(RelationInclude::new("comments"))
            .include(RelationInclude::new("posts"));

        let fp1 = QueryFingerprint::from_query(&query1);
        let fp2 = QueryFingerprint::from_query(&query2);

        assert_eq!(fp1, fp2);
    }
}
