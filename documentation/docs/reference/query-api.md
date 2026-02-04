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
