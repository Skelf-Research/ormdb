# @ormdb/client

TypeScript client for ORMDB with adapters for popular ORMs.

## Installation

```bash
npm install @ormdb/client
```

## Quick Start

### Direct Client Usage

```typescript
import { OrmdbClient } from "@ormdb/client";

const client = new OrmdbClient("http://localhost:8080");

// Query entities
const result = await client.query("User", {
  filter: { field: "status", op: "eq", value: "active" },
  orderBy: [{ field: "name", direction: "asc" }],
  limit: 10,
});

console.log(`Found ${result.entities.length} users`);

// Insert entity
const insertResult = await client.insert("User", {
  name: "Alice",
  email: "alice@example.com",
});

// Update entity
await client.update("User", insertResult.insertedIds[0], {
  status: "inactive",
});

// Delete entity
await client.delete("User", insertResult.insertedIds[0]);
```

## ORM Adapters

### Prisma

```typescript
import { createPrismaClient } from "@ormdb/client/prisma";

const prisma = createPrismaClient("http://localhost:8080");

// Prisma-like API
const users = await prisma.user.findMany({
  where: {
    status: "active",
    age: { gte: 18 },
  },
  orderBy: { name: "asc" },
  take: 10,
});

const user = await prisma.user.create({
  data: { name: "Alice", email: "alice@example.com" },
});

await prisma.user.update({
  where: { id: user.id },
  data: { status: "inactive" },
});

await prisma.user.delete({
  where: { id: user.id },
});
```

### Drizzle ORM

```typescript
import { drizzle, ormdbTable, text, integer, boolean, eq } from "@ormdb/client/drizzle";

// Define schema
const users = ormdbTable("User", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  email: text("email").notNull(),
  age: integer("age"),
  active: boolean("active").default(true),
});

// Create client
const db = drizzle("http://localhost:8080");

// Type-safe queries
const activeUsers = await db
  .select()
  .from(users)
  .where(eq(users.active, true));

// Insert
await db.insert(users).values({
  name: "Alice",
  email: "alice@example.com",
});

// Update
await db
  .update(users)
  .set({ active: false })
  .where(eq(users.id, "some-id"));

// Delete
await db.delete(users).where(eq(users.id, "some-id"));
```

### TypeORM

```typescript
import { OrmdbDataSource, Entity, Column, PrimaryGeneratedColumn } from "@ormdb/client/typeorm";

@Entity()
class User {
  @PrimaryGeneratedColumn("uuid")
  id: string;

  @Column()
  name: string;

  @Column()
  email: string;

  @Column({ nullable: true })
  age: number;
}

const dataSource = new OrmdbDataSource({
  url: "http://localhost:8080",
  entities: [User],
});

await dataSource.initialize();

const userRepo = dataSource.getRepository(User);

// Find
const users = await userRepo.find({
  where: { name: "Alice" },
  order: { name: "ASC" },
});

// Create
const user = await userRepo.save({
  name: "Alice",
  email: "alice@example.com",
});

// Update
await userRepo.update(user.id, { age: 30 });

// Delete
await userRepo.delete(user.id);

// Query Builder
const results = await userRepo
  .createQueryBuilder("user")
  .where("user.age > :age", { age: 18 })
  .orderBy("user.name", "ASC")
  .getMany();
```

### Kysely

```typescript
import { Kysely } from "kysely";
import { OrmdbDialect } from "@ormdb/client/kysely";

interface Database {
  User: {
    id: string;
    name: string;
    email: string;
    age: number | null;
  };
  Post: {
    id: string;
    title: string;
    userId: string;
  };
}

const db = new Kysely<Database>({
  dialect: new OrmdbDialect({
    baseUrl: "http://localhost:8080",
  }),
});

// Type-safe queries
const users = await db
  .selectFrom("User")
  .selectAll()
  .where("age", ">", 18)
  .orderBy("name", "asc")
  .execute();

// Insert
await db
  .insertInto("User")
  .values({ name: "Alice", email: "alice@example.com" })
  .execute();

// Update
await db
  .updateTable("User")
  .set({ age: 30 })
  .where("id", "=", "some-id")
  .execute();

// Delete
await db
  .deleteFrom("User")
  .where("id", "=", "some-id")
  .execute();
```

### Sequelize

```typescript
import { OrmdbSequelize, DataTypes, Op } from "@ormdb/client/sequelize";

const sequelize = new OrmdbSequelize("http://localhost:8080");

const User = sequelize.define("User", {
  name: DataTypes.STRING,
  email: DataTypes.STRING,
  age: DataTypes.INTEGER,
  active: DataTypes.BOOLEAN,
});

// Find all
const users = await User.findAll({
  where: {
    active: true,
    age: { [Op.gt]: 18 },
  },
  order: [["name", "ASC"]],
  limit: 10,
});

// Find one
const user = await User.findOne({
  where: { email: "alice@example.com" },
});

// Create
const newUser = await User.create({
  name: "Alice",
  email: "alice@example.com",
});

// Update
await User.update(
  { active: false },
  { where: { id: newUser.id } }
);

// Delete
await User.destroy({
  where: { id: newUser.id },
});

// Count
const count = await User.count({
  where: { active: true },
});
```

## API Reference

### OrmdbClient

#### Constructor

```typescript
new OrmdbClient(config: OrmdbConfig | string)

interface OrmdbConfig {
  baseUrl: string;
  timeout?: number;      // Default: 30000ms
  headers?: Record<string, string>;
  retry?: {
    maxRetries: number;  // Default: 3
    retryDelay: number;  // Default: 1000ms
  };
}
```

#### Methods

- `query<T>(entity, options)` - Query entities
- `findFirst<T>(entity, options)` - Find first matching entity
- `findById<T>(entity, id, options)` - Find entity by ID
- `count(entity, filter)` - Count entities
- `insert(entity, data)` - Insert entity
- `insertMany(entity, records)` - Insert multiple entities
- `update(entity, id, data)` - Update entity by ID
- `updateMany(entity, filter, data)` - Update entities matching filter
- `delete(entity, id)` - Delete entity by ID
- `deleteMany(entity, filter)` - Delete entities matching filter
- `upsert(entity, data, id?)` - Insert or update entity
- `health()` - Check gateway health
- `getSchema()` - Get database schema
- `getReplicationStatus()` - Get replication status
- `streamChanges(options)` - Stream CDC changes

### Filter Operators

```typescript
// Comparison
{ field: "status", op: "eq", value: "active" }    // equals
{ field: "age", op: "ne", value: 18 }             // not equals
{ field: "age", op: "lt", value: 30 }             // less than
{ field: "age", op: "le", value: 30 }             // less than or equal
{ field: "age", op: "gt", value: 18 }             // greater than
{ field: "age", op: "ge", value: 18 }             // greater than or equal

// Pattern matching
{ field: "name", op: "like", value: "Alice%" }    // like (case-sensitive)
{ field: "name", op: "ilike", value: "alice%" }   // like (case-insensitive)

// Set membership
{ field: "status", op: "in", value: ["active", "pending"] }
{ field: "status", op: "not_in", value: ["deleted"] }

// Null checks
{ field: "deletedAt", op: "is_null" }
{ field: "email", op: "is_not_null" }

// Logical operators
{ and: [filter1, filter2] }
{ or: [filter1, filter2] }
{ not: filter }
```

## Error Handling

```typescript
import { OrmdbClient, OrmdbError } from "@ormdb/client";

const client = new OrmdbClient("http://localhost:8080");

try {
  await client.query("NonExistent");
} catch (error) {
  if (error instanceof OrmdbError) {
    console.error(`Error: ${error.message}`);
    console.error(`Code: ${error.code}`);
    // Codes: CONNECTION_ERROR, QUERY_ERROR, MUTATION_ERROR,
    //        VALIDATION_ERROR, SCHEMA_ERROR, TIMEOUT_ERROR
  }
}
```

## TypeScript Support

All adapters provide full TypeScript support with type inference:

```typescript
interface User {
  id: string;
  name: string;
  email: string;
  age: number | null;
}

// Direct client with types
const result = await client.query<User>("User", {
  filter: { field: "age", op: "gt", value: 18 },
});
// result.entities is User[]

// Prisma with inferred types
const users = await prisma.user.findMany({
  where: { age: { gte: 18 } },
});
// users is typed based on your schema
```

## License

MIT
