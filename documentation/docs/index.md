# ORMDB

**The ORM-First Relational Database**

ORMDB is a relational database designed with ORMs as the primary interface. It provides typed queries, graph-shaped fetches, and safe schema migrations while preserving ACID guarantees.

---

## Why ORMDB?

Traditional databases expose SQL, leaving ORMs to bridge the gap between your application's object model and relational storage. This creates problems:

| Problem | Traditional Approach | ORMDB Solution |
|---------|---------------------|----------------|
| **N+1 Queries** | ORMs issue separate queries for related data | Graph fetches load related data in one round trip |
| **Type Safety** | SQL strings are untyped at compile time | Typed query protocol prevents injection and enables optimization |
| **Schema Drift** | Migrations are manual and error-prone | Built-in migration engine with safety grades |
| **Graph Explosion** | No limits on join depth or fanout | Enforced budgets prevent runaway queries |

---

## Quick Example

=== "Rust"

    ```rust
    use ormdb_client::{Client, ClientConfig};
    use ormdb_proto::{GraphQuery, FilterExpr, Value};

    #[tokio::main]
    async fn main() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::connect(ClientConfig::localhost()).await?;

        // Query users with their posts
        let query = GraphQuery::new("User")
            .with_fields(vec!["id", "name", "email"])
            .with_filter(FilterExpr::eq("status", Value::String("active".into())))
            .include("posts");

        let result = client.query(query).await?;
        println!("Found {} users", result.total_entities());

        Ok(())
    }
    ```

=== "TypeScript"

    ```typescript
    import { OrmdbClient } from "@ormdb/client";

    const client = new OrmdbClient("http://localhost:8080");

    // Query users with their posts
    const result = await client.query("User", {
      fields: ["id", "name", "email"],
      filter: { field: "status", op: "eq", value: "active" },
      includes: [{ relation: "posts" }],
    });

    console.log(`Found ${result.entities.length} users`);
    ```

=== "Python"

    ```python
    from ormdb import OrmdbClient

    client = OrmdbClient("http://localhost:8080")

    # Query users with their posts
    result = client.query(
        "User",
        fields=["id", "name", "email"],
        filter={"field": "status", "op": "eq", "value": "active"},
        includes=[{"relation": "posts"}],
    )

    print(f"Found {len(result.entities)} users")
    ```

---

## Key Features

### Graph Fetches

Load entities and their relations in a single query. No more N+1 problems.

```
User → Posts → Comments
     ↘ Teams → Members
```

### Typed Protocol

Queries are structured data, not strings. The database understands your schema and validates queries at compile time.

### Safe Migrations

Schema changes are analyzed and graded for safety:

- **Grade A**: Online, no blocking (add optional field)
- **Grade B**: Online with background backfill (add indexed field)
- **Grade C**: Brief write lock required (rename field)
- **Grade D**: Destructive, requires confirmation (drop field)

### Security Built-In

- **Row-Level Security**: Filter data based on user context
- **Field-Level Security**: Mask sensitive fields
- **Capabilities**: Fine-grained permissions per connection
- **Audit Logging**: Track all changes

### Budget Enforcement

Prevent runaway queries with built-in limits:

- Maximum entities returned
- Maximum include depth
- Maximum edges traversed
- Query execution timeout

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Your Application                      │
├─────────────────────────────────────────────────────────┤
│   Rust Client  │  TypeScript Client  │  Python Client   │
├─────────────────────────────────────────────────────────┤
│                    ORMDB Server                          │
├──────────────┬──────────────┬───────────────────────────┤
│ Query Engine │   Catalog    │      Storage Engine       │
│  - Planner   │  - Entities  │  - Row Store (MVCC)       │
│  - Executor  │  - Relations │  - Columnar Store         │
│  - Cache     │  - Policies  │  - Hash/B-tree Indexes    │
└──────────────┴──────────────┴───────────────────────────┘
```

---

## Getting Started

Ready to try ORMDB? Start with the [Installation Guide](getting-started/installation.md) or jump straight to the [5-Minute Quickstart](getting-started/quickstart.md).

<div class="grid cards" markdown>

- :material-download: **Installation**

    ---

    Install ORMDB server and client libraries

    [:octicons-arrow-right-24: Get started](getting-started/installation.md)

- :material-rocket-launch: **Quickstart**

    ---

    Build your first app in 5 minutes

    [:octicons-arrow-right-24: Tutorial](getting-started/quickstart.md)

- :material-book-open-variant: **Tutorials**

    ---

    Learn schema design, queries, and mutations

    [:octicons-arrow-right-24: Learn more](tutorials/index.md)

- :material-api: **API Reference**

    ---

    Complete API documentation

    [:octicons-arrow-right-24: Reference](reference/index.md)

</div>
