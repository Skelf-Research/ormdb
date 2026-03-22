# ORMDB vs SQLite

This guide compares ORMDB with SQLite to help you choose the right database for your use case.

---

## Overview

| Aspect | ORMDB | SQLite |
|--------|-------|--------|
| **Type** | ORM-first database | General-purpose SQL |
| **Query Language** | Typed protocol | SQL |
| **Primary Use Case** | Application backends | Embedded, general purpose |
| **N+1 Problem** | Solved by design | Manual optimization needed |
| **Migrations** | Safety-graded | Manual scripts |
| **Versioning** | Built-in MVCC | WAL mode |

---

## When to Choose ORMDB

**Choose ORMDB when:**

- Your application uses an ORM or object-oriented data access
- N+1 queries are a recurring problem
- You want type-safe queries validated at compile time
- Schema migrations cause anxiety
- You need built-in versioning and audit trails
- Row-level security is important

**Example: E-commerce API**
```rust
// Single request loads order with all related data
let query = GraphQuery::new("Order")
    .with_filter(FilterExpr::eq("id", order_id))
    .include("items")
    .include("items.product")
    .include("customer")
    .include("shipping_address");
```

---

## When to Choose SQLite

**Choose SQLite when:**

- You need ad-hoc SQL queries and reporting
- Your team has strong SQL expertise
- You need maximum compatibility with existing tools
- The application is read-heavy with simple queries
- You're building a desktop or mobile app

**Example: Analytics Dashboard**
```sql
-- Complex aggregation easier in SQL
SELECT
    DATE(created_at) as date,
    COUNT(*) as orders,
    SUM(total) as revenue
FROM orders
WHERE created_at > DATE('now', '-30 days')
GROUP BY DATE(created_at)
ORDER BY date;
```

---

## N+1 Query Comparison

The N+1 problem is one of the biggest performance issues in ORM-based applications.

### The Problem

Loading 100 users with their posts:

**SQLite with typical ORM:**
```python
users = User.all()           # 1 query
for user in users:
    print(user.posts)        # 100 queries
# Total: 101 queries
```

**SQLite with manual batching:**
```python
users = User.all()                           # 1 query
posts = Post.where(user_id=user_ids)         # 1 query
# Manually associate in code                 # More complex code
# Total: 2 queries
```

**SQLite with JOIN:**
```sql
SELECT users.*, posts.*
FROM users
LEFT JOIN posts ON posts.user_id = users.id;
-- 1 query, but denormalized results to process
```

**ORMDB:**
```rust
let query = GraphQuery::new("User")
    .include("posts");
// 1 request, structured result
```

### Benchmark Results

Loading 100 users with ~5 posts each (500 total posts):

| Approach | Queries | Time |
|----------|---------|------|
| SQLite N+1 | 101 | 45ms |
| SQLite batched | 2 | 12ms |
| SQLite JOIN | 1 | 8ms |
| ORMDB graph query | 1 | 7ms |

### Nested Relations

Loading users → posts → comments (100 users, 500 posts, 2000 comments):

| Approach | Queries | Time |
|----------|---------|------|
| SQLite N+1 | 2,601 | 890ms |
| SQLite batched | 3 | 35ms |
| SQLite JOIN | 1 | 25ms |
| ORMDB graph query | 1 | 28ms |

ORMDB matches JOIN performance without the complexity of writing and maintaining JOIN queries.

---

## Type Safety Comparison

### SQLite

```python
# Typo won't be caught until runtime
cursor.execute("SELECT * FROM usres WHERE naem = ?", [name])
# "usres"? "naem"? Discovered in production.

# Type mismatch is a runtime error
cursor.execute("INSERT INTO users (age) VALUES (?)", ["twenty-five"])
# Fails at execution time
```

### ORMDB

```rust
// Typo caught at planning time
let query = GraphQuery::new("Usres");  // Error: Unknown entity 'Usres'

// Type validated before execution
FilterExpr::eq("age", Value::String("twenty-five"))  // Error: type mismatch
```

---

## Migration Comparison

### SQLite

```sql
-- migration_001.sql
ALTER TABLE users ADD COLUMN status TEXT;

-- Hope existing NULLs don't break anything
-- Hope clients are updated before deployment
-- Hope we can rollback if needed
```

**Issues:**
- No validation of safety
- Manual coordination of client updates
- Rollback often requires separate scripts
- Easy to accidentally lose data

### ORMDB

```rust
// Define new schema
let new_schema = current_schema.with_change(|user| {
    user.add_field(FieldDef::optional("status", ScalarType::String))
});

// Grade the migration
let grade = SafetyGrader::grade(&diff);
// Grade: A - Non-breaking, online

// Apply with confidence
catalog.apply_schema(new_schema)?;
```

**Benefits:**
- Explicit safety grades (A/B/C/D)
- Blocking changes require confirmation
- Automatic backfill for defaults
- Built-in rollback capabilities

---

## Feature Comparison

### Query Features

| Feature | ORMDB | SQLite |
|---------|-------|--------|
| Graph fetches | Native | Manual JOINs |
| Filter expressions | Typed combinators | SQL strings |
| Pagination | Built-in cursors | LIMIT/OFFSET |
| Sorting | OrderSpec | ORDER BY |
| Aggregations | COUNT, SUM, AVG, etc. | Full SQL |
| Window functions | Not yet | Yes |
| CTEs | Not yet | Yes |
| Subqueries | Via includes | Yes |

### Storage Features

| Feature | ORMDB | SQLite |
|---------|-------|--------|
| MVCC versioning | Built-in | WAL mode |
| Point-in-time queries | Native | Manual |
| Soft deletes | Default behavior | Manual |
| Secondary indexes | Hash + B-tree | B-tree |
| Columnar storage | Hybrid | Row-only |
| Compression | Dictionary encoding | None default |

### Security Features

| Feature | ORMDB | SQLite |
|---------|-------|--------|
| Row-level security | Built-in policies | Application code |
| Field masking | Native | Manual |
| Audit logging | Built-in | Triggers |
| Query budgets | Enforced limits | No limits |

### Operations

| Feature | ORMDB | SQLite |
|---------|-------|--------|
| Backup | Built-in | sqlite3 .backup |
| Replication | CDC-based | File sync |
| Metrics | Prometheus export | None |
| Query explain | Native | EXPLAIN |

---

## Code Comparison

### Simple Query

**SQLite:**
```python
cursor.execute("""
    SELECT id, name, email
    FROM users
    WHERE status = ?
    ORDER BY name
    LIMIT 20
""", ['active'])
users = cursor.fetchall()
```

**ORMDB:**
```rust
let query = GraphQuery::new("User")
    .with_fields(vec!["id", "name", "email"])
    .with_filter(FilterExpr::eq("status", Value::String("active".into())))
    .with_order(OrderSpec::asc("name"))
    .with_pagination(Pagination::new(20, 0));

let result = client.query(query).await?;
```

### Related Data

**SQLite:**
```python
# Fetch users
cursor.execute("SELECT * FROM users WHERE status = ?", ['active'])
users = cursor.fetchall()

# Fetch posts for each user (N+1!)
for user in users:
    cursor.execute("SELECT * FROM posts WHERE author_id = ?", [user['id']])
    user['posts'] = cursor.fetchall()

# Or use JOIN (flattened results)
cursor.execute("""
    SELECT u.*, p.*
    FROM users u
    LEFT JOIN posts p ON p.author_id = u.id
    WHERE u.status = ?
""", ['active'])
```

**ORMDB:**
```rust
let query = GraphQuery::new("User")
    .with_filter(FilterExpr::eq("status", Value::String("active".into())))
    .include("posts");

let result = client.query(query).await?;

for user in result.entities("User") {
    for post in result.related(&user, "posts") {
        // Structured access to related data
    }
}
```

### Mutations

**SQLite:**
```python
cursor.execute("""
    INSERT INTO users (id, name, email, created_at)
    VALUES (?, ?, ?, ?)
""", [uuid4(), name, email, datetime.now()])
conn.commit()
```

**ORMDB:**
```rust
let mutation = Mutation::insert("User")
    .with_field("name", Value::String(name))
    .with_field("email", Value::String(email));
    // id and created_at auto-generated

let result = client.mutate(mutation).await?;
```

---

## Performance Characteristics

### Insert Performance

| Operation | ORMDB | SQLite |
|-----------|-------|--------|
| Single insert | ~50µs | ~30µs |
| Batch 100 | ~2ms | ~1.5ms |
| Batch 1000 | ~15ms | ~10ms |

SQLite is slightly faster for raw inserts due to simpler storage.

### Query Performance

| Operation | ORMDB | SQLite |
|-----------|-------|--------|
| Point lookup | ~10µs | ~5µs |
| Index scan (1K rows) | ~500µs | ~400µs |
| Full scan (10K rows) | ~50ms | ~40ms |
| N+1 (100 parents) | ~7ms | ~45ms |

ORMDB has slight overhead for simple queries but excels at relational queries.

### Memory Usage

| Aspect | ORMDB | SQLite |
|--------|-------|--------|
| Base memory | ~50MB | ~5MB |
| Per-connection | ~1MB | ~50KB |
| Cache | Configurable | Page cache |

ORMDB uses more memory for its hybrid storage and caching.

---

## Migration Path

### From SQLite to ORMDB

1. **Export schema**: Generate ORMDB schema from SQLite tables
2. **Migrate data**: Export to JSON, import to ORMDB
3. **Update queries**: Replace SQL with graph queries
4. **Test thoroughly**: Verify data integrity and performance

### From ORMDB to SQLite

1. **Export schema**: Generate CREATE TABLE from catalog
2. **Export data**: Use entity scan to dump records
3. **Flatten relations**: Convert to foreign keys
4. **Lose features**: MVCC history, RLS, etc. not portable

---

## Hybrid Approach

You can use both databases for different parts of your application:

```
┌─────────────────────────────────────────┐
│              Application                 │
├─────────────────────┬───────────────────┤
│     ORMDB           │      SQLite       │
│   - User data       │   - Analytics     │
│   - Orders          │   - Reports       │
│   - Real-time ops   │   - Ad-hoc SQL    │
└─────────────────────┴───────────────────┘
```

---

## Summary

| Choose ORMDB | Choose SQLite |
|--------------|---------------|
| ORM-centric apps | SQL-centric apps |
| Complex relations | Simple queries |
| Type safety critical | Flexibility needed |
| N+1 is a pain | Queries are simple |
| Need audit trail | Space constrained |
| Schema safety | Maximum compatibility |

---

## Next Steps

- **[Getting Started](../getting-started/quickstart.md)** - Try ORMDB
- **[vs Traditional ORMs](vs-traditional-orms.md)** - Compare with ORM patterns
- **[Performance Guide](../guides/performance.md)** - Optimize your queries
