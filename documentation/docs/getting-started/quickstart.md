# Quickstart

Build your first ORMDB application in 5 minutes.

## Prerequisites

- ORMDB server running (see [Installation](installation.md))
- Your preferred client library installed

## Step 1: Define a Schema

First, let's define a simple schema with Users and Posts.

=== "Rust"

    ```rust
    use ormdb_core::catalog::{
        Catalog, EntityDef, FieldDef, FieldType, RelationDef,
        ScalarType, SchemaBundle
    };

    // Define User entity
    let user = EntityDef::new("User", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String)));

    // Define Post entity
    let post = EntityDef::new("Post", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("content", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid)));

    // Define relation: User has many Posts
    let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");

    // Create schema bundle
    let schema = SchemaBundle::new(1)
        .with_entity(user)
        .with_entity(post)
        .with_relation(user_posts);
    ```

=== "TypeScript"

    Schema is typically defined via the HTTP API or admin interface:

    ```typescript
    // Schema definition via admin API
    await fetch("http://localhost:8080/admin/schema", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        version: 1,
        entities: [
          {
            name: "User",
            primaryKey: "id",
            fields: [
              { name: "id", type: "uuid" },
              { name: "name", type: "string" },
              { name: "email", type: "string" },
            ],
          },
          {
            name: "Post",
            primaryKey: "id",
            fields: [
              { name: "id", type: "uuid" },
              { name: "title", type: "string" },
              { name: "content", type: "string" },
              { name: "author_id", type: "uuid" },
            ],
          },
        ],
        relations: [
          {
            name: "posts",
            from: { entity: "User", field: "id" },
            to: { entity: "Post", field: "author_id" },
            cardinality: "one_to_many",
          },
        ],
      }),
    });
    ```

=== "Python"

    ```python
    # Schema definition via admin API
    import requests

    schema = {
        "version": 1,
        "entities": [
            {
                "name": "User",
                "primaryKey": "id",
                "fields": [
                    {"name": "id", "type": "uuid"},
                    {"name": "name", "type": "string"},
                    {"name": "email", "type": "string"},
                ],
            },
            {
                "name": "Post",
                "primaryKey": "id",
                "fields": [
                    {"name": "id", "type": "uuid"},
                    {"name": "title", "type": "string"},
                    {"name": "content", "type": "string"},
                    {"name": "author_id", "type": "uuid"},
                ],
            },
        ],
        "relations": [
            {
                "name": "posts",
                "from": {"entity": "User", "field": "id"},
                "to": {"entity": "Post", "field": "author_id"},
                "cardinality": "one_to_many",
            },
        ],
    }

    requests.post("http://localhost:8080/admin/schema", json=schema)
    ```

## Step 2: Insert Data

=== "Rust"

    ```rust
    use ormdb_client::{Client, ClientConfig};
    use ormdb_proto::{Mutation, FieldValue, Value};

    let client = Client::connect(ClientConfig::localhost()).await?;

    // Insert a user
    let user_mutation = Mutation::insert("User")
        .with_field("name", Value::String("Alice".into()))
        .with_field("email", Value::String("alice@example.com".into()));

    let result = client.mutate(user_mutation).await?;
    let user_id = result.inserted_id();

    // Insert a post
    let post_mutation = Mutation::insert("Post")
        .with_field("title", Value::String("Hello ORMDB".into()))
        .with_field("content", Value::String("My first post!".into()))
        .with_field("author_id", Value::Uuid(user_id));

    client.mutate(post_mutation).await?;
    ```

=== "TypeScript"

    ```typescript
    import { OrmdbClient } from "@ormdb/client";

    const client = new OrmdbClient("http://localhost:8080");

    // Insert a user
    const userResult = await client.insert("User", {
      name: "Alice",
      email: "alice@example.com",
    });
    const userId = userResult.insertedIds[0];

    // Insert a post
    await client.insert("Post", {
      title: "Hello ORMDB",
      content: "My first post!",
      author_id: userId,
    });
    ```

=== "Python"

    ```python
    from ormdb import OrmdbClient

    client = OrmdbClient("http://localhost:8080")

    # Insert a user
    user_result = client.insert("User", {
        "name": "Alice",
        "email": "alice@example.com",
    })
    user_id = user_result.inserted_ids[0]

    # Insert a post
    client.insert("Post", {
        "title": "Hello ORMDB",
        "content": "My first post!",
        "author_id": user_id,
    })
    ```

## Step 3: Query Data

Here's the magic - query users with their posts in **one request**:

=== "Rust"

    ```rust
    use ormdb_proto::{GraphQuery, RelationInclude};

    // Query users with their posts (no N+1!)
    let query = GraphQuery::new("User")
        .with_fields(vec!["id", "name", "email"])
        .include(RelationInclude::new("posts")
            .with_fields(vec!["id", "title"]));

    let result = client.query(query).await?;

    // Process results
    for user in result.entities("User") {
        println!("User: {}", user.get_string("name")?);

        for post in result.related(&user, "posts") {
            println!("  Post: {}", post.get_string("title")?);
        }
    }
    ```

=== "TypeScript"

    ```typescript
    // Query users with their posts (no N+1!)
    const result = await client.query("User", {
      fields: ["id", "name", "email"],
      includes: [
        {
          relation: "posts",
          fields: ["id", "title"],
        },
      ],
    });

    // Process results
    for (const user of result.entities) {
      console.log(`User: ${user.name}`);

      for (const post of user.posts || []) {
        console.log(`  Post: ${post.title}`);
      }
    }
    ```

=== "Python"

    ```python
    # Query users with their posts (no N+1!)
    result = client.query(
        "User",
        fields=["id", "name", "email"],
        includes=[
            {
                "relation": "posts",
                "fields": ["id", "title"],
            },
        ],
    )

    # Process results
    for user in result.entities:
        print(f"User: {user['name']}")

        for post in user.get("posts", []):
            print(f"  Post: {post['title']}")
    ```

## Step 4: Filter and Sort

=== "Rust"

    ```rust
    use ormdb_proto::{FilterExpr, OrderSpec, Pagination};

    let query = GraphQuery::new("User")
        .with_filter(FilterExpr::like("email", "%@example.com"))
        .with_order(OrderSpec::asc("name"))
        .with_pagination(Pagination::new(10, 0));  // limit 10, offset 0

    let result = client.query(query).await?;
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      filter: { field: "email", op: "like", value: "%@example.com" },
      orderBy: [{ field: "name", direction: "asc" }],
      limit: 10,
      offset: 0,
    });
    ```

=== "Python"

    ```python
    result = client.query(
        "User",
        filter={"field": "email", "op": "like", "value": "%@example.com"},
        order_by=[{"field": "name", "direction": "asc"}],
        limit=10,
        offset=0,
    )
    ```

## What's Next?

Congratulations! You've built your first ORMDB application. Here's where to go next:

- **[Your First App](first-app.md)** - A complete tutorial with more features
- **[Schema Design](../tutorials/schema-design.md)** - Learn about fields, relations, and constraints
- **[Querying Data](../tutorials/querying-data.md)** - Deep dive into the query API
- **[Mutations](../tutorials/mutations.md)** - Insert, update, and delete operations
