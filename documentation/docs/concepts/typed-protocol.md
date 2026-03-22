# Typed Protocol

ORMDB uses a typed query protocol instead of SQL strings. Queries are structured data that the database validates at compile time, preventing injection attacks and enabling better optimization.

---

## Why Not SQL?

SQL is a powerful language, but using it from application code has drawbacks:

### String Interpolation Risks

```python
# Dangerous: SQL injection vulnerability
query = f"SELECT * FROM users WHERE name = '{user_input}'"
```

### No Compile-Time Validation

```python
# Typo won't be caught until runtime
query = "SELECT * FROM usres WHERE naem = ?"  # 'usres'? 'naem'?
```

### Lost Type Information

```python
# Is age an int or string? SQL doesn't know until execution
cursor.execute("INSERT INTO users (name, age) VALUES (?, ?)", [name, age])
```

---

## ORMDB's Approach

Queries in ORMDB are structured data types:

```rust
pub struct GraphQuery {
    pub root_entity: String,
    pub fields: Vec<String>,
    pub filter: Option<FilterSpec>,
    pub pagination: Option<Pagination>,
    pub order_by: Vec<OrderSpec>,
    pub includes: Vec<RelationInclude>,
}
```

### Benefits

1. **No injection** - Values are parameters, not interpolated strings
2. **Schema validation** - Invalid queries rejected before execution
3. **Type checking** - Filter values validated against field types
4. **Optimization hints** - Structure tells the planner what you need

---

## Building Queries

### GraphQuery

The main query type for reading data:

=== "Rust"

    ```rust
    use ormdb_proto::{GraphQuery, FilterExpr, Value, Pagination, OrderSpec};

    let query = GraphQuery::new("User")
        .with_fields(vec!["id", "name", "email"])
        .with_filter(FilterExpr::eq("status", Value::String("active".into())))
        .with_pagination(Pagination::new(20, 0))
        .with_order(OrderSpec::asc("name"));
    ```

=== "TypeScript"

    ```typescript
    const query = {
      entity: "User",
      fields: ["id", "name", "email"],
      filter: { field: "status", op: "eq", value: "active" },
      limit: 20,
      offset: 0,
      orderBy: [{ field: "name", direction: "asc" }],
    };
    ```

=== "Python"

    ```python
    query = {
        "entity": "User",
        "fields": ["id", "name", "email"],
        "filter": {"field": "status", "op": "eq", "value": "active"},
        "limit": 20,
        "offset": 0,
        "order_by": [{"field": "name", "direction": "asc"}],
    }
    ```

---

## Filter Expressions

Filters are composable expression trees, not string fragments.

### Comparison Operators

```rust
// Equality
FilterExpr::eq("status", Value::String("active".into()))

// Not equal
FilterExpr::ne("role", Value::String("admin".into()))

// Greater than
FilterExpr::gt("age", Value::Int32(18))

// Greater than or equal
FilterExpr::gte("score", Value::Float64(90.0))

// Less than
FilterExpr::lt("price", Value::Float64(100.0))

// Less than or equal
FilterExpr::lte("quantity", Value::Int32(10))
```

### Pattern Matching

```rust
// LIKE pattern (% for wildcard)
FilterExpr::like("email", "%@example.com")

// NOT LIKE
FilterExpr::not_like("name", "Test%")
```

### Null Checks

```rust
// IS NULL
FilterExpr::null("deleted_at")

// IS NOT NULL
FilterExpr::not_null("verified_at")
```

### Set Membership

```rust
// IN list
FilterExpr::in_list(
    "status",
    vec![
        Value::String("active".into()),
        Value::String("pending".into()),
    ]
)

// NOT IN list
FilterExpr::not_in_list(
    "role",
    vec![Value::String("banned".into())]
)
```

### Logical Operators

```rust
// AND
FilterExpr::and(
    FilterExpr::eq("status", Value::String("active".into())),
    FilterExpr::gt("age", Value::Int32(18))
)

// OR
FilterExpr::or(
    FilterExpr::eq("role", Value::String("admin".into())),
    FilterExpr::eq("role", Value::String("moderator".into()))
)

// NOT
FilterExpr::not(
    FilterExpr::eq("banned", Value::Bool(true))
)
```

### Complex Expressions

```rust
// (status = 'active' AND age >= 18) OR role = 'admin'
FilterExpr::or(
    FilterExpr::and(
        FilterExpr::eq("status", Value::String("active".into())),
        FilterExpr::gte("age", Value::Int32(18))
    ),
    FilterExpr::eq("role", Value::String("admin".into()))
)
```

---

## Value Types

The `Value` enum represents all supported data types:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    String(String),
    Bytes(Vec<u8>),
    Uuid([u8; 16]),
    Timestamp(i64),  // Unix timestamp in microseconds
    Json(serde_json::Value),
    Array(Vec<Value>),
}
```

### Type Coercion

ORMDB validates filter values against field types:

```rust
// Schema: age is Int32
FilterExpr::eq("age", Value::String("25".into()))  // Error: type mismatch

FilterExpr::eq("age", Value::Int32(25))  // OK
```

### Null Handling

Null values require explicit null checks:

```rust
// This won't match rows where email is NULL
FilterExpr::eq("email", Value::String("test@example.com".into()))

// To find NULL values
FilterExpr::null("email")

// To find non-NULL values
FilterExpr::not_null("email")
```

---

## Mutations

Write operations are also structured:

### Insert

```rust
let mutation = Mutation::insert("User")
    .with_field("name", Value::String("Alice".into()))
    .with_field("email", Value::String("alice@example.com".into()))
    .with_field("age", Value::Int32(25));

let result = client.mutate(mutation).await?;
let user_id = result.inserted_id();
```

### Update

```rust
let mutation = Mutation::update("User")
    .with_filter(FilterExpr::eq("id", Value::Uuid(user_id)))
    .set("name", Value::String("Alice Smith".into()))
    .set("updated_at", Value::Timestamp(now()));

client.mutate(mutation).await?;
```

### Delete

```rust
let mutation = Mutation::delete("User")
    .with_filter(FilterExpr::eq("id", Value::Uuid(user_id)));

client.mutate(mutation).await?;
```

### Batch Mutations

```rust
let batch = MutationBatch::new()
    .add(Mutation::insert("User").with_field("name", Value::String("Alice".into())))
    .add(Mutation::insert("User").with_field("name", Value::String("Bob".into())))
    .add(Mutation::insert("User").with_field("name", Value::String("Charlie".into())));

let result = client.mutate_batch(batch).await?;
```

---

## Protocol Wire Format

Queries are serialized using a compact binary format (rkyv) for efficiency:

```
┌──────────────────────────────────────────┐
│ Header (8 bytes)                         │
│ - Magic number (4 bytes)                 │
│ - Version (2 bytes)                      │
│ - Message type (2 bytes)                 │
├──────────────────────────────────────────┤
│ Body (variable)                          │
│ - rkyv-serialized query/mutation         │
└──────────────────────────────────────────┘
```

### Benefits of Binary Protocol

| Aspect | JSON | rkyv Binary |
|--------|------|-------------|
| Parse time | ~100µs | ~1µs |
| Size | ~500 bytes | ~200 bytes |
| Type safety | Runtime | Compile-time |

---

## Schema Validation

Queries are validated against the catalog before execution:

### Entity Validation

```rust
// Error: Unknown entity 'Usre'
GraphQuery::new("Usre")  // Typo!
```

### Field Validation

```rust
// Error: Unknown field 'naem' on entity 'User'
GraphQuery::new("User")
    .with_fields(vec!["id", "naem"])  // Typo!
```

### Relation Validation

```rust
// Error: Unknown relation 'post' on entity 'User'
GraphQuery::new("User")
    .include("post")  // Should be 'posts'
```

### Type Validation

```rust
// Schema: created_at is Timestamp
// Error: Cannot compare Timestamp with String
FilterExpr::eq("created_at", Value::String("2024-01-01".into()))
```

---

## Query Fingerprinting

ORMDB caches query plans using fingerprints - a hash of the query structure without parameter values:

```rust
// These queries have the same fingerprint
GraphQuery::new("User").with_filter(FilterExpr::eq("id", Value::Uuid(id1)))
GraphQuery::new("User").with_filter(FilterExpr::eq("id", Value::Uuid(id2)))

// This query has a different fingerprint
GraphQuery::new("User").with_filter(FilterExpr::eq("email", Value::String(email)))
```

This enables plan caching while still allowing different parameter values.

---

## Comparison with SQL

| Feature | SQL | ORMDB Protocol |
|---------|-----|----------------|
| Injection risk | High | None |
| Compile-time validation | No | Yes |
| Schema awareness | Limited | Full |
| Parameter typing | Weak | Strong |
| Plan caching | Query-based | Structure-based |
| Serialization | Text | Binary |

---

## Best Practices

### 1. Use Typed Values

```rust
// Good: Explicit types
FilterExpr::eq("age", Value::Int32(25))

// Avoid: String representations of numbers
FilterExpr::eq("age", Value::String("25".into()))
```

### 2. Build Filters Programmatically

```rust
// Good: Composable filter building
fn build_user_filter(status: Option<&str>, min_age: Option<i32>) -> Option<FilterExpr> {
    let mut filters = Vec::new();

    if let Some(s) = status {
        filters.push(FilterExpr::eq("status", Value::String(s.into())));
    }
    if let Some(age) = min_age {
        filters.push(FilterExpr::gte("age", Value::Int32(age)));
    }

    match filters.len() {
        0 => None,
        1 => Some(filters.remove(0)),
        _ => Some(FilterExpr::and_all(filters)),
    }
}
```

### 3. Validate Early

Client libraries validate queries before sending:

```rust
// Validation happens here, not at the database
let query = GraphQuery::new("User")
    .with_fields(vec!["nonexistent"])?;  // Returns error immediately
```

---

## Next Steps

- **[Graph Queries](graph-queries.md)** - Query structure and includes
- **[Value Types Reference](../reference/value-types.md)** - Complete type documentation
- **[Query API Reference](../reference/query-api.md)** - Full API documentation
