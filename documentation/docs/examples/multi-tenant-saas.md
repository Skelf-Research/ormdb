# Multi-Tenant SaaS Example

A complete SaaS template demonstrating tenant isolation, user management, and security patterns.

---

## Overview

This example builds a multi-tenant SaaS platform with:
- Complete tenant isolation
- Capability-based access control
- Field masking for sensitive data
- Safe schema migrations
- Subscription management

---

## Schema

```ormdb
// schema.ormdb

// Organization (Tenant)
entity Organization {
    id: uuid @id @default(uuid())
    name: string
    slug: string @unique
    plan: string @default("free")  // free, pro, enterprise
    settings: json?
    created_at: timestamp @default(now())

    members: OrganizationMember[] @relation(field: org_id)
    projects: Project[] @relation(field: org_id)
    api_keys: ApiKey[] @relation(field: org_id)

    @index(plan)
}

entity OrganizationMember {
    id: uuid @id @default(uuid())
    org_id: uuid
    user_id: uuid
    role: string @default("member")  // owner, admin, member, viewer
    invited_at: timestamp @default(now())
    joined_at: timestamp?

    organization: Organization @relation(field: org_id, references: id)
    user: User @relation(field: user_id, references: id)

    @unique([org_id, user_id])
    @index(org_id)
    @index(user_id)
}

entity User {
    id: uuid @id @default(uuid())
    email: string @unique
    password_hash: string @masked
    name: string
    avatar_url: string?
    email_verified: bool @default(false)
    created_at: timestamp @default(now())

    memberships: OrganizationMember[] @relation(field: user_id)

    @index(email)
}

entity Project {
    id: uuid @id @default(uuid())
    org_id: uuid
    name: string
    description: string?
    status: string @default("active")
    created_at: timestamp @default(now())
    created_by: uuid

    organization: Organization @relation(field: org_id, references: id)
    creator: User @relation(field: created_by, references: id)
    tasks: Task[] @relation(field: project_id)

    @index(org_id)
    @index(status)
}

entity Task {
    id: uuid @id @default(uuid())
    project_id: uuid
    title: string
    description: string?
    status: string @default("todo")  // todo, in_progress, done
    priority: int32 @default(0)
    assignee_id: uuid?
    due_date: timestamp?
    created_at: timestamp @default(now())
    completed_at: timestamp?

    project: Project @relation(field: project_id, references: id)
    assignee: User? @relation(field: assignee_id, references: id)

    @index(project_id)
    @index(status)
    @index(assignee_id)
}

entity ApiKey {
    id: uuid @id @default(uuid())
    org_id: uuid
    name: string
    key_hash: string @masked
    key_prefix: string  // First 8 chars for identification
    permissions: string[]
    last_used_at: timestamp?
    expires_at: timestamp?
    created_at: timestamp @default(now())

    organization: Organization @relation(field: org_id, references: id)

    @index(org_id)
    @index(key_prefix)
}

// Audit Log
entity AuditLog {
    id: uuid @id @default(uuid())
    org_id: uuid
    user_id: uuid?
    action: string
    entity_type: string
    entity_id: uuid?
    details: json?
    ip_address: string?
    user_agent: string?
    created_at: timestamp @default(now())

    @index(org_id)
    @index(user_id)
    @index(created_at)
}

// ============================================
// Security Policies
// ============================================

// Tenant Isolation - Users can only see their org's data
policy TenantIsolation {
    entity: [Project, Task, ApiKey, AuditLog]
    action: [read, create, update, delete]
    condition: org_id IN $user_org_ids
}

// Organization visibility
policy OrgMemberAccess {
    entity: Organization
    action: read
    condition: id IN $user_org_ids
}

// Only admins can manage organization settings
policy OrgAdminManagement {
    entity: Organization
    action: update
    condition: id IN $user_admin_org_ids
}

// Project access based on membership
policy ProjectAccess {
    entity: Project
    action: [create, update, delete]
    condition: org_id IN $user_member_org_ids
}

// Task assignment
policy TaskAssignment {
    entity: Task
    action: update
    condition: project.org_id IN $user_member_org_ids
}
```

---

## Security Context Setup

```rust
// src/security.rs
use ormdb_core::{Database, SecurityContext, Capability};
use uuid::Uuid;

#[derive(Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub org_ids: Vec<Uuid>,           // All orgs user belongs to
    pub admin_org_ids: Vec<Uuid>,     // Orgs where user is admin/owner
    pub member_org_ids: Vec<Uuid>,    // Orgs where user can create content
    pub current_org_id: Option<Uuid>, // Active organization
}

impl AuthContext {
    pub async fn from_user(db: &Database, user_id: Uuid) -> Self {
        // Fetch all memberships
        let query = GraphQuery::new("OrganizationMember")
            .filter(FilterExpr::eq("user_id", Value::Uuid(user_id.into_bytes())))
            .include(Include::new("organization"));

        let result = db.query(query).await.unwrap();

        let mut org_ids = Vec::new();
        let mut admin_org_ids = Vec::new();
        let mut member_org_ids = Vec::new();

        for membership in result.entities() {
            let org_id: Uuid = membership.get("org_id").unwrap();
            let role: String = membership.get("role").unwrap();

            org_ids.push(org_id);

            match role.as_str() {
                "owner" | "admin" => {
                    admin_org_ids.push(org_id);
                    member_org_ids.push(org_id);
                }
                "member" => {
                    member_org_ids.push(org_id);
                }
                _ => {} // viewers can only read
            }
        }

        Self {
            user_id,
            org_ids,
            admin_org_ids,
            member_org_ids,
            current_org_id: None,
        }
    }

    pub fn with_org(mut self, org_id: Uuid) -> Self {
        self.current_org_id = Some(org_id);
        self
    }

    pub fn to_security_context(&self) -> SecurityContext {
        let mut ctx = SecurityContext::new()
            .set("user_id", self.user_id)
            .set_array("user_org_ids", &self.org_ids)
            .set_array("user_admin_org_ids", &self.admin_org_ids)
            .set_array("user_member_org_ids", &self.member_org_ids);

        if let Some(org_id) = self.current_org_id {
            ctx = ctx.set("current_org_id", org_id);
        }

        ctx
    }
}

/// Execute a database operation with tenant context
pub async fn with_tenant<T, F, Fut>(
    db: &Database,
    auth: &AuthContext,
    f: F,
) -> Result<T, Error>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    db.with_security_context(auth.to_security_context(), f).await
}
```

---

## Capability-Based Access

```rust
// src/capabilities.rs
use ormdb_core::Capability;

/// Define capabilities for different roles
pub fn capabilities_for_role(role: &str) -> Vec<Capability> {
    match role {
        "owner" => vec![
            Capability::Read("*".into()),
            Capability::Write("*".into()),
            Capability::Delete("*".into()),
            Capability::ManageMembers,
            Capability::ManageBilling,
            Capability::ManageApiKeys,
            Capability::ViewAuditLog,
        ],
        "admin" => vec![
            Capability::Read("*".into()),
            Capability::Write("*".into()),
            Capability::Delete("*".into()),
            Capability::ManageMembers,
            Capability::ManageApiKeys,
            Capability::ViewAuditLog,
        ],
        "member" => vec![
            Capability::Read("*".into()),
            Capability::Write("Project".into()),
            Capability::Write("Task".into()),
            Capability::Delete("Task".into()),
        ],
        "viewer" => vec![
            Capability::Read("*".into()),
        ],
        _ => vec![],
    }
}

/// Check if user has capability
pub fn has_capability(auth: &AuthContext, org_id: Uuid, cap: &Capability) -> bool {
    // Get user's role in this org
    // (In practice, cache this in AuthContext)
    let role = get_user_role_in_org(auth.user_id, org_id);
    let capabilities = capabilities_for_role(&role);
    capabilities.contains(cap)
}
```

---

## API Implementation

### Organization Management

```rust
// src/handlers/organizations.rs

/// POST /organizations
pub async fn create_organization(
    State(db): State<Db>,
    auth: AuthContext,
    Json(input): Json<CreateOrgInput>,
) -> Result<(StatusCode, Json<Organization>), AppError> {
    let org_id = Uuid::new_v4();
    let slug = slugify(&input.name);

    // Create organization
    db.mutate(
        Mutation::create("Organization")
            .set("id", Value::Uuid(org_id.into_bytes()))
            .set("name", Value::String(input.name))
            .set("slug", Value::String(slug))
    ).await?;

    // Add creator as owner
    db.mutate(
        Mutation::create("OrganizationMember")
            .set("id", Value::Uuid(Uuid::new_v4().into_bytes()))
            .set("org_id", Value::Uuid(org_id.into_bytes()))
            .set("user_id", Value::Uuid(auth.user_id.into_bytes()))
            .set("role", Value::String("owner".into()))
            .set("joined_at", Value::Timestamp(Utc::now().timestamp_micros()))
    ).await?;

    // Log audit event
    audit_log(&db, org_id, auth.user_id, "organization.created", None).await;

    let org = get_organization(&db, org_id).await?;
    Ok((StatusCode::CREATED, Json(org)))
}

/// POST /organizations/:org_id/members
pub async fn invite_member(
    State(db): State<Db>,
    auth: AuthContext,
    Path(org_id): Path<Uuid>,
    Json(input): Json<InviteMemberInput>,
) -> Result<Json<OrganizationMember>, AppError> {
    // Check permission
    if !has_capability(&auth, org_id, &Capability::ManageMembers) {
        return Err(AppError::Forbidden);
    }

    // Find user by email
    let user = find_user_by_email(&db, &input.email).await
        .ok_or(AppError::NotFound("User not found"))?;

    // Check not already a member
    let existing = find_membership(&db, org_id, user.id).await;
    if existing.is_some() {
        return Err(AppError::Conflict("User is already a member"));
    }

    // Create membership
    let member_id = Uuid::new_v4();
    db.mutate(
        Mutation::create("OrganizationMember")
            .set("id", Value::Uuid(member_id.into_bytes()))
            .set("org_id", Value::Uuid(org_id.into_bytes()))
            .set("user_id", Value::Uuid(user.id.into_bytes()))
            .set("role", Value::String(input.role.unwrap_or("member".into())))
    ).await?;

    // Log audit event
    audit_log(&db, org_id, auth.user_id, "member.invited", Some(json!({
        "invited_user_id": user.id,
        "role": input.role,
    }))).await;

    let member = get_membership(&db, member_id).await?;
    Ok(Json(member))
}

/// DELETE /organizations/:org_id/members/:user_id
pub async fn remove_member(
    State(db): State<Db>,
    auth: AuthContext,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    // Check permission
    if !has_capability(&auth, org_id, &Capability::ManageMembers) {
        return Err(AppError::Forbidden);
    }

    // Can't remove the last owner
    let owners = count_owners(&db, org_id).await?;
    let membership = find_membership(&db, org_id, user_id).await
        .ok_or(AppError::NotFound("Membership not found"))?;

    if membership.role == "owner" && owners <= 1 {
        return Err(AppError::BadRequest("Cannot remove the last owner"));
    }

    // Delete membership
    with_tenant(&db, &auth, || async {
        db.mutate(
            Mutation::delete("OrganizationMember")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                    FilterExpr::eq("user_id", Value::Uuid(user_id.into_bytes())),
                ]))
        ).await
    }).await?;

    // Log audit event
    audit_log(&db, org_id, auth.user_id, "member.removed", Some(json!({
        "removed_user_id": user_id,
    }))).await;

    Ok(StatusCode::NO_CONTENT)
}
```

### API Key Management

```rust
// src/handlers/api_keys.rs
use argon2::{Argon2, PasswordHasher};
use rand::Rng;

/// POST /organizations/:org_id/api-keys
pub async fn create_api_key(
    State(db): State<Db>,
    auth: AuthContext,
    Path(org_id): Path<Uuid>,
    Json(input): Json<CreateApiKeyInput>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    // Check permission
    if !has_capability(&auth, org_id, &Capability::ManageApiKeys) {
        return Err(AppError::Forbidden);
    }

    // Generate key
    let key = generate_api_key();
    let key_prefix = &key[..8];
    let key_hash = hash_api_key(&key);

    let key_id = Uuid::new_v4();

    db.mutate(
        Mutation::create("ApiKey")
            .set("id", Value::Uuid(key_id.into_bytes()))
            .set("org_id", Value::Uuid(org_id.into_bytes()))
            .set("name", Value::String(input.name))
            .set("key_hash", Value::String(key_hash))
            .set("key_prefix", Value::String(key_prefix.to_string()))
            .set("permissions", Value::StringArray(input.permissions))
            .set_opt("expires_at", input.expires_at.map(|t| Value::Timestamp(t.timestamp_micros())))
    ).await?;

    // Log audit event
    audit_log(&db, org_id, auth.user_id, "api_key.created", Some(json!({
        "key_id": key_id,
        "key_prefix": key_prefix,
    }))).await;

    // Return the key only once - it won't be retrievable again
    Ok(Json(ApiKeyResponse {
        id: key_id,
        name: input.name,
        key,  // Full key, only shown once
        key_prefix: key_prefix.to_string(),
        permissions: input.permissions,
        created_at: Utc::now(),
    }))
}

fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    format!("ormdb_{}", base64::encode_config(bytes, base64::URL_SAFE_NO_PAD))
}

fn hash_api_key(key: &str) -> String {
    let salt = argon2::password_hash::SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();
    argon2.hash_password(key.as_bytes(), &salt)
        .unwrap()
        .to_string()
}
```

### Audit Logging

```rust
// src/audit.rs

pub async fn audit_log(
    db: &Database,
    org_id: Uuid,
    user_id: Uuid,
    action: &str,
    details: Option<serde_json::Value>,
) {
    let _ = db.mutate(
        Mutation::create("AuditLog")
            .set("id", Value::Uuid(Uuid::new_v4().into_bytes()))
            .set("org_id", Value::Uuid(org_id.into_bytes()))
            .set("user_id", Value::Uuid(user_id.into_bytes()))
            .set("action", Value::String(action.into()))
            .set_opt("details", details.map(|d| Value::Json(d.to_string())))
    ).await;
}

/// GET /organizations/:org_id/audit-log
pub async fn list_audit_log(
    State(db): State<Db>,
    auth: AuthContext,
    Path(org_id): Path<Uuid>,
    Query(params): Query<AuditLogParams>,
) -> Result<Json<Vec<AuditLogEntry>>, AppError> {
    // Check permission
    if !has_capability(&auth, org_id, &Capability::ViewAuditLog) {
        return Err(AppError::Forbidden);
    }

    let mut query = GraphQuery::new("AuditLog")
        .filter(FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())))
        .order_by(OrderSpec::desc("created_at"))
        .include(Include::new("user").fields(["id", "name", "email"]))
        .limit(params.limit.unwrap_or(50));

    if let Some(action) = params.action {
        query = query.filter(FilterExpr::eq("action", Value::String(action)));
    }

    if let Some(user_id) = params.user_id {
        query = query.filter(FilterExpr::eq("user_id", Value::Uuid(user_id.into_bytes())));
    }

    if let Some(since) = params.since {
        query = query.filter(FilterExpr::gte("created_at", Value::Timestamp(since.timestamp_micros())));
    }

    let result = with_tenant(&db, &auth.with_org(org_id), || async {
        db.query(query).await
    }).await?;

    let entries: Vec<AuditLogEntry> = result.entities().map(Into::into).collect();
    Ok(Json(entries))
}
```

---

## Field Masking

Sensitive fields are automatically masked based on context:

```rust
// Password hash is never returned
let user = db.query(GraphQuery::new("User").filter(...)).await?;
assert!(user.get("password_hash").is_none());

// API key hash is masked
let api_key = db.query(GraphQuery::new("ApiKey").filter(...)).await?;
assert!(api_key.get("key_hash").is_none());
```

To access masked fields (e.g., for authentication), use elevated context:

```rust
pub async fn verify_api_key(db: &Database, key: &str) -> Option<ApiKey> {
    let prefix = &key[..8];

    // Use elevated context to access key_hash
    let result = db.with_elevated_access(|| async {
        db.query(
            GraphQuery::new("ApiKey")
                .filter(FilterExpr::eq("key_prefix", Value::String(prefix.into())))
                .fields(["id", "org_id", "key_hash", "permissions", "expires_at"])
        ).await
    }).await.ok()?;

    for api_key in result.entities() {
        let hash: String = api_key.get("key_hash")?;
        if verify_password(key, &hash) {
            // Check expiration
            if let Some(expires_at) = api_key.get::<i64>("expires_at") {
                if expires_at < Utc::now().timestamp_micros() {
                    continue;  // Expired
                }
            }
            return Some(api_key.into());
        }
    }

    None
}
```

---

## Safe Migrations

### Adding a Feature to Existing Tenants

```ormdb
// migration: Add task labels feature

entity TaskLabel {
    id: uuid @id @default(uuid())
    org_id: uuid
    name: string
    color: string @default("#gray")

    @unique([org_id, name])
    @index(org_id)
}

entity TaskLabelAssignment {
    id: uuid @id @default(uuid())
    task_id: uuid
    label_id: uuid

    task: Task @relation(field: task_id, references: id)
    label: TaskLabel @relation(field: label_id, references: id)

    @unique([task_id, label_id])
}
```

Migration is Grade A (non-breaking) - new entities only.

### Adding a Required Field

```ormdb
// migration: Add billing email to organizations

entity Organization {
    // ... existing fields ...
    billing_email: string @default("")  // Default for existing rows
}
```

Run backfill:

```rust
// Backfill billing_email from first owner's email
async fn backfill_billing_emails(db: &Database) {
    let orgs = db.query(GraphQuery::new("Organization")).await.unwrap();

    for org in orgs.entities() {
        let org_id: Uuid = org.get("id").unwrap();

        // Find owner
        let owner = db.query(
            GraphQuery::new("OrganizationMember")
                .filter(FilterExpr::and(vec![
                    FilterExpr::eq("org_id", Value::Uuid(org_id.into_bytes())),
                    FilterExpr::eq("role", Value::String("owner".into())),
                ]))
                .include(Include::new("user").fields(["email"]))
                .limit(1)
        ).await.unwrap();

        if let Some(member) = owner.entities().next() {
            let email: String = member.nested("user").get("email").unwrap();

            db.mutate(
                Mutation::update("Organization")
                    .filter(FilterExpr::eq("id", Value::Uuid(org_id.into_bytes())))
                    .set("billing_email", Value::String(email))
            ).await.ok();
        }
    }
}
```

---

## Key Takeaways

1. **Tenant isolation via RLS** - Policies enforce data boundaries automatically
2. **Capability-based access** - Fine-grained permissions per role
3. **Field masking protects secrets** - Sensitive data never leaks to clients
4. **Audit logging is essential** - Track all administrative actions
5. **Safe migrations** - ORMDB grades changes and prevents data loss

---

## Next Steps

- Add Stripe integration for billing
- Implement SSO with [Security Guide](../guides/security.md)
- Add real-time features with [Real-time Dashboard](realtime-dashboard.md)

