# Mutation API Reference

Complete reference for ORMDB mutation types.

## Mutation Enum

The main mutation type for modifying data.

```rust
pub enum Mutation {
    Insert(InsertMutation),
    Update(UpdateMutation),
    Delete(DeleteMutation),
    Upsert(UpsertMutation),
}
```

---

## InsertMutation

Insert one or more entities.

### Rust

```rust
pub struct InsertMutation {
    pub entity: String,
    pub data: Vec<EntityData>,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(entity)` | Create insert for entity type |
| `with_data(data)` | Add single entity data |
| `with_batch(data)` | Add multiple entity data |

### Example

```rust
let mutation = InsertMutation::new("User")
    .with_data(EntityData::new()
        .set("name", Value::String("Alice".into()))
        .set("email", Value::String("alice@example.com".into())));

let result = client.mutate(Mutation::Insert(mutation)).await?;
let user_id = result.inserted_ids[0];
```

### Batch Insert

```rust
let users = vec![
    EntityData::new()
        .set("name", Value::String("Alice".into()))
        .set("email", Value::String("alice@example.com".into())),
    EntityData::new()
        .set("name", Value::String("Bob".into()))
        .set("email", Value::String("bob@example.com".into())),
];

let mutation = InsertMutation::new("User").with_batch(users);
let result = client.mutate(Mutation::Insert(mutation)).await?;
// result.inserted_ids contains both IDs
```

---

## UpdateMutation

Update existing entities.

### Rust

```rust
pub struct UpdateMutation {
    pub entity: String,
    pub ids: Vec<[u8; 16]>,
    pub data: EntityData,
    pub filter: Option<Filter>,
    pub expected_version: Option<u64>,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(entity)` | Create update for entity type |
| `with_id(id)` | Update single entity by ID |
| `with_ids(ids)` | Update multiple entities by IDs |
| `with_filter(filter)` | Update entities matching filter |
| `with_data(data)` | Set fields to update |
| `with_expected_version(version)` | Optimistic concurrency check |

### Example - Update by ID

```rust
let mutation = UpdateMutation::new("User")
    .with_id(user_id)
    .with_data(EntityData::new()
        .set("name", Value::String("Alice Smith".into())));

client.mutate(Mutation::Update(mutation)).await?;
```

### Example - Update by Filter

```rust
let mutation = UpdateMutation::new("User")
    .with_filter(FilterExpr::eq("status", Value::String("pending".into())))
    .with_data(EntityData::new()
        .set("status", Value::String("active".into())));

let result = client.mutate(Mutation::Update(mutation)).await?;
println!("Updated {} users", result.affected_count);
```

### Example - Optimistic Concurrency

```rust
let mutation = UpdateMutation::new("User")
    .with_id(user_id)
    .with_expected_version(5)  // Fail if version != 5
    .with_data(EntityData::new()
        .set("balance", Value::Float64(150.0)));

match client.mutate(Mutation::Update(mutation)).await {
    Ok(_) => println!("Updated successfully"),
    Err(Error::TransactionConflict { expected, actual, .. }) => {
        println!("Conflict: expected v{}, found v{}", expected, actual);
    }
    Err(e) => return Err(e),
}
```

---

## DeleteMutation

Delete entities.

### Rust

```rust
pub struct DeleteMutation {
    pub entity: String,
    pub ids: Vec<[u8; 16]>,
    pub filter: Option<Filter>,
    pub cascade: bool,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(entity)` | Create delete for entity type |
| `with_id(id)` | Delete single entity by ID |
| `with_ids(ids)` | Delete multiple entities by IDs |
| `with_filter(filter)` | Delete entities matching filter |
| `cascade()` | Enable cascade delete |

### Example - Delete by ID

```rust
let mutation = DeleteMutation::new("User")
    .with_id(user_id);

client.mutate(Mutation::Delete(mutation)).await?;
```

### Example - Delete by Filter

```rust
let mutation = DeleteMutation::new("Session")
    .with_filter(FilterExpr::lt("expires_at", Value::Timestamp(now)));

let result = client.mutate(Mutation::Delete(mutation)).await?;
println!("Deleted {} expired sessions", result.affected_count);
```

### Example - Cascade Delete

```rust
// Delete user and all their posts, comments, etc.
let mutation = DeleteMutation::new("User")
    .with_id(user_id)
    .cascade();

client.mutate(Mutation::Delete(mutation)).await?;
```

---

## UpsertMutation

Insert or update (upsert) entities.

### Rust

```rust
pub struct UpsertMutation {
    pub entity: String,
    pub data: Vec<EntityData>,
    pub conflict_fields: Vec<String>,
    pub update_fields: Option<Vec<String>>,
}
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `new(entity)` | Create upsert for entity type |
| `with_data(data)` | Add entity data |
| `on_conflict(fields)` | Fields that determine conflict |
| `update_fields(fields)` | Fields to update on conflict |

### Example - Basic Upsert

```rust
let mutation = UpsertMutation::new("User")
    .with_data(EntityData::new()
        .set("email", Value::String("alice@example.com".into()))
        .set("name", Value::String("Alice".into()))
        .set("last_login", Value::Timestamp(now)))
    .on_conflict(vec!["email"]);

// Inserts if email doesn't exist, updates if it does
client.mutate(Mutation::Upsert(mutation)).await?;
```

### Example - Selective Update

```rust
let mutation = UpsertMutation::new("User")
    .with_data(EntityData::new()
        .set("email", Value::String("alice@example.com".into()))
        .set("name", Value::String("Alice".into()))
        .set("login_count", Value::Int32(1)))
    .on_conflict(vec!["email"])
    .update_fields(vec!["last_login"]);  // Only update last_login on conflict

client.mutate(Mutation::Upsert(mutation)).await?;
```

---

## EntityData

Container for field values.

### Rust

```rust
pub struct EntityData {
    pub fields: HashMap<String, Value>,
}
```

### Methods

| Method | Description |
|--------|-------------|
| `new()` | Create empty data |
| `set(field, value)` | Set field value (builder) |
| `get(field)` | Get field value |
| `remove(field)` | Remove field |

### Example

```rust
let data = EntityData::new()
    .set("name", Value::String("Alice".into()))
    .set("age", Value::Int32(30))
    .set("email", Value::String("alice@example.com".into()))
    .set("active", Value::Bool(true));
```

---

## MutationResult

Result of a mutation operation.

```rust
pub struct MutationResult {
    pub inserted_ids: Vec<[u8; 16]>,
    pub affected_count: usize,
    pub version: u64,
}
```

| Field | Description |
|-------|-------------|
| `inserted_ids` | IDs of inserted entities (for Insert/Upsert) |
| `affected_count` | Number of entities affected |
| `version` | New version number after mutation |

---

## Client Examples

### TypeScript

```typescript
// Insert
const result = await client.insert("User", {
  name: "Alice",
  email: "alice@example.com",
});

// Update
await client.update("User", userId, {
  name: "Alice Smith",
});

// Delete
await client.delete("User", userId);

// Upsert
await client.upsert("User", {
  data: { email: "alice@example.com", name: "Alice" },
  conflictFields: ["email"],
});
```

### Python

```python
# Insert
result = client.insert("User", {
    "name": "Alice",
    "email": "alice@example.com",
})

# Update
client.update("User", user_id, {"name": "Alice Smith"})

# Delete
client.delete("User", user_id)

# Upsert
client.upsert("User",
    data={"email": "alice@example.com", "name": "Alice"},
    conflict_fields=["email"])
```

---

## Delete Behaviors

When deleting entities with relations:

| Behavior | Description |
|----------|-------------|
| `Cascade` | Delete related entities |
| `SetNull` | Set foreign key to null |
| `Restrict` | Prevent delete if references exist |

```rust
// Defined in schema
RelationDef::new("author", "User", "Post")
    .with_delete_behavior(DeleteBehavior::Cascade)
```

---

## Transaction Support

All mutations are ACID-compliant and can be combined in transactions:

```rust
let tx = client.begin_transaction().await?;

let user = tx.insert("User", user_data).await?;
tx.insert("Profile", profile_data.set("user_id", user.id)).await?;

tx.commit().await?;
```

See [Transactions Tutorial](../tutorials/transactions.md) for details.

---

## Next Steps

- **[Query API](query-api.md)** - Fetch and filter data
- **[Mutations Tutorial](../tutorials/mutations.md)** - Learn mutation patterns
- **[Transactions Tutorial](../tutorials/transactions.md)** - Multi-operation transactions
