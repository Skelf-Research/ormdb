# Security Guide

Comprehensive security features for ORMDB applications.

## Overview

ORMDB provides multiple layers of security:

1. **Row-Level Security (RLS)** - Filter data access per user
2. **Capability Tokens** - Fine-grained operation permissions
3. **Field Masking** - Hide or redact sensitive fields
4. **Audit Logging** - Track all data access

## Row-Level Security (RLS)

### Enabling RLS

```toml
# ormdb.toml
[security]
enable_rls = true
```

### Defining Policies

Policies filter which rows a user can access:

```rust
// Users can only see their own data
let policy = RlsPolicy::new("User", "user_own_data")
    .with_filter(|ctx| {
        FilterExpr::eq("id", ctx.user_id())
    });

// Users can see posts from users they follow
let policy = RlsPolicy::new("Post", "followed_posts")
    .with_filter(|ctx| {
        FilterExpr::in_subquery("author_id",
            "SELECT followed_id FROM Follow WHERE follower_id = ?",
            vec![ctx.user_id()])
    });

schema.add_policy(policy);
```

### Policy Types

| Type | Description |
|------|-------------|
| `select` | Filter read queries |
| `insert` | Validate inserts |
| `update` | Filter and validate updates |
| `delete` | Filter deletes |

```rust
// Read-only policy
let policy = RlsPolicy::new("User", "public_profiles")
    .select_only()
    .with_filter(|_| FilterExpr::eq("public", Value::Bool(true)));

// Full access policy
let policy = RlsPolicy::new("Post", "own_posts")
    .all_operations()
    .with_filter(|ctx| FilterExpr::eq("author_id", ctx.user_id()));
```

### Context Variables

Policies have access to request context:

```rust
let policy = RlsPolicy::new("Order", "tenant_orders")
    .with_filter(|ctx| {
        FilterExpr::and(vec![
            FilterExpr::eq("tenant_id", ctx.get("tenant_id")),
            FilterExpr::or(vec![
                FilterExpr::eq("user_id", ctx.user_id()),
                FilterExpr::eq("role", ctx.get("role")),
            ])
        ])
    });
```

### Client Usage

=== "Rust"

    ```rust
    let ctx = SecurityContext::new()
        .with_user_id(user_id)
        .with_claim("tenant_id", tenant_id)
        .with_claim("role", "admin");

    let result = client.query_with_context(query, ctx).await?;
    ```

=== "TypeScript"

    ```typescript
    const result = await client.query("Post", {
      context: {
        userId: userId,
        claims: {
          tenant_id: tenantId,
          role: "admin",
        },
      },
    });
    ```

=== "Python"

    ```python
    result = client.query("Post",
        context={
            "user_id": user_id,
            "claims": {
                "tenant_id": tenant_id,
                "role": "admin",
            },
        })
    ```

## Capability Tokens

Fine-grained access control using cryptographic tokens.

### Token Structure

```rust
pub struct CapabilityToken {
    pub entity: String,
    pub operations: Vec<Operation>,
    pub filter: Option<Filter>,
    pub fields: Option<Vec<String>>,
    pub expires_at: i64,
    pub signature: [u8; 32],
}

pub enum Operation {
    Read,
    Insert,
    Update,
    Delete,
}
```

### Creating Tokens

=== "Rust"

    ```rust
    // Read-only token for specific user's posts
    let token = CapabilityToken::new("Post")
        .allow_read()
        .with_filter(FilterExpr::eq("author_id", user_id))
        .expires_in(Duration::from_secs(3600))
        .sign(&secret_key)?;

    // Limited field access
    let token = CapabilityToken::new("User")
        .allow_read()
        .with_fields(vec!["id", "name", "avatar_url"])  // No email
        .expires_in(Duration::from_secs(3600))
        .sign(&secret_key)?;
    ```

=== "TypeScript"

    ```typescript
    const token = await client.createCapabilityToken({
      entity: "Post",
      operations: ["read"],
      filter: { field: "author_id", op: "eq", value: userId },
      expiresIn: 3600,
    });
    ```

### Using Tokens

```rust
let result = client.query_with_token(query, token).await?;
```

### Token Validation

```toml
# ormdb.toml
[security]
capability_check = true
max_token_age_seconds = 3600
```

## Field Masking

Hide or redact sensitive fields based on context.

### Defining Masks

```rust
// Completely hide field
let mask = FieldMask::new("User", "password_hash")
    .hide();

// Redact to fixed value
let mask = FieldMask::new("User", "ssn")
    .redact("***-**-****");

// Partial mask
let mask = FieldMask::new("User", "email")
    .partial_mask(|value| {
        let email = value.as_str()?;
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() == 2 {
            let masked = format!("{}***@{}", &parts[0][..2], parts[1]);
            Some(Value::String(masked))
        } else {
            None
        }
    });

// Conditional mask
let mask = FieldMask::new("User", "phone")
    .when(|ctx| ctx.get("role") != "admin")
    .redact("(***) ***-****");

schema.add_mask(mask);
```

### Mask Types

| Type | Description | Example |
|------|-------------|---------|
| `hide` | Field not returned | Field omitted from response |
| `redact` | Fixed replacement | `"***-**-****"` |
| `partial` | Custom transformation | `"jo***@example.com"` |
| `hash` | One-way hash | `"a1b2c3..."` |

### Client Usage

```typescript
// Without permissions: email is masked
const user = await client.query("User", {
  filter: { field: "id", op: "eq", value: userId },
});
// user.email = "al***@example.com"

// With admin context: email is visible
const user = await client.query("User", {
  filter: { field: "id", op: "eq", value: userId },
  context: { role: "admin" },
});
// user.email = "alice@example.com"
```

## Audit Logging

Track all data access and modifications.

### Enabling Audit Logs

```toml
# ormdb.toml
[security]
audit_logging = true
audit_log_path = "/var/log/ormdb/audit.log"
```

### Audit Log Format

```json
{
  "timestamp": "2024-01-15T12:00:00Z",
  "event": "query",
  "user_id": "550e8400-...",
  "entity": "User",
  "operation": "read",
  "filter": {"field": "status", "op": "eq", "value": "active"},
  "result_count": 42,
  "duration_ms": 5,
  "ip_address": "192.168.1.100"
}
```

### Audit Events

| Event | Description |
|-------|-------------|
| `query` | Data read |
| `insert` | Entity created |
| `update` | Entity modified |
| `delete` | Entity deleted |
| `schema_change` | Schema modified |
| `auth_success` | Successful authentication |
| `auth_failure` | Failed authentication |
| `policy_violation` | RLS policy blocked access |

### Querying Audit Logs

```bash
# Recent events
ormdb admin audit --since 1h

# Specific user
ormdb admin audit --user-id 550e8400-...

# Specific entity
ormdb admin audit --entity User --operation delete
```

## Authentication Integration

### JWT Authentication

```rust
// Validate JWT and extract claims
let claims = validate_jwt(&token, &public_key)?;

let ctx = SecurityContext::new()
    .with_user_id(claims.sub)
    .with_claims(claims.custom);

client.query_with_context(query, ctx).await?;
```

### API Key Authentication

```rust
// Middleware example
async fn auth_middleware(req: Request, client: &Client) -> Result<SecurityContext> {
    let api_key = req.header("X-API-Key")?;
    let key_info = client.validate_api_key(api_key).await?;

    Ok(SecurityContext::new()
        .with_user_id(key_info.user_id)
        .with_claims(key_info.permissions))
}
```

## Best Practices

### 1. Principle of Least Privilege

```rust
// Bad: Broad access
let token = CapabilityToken::new("User")
    .allow_all();

// Good: Specific access
let token = CapabilityToken::new("User")
    .allow_read()
    .with_fields(vec!["id", "name", "avatar_url"])
    .with_filter(FilterExpr::eq("public", true));
```

### 2. Always Use RLS for Multi-Tenant Apps

```rust
// Ensure tenant isolation
let policy = RlsPolicy::new("*", "tenant_isolation")
    .all_operations()
    .with_filter(|ctx| FilterExpr::eq("tenant_id", ctx.get("tenant_id")));
```

### 3. Mask Sensitive Data by Default

```rust
// Mask PII fields
for field in ["email", "phone", "ssn", "address"] {
    schema.add_mask(FieldMask::new("User", field)
        .when(|ctx| !ctx.has_permission("view_pii"))
        .partial_mask(mask_pii));
}
```

### 4. Rotate Capability Tokens

```rust
// Short-lived tokens
let token = CapabilityToken::new("Post")
    .expires_in(Duration::from_secs(300))  // 5 minutes
    .sign(&secret_key)?;
```

### 5. Monitor Audit Logs

```bash
# Alert on suspicious patterns
ormdb admin audit --since 1h \
    --filter 'result_count > 1000 OR operation = "delete"' \
    --alert slack
```

## Security Checklist

- [ ] Enable RLS for multi-tenant applications
- [ ] Define policies for all sensitive entities
- [ ] Use capability tokens for external API access
- [ ] Mask PII fields (email, phone, SSN, etc.)
- [ ] Enable audit logging in production
- [ ] Rotate secrets and tokens regularly
- [ ] Use HTTPS/TLS for all connections
- [ ] Validate all user input
- [ ] Review security policies periodically

---

## Next Steps

- **[Multi-Tenant SaaS Example](../examples/multi-tenant-saas.md)** - Complete security implementation
- **[Operations Guide](../operations/index.md)** - Production deployment
- **[Performance Guide](performance.md)** - Optimize secure queries
