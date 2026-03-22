# Value Types Reference

Complete reference for ORMDB scalar types and the Value enum.

## Value Enum

The universal value type for all data.

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    String(String),
    Bytes(Vec<u8>),
    Uuid([u8; 16]),
    Timestamp(i64),
    Json(serde_json::Value),
}
```

---

## Scalar Types

### uuid

128-bit universally unique identifier.

| Property | Value |
|----------|-------|
| Rust type | `[u8; 16]` |
| Size | 16 bytes |
| Default | Auto-generated |

```rust
// Schema definition
FieldDef::new("id", FieldType::Uuid)

// Usage
Value::Uuid([0x12, 0x34, ...])  // Raw bytes
Value::Uuid(uuid::Uuid::new_v4().into_bytes())  // From uuid crate
```

**TypeScript:**
```typescript
// UUIDs are strings in client
const userId: string = "550e8400-e29b-41d4-a716-446655440000";
```

**Python:**
```python
import uuid
user_id = str(uuid.uuid4())
```

---

### string

Variable-length UTF-8 text.

| Property | Value |
|----------|-------|
| Rust type | `String` |
| Size | Variable |
| Max length | Configurable (default: unlimited) |

```rust
// Schema definition
FieldDef::new("name", FieldType::String)

// With max length
FieldDef::new("name", FieldType::String)
    .with_max_length(255)

// Usage
Value::String("Hello, World!".into())
```

**Indexing:** Supports equality, prefix matching (LIKE 'prefix%'), and full pattern matching.

---

### int32

32-bit signed integer.

| Property | Value |
|----------|-------|
| Rust type | `i32` |
| Size | 4 bytes |
| Range | -2,147,483,648 to 2,147,483,647 |

```rust
// Schema definition
FieldDef::new("age", FieldType::Int32)

// Usage
Value::Int32(42)
```

---

### int64

64-bit signed integer.

| Property | Value |
|----------|-------|
| Rust type | `i64` |
| Size | 8 bytes |
| Range | -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807 |

```rust
// Schema definition
FieldDef::new("view_count", FieldType::Int64)

// Usage
Value::Int64(1_000_000_000_000)
```

---

### float32

32-bit IEEE 754 floating point.

| Property | Value |
|----------|-------|
| Rust type | `f32` |
| Size | 4 bytes |
| Precision | ~7 decimal digits |

```rust
// Schema definition
FieldDef::new("rating", FieldType::Float32)

// Usage
Value::Float32(4.5)
```

---

### float64

64-bit IEEE 754 floating point.

| Property | Value |
|----------|-------|
| Rust type | `f64` |
| Size | 8 bytes |
| Precision | ~15 decimal digits |

```rust
// Schema definition
FieldDef::new("price", FieldType::Float64)

// Usage
Value::Float64(99.99)
```

**Note:** Use float64 for monetary values requiring precision, or consider using int64 with cents.

---

### bool

Boolean true/false value.

| Property | Value |
|----------|-------|
| Rust type | `bool` |
| Size | 1 byte |
| Values | `true`, `false` |

```rust
// Schema definition
FieldDef::new("active", FieldType::Bool)

// Usage
Value::Bool(true)
```

---

### bytes

Variable-length binary data.

| Property | Value |
|----------|-------|
| Rust type | `Vec<u8>` |
| Size | Variable |
| Use cases | Files, images, encrypted data |

```rust
// Schema definition
FieldDef::new("avatar", FieldType::Bytes)

// Usage
Value::Bytes(vec![0x89, 0x50, 0x4E, 0x47, ...])  // PNG header
```

**TypeScript:**
```typescript
// Bytes are base64-encoded strings
const avatar: string = "iVBORw0KGgo...";
```

**Python:**
```python
import base64
avatar = base64.b64encode(image_bytes).decode()
```

---

### timestamp

Microseconds since Unix epoch (UTC).

| Property | Value |
|----------|-------|
| Rust type | `i64` |
| Size | 8 bytes |
| Resolution | Microseconds |
| Range | ~292,000 years before/after epoch |

```rust
// Schema definition
FieldDef::new("created_at", FieldType::Timestamp)

// Usage
use std::time::{SystemTime, UNIX_EPOCH};
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_micros() as i64;
Value::Timestamp(now)
```

**TypeScript:**
```typescript
// Timestamps are ISO 8601 strings in client
const createdAt = new Date().toISOString();
// Or milliseconds
const createdAt = Date.now() * 1000; // Convert to microseconds
```

**Python:**
```python
from datetime import datetime, timezone
# ISO format
created_at = datetime.now(timezone.utc).isoformat()
# Or microseconds
import time
created_at = int(time.time() * 1_000_000)
```

---

### json

JSON document (stored as JSONB).

| Property | Value |
|----------|-------|
| Rust type | `serde_json::Value` |
| Size | Variable |
| Use cases | Flexible schema, metadata |

```rust
// Schema definition
FieldDef::new("metadata", FieldType::Json)

// Usage
Value::Json(serde_json::json!({
    "tags": ["rust", "database"],
    "views": 1000
}))
```

**TypeScript:**
```typescript
const metadata = {
  tags: ["rust", "database"],
  views: 1000,
};
```

**Note:** JSON fields support limited querying. For frequently queried data, use dedicated fields.

---

## Type Conversion

### Client Type Mappings

| ORMDB Type | TypeScript | Python | Rust |
|------------|------------|--------|------|
| uuid | `string` | `str` | `[u8; 16]` |
| string | `string` | `str` | `String` |
| int32 | `number` | `int` | `i32` |
| int64 | `number` \| `bigint` | `int` | `i64` |
| float32 | `number` | `float` | `f32` |
| float64 | `number` | `float` | `f64` |
| bool | `boolean` | `bool` | `bool` |
| bytes | `string` (base64) | `bytes` | `Vec<u8>` |
| timestamp | `string` \| `number` | `datetime` \| `int` | `i64` |
| json | `object` | `dict` | `serde_json::Value` |

### Automatic Coercion

The server performs automatic type coercion where safe:

| From | To | Notes |
|------|-----|-------|
| int32 | int64 | Widening |
| int32 | float64 | Precision loss possible |
| int64 | float64 | Precision loss possible |
| float32 | float64 | Widening |
| string | timestamp | ISO 8601 parsing |

---

## Nullable Fields

All fields can be nullable:

```rust
// Schema definition
FieldDef::new("middle_name", FieldType::String)
    .nullable()

// Usage
Value::Null
Value::String("William".into())
```

**Querying null values:**
```rust
FilterExpr::is_null("deleted_at")
FilterExpr::is_not_null("email")
```

---

## Default Values

Fields can have default values:

```rust
FieldDef::new("status", FieldType::String)
    .with_default(Value::String("pending".into()))

FieldDef::new("created_at", FieldType::Timestamp)
    .with_default_fn(DefaultFn::Now)

FieldDef::new("id", FieldType::Uuid)
    .with_default_fn(DefaultFn::Uuid)
```

### DefaultFn Options

| Function | Description |
|----------|-------------|
| `Now` | Current timestamp |
| `Uuid` | Generate UUID v4 |
| `Zero` | Zero value for type |

---

## Validation

### Built-in Validators

```rust
// String length
FieldDef::new("username", FieldType::String)
    .with_min_length(3)
    .with_max_length(50)

// Numeric range
FieldDef::new("age", FieldType::Int32)
    .with_min(0)
    .with_max(150)

// Pattern matching
FieldDef::new("email", FieldType::String)
    .with_pattern(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
```

### Check Constraints

```rust
FieldDef::new("price", FieldType::Float64)
    .with_check("price >= 0")
```

---

## Wire Format

Values are serialized using a compact binary format:

| Type | Format |
|------|--------|
| Null | 1 byte (tag only) |
| Bool | 2 bytes (tag + value) |
| Int32 | 5 bytes (tag + 4 bytes LE) |
| Int64 | 9 bytes (tag + 8 bytes LE) |
| Float32 | 5 bytes (tag + 4 bytes IEEE) |
| Float64 | 9 bytes (tag + 8 bytes IEEE) |
| String | tag + varint length + UTF-8 bytes |
| Bytes | tag + varint length + raw bytes |
| Uuid | 17 bytes (tag + 16 bytes) |
| Timestamp | 9 bytes (tag + 8 bytes LE) |
| Json | tag + varint length + UTF-8 JSON |

---

## Next Steps

- **[Query API](query-api.md)** - Use values in queries
- **[Mutation API](mutation-api.md)** - Use values in mutations
- **[Schema Design Tutorial](../tutorials/schema-design.md)** - Design entity fields
