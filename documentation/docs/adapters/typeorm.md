# TypeORM Adapter

Use ORMDB with TypeORM's decorator-based entity definitions.

---

## Installation

```bash
npm install typeorm @ormdb/typeorm-adapter reflect-metadata
# or
yarn add typeorm @ormdb/typeorm-adapter reflect-metadata
```

Add to your `tsconfig.json`:

```json
{
  "compilerOptions": {
    "experimentalDecorators": true,
    "emitDecoratorMetadata": true
  }
}
```

---

## Setup

### 1. Define Entities

```typescript
// entities/User.ts
import {
  Entity,
  PrimaryGeneratedColumn,
  Column,
  CreateDateColumn,
  OneToMany,
} from 'typeorm';
import { Post } from './Post';

@Entity('users')
export class User {
  @PrimaryGeneratedColumn('uuid')
  id: string;

  @Column()
  name: string;

  @Column({ unique: true })
  email: string;

  @Column({ default: 'active' })
  status: string;

  @CreateDateColumn()
  createdAt: Date;

  @OneToMany(() => Post, (post) => post.author)
  posts: Post[];
}
```

```typescript
// entities/Post.ts
import {
  Entity,
  PrimaryGeneratedColumn,
  Column,
  ManyToOne,
  OneToMany,
  CreateDateColumn,
} from 'typeorm';
import { User } from './User';
import { Comment } from './Comment';

@Entity('posts')
export class Post {
  @PrimaryGeneratedColumn('uuid')
  id: string;

  @Column()
  title: string;

  @Column({ nullable: true })
  content: string;

  @Column({ default: false })
  published: boolean;

  @ManyToOne(() => User, (user) => user.posts)
  author: User;

  @Column()
  authorId: string;

  @OneToMany(() => Comment, (comment) => comment.post)
  comments: Comment[];

  @CreateDateColumn()
  createdAt: Date;
}
```

### 2. Create Data Source

```typescript
// data-source.ts
import 'reflect-metadata';
import { DataSource } from 'typeorm';
import { OrmdbDriver } from '@ormdb/typeorm-adapter';
import { User } from './entities/User';
import { Post } from './entities/Post';
import { Comment } from './entities/Comment';

export const AppDataSource = new DataSource({
  type: 'ormdb' as any,
  driver: new OrmdbDriver({
    host: 'localhost',
    port: 8080,
  }),
  entities: [User, Post, Comment],
  synchronize: true,  // Auto-sync schema in development
});
```

### 3. Initialize Connection

```typescript
// index.ts
import { AppDataSource } from './data-source';

async function main() {
  await AppDataSource.initialize();
  console.log('Connected to ORMDB');
}

main().catch(console.error);
```

---

## Repository Pattern

### Get Repository

```typescript
const userRepository = AppDataSource.getRepository(User);
```

### Find Methods

```typescript
// Find all
const users = await userRepository.find();

// Find with conditions
const activeUsers = await userRepository.find({
  where: { status: 'active' },
});

// Find one
const user = await userRepository.findOne({
  where: { id: 'user-123' },
});

// Find one or fail
const user = await userRepository.findOneOrFail({
  where: { email: 'alice@example.com' },
});

// Find by IDs
const users = await userRepository.findByIds(['user-1', 'user-2']);
```

### Relations

```typescript
// Load relations
const user = await userRepository.findOne({
  where: { id: 'user-123' },
  relations: ['posts'],
});

// Nested relations
const user = await userRepository.findOne({
  where: { id: 'user-123' },
  relations: ['posts', 'posts.comments'],
});

// Selective loading
const user = await userRepository.findOne({
  where: { id: 'user-123' },
  relations: {
    posts: true,
    profile: true,
  },
});
```

### Select Specific Columns

```typescript
const users = await userRepository.find({
  select: ['id', 'name', 'email'],
});

// With relations
const users = await userRepository.find({
  select: {
    id: true,
    name: true,
    posts: {
      id: true,
      title: true,
    },
  },
  relations: ['posts'],
});
```

---

## Query Builder

### Basic Query

```typescript
const users = await userRepository
  .createQueryBuilder('user')
  .where('user.status = :status', { status: 'active' })
  .getMany();
```

### Complex Conditions

```typescript
const users = await userRepository
  .createQueryBuilder('user')
  .where('user.status = :status', { status: 'active' })
  .andWhere('user.age >= :age', { age: 18 })
  .orWhere('user.role = :role', { role: 'admin' })
  .getMany();
```

### Joins

```typescript
const users = await userRepository
  .createQueryBuilder('user')
  .leftJoinAndSelect('user.posts', 'post')
  .where('post.published = :published', { published: true })
  .getMany();
```

### Ordering and Pagination

```typescript
const users = await userRepository
  .createQueryBuilder('user')
  .orderBy('user.name', 'ASC')
  .skip(20)
  .take(10)
  .getMany();
```

---

## Mutations

### Save (Insert/Update)

```typescript
// Create new
const user = new User();
user.name = 'Alice';
user.email = 'alice@example.com';
await userRepository.save(user);

// Update existing
user.name = 'Alice Smith';
await userRepository.save(user);

// Save multiple
await userRepository.save([user1, user2, user3]);
```

### Insert

```typescript
// Single insert
await userRepository.insert({
  name: 'Alice',
  email: 'alice@example.com',
});

// Bulk insert
await userRepository.insert([
  { name: 'Alice', email: 'alice@example.com' },
  { name: 'Bob', email: 'bob@example.com' },
]);
```

### Update

```typescript
// Update by criteria
await userRepository.update(
  { status: 'pending' },
  { status: 'active' }
);

// Update by ID
await userRepository.update('user-123', {
  name: 'Alice Smith',
});
```

### Delete

```typescript
// Delete by criteria
await userRepository.delete({ status: 'banned' });

// Delete by ID
await userRepository.delete('user-123');

// Soft delete (ORMDB default)
await userRepository.softDelete('user-123');

// Restore soft-deleted
await userRepository.restore('user-123');
```

---

## Transactions

### Using Transaction Manager

```typescript
await AppDataSource.transaction(async (manager) => {
  const user = manager.create(User, {
    name: 'Alice',
    email: 'alice@example.com',
  });
  await manager.save(user);

  const post = manager.create(Post, {
    title: 'Welcome Post',
    authorId: user.id,
  });
  await manager.save(post);
});
```

### Using Query Runner

```typescript
const queryRunner = AppDataSource.createQueryRunner();
await queryRunner.connect();
await queryRunner.startTransaction();

try {
  await queryRunner.manager.save(user);
  await queryRunner.manager.save(post);
  await queryRunner.commitTransaction();
} catch (err) {
  await queryRunner.rollbackTransaction();
  throw err;
} finally {
  await queryRunner.release();
}
```

---

## Entity Listeners

```typescript
@Entity()
export class User {
  // ... fields

  @BeforeInsert()
  generateId() {
    this.id = uuid();
  }

  @AfterLoad()
  computeFullName() {
    this.fullName = `${this.firstName} ${this.lastName}`;
  }

  @BeforeUpdate()
  updateTimestamp() {
    this.updatedAt = new Date();
  }
}
```

---

## Subscribers

```typescript
@EventSubscriber()
export class UserSubscriber implements EntitySubscriberInterface<User> {
  listenTo() {
    return User;
  }

  beforeInsert(event: InsertEvent<User>) {
    console.log('Before insert:', event.entity);
  }

  afterInsert(event: InsertEvent<User>) {
    console.log('After insert:', event.entity);
  }
}
```

---

## Eager Relations

```typescript
@Entity()
export class Post {
  @ManyToOne(() => User, { eager: true })
  author: User;
}

// Author is automatically loaded
const post = await postRepository.findOne({
  where: { id: 'post-123' },
});
console.log(post.author.name);
```

---

## Cascade Operations

```typescript
@Entity()
export class User {
  @OneToMany(() => Post, (post) => post.author, {
    cascade: true,
  })
  posts: Post[];
}

// Posts are saved automatically
const user = new User();
user.name = 'Alice';
user.posts = [
  { title: 'Post 1' },
  { title: 'Post 2' },
];
await userRepository.save(user);
```

---

## ORMDB-Specific Features

### Access Native Client

```typescript
import { getOrmdbClient } from '@ormdb/typeorm-adapter';

const ormdb = getOrmdbClient(AppDataSource);

// Use native ORMDB features
const result = await ormdb.query(/* ... */);
```

### Query Budget

```typescript
// Via query builder extension
const users = await userRepository
  .createQueryBuilder('user')
  .setParameter('$budget', { maxEntities: 100 })
  .leftJoinAndSelect('user.posts', 'post')
  .getMany();
```

### Include Soft-Deleted

```typescript
const allUsers = await userRepository.find({
  withDeleted: true,
});
```

---

## Migration from PostgreSQL

### 1. Update Data Source

```typescript
// Before
export const AppDataSource = new DataSource({
  type: 'postgres',
  host: 'localhost',
  port: 5432,
  database: 'mydb',
  // ...
});

// After
import { OrmdbDriver } from '@ormdb/typeorm-adapter';

export const AppDataSource = new DataSource({
  type: 'ormdb' as any,
  driver: new OrmdbDriver({
    host: 'localhost',
    port: 8080,
  }),
  // ...
});
```

### 2. Sync Schema

```typescript
// Enable synchronize to create schema
const AppDataSource = new DataSource({
  // ...
  synchronize: true,
});
```

---

## Limitations

| Feature | Status | Notes |
|---------|--------|-------|
| Raw SQL | Not supported | Use native client |
| Query cache | Not supported | ORMDB has built-in cache |
| Views | Not supported | |
| Migrations | Partial | Use ORMDB migrations |
| Many-to-many auto | Partial | Define explicit junction |

---

## Next Steps

- **[Prisma Adapter](prisma.md)** - Schema-first ORM
- **[Drizzle Adapter](drizzle.md)** - Type-safe query builder
- **[Native Client](../clients/typescript.md)** - Direct ORMDB access
