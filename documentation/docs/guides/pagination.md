# Pagination Guide

Efficient pagination patterns for large datasets in ORMDB.

## Overview

ORMDB supports multiple pagination strategies:

1. **Offset Pagination** - Traditional skip/take approach
2. **Cursor Pagination** - Efficient for large datasets
3. **Keyset Pagination** - Best for real-time data

## Offset Pagination

Simple and intuitive, best for small to medium datasets.

### Basic Usage

=== "Rust"

    ```rust
    let query = GraphQuery::new("Post")
        .with_pagination(Pagination::new(10, 0))  // limit, offset
        .with_order(OrderSpec::desc("created_at"));

    let result = client.query(query).await?;

    // Check for more results
    if result.has_more {
        // Fetch next page
        let next_query = GraphQuery::new("Post")
            .with_pagination(Pagination::new(10, 10))
            .with_order(OrderSpec::desc("created_at"));
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("Post", {
      limit: 10,
      offset: 0,
      orderBy: [{ field: "created_at", direction: "desc" }],
    });

    if (result.hasMore) {
      const nextPage = await client.query("Post", {
        limit: 10,
        offset: 10,
        orderBy: [{ field: "created_at", direction: "desc" }],
      });
    }
    ```

=== "Python"

    ```python
    result = client.query("Post",
        limit=10,
        offset=0,
        order_by=[{"field": "created_at", "direction": "desc"}])

    if result.has_more:
        next_page = client.query("Post",
            limit=10,
            offset=10,
            order_by=[{"field": "created_at", "direction": "desc"}])
    ```

### Pagination Helper

=== "TypeScript"

    ```typescript
    async function* paginate<T>(
      entity: string,
      options: QueryOptions,
      pageSize: number = 100
    ): AsyncGenerator<T[]> {
      let offset = 0;
      while (true) {
        const result = await client.query<T>(entity, {
          ...options,
          limit: pageSize,
          offset,
        });

        yield result.entities;

        if (!result.hasMore) break;
        offset += pageSize;
      }
    }

    // Usage
    for await (const posts of paginate<Post>("Post", { orderBy: [{ field: "created_at", direction: "desc" }] })) {
      for (const post of posts) {
        await processPost(post);
      }
    }
    ```

=== "Python"

    ```python
    def paginate(entity: str, options: dict, page_size: int = 100):
        offset = 0
        while True:
            result = client.query(entity, **options, limit=page_size, offset=offset)
            yield result.entities

            if not result.has_more:
                break
            offset += page_size

    # Usage
    for posts in paginate("Post", {"order_by": [{"field": "created_at", "direction": "desc"}]}):
        for post in posts:
            process_post(post)
    ```

### Limitations

| Dataset Size | Performance | Recommendation |
|--------------|-------------|----------------|
| < 10,000 rows | Good | Offset pagination is fine |
| 10,000 - 100,000 rows | Moderate | Consider cursor pagination |
| > 100,000 rows | Poor | Use cursor or keyset pagination |

**Why offset pagination slows down:**
- Database must skip N rows before returning results
- At offset 100,000, server reads 100,000 + limit rows

## Cursor Pagination

More efficient for large datasets. Uses opaque cursors for navigation.

### Basic Usage

=== "Rust"

    ```rust
    // First page
    let query = GraphQuery::new("Post")
        .with_limit(10)
        .with_order(OrderSpec::desc("created_at"));

    let result = client.query(query).await?;

    // Next page using cursor
    if let Some(cursor) = result.next_cursor {
        let next_query = GraphQuery::new("Post")
            .with_cursor(cursor)
            .with_limit(10)
            .with_order(OrderSpec::desc("created_at"));

        let next_result = client.query(next_query).await?;
    }
    ```

=== "TypeScript"

    ```typescript
    // First page
    const result = await client.query("Post", {
      limit: 10,
      orderBy: [{ field: "created_at", direction: "desc" }],
    });

    // Next page
    if (result.nextCursor) {
      const nextPage = await client.query("Post", {
        cursor: result.nextCursor,
        limit: 10,
        orderBy: [{ field: "created_at", direction: "desc" }],
      });
    }

    // Previous page
    if (result.prevCursor) {
      const prevPage = await client.query("Post", {
        cursor: result.prevCursor,
        limit: 10,
        orderBy: [{ field: "created_at", direction: "desc" }],
      });
    }
    ```

### Cursor Structure

Cursors are opaque base64-encoded strings:

```
eyJpZCI6IjU1MGU4NDAwLWUyOWItNDFkNC1hNzE2LTQ0NjY1NTQ0MDAwMCIsImNyZWF0ZWRfYXQiOjE3MDUzMTIwMDB9
```

Decoded:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "created_at": 1705312000
}
```

### Bidirectional Navigation

```typescript
interface PaginatedResult<T> {
  entities: T[];
  nextCursor: string | null;
  prevCursor: string | null;
  hasMore: boolean;
}

// Navigate forward
let cursor = null;
while (true) {
  const result = await client.query("Post", {
    cursor,
    limit: 10,
    orderBy: [{ field: "created_at", direction: "desc" }],
  });

  processPage(result.entities);

  if (!result.nextCursor) break;
  cursor = result.nextCursor;
}
```

## Keyset Pagination

Most efficient for real-time data. Uses actual field values as boundaries.

### Basic Usage

=== "Rust"

    ```rust
    // First page
    let query = GraphQuery::new("Post")
        .with_limit(10)
        .with_order(OrderSpec::desc("created_at"));

    let result = client.query(query).await?;
    let last_post = result.entities.last();

    // Next page - filter by last seen value
    if let Some(post) = last_post {
        let next_query = GraphQuery::new("Post")
            .with_filter(FilterExpr::lt("created_at", post.created_at))
            .with_limit(10)
            .with_order(OrderSpec::desc("created_at"));
    }
    ```

=== "TypeScript"

    ```typescript
    // First page
    const result = await client.query("Post", {
      limit: 10,
      orderBy: [{ field: "created_at", direction: "desc" }],
    });

    // Next page
    const lastPost = result.entities[result.entities.length - 1];
    if (lastPost) {
      const nextPage = await client.query("Post", {
        filter: { field: "created_at", op: "lt", value: lastPost.created_at },
        limit: 10,
        orderBy: [{ field: "created_at", direction: "desc" }],
      });
    }
    ```

### Handling Ties

When multiple rows have the same sort value:

```rust
// Use composite key for tie-breaking
let query = GraphQuery::new("Post")
    .with_filter(FilterExpr::or(vec![
        FilterExpr::lt("created_at", last_created_at),
        FilterExpr::and(vec![
            FilterExpr::eq("created_at", last_created_at),
            FilterExpr::lt("id", last_id),
        ]),
    ]))
    .with_orders(vec![
        OrderSpec::desc("created_at"),
        OrderSpec::desc("id"),  // Tie-breaker
    ])
    .with_limit(10);
```

## Pagination with Relations

### Paginate Root and Relations

```rust
let query = GraphQuery::new("User")
    .with_pagination(Pagination::new(10, 0))
    .with_order(OrderSpec::asc("name"))
    .include(RelationInclude::new("posts")
        .with_limit(5)  // Limit posts per user
        .with_order(OrderSpec::desc("created_at")));

let result = client.query(query).await?;
```

### Load More Pattern

```typescript
// Initial load
const user = await client.query("User", {
  filter: { field: "id", op: "eq", value: userId },
  includes: [
    {
      relation: "posts",
      limit: 5,
      orderBy: [{ field: "created_at", direction: "desc" }],
    },
  ],
});

// Load more posts for a specific user
const morePosts = await client.query("Post", {
  filter: { field: "author_id", op: "eq", value: userId },
  cursor: lastPostCursor,
  limit: 5,
  orderBy: [{ field: "created_at", direction: "desc" }],
});
```

## Counting Total Results

### With Count

```rust
// Get total count alongside results
let query = GraphQuery::new("Post")
    .with_pagination(Pagination::new(10, 0))
    .with_count(true);

let result = client.query(query).await?;
println!("Showing {} of {} total", result.entities.len(), result.total_count);
```

### Separate Count Query

For better performance with large datasets:

```rust
// Only get count
let count = client.aggregate(
    AggregateQuery::new("Post")
        .count()
        .with_filter(filter.clone())
).await?.count;

// Get page without count
let result = client.query(
    GraphQuery::new("Post")
        .with_filter(filter)
        .with_pagination(Pagination::new(10, 0))
).await?;
```

## API Design Patterns

### REST-Style Response

```typescript
interface PagedResponse<T> {
  data: T[];
  pagination: {
    page: number;
    pageSize: number;
    totalPages: number;
    totalCount: number;
  };
  links: {
    self: string;
    first: string;
    prev: string | null;
    next: string | null;
    last: string;
  };
}
```

### GraphQL-Style Connections

```typescript
interface Connection<T> {
  edges: Array<{
    node: T;
    cursor: string;
  }>;
  pageInfo: {
    hasNextPage: boolean;
    hasPrevPage: boolean;
    startCursor: string;
    endCursor: string;
  };
  totalCount: number;
}
```

## Performance Comparison

| Method | First Page | Page 100 | Page 10,000 | Random Access |
|--------|------------|----------|-------------|---------------|
| Offset | 5ms | 15ms | 500ms | Yes |
| Cursor | 5ms | 5ms | 5ms | No |
| Keyset | 5ms | 5ms | 5ms | Limited |

## Best Practices

### 1. Choose the Right Strategy

| Use Case | Recommended |
|----------|-------------|
| Admin tables with random access | Offset |
| Infinite scroll | Cursor |
| Real-time feeds | Keyset |
| Search results | Offset or Cursor |

### 2. Always Include Order

```rust
// Bad: Unstable pagination
let query = GraphQuery::new("Post")
    .with_pagination(Pagination::new(10, 0));

// Good: Consistent ordering
let query = GraphQuery::new("Post")
    .with_pagination(Pagination::new(10, 0))
    .with_orders(vec![
        OrderSpec::desc("created_at"),
        OrderSpec::asc("id"),  // Tie-breaker
    ]);
```

### 3. Set Reasonable Limits

```rust
// Server-side max limit
const MAX_PAGE_SIZE: u32 = 100;

let limit = std::cmp::min(requested_limit, MAX_PAGE_SIZE);
let query = GraphQuery::new("Post")
    .with_pagination(Pagination::new(limit, offset));
```

### 4. Cache Count Separately

```typescript
// Cache total count (changes less frequently)
const cacheKey = `post_count_${JSON.stringify(filter)}`;
let totalCount = await cache.get(cacheKey);

if (!totalCount) {
  totalCount = await client.aggregate("Post", { count: true, filter });
  await cache.set(cacheKey, totalCount, { ttl: 60 });
}
```

### 5. Handle Concurrent Modifications

```typescript
// Keyset pagination handles new items gracefully
const feed = await client.query("Post", {
  filter: { field: "created_at", op: "lt", value: lastSeenTimestamp },
  limit: 20,
  orderBy: [{ field: "created_at", direction: "desc" }],
});

// New posts added after lastSeenTimestamp won't affect this page
```

---

## Next Steps

- **[Filtering Tutorial](../tutorials/filtering.md)** - Advanced filter expressions
- **[Performance Guide](performance.md)** - Optimize paginated queries
- **[Blog Platform Example](../examples/blog-platform.md)** - Pagination in practice
