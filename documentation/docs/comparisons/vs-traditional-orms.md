# ORMDB vs Traditional ORMs

This guide compares ORMDB with traditional ORM architectures to help you understand the fundamental differences and benefits.

---

## Architectural Difference

### Traditional ORM Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Application                              │
├─────────────────────────────────────────────────────────────┤
│                         ORM Layer                             │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  - Object mapping                                    │   │
│   │  - Query building                                    │   │
│   │  - Relationship loading                              │   │
│   │  - Change tracking                                   │   │
│   │  - SQL generation                                    │   │
│   └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                     Database (SQL)                            │
│   - Executes SQL strings                                      │
│   - No understanding of objects                               │
│   - No relationship awareness                                 │
└─────────────────────────────────────────────────────────────┘
```

### ORMDB Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Application                              │
├─────────────────────────────────────────────────────────────┤
│                    ORM Adapter (Thin)                         │
│   - Maps language types to protocol                           │
│   - No SQL generation                                         │
├─────────────────────────────────────────────────────────────┤
│                    ORMDB (Database)                           │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  - Native entity/relation understanding             │   │
│   │  - Graph query execution                            │   │
│   │  - Optimized batch loading                          │   │
│   │  - Type validation                                  │   │
│   │  - Security enforcement                             │   │
│   └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Key insight**: In ORMDB, the intelligence moves from the ORM to the database.

---

## The N+1 Problem

### How ORMs Handle Relationships

**Lazy Loading (Default in most ORMs):**
```python
# SQLAlchemy / Django / ActiveRecord pattern
users = User.query.all()  # 1 query

for user in users:
    print(user.posts)  # N queries!
    for post in user.posts:
        print(post.comments)  # N*M more queries!
```

**Eager Loading (Manual optimization):**
```python
# SQLAlchemy
users = User.query.options(
    joinedload(User.posts).joinedload(Post.comments)
).all()

# Django
users = User.objects.prefetch_related('posts__comments').all()

# ActiveRecord
users = User.includes(posts: :comments).all()
```

Problems with eager loading:
- Must remember to add it everywhere
- Easy to miss and cause production issues
- Code duplication across queries
- Changes in one place don't propagate

**ORMDB:**
```rust
// N+1 is impossible by design
let query = GraphQuery::new("User")
    .include("posts")
    .include("posts.comments");

// Always batched, no lazy loading pitfalls
```

---

## Query Building Comparison

### Django ORM

```python
# Building a complex query
from django.db.models import Q

users = User.objects.filter(
    Q(status='active') & Q(age__gte=18),
    posts__published=True
).select_related('profile').prefetch_related(
    'posts__comments'
).order_by('name')[:20]
```

### SQLAlchemy

```python
from sqlalchemy.orm import joinedload

users = session.query(User).filter(
    and_(User.status == 'active', User.age >= 18)
).options(
    joinedload(User.profile),
    joinedload(User.posts).joinedload(Post.comments)
).order_by(User.name).limit(20).all()
```

### TypeORM

```typescript
const users = await userRepository.find({
  where: { status: 'active', age: MoreThanOrEqual(18) },
  relations: ['profile', 'posts', 'posts.comments'],
  order: { name: 'ASC' },
  take: 20,
});
```

### ORMDB

```rust
let query = GraphQuery::new("User")
    .with_filter(FilterExpr::and(
        FilterExpr::eq("status", Value::String("active".into())),
        FilterExpr::gte("age", Value::Int32(18))
    ))
    .with_order(OrderSpec::asc("name"))
    .with_pagination(Pagination::new(20, 0))
    .include("profile")
    .include(RelationInclude::new("posts")
        .with_filter(FilterExpr::eq("published", Value::Bool(true))))
    .include("posts.comments");
```

**Key differences:**
- ORMDB validates the query against the schema before execution
- Include filters are applied at the database level, not in memory
- No distinction between joinedload/subqueryload strategies needed

---

## Schema and Migration Comparison

### Django Migrations

```python
# models.py
class User(models.Model):
    name = models.CharField(max_length=100)
    email = models.EmailField()
    # Adding a new field...
    status = models.CharField(max_length=20, default='active')

# Generated migration
class Migration(migrations.Migration):
    operations = [
        migrations.AddField(
            model_name='user',
            name='status',
            field=models.CharField(default='active', max_length=20),
        ),
    ]

# No safety validation - hope it works!
```

### Prisma Migrations

```prisma
// schema.prisma
model User {
  id     String @id @default(uuid())
  name   String
  email  String @unique
  status String @default("active")  // new field
}

// prisma migrate dev
// Creates migration file, but no safety grades
```

### ORMDB Migrations

```rust
// Define schema change
let schema_v2 = SchemaBundle::new(2)
    .with_entity(
        EntityDef::new("User", "id")
            .with_field(FieldDef::new("name", ScalarType::String))
            .with_field(FieldDef::new("email", ScalarType::String).with_index())
            .with_field(
                FieldDef::new("status", ScalarType::String)
                    .with_default(DefaultValue::String("active".into()))
            )
    );

// Grade the migration
let grade = SafetyGrader::grade(&SchemaDiff::compute(&v1, &v2));
// Grade: B - Safe with backfill
// Reason: Required field with default needs backfill

// Apply with understanding
if grade.can_run_online() {
    catalog.apply_schema(schema_v2)?;
}
```

---

## Type Safety Comparison

### Python ORMs (Django, SQLAlchemy)

```python
# No compile-time checking
user = User.objects.get(id=user_id)
user.naem = "Alice"  # Typo - fails at runtime
user.save()

# Filter typos not caught
User.objects.filter(staus='active')  # Silently returns nothing
```

### TypeScript ORMs (TypeORM, Prisma)

```typescript
// TypeORM - partial type safety
const user = await userRepository.findOne({ where: { id } });
user.name = "Alice";  // Type checked
await userRepository.save(user);

// But string-based queries aren't
await userRepository.query("SELECT * FROM usres");  // Typo not caught

// Prisma - excellent type safety
const user = await prisma.user.findUnique({ where: { id } });
// user.nonexistent  // Compile error!
```

### ORMDB

```rust
// Query validated against schema
let query = GraphQuery::new("Usre");  // Error: Unknown entity 'Usre'

let query = GraphQuery::new("User")
    .with_fields(vec!["naem"]);  // Error: Unknown field 'naem'

let query = GraphQuery::new("User")
    .with_filter(FilterExpr::eq("age", Value::String("25")));
    // Error: Cannot compare Int32 field with String value
```

Type safety at:
1. Entity names
2. Field names
3. Relation names
4. Filter value types
5. Pagination parameters

---

## Security Comparison

### Traditional ORMs

```python
# Row-level security implemented in application code
class UserQuerySet(models.QuerySet):
    def for_user(self, user):
        if user.is_admin:
            return self.all()
        return self.filter(organization=user.organization)

# Easy to forget in some queries
User.objects.all()  # Oops, no tenant filtering!
```

### ORMDB

```rust
// Row-level security in the database
let policy = RlsPolicy::new("tenant_isolation")
    .on_entity("User")
    .where_expr(FilterExpr::eq("tenant_id", context.tenant_id));

// Applied automatically to ALL queries
let query = GraphQuery::new("User");  // Policy enforced
```

---

## Performance Comparison

### Typical ORM Patterns

**Problem: Accidental full table load**
```python
# Django: Silently loads all users into memory
for user in User.objects.all():
    if user.is_active:
        process(user)
```

**Problem: Select N+1**
```python
# Accessing related data in loops
for order in Order.objects.all():
    print(order.customer.name)  # Query per order!
```

**Problem: Over-fetching**
```python
# Loading entire objects when you need one field
names = [u.name for u in User.objects.all()]
# Loaded: id, email, password_hash, created_at, ...
```

### ORMDB Protections

```rust
// Budget enforcement prevents runaway queries
let query = GraphQuery::new("User");  // Limited to 10,000 by default

// Field selection is explicit
let query = GraphQuery::new("User")
    .with_fields(vec!["name"]);  // Only loads name

// Pagination is standard
let query = GraphQuery::new("User")
    .with_pagination(Pagination::new(100, 0));  // Explicit limit
```

---

## Feature Matrix

| Feature | Django | SQLAlchemy | TypeORM | Prisma | ORMDB |
|---------|--------|------------|---------|--------|-------|
| Type safety | Runtime | Runtime | Partial | Full | Full |
| N+1 prevention | Manual | Manual | Manual | Manual | Built-in |
| Migration grades | No | No | No | No | Yes |
| Row-level security | App code | App code | App code | Row filters | Database |
| Query budgets | No | No | No | No | Yes |
| MVCC versioning | No | No | No | No | Yes |
| Field masking | No | No | No | No | Yes |
| Audit logging | Manual | Manual | Manual | Manual | Built-in |

---

## Adoption Patterns

### Using ORMDB with Your Existing ORM

You don't have to abandon your ORM. ORMDB provides adapters:

**Prisma + ORMDB:**
```typescript
import { PrismaOrmdb } from '@ormdb/prisma-adapter';

const prisma = new PrismaOrmdb({
  datasources: { db: { url: 'ormdb://localhost:8080' } }
});

// Use familiar Prisma syntax
const users = await prisma.user.findMany({
  where: { status: 'active' },
  include: { posts: true },
});
```

**SQLAlchemy + ORMDB:**
```python
from ormdb.sqlalchemy import create_engine

engine = create_engine('ormdb://localhost:8080')

# Use familiar SQLAlchemy syntax
with Session(engine) as session:
    users = session.query(User).filter(User.status == 'active').all()
```

---

## When to Use What

### Keep Your Current ORM When:

- You need maximum SQL flexibility
- You have complex raw SQL queries
- Your team prefers SQL thinking
- Migration risk is too high

### Consider ORMDB When:

- N+1 queries are a constant problem
- Type safety is a priority
- You want database-level security
- Schema migrations are painful
- You need audit trails

### Hybrid Approach:

Use ORMDB for:
- Core business entities
- High-traffic queries
- Security-sensitive data

Keep SQL/ORM for:
- Complex reporting
- Data warehousing
- Legacy integrations

---

## Migration Strategy

### Gradual Migration

1. **Add ORMDB alongside existing database**
2. **Mirror critical entities to ORMDB**
3. **Route new features to ORMDB**
4. **Gradually migrate read paths**
5. **Migrate write paths when confident**

### Entity by Entity

```
Week 1: User entity → ORMDB
Week 2: Post entity → ORMDB
Week 3: Comment entity → ORMDB
...
```

### Feature Flag Approach

```python
if feature_flags.use_ormdb_for_users:
    users = ormdb_client.query("User", ...)
else:
    users = User.objects.filter(...)
```

---

## Summary

| Traditional ORMs | ORMDB |
|-----------------|-------|
| SQL generation in application | Typed protocol to database |
| N+1 requires manual optimization | N+1 impossible by design |
| Security in application code | Security in database |
| Runtime query validation | Compile-time validation |
| Migration scripts | Safety-graded migrations |

---

## Next Steps

- **[ORM Adapters](../adapters/index.md)** - Use ORMDB with Prisma, TypeORM, etc.
- **[vs SQLite](vs-sqlite.md)** - Compare with embedded databases
- **[Quickstart](../getting-started/quickstart.md)** - Try ORMDB
