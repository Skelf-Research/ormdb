# Query API Reference

Complete reference for ORMDB query types.

## GraphQuery

The main query type for fetching entities and relations.

### Rust

```rust
pub struct GraphQuery {
    pub root_entity: String,
    pub fields: Option<Vec<String>>,
    pub filter: Option<Filter>,
    pub order_by: Vec<OrderSpec>,
    pub pagination: Option<Pagination>,
    pub includes: Vec<RelationInclude>,
    pub budget: Option<FanoutBudget>,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(entity)` | Create query for entity type |
| `with_fields(fields)` | Select specific fields |
| `with_filter(filter)` | Add filter predicate |
| `with_order(order)` | Add single ordering |
| `with_orders(orders)` | Add multiple orderings |
| `with_pagination(pagination)` | Add limit/offset |
| `include(include)` | Add relation include |
| `with_budget(budget)` | Set query budget |

### Example

```rust
let query = GraphQuery::new("User")
    .with_fields(vec!["id", "name", "email"])
    .with_filter(FilterExpr::eq("status", Value::String("active".into())))
    .with_order(OrderSpec::asc("name"))
    .with_pagination(Pagination::new(10, 0))
    .include(RelationInclude::new("posts"));
```

---

## FilterExpr

Filter expressions for querying data.

### Variants

```rust
pub enum FilterExpr {
    // Comparison
    Eq { field: String, value: Value },
    Ne { field: String, value: Value },
    Lt { field: String, value: Value },
    Le { field: String, value: Value },
    Gt { field: String, value: Value },
    Ge { field: String, value: Value },

    // Pattern matching
    Like { field: String, pattern: String },
    ILike { field: String, pattern: String },

    // Set operations
    In { field: String, values: Vec<Value> },
    NotIn { field: String, values: Vec<Value> },

    // Null checks
    IsNull { field: String },
    IsNotNull { field: String },

    // Logical operators
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
    Not(Box<FilterExpr>),
}
```

### Constructor Functions

| Function | Description | Example |
|----------|-------------|---------|
| `eq(field, value)` | Equals | `FilterExpr::eq("status", Value::String("active".into()))` |
| `ne(field, value)` | Not equals | `FilterExpr::ne("status", Value::String("deleted".into()))` |
| `lt(field, value)` | Less than | `FilterExpr::lt("age", Value::Int32(30))` |
| `le(field, value)` | Less than or equal | `FilterExpr::le("price", Value::Float64(100.0))` |
| `gt(field, value)` | Greater than | `FilterExpr::gt("age", Value::Int32(18))` |
| `ge(field, value)` | Greater than or equal | `FilterExpr::ge("score", Value::Float64(0.5))` |
| `like(field, pattern)` | LIKE pattern | `FilterExpr::like("email", "%@example.com")` |
| `ilike(field, pattern)` | Case-insensitive LIKE | `FilterExpr::ilike("name", "%alice%")` |
| `in_values(field, values)` | IN list | `FilterExpr::in_values("status", vec![...])` |
| `is_null(field)` | IS NULL | `FilterExpr::is_null("deleted_at")` |
| `is_not_null(field)` | IS NOT NULL | `FilterExpr::is_not_null("email")` |
| `and(exprs)` | Logical AND | `FilterExpr::and(vec![expr1, expr2])` |
| `or(exprs)` | Logical OR | `FilterExpr::or(vec![expr1, expr2])` |
| `not(expr)` | Logical NOT | `FilterExpr::not(Box::new(expr))` |

---

## Search Filters

Beyond standard comparison operators, ORMDB supports specialized search filters.

### VectorNearestNeighbor

HNSW k-nearest neighbor search for vector fields.

```rust
pub struct VectorNearestNeighbor {
    pub field: String,
    pub query_vector: Vec<f32>,
    pub k: usize,
    pub max_distance: Option<f32>,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | Vector field to search |
| `query_vector` | float[] | Query embedding vector |
| `k` | int | Number of nearest neighbors to return |
| `max_distance` | float? | Optional maximum distance threshold |

### GeoWithinRadius

Geographic radius search for GeoPoint fields.

```rust
pub struct GeoWithinRadius {
    pub field: String,
    pub center_lat: f64,
    pub center_lon: f64,
    pub radius_km: f64,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | GeoPoint field to search |
| `center_lat` | float | Center latitude (-90 to 90) |
| `center_lon` | float | Center longitude (-180 to 180) |
| `radius_km` | float | Search radius in kilometers |

### GeoWithinBox

Geographic bounding box search.

```rust
pub struct GeoWithinBox {
    pub field: String,
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | GeoPoint field to search |
| `min_lat` | float | Minimum latitude |
| `min_lon` | float | Minimum longitude |
| `max_lat` | float | Maximum latitude |
| `max_lon` | float | Maximum longitude |

### GeoWithinPolygon

Geographic polygon containment search.

```rust
pub struct GeoWithinPolygon {
    pub field: String,
    pub vertices: Vec<(f64, f64)>,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | GeoPoint field to search |
| `vertices` | (float, float)[] | Polygon vertices as (lat, lon) pairs |

### GeoNearestNeighbor

Geographic k-nearest neighbor search.

```rust
pub struct GeoNearestNeighbor {
    pub field: String,
    pub center_lat: f64,
    pub center_lon: f64,
    pub k: usize,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | GeoPoint field to search |
| `center_lat` | float | Center latitude |
| `center_lon` | float | Center longitude |
| `k` | int | Number of nearest neighbors to return |

### TextMatch

Full-text search with BM25 scoring.

```rust
pub struct TextMatch {
    pub field: String,
    pub query: String,
    pub min_score: Option<f32>,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | Text field to search |
| `query` | string | Search query (tokenized and stemmed) |
| `min_score` | float? | Minimum relevance score (0.0 to 1.0) |

### TextPhrase

Full-text exact phrase search.

```rust
pub struct TextPhrase {
    pub field: String,
    pub phrase: String,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | Text field to search |
| `phrase` | string | Exact phrase to match |

### TextBoolean

Full-text boolean search with must/should/must_not terms.

```rust
pub struct TextBoolean {
    pub field: String,
    pub must: Vec<String>,
    pub should: Vec<String>,
    pub must_not: Vec<String>,
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | Text field to search |
| `must` | string[] | Terms that must appear |
| `should` | string[] | Terms that should appear (boost relevance) |
| `must_not` | string[] | Terms that must not appear |

---

## OrderSpec

Ordering specification.

```rust
pub struct OrderSpec {
    pub field: String,
    pub direction: OrderDirection,
}

pub enum OrderDirection {
    Asc,
    Desc,
}
```

### Constructor Functions

| Function | Description |
|----------|-------------|
| `OrderSpec::asc(field)` | Ascending order |
| `OrderSpec::desc(field)` | Descending order |

---

## Pagination

Limit and offset pagination.

```rust
pub struct Pagination {
    pub limit: u32,
    pub offset: u32,
}
```

### Constructor

```rust
Pagination::new(limit, offset)
```

---

## RelationInclude

Include related entities.

```rust
pub struct RelationInclude {
    pub relation: String,
    pub fields: Option<Vec<String>>,
    pub filter: Option<Filter>,
    pub order_by: Vec<OrderSpec>,
    pub limit: Option<u32>,
    pub includes: Vec<RelationInclude>,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(relation)` | Create include for relation |
| `with_fields(fields)` | Select specific fields |
| `with_filter(filter)` | Filter related entities |
| `with_order(order)` | Order related entities |
| `with_limit(limit)` | Limit related entities |
| `include(include)` | Nested include |

### Example

```rust
RelationInclude::new("posts")
    .with_fields(vec!["id", "title"])
    .with_filter(FilterExpr::eq("published", Value::Bool(true)))
    .with_order(OrderSpec::desc("created_at"))
    .with_limit(10)
    .include(RelationInclude::new("comments"))
```

---

## FanoutBudget

Query budget limits.

```rust
pub struct FanoutBudget {
    pub max_entities: usize,
    pub max_edges: usize,
    pub max_depth: usize,
}
```

### Default Values

| Field | Default |
|-------|---------|
| `max_entities` | 10,000 |
| `max_edges` | 50,000 |
| `max_depth` | 5 |

---

## QueryResult

Result of a graph query.

```rust
pub struct QueryResult {
    pub entity_blocks: Vec<EntityBlock>,
    pub edge_blocks: Vec<EdgeBlock>,
    pub has_more: bool,
}
```

### Methods

| Method | Description |
|--------|-------------|
| `entities(type)` | Get entities of a type |
| `related(entity, relation)` | Get related entities |
| `related_one(entity, relation)` | Get single related entity |
| `total_entities()` | Count total entities |

---

## AggregateQuery

Query for aggregate operations.

```rust
pub struct AggregateQuery {
    pub root_entity: String,
    pub aggregations: Vec<Aggregation>,
    pub filter: Option<Filter>,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(entity)` | Create aggregate query |
| `count()` | COUNT(*) |
| `count_field(field)` | COUNT(field) |
| `sum(field)` | SUM(field) |
| `avg(field)` | AVG(field) |
| `min(field)` | MIN(field) |
| `max(field)` | MAX(field) |
| `with_filter(filter)` | Add filter |

### Example

```rust
let query = AggregateQuery::new("Order")
    .count()
    .sum("total")
    .avg("total")
    .with_filter(FilterExpr::eq("status", Value::String("completed".into())));
```

---

## Next Steps

- **[Mutation API](mutation-api.md)** - Create, update, and delete data
- **[Querying Data Tutorial](../tutorials/querying-data.md)** - Learn query patterns
- **[Performance Guide](../guides/performance.md)** - Optimize query performance
