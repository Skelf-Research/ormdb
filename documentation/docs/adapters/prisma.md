# Prisma Adapter

Use ORMDB with Prisma's familiar API for type-safe database access.

---

## Installation

```bash
npm install @ormdb/prisma-adapter
# or
yarn add @ormdb/prisma-adapter
# or
pnpm add @ormdb/prisma-adapter
```

---

## Setup

### 1. Configure Prisma Schema

```prisma
// schema.prisma
generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "ormdb"
  url      = env("ORMDB_URL")
}

model User {
  id        String   @id @default(uuid())
  name      String
  email     String   @unique
  status    String   @default("active")
  posts     Post[]
  createdAt DateTime @default(now())
}

model Post {
  id        String   @id @default(uuid())
  title     String
  content   String?
  published Boolean  @default(false)
  author    User     @relation(fields: [authorId], references: [id])
  authorId  String
  comments  Comment[]
}

model Comment {
  id       String @id @default(uuid())
  text     String
  post     Post   @relation(fields: [postId], references: [id])
  postId   String
}
```

### 2. Set Environment Variable

```bash
# .env
ORMDB_URL="ormdb://localhost:8080"
```

### 3. Generate Client

```bash
npx prisma generate
```

### 4. Initialize Client

```typescript
import { PrismaClient } from '@prisma/client';

const prisma = new PrismaClient();

// The adapter is automatically applied when using the ormdb provider
```

---

## Basic Operations

### Find Many

```typescript
// All users
const users = await prisma.user.findMany();

// With filter
const activeUsers = await prisma.user.findMany({
  where: { status: 'active' },
});

// With pagination
const pagedUsers = await prisma.user.findMany({
  skip: 20,
  take: 10,
  orderBy: { name: 'asc' },
});
```

### Find Unique

```typescript
// By ID
const user = await prisma.user.findUnique({
  where: { id: 'user-123' },
});

// By unique field
const user = await prisma.user.findUnique({
  where: { email: 'alice@example.com' },
});
```

### Find First

```typescript
const user = await prisma.user.findFirst({
  where: { status: 'active' },
  orderBy: { createdAt: 'desc' },
});
```

---

## Relations

### Include Relations

```typescript
// Single relation
const user = await prisma.user.findUnique({
  where: { id: 'user-123' },
  include: { posts: true },
});

// Multiple relations
const user = await prisma.user.findUnique({
  where: { id: 'user-123' },
  include: {
    posts: true,
    profile: true,
  },
});

// Nested relations
const user = await prisma.user.findUnique({
  where: { id: 'user-123' },
  include: {
    posts: {
      include: {
        comments: true,
      },
    },
  },
});
```

### Filtered Relations

```typescript
const user = await prisma.user.findUnique({
  where: { id: 'user-123' },
  include: {
    posts: {
      where: { published: true },
      orderBy: { createdAt: 'desc' },
      take: 5,
    },
  },
});
```

### Select Specific Fields

```typescript
const user = await prisma.user.findUnique({
  where: { id: 'user-123' },
  select: {
    id: true,
    name: true,
    posts: {
      select: {
        id: true,
        title: true,
      },
    },
  },
});
```

---

## Filtering

### Basic Operators

```typescript
// Equality
where: { status: 'active' }

// Not equal
where: { status: { not: 'banned' } }

// Greater than
where: { age: { gt: 18 } }

// Greater than or equal
where: { age: { gte: 18 } }

// Less than
where: { price: { lt: 100 } }

// Less than or equal
where: { price: { lte: 100 } }
```

### String Operators

```typescript
// Contains
where: { name: { contains: 'alice' } }

// Starts with
where: { email: { startsWith: 'admin' } }

// Ends with
where: { email: { endsWith: '@example.com' } }
```

### List Operators

```typescript
// In list
where: { status: { in: ['active', 'pending'] } }

// Not in list
where: { role: { notIn: ['banned', 'suspended'] } }
```

### Null Checks

```typescript
// Is null
where: { deletedAt: null }

// Is not null
where: { verifiedAt: { not: null } }
```

### Logical Operators

```typescript
// AND (implicit)
where: {
  status: 'active',
  age: { gte: 18 },
}

// AND (explicit)
where: {
  AND: [
    { status: 'active' },
    { age: { gte: 18 } },
  ],
}

// OR
where: {
  OR: [
    { role: 'admin' },
    { role: 'moderator' },
  ],
}

// NOT
where: {
  NOT: { status: 'banned' },
}
```

---

## Mutations

### Create

```typescript
// Simple create
const user = await prisma.user.create({
  data: {
    name: 'Alice',
    email: 'alice@example.com',
  },
});

// Create with relation
const user = await prisma.user.create({
  data: {
    name: 'Alice',
    email: 'alice@example.com',
    posts: {
      create: {
        title: 'My First Post',
        content: 'Hello, world!',
      },
    },
  },
  include: { posts: true },
});
```

### Create Many

```typescript
const result = await prisma.user.createMany({
  data: [
    { name: 'Alice', email: 'alice@example.com' },
    { name: 'Bob', email: 'bob@example.com' },
    { name: 'Charlie', email: 'charlie@example.com' },
  ],
});

console.log(`Created ${result.count} users`);
```

### Update

```typescript
// Update single
const user = await prisma.user.update({
  where: { id: 'user-123' },
  data: { name: 'Alice Smith' },
});

// Update with upsert
const user = await prisma.user.upsert({
  where: { email: 'alice@example.com' },
  update: { name: 'Alice Smith' },
  create: { name: 'Alice Smith', email: 'alice@example.com' },
});
```

### Update Many

```typescript
const result = await prisma.user.updateMany({
  where: { status: 'pending' },
  data: { status: 'active' },
});

console.log(`Updated ${result.count} users`);
```

### Delete

```typescript
// Delete single
const user = await prisma.user.delete({
  where: { id: 'user-123' },
});

// Delete many
const result = await prisma.user.deleteMany({
  where: { status: 'banned' },
});
```

---

## Transactions

### Interactive Transactions

```typescript
const result = await prisma.$transaction(async (tx) => {
  const user = await tx.user.create({
    data: { name: 'Alice', email: 'alice@example.com' },
  });

  const post = await tx.post.create({
    data: {
      title: 'Welcome Post',
      authorId: user.id,
    },
  });

  return { user, post };
});
```

### Batch Transactions

```typescript
const [users, posts] = await prisma.$transaction([
  prisma.user.findMany({ where: { status: 'active' } }),
  prisma.post.findMany({ where: { published: true } }),
]);
```

---

## Aggregations

```typescript
// Count
const count = await prisma.user.count({
  where: { status: 'active' },
});

// Aggregate
const stats = await prisma.post.aggregate({
  _count: true,
  _avg: { views: true },
  _sum: { views: true },
  _min: { createdAt: true },
  _max: { createdAt: true },
  where: { published: true },
});

// Group by
const byStatus = await prisma.user.groupBy({
  by: ['status'],
  _count: true,
});
```

---

## Type Safety

Prisma generates types from your schema:

```typescript
import { User, Post, Prisma } from '@prisma/client';

// Model types
const user: User = await prisma.user.findUniqueOrThrow({
  where: { id: 'user-123' },
});

// Input types
const data: Prisma.UserCreateInput = {
  name: 'Alice',
  email: 'alice@example.com',
};

// With relations
type UserWithPosts = Prisma.UserGetPayload<{
  include: { posts: true };
}>;
```

---

## ORMDB-Specific Features

### Access Native Client

```typescript
import { getOrmdbClient } from '@ormdb/prisma-adapter';

const ormdb = getOrmdbClient(prisma);

// Use native ORMDB features
const result = await ormdb.query(
  GraphQuery::new("User")
    .with_budget(FanoutBudget::new(100, 500, 3))
);
```

### Query Budget

```typescript
// Set budget for a query
const users = await prisma.user.findMany({
  // @ts-expect-error - ORMDB extension
  $budget: { maxEntities: 100 },
  include: { posts: true },
});
```

### Soft Deletes

ORMDB soft deletes by default. To see deleted records:

```typescript
const allUsers = await prisma.user.findMany({
  // @ts-expect-error - ORMDB extension
  $includeDeleted: true,
});
```

---

## Migration from PostgreSQL

### 1. Update Schema

```prisma
// Before
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

// After
datasource db {
  provider = "ormdb"
  url      = env("ORMDB_URL")
}
```

### 2. Regenerate Client

```bash
npx prisma generate
```

### 3. Migrate Data

```bash
# Export from PostgreSQL
pg_dump -Fc mydb > backup.dump

# Import to ORMDB
ormdb import --from-pg backup.dump
```

### 4. Update Connection String

```bash
# .env
ORMDB_URL="ormdb://localhost:8080"
```

---

## Limitations

Features not supported with ORMDB:

| Feature | Status | Alternative |
|---------|--------|-------------|
| Raw SQL | Not supported | Use native client |
| `$queryRaw` | Not supported | Use native client |
| `$executeRaw` | Not supported | Use native client |
| Json filtering | Partial | Basic support only |
| Full-text search | Not supported | Use external search |

---

## Troubleshooting

### Connection Errors

```typescript
// Check connection
try {
  await prisma.$connect();
  console.log('Connected to ORMDB');
} catch (e) {
  console.error('Connection failed:', e.message);
}
```

### Schema Sync

```bash
# Verify schema matches ORMDB catalog
npx prisma db pull  # Pull current schema
npx prisma db push  # Push schema to ORMDB
```

### Debug Queries

```typescript
// Enable query logging
const prisma = new PrismaClient({
  log: ['query', 'info', 'warn', 'error'],
});
```

---

## Next Steps

- **[Drizzle Adapter](drizzle.md)** - Alternative TypeScript ORM
- **[TypeORM Adapter](typeorm.md)** - Decorator-based ORM
- **[Native Client](../clients/typescript.md)** - Direct ORMDB access
