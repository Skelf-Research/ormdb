# API Reference

Complete reference documentation for ORMDB APIs.

## Core APIs

| Reference | Description |
|-----------|-------------|
| [Query API](query-api.md) | GraphQuery, FilterExpr, OrderSpec, Pagination |
| [Mutation API](mutation-api.md) | Insert, Update, Delete, Upsert operations |
| [Value Types](value-types.md) | Scalar types and the Value enum |
| [Errors](errors.md) | Error types and handling |
| [Configuration](configuration.md) | Server and client configuration options |
| [CLI](cli.md) | Command-line interface reference |

## Quick Reference

### Filter Operators

| Operator | Description |
|----------|-------------|
| `eq` | Equals |
| `ne` | Not equals |
| `lt` | Less than |
| `le` | Less than or equal |
| `gt` | Greater than |
| `ge` | Greater than or equal |
| `like` | Pattern match (case sensitive) |
| `ilike` | Pattern match (case insensitive) |
| `in` | In list |
| `not_in` | Not in list |
| `is_null` | Is null |
| `is_not_null` | Is not null |
| `and` | Logical AND |
| `or` | Logical OR |
| `not` | Logical NOT |

### Scalar Types

| Type | Rust | Size | Description |
|------|------|------|-------------|
| `uuid` | `[u8; 16]` | 16 bytes | 128-bit UUID |
| `string` | `String` | variable | UTF-8 text |
| `int32` | `i32` | 4 bytes | 32-bit signed integer |
| `int64` | `i64` | 8 bytes | 64-bit signed integer |
| `float32` | `f32` | 4 bytes | 32-bit IEEE 754 |
| `float64` | `f64` | 8 bytes | 64-bit IEEE 754 |
| `bool` | `bool` | 1 byte | true/false |
| `bytes` | `Vec<u8>` | variable | Binary data |
| `timestamp` | `i64` | 8 bytes | Microseconds since epoch |

### Aggregate Functions

| Function | Description |
|----------|-------------|
| `COUNT` | Count entities (or non-null field values) |
| `SUM` | Sum of numeric field |
| `AVG` | Average of numeric field |
| `MIN` | Minimum value |
| `MAX` | Maximum value |
