# Querying Data

Learn how to query entities with filters, includes, ordering, and pagination.

## Basic Queries

### Select All Fields

=== "Rust"

    ```rust
    use ormdb_proto::GraphQuery;

    // Query all users
    let query = GraphQuery::new("User");
    let result = client.query(query).await?;
    ```

=== "TypeScript"

    ```typescript
    // Query all users
    const result = await client.query("User");
    console.log(result.entities);
    ```

=== "Python"

    ```python
    # Query all users
    result = client.query("User")
    print(result.entities)
    ```

### Select Specific Fields

Only load the fields you need:

=== "Rust"

    ```rust
    let query = GraphQuery::new("User")
        .with_fields(vec!["id", "name", "email"]);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      fields: ["id", "name", "email"],
    });
    ```

=== "Python"

    ```python
    result = client.query("User", fields=["id", "name", "email"])
    ```

## Filtering

### Simple Equality

=== "Rust"

    ```rust
    use ormdb_proto::{FilterExpr, Value};

    let query = GraphQuery::new("User")
        .with_filter(FilterExpr::eq("status", Value::String("active".into())));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: { field: "status", op: "eq", value: "active" },
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        filter={"field": "status", "op": "eq", "value": "active"})
    ```

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equals | `status = "active"` |
| `ne` | Not equals | `status != "deleted"` |
| `lt` | Less than | `age < 30` |
| `le` | Less than or equal | `age <= 30` |
| `gt` | Greater than | `age > 18` |
| `ge` | Greater than or equal | `age >= 18` |

=== "Rust"

    ```rust
    // Users older than 18
    FilterExpr::gt("age", Value::Int32(18))

    // Users with age between 18 and 30
    FilterExpr::and(vec![
        FilterExpr::ge("age", Value::Int32(18)),
        FilterExpr::le("age", Value::Int32(30)),
    ])
    ```

=== "TypeScript"

    ```typescript
    // Users older than 18
    { field: "age", op: "gt", value: 18 }

    // Users with age between 18 and 30
    {
      and: [
        { field: "age", op: "ge", value: 18 },
        { field: "age", op: "le", value: 30 },
      ]
    }
    ```

=== "Python"

    ```python
    # Users older than 18
    {"field": "age", "op": "gt", "value": 18}

    # Users with age between 18 and 30
    {
        "and": [
            {"field": "age", "op": "ge", "value": 18},
            {"field": "age", "op": "le", "value": 30},
        ]
    }
    ```

### Pattern Matching (LIKE)

=== "Rust"

    ```rust
    // Emails ending in @example.com
    FilterExpr::like("email", "%@example.com")

    // Case-insensitive search
    FilterExpr::ilike("name", "%alice%")
    ```

=== "TypeScript"

    ```typescript
    // Emails ending in @example.com
    { field: "email", op: "like", value: "%@example.com" }

    // Case-insensitive search
    { field: "name", op: "ilike", value: "%alice%" }
    ```

=== "Python"

    ```python
    # Emails ending in @example.com
    {"field": "email", "op": "like", "value": "%@example.com"}

    # Case-insensitive search
    {"field": "name", "op": "ilike", "value": "%alice%"}
    ```

### IN Operator

=== "Rust"

    ```rust
    // Users with specific statuses
    FilterExpr::in_values("status", vec![
        Value::String("active".into()),
        Value::String("pending".into()),
    ])
    ```

=== "TypeScript"

    ```typescript
    { field: "status", op: "in", value: ["active", "pending"] }
    ```

=== "Python"

    ```python
    {"field": "status", "op": "in", "value": ["active", "pending"]}
    ```

### NULL Checks

=== "Rust"

    ```rust
    // Users without a bio
    FilterExpr::is_null("bio")

    // Users with a bio
    FilterExpr::is_not_null("bio")
    ```

=== "TypeScript"

    ```typescript
    // Users without a bio
    { field: "bio", op: "is_null" }

    // Users with a bio
    { field: "bio", op: "is_not_null" }
    ```

=== "Python"

    ```python
    # Users without a bio
    {"field": "bio", "op": "is_null"}

    # Users with a bio
    {"field": "bio", "op": "is_not_null"}
    ```

## Including Relations

The power of ORMDB is loading related data without N+1 queries.

### Single Include

=== "Rust"

    ```rust
    use ormdb_proto::RelationInclude;

    // Load users with their posts
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("posts"));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [{ relation: "posts" }],
    });

    // Access related data
    for (const user of result.entities) {
      console.log(`${user.name} has ${user.posts?.length || 0} posts`);
    }
    ```

=== "Python"

    ```python
    result = client.query("User",
        includes=[{"relation": "posts"}])

    for user in result.entities:
        print(f"{user['name']} has {len(user.get('posts', []))} posts")
    ```

### Nested Includes

Load multiple levels of relations:

=== "Rust"

    ```rust
    // Users → Posts → Comments → Author
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("posts")
            .include(RelationInclude::new("comments")
                .include(RelationInclude::new("author"))));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [
        {
          relation: "posts",
          includes: [
            {
              relation: "comments",
              includes: [{ relation: "author" }],
            },
          ],
        },
      ],
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        includes=[{
            "relation": "posts",
            "includes": [{
                "relation": "comments",
                "includes": [{"relation": "author"}],
            }],
        }])
    ```

### Filtered Includes

Filter related entities:

=== "Rust"

    ```rust
    // Only include published posts
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

=== "Python"

    ```python
    result = client.query("User",
        includes=[{
            "relation": "posts",
            "filter": {"field": "published", "op": "eq", "value": True},
        }])
    ```

## Ordering

=== "Rust"

    ```rust
    use ormdb_proto::OrderSpec;

    // Sort by name ascending
    let query = GraphQuery::new("User")
        .with_order(OrderSpec::asc("name"));

    // Sort by created_at descending
    let query = GraphQuery::new("User")
        .with_order(OrderSpec::desc("created_at"));

    // Multiple sort keys
    let query = GraphQuery::new("User")
        .with_orders(vec![
            OrderSpec::asc("status"),
            OrderSpec::desc("created_at"),
        ]);
    ```

=== "TypeScript"

    ```typescript
    // Sort by name ascending
    const result = await client.query("User", {
      orderBy: [{ field: "name", direction: "asc" }],
    });

    // Multiple sort keys
    const result = await client.query("User", {
      orderBy: [
        { field: "status", direction: "asc" },
        { field: "created_at", direction: "desc" },
      ],
    });
    ```

=== "Python"

    ```python
    # Sort by name ascending
    result = client.query("User",
        order_by=[{"field": "name", "direction": "asc"}])

    # Multiple sort keys
    result = client.query("User",
        order_by=[
            {"field": "status", "direction": "asc"},
            {"field": "created_at", "direction": "desc"},
        ])
    ```

## Pagination

=== "Rust"

    ```rust
    use ormdb_proto::Pagination;

    // First 10 users
    let query = GraphQuery::new("User")
        .with_pagination(Pagination::new(10, 0));  // limit, offset

    // Next 10 users
    let query = GraphQuery::new("User")
        .with_pagination(Pagination::new(10, 10));
    ```

=== "TypeScript"

    ```typescript
    // First 10 users
    const result = await client.query("User", {
      limit: 10,
      offset: 0,
    });

    // Check if there are more
    if (result.hasMore) {
      // Fetch next page
    }
    ```

=== "Python"

    ```python
    # First 10 users
    result = client.query("User", limit=10, offset=0)

    # Check if there are more
    if result.has_more:
        # Fetch next page
        pass
    ```

## Query Budgets

Prevent runaway queries with budgets:

=== "Rust"

    ```rust
    use ormdb_core::query::FanoutBudget;

    let budget = FanoutBudget {
        max_entities: 1000,  // Max total entities
        max_edges: 5000,     // Max relation edges
        max_depth: 3,        // Max include nesting
    };

    let result = executor.execute_with_budget(&query, budget)?;
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [{ relation: "posts" }],
      budget: {
        maxEntities: 1000,
        maxEdges: 5000,
        maxDepth: 3,
      },
    });
    ```

=== "Python"

    ```python
    result = client.query("User",
        includes=[{"relation": "posts"}],
        budget={
            "max_entities": 1000,
            "max_edges": 5000,
            "max_depth": 3,
        })
    ```

## Next Steps

- **[Filtering](filtering.md)** - Advanced filtering with AND/OR/NOT
- **[Pagination](../guides/pagination.md)** - Pagination best practices
- **[Performance](../guides/performance.md)** - Query optimization tips
