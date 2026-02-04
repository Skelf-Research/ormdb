# ORM Adapters

ORMDB provides adapters for popular ORM libraries, allowing you to use familiar APIs while benefiting from ORMDB's features.

---

## Overview

Instead of learning a completely new API, you can use ORMDB with the ORM you already know:

```
┌─────────────────────────────────────────────────────────────┐
│                     Your Application                         │
├─────────────────────────────────────────────────────────────┤
│  Prisma  │  TypeORM  │  Drizzle  │ SQLAlchemy │   Django   │
├─────────────────────────────────────────────────────────────┤
│                    ORMDB Adapter Layer                       │
├─────────────────────────────────────────────────────────────┤
│                        ORMDB                                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Supported Adapters

### TypeScript/JavaScript

| ORM | Status | Package |
|-----|--------|---------|
| [Prisma](prisma.md) | Stable | `@ormdb/prisma-adapter` |
| [Drizzle](drizzle.md) | Stable | `@ormdb/drizzle-adapter` |
| [TypeORM](typeorm.md) | Stable | `@ormdb/typeorm-adapter` |
| [Sequelize](sequelize.md) | Beta | `@ormdb/sequelize-adapter` |
| [Kysely](kysely.md) | Beta | `@ormdb/kysely-adapter` |

### Python

| ORM | Status | Package |
|-----|--------|---------|
| [SQLAlchemy](sqlalchemy.md) | Stable | `ormdb-sqlalchemy` |
| [Django ORM](django.md) | Stable | `django-ormdb` |

---

## How Adapters Work

Adapters translate ORM-specific queries into ORMDB's typed protocol:

```typescript
// Your code (Prisma syntax)
const users = await prisma.user.findMany({
  where: { status: 'active' },
  include: { posts: true },
});

// Adapter translates to ORMDB protocol
// GraphQuery::new("User")
//   .with_filter(FilterExpr::eq("status", "active"))
//   .include("posts")
```

### What Adapters Handle

- **Query translation**: ORM query syntax → ORMDB protocol
- **Result mapping**: ORMDB results → ORM model instances
- **Type conversion**: Language types ↔ ORMDB Value types
- **Connection management**: Connection pooling and lifecycle
- **Schema sync**: ORM schema definitions ↔ ORMDB catalog

### What Adapters Don't Change

- **Query execution**: Still uses ORMDB's query engine
- **Security**: ORMDB's RLS and field masking still apply
- **Versioning**: MVCC versioning is preserved
- **Budgets**: Query limits are enforced

---

## Feature Compatibility

Not all ORM features have ORMDB equivalents. Here's what works:

| Feature | Supported | Notes |
|---------|-----------|-------|
| Find by ID | Yes | |
| Find with filter | Yes | |
| Eager loading | Yes | Maps to includes |
| Pagination | Yes | |
| Sorting | Yes | |
| Create | Yes | |
| Update | Yes | |
| Delete | Yes | Soft delete by default |
| Transactions | Yes | |
| Raw SQL | No | Use native client instead |
| Migrations | Partial | Recommend ORMDB migrations |
| Hooks/Middlewares | Partial | ORM-side only |

---

## Quick Start

### TypeScript (Prisma)

```typescript
import { PrismaClient } from '@prisma/client';
import { withOrmdb } from '@ormdb/prisma-adapter';

// Wrap your Prisma client
const prisma = withOrmdb(new PrismaClient(), {
  connectionString: 'ormdb://localhost:8080',
});

// Use normal Prisma syntax
const users = await prisma.user.findMany({
  where: { status: 'active' },
  include: { posts: true },
});
```

### Python (SQLAlchemy)

```python
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

# Use ORMDB connection string
engine = create_engine('ormdb://localhost:8080')

with Session(engine) as session:
    users = session.query(User).filter(User.status == 'active').all()
```

### Python (Django)

```python
# settings.py
DATABASES = {
    'default': {
        'ENGINE': 'django_ormdb',
        'HOST': 'localhost',
        'PORT': 8080,
    }
}

# Use normal Django ORM
users = User.objects.filter(status='active').prefetch_related('posts')
```

---

## Choosing an Adapter

### For New Projects

| Scenario | Recommendation |
|----------|----------------|
| TypeScript, type safety priority | Prisma or Drizzle |
| TypeScript, decorator patterns | TypeORM |
| Python, modern async | SQLAlchemy 2.0 |
| Python, Django project | Django adapter |
| Maximum performance | Native ORMDB client |

### For Existing Projects

If you have an existing codebase:

1. **Add ORMDB alongside existing DB** - Start with dual-write
2. **Install appropriate adapter** - Match your current ORM
3. **Migrate queries gradually** - Use feature flags
4. **Remove old database** - When confident

---

## Performance Considerations

### Adapter Overhead

Adapters add minimal overhead:

| Layer | Latency |
|-------|---------|
| Native ORMDB client | ~1ms |
| With adapter | ~1.5ms |

The overhead is mostly in result mapping and type conversion.

### When to Use Native Client

Use the native ORMDB client when:

- Performance is critical
- You need ORMDB-specific features
- Building a new service from scratch
- Writing data pipelines

Use adapters when:

- Migrating existing code
- Team familiarity matters
- Mixing ORMDB with other databases
- Rapid prototyping

---

## Common Patterns

### Gradual Migration

```typescript
// Feature flag approach
async function getUsers() {
  if (featureFlags.useOrmdb) {
    return prismaOrmdb.user.findMany({ ... });
  } else {
    return prismaPostgres.user.findMany({ ... });
  }
}
```

### Dual Write

```typescript
// Write to both during migration
async function createUser(data: UserData) {
  const ormdbUser = await prismaOrmdb.user.create({ data });
  await prismaPostgres.user.create({ data });  // Shadow write
  return ormdbUser;
}
```

### Read from ORMDB, Write to Legacy

```typescript
// Conservative migration
async function getUser(id: string) {
  return prismaOrmdb.user.findUnique({ where: { id } });
}

async function updateUser(id: string, data: UserData) {
  // Still write to PostgreSQL during migration
  await prismaPostgres.user.update({ where: { id }, data });
  // ORMDB updated via CDC or dual write
}
```

---

## Troubleshooting

### Connection Issues

```typescript
// Check connection
try {
  await prisma.$connect();
} catch (e) {
  console.error('ORMDB connection failed:', e);
}
```

### Query Translation Errors

```typescript
// Some queries may not translate
try {
  await prisma.user.findMany({ ... });
} catch (e) {
  if (e.code === 'UNSUPPORTED_QUERY') {
    // Fall back to native client or different approach
  }
}
```

### Schema Sync Issues

```bash
# Verify schema matches
ormdb schema diff --prisma schema.prisma
```

---

## Next Steps

Choose your adapter guide:

- **[Prisma](prisma.md)** - Most popular TypeScript ORM
- **[Drizzle](drizzle.md)** - Type-safe SQL builder
- **[TypeORM](typeorm.md)** - Decorator-based ORM
- **[Sequelize](sequelize.md)** - Mature Node.js ORM
- **[Kysely](kysely.md)** - Type-safe query builder
- **[SQLAlchemy](sqlalchemy.md)** - Python's most powerful ORM
- **[Django](django.md)** - Django's built-in ORM
