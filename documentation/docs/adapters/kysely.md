# Kysely Adapter

Use ORMDB with Kysely, a type-safe TypeScript SQL query builder.

---

## Installation

```bash
npm install kysely @ormdb/kysely-adapter
# or
yarn add kysely @ormdb/kysely-adapter
```

---

## Setup

### 1. Define Database Types

```typescript
// types.ts
import { Generated, Insertable, Selectable, Updateable } from 'kysely';

export interface Database {
  users: UserTable;
  posts: PostTable;
  comments: CommentTable;
}

export interface UserTable {
  id: Generated<string>;
  name: string;
  email: string;
  status: string;
  created_at: Generated<Date>;
}

export interface PostTable {
  id: Generated<string>;
  title: string;
  content: string | null;
  published: boolean;
  author_id: string;
  created_at: Generated<Date>;
}

export interface CommentTable {
  id: Generated<string>;
  text: string;
  post_id: string;
  author_id: string;
}

// Type helpers
export type User = Selectable<UserTable>;
export type NewUser = Insertable<UserTable>;
export type UserUpdate = Updateable<UserTable>;
```

### 2. Create Kysely Instance

```typescript
// db.ts
import { Kysely } from 'kysely';
import { OrmdbDialect } from '@ormdb/kysely-adapter';
import { Database } from './types';

export const db = new Kysely<Database>({
  dialect: new OrmdbDialect({
    host: 'localhost',
    port: 8080,
  }),
});
```

---

## Basic Queries

### Select All

```typescript
const users = await db.selectFrom('users').selectAll().execute();
```

### Select Specific Columns

```typescript
const users = await db
  .selectFrom('users')
  .select(['id', 'name', 'email'])
  .execute();
```

### Where Clause

```typescript
// Equality
const activeUsers = await db
  .selectFrom('users')
  .selectAll()
  .where('status', '=', 'active')
  .execute();

// Not equal
const nonBanned = await db
  .selectFrom('users')
  .selectAll()
  .where('status', '!=', 'banned')
  .execute();

// Greater than
const adults = await db
  .selectFrom('users')
  .selectAll()
  .where('age', '>', 18)
  .execute();

// LIKE
const gmailUsers = await db
  .selectFrom('users')
  .selectAll()
  .where('email', 'like', '%@gmail.com')
  .execute();

// IN
const multiStatus = await db
  .selectFrom('users')
  .selectAll()
  .where('status', 'in', ['active', 'pending'])
  .execute();

// IS NULL
const noEmail = await db
  .selectFrom('users')
  .selectAll()
  .where('email', 'is', null)
  .execute();
```

### Compound Conditions

```typescript
// AND (chaining)
const activeAdults = await db
  .selectFrom('users')
  .selectAll()
  .where('status', '=', 'active')
  .where('age', '>=', 18)
  .execute();

// OR
const adminsOrMods = await db
  .selectFrom('users')
  .selectAll()
  .where((eb) =>
    eb.or([
      eb('role', '=', 'admin'),
      eb('role', '=', 'moderator'),
    ])
  )
  .execute();

// Complex
const complex = await db
  .selectFrom('users')
  .selectAll()
  .where((eb) =>
    eb.and([
      eb('status', '=', 'active'),
      eb.or([
        eb('role', '=', 'admin'),
        eb('age', '>=', 21),
      ]),
    ])
  )
  .execute();
```

### Ordering

```typescript
const users = await db
  .selectFrom('users')
  .selectAll()
  .orderBy('name', 'asc')
  .orderBy('created_at', 'desc')
  .execute();
```

### Pagination

```typescript
const users = await db
  .selectFrom('users')
  .selectAll()
  .limit(10)
  .offset(20)
  .execute();
```

### First/One

```typescript
// First result
const user = await db
  .selectFrom('users')
  .selectAll()
  .where('status', '=', 'active')
  .executeTakeFirst();

// First or throw
const user = await db
  .selectFrom('users')
  .selectAll()
  .where('id', '=', userId)
  .executeTakeFirstOrThrow();
```

---

## Joins

### Inner Join

```typescript
const result = await db
  .selectFrom('users')
  .innerJoin('posts', 'posts.author_id', 'users.id')
  .select(['users.id', 'users.name', 'posts.title'])
  .execute();
```

### Left Join

```typescript
const result = await db
  .selectFrom('users')
  .leftJoin('posts', 'posts.author_id', 'users.id')
  .select(['users.id', 'users.name', 'posts.title'])
  .execute();
```

### Multiple Joins

```typescript
const result = await db
  .selectFrom('users')
  .leftJoin('posts', 'posts.author_id', 'users.id')
  .leftJoin('comments', 'comments.post_id', 'posts.id')
  .select([
    'users.name as userName',
    'posts.title as postTitle',
    'comments.text as commentText',
  ])
  .execute();
```

---

## Mutations

### Insert

```typescript
// Single insert
const result = await db
  .insertInto('users')
  .values({
    name: 'Alice',
    email: 'alice@example.com',
    status: 'active',
  })
  .returningAll()
  .executeTakeFirst();

// Multiple insert
const result = await db
  .insertInto('users')
  .values([
    { name: 'Alice', email: 'alice@example.com', status: 'active' },
    { name: 'Bob', email: 'bob@example.com', status: 'active' },
  ])
  .returningAll()
  .execute();
```

### Update

```typescript
// Update with returning
const updated = await db
  .updateTable('users')
  .set({ name: 'Alice Smith' })
  .where('id', '=', userId)
  .returningAll()
  .executeTakeFirst();

// Bulk update
await db
  .updateTable('users')
  .set({ status: 'active' })
  .where('status', '=', 'pending')
  .execute();
```

### Delete

```typescript
// Delete with returning
const deleted = await db
  .deleteFrom('users')
  .where('id', '=', userId)
  .returningAll()
  .executeTakeFirst();

// Bulk delete
await db
  .deleteFrom('users')
  .where('status', '=', 'banned')
  .execute();
```

### Upsert (On Conflict)

```typescript
await db
  .insertInto('users')
  .values({
    id: userId,
    name: 'Alice',
    email: 'alice@example.com',
    status: 'active',
  })
  .onConflict((oc) =>
    oc.column('email').doUpdateSet({ name: 'Alice Smith' })
  )
  .execute();
```

---

## Transactions

```typescript
await db.transaction().execute(async (trx) => {
  const user = await trx
    .insertInto('users')
    .values({ name: 'Alice', email: 'alice@example.com', status: 'active' })
    .returningAll()
    .executeTakeFirstOrThrow();

  await trx
    .insertInto('posts')
    .values({ title: 'Welcome', author_id: user.id, published: false })
    .execute();
});
```

---

## Aggregations

```typescript
import { sql } from 'kysely';

// Count
const { count } = await db
  .selectFrom('users')
  .select(db.fn.count<number>('id').as('count'))
  .where('status', '=', 'active')
  .executeTakeFirstOrThrow();

// Sum
const { total } = await db
  .selectFrom('posts')
  .select(db.fn.sum<number>('views').as('total'))
  .executeTakeFirstOrThrow();

// Avg
const { avg } = await db
  .selectFrom('posts')
  .select(db.fn.avg<number>('views').as('avg'))
  .executeTakeFirstOrThrow();

// Min/Max
const { oldest, newest } = await db
  .selectFrom('users')
  .select([
    db.fn.min('created_at').as('oldest'),
    db.fn.max('created_at').as('newest'),
  ])
  .executeTakeFirstOrThrow();

// Group by
const stats = await db
  .selectFrom('users')
  .select(['status', db.fn.count<number>('id').as('count')])
  .groupBy('status')
  .execute();
```

---

## Type Safety

Kysely provides excellent type inference:

```typescript
// Fully typed results
const users = await db
  .selectFrom('users')
  .select(['id', 'name'])
  .execute();

// users is User[] with only id and name

// Type-safe where clauses
await db
  .selectFrom('users')
  .where('nonexistent', '=', 'value')  // Compile error!
  .execute();

// Type-safe inserts
await db
  .insertInto('users')
  .values({
    name: 'Alice',
    // email missing - compile error!
  })
  .execute();
```

---

## Dynamic Queries

```typescript
import { ExpressionBuilder } from 'kysely';

function withStatusFilter(
  eb: ExpressionBuilder<Database, 'users'>,
  status?: string
) {
  if (status) {
    return eb('status', '=', status);
  }
  return eb.val(true);  // No filter
}

const users = await db
  .selectFrom('users')
  .selectAll()
  .where((eb) => withStatusFilter(eb, userStatus))
  .execute();
```

---

## ORMDB-Specific Features

### Access Native Client

```typescript
import { getOrmdbClient } from '@ormdb/kysely-adapter';

const ormdb = getOrmdbClient(db);

// Use native ORMDB features
const result = await ormdb.query(/* ... */);
```

### Include Soft-Deleted

```typescript
import { withDeleted } from '@ormdb/kysely-adapter';

const allUsers = await db
  .selectFrom('users')
  .selectAll()
  .$call(withDeleted)
  .execute();
```

---

## Limitations

| Feature | Status | Notes |
|---------|--------|-------|
| Raw SQL | Not supported | Use native client |
| Subqueries | Partial | Basic support |
| CTEs | Not supported | |
| Window functions | Not supported | |

---

## Next Steps

- **[Prisma Adapter](prisma.md)** - Schema-first ORM
- **[Drizzle Adapter](drizzle.md)** - Alternative query builder
- **[Native Client](../clients/typescript.md)** - Direct ORMDB access
