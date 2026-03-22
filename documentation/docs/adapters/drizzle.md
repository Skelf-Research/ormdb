# Drizzle Adapter

Use ORMDB with Drizzle's type-safe SQL-like query builder.

---

## Installation

```bash
npm install drizzle-orm @ormdb/drizzle-adapter
# or
yarn add drizzle-orm @ormdb/drizzle-adapter
# or
pnpm add drizzle-orm @ormdb/drizzle-adapter
```

---

## Setup

### 1. Define Schema

```typescript
// schema.ts
import { pgTable, uuid, text, boolean, timestamp } from 'drizzle-orm/pg-core';

export const users = pgTable('users', {
  id: uuid('id').primaryKey().defaultRandom(),
  name: text('name').notNull(),
  email: text('email').notNull().unique(),
  status: text('status').notNull().default('active'),
  createdAt: timestamp('created_at').notNull().defaultNow(),
});

export const posts = pgTable('posts', {
  id: uuid('id').primaryKey().defaultRandom(),
  title: text('title').notNull(),
  content: text('content'),
  published: boolean('published').notNull().default(false),
  authorId: uuid('author_id').notNull().references(() => users.id),
  createdAt: timestamp('created_at').notNull().defaultNow(),
});

export const comments = pgTable('comments', {
  id: uuid('id').primaryKey().defaultRandom(),
  text: text('text').notNull(),
  postId: uuid('post_id').notNull().references(() => posts.id),
  authorId: uuid('author_id').notNull().references(() => users.id),
});
```

### 2. Create Database Instance

```typescript
// db.ts
import { drizzle } from '@ormdb/drizzle-adapter';

export const db = drizzle({
  connectionString: process.env.ORMDB_URL || 'ormdb://localhost:8080',
});
```

### 3. Use in Application

```typescript
import { db } from './db';
import { users, posts } from './schema';

// Start querying
const allUsers = await db.select().from(users);
```

---

## Basic Queries

### Select All

```typescript
const allUsers = await db.select().from(users);
```

### Select Specific Columns

```typescript
const userNames = await db
  .select({
    id: users.id,
    name: users.name,
  })
  .from(users);
```

### Where Clause

```typescript
import { eq, gt, and, or, like } from 'drizzle-orm';

// Equality
const activeUsers = await db
  .select()
  .from(users)
  .where(eq(users.status, 'active'));

// Greater than
const adults = await db
  .select()
  .from(users)
  .where(gt(users.age, 18));

// AND
const activeAdults = await db
  .select()
  .from(users)
  .where(and(eq(users.status, 'active'), gt(users.age, 18)));

// OR
const adminsOrMods = await db
  .select()
  .from(users)
  .where(or(eq(users.role, 'admin'), eq(users.role, 'moderator')));

// LIKE
const gmailUsers = await db
  .select()
  .from(users)
  .where(like(users.email, '%@gmail.com'));
```

### Ordering

```typescript
import { asc, desc } from 'drizzle-orm';

const sortedUsers = await db
  .select()
  .from(users)
  .orderBy(asc(users.name));

const recentFirst = await db
  .select()
  .from(users)
  .orderBy(desc(users.createdAt));
```

### Pagination

```typescript
const pagedUsers = await db
  .select()
  .from(users)
  .limit(10)
  .offset(20);
```

---

## Relations

### Define Relations

```typescript
// schema.ts
import { relations } from 'drizzle-orm';

export const usersRelations = relations(users, ({ many }) => ({
  posts: many(posts),
}));

export const postsRelations = relations(posts, ({ one, many }) => ({
  author: one(users, {
    fields: [posts.authorId],
    references: [users.id],
  }),
  comments: many(comments),
}));

export const commentsRelations = relations(comments, ({ one }) => ({
  post: one(posts, {
    fields: [comments.postId],
    references: [posts.id],
  }),
  author: one(users, {
    fields: [comments.authorId],
    references: [users.id],
  }),
}));
```

### Query with Relations

```typescript
import { db } from './db';

// User with posts
const usersWithPosts = await db.query.users.findMany({
  with: {
    posts: true,
  },
});

// Nested relations
const usersWithPostsAndComments = await db.query.users.findMany({
  with: {
    posts: {
      with: {
        comments: true,
      },
    },
  },
});

// Filtered relations
const usersWithPublishedPosts = await db.query.users.findMany({
  with: {
    posts: {
      where: eq(posts.published, true),
      orderBy: desc(posts.createdAt),
      limit: 5,
    },
  },
});
```

### Find One

```typescript
const user = await db.query.users.findFirst({
  where: eq(users.id, 'user-123'),
  with: {
    posts: true,
  },
});
```

---

## Joins

### Inner Join

```typescript
const result = await db
  .select({
    user: users,
    post: posts,
  })
  .from(users)
  .innerJoin(posts, eq(users.id, posts.authorId));
```

### Left Join

```typescript
const result = await db
  .select({
    user: users,
    post: posts,
  })
  .from(users)
  .leftJoin(posts, eq(users.id, posts.authorId));
```

### Multiple Joins

```typescript
const result = await db
  .select({
    user: users,
    post: posts,
    comment: comments,
  })
  .from(users)
  .leftJoin(posts, eq(users.id, posts.authorId))
  .leftJoin(comments, eq(posts.id, comments.postId));
```

---

## Mutations

### Insert

```typescript
// Single insert
const newUser = await db
  .insert(users)
  .values({
    name: 'Alice',
    email: 'alice@example.com',
  })
  .returning();

// Multiple insert
const newUsers = await db
  .insert(users)
  .values([
    { name: 'Alice', email: 'alice@example.com' },
    { name: 'Bob', email: 'bob@example.com' },
  ])
  .returning();
```

### Update

```typescript
// Update with returning
const updated = await db
  .update(users)
  .set({ status: 'inactive' })
  .where(eq(users.id, 'user-123'))
  .returning();

// Update many
await db
  .update(users)
  .set({ status: 'active' })
  .where(eq(users.status, 'pending'));
```

### Delete

```typescript
// Delete single
const deleted = await db
  .delete(users)
  .where(eq(users.id, 'user-123'))
  .returning();

// Delete many
await db
  .delete(users)
  .where(eq(users.status, 'banned'));
```

### Upsert

```typescript
await db
  .insert(users)
  .values({
    id: 'user-123',
    name: 'Alice',
    email: 'alice@example.com',
  })
  .onConflictDoUpdate({
    target: users.email,
    set: { name: 'Alice Smith' },
  });
```

---

## Transactions

```typescript
await db.transaction(async (tx) => {
  const user = await tx
    .insert(users)
    .values({ name: 'Alice', email: 'alice@example.com' })
    .returning();

  await tx
    .insert(posts)
    .values({
      title: 'Welcome Post',
      authorId: user[0].id,
    });
});
```

---

## Aggregations

### Count

```typescript
import { count } from 'drizzle-orm';

const result = await db
  .select({ count: count() })
  .from(users)
  .where(eq(users.status, 'active'));
```

### Sum, Avg, Min, Max

```typescript
import { sum, avg, min, max } from 'drizzle-orm';

const stats = await db
  .select({
    totalViews: sum(posts.views),
    avgViews: avg(posts.views),
    minViews: min(posts.views),
    maxViews: max(posts.views),
  })
  .from(posts)
  .where(eq(posts.published, true));
```

### Group By

```typescript
const byStatus = await db
  .select({
    status: users.status,
    count: count(),
  })
  .from(users)
  .groupBy(users.status);
```

---

## Type Inference

Drizzle provides excellent type inference:

```typescript
// Infer types from schema
type User = typeof users.$inferSelect;
type NewUser = typeof users.$inferInsert;

// Type-safe queries
const user: User = await db.query.users.findFirst({
  where: eq(users.id, 'user-123'),
});

// Type-safe inserts
const newUser: NewUser = {
  name: 'Alice',
  email: 'alice@example.com',
};
await db.insert(users).values(newUser);
```

---

## ORMDB-Specific Features

### Access Native Client

```typescript
import { getOrmdbClient } from '@ormdb/drizzle-adapter';

const ormdb = getOrmdbClient(db);

// Use native ORMDB features
const result = await ormdb.query(/* ... */);
```

### Query Budget

```typescript
// Set budget through extension
const users = await db
  .select()
  .from(users)
  .$budget({ maxEntities: 100 });
```

### Include Deleted

```typescript
// ORMDB soft deletes by default
const allUsers = await db
  .select()
  .from(users)
  .$includeDeleted();
```

---

## Schema Push

Push your Drizzle schema to ORMDB:

```bash
npx drizzle-kit push:ormdb
```

Or pull existing schema:

```bash
npx drizzle-kit introspect:ormdb
```

---

## Drizzle Kit Config

```typescript
// drizzle.config.ts
import type { Config } from 'drizzle-kit';

export default {
  schema: './src/schema.ts',
  driver: 'ormdb',
  dbCredentials: {
    connectionString: process.env.ORMDB_URL!,
  },
} satisfies Config;
```

---

## Migration from PostgreSQL

### 1. Update Drizzle Config

```typescript
// Before
export default {
  driver: 'pg',
  dbCredentials: {
    connectionString: process.env.DATABASE_URL!,
  },
};

// After
export default {
  driver: 'ormdb',
  dbCredentials: {
    connectionString: process.env.ORMDB_URL!,
  },
};
```

### 2. Update Database Instance

```typescript
// Before
import { drizzle } from 'drizzle-orm/node-postgres';
import { Pool } from 'pg';

const pool = new Pool({ connectionString: process.env.DATABASE_URL });
const db = drizzle(pool);

// After
import { drizzle } from '@ormdb/drizzle-adapter';

const db = drizzle({
  connectionString: process.env.ORMDB_URL,
});
```

---

## Limitations

| Feature | Status | Notes |
|---------|--------|-------|
| Raw SQL | Not supported | Use native client |
| SQL template literals | Not supported | Use native client |
| Subqueries | Partial | Basic support |
| CTEs | Not supported | |
| Window functions | Not supported | |

---

## Next Steps

- **[Prisma Adapter](prisma.md)** - Schema-first ORM
- **[TypeORM Adapter](typeorm.md)** - Decorator-based ORM
- **[Native Client](../clients/typescript.md)** - Direct ORMDB access
