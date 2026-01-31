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
}
