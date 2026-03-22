//! Benchmark harness helpers.
//!
//! This module provides utilities for setting up and running benchmarks.

use ormdb_core::catalog::Catalog;
use ormdb_core::query::{encode_entity, QueryExecutor};
use ormdb_core::storage::{Record, StorageConfig, StorageEngine, VersionedKey};
use ormdb_proto::Value;

use crate::fixtures::{
    blog_schema, comment_to_fields, generate_comments, generate_posts, generate_users,
    post_to_fields, user_to_fields, Scale,
};

/// Test context for benchmarks.
///
/// Manages temporary storage and catalog for isolated benchmark runs.
pub struct TestContext {
    pub storage: StorageEngine,
    pub catalog: Catalog,
    _storage_dir: tempfile::TempDir,
    _catalog_db: sled::Db,
}

impl TestContext {
    /// Create a new empty test context.
    pub fn new() -> Self {
        let storage_dir = tempfile::tempdir().unwrap();
        let storage = StorageEngine::open(StorageConfig::new(storage_dir.path())).unwrap();
        let catalog_db = sled::Config::new().temporary(true).open().unwrap();
        let catalog = Catalog::open(&catalog_db).unwrap();

        Self {
            storage,
            catalog,
            _storage_dir: storage_dir,
            _catalog_db: catalog_db,
        }
    }

    /// Create a test context with blog schema.
    pub fn with_schema() -> Self {
        let ctx = Self::new();
        ctx.catalog.apply_schema(blog_schema()).unwrap();
        ctx
    }

    /// Create a test context with blog schema and populated data.
    pub fn with_scale(scale: Scale) -> Self {
        let ctx = Self::with_schema();
        populate_storage(&ctx, scale);
        ctx
    }

    /// Get a query executor for this context.
    pub fn executor(&self) -> QueryExecutor<'_> {
        QueryExecutor::new(&self.storage, &self.catalog)
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Insert an entity into storage.
pub fn insert_entity(
    storage: &StorageEngine,
    entity_type: &str,
    fields: Vec<(String, Value)>,
) -> [u8; 16] {
    // Extract ID from fields or generate one
    let id = fields
        .iter()
        .find(|(k, _)| k == "id")
        .and_then(|(_, v)| {
            if let Value::Uuid(id) = v {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap_or_else(StorageEngine::generate_id);

    let data = encode_entity(&fields).unwrap();
    let key = VersionedKey::now(id);
    storage
        .put_typed(entity_type, key, Record::new(data))
        .unwrap();
    id
}

/// Batch size for grouped inserts during population.
/// Larger batches reduce transaction commit overhead.
const POPULATION_BATCH_SIZE: usize = 10_000;

/// Populate storage with benchmark data at the specified scale.
///
/// Uses batch transactions for efficient insertion when dealing with large datasets.
pub fn populate_storage(ctx: &TestContext, scale: Scale) {
    let user_count = scale.count();
    let posts_per_user = scale.posts_per_user();
    let comments_per_post = scale.comments_per_post();

    let users = generate_users(user_count);
    let user_ids: Vec<_> = users.iter().map(|u| u.id).collect();

    // Insert users in batches
    for chunk in users.chunks(POPULATION_BATCH_SIZE) {
        let mut txn = ctx.storage.transaction();
        for user in chunk {
            let fields = user_to_fields(user);
            let data = encode_entity(&fields).unwrap();
            let key = VersionedKey::now(user.id);
            txn.put_typed("User", key, Record::new(data));
        }
        txn.commit().unwrap();
    }

    // Build hash index for "status" field using batch API (O(n) instead of O(n^2))
    let status_entries: Vec<(Value, [u8; 16])> = users
        .iter()
        .map(|u| (Value::String(u.status.clone()), u.id))
        .collect();
    ctx.storage
        .hash_index()
        .insert_batch("User", "status", status_entries)
        .unwrap();

    // Build B-tree index for "age" field
    if let Some(btree) = ctx.storage.btree_index() {
        for user in &users {
            btree
                .insert("User", "age", &Value::Int32(user.age), user.id)
                .unwrap();
        }
    }

    let post_count = user_count * posts_per_user;
    let posts = generate_posts(post_count, &user_ids);
    let post_ids: Vec<_> = posts.iter().map(|p| p.id).collect();

    // Insert posts in batches
    for chunk in posts.chunks(POPULATION_BATCH_SIZE) {
        let mut txn = ctx.storage.transaction();
        for post in chunk {
            let fields = post_to_fields(post);
            let data = encode_entity(&fields).unwrap();
            let key = VersionedKey::now(post.id);
            txn.put_typed("Post", key, Record::new(data));
        }
        txn.commit().unwrap();
    }

    let comment_count = post_count * comments_per_post;
    let comments = generate_comments(comment_count, &post_ids, &user_ids);

    // Insert comments in batches
    for chunk in comments.chunks(POPULATION_BATCH_SIZE) {
        let mut txn = ctx.storage.transaction();
        for comment in chunk {
            let fields = comment_to_fields(comment);
            let data = encode_entity(&fields).unwrap();
            let key = VersionedKey::now(comment.id);
            txn.put_typed("Comment", key, Record::new(data));
        }
        txn.commit().unwrap();
    }

    ctx.storage.flush().unwrap();
}

/// Setup storage with only raw key-value data (no schema).
pub fn setup_raw_storage() -> (StorageEngine, tempfile::TempDir) {
    let storage_dir = tempfile::tempdir().unwrap();
    let storage = StorageEngine::open(StorageConfig::new(storage_dir.path())).unwrap();
    (storage, storage_dir)
}

/// Benchmark helper: measure time for a single operation.
#[macro_export]
macro_rules! bench_op {
    ($b:expr, $setup:expr, $op:expr) => {{
        $b.iter_batched(
            || $setup,
            |state| $op(state),
            criterion::BatchSize::SmallInput,
        )
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = TestContext::new();
        assert!(ctx.catalog.list_entities().unwrap().is_empty());
    }

    #[test]
    fn test_context_with_schema() {
        let ctx = TestContext::with_schema();
        let entities = ctx.catalog.list_entities().unwrap();
        assert!(entities.contains(&"User".to_string()));
        assert!(entities.contains(&"Post".to_string()));
        assert!(entities.contains(&"Comment".to_string()));
    }

    #[test]
    fn test_populate_small() {
        let ctx = TestContext::with_scale(Scale::Small);
        let executor = ctx.executor();
        let query = ormdb_proto::GraphQuery::new("User");
        let result = executor.execute(&query).unwrap();
        assert_eq!(result.entities[0].len(), 100);
    }

    #[test]
    fn test_hash_index_built() {
        let ctx = TestContext::with_scale(Scale::Small);

        // Verify hash index was built for "status" column
        assert!(
            ctx.storage.hash_index().has_index("User", "status").unwrap(),
            "Hash index should exist for User.status"
        );

        // Verify we can lookup by status
        let active_ids = ctx
            .storage
            .hash_index()
            .lookup("User", "status", &Value::String("active".to_string()))
            .unwrap();

        // With 100 users and 4 status values, we should have ~25 active users
        assert!(
            !active_ids.is_empty(),
            "Should find some active users in hash index"
        );
        println!("Found {} active users via hash index", active_ids.len());
    }

    #[test]
    fn test_query_uses_hash_index() {
        use ormdb_proto::{FilterExpr, GraphQuery};

        let ctx = TestContext::with_scale(Scale::Small);

        // Verify hash index exists before query
        let has_idx = ctx.storage.hash_index().has_index("User", "status").unwrap();
        println!("has_index('User', 'status') = {}", has_idx);
        assert!(has_idx, "Hash index should exist for User.status");

        // Query using filter_eq on indexed status column
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into());

        // Execute
        let executor = ctx.executor();
        let result = executor.execute(&query).unwrap();

        // Should find ~25 active users (100 users / 4 statuses)
        let count = result.entities[0].len();
        println!("Query returned {} active users", count);
        assert_eq!(count, 25, "Should find 25 active users");

        // Verify all have status = "active"
        let status_col = result.entities[0].column("status").unwrap();
        for v in &status_col.values {
            assert_eq!(*v, Value::String("active".to_string()));
        }
    }

    #[test]
    fn test_hash_index_direct_lookup_performance() {
        use std::time::Instant;

        let ctx = TestContext::with_scale(Scale::Medium); // 10,000 users

        // Time the hash index lookup
        let start = Instant::now();
        let ids = ctx
            .storage
            .hash_index()
            .lookup("User", "status", &Value::String("active".to_string()))
            .unwrap();
        let hash_lookup_time = start.elapsed();

        println!(
            "Hash index lookup: {} ids in {:?}",
            ids.len(),
            hash_lookup_time
        );

        // Hash index lookup should be sub-millisecond
        assert!(
            hash_lookup_time.as_millis() < 10,
            "Hash index lookup should be fast (<10ms), got {:?}",
            hash_lookup_time
        );
    }

    #[test]
    fn test_full_query_with_hash_index_performance() {
        use ormdb_proto::{FilterExpr, GraphQuery};
        use std::time::Instant;

        let ctx = TestContext::with_scale(Scale::Small); // 100 users for faster setup

        // Warm up
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into());
        let _ = ctx.executor().execute(&query).unwrap();

        // Time 10 queries
        let iterations = 10;
        let start = Instant::now();
        for _ in 0..iterations {
            let result = ctx.executor().execute(&query).unwrap();
            assert_eq!(result.entities[0].len(), 25);
        }
        let total_time = start.elapsed();
        let avg_time = total_time / iterations;

        println!(
            "Full query (hash index + row fetch) [Small]: avg {:?} per query ({} iterations)",
            avg_time, iterations
        );

        // Full query should be much faster than 13ms (previous benchmark result)
        // Target: < 1ms with hash index
        assert!(
            avg_time.as_millis() < 5,
            "Full query should be fast (<5ms), got {:?}",
            avg_time
        );
    }

    #[test]
    fn test_full_query_with_hash_index_medium_scale() {
        use ormdb_proto::{FilterExpr, GraphQuery};
        use std::time::Instant;

        let ctx = TestContext::with_scale(Scale::Medium); // 10,000 users

        // Query
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String("active".to_string())).into());

        // Warm up
        let _ = ctx.executor().execute(&query).unwrap();

        // Time 5 queries
        let iterations = 5;
        let start = Instant::now();
        for _ in 0..iterations {
            let result = ctx.executor().execute(&query).unwrap();
            assert_eq!(result.entities[0].len(), 500); // 2,000 / 4 statuses
        }
        let total_time = start.elapsed();
        let avg_time = total_time / iterations;

        println!(
            "Full query (hash index + row fetch) [Medium 10K users, 2500 results]: avg {:?} per query",
            avg_time
        );

        // With hash index + row fetch, should be much faster than 13ms
        // Target: < 5ms (was 13.7ms before)
        assert!(
            avg_time.as_millis() < 10,
            "Full query should be faster than before (<10ms), got {:?}",
            avg_time
        );
    }

    #[test]
    fn test_btree_index_built() {
        let ctx = TestContext::with_scale(Scale::Small);

        // Check if B-tree index is available
        if let Some(btree) = ctx.storage.btree_index() {
            // Verify B-tree index was built for "age" column
            let has_idx = btree.has_index("User", "age").unwrap();
            assert!(has_idx, "B-tree index should exist for User.age");

            // Verify we can do a range scan (age > 50)
            let matching_ids = btree
                .scan_greater_than("User", "age", &Value::Int32(50))
                .unwrap();

            // With 100 users and ages 18-75, should have some users > 50
            assert!(
                !matching_ids.is_empty(),
                "Should find some users with age > 50 in B-tree index"
            );
            println!("Found {} users with age > 50 via B-tree index", matching_ids.len());
        } else {
            println!("B-tree index not available (expected in temporary storage)");
        }
    }

    #[test]
    fn test_query_uses_btree_index() {
        use ormdb_proto::{FilterExpr, GraphQuery};
        use std::time::Instant;

        let ctx = TestContext::with_scale(Scale::Small);

        // Skip if no B-tree index
        if ctx.storage.btree_index().is_none() {
            println!("Skipping B-tree test - index not available");
            return;
        }

        // Query using filter_gt on indexed age column
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::gt("age", Value::Int32(50)).into());

        // Warm up
        let _ = ctx.executor().execute(&query).unwrap();

        // Execute and verify
        let start = Instant::now();
        let result = ctx.executor().execute(&query).unwrap();
        let query_time = start.elapsed();

        let count = result.entities[0].len();
        println!("Query (age > 50) returned {} users in {:?}", count, query_time);

        // Verify all have age > 50
        let age_col = result.entities[0].column("age").unwrap();
        for v in &age_col.values {
            if let Value::Int32(age) = v {
                assert!(*age > 50, "Expected age > 50, got {}", age);
            }
        }
    }

    #[test]
    fn test_btree_index_range_query_performance() {
        use ormdb_proto::{FilterExpr, GraphQuery};
        use std::time::Instant;

        let ctx = TestContext::with_scale(Scale::Medium); // 10,000 users

        // Skip if no B-tree index
        if ctx.storage.btree_index().is_none() {
            println!("Skipping B-tree performance test - index not available");
            return;
        }

        // Query
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::gt("age", Value::Int32(50)).into());

        // Warm up
        let _ = ctx.executor().execute(&query).unwrap();

        // Time 5 queries
        let iterations = 5;
        let start = Instant::now();
        let mut count = 0;
        for _ in 0..iterations {
            let result = ctx.executor().execute(&query).unwrap();
            count = result.entities[0].len();
        }
        let total_time = start.elapsed();
        let avg_time = total_time / iterations;

        println!(
            "Range query (B-tree index) [Medium 10K users, {} results]: avg {:?} per query",
            count, avg_time
        );

        // With B-tree index, should be much faster than full scan
        // Target: < 10ms (was ~23ms with full scan)
        assert!(
            avg_time.as_millis() < 15,
            "Range query should be fast with B-tree index (<15ms), got {:?}",
            avg_time
        );
    }
}
