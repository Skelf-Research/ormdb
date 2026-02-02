//! Join strategies for resolving relation includes.
//!
//! This module provides different join algorithms for fetching related entities:
//! - NestedLoop: Simple O(N*M) approach for small datasets
//! - HashJoin: Efficient O(N+M) approach using hash table lookups

use std::collections::{HashMap, HashSet};

use crate::error::Error;
use crate::storage::StorageEngine;

use super::filter::{extract_filter_fields, FilterEvaluator};
use super::planner::IncludePlan;
use super::value_codec::{decode_entity, decode_entity_projected};

use ormdb_proto::{Edge, Value};

/// Internal representation of an entity row during execution.
#[derive(Debug, Clone)]
pub struct EntityRow {
    pub id: [u8; 16],
    pub fields: Vec<(String, Value)>,
}

/// Join strategy selection for relation resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinStrategy {
    /// Nested loop join - O(N*M), best for small datasets.
    NestedLoop,
    /// Hash join - O(N+M), best for larger datasets.
    HashJoin,
}

impl JoinStrategy {
    /// Select the optimal join strategy based on cardinality estimates.
    ///
    /// Uses hash join when:
    /// - More than 100 parent entities, OR
    /// - Estimated child count exceeds 1000
    ///
    /// Otherwise uses nested loop for lower overhead on small sets.
    pub fn select(parent_count: usize, estimated_child_count: u64) -> Self {
        if parent_count > 100 || estimated_child_count > 1000 {
            JoinStrategy::HashJoin
        } else {
            JoinStrategy::NestedLoop
        }
    }
}

/// Hash join executor for efficient relation resolution.
///
/// Algorithm:
/// 1. Build phase: Scan child entities once, build HashMap<FK_value, Vec<EntityRow>>
/// 2. Probe phase: For each parent ID, lookup matching children in O(1)
///
/// Complexity: O(N + M) where N = parent count, M = child entity count
pub struct HashJoinExecutor;

impl HashJoinExecutor {
    /// Execute a hash join for the given include.
    ///
    /// # Arguments
    /// * `storage` - Storage engine for scanning entities
    /// * `source_ids` - Parent entity IDs to match against
    /// * `include` - Include plan with relation and filter info
    ///
    /// # Returns
    /// Tuple of (matched entity rows, edges connecting parents to children)
    pub fn execute(
        storage: &StorageEngine,
        source_ids: &[[u8; 16]],
        include: &IncludePlan,
    ) -> Result<(Vec<EntityRow>, Vec<Edge>), Error> {
        let relation = &include.relation;
        let target_entity = &relation.to_entity;
        let fk_field = &relation.to_field;
        let needed_fields = collect_needed_fields(include, fk_field);

        // Build phase: scan child entities and build FK -> entities map
        let mut fk_to_entities: HashMap<[u8; 16], Vec<EntityRow>> = HashMap::new();

        for result in storage.scan_entity_type(target_entity) {
            let (entity_id, _version_ts, record) = result?;

            // Decode the entity data
            let fields = match needed_fields.as_ref() {
                Some(fields) => decode_entity_projected(&record.data, fields)?,
                None => decode_entity(&record.data)?,
            };

            // Get foreign key value
            let fk_value = fields
                .iter()
                .find(|(n, _)| n == fk_field)
                .map(|(_, v)| v);

            // Only process if FK is a valid UUID
            let fk_id = match fk_value {
                Some(Value::Uuid(id)) => *id,
                _ => continue,
            };

            // Apply filter early if present (filter pushdown)
            if let Some(filter) = &include.filter {
                if !FilterEvaluator::evaluate(filter, &fields)? {
                    continue;
                }
            }

            // Add to hash map
            fk_to_entities
                .entry(fk_id)
                .or_default()
                .push(EntityRow { id: entity_id, fields });
        }

        // Probe phase: lookup each parent ID
        let mut rows = Vec::new();
        let mut edges = Vec::new();

        for &parent_id in source_ids {
            if let Some(children) = fk_to_entities.remove(&parent_id) {
                for child in children {
                    let child_id = child.id;
                    rows.push(child);
                    edges.push(Edge::new(parent_id, child_id));
                }
            }
        }

        Ok((rows, edges))
    }
}

/// Nested loop join executor for small datasets.
///
/// Algorithm:
/// For each child entity, check if its FK matches any parent ID.
///
/// Complexity: O(M) scan with O(1) HashSet lookup per entity
/// (equivalent to current implementation)
pub struct NestedLoopExecutor;

impl NestedLoopExecutor {
    /// Execute a nested loop join for the given include.
    ///
    /// This is the original implementation, kept for small datasets
    /// where hash table construction overhead isn't worthwhile.
    pub fn execute(
        storage: &StorageEngine,
        source_ids: &[[u8; 16]],
        include: &IncludePlan,
    ) -> Result<(Vec<EntityRow>, Vec<Edge>), Error> {
        let relation = &include.relation;
        let target_entity = &relation.to_entity;
        let fk_field = &relation.to_field;
        let needed_fields = collect_needed_fields(include, fk_field);

        // Create set for O(1) lookup
        let source_id_set: HashSet<[u8; 16]> = source_ids.iter().cloned().collect();

        let mut rows = Vec::new();
        let mut edges = Vec::new();

        // Scan target entities
        for result in storage.scan_entity_type(target_entity) {
            let (entity_id, _version_ts, record) = result?;

            // Decode the entity data
            let fields = match needed_fields.as_ref() {
                Some(fields) => decode_entity_projected(&record.data, fields)?,
                None => decode_entity(&record.data)?,
            };

            // Get the foreign key value
            let fk_value = fields
                .iter()
                .find(|(n, _)| n == fk_field)
                .map(|(_, v)| v);

            // Check if this entity relates to any of our source entities
            let related_source_id = match fk_value {
                Some(Value::Uuid(fk_id)) => {
                    if source_id_set.contains(fk_id) {
                        Some(*fk_id)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(from_id) = related_source_id {
                // Apply include filter if present
                if let Some(filter) = &include.filter {
                    if !FilterEvaluator::evaluate(filter, &fields)? {
                        continue;
                    }
                }

                rows.push(EntityRow { id: entity_id, fields });
                edges.push(Edge::new(from_id, entity_id));
            }
        }

        Ok((rows, edges))
    }
}

fn collect_needed_fields(
    include: &IncludePlan,
    fk_field: &str,
) -> Option<HashSet<String>> {
    if include.fields.is_empty() {
        return None;
    }

    let mut fields: HashSet<String> = include.fields.iter().cloned().collect();
    fields.insert(fk_field.to_string());

    if let Some(filter) = &include.filter {
        fields.extend(extract_filter_fields(filter));
    }

    for order in &include.order_by {
        fields.insert(order.field.clone());
    }

    Some(fields)
}

/// Execute a join using the specified strategy.
pub fn execute_join(
    strategy: JoinStrategy,
    storage: &StorageEngine,
    source_ids: &[[u8; 16]],
    include: &IncludePlan,
) -> Result<(Vec<EntityRow>, Vec<Edge>), Error> {
    match strategy {
        JoinStrategy::NestedLoop => NestedLoopExecutor::execute(storage, source_ids, include),
        JoinStrategy::HashJoin => HashJoinExecutor::execute(storage, source_ids, include),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{EntityDef, FieldDef, FieldType, RelationDef, ScalarType};
    use crate::storage::{Record, StorageConfig, VersionedKey};
    use super::super::value_codec::encode_entity;

    fn setup_test_storage() -> (StorageEngine, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = StorageEngine::open(StorageConfig::new(dir.path())).unwrap();
        (storage, dir)
    }

    fn create_test_include() -> IncludePlan {
        let relation = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");
        let target_entity_def = EntityDef::new("Post", "id")
            .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
            .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
            .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid)));

        IncludePlan {
            path: "posts".to_string(),
            relation,
            target_entity_def,
            fields: vec![],
            filter: None,
            order_by: vec![],
            pagination: None,
        }
    }

    fn insert_post(storage: &StorageEngine, id: [u8; 16], title: &str, author_id: [u8; 16]) {
        let fields = vec![
            ("id".to_string(), Value::Uuid(id)),
            ("title".to_string(), Value::String(title.to_string())),
            ("author_id".to_string(), Value::Uuid(author_id)),
        ];
        let data = encode_entity(&fields).unwrap();
        let key = VersionedKey::now(id);
        storage.put_typed("Post", key, Record::new(data)).unwrap();
    }

    #[test]
    fn test_hash_join_basic() {
        let (storage, _dir) = setup_test_storage();
        let include = create_test_include();

        // Create two users
        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();

        // Create posts for user1
        let post1_id = StorageEngine::generate_id();
        let post2_id = StorageEngine::generate_id();
        insert_post(&storage, post1_id, "Post 1", user1_id);
        insert_post(&storage, post2_id, "Post 2", user1_id);

        // Create post for user2
        let post3_id = StorageEngine::generate_id();
        insert_post(&storage, post3_id, "Post 3", user2_id);

        storage.flush().unwrap();

        // Query only user1's posts
        let source_ids = vec![user1_id];
        let (rows, edges) = HashJoinExecutor::execute(&storage, &source_ids, &include).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(edges.len(), 2);

        // All edges should point from user1
        for edge in &edges {
            assert_eq!(edge.from_id, user1_id);
        }
    }

    #[test]
    fn test_hash_join_multiple_parents() {
        let (storage, _dir) = setup_test_storage();
        let include = create_test_include();

        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();
        let user3_id = StorageEngine::generate_id(); // No posts

        // Create posts
        insert_post(&storage, StorageEngine::generate_id(), "U1 Post 1", user1_id);
        insert_post(&storage, StorageEngine::generate_id(), "U1 Post 2", user1_id);
        insert_post(&storage, StorageEngine::generate_id(), "U2 Post 1", user2_id);

        storage.flush().unwrap();

        // Query all three users
        let source_ids = vec![user1_id, user2_id, user3_id];
        let (rows, edges) = HashJoinExecutor::execute(&storage, &source_ids, &include).unwrap();

        assert_eq!(rows.len(), 3); // 2 for user1, 1 for user2, 0 for user3
        assert_eq!(edges.len(), 3);
    }

    #[test]
    fn test_hash_join_empty_parents() {
        let (storage, _dir) = setup_test_storage();
        let include = create_test_include();

        // Insert a post that won't match
        let other_user = StorageEngine::generate_id();
        insert_post(&storage, StorageEngine::generate_id(), "Other Post", other_user);

        storage.flush().unwrap();

        // Query with empty parent list
        let source_ids: Vec<[u8; 16]> = vec![];
        let (rows, edges) = HashJoinExecutor::execute(&storage, &source_ids, &include).unwrap();

        assert!(rows.is_empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn test_hash_join_no_matching_children() {
        let (storage, _dir) = setup_test_storage();
        let include = create_test_include();

        let user1_id = StorageEngine::generate_id();
        let other_user = StorageEngine::generate_id();

        // Insert posts for a different user
        insert_post(&storage, StorageEngine::generate_id(), "Other Post", other_user);

        storage.flush().unwrap();

        // Query user1 who has no posts
        let source_ids = vec![user1_id];
        let (rows, edges) = HashJoinExecutor::execute(&storage, &source_ids, &include).unwrap();

        assert!(rows.is_empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn test_nested_loop_same_results_as_hash_join() {
        let (storage, _dir) = setup_test_storage();
        let include = create_test_include();

        let user1_id = StorageEngine::generate_id();
        let user2_id = StorageEngine::generate_id();

        insert_post(&storage, StorageEngine::generate_id(), "Post 1", user1_id);
        insert_post(&storage, StorageEngine::generate_id(), "Post 2", user1_id);
        insert_post(&storage, StorageEngine::generate_id(), "Post 3", user2_id);

        storage.flush().unwrap();

        let source_ids = vec![user1_id, user2_id];

        let (hash_rows, hash_edges) =
            HashJoinExecutor::execute(&storage, &source_ids, &include).unwrap();
        let (loop_rows, loop_edges) =
            NestedLoopExecutor::execute(&storage, &source_ids, &include).unwrap();

        // Same number of results
        assert_eq!(hash_rows.len(), loop_rows.len());
        assert_eq!(hash_edges.len(), loop_edges.len());

        // Same entity IDs (order may differ)
        let mut hash_ids: Vec<_> = hash_rows.iter().map(|r| r.id).collect();
        let mut loop_ids: Vec<_> = loop_rows.iter().map(|r| r.id).collect();
        hash_ids.sort();
        loop_ids.sort();
        assert_eq!(hash_ids, loop_ids);
    }

    #[test]
    fn test_join_strategy_selection() {
        // Small dataset: prefer nested loop
        assert_eq!(JoinStrategy::select(10, 100), JoinStrategy::NestedLoop);
        assert_eq!(JoinStrategy::select(50, 500), JoinStrategy::NestedLoop);

        // Large parent count: prefer hash join
        assert_eq!(JoinStrategy::select(101, 100), JoinStrategy::HashJoin);

        // Large child count: prefer hash join
        assert_eq!(JoinStrategy::select(10, 1001), JoinStrategy::HashJoin);

        // Both large: prefer hash join
        assert_eq!(JoinStrategy::select(200, 5000), JoinStrategy::HashJoin);
    }

    #[test]
    fn test_execute_join_function() {
        let (storage, _dir) = setup_test_storage();
        let include = create_test_include();

        let user_id = StorageEngine::generate_id();
        insert_post(&storage, StorageEngine::generate_id(), "Test Post", user_id);

        storage.flush().unwrap();

        let source_ids = vec![user_id];

        // Test with both strategies
        let (rows1, _) =
            execute_join(JoinStrategy::NestedLoop, &storage, &source_ids, &include).unwrap();
        let (rows2, _) =
            execute_join(JoinStrategy::HashJoin, &storage, &source_ids, &include).unwrap();

        assert_eq!(rows1.len(), rows2.len());
    }
}
