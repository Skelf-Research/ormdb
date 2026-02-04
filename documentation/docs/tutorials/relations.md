# Relations

Learn how to work with relational data in ORMDB.

## Relation Types

ORMDB supports three types of relations:

| Type | Example | Description |
|------|---------|-------------|
| **One-to-One** | User → Profile | Each user has exactly one profile |
| **One-to-Many** | User → Posts | A user can have many posts |
| **Many-to-Many** | Users ↔ Teams | Users belong to many teams, teams have many users |

## One-to-Many Relations

The most common relation type.

### Schema

```
User (1) ────→ (N) Post
```

=== "Rust"

    ```rust
    // Post has a foreign key to User
    let post = EntityDef::new("Post", "id")
        .with_field(FieldDef::new("author_id", FieldType::Scalar(ScalarType::Uuid)));

    // Define the relation
    let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");
    ```

### Querying

=== "Rust"

    ```rust
    // Get users with their posts
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("posts"));

    let result = client.query(query).await?;

    for user in result.entities("User") {
        let posts = result.related(&user, "posts");
        println!("{} has {} posts", user.get_string("name")?, posts.len());
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [{ relation: "posts" }],
    });

    for (const user of result.entities) {
      console.log(`${user.name} has ${user.posts?.length || 0} posts`);
    }
    ```

=== "Python"

    ```python
    result = client.query("User", includes=[{"relation": "posts"}])

    for user in result.entities:
        print(f"{user['name']} has {len(user.get('posts', []))} posts")
    ```

### Inverse Relation

Query from the "many" side back to the "one":

=== "Rust"

    ```rust
    // Define inverse relation
    let post_author = RelationDef::many_to_one("author", "Post", "author_id", "User", "id");

    // Query posts with their author
    let query = GraphQuery::new("Post")
        .include(RelationInclude::new("author"));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("Post", {
      includes: [{ relation: "author" }],
    });

    for (const post of result.entities) {
      console.log(`${post.title} by ${post.author?.name}`);
    }
    ```

## One-to-One Relations

### Schema

```
User (1) ────→ (1) Profile
```

=== "Rust"

    ```rust
    let profile = EntityDef::new("Profile", "id")
        .with_field(FieldDef::new("user_id", FieldType::Scalar(ScalarType::Uuid)));

    let user_profile = RelationDef::one_to_one("profile", "User", "id", "Profile", "user_id");
    ```

### Querying

=== "Rust"

    ```rust
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("profile"));

    let result = client.query(query).await?;

    for user in result.entities("User") {
        if let Some(profile) = result.related_one(&user, "profile")? {
            println!("{}'s bio: {}", user.get_string("name")?, profile.get_string("bio")?);
        }
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [{ relation: "profile" }],
    });

    for (const user of result.entities) {
      if (user.profile) {
        console.log(`${user.name}'s bio: ${user.profile.bio}`);
      }
    }
    ```

## Many-to-Many Relations

Many-to-many relations require an edge entity (join table).

### Schema

```
User (N) ←───→ (M) Team
           │
      TeamMember (edge)
```

=== "Rust"

    ```rust
    // Edge entity with additional fields
    let team_member = EntityDef::new("TeamMember", "id")
        .with_field(FieldDef::new("user_id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("team_id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("role", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("joined_at", FieldType::Scalar(ScalarType::Timestamp)));

    // Relations
    let user_memberships = RelationDef::one_to_many("memberships", "User", "id", "TeamMember", "user_id");
    let team_memberships = RelationDef::one_to_many("memberships", "Team", "id", "TeamMember", "team_id");
    let membership_user = RelationDef::many_to_one("user", "TeamMember", "user_id", "User", "id");
    let membership_team = RelationDef::many_to_one("team", "TeamMember", "team_id", "Team", "id");
    ```

### Querying

=== "Rust"

    ```rust
    // Get user's teams (through memberships)
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("memberships")
            .include(RelationInclude::new("team")));

    let result = client.query(query).await?;

    for user in result.entities("User") {
        for membership in result.related(&user, "memberships") {
            let team = result.related_one(&membership, "team")?;
            let role = membership.get_string("role")?;
            println!("{} is a {} in {}",
                user.get_string("name")?,
                role,
                team.get_string("name")?);
        }
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [
        {
          relation: "memberships",
          includes: [{ relation: "team" }],
        },
      ],
    });

    for (const user of result.entities) {
      for (const membership of user.memberships || []) {
        console.log(
          `${user.name} is a ${membership.role} in ${membership.team?.name}`
        );
      }
    }
    ```

=== "Python"

    ```python
    result = client.query("User",
        includes=[{
            "relation": "memberships",
            "includes": [{"relation": "team"}],
        }])

    for user in result.entities:
        for membership in user.get("memberships", []):
            print(f"{user['name']} is a {membership['role']} in {membership['team']['name']}")
    ```

### Creating Many-to-Many

=== "Rust"

    ```rust
    // Add user to team
    let membership = Mutation::insert("TeamMember")
        .with_field("user_id", Value::Uuid(user_id))
        .with_field("team_id", Value::Uuid(team_id))
        .with_field("role", Value::String("member".into()));

    client.mutate(membership).await?;
    ```

=== "TypeScript"

    ```typescript
    // Add user to team
    await client.insert("TeamMember", {
      user_id: userId,
      team_id: teamId,
      role: "member",
    });
    ```

=== "Python"

    ```python
    # Add user to team
    client.insert("TeamMember", {
        "user_id": user_id,
        "team_id": team_id,
        "role": "member",
    })
    ```

## Nested Includes

Load multiple levels of relations:

=== "Rust"

    ```rust
    // User → Posts → Comments → Author
    let query = GraphQuery::new("User")
        .with_fields(vec!["id", "name"])
        .include(RelationInclude::new("posts")
            .with_fields(vec!["id", "title"])
            .include(RelationInclude::new("comments")
                .with_fields(vec!["content"])
                .include(RelationInclude::new("author")
                    .with_fields(vec!["name"]))));
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      fields: ["id", "name"],
      includes: [
        {
          relation: "posts",
          fields: ["id", "title"],
          includes: [
            {
              relation: "comments",
              fields: ["content"],
              includes: [{ relation: "author", fields: ["name"] }],
            },
          ],
        },
      ],
    });
    ```

## Filtered and Sorted Includes

=== "Rust"

    ```rust
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("posts")
            .with_filter(FilterExpr::eq("published", Value::Bool(true)))
            .with_order(OrderSpec::desc("created_at"))
            .with_limit(5));  // Only first 5 published posts
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("User", {
      includes: [
        {
          relation: "posts",
          filter: { field: "published", op: "eq", value: true },
          orderBy: [{ field: "created_at", direction: "desc" }],
          limit: 5,
        },
      ],
    });
    ```

## Next Steps

- **[Filtering](filtering.md)** - Filter relations and root entities
- **[Pagination](../guides/pagination.md)** - Paginate through large datasets
- **[Performance](../guides/performance.md)** - Optimize relation queries
