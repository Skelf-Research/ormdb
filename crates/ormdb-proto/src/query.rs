//! Query IR types for graph queries.

use crate::value::Value;
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Aggregate function types for analytics queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum AggregateFunction {
    /// Count of entities/values.
    Count,
    /// Sum of numeric values.
    Sum,
    /// Average of numeric values.
    Avg,
    /// Minimum value.
    Min,
    /// Maximum value.
    Max,
}

/// A single aggregation operation.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Aggregation {
    /// The aggregation function to apply.
    pub function: AggregateFunction,
    /// Field to aggregate (None for COUNT(*)).
    pub field: Option<String>,
}

impl Aggregation {
    /// Create a COUNT(*) aggregation.
    pub fn count() -> Self {
        Self {
            function: AggregateFunction::Count,
            field: None,
        }
    }

    /// Create a COUNT(field) aggregation.
    pub fn count_field(field: impl Into<String>) -> Self {
        Self {
            function: AggregateFunction::Count,
            field: Some(field.into()),
        }
    }

    /// Create a SUM aggregation.
    pub fn sum(field: impl Into<String>) -> Self {
        Self {
            function: AggregateFunction::Sum,
            field: Some(field.into()),
        }
    }

    /// Create an AVG aggregation.
    pub fn avg(field: impl Into<String>) -> Self {
        Self {
            function: AggregateFunction::Avg,
            field: Some(field.into()),
        }
    }

    /// Create a MIN aggregation.
    pub fn min(field: impl Into<String>) -> Self {
        Self {
            function: AggregateFunction::Min,
            field: Some(field.into()),
        }
    }

    /// Create a MAX aggregation.
    pub fn max(field: impl Into<String>) -> Self {
        Self {
            function: AggregateFunction::Max,
            field: Some(field.into()),
        }
    }
}

/// An aggregate query for analytics operations.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct AggregateQuery {
    /// Root entity type to aggregate.
    pub root_entity: String,
    /// Aggregation operations to perform.
    pub aggregations: Vec<Aggregation>,
    /// Optional filter to apply before aggregation.
    pub filter: Option<Filter>,
}

impl AggregateQuery {
    /// Create a new aggregate query for an entity.
    pub fn new(root_entity: impl Into<String>) -> Self {
        Self {
            root_entity: root_entity.into(),
            aggregations: vec![],
            filter: None,
        }
    }

    /// Add an aggregation operation.
    pub fn with_aggregation(mut self, aggregation: Aggregation) -> Self {
        self.aggregations.push(aggregation);
        self
    }

    /// Add a COUNT(*) aggregation.
    pub fn count(mut self) -> Self {
        self.aggregations.push(Aggregation::count());
        self
    }

    /// Add a SUM aggregation.
    pub fn sum(mut self, field: impl Into<String>) -> Self {
        self.aggregations.push(Aggregation::sum(field));
        self
    }

    /// Add an AVG aggregation.
    pub fn avg(mut self, field: impl Into<String>) -> Self {
        self.aggregations.push(Aggregation::avg(field));
        self
    }

    /// Add a MIN aggregation.
    pub fn min(mut self, field: impl Into<String>) -> Self {
        self.aggregations.push(Aggregation::min(field));
        self
    }

    /// Add a MAX aggregation.
    pub fn max(mut self, field: impl Into<String>) -> Self {
        self.aggregations.push(Aggregation::max(field));
        self
    }

    /// Set a filter for this query.
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// A graph query that fetches entities and their relations.
///
/// Note: To avoid recursive type issues with rkyv, includes are represented
/// as a flat list with path-based nesting (e.g., "posts", "posts.comments").
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct GraphQuery {
    /// The root entity type to query.
    pub root_entity: String,
    /// Fields to select from the root entity.
    pub fields: Vec<String>,
    /// Flat list of relation includes (nested relations use dot-notation paths).
    pub includes: Vec<RelationInclude>,
    /// Optional filter for the root entity.
    pub filter: Option<Filter>,
    /// Ordering specification.
    pub order_by: Vec<OrderSpec>,
    /// Pagination parameters.
    pub pagination: Option<Pagination>,
}

/// An included relation in a graph query.
///
/// The `path` field uses dot-notation for nested relations:
/// - "posts" - include posts from root entity
/// - "posts.comments" - include comments from posts
/// - "posts.author" - include author from posts
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct RelationInclude {
    /// Dot-separated path to this relation (e.g., "posts.comments").
    pub path: String,
    /// Fields to select from the related entity.
    pub fields: Vec<String>,
    /// Optional filter for the related entities.
    pub filter: Option<Filter>,
    /// Ordering for the related entities.
    pub order_by: Vec<OrderSpec>,
    /// Pagination for the related entities.
    pub pagination: Option<Pagination>,
}

impl RelationInclude {
    /// Create a new include for a relation.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            fields: vec![],
            filter: None,
            order_by: vec![],
            pagination: None,
        }
    }

    /// Set the fields to select.
    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = fields;
        self
    }

    /// Set a filter for this include.
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Add ordering for this include.
    pub fn with_order(mut self, order: OrderSpec) -> Self {
        self.order_by.push(order);
        self
    }

    /// Set pagination for this include.
    pub fn with_pagination(mut self, pagination: Pagination) -> Self {
        self.pagination = Some(pagination);
        self
    }

    /// Get the relation name (last segment of the path).
    pub fn relation_name(&self) -> &str {
        self.path.rsplit('.').next().unwrap_or(&self.path)
    }

    /// Get the parent path (all segments except the last).
    pub fn parent_path(&self) -> Option<&str> {
        self.path.rsplit_once('.').map(|(parent, _)| parent)
    }

    /// Check if this is a top-level include (no dots in path).
    pub fn is_top_level(&self) -> bool {
        !self.path.contains('.')
    }

    /// Get the depth of this include (number of dots + 1).
    pub fn depth(&self) -> usize {
        self.path.matches('.').count() + 1
    }
}

/// A filter condition wrapper.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Filter {
    /// The filter expression.
    pub expression: FilterExpr,
}

impl Filter {
    /// Create a filter from an expression.
    pub fn new(expression: FilterExpr) -> Self {
        Self { expression }
    }
}

impl From<FilterExpr> for Filter {
    fn from(expression: FilterExpr) -> Self {
        Self { expression }
    }
}

/// Filter expression for querying entities.
///
/// Note: This uses a flat design without recursive Box types to work with rkyv.
/// And/Or contain Vec<FilterExpr> which is handled specially.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum FilterExpr {
    /// Field equals value.
    Eq { field: String, value: Value },
    /// Field not equals value.
    Ne { field: String, value: Value },
    /// Field less than value.
    Lt { field: String, value: Value },
    /// Field less than or equal to value.
    Le { field: String, value: Value },
    /// Field greater than value.
    Gt { field: String, value: Value },
    /// Field greater than or equal to value.
    Ge { field: String, value: Value },
    /// Field is in a set of values.
    In { field: String, values: Vec<Value> },
    /// Field is not in a set of values.
    NotIn { field: String, values: Vec<Value> },
    /// Field is null.
    IsNull { field: String },
    /// Field is not null.
    IsNotNull { field: String },
    /// Field matches a LIKE pattern.
    Like { field: String, pattern: String },
    /// Field does not match a LIKE pattern.
    NotLike { field: String, pattern: String },
    /// All conditions must be true (flat list, single level).
    And(Vec<SimpleFilter>),
    /// At least one condition must be true (flat list, single level).
    Or(Vec<SimpleFilter>),

    // === Vector Search ===
    /// Find k nearest neighbors by vector similarity.
    VectorNearestNeighbor {
        /// Field containing the vector.
        field: String,
        /// Query vector to search for.
        query_vector: Vec<f32>,
        /// Number of nearest neighbors to return.
        k: u32,
        /// Optional maximum distance threshold.
        max_distance: Option<f32>,
    },

    // === Geo Search ===
    /// Find points within a radius of a center point.
    GeoWithinRadius {
        /// Field containing the geo point.
        field: String,
        /// Center latitude in degrees (-90 to 90).
        center_lat: f64,
        /// Center longitude in degrees (-180 to 180).
        center_lon: f64,
        /// Radius in kilometers.
        radius_km: f64,
    },
    /// Find points within a bounding box.
    GeoWithinBox {
        /// Field containing the geo point.
        field: String,
        /// Minimum latitude.
        min_lat: f64,
        /// Minimum longitude.
        min_lon: f64,
        /// Maximum latitude.
        max_lat: f64,
        /// Maximum longitude.
        max_lon: f64,
    },
    /// Find points within a polygon.
    GeoWithinPolygon {
        /// Field containing the geo point.
        field: String,
        /// Polygon vertices as (lat, lon) pairs.
        vertices: Vec<(f64, f64)>,
    },
    /// Find k nearest points to a location.
    GeoNearestNeighbor {
        /// Field containing the geo point.
        field: String,
        /// Center latitude.
        center_lat: f64,
        /// Center longitude.
        center_lon: f64,
        /// Number of nearest neighbors.
        k: u32,
    },

    // === Full Text Search ===
    /// Match documents containing query terms (BM25 scored).
    TextMatch {
        /// Field containing the text.
        field: String,
        /// Search query (will be tokenized).
        query: String,
        /// Optional minimum relevance score (0.0 to 1.0).
        min_score: Option<f32>,
    },
    /// Match documents containing an exact phrase.
    TextPhrase {
        /// Field containing the text.
        field: String,
        /// Exact phrase to search for.
        phrase: String,
    },
    /// Boolean text search with must/should/must_not.
    TextBoolean {
        /// Field containing the text.
        field: String,
        /// Terms that must appear.
        must: Vec<String>,
        /// Terms that should appear (boost score).
        should: Vec<String>,
        /// Terms that must not appear.
        must_not: Vec<String>,
    },
}

/// A simple (non-compound) filter for use in And/Or expressions.
///
/// This flattens the filter structure to avoid recursion issues with rkyv.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum SimpleFilter {
    /// Field equals value.
    Eq { field: String, value: Value },
    /// Field not equals value.
    Ne { field: String, value: Value },
    /// Field less than value.
    Lt { field: String, value: Value },
    /// Field less than or equal to value.
    Le { field: String, value: Value },
    /// Field greater than value.
    Gt { field: String, value: Value },
    /// Field greater than or equal to value.
    Ge { field: String, value: Value },
    /// Field is in a set of values.
    In { field: String, values: Vec<Value> },
    /// Field is not in a set of values.
    NotIn { field: String, values: Vec<Value> },
    /// Field is null.
    IsNull { field: String },
    /// Field is not null.
    IsNotNull { field: String },
    /// Field matches a LIKE pattern.
    Like { field: String, pattern: String },
    /// Field does not match a LIKE pattern.
    NotLike { field: String, pattern: String },

    // === Vector Search ===
    /// Find k nearest neighbors by vector similarity.
    VectorNearestNeighbor {
        /// Field containing the vector.
        field: String,
        /// Query vector to search for.
        query_vector: Vec<f32>,
        /// Number of nearest neighbors to return.
        k: u32,
        /// Optional maximum distance threshold.
        max_distance: Option<f32>,
    },

    // === Geo Search ===
    /// Find points within a radius of a center point.
    GeoWithinRadius {
        /// Field containing the geo point.
        field: String,
        /// Center latitude in degrees (-90 to 90).
        center_lat: f64,
        /// Center longitude in degrees (-180 to 180).
        center_lon: f64,
        /// Radius in kilometers.
        radius_km: f64,
    },
    /// Find points within a bounding box.
    GeoWithinBox {
        /// Field containing the geo point.
        field: String,
        /// Minimum latitude.
        min_lat: f64,
        /// Minimum longitude.
        min_lon: f64,
        /// Maximum latitude.
        max_lat: f64,
        /// Maximum longitude.
        max_lon: f64,
    },
    /// Find points within a polygon.
    GeoWithinPolygon {
        /// Field containing the geo point.
        field: String,
        /// Polygon vertices as (lat, lon) pairs.
        vertices: Vec<(f64, f64)>,
    },
    /// Find k nearest points to a location.
    GeoNearestNeighbor {
        /// Field containing the geo point.
        field: String,
        /// Center latitude.
        center_lat: f64,
        /// Center longitude.
        center_lon: f64,
        /// Number of nearest neighbors.
        k: u32,
    },

    // === Full Text Search ===
    /// Match documents containing query terms (BM25 scored).
    TextMatch {
        /// Field containing the text.
        field: String,
        /// Search query (will be tokenized).
        query: String,
        /// Optional minimum relevance score (0.0 to 1.0).
        min_score: Option<f32>,
    },
    /// Match documents containing an exact phrase.
    TextPhrase {
        /// Field containing the text.
        field: String,
        /// Exact phrase to search for.
        phrase: String,
    },
    /// Boolean text search with must/should/must_not.
    TextBoolean {
        /// Field containing the text.
        field: String,
        /// Terms that must appear.
        must: Vec<String>,
        /// Terms that should appear (boost score).
        should: Vec<String>,
        /// Terms that must not appear.
        must_not: Vec<String>,
    },
}

impl SimpleFilter {
    /// Create an equality filter.
    pub fn eq(field: impl Into<String>, value: impl Into<Value>) -> Self {
        SimpleFilter::Eq {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a not-equal filter.
    pub fn ne(field: impl Into<String>, value: impl Into<Value>) -> Self {
        SimpleFilter::Ne {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create an IS NULL filter.
    pub fn is_null(field: impl Into<String>) -> Self {
        SimpleFilter::IsNull {
            field: field.into(),
        }
    }

    /// Create an IS NOT NULL filter.
    pub fn is_not_null(field: impl Into<String>) -> Self {
        SimpleFilter::IsNotNull {
            field: field.into(),
        }
    }
}

impl FilterExpr {
    /// Create an equality filter.
    pub fn eq(field: impl Into<String>, value: impl Into<Value>) -> Self {
        FilterExpr::Eq {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a not-equal filter.
    pub fn ne(field: impl Into<String>, value: impl Into<Value>) -> Self {
        FilterExpr::Ne {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a less-than filter.
    pub fn lt(field: impl Into<String>, value: impl Into<Value>) -> Self {
        FilterExpr::Lt {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a less-than-or-equal filter.
    pub fn le(field: impl Into<String>, value: impl Into<Value>) -> Self {
        FilterExpr::Le {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a greater-than filter.
    pub fn gt(field: impl Into<String>, value: impl Into<Value>) -> Self {
        FilterExpr::Gt {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create a greater-than-or-equal filter.
    pub fn ge(field: impl Into<String>, value: impl Into<Value>) -> Self {
        FilterExpr::Ge {
            field: field.into(),
            value: value.into(),
        }
    }

    /// Create an IN filter.
    pub fn in_values(field: impl Into<String>, values: Vec<Value>) -> Self {
        FilterExpr::In {
            field: field.into(),
            values,
        }
    }

    /// Create a NOT IN filter.
    pub fn not_in_values(field: impl Into<String>, values: Vec<Value>) -> Self {
        FilterExpr::NotIn {
            field: field.into(),
            values,
        }
    }

    /// Create an IS NULL filter.
    pub fn is_null(field: impl Into<String>) -> Self {
        FilterExpr::IsNull {
            field: field.into(),
        }
    }

    /// Create an IS NOT NULL filter.
    pub fn is_not_null(field: impl Into<String>) -> Self {
        FilterExpr::IsNotNull {
            field: field.into(),
        }
    }

    /// Create a LIKE filter.
    pub fn like(field: impl Into<String>, pattern: impl Into<String>) -> Self {
        FilterExpr::Like {
            field: field.into(),
            pattern: pattern.into(),
        }
    }

    /// Create an AND filter combining multiple simple expressions.
    pub fn and(exprs: Vec<SimpleFilter>) -> Self {
        FilterExpr::And(exprs)
    }

    /// Create an OR filter combining multiple simple expressions.
    pub fn or(exprs: Vec<SimpleFilter>) -> Self {
        FilterExpr::Or(exprs)
    }

    // === Vector Search Builders ===

    /// Create a vector nearest neighbor search filter.
    pub fn vector_nearest_neighbor(
        field: impl Into<String>,
        query_vector: Vec<f32>,
        k: u32,
        max_distance: Option<f32>,
    ) -> Self {
        FilterExpr::VectorNearestNeighbor {
            field: field.into(),
            query_vector,
            k,
            max_distance,
        }
    }

    // === Geo Search Builders ===

    /// Create a geo radius search filter.
    pub fn geo_within_radius(
        field: impl Into<String>,
        center_lat: f64,
        center_lon: f64,
        radius_km: f64,
    ) -> Self {
        FilterExpr::GeoWithinRadius {
            field: field.into(),
            center_lat,
            center_lon,
            radius_km,
        }
    }

    /// Create a geo bounding box search filter.
    pub fn geo_within_box(
        field: impl Into<String>,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
    ) -> Self {
        FilterExpr::GeoWithinBox {
            field: field.into(),
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        }
    }

    /// Create a geo polygon containment search filter.
    pub fn geo_within_polygon(field: impl Into<String>, vertices: Vec<(f64, f64)>) -> Self {
        FilterExpr::GeoWithinPolygon {
            field: field.into(),
            vertices,
        }
    }

    /// Create a geo nearest neighbor search filter.
    pub fn geo_nearest_neighbor(
        field: impl Into<String>,
        center_lat: f64,
        center_lon: f64,
        k: u32,
    ) -> Self {
        FilterExpr::GeoNearestNeighbor {
            field: field.into(),
            center_lat,
            center_lon,
            k,
        }
    }

    // === Full Text Search Builders ===

    /// Create a full-text match search filter.
    pub fn text_match(field: impl Into<String>, query: impl Into<String>, min_score: Option<f32>) -> Self {
        FilterExpr::TextMatch {
            field: field.into(),
            query: query.into(),
            min_score,
        }
    }

    /// Create a full-text phrase search filter.
    pub fn text_phrase(field: impl Into<String>, phrase: impl Into<String>) -> Self {
        FilterExpr::TextPhrase {
            field: field.into(),
            phrase: phrase.into(),
        }
    }

    /// Create a boolean text search filter.
    pub fn text_boolean(
        field: impl Into<String>,
        must: Vec<String>,
        should: Vec<String>,
        must_not: Vec<String>,
    ) -> Self {
        FilterExpr::TextBoolean {
            field: field.into(),
            must,
            should,
            must_not,
        }
    }
}

/// Order specification for sorting results.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct OrderSpec {
    /// Field to order by.
    pub field: String,
    /// Sort direction.
    pub direction: OrderDirection,
}

impl OrderSpec {
    /// Create an ascending order spec.
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: OrderDirection::Asc,
        }
    }

    /// Create a descending order spec.
    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            direction: OrderDirection::Desc,
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub enum OrderDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

/// Pagination parameters.
#[derive(Debug, Clone, PartialEq, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct Pagination {
    /// Maximum number of results to return.
    pub limit: u32,
    /// Number of results to skip.
    pub offset: u32,
    /// Optional cursor for cursor-based pagination.
    pub cursor: Option<Vec<u8>>,
}

impl Pagination {
    /// Create pagination with limit and offset.
    pub fn new(limit: u32, offset: u32) -> Self {
        Self {
            limit,
            offset,
            cursor: None,
        }
    }

    /// Create pagination with just a limit.
    pub fn limit(limit: u32) -> Self {
        Self {
            limit,
            offset: 0,
            cursor: None,
        }
    }

    /// Create cursor-based pagination.
    pub fn cursor(cursor: Vec<u8>, limit: u32) -> Self {
        Self {
            limit,
            offset: 0,
            cursor: Some(cursor),
        }
    }
}

impl GraphQuery {
    /// Create a new graph query for an entity.
    pub fn new(root_entity: impl Into<String>) -> Self {
        Self {
            root_entity: root_entity.into(),
            fields: vec![],
            includes: vec![],
            filter: None,
            order_by: vec![],
            pagination: None,
        }
    }

    /// Set the fields to select.
    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = fields;
        self
    }

    /// Add a field to select.
    pub fn select(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Add an include.
    pub fn include(mut self, include: RelationInclude) -> Self {
        self.includes.push(include);
        self
    }

    /// Set a filter for this query.
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Add ordering for this query.
    pub fn with_order(mut self, order: OrderSpec) -> Self {
        self.order_by.push(order);
        self
    }

    /// Set pagination for this query.
    pub fn with_pagination(mut self, pagination: Pagination) -> Self {
        self.pagination = Some(pagination);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let query = GraphQuery::new("User")
            .with_fields(vec!["id".into(), "name".into(), "email".into()])
            .with_filter(FilterExpr::eq("active", true).into())
            .with_order(OrderSpec::asc("name"))
            .with_pagination(Pagination::limit(10));

        assert_eq!(query.root_entity, "User");
        assert_eq!(query.fields.len(), 3);
        assert!(query.filter.is_some());
        assert_eq!(query.order_by.len(), 1);
        assert!(query.pagination.is_some());
    }

    #[test]
    fn test_nested_query_with_path_notation() {
        let query = GraphQuery::new("User")
            .with_fields(vec!["id".into(), "name".into()])
            .include(RelationInclude::new("posts").with_fields(vec!["id".into(), "title".into()]))
            .include(
                RelationInclude::new("posts.comments")
                    .with_filter(FilterExpr::is_not_null("content").into()),
            )
            .include(RelationInclude::new("posts.author"))
            .with_filter(FilterExpr::eq("id", Value::Uuid([1; 16])).into());

        assert_eq!(query.includes.len(), 3);
        assert_eq!(query.includes[0].path, "posts");
        assert!(query.includes[0].is_top_level());
        assert_eq!(query.includes[1].path, "posts.comments");
        assert!(!query.includes[1].is_top_level());
        assert_eq!(query.includes[1].parent_path(), Some("posts"));
        assert_eq!(query.includes[1].depth(), 2);
    }

    #[test]
    fn test_relation_include_helpers() {
        let include = RelationInclude::new("posts.comments.likes");
        assert_eq!(include.relation_name(), "likes");
        assert_eq!(include.parent_path(), Some("posts.comments"));
        assert_eq!(include.depth(), 3);
        assert!(!include.is_top_level());

        let top_level = RelationInclude::new("posts");
        assert_eq!(top_level.relation_name(), "posts");
        assert_eq!(top_level.parent_path(), None);
        assert_eq!(top_level.depth(), 1);
        assert!(top_level.is_top_level());
    }

    #[test]
    fn test_complex_filter() {
        let filter = FilterExpr::and(vec![
            SimpleFilter::eq("status", "active"),
            SimpleFilter::is_not_null("email"),
        ]);

        if let FilterExpr::And(exprs) = &filter {
            assert_eq!(exprs.len(), 2);
        } else {
            panic!("Expected And filter");
        }
    }

    #[test]
    fn test_query_serialization_roundtrip() {
        let query = GraphQuery::new("Post")
            .with_fields(vec!["id".into(), "title".into(), "content".into()])
            .include(RelationInclude::new("author"))
            .include(RelationInclude::new("comments").with_pagination(Pagination::limit(10)))
            .with_filter(
                FilterExpr::and(vec![
                    SimpleFilter::eq("published", true),
                ])
                .into(),
            )
            .with_order(OrderSpec::desc("created_at"))
            .with_pagination(Pagination::new(20, 0));

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&query).unwrap();
        let archived = rkyv::access::<ArchivedGraphQuery, rkyv::rancor::Error>(&bytes).unwrap();
        let deserialized: GraphQuery =
            rkyv::deserialize::<GraphQuery, rkyv::rancor::Error>(archived).unwrap();

        assert_eq!(query, deserialized);
    }

    #[test]
    fn test_vector_search_filter() {
        let filter = FilterExpr::vector_nearest_neighbor(
            "embedding",
            vec![0.1, 0.2, 0.3, 0.4],
            10,
            Some(0.5),
        );

        if let FilterExpr::VectorNearestNeighbor {
            field,
            query_vector,
            k,
            max_distance,
        } = filter
        {
            assert_eq!(field, "embedding");
            assert_eq!(query_vector.len(), 4);
            assert_eq!(k, 10);
            assert_eq!(max_distance, Some(0.5));
        } else {
            panic!("Expected VectorNearestNeighbor");
        }
    }

    #[test]
    fn test_geo_search_filters() {
        // Radius search
        let filter = FilterExpr::geo_within_radius("location", 37.7749, -122.4194, 10.0);
        if let FilterExpr::GeoWithinRadius {
            field,
            center_lat,
            center_lon,
            radius_km,
        } = filter
        {
            assert_eq!(field, "location");
            assert_eq!(center_lat, 37.7749);
            assert_eq!(center_lon, -122.4194);
            assert_eq!(radius_km, 10.0);
        } else {
            panic!("Expected GeoWithinRadius");
        }

        // Bounding box search
        let filter = FilterExpr::geo_within_box("location", 37.0, -123.0, 38.0, -122.0);
        if let FilterExpr::GeoWithinBox {
            min_lat,
            max_lat,
            min_lon,
            max_lon,
            ..
        } = filter
        {
            assert_eq!(min_lat, 37.0);
            assert_eq!(max_lat, 38.0);
            assert_eq!(min_lon, -123.0);
            assert_eq!(max_lon, -122.0);
        } else {
            panic!("Expected GeoWithinBox");
        }

        // Polygon containment
        let vertices = vec![
            (37.0, -122.0),
            (38.0, -122.0),
            (38.0, -121.0),
            (37.0, -121.0),
        ];
        let filter = FilterExpr::geo_within_polygon("location", vertices.clone());
        if let FilterExpr::GeoWithinPolygon { vertices: v, .. } = filter {
            assert_eq!(v, vertices);
        } else {
            panic!("Expected GeoWithinPolygon");
        }

        // Nearest neighbor
        let filter = FilterExpr::geo_nearest_neighbor("location", 37.7749, -122.4194, 5);
        if let FilterExpr::GeoNearestNeighbor { k, .. } = filter {
            assert_eq!(k, 5);
        } else {
            panic!("Expected GeoNearestNeighbor");
        }
    }

    #[test]
    fn test_text_search_filters() {
        // Text match
        let filter = FilterExpr::text_match("content", "rust programming", Some(0.5));
        if let FilterExpr::TextMatch {
            field,
            query,
            min_score,
        } = filter
        {
            assert_eq!(field, "content");
            assert_eq!(query, "rust programming");
            assert_eq!(min_score, Some(0.5));
        } else {
            panic!("Expected TextMatch");
        }

        // Phrase search
        let filter = FilterExpr::text_phrase("content", "memory safety");
        if let FilterExpr::TextPhrase { field, phrase } = filter {
            assert_eq!(field, "content");
            assert_eq!(phrase, "memory safety");
        } else {
            panic!("Expected TextPhrase");
        }

        // Boolean search
        let filter = FilterExpr::text_boolean(
            "content",
            vec!["rust".into()],
            vec!["programming".into(), "systems".into()],
            vec!["java".into()],
        );
        if let FilterExpr::TextBoolean {
            must,
            should,
            must_not,
            ..
        } = filter
        {
            assert_eq!(must, vec!["rust"]);
            assert_eq!(should, vec!["programming", "systems"]);
            assert_eq!(must_not, vec!["java"]);
        } else {
            panic!("Expected TextBoolean");
        }
    }

    #[test]
    fn test_search_filter_serialization() {
        let filters = vec![
            FilterExpr::vector_nearest_neighbor("embedding", vec![0.1, 0.2, 0.3], 5, None),
            FilterExpr::geo_within_radius("location", 40.7128, -74.0060, 25.0),
            FilterExpr::text_match("body", "hello world", Some(0.3)),
        ];

        for filter in filters {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&filter).unwrap();
            let archived = rkyv::access::<ArchivedFilterExpr, rkyv::rancor::Error>(&bytes).unwrap();
            let deserialized: FilterExpr =
                rkyv::deserialize::<FilterExpr, rkyv::rancor::Error>(archived).unwrap();
            assert_eq!(filter, deserialized);
        }
    }
}
