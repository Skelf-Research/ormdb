# Error Reference

Complete reference for ORMDB error types.

## Error Enum

```rust
pub enum Error {
    // Storage errors
    Storage(sled::Error),
    Protocol(ormdb_proto::Error),

    // Serialization
    Serialization(String),
    Deserialization(String),

    // Key/data errors
    InvalidKey,
    NotFound,
    InvalidData(String),

    // Transaction errors
    Transaction(String),
    TransactionConflict {
        entity_id: [u8; 16],
        expected: u64,
        actual: u64,
    },

    // Constraint errors
    ConstraintViolation(ConstraintError),
    CascadeError(CascadeError),
}
```

---

## ConstraintError

Constraint violation errors.

### UniqueViolation

Duplicate value for unique constraint.

```rust
ConstraintError::UniqueViolation {
    constraint: String,      // Constraint name
    entity: String,          // Entity type
    fields: Vec<String>,     // Fields in constraint
    value: String,           // Duplicate value
}
```

**Example:**
```
UniqueViolation {
    constraint: "User_email_unique",
    entity: "User",
    fields: ["email"],
    value: "alice@example.com"
}
```

**Handling:**
```rust
Err(Error::ConstraintViolation(ConstraintError::UniqueViolation { fields, .. })) => {
    println!("Duplicate value for: {:?}", fields);
}
```

### ForeignKeyViolation

Referenced entity doesn't exist.

```rust
ConstraintError::ForeignKeyViolation {
    constraint: String,          // Constraint name
    entity: String,              // Entity with FK
    field: String,               // FK field name
    referenced_entity: String,   // Referenced entity type
}
```

**Example:**
```
ForeignKeyViolation {
    constraint: "Post_author_fk",
    entity: "Post",
    field: "author_id",
    referenced_entity: "User"
}
```

### CheckViolation

Check constraint failed.

```rust
ConstraintError::CheckViolation {
    constraint: String,    // Constraint name
    entity: String,        // Entity type
    expression: String,    // Check expression
}
```

**Example:**
```
CheckViolation {
    constraint: "Order_amount_positive",
    entity: "Order",
    expression: "amount > 0"
}
```

### RestrictViolation

Cannot delete due to referencing entities.

```rust
ConstraintError::RestrictViolation {
    constraint: String,           // Constraint name
    entity: String,               // Entity being deleted
    referencing_entity: String,   // Entity with references
    count: usize,                 // Number of references
}
```

**Example:**
```
RestrictViolation {
    constraint: "Team_members_restrict",
    entity: "Team",
    referencing_entity: "TeamMember",
    count: 5
}
```

---

## CascadeError

Delete cascade errors.

### RestrictViolation

Cascade blocked by restrict constraint.

```rust
CascadeError::RestrictViolation {
    entity: String,
    referencing_entity: String,
    count: usize,
}
```

### CircularCascade

Circular reference detected in cascade.

```rust
CascadeError::CircularCascade {
    path: Vec<String>,  // Entity types in cycle
}
```

### MaxDepthExceeded

Cascade exceeded maximum depth.

```rust
CascadeError::MaxDepthExceeded {
    depth: usize,
}
```

---

## TransactionConflict

Optimistic concurrency conflict.

```rust
Error::TransactionConflict {
    entity_id: [u8; 16],  // Entity that conflicted
    expected: u64,        // Expected version
    actual: u64,          // Actual version found
}
```

**Handling:**
```rust
Err(Error::TransactionConflict { entity_id, expected, actual }) => {
    println!("Conflict: expected v{}, found v{}", expected, actual);
    // Retry with fresh data
}
```

---

## Client Error Codes

HTTP/JSON clients receive error codes:

| Code | Description |
|------|-------------|
| `CONNECTION_ERROR` | Cannot connect to server |
| `TIMEOUT_ERROR` | Request timed out |
| `QUERY_ERROR` | Query execution failed |
| `MUTATION_ERROR` | Mutation failed |
| `VALIDATION_ERROR` | Invalid request data |
| `SCHEMA_ERROR` | Schema-related error |
| `UNIQUE_VIOLATION` | Unique constraint violated |
| `FOREIGN_KEY_VIOLATION` | Foreign key violated |
| `CHECK_VIOLATION` | Check constraint violated |
| `RESTRICT_VIOLATION` | Delete restricted |
| `VERSION_CONFLICT` | Optimistic concurrency conflict |
| `NOT_FOUND` | Entity not found |
| `AUTHENTICATION_FAILED` | Invalid credentials or missing authentication |
| `PERMISSION_DENIED` | Insufficient permissions for operation |
| `BUDGET_EXCEEDED` | Query budget exceeded (depth, entities, or edges limit) |
| `SCHEMA_MISMATCH` | Client schema version doesn't match server |
| `INTERNAL` | Internal server error (details logged server-side) |

### Security Error Codes

Security-related errors provide safe messages to clients while logging details server-side:

| Code | Client Message | Server Log |
|------|----------------|------------|
| `AUTHENTICATION_FAILED` | "authentication failed" | Full auth error details |
| `PERMISSION_DENIED` | "permission denied: [reason]" | Full capability info |
| `BUDGET_EXCEEDED` | "budget exceeded: [reason]" | Query depth/entity counts |
| `INTERNAL` | "database operation failed" | Full stack trace |

**Note:** Internal errors are sanitized to prevent information disclosure. Check server logs for detailed error information.

---

## Error Handling Examples

### Rust

```rust
use ormdb_core::error::{Error, ConstraintError};

match result {
    Ok(data) => process(data),
    Err(Error::NotFound) => {
        println!("Entity not found");
    }
    Err(Error::ConstraintViolation(ConstraintError::UniqueViolation { fields, .. })) => {
        println!("Duplicate: {:?}", fields);
    }
    Err(Error::TransactionConflict { .. }) => {
        // Retry with fresh data
    }
    Err(e) => {
        println!("Error: {}", e);
    }
}
```

### TypeScript

```typescript
try {
  await client.insert("User", data);
} catch (error) {
  if (error.code === "UNIQUE_VIOLATION") {
    // Handle duplicate
  } else if (error.code === "FOREIGN_KEY_VIOLATION") {
    // Handle invalid reference
  } else if (error.code === "VERSION_CONFLICT") {
    // Retry with fresh data
  } else {
    throw error;
  }
}
```

### Python

```python
from ormdb import MutationError

try:
    client.insert("User", data)
except MutationError as e:
    if e.code == "UNIQUE_VIOLATION":
        # Handle duplicate
        pass
    elif e.code == "FOREIGN_KEY_VIOLATION":
        # Handle invalid reference
        pass
    else:
        raise
```

---

## Next Steps

- **[Troubleshooting](../operations/troubleshooting.md)** - Diagnose common issues
- **[Transactions Tutorial](../tutorials/transactions.md)** - Handle conflicts properly
- **[CLI Reference](cli.md)** - Admin commands for diagnostics
