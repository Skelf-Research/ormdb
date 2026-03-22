# Your First App

In this tutorial, we'll build a complete blog application with users, posts, and comments. You'll learn:

- Schema design with multiple entities and relations
- CRUD operations (Create, Read, Update, Delete)
- Querying with nested includes
- Filtering and pagination
- Error handling

## The Blog Schema

Our blog has three entities:

```
User (1) ─────→ (N) Post (1) ─────→ (N) Comment
  │                                       │
  └───────────────────────────────────────┘
                   author
```

- A **User** can write many **Posts**
- A **Post** can have many **Comments**
- A **Comment** has an author (User)

## Step 1: Define the Schema

=== "Rust"

    ```rust
    use ormdb_core::catalog::{
        Catalog, EntityDef, FieldDef, FieldType, RelationDef,
        ScalarType, SchemaBundle, DeleteBehavior
    };

    // User entity
    let user = EntityDef::new("User", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("username", FieldType::Scalar(ScalarType::String))
            .required())
        .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String))
            .required()
            .indexed())
        .with_field(FieldDef::new("bio", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("created_at", FieldType::Scalar(ScalarType::Timestamp))
            .with_default_current_timestamp());

    // Post entity
    let post = EntityDef::new("Post", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String))
            .required())
        .with_field(FieldDef::new("content", FieldType::Scalar(ScalarType::String))
            .required())
        .with_field(FieldDef::new("published", FieldType::Scalar(ScalarType::Bool))
            .with_default(false))
        .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid))
            .required())
        .with_field(FieldDef::new("created_at", FieldType::Scalar(ScalarType::Timestamp))
            .with_default_current_timestamp());

    // Comment entity
    let comment = EntityDef::new("Comment", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("content", FieldType::Scalar(ScalarType::String))
            .required())
        .with_field(FieldDef::new("post_id", FieldType::Scalar(ScalarType::Uuid))
            .required())
        .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid))
            .required())
        .with_field(FieldDef::new("created_at", FieldType::Scalar(ScalarType::Timestamp))
            .with_default_current_timestamp());

    // Relations
    let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id")
        .with_delete_behavior(DeleteBehavior::Cascade);

    let post_comments = RelationDef::one_to_many("comments", "Post", "id", "Comment", "post_id")
        .with_delete_behavior(DeleteBehavior::Cascade);

    let comment_author = RelationDef::many_to_one("author", "Comment", "author_id", "User", "id");

    // Build schema
    let schema = SchemaBundle::new(1)
        .with_entity(user)
        .with_entity(post)
        .with_entity(comment)
        .with_relation(user_posts)
        .with_relation(post_comments)
        .with_relation(comment_author);

    catalog.apply_schema(schema)?;
    ```

=== "TypeScript"

    ```typescript
    const schema = {
      version: 1,
      entities: [
        {
          name: "User",
          primaryKey: "id",
          fields: [
            { name: "id", type: "uuid" },
            { name: "username", type: "string", required: true },
            { name: "email", type: "string", required: true, indexed: true },
            { name: "bio", type: "string" },
            { name: "created_at", type: "timestamp", default: "current_timestamp" },
          ],
        },
        {
          name: "Post",
          primaryKey: "id",
          fields: [
            { name: "id", type: "uuid" },
            { name: "title", type: "string", required: true },
            { name: "content", type: "string", required: true },
            { name: "published", type: "bool", default: false },
            { name: "author_id", type: "uuid", required: true },
            { name: "created_at", type: "timestamp", default: "current_timestamp" },
          ],
        },
        {
          name: "Comment",
          primaryKey: "id",
          fields: [
            { name: "id", type: "uuid" },
            { name: "content", type: "string", required: true },
            { name: "post_id", type: "uuid", required: true },
            { name: "author_id", type: "uuid", required: true },
            { name: "created_at", type: "timestamp", default: "current_timestamp" },
          ],
        },
      ],
      relations: [
        {
          name: "posts",
          from: { entity: "User", field: "id" },
          to: { entity: "Post", field: "author_id" },
          cardinality: "one_to_many",
          onDelete: "cascade",
        },
        {
          name: "comments",
          from: { entity: "Post", field: "id" },
          to: { entity: "Comment", field: "post_id" },
          cardinality: "one_to_many",
          onDelete: "cascade",
        },
        {
          name: "author",
          from: { entity: "Comment", field: "author_id" },
          to: { entity: "User", field: "id" },
          cardinality: "many_to_one",
        },
      ],
    };

    await fetch("http://localhost:8080/admin/schema", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(schema),
    });
    ```

=== "Python"

    ```python
    schema = {
        "version": 1,
        "entities": [
            {
                "name": "User",
                "primaryKey": "id",
                "fields": [
                    {"name": "id", "type": "uuid"},
                    {"name": "username", "type": "string", "required": True},
                    {"name": "email", "type": "string", "required": True, "indexed": True},
                    {"name": "bio", "type": "string"},
                    {"name": "created_at", "type": "timestamp", "default": "current_timestamp"},
                ],
            },
            {
                "name": "Post",
                "primaryKey": "id",
                "fields": [
                    {"name": "id", "type": "uuid"},
                    {"name": "title", "type": "string", "required": True},
                    {"name": "content", "type": "string", "required": True},
                    {"name": "published", "type": "bool", "default": False},
                    {"name": "author_id", "type": "uuid", "required": True},
                    {"name": "created_at", "type": "timestamp", "default": "current_timestamp"},
                ],
            },
            {
                "name": "Comment",
                "primaryKey": "id",
                "fields": [
                    {"name": "id", "type": "uuid"},
                    {"name": "content", "type": "string", "required": True},
                    {"name": "post_id", "type": "uuid", "required": True},
                    {"name": "author_id", "type": "uuid", "required": True},
                    {"name": "created_at", "type": "timestamp", "default": "current_timestamp"},
                ],
            },
        ],
        "relations": [
            {
                "name": "posts",
                "from": {"entity": "User", "field": "id"},
                "to": {"entity": "Post", "field": "author_id"},
                "cardinality": "one_to_many",
                "onDelete": "cascade",
            },
            {
                "name": "comments",
                "from": {"entity": "Post", "field": "id"},
                "to": {"entity": "Comment", "field": "post_id"},
                "cardinality": "one_to_many",
                "onDelete": "cascade",
            },
            {
                "name": "author",
                "from": {"entity": "Comment", "field": "author_id"},
                "to": {"entity": "User", "field": "id"},
                "cardinality": "many_to_one",
            },
        ],
    }

    import requests
    requests.post("http://localhost:8080/admin/schema", json=schema)
    ```

## Step 2: Create Users

=== "Rust"

    ```rust
    use ormdb_client::{Client, ClientConfig};
    use ormdb_proto::{Mutation, Value};

    let client = Client::connect(ClientConfig::localhost()).await?;

    // Create multiple users
    let users = vec![
        ("alice", "alice@example.com", "Software engineer"),
        ("bob", "bob@example.com", "Designer"),
        ("charlie", "charlie@example.com", "Writer"),
    ];

    let mut user_ids = Vec::new();
    for (username, email, bio) in users {
        let mutation = Mutation::insert("User")
            .with_field("username", Value::String(username.into()))
            .with_field("email", Value::String(email.into()))
            .with_field("bio", Value::String(bio.into()));

        let result = client.mutate(mutation).await?;
        user_ids.push(result.inserted_id());
    }
    ```

=== "TypeScript"

    ```typescript
    const client = new OrmdbClient("http://localhost:8080");

    const users = [
      { username: "alice", email: "alice@example.com", bio: "Software engineer" },
      { username: "bob", email: "bob@example.com", bio: "Designer" },
      { username: "charlie", email: "charlie@example.com", bio: "Writer" },
    ];

    const userIds: string[] = [];
    for (const user of users) {
      const result = await client.insert("User", user);
      userIds.push(result.insertedIds[0]);
    }
    ```

=== "Python"

    ```python
    client = OrmdbClient("http://localhost:8080")

    users = [
        {"username": "alice", "email": "alice@example.com", "bio": "Software engineer"},
        {"username": "bob", "email": "bob@example.com", "bio": "Designer"},
        {"username": "charlie", "email": "charlie@example.com", "bio": "Writer"},
    ]

    user_ids = []
    for user in users:
        result = client.insert("User", user)
        user_ids.append(result.inserted_ids[0])
    ```

## Step 3: Create Posts and Comments

=== "Rust"

    ```rust
    // Alice creates a post
    let post = Mutation::insert("Post")
        .with_field("title", Value::String("Getting Started with ORMDB".into()))
        .with_field("content", Value::String("This is my first post...".into()))
        .with_field("published", Value::Bool(true))
        .with_field("author_id", Value::Uuid(user_ids[0]));  // Alice

    let post_result = client.mutate(post).await?;
    let post_id = post_result.inserted_id();

    // Bob comments on Alice's post
    let comment = Mutation::insert("Comment")
        .with_field("content", Value::String("Great post!".into()))
        .with_field("post_id", Value::Uuid(post_id))
        .with_field("author_id", Value::Uuid(user_ids[1]));  // Bob

    client.mutate(comment).await?;
    ```

=== "TypeScript"

    ```typescript
    // Alice creates a post
    const postResult = await client.insert("Post", {
      title: "Getting Started with ORMDB",
      content: "This is my first post...",
      published: true,
      author_id: userIds[0], // Alice
    });
    const postId = postResult.insertedIds[0];

    // Bob comments on Alice's post
    await client.insert("Comment", {
      content: "Great post!",
      post_id: postId,
      author_id: userIds[1], // Bob
    });
    ```

=== "Python"

    ```python
    # Alice creates a post
    post_result = client.insert("Post", {
        "title": "Getting Started with ORMDB",
        "content": "This is my first post...",
        "published": True,
        "author_id": user_ids[0],  # Alice
    })
    post_id = post_result.inserted_ids[0]

    # Bob comments on Alice's post
    client.insert("Comment", {
        "content": "Great post!",
        "post_id": post_id,
        "author_id": user_ids[1],  # Bob
    })
    ```

## Step 4: Query the Blog

Now let's query the blog with nested includes:

=== "Rust"

    ```rust
    use ormdb_proto::{GraphQuery, RelationInclude, FilterExpr, OrderSpec};

    // Get published posts with author and comments (including comment authors)
    let query = GraphQuery::new("Post")
        .with_fields(vec!["id", "title", "content", "created_at"])
        .with_filter(FilterExpr::eq("published", Value::Bool(true)))
        .with_order(OrderSpec::desc("created_at"))
        .include(RelationInclude::new("author")
            .with_fields(vec!["username"]))
        .include(RelationInclude::new("comments")
            .with_fields(vec!["content", "created_at"])
            .with_order(OrderSpec::asc("created_at"))
            .include(RelationInclude::new("author")
                .with_fields(vec!["username"])));

    let result = client.query(query).await?;

    for post in result.entities("Post") {
        let author = result.related_one(&post, "author")?;
        println!("{} by {}", post.get_string("title")?, author.get_string("username")?);

        for comment in result.related(&post, "comments") {
            let comment_author = result.related_one(&comment, "author")?;
            println!("  - {} ({})", comment.get_string("content")?, comment_author.get_string("username")?);
        }
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("Post", {
      fields: ["id", "title", "content", "created_at"],
      filter: { field: "published", op: "eq", value: true },
      orderBy: [{ field: "created_at", direction: "desc" }],
      includes: [
        { relation: "author", fields: ["username"] },
        {
          relation: "comments",
          fields: ["content", "created_at"],
          orderBy: [{ field: "created_at", direction: "asc" }],
          includes: [{ relation: "author", fields: ["username"] }],
        },
      ],
    });

    for (const post of result.entities) {
      console.log(`${post.title} by ${post.author?.username}`);

      for (const comment of post.comments || []) {
        console.log(`  - ${comment.content} (${comment.author?.username})`);
      }
    }
    ```

=== "Python"

    ```python
    result = client.query(
        "Post",
        fields=["id", "title", "content", "created_at"],
        filter={"field": "published", "op": "eq", "value": True},
        order_by=[{"field": "created_at", "direction": "desc"}],
        includes=[
            {"relation": "author", "fields": ["username"]},
            {
                "relation": "comments",
                "fields": ["content", "created_at"],
                "order_by": [{"field": "created_at", "direction": "asc"}],
                "includes": [{"relation": "author", "fields": ["username"]}],
            },
        ],
    )

    for post in result.entities:
        print(f"{post['title']} by {post['author']['username']}")

        for comment in post.get("comments", []):
            print(f"  - {comment['content']} ({comment['author']['username']})")
    ```

## Step 5: Update and Delete

=== "Rust"

    ```rust
    // Update a post
    let update = Mutation::update("Post", post_id)
        .with_field("title", Value::String("Updated Title".into()));
    client.mutate(update).await?;

    // Delete a comment
    let delete = Mutation::delete("Comment", comment_id);
    client.mutate(delete).await?;

    // Delete a user (cascades to posts and comments)
    let delete_user = Mutation::delete("User", user_ids[2]);  // Charlie
    client.mutate(delete_user).await?;
    ```

=== "TypeScript"

    ```typescript
    // Update a post
    await client.update("Post", postId, { title: "Updated Title" });

    // Delete a comment
    await client.delete("Comment", commentId);

    // Delete a user (cascades to posts and comments)
    await client.delete("User", userIds[2]); // Charlie
    ```

=== "Python"

    ```python
    # Update a post
    client.update("Post", post_id, {"title": "Updated Title"})

    # Delete a comment
    client.delete("Comment", comment_id)

    # Delete a user (cascades to posts and comments)
    client.delete("User", user_ids[2])  # Charlie
    ```

## Step 6: Error Handling

=== "Rust"

    ```rust
    use ormdb_client::Error;

    match client.mutate(mutation).await {
        Ok(result) => println!("Success: {:?}", result),
        Err(Error::ConstraintViolation(e)) => {
            println!("Constraint violated: {}", e.message);
        }
        Err(Error::NotFound) => {
            println!("Entity not found");
        }
        Err(e) => {
            println!("Other error: {}", e);
        }
    }
    ```

=== "TypeScript"

    ```typescript
    import { OrmdbError } from "@ormdb/client";

    try {
      await client.insert("User", { email: "alice@example.com" }); // Missing required field
    } catch (error) {
      if (error instanceof OrmdbError) {
        console.error(`Error: ${error.message} (code: ${error.code})`);
      }
    }
    ```

=== "Python"

    ```python
    from ormdb import QueryError, MutationError, ConnectionError

    try:
        client.insert("User", {"email": "alice@example.com"})  # Missing required field
    except MutationError as e:
        print(f"Mutation failed: {e.message} (code: {e.code})")
    except ConnectionError as e:
        print(f"Connection failed: {e}")
    ```

## Summary

You've built a complete blog application with:

- **Three entities** with relations between them
- **CRUD operations** for all entities
- **Nested queries** loading posts with authors and comments
- **Cascade deletes** that clean up related data
- **Error handling** for common failure cases

## Next Steps

- **[Schema Design](../tutorials/schema-design.md)** - Learn about all field types and constraints
- **[Filtering](../tutorials/filtering.md)** - Advanced filtering with AND/OR/NOT
- **[Pagination](../guides/pagination.md)** - Handle large datasets
- **[Security](../guides/security.md)** - Add row-level security
