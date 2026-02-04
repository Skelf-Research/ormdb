# Schema Design

Learn how to design your data model with entities, fields, and relations.

## Entities

An entity represents a type of object in your application (like User, Post, or Order).

### Basic Entity

=== "Rust"

    ```rust
    use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType};

    let user = EntityDef::new("User", "id")  // name and primary key field
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String)));
    ```

=== "TypeScript"

    ```typescript
    const user = {
      name: "User",
      primaryKey: "id",
      fields: [
        { name: "id", type: "uuid" },
        { name: "name", type: "string" },
        { name: "email", type: "string" },
      ],
    };
    ```

=== "Python"

    ```python
    user = {
        "name": "User",
        "primaryKey": "id",
        "fields": [
            {"name": "id", "type": "uuid"},
            {"name": "name", "type": "string"},
            {"name": "email", "type": "string"},
        ],
    }
    ```

## Field Types

### Scalar Types

| Type | Description | Rust | Example |
|------|-------------|------|---------|
| `uuid` | 128-bit UUID | `ScalarType::Uuid` | `"550e8400-e29b-41d4-a716-446655440000"` |
| `string` | UTF-8 text | `ScalarType::String` | `"Hello"` |
| `int32` | 32-bit integer | `ScalarType::Int32` | `42` |
| `int64` | 64-bit integer | `ScalarType::Int64` | `9223372036854775807` |
| `float32` | 32-bit float | `ScalarType::Float32` | `3.14` |
| `float64` | 64-bit float | `ScalarType::Float64` | `3.141592653589793` |
| `bool` | Boolean | `ScalarType::Bool` | `true` / `false` |
| `bytes` | Binary data | `ScalarType::Bytes` | `[0x01, 0x02, 0x03]` |
| `timestamp` | Microseconds since epoch | `ScalarType::Timestamp` | `1704067200000000` |

### Field Options

=== "Rust"

    ```rust
    // Required field (not nullable)
    FieldDef::new("email", FieldType::Scalar(ScalarType::String))
        .required()

    // With default value
    FieldDef::new("active", FieldType::Scalar(ScalarType::Bool))
        .with_default(true)

    // Auto-generate timestamp
    FieldDef::new("created_at", FieldType::Scalar(ScalarType::Timestamp))
        .with_default_current_timestamp()

    // Auto-generate UUID
    FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid))
        .with_default_auto_uuid()

    // Indexed for fast lookups
    FieldDef::new("email", FieldType::Scalar(ScalarType::String))
        .indexed()
    ```

=== "TypeScript"

    ```typescript
    const fields = [
      // Required field
      { name: "email", type: "string", required: true },

      // With default value
      { name: "active", type: "bool", default: true },

      // Auto-generate timestamp
      { name: "created_at", type: "timestamp", default: "current_timestamp" },

      // Auto-generate UUID
      { name: "id", type: "uuid", default: "auto_uuid" },

      // Indexed for fast lookups
      { name: "email", type: "string", indexed: true },
    ];
    ```

=== "Python"

    ```python
    fields = [
        # Required field
        {"name": "email", "type": "string", "required": True},

        # With default value
        {"name": "active", "type": "bool", "default": True},

        # Auto-generate timestamp
        {"name": "created_at", "type": "timestamp", "default": "current_timestamp"},

        # Auto-generate UUID
        {"name": "id", "type": "uuid", "default": "auto_uuid"},

        # Indexed for fast lookups
        {"name": "email", "type": "string", "indexed": True},
    ]
    ```

## Relations

Relations connect entities together.

### One-to-Many

A user has many posts:

```
User (1) ────→ (N) Post
```

=== "Rust"

    ```rust
    use ormdb_core::catalog::RelationDef;

    // User has many Posts (via Post.author_id → User.id)
    let user_posts = RelationDef::one_to_many(
        "posts",      // relation name
        "User",       // from entity
        "id",         // from field
        "Post",       // to entity
        "author_id",  // to field (foreign key)
    );
    ```

=== "TypeScript"

    ```typescript
    const userPosts = {
      name: "posts",
      from: { entity: "User", field: "id" },
      to: { entity: "Post", field: "author_id" },
      cardinality: "one_to_many",
    };
    ```

=== "Python"

    ```python
    user_posts = {
        "name": "posts",
        "from": {"entity": "User", "field": "id"},
        "to": {"entity": "Post", "field": "author_id"},
        "cardinality": "one_to_many",
    }
    ```

### One-to-One

A user has one profile:

```
User (1) ────→ (1) Profile
```

=== "Rust"

    ```rust
    let user_profile = RelationDef::one_to_one(
        "profile",
        "User",
        "id",
        "Profile",
        "user_id",
    );
    ```

=== "TypeScript"

    ```typescript
    const userProfile = {
      name: "profile",
      from: { entity: "User", field: "id" },
      to: { entity: "Profile", field: "user_id" },
      cardinality: "one_to_one",
    };
    ```

=== "Python"

    ```python
    user_profile = {
        "name": "profile",
        "from": {"entity": "User", "field": "id"},
        "to": {"entity": "Profile", "field": "user_id"},
        "cardinality": "one_to_one",
    }
    ```

### Many-to-Many

Users belong to many teams, teams have many users:

```
User (N) ←───→ (M) Team
           │
      TeamMember (edge entity)
```

=== "Rust"

    ```rust
    // Edge entity for many-to-many
    let team_member = EntityDef::new("TeamMember", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("user_id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("team_id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("role", FieldType::Scalar(ScalarType::String)));

    // Relations
    let user_memberships = RelationDef::one_to_many("memberships", "User", "id", "TeamMember", "user_id");
    let team_memberships = RelationDef::one_to_many("memberships", "Team", "id", "TeamMember", "team_id");
    let membership_user = RelationDef::many_to_one("user", "TeamMember", "user_id", "User", "id");
    let membership_team = RelationDef::many_to_one("team", "TeamMember", "team_id", "Team", "id");
    ```

=== "TypeScript"

    ```typescript
    // Edge entity for many-to-many
    const teamMember = {
      name: "TeamMember",
      primaryKey: "id",
      fields: [
        { name: "id", type: "uuid" },
        { name: "user_id", type: "uuid", required: true },
        { name: "team_id", type: "uuid", required: true },
        { name: "role", type: "string" },
      ],
    };

    // Relations
    const relations = [
      {
        name: "memberships",
        from: { entity: "User", field: "id" },
        to: { entity: "TeamMember", field: "user_id" },
        cardinality: "one_to_many",
      },
      {
        name: "memberships",
        from: { entity: "Team", field: "id" },
        to: { entity: "TeamMember", field: "team_id" },
        cardinality: "one_to_many",
      },
      {
        name: "user",
        from: { entity: "TeamMember", field: "user_id" },
        to: { entity: "User", field: "id" },
        cardinality: "many_to_one",
      },
      {
        name: "team",
        from: { entity: "TeamMember", field: "team_id" },
        to: { entity: "Team", field: "id" },
        cardinality: "many_to_one",
      },
    ];
    ```

=== "Python"

    ```python
    # Edge entity for many-to-many
    team_member = {
        "name": "TeamMember",
        "primaryKey": "id",
        "fields": [
            {"name": "id", "type": "uuid"},
            {"name": "user_id", "type": "uuid", "required": True},
            {"name": "team_id", "type": "uuid", "required": True},
            {"name": "role", "type": "string"},
        ],
    }

    # Relations (same pattern as TypeScript)
    ```

## Delete Behavior

Control what happens to related entities when a parent is deleted:

| Behavior | Description |
|----------|-------------|
| `Cascade` | Delete related entities |
| `Restrict` | Prevent deletion if related entities exist |
| `SetNull` | Set foreign key to null |

=== "Rust"

    ```rust
    use ormdb_core::catalog::DeleteBehavior;

    // When a user is deleted, delete their posts
    let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id")
        .with_delete_behavior(DeleteBehavior::Cascade);

    // Prevent deleting a team if it has members
    let team_members = RelationDef::one_to_many("members", "Team", "id", "TeamMember", "team_id")
        .with_delete_behavior(DeleteBehavior::Restrict);
    ```

=== "TypeScript"

    ```typescript
    const userPosts = {
      name: "posts",
      from: { entity: "User", field: "id" },
      to: { entity: "Post", field: "author_id" },
      cardinality: "one_to_many",
      onDelete: "cascade", // or "restrict" or "set_null"
    };
    ```

=== "Python"

    ```python
    user_posts = {
        "name": "posts",
        "from": {"entity": "User", "field": "id"},
        "to": {"entity": "Post", "field": "author_id"},
        "cardinality": "one_to_many",
        "onDelete": "cascade",  # or "restrict" or "set_null"
    }
    ```

## Schema Bundles

A schema bundle is a complete, versioned snapshot of your data model:

=== "Rust"

    ```rust
    use ormdb_core::catalog::SchemaBundle;

    let schema = SchemaBundle::new(1)  // version number
        .with_entity(user)
        .with_entity(post)
        .with_entity(comment)
        .with_relation(user_posts)
        .with_relation(post_comments);

    // Apply to catalog
    catalog.apply_schema(schema)?;
    ```

=== "TypeScript"

    ```typescript
    const schema = {
      version: 1,
      entities: [user, post, comment],
      relations: [userPosts, postComments],
    };

    await fetch("http://localhost:8080/admin/schema", {
      method: "POST",
      body: JSON.stringify(schema),
    });
    ```

=== "Python"

    ```python
    schema = {
        "version": 1,
        "entities": [user, post, comment],
        "relations": [user_posts, post_comments],
    }

    requests.post("http://localhost:8080/admin/schema", json=schema)
    ```

## Best Practices

1. **Use UUIDs for primary keys** - They're globally unique and don't leak sequence information
2. **Add timestamps** - `created_at` and `updated_at` are invaluable for debugging
3. **Index foreign keys** - Relations are faster when foreign keys are indexed
4. **Use meaningful relation names** - `posts` is clearer than `user_post_rel`
5. **Plan for soft deletes** - Consider adding a `deleted_at` field instead of hard deletes

## Next Steps

- **[Querying Data](querying-data.md)** - Learn how to query your schema
- **[Mutations](mutations.md)** - Insert, update, and delete data
- **[Migrations](../guides/schema-migrations.md)** - Evolve your schema safely
