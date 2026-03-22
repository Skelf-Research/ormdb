# TypeScript Client

TypeScript/JavaScript client for ORMDB with adapters for popular ORMs.

## Installation

```bash
npm install @ormdb/client
# or
yarn add @ormdb/client
# or
pnpm add @ormdb/client
```

## Direct Client

### Basic Usage

```typescript
import { OrmdbClient } from "@ormdb/client";

const client = new OrmdbClient("http://localhost:8080");

// Query
const result = await client.query("User", {
  fields: ["id", "name", "email"],
  filter: { field: "status", op: "eq", value: "active" },
  limit: 10,
});

// Insert
const insertResult = await client.insert("User", {
  name: "Alice",
  email: "alice@example.com",
});

// Update
await client.update("User", insertResult.insertedIds[0], {
  name: "Alice Smith",
});

// Delete
await client.delete("User", insertResult.insertedIds[0]);
```

### Configuration

```typescript
const client = new OrmdbClient({
  baseUrl: "http://localhost:8080",
  timeout: 30000,
  headers: {
    Authorization: "Bearer token",
  },
  retry: {
    maxRetries: 3,
    retryDelay: 1000,
  },
});
```

### Type Safety

```typescript
interface User {
  id: string;
  name: string;
  email: string;
  age?: number;
}

const result = await client.query<User>("User", {
  fields: ["id", "name", "email", "age"],
});

// result.entities is User[]
for (const user of result.entities) {
  console.log(user.name); // TypeScript knows this is string
}
```

## Prisma Adapter

Use familiar Prisma-style API:

```typescript
import { createPrismaClient } from "@ormdb/client/prisma";

const prisma = createPrismaClient("http://localhost:8080");

// Find many
const users = await prisma.user.findMany({
  where: {
    status: "active",
    age: { gte: 18 },
  },
  orderBy: { name: "asc" },
  take: 10,
});

// Find unique
const user = await prisma.user.findUnique({
  where: { id: userId },
});

// Create
const newUser = await prisma.user.create({
  data: { name: "Alice", email: "alice@example.com" },
});

// Update
await prisma.user.update({
  where: { id: userId },
  data: { status: "inactive" },
});

// Delete
await prisma.user.delete({
  where: { id: userId },
});

// Include relations
const usersWithPosts = await prisma.user.findMany({
  include: {
    posts: {
      where: { published: true },
    },
  },
});
```

## Drizzle Adapter

Type-safe queries with Drizzle-style API:

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

const db = drizzle("http://localhost:8080");

// Select
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

## TypeORM Adapter

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
});

// Create
const user = await userRepo.save({
  name: "Alice",
  email: "alice@example.com",
});

// Query builder
const results = await userRepo
  .createQueryBuilder("user")
  .where("user.age > :age", { age: 18 })
  .getMany();
```

## Kysely Adapter

```typescript
import { Kysely } from "kysely";
import { OrmdbDialect } from "@ormdb/client/kysely";

interface Database {
  User: { id: string; name: string; email: string };
  Post: { id: string; title: string; userId: string };
}

const db = new Kysely<Database>({
  dialect: new OrmdbDialect({ baseUrl: "http://localhost:8080" }),
});

const users = await db
  .selectFrom("User")
  .selectAll()
  .where("age", ">", 18)
  .orderBy("name", "asc")
  .execute();
```

## Error Handling

```typescript
import { OrmdbError } from "@ormdb/client";

try {
  await client.insert("User", { email: "existing@example.com" });
} catch (error) {
  if (error instanceof OrmdbError) {
    switch (error.code) {
      case "UNIQUE_VIOLATION":
        console.log("Email already exists");
        break;
      case "FOREIGN_KEY_VIOLATION":
        console.log("Invalid reference");
        break;
      case "VALIDATION_ERROR":
        console.log("Invalid data:", error.message);
        break;
      default:
        console.log("Error:", error.message);
    }
  }
}
```

## Best Practices

1. **Use typed clients** for compile-time safety
2. **Choose the right adapter** for your existing codebase
3. **Handle errors** appropriately
4. **Use includes sparingly** - deep nesting impacts performance

## Next Steps

- **[Query API Reference](../reference/query-api.md)**
- **[Filter Operators](../tutorials/filtering.md)**
