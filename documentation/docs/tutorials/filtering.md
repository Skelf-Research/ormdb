# Filtering

Learn advanced filtering techniques with AND, OR, NOT, and complex expressions.

## Filter Operators

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equals | `age = 25` |
| `ne` | Not equals | `status != "deleted"` |
| `lt` | Less than | `price < 100` |
| `le` | Less than or equal | `price <= 100` |
| `gt` | Greater than | `age > 18` |
| `ge` | Greater than or equal | `age >= 18` |
| `like` | Pattern match (case sensitive) | `name LIKE "A%"` |
| `ilike` | Pattern match (case insensitive) | `name ILIKE "a%"` |
| `in` | In list | `status IN ["active", "pending"]` |
| `not_in` | Not in list | `status NOT IN ["deleted"]` |
| `is_null` | Is null | `deleted_at IS NULL` |
| `is_not_null` | Is not null | `email IS NOT NULL` |

## Logical Operators

### AND

All conditions must be true:

=== "Rust"

    ```rust
    use ormdb_proto::FilterExpr;

    // Active users over 18
    let filter = FilterExpr::and(vec![
        FilterExpr::eq("status", Value::String("active".into())),
        FilterExpr::gt("age", Value::Int32(18)),
    ]);

    let query = GraphQuery::new("User").with_filter(filter);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: {
        and: [
          { field: "status", op: "eq", value: "active" },
          { field: "age", op: "gt", value: 18 },
        ],
      },
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        filter={
            "and": [
                {"field": "status", "op": "eq", "value": "active"},
                {"field": "age", "op": "gt", "value": 18},
            ]
        })
    ```

### OR

At least one condition must be true:

=== "Rust"

    ```rust
    // Active or pending users
    let filter = FilterExpr::or(vec![
        FilterExpr::eq("status", Value::String("active".into())),
        FilterExpr::eq("status", Value::String("pending".into())),
    ]);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: {
        or: [
          { field: "status", op: "eq", value: "active" },
          { field: "status", op: "eq", value: "pending" },
        ],
      },
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        filter={
            "or": [
                {"field": "status", "op": "eq", "value": "active"},
                {"field": "status", "op": "eq", "value": "pending"},
            ]
        })
    ```

### NOT

Negate a condition:

=== "Rust"

    ```rust
    // Users who are NOT deleted
    let filter = FilterExpr::not(Box::new(
        FilterExpr::eq("status", Value::String("deleted".into()))
    ));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: {
        not: { field: "status", op: "eq", value: "deleted" },
      },
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        filter={
            "not": {"field": "status", "op": "eq", "value": "deleted"}
        })
    ```

## Complex Expressions

### Combining AND and OR

```sql
-- Users who are (active AND over 18) OR (verified AND any age)
(status = 'active' AND age > 18) OR (verified = true)
```

=== "Rust"

    ```rust
    let filter = FilterExpr::or(vec![
        FilterExpr::and(vec![
            FilterExpr::eq("status", Value::String("active".into())),
            FilterExpr::gt("age", Value::Int32(18)),
        ]),
        FilterExpr::eq("verified", Value::Bool(true)),
    ]);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: {
        or: [
          {
            and: [
              { field: "status", op: "eq", value: "active" },
              { field: "age", op: "gt", value: 18 },
            ],
          },
          { field: "verified", op: "eq", value: true },
        ],
      },
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        filter={
            "or": [
                {
                    "and": [
                        {"field": "status", "op": "eq", "value": "active"},
                        {"field": "age", "op": "gt", "value": 18},
                    ]
                },
                {"field": "verified", "op": "eq", "value": True},
            ]
        })
    ```

### Range Queries

```sql
-- Users between 18 and 30 years old
age >= 18 AND age <= 30
```

=== "Rust"

    ```rust
    let filter = FilterExpr::and(vec![
        FilterExpr::ge("age", Value::Int32(18)),
        FilterExpr::le("age", Value::Int32(30)),
    ]);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: {
        and: [
          { field: "age", op: "ge", value: 18 },
          { field: "age", op: "le", value: 30 },
        ],
      },
    });
    ```

## Pattern Matching

### LIKE Patterns

| Pattern | Matches |
|---------|---------|
| `A%` | Starts with "A" |
| `%a` | Ends with "a" |
| `%alice%` | Contains "alice" |
| `A_ice` | "A" + one char + "ice" |

=== "Rust"

    ```rust
    // Users with email from example.com
    let filter = FilterExpr::like("email", "%@example.com");

    // Case-insensitive name search
    let filter = FilterExpr::ilike("name", "%alice%");
    ```

=== "TypeScript"

    ```typescript
    // Users with email from example.com
    const result = await client.query("User", {
      filter: { field: "email", op: "like", value: "%@example.com" },
    });

    // Case-insensitive name search
    const result = await client.query("User", {
      filter: { field: "name", op: "ilike", value: "%alice%" },
    });
    ```

## IN Operator

Check if a value is in a list:

=== "Rust"

    ```rust
    // Users with specific IDs
    let filter = FilterExpr::in_values("id", vec![
        Value::Uuid(id1),
        Value::Uuid(id2),
        Value::Uuid(id3),
    ]);

    // Users with specific statuses
    let filter = FilterExpr::in_values("status", vec![
        Value::String("active".into()),
        Value::String("pending".into()),
    ]);
    ```

=== "TypeScript"

    ```typescript
    // Users with specific IDs
    const result = await client.query("User", {
      filter: { field: "id", op: "in", value: [id1, id2, id3] },
    });

    // Users with specific statuses
    const result = await client.query("User", {
      filter: { field: "status", op: "in", value: ["active", "pending"] },
    });
    ```

## NULL Handling

=== "Rust"

    ```rust
    // Users without a profile picture
    let filter = FilterExpr::is_null("avatar_url");

    // Users with a profile picture
    let filter = FilterExpr::is_not_null("avatar_url");
    ```

=== "TypeScript"

    ```typescript
    // Users without a profile picture
    const result = await client.query("User", {
      filter: { field: "avatar_url", op: "is_null" },
    });

    // Users with a profile picture
    const result = await client.query("User", {
      filter: { field: "avatar_url", op: "is_not_null" },
    });
    ```

## Filtering Relations

Filter included relations:

=== "Rust"

    ```rust
    // Users with only published posts
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("posts")
            .with_filter(FilterExpr::eq("published", Value::Bool(true))));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [
        {
          relation: "posts",
          filter: { field: "published", op: "eq", value: true },
        },
      ],
    });
    ```

## Best Practices

1. **Use indexes** - Filter on indexed fields for best performance
2. **Prefer IN over OR** - `status IN [a, b]` is faster than `status = a OR status = b`
3. **Filter early** - Apply filters to reduce data before includes
4. **Avoid LIKE with leading wildcard** - `LIKE '%text'` can't use indexes

## Next Steps

- **[Performance](../guides/performance.md)** - Optimize filter performance
- **[Querying Data](querying-data.md)** - Complete query reference
