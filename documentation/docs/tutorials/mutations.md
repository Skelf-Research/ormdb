# Mutations

Learn how to insert, update, and delete data in ORMDB.

## Insert

### Single Insert

=== "Rust"

    ```rust
    use ormdb_proto::{Mutation, Value};

    let mutation = Mutation::insert("User")
        .with_field("name", Value::String("Alice".into()))
        .with_field("email", Value::String("alice@example.com".into()));

    let result = client.mutate(mutation).await?;
    let user_id = result.inserted_id();
    println!("Created user: {}", user_id);
    ```

=== "TypeScript"

    ```typescript
    const result = await client.insert("User", {
      name: "Alice",
      email: "alice@example.com",
    });

    console.log(`Created user: ${result.insertedIds[0]}`);
    ```

=== "Python"

    ```python
    result = client.insert("User", {
        "name": "Alice",
        "email": "alice@example.com",
    })

    print(f"Created user: {result.inserted_ids[0]}")
    ```

### Batch Insert

Insert multiple records efficiently:

=== "Rust"

    ```rust
    use ormdb_proto::MutationBatch;

    let batch = MutationBatch::new()
        .add(Mutation::insert("User")
            .with_field("name", Value::String("Alice".into()))
            .with_field("email", Value::String("alice@example.com".into())))
        .add(Mutation::insert("User")
            .with_field("name", Value::String("Bob".into()))
            .with_field("email", Value::String("bob@example.com".into())));

    let results = client.mutate_batch(batch).await?;
    ```

=== "TypeScript"

    ```typescript
    const users = [
      { name: "Alice", email: "alice@example.com" },
      { name: "Bob", email: "bob@example.com" },
    ];

    const result = await client.insertMany("User", users);
    console.log(`Created ${result.insertedIds.length} users`);
    ```

=== "Python"

    ```python
    users = [
        {"name": "Alice", "email": "alice@example.com"},
        {"name": "Bob", "email": "bob@example.com"},
    ]

    result = client.insert_many("User", users)
    print(f"Created {len(result.inserted_ids)} users")
    ```

## Update

### Update by ID

=== "Rust"

    ```rust
    let mutation = Mutation::update("User", user_id)
        .with_field("name", Value::String("Alice Smith".into()));

    client.mutate(mutation).await?;
    ```

=== "TypeScript"

    ```typescript
    await client.update("User", userId, {
      name: "Alice Smith",
    });
    ```

=== "Python"

    ```python
    client.update("User", user_id, {"name": "Alice Smith"})
    ```

### Update with Filter

Update multiple records matching a filter:

=== "Rust"

    ```rust
    let mutation = Mutation::update_where("User")
        .with_filter(FilterExpr::eq("status", Value::String("pending".into())))
        .with_field("status", Value::String("active".into()));

    let result = client.mutate(mutation).await?;
    println!("Updated {} users", result.affected_count());
    ```

=== "TypeScript"

    ```typescript
    const result = await client.updateMany(
      "User",
      { field: "status", op: "eq", value: "pending" },
      { status: "active" }
    );

    console.log(`Updated ${result.affectedCount} users`);
    ```

=== "Python"

    ```python
    result = client.update_many(
        "User",
        filter={"field": "status", "op": "eq", "value": "pending"},
        data={"status": "active"},
    )

    print(f"Updated {result.affected_count} users")
    ```

## Delete

### Delete by ID

=== "Rust"

    ```rust
    let mutation = Mutation::delete("User", user_id);
    client.mutate(mutation).await?;
    ```

=== "TypeScript"

    ```typescript
    await client.delete("User", userId);
    ```

=== "Python"

    ```python
    client.delete("User", user_id)
    ```

### Delete with Filter

=== "Rust"

    ```rust
    let mutation = Mutation::delete_where("User")
        .with_filter(FilterExpr::eq("status", Value::String("deleted".into())));

    let result = client.mutate(mutation).await?;
    println!("Deleted {} users", result.affected_count());
    ```

=== "TypeScript"

    ```typescript
    const result = await client.deleteMany("User", {
      field: "status",
      op: "eq",
      value: "deleted",
    });

    console.log(`Deleted ${result.affectedCount} users`);
    ```

=== "Python"

    ```python
    result = client.delete_many("User",
        filter={"field": "status", "op": "eq", "value": "deleted"})

    print(f"Deleted {result.affected_count} users")
    ```

## Upsert

Insert or update based on a unique key:

=== "Rust"

    ```rust
    let mutation = Mutation::upsert("User")
        .with_field("email", Value::String("alice@example.com".into()))
        .with_field("name", Value::String("Alice".into()))
        .with_field("status", Value::String("active".into()));

    let result = client.mutate(mutation).await?;
    if result.was_inserted() {
        println!("Created new user");
    } else {
        println!("Updated existing user");
    }
    ```

=== "TypeScript"

    ```typescript
    const result = await client.upsert("User", {
      email: "alice@example.com", // unique key
      name: "Alice",
      status: "active",
    });

    if (result.wasInserted) {
      console.log("Created new user");
    } else {
      console.log("Updated existing user");
    }
    ```

=== "Python"

    ```python
    result = client.upsert("User", {
        "email": "alice@example.com",  # unique key
        "name": "Alice",
        "status": "active",
    })

    if result.was_inserted:
        print("Created new user")
    else:
        print("Updated existing user")
    ```

## Cascade Behavior

When deleting entities with relations, ORMDB follows the configured delete behavior:

```
User (1) ────→ (N) Post
         cascade
```

=== "Rust"

    ```rust
    // If relation is configured with DeleteBehavior::Cascade:
    // Deleting a user also deletes their posts
    let mutation = Mutation::delete("User", user_id);
    client.mutate(mutation).await?;
    // User and all their posts are now deleted
    ```

=== "TypeScript"

    ```typescript
    // Deleting a user also deletes their posts
    await client.delete("User", userId);
    // User and all their posts are now deleted
    ```

=== "Python"

    ```python
    # Deleting a user also deletes their posts
    client.delete("User", user_id)
    # User and all their posts are now deleted
    ```

## Error Handling

### Constraint Violations

=== "Rust"

    ```rust
    use ormdb_client::Error;

    match client.mutate(mutation).await {
        Ok(result) => println!("Success"),
        Err(Error::ConstraintViolation(e)) => {
            match e {
                ConstraintError::UniqueViolation { fields, .. } => {
                    println!("Duplicate value for: {:?}", fields);
                }
                ConstraintError::ForeignKeyViolation { field, .. } => {
                    println!("Invalid reference: {}", field);
                }
                _ => println!("Constraint error: {:?}", e),
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    ```

=== "TypeScript"

    ```typescript
    try {
      await client.insert("User", { email: "existing@example.com" });
    } catch (error) {
      if (error.code === "UNIQUE_VIOLATION") {
        console.log("Email already exists");
      } else if (error.code === "FOREIGN_KEY_VIOLATION") {
        console.log("Invalid reference");
      }
    }
    ```

=== "Python"

    ```python
    from ormdb import MutationError

    try:
        client.insert("User", {"email": "existing@example.com"})
    except MutationError as e:
        if e.code == "UNIQUE_VIOLATION":
            print("Email already exists")
        elif e.code == "FOREIGN_KEY_VIOLATION":
            print("Invalid reference")
    ```

## Next Steps

- **[Transactions](transactions.md)** - Group mutations atomically
- **[Relations](relations.md)** - Working with related data
- **[Error Handling](../reference/errors.md)** - Complete error reference
