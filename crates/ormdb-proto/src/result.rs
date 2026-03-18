//! Result types for query responses.

use crate::query::AggregateFunction;
use crate::value::Value;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// A block of entities from a query result.
///
/// Uses column-oriented storage for efficient serialization and processing.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct EntityBlock {
    /// Entity type name.
    pub entity: String,
    /// Entity IDs (parallel with column values).
    pub ids: Vec<[u8; 16]>,
    /// Column data (each column has same length as ids).
    pub columns: Vec<ColumnData>,
}

/// Column data within an entity block.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct ColumnData {
    /// Column (field) name.
    pub name: String,
    /// Values for each row.
    pub values: Vec<Value>,
}

impl ColumnData {
    /// Create a new column.
    pub fn new(name: impl Into<String>, values: Vec<Value>) -> Self {
        Self {
            name: name.into(),
            values,
        }
    }
}

impl EntityBlock {
    /// Create a new empty entity block.
    pub fn new(entity: impl Into<String>) -> Self {
        Self {
            entity: entity.into(),
            ids: vec![],
            columns: vec![],
        }
    }

    /// Create an entity block with data.
    pub fn with_data(
        entity: impl Into<String>,
        ids: Vec<[u8; 16]>,
        columns: Vec<ColumnData>,
    ) -> Self {
        Self {
            entity: entity.into(),
            ids,
            columns,
        }
    }

    /// Get the number of rows in this block.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Check if this block is empty.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Get a column by name.
    pub fn column(&self, name: &str) -> Option<&ColumnData> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Get the value at a specific row and column.
    pub fn get(&self, row: usize, column: &str) -> Option<&Value> {
        self.column(column).and_then(|c| c.values.get(row))
    }

    /// Iterate over rows as (id, field_values) pairs.
    pub fn rows(&self) -> impl Iterator<Item = (&[u8; 16], Vec<(&str, &Value)>)> {
        self.ids.iter().enumerate().map(|(i, id)| {
            let fields: Vec<(&str, &Value)> = self
                .columns
                .iter()
                .map(|col| (col.name.as_str(), &col.values[i]))
                .collect();
            (id, fields)
        })
    }
}

/// A block of edges (relationships) from a query result.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct EdgeBlock {
    /// Relation name.
    pub relation: String,
    /// Edges in this block.
    pub edges: Vec<Edge>,
}

/// A single edge connecting two entities.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Edge {
    /// ID of the source entity.
    pub from_id: [u8; 16],
    /// ID of the target entity.
    pub to_id: [u8; 16],
}

impl Edge {
    /// Create a new edge.
    pub fn new(from_id: [u8; 16], to_id: [u8; 16]) -> Self {
        Self { from_id, to_id }
    }
}

impl EdgeBlock {
    /// Create a new empty edge block.
    pub fn new(relation: impl Into<String>) -> Self {
        Self {
            relation: relation.into(),
            edges: vec![],
        }
    }

    /// Create an edge block with edges.
    pub fn with_edges(relation: impl Into<String>, edges: Vec<Edge>) -> Self {
        Self {
            relation: relation.into(),
            edges,
        }
    }

    /// Get the number of edges in this block.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Check if this block is empty.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Add an edge to this block.
    pub fn push(&mut self, edge: Edge) {
        self.edges.push(edge);
    }
}

/// Complete result of a graph query.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct QueryResult {
    /// Entity blocks (one per entity type in the result).
    pub entities: Vec<EntityBlock>,
    /// Edge blocks (one per relation in the result).
    pub edges: Vec<EdgeBlock>,
    /// Whether there are more results available (pagination).
    pub has_more: bool,
}

impl QueryResult {
    /// Create an empty query result.
    pub fn empty() -> Self {
        Self {
            entities: vec![],
            edges: vec![],
            has_more: false,
        }
    }

    /// Create a query result with entity and edge blocks.
    pub fn new(entities: Vec<EntityBlock>, edges: Vec<EdgeBlock>, has_more: bool) -> Self {
        Self {
            entities,
            edges,
            has_more,
        }
    }

    /// Get an entity block by entity type.
    pub fn entity_block(&self, entity: &str) -> Option<&EntityBlock> {
        self.entities.iter().find(|b| b.entity == entity)
    }

    /// Get an edge block by relation name.
    pub fn edge_block(&self, relation: &str) -> Option<&EdgeBlock> {
        self.edges.iter().find(|b| b.relation == relation)
    }

    /// Get total number of entities across all blocks.
    pub fn total_entities(&self) -> usize {
        self.entities.iter().map(|b| b.len()).sum()
    }

    /// Get total number of edges across all blocks.
    pub fn total_edges(&self) -> usize {
        self.edges.iter().map(|b| b.len()).sum()
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::empty()
    }
}

/// Result of a mutation operation.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct MutationResult {
    /// Number of entities affected.
    pub affected: u64,
    /// IDs of inserted entities (for inserts).
    pub inserted_ids: Vec<[u8; 16]>,
}

impl MutationResult {
    /// Create a result for a successful insert.
    pub fn inserted(id: [u8; 16]) -> Self {
        Self {
            affected: 1,
            inserted_ids: vec![id],
        }
    }

    /// Create a result for a successful update or delete.
    pub fn affected(count: u64) -> Self {
        Self {
            affected: count,
            inserted_ids: vec![],
        }
    }

    /// Create a result for multiple inserts.
    pub fn bulk_inserted(ids: Vec<[u8; 16]>) -> Self {
        let affected = ids.len() as u64;
        Self {
            affected,
            inserted_ids: ids,
        }
    }
}

/// Result of an aggregate query.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct AggregateResult {
    /// Entity type that was aggregated.
    pub entity: String,
    /// Results for each aggregation.
    pub values: Vec<AggregateValue>,
}

impl AggregateResult {
    /// Create a new aggregate result.
    pub fn new(entity: impl Into<String>, values: Vec<AggregateValue>) -> Self {
        Self {
            entity: entity.into(),
            values,
        }
    }

    /// Create an empty aggregate result.
    pub fn empty(entity: impl Into<String>) -> Self {
        Self {
            entity: entity.into(),
            values: vec![],
        }
    }

    /// Get the first value (for single-aggregation queries).
    pub fn first_value(&self) -> Option<&Value> {
        self.values.first().map(|v| &v.value)
    }

    /// Get the count value if present.
    pub fn count(&self) -> Option<i64> {
        self.values
            .iter()
            .find(|v| matches!(v.function, AggregateFunction::Count))
            .and_then(|v| match &v.value {
                Value::Int64(n) => Some(*n),
                _ => None,
            })
    }
}

/// A single aggregated value from an aggregate query.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct AggregateValue {
    /// Function that produced this value.
    pub function: AggregateFunction,
    /// Field that was aggregated (None for COUNT(*)).
    pub field: Option<String>,
    /// The aggregated value.
    pub value: Value,
}

impl AggregateValue {
    /// Create a new aggregate value.
    pub fn new(function: AggregateFunction, field: Option<String>, value: Value) -> Self {
        Self {
            function,
            field,
            value,
        }
    }

    /// Create a COUNT(*) result.
    pub fn count(value: i64) -> Self {
        Self {
            function: AggregateFunction::Count,
            field: None,
            value: Value::Int64(value),
        }
    }

    /// Create a COUNT(field) result.
    pub fn count_field(field: impl Into<String>, value: i64) -> Self {
        Self {
            function: AggregateFunction::Count,
            field: Some(field.into()),
            value: Value::Int64(value),
        }
    }

    /// Create a SUM result.
    pub fn sum(field: impl Into<String>, value: f64) -> Self {
        Self {
            function: AggregateFunction::Sum,
            field: Some(field.into()),
            value: Value::Float64(value),
        }
    }

    /// Create an AVG result.
    pub fn avg(field: impl Into<String>, value: f64) -> Self {
        Self {
            function: AggregateFunction::Avg,
            field: Some(field.into()),
            value: Value::Float64(value),
        }
    }

    /// Create a MIN result.
    pub fn min(field: impl Into<String>, value: Value) -> Self {
        Self {
            function: AggregateFunction::Min,
            field: Some(field.into()),
            value,
        }
    }

    /// Create a MAX result.
    pub fn max(field: impl Into<String>, value: Value) -> Self {
        Self {
            function: AggregateFunction::Max,
            field: Some(field.into()),
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_block() {
        let block = EntityBlock::with_data(
            "User",
            vec![[1u8; 16], [2u8; 16]],
            vec![
                ColumnData::new("name", vec![Value::String("Alice".into()), Value::String("Bob".into())]),
                ColumnData::new("age", vec![Value::Int32(30), Value::Int32(25)]),
            ],
        );

        assert_eq!(block.entity, "User");
        assert_eq!(block.len(), 2);
        assert!(!block.is_empty());

        assert_eq!(
            block.get(0, "name"),
            Some(&Value::String("Alice".into()))
        );
        assert_eq!(block.get(1, "age"), Some(&Value::Int32(25)));
        assert_eq!(block.get(2, "name"), None); // Out of bounds
    }

    #[test]
    fn test_entity_block_rows() {
        let block = EntityBlock::with_data(
            "Post",
            vec![[1u8; 16]],
            vec![
                ColumnData::new("title", vec![Value::String("Hello".into())]),
                ColumnData::new("views", vec![Value::Int64(100)]),
            ],
        );

        let rows: Vec<_> = block.rows().collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, &[1u8; 16]);
        assert_eq!(rows[0].1.len(), 2);
    }

    #[test]
    fn test_edge_block() {
        let mut block = EdgeBlock::new("user_posts");
        assert!(block.is_empty());

        block.push(Edge::new([1u8; 16], [10u8; 16]));
        block.push(Edge::new([1u8; 16], [11u8; 16]));

        assert_eq!(block.len(), 2);
        assert!(!block.is_empty());
    }

    #[test]
    fn test_query_result() {
        let result = QueryResult::new(
            vec![
                EntityBlock::with_data("User", vec![[1u8; 16]], vec![]),
                EntityBlock::with_data("Post", vec![[10u8; 16], [11u8; 16]], vec![]),
            ],
            vec![EdgeBlock::with_edges(
                "user_posts",
                vec![
                    Edge::new([1u8; 16], [10u8; 16]),
                    Edge::new([1u8; 16], [11u8; 16]),
                ],
            )],
            false,
        );

        assert_eq!(result.total_entities(), 3);
        assert_eq!(result.total_edges(), 2);
        assert!(!result.has_more);

        assert!(result.entity_block("User").is_some());
        assert!(result.entity_block("Comment").is_none());
        assert!(result.edge_block("user_posts").is_some());
    }

    #[test]
    fn test_result_serialization_roundtrip() {
        let result = QueryResult::new(
            vec![EntityBlock::with_data(
                "User",
                vec![[1u8; 16], [2u8; 16]],
                vec![
                    ColumnData::new(
                        "name",
                        vec![Value::String("Alice".into()), Value::String("Bob".into())],
                    ),
                    ColumnData::new("active", vec![Value::Bool(true), Value::Bool(false)]),
                ],
            )],
            vec![EdgeBlock::with_edges(
                "friends",
                vec![Edge::new([1u8; 16], [2u8; 16])],
            )],
            true,
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
        let archived = rkyv::access::<ArchivedQueryResult, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: QueryResult =
            rkyv::deserialize::<QueryResult, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(result, deserialized);
    }
}
