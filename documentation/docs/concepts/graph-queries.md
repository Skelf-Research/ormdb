# Graph Queries

Graph queries are ORMDB's solution to the N+1 problem. Instead of issuing separate queries for related data, you specify what relations to include and ORMDB fetches everything in a single request.

---

## The N+1 Problem

The N+1 problem is one of the most common performance issues in applications using ORMs. Consider this scenario:

```python
# Traditional ORM approach
users = User.all()  # 1 query
for user in users:
    print(user.posts)  # N queries (one per user)
```

If you have 100 users, this generates 101 database queries. With nested relations, it gets exponentially worse.

### ORMDB Solution

```rust
let query = GraphQuery::new("User")
    .include("posts");  // All users + all their posts in ONE query

let result = client.query(query).await?;
```

ORMDB loads all users and all related posts in a single round trip. The database handles the join internally, eliminating network overhead.

---

## How Graph Queries Work

### Basic Structure

A `GraphQuery` specifies:

1. **Root entity** - The starting point (e.g., "User")
2. **Fields** - Which fields to return (optional, defaults to all)
3. **Filter** - Conditions for root entities
4. **Includes** - Related entities to fetch
5. **Pagination** - Limit and offset

```rust
GraphQuery::new("User")
    .with_fields(vec!["id", "name", "email"])
    .with_filter(FilterExpr::eq("status", Value::String("active")))
    .include(RelationInclude::new("posts")
        .with_fields(vec!["id", "title"]))
    .with_pagination(Pagination::new(10, 0))
```

### Nested Includes

You can include relations of relations using dot notation:

```rust
GraphQuery::new("User")
    .include("posts")              // User's posts
    .include("posts.comments")     // Posts' comments
    .include("posts.comments.author")  // Comments' authors
```

This creates a tree of related data:

```
User
├── Post[]
│   ├── Comment[]
│   │   └── User (author)
```

### Include Options

Each include can have its own filter, pagination, and field selection:

```rust
RelationInclude::new("posts")
    .with_fields(vec!["id", "title", "published_at"])
    .with_filter(FilterExpr::eq("published", Value::Bool(true)))
    .with_order(OrderSpec::desc("published_at"))
    .with_pagination(Pagination::new(5, 0))  // Latest 5 posts per user
```

---

## Query Planning

When you submit a graph query, ORMDB's query planner:

1. **Resolves schema** - Validates entities and relations exist
2. **Validates fields** - Ensures requested fields are defined
3. **Checks depth** - Enforces maximum include depth
4. **Optimizes order** - Reorders includes by estimated fanout
5. **Enforces budget** - Ensures query won't exceed limits

### Query Plan Structure

```rust
pub struct QueryPlan {
    pub root_entity: String,
    pub fields: Vec<String>,
    pub filter: Option<FilterExpr>,
    pub order_by: Vec<OrderSpec>,
    pub pagination: Option<Pagination>,
    pub includes: Vec<IncludePlan>,
    pub budget: FanoutBudget,
}
```

---

## Fanout Budgets

Fanout budgets prevent runaway queries that could overwhelm the database or return massive result sets.

### Default Limits

```rust
FanoutBudget {
    max_entities: 10_000,  // Total entities across all blocks
    max_edges: 50_000,     // Total relationships traversed
    max_depth: 5,          // Maximum include nesting
}
```

### Custom Budgets

For specific queries, you can set custom limits:

```rust
let budget = FanoutBudget::new(
    100,   // max_entities
    500,   // max_edges
    3,     // max_depth
);

let plan = planner.plan_with_budget(&query, budget)?;
```

### Budget Enforcement

If a query would exceed its budget, ORMDB returns an error:

```
Error: Query depth 6 exceeds maximum allowed depth 5
```

This happens at planning time, not execution time, so you get fast feedback.

### Fanout Estimation

ORMDB estimates fanout based on relation cardinality:

| Cardinality | Estimated Fanout |
|-------------|------------------|
| OneToOne    | 1                |
| OneToMany   | 10               |
| ManyToMany  | 25               |

The planner uses these estimates to optimize include order and check budgets.

---

## Query Execution

### Execution Flow

1. **Root fetch** - Load root entities matching filter
2. **Include batching** - For each include, batch-fetch related entities
3. **Edge linking** - Build the entity-to-entity relationships
4. **Result assembly** - Return entities + edges as structured result

### Join Strategies

For each include, the executor chooses between:

**Nested Loop Join** - Simple O(N*M), efficient for small parent sets

```rust
for parent_id in parent_ids {
    let children = fetch_children(parent_id)?;
    results.extend(children);
}
```

**Hash Join** - O(N+M), efficient for larger datasets

```rust
// Build hash map of child_id -> child
let children: HashMap<_, _> = fetch_all_children();

// Probe for each parent
for parent in parents {
    let related = children.get(&parent.id);
}
```

The executor automatically selects the best strategy based on estimated result sizes.

---

## Result Structure

Query results contain two types of blocks:

### Entity Blocks

Collections of entities grouped by type:

```rust
struct EntityBlock {
    entity_type: String,
    entities: Vec<Entity>,
}
```

### Edge Blocks

Relationships between entities:

```rust
struct EdgeBlock {
    relation: String,
    edges: Vec<(EntityId, EntityId)>,  // (from, to)
}
```

### Navigating Results

```rust
let result = client.query(query).await?;

// Get all users
for user in result.entities("User") {
    println!("User: {}", user.get_string("name")?);

    // Get related posts for this user
    for post in result.related(&user, "posts") {
        println!("  Post: {}", post.get_string("title")?);
    }
}
```

---

## Performance Comparison

### Benchmark: Users with Posts

Loading 100 users with their posts (avg 5 posts per user):

| Approach | Queries | Time |
|----------|---------|------|
| N+1 (separate queries) | 101 | 45ms |
| SQL JOIN | 1 | 8ms |
| ORMDB Graph Query | 1 | 7ms |

### Benchmark: Nested Relations

Loading users with posts and comments (100 users, 500 posts, 2000 comments):

| Approach | Queries | Time |
|----------|---------|------|
| N+1 | 2,601 | 890ms |
| Manual batching | 3 | 35ms |
| ORMDB Graph Query | 1 | 28ms |

ORMDB's graph queries match or exceed manually optimized SQL while providing a simpler API.

---

## Best Practices

### 1. Select Only Needed Fields

```rust
// Good: Only fetch what you need
GraphQuery::new("User")
    .with_fields(vec!["id", "name"])
    .include(RelationInclude::new("posts")
        .with_fields(vec!["id", "title"]))

// Avoid: Fetching all fields when you only need a few
GraphQuery::new("User")
    .include("posts")
```

### 2. Use Pagination for Large Result Sets

```rust
// Good: Paginate root entities
GraphQuery::new("User")
    .with_pagination(Pagination::new(20, 0))
    .include("posts")

// Good: Paginate includes too
GraphQuery::new("User")
    .include(RelationInclude::new("posts")
        .with_pagination(Pagination::new(10, 0)))
```

### 3. Filter at the Deepest Level

```rust
// Good: Filter where data lives
GraphQuery::new("User")
    .include(RelationInclude::new("posts")
        .with_filter(FilterExpr::eq("published", Value::Bool(true))))

// Avoid: Loading everything then filtering in application
```

### 4. Mind the Depth

Deeply nested includes can still cause performance issues:

```rust
// Be cautious with deep nesting
GraphQuery::new("User")
    .include("posts")
    .include("posts.comments")
    .include("posts.comments.author")
    .include("posts.comments.author.profile")  // Getting deep...
```

Consider whether you really need all that data in one query.

---

## Common Patterns

### Eager Loading for Display

Load all data needed for a page in one query:

```rust
// Dashboard: User profile with recent activity
let query = GraphQuery::new("User")
    .with_filter(FilterExpr::eq("id", Value::Uuid(user_id)))
    .include(RelationInclude::new("posts")
        .with_order(OrderSpec::desc("created_at"))
        .with_pagination(Pagination::new(5, 0)))
    .include(RelationInclude::new("notifications")
        .with_filter(FilterExpr::eq("read", Value::Bool(false)))
        .with_pagination(Pagination::new(10, 0)));
```

### List with Preview

Load list items with a preview of related data:

```rust
// Blog post list with author info
let query = GraphQuery::new("Post")
    .with_fields(vec!["id", "title", "excerpt", "published_at"])
    .with_filter(FilterExpr::eq("published", Value::Bool(true)))
    .with_order(OrderSpec::desc("published_at"))
    .with_pagination(Pagination::new(20, 0))
    .include(RelationInclude::new("author")
        .with_fields(vec!["id", "name", "avatar_url"]));
```

### Recursive Relationships

For self-referential relations (like comments with replies):

```rust
// Comments with one level of replies
let query = GraphQuery::new("Comment")
    .with_filter(FilterExpr::null("parent_id"))  // Top-level comments
    .include(RelationInclude::new("replies")
        .with_pagination(Pagination::new(3, 0)));  // First 3 replies
```

---

## Next Steps

- **[Typed Protocol](typed-protocol.md)** - How queries are structured
- **[Performance Guide](../guides/performance.md)** - Optimization tips
- **[Filtering](../tutorials/filtering.md)** - Filter expression reference
