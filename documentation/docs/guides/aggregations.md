# Aggregations Guide

Perform data aggregations with ORMDB.

## Overview

ORMDB supports common aggregation functions for analytics and reporting:

- `COUNT` - Count entities
- `SUM` - Sum numeric values
- `AVG` - Calculate average
- `MIN` - Find minimum value
- `MAX` - Find maximum value

## Basic Aggregations

### Count

=== "Rust"

    ```rust
    let query = AggregateQuery::new("User")
        .count();

    let result = client.aggregate(query).await?;
    println!("Total users: {}", result.count);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.aggregate("User", {
      count: true,
    });
    console.log(`Total users: ${result.count}`);
    ```

=== "Python"

    ```python
    result = client.aggregate("User", count=True)
    print(f"Total users: {result.count}")
    ```

### Count with Filter

```rust
let query = AggregateQuery::new("User")
    .count()
    .with_filter(FilterExpr::eq("status", Value::String("active".into())));

let result = client.aggregate(query).await?;
println!("Active users: {}", result.count);
```

### Count Non-Null Field

```rust
// Count users with email (non-null)
let query = AggregateQuery::new("User")
    .count_field("email");

let result = client.aggregate(query).await?;
println!("Users with email: {}", result.count);
```

## Numeric Aggregations

### Sum

=== "Rust"

    ```rust
    let query = AggregateQuery::new("Order")
        .sum("total")
        .with_filter(FilterExpr::eq("status", Value::String("completed".into())));

    let result = client.aggregate(query).await?;
    println!("Total revenue: ${:.2}", result.sum.unwrap());
    ```

=== "TypeScript"

    ```typescript
    const result = await client.aggregate("Order", {
      sum: "total",
      filter: { field: "status", op: "eq", value: "completed" },
    });
    console.log(`Total revenue: $${result.sum.toFixed(2)}`);
    ```

=== "Python"

    ```python
    result = client.aggregate("Order",
        sum="total",
        filter={"field": "status", "op": "eq", "value": "completed"})
    print(f"Total revenue: ${result.sum:.2f}")
    ```

### Average

```rust
let query = AggregateQuery::new("Product")
    .avg("price")
    .with_filter(FilterExpr::eq("category", Value::String("Electronics".into())));

let result = client.aggregate(query).await?;
println!("Average price: ${:.2}", result.avg.unwrap());
```

### Min and Max

```rust
let query = AggregateQuery::new("Order")
    .min("total")
    .max("total")
    .with_filter(FilterExpr::ge("created_at", Value::Timestamp(start_of_month)));

let result = client.aggregate(query).await?;
println!("Order range: ${:.2} - ${:.2}",
    result.min.unwrap(),
    result.max.unwrap());
```

## Multiple Aggregations

Combine multiple aggregations in one query:

=== "Rust"

    ```rust
    let query = AggregateQuery::new("Order")
        .count()
        .sum("total")
        .avg("total")
        .min("total")
        .max("total")
        .with_filter(FilterExpr::eq("status", Value::String("completed".into())));

    let result = client.aggregate(query).await?;
    println!("Orders: {}", result.count);
    println!("Total: ${:.2}", result.sum.unwrap());
    println!("Average: ${:.2}", result.avg.unwrap());
    println!("Range: ${:.2} - ${:.2}", result.min.unwrap(), result.max.unwrap());
    ```

=== "TypeScript"

    ```typescript
    const result = await client.aggregate("Order", {
      count: true,
      sum: "total",
      avg: "total",
      min: "total",
      max: "total",
      filter: { field: "status", op: "eq", value: "completed" },
    });

    console.log(`Orders: ${result.count}`);
    console.log(`Total: $${result.sum.toFixed(2)}`);
    console.log(`Average: $${result.avg.toFixed(2)}`);
    console.log(`Range: $${result.min.toFixed(2)} - $${result.max.toFixed(2)}`);
    ```

## Grouped Aggregations

### Group By Single Field

=== "Rust"

    ```rust
    let query = AggregateQuery::new("Order")
        .count()
        .sum("total")
        .group_by("status");

    let result = client.aggregate(query).await?;

    for group in result.groups {
        println!("{}: {} orders, ${:.2} total",
            group.key["status"],
            group.count,
            group.sum.unwrap());
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.aggregate("Order", {
      count: true,
      sum: "total",
      groupBy: ["status"],
    });

    for (const group of result.groups) {
      console.log(`${group.key.status}: ${group.count} orders, $${group.sum.toFixed(2)}`);
    }
    ```

=== "Python"

    ```python
    result = client.aggregate("Order",
        count=True,
        sum="total",
        group_by=["status"])

    for group in result.groups:
        print(f"{group.key['status']}: {group.count} orders, ${group.sum:.2f}")
    ```

**Output:**
```
pending: 150 orders, $15,234.50 total
completed: 1,245 orders, $145,678.90 total
cancelled: 23 orders, $2,345.00 total
```

### Group By Multiple Fields

```rust
let query = AggregateQuery::new("Order")
    .count()
    .sum("total")
    .group_by("status")
    .group_by("payment_method");

let result = client.aggregate(query).await?;

for group in result.groups {
    println!("{} / {}: {} orders",
        group.key["status"],
        group.key["payment_method"],
        group.count);
}
```

### Group By with Filter

```rust
let query = AggregateQuery::new("Order")
    .count()
    .sum("total")
    .group_by("category")
    .with_filter(FilterExpr::and(vec![
        FilterExpr::ge("created_at", start_of_month),
        FilterExpr::lt("created_at", end_of_month),
    ]));

let result = client.aggregate(query).await?;
```

## Time-Based Aggregations

### Group By Date

=== "Rust"

    ```rust
    let query = AggregateQuery::new("Order")
        .count()
        .sum("total")
        .group_by_date("created_at", DateTruncate::Day);

    let result = client.aggregate(query).await?;

    for group in result.groups {
        println!("{}: {} orders, ${:.2}",
            group.key["created_at"],
            group.count,
            group.sum.unwrap());
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.aggregate("Order", {
      count: true,
      sum: "total",
      groupBy: [{ field: "created_at", truncate: "day" }],
    });

    for (const group of result.groups) {
      console.log(`${group.key.created_at}: ${group.count} orders`);
    }
    ```

### Date Truncation Options

| Truncate | Description | Example |
|----------|-------------|---------|
| `second` | Per second | 2024-01-15 12:30:45 |
| `minute` | Per minute | 2024-01-15 12:30:00 |
| `hour` | Per hour | 2024-01-15 12:00:00 |
| `day` | Per day | 2024-01-15 |
| `week` | Per week | 2024-01-15 (Monday) |
| `month` | Per month | 2024-01-01 |
| `year` | Per year | 2024-01-01 |

### Time Series Example

```typescript
// Daily sales for the last 30 days
const thirtyDaysAgo = new Date();
thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);

const result = await client.aggregate("Order", {
  count: true,
  sum: "total",
  groupBy: [{ field: "created_at", truncate: "day" }],
  filter: {
    and: [
      { field: "created_at", op: "ge", value: thirtyDaysAgo.toISOString() },
      { field: "status", op: "eq", value: "completed" },
    ],
  },
  orderBy: [{ field: "created_at", direction: "asc" }],
});

// Fill in missing days with zeros
const salesByDay = new Map(
  result.groups.map((g) => [g.key.created_at, g])
);
```

## Aggregations with Relations

### Count Related Entities

```rust
// Count posts per user
let query = GraphQuery::new("User")
    .with_fields(vec!["id", "name"])
    .include(RelationInclude::new("posts")
        .aggregate(AggregateSpec::count()));

let result = client.query(query).await?;

for user in result.entities {
    println!("{}: {} posts", user.name, user.posts_count);
}
```

### Sum Related Values

```rust
// Total order amount per customer
let query = GraphQuery::new("Customer")
    .with_fields(vec!["id", "name"])
    .include(RelationInclude::new("orders")
        .with_filter(FilterExpr::eq("status", Value::String("completed".into())))
        .aggregate(AggregateSpec::sum("total")));

let result = client.query(query).await?;
```

## Having Clause

Filter groups based on aggregate values:

```rust
let query = AggregateQuery::new("Order")
    .count()
    .sum("total")
    .group_by("customer_id")
    .having(HavingExpr::gt("count", 10))  // Customers with > 10 orders
    .having(HavingExpr::ge("sum", 1000.0));  // And total >= $1000

let result = client.aggregate(query).await?;
```

## Distinct Count

Count unique values:

```rust
// Count unique customers who placed orders
let query = AggregateQuery::new("Order")
    .count_distinct("customer_id")
    .with_filter(FilterExpr::ge("created_at", start_of_month));

let result = client.aggregate(query).await?;
println!("Unique customers: {}", result.count_distinct);
```

## Percentiles (Advanced)

```rust
// Calculate order percentiles
let query = AggregateQuery::new("Order")
    .percentile("total", 0.5)   // Median
    .percentile("total", 0.95)  // 95th percentile
    .percentile("total", 0.99); // 99th percentile

let result = client.aggregate(query).await?;
println!("Median order: ${:.2}", result.percentiles[0]);
println!("P95 order: ${:.2}", result.percentiles[1]);
println!("P99 order: ${:.2}", result.percentiles[2]);
```

## Performance Tips

### 1. Use Filters to Reduce Dataset

```rust
// Bad: Aggregates all orders
let query = AggregateQuery::new("Order")
    .sum("total")
    .group_by("status");

// Good: Only recent orders
let query = AggregateQuery::new("Order")
    .sum("total")
    .group_by("status")
    .with_filter(FilterExpr::ge("created_at", thirty_days_ago));
```

### 2. Limit Group Count

```rust
// Top 10 customers by order count
let query = AggregateQuery::new("Order")
    .count()
    .group_by("customer_id")
    .order_by_aggregate("count", OrderDirection::Desc)
    .limit(10);
```

### 3. Pre-Aggregate for Dashboards

```rust
// Create materialized aggregation
let daily_stats = AggregateQuery::new("Order")
    .count()
    .sum("total")
    .group_by_date("created_at", DateTruncate::Day);

// Store results for fast dashboard queries
client.insert("DailySalesStats", daily_stats_result).await?;
```

### 4. Use Approximate Counts for Large Tables

```rust
// Fast approximate count
let query = AggregateQuery::new("Event")
    .approximate_count();

let result = client.aggregate(query).await?;
println!("Approximately {} events", result.approximate_count);
```

## Common Use Cases

### Dashboard Metrics

```typescript
async function getDashboardMetrics() {
  const [users, orders, revenue] = await Promise.all([
    client.aggregate("User", { count: true }),
    client.aggregate("Order", {
      count: true,
      filter: { field: "created_at", op: "ge", value: today },
    }),
    client.aggregate("Order", {
      sum: "total",
      filter: {
        and: [
          { field: "status", op: "eq", value: "completed" },
          { field: "created_at", op: "ge", value: startOfMonth },
        ],
      },
    }),
  ]);

  return {
    totalUsers: users.count,
    ordersToday: orders.count,
    monthlyRevenue: revenue.sum,
  };
}
```

### Leaderboard

```typescript
const leaderboard = await client.aggregate("Score", {
  sum: "points",
  groupBy: ["user_id"],
  orderBy: [{ aggregate: "sum", direction: "desc" }],
  limit: 100,
});
```

### Histogram / Distribution

```typescript
const priceDistribution = await client.aggregate("Product", {
  count: true,
  groupBy: [
    {
      field: "price",
      bucket: { start: 0, end: 1000, size: 100 },
    },
  ],
});

// Returns: 0-100: 45, 100-200: 123, 200-300: 89, ...
```

---

## Next Steps

- **[Real-time Dashboard Example](../examples/realtime-dashboard.md)** - Aggregations in action
- **[Change Data Capture](cdc.md)** - Stream aggregation updates
- **[Performance Guide](performance.md)** - Optimize aggregate queries
