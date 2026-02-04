# Sequelize Adapter

Use ORMDB with Sequelize, a mature promise-based Node.js ORM.

---

## Installation

```bash
npm install sequelize @ormdb/sequelize-adapter
# or
yarn add sequelize @ormdb/sequelize-adapter
```

---

## Setup

### 1. Create Connection

```typescript
import { Sequelize } from 'sequelize';
import { OrmdbDialect } from '@ormdb/sequelize-adapter';

const sequelize = new Sequelize({
  dialect: OrmdbDialect,
  host: 'localhost',
  port: 8080,
  pool: {
    max: 10,
    min: 2,
  },
});
```

### 2. Define Models

```typescript
import { DataTypes, Model } from 'sequelize';

class User extends Model {
  declare id: string;
  declare name: string;
  declare email: string;
  declare status: string;
  declare createdAt: Date;
}

User.init(
  {
    id: {
      type: DataTypes.UUID,
      defaultValue: DataTypes.UUIDV4,
      primaryKey: true,
    },
    name: {
      type: DataTypes.STRING,
      allowNull: false,
    },
    email: {
      type: DataTypes.STRING,
      allowNull: false,
      unique: true,
    },
    status: {
      type: DataTypes.STRING,
      allowNull: false,
      defaultValue: 'active',
    },
  },
  {
    sequelize,
    tableName: 'users',
    timestamps: true,
    createdAt: 'created_at',
    updatedAt: false,
  }
);

class Post extends Model {
  declare id: string;
  declare title: string;
  declare content: string | null;
  declare published: boolean;
  declare authorId: string;
}

Post.init(
  {
    id: {
      type: DataTypes.UUID,
      defaultValue: DataTypes.UUIDV4,
      primaryKey: true,
    },
    title: {
      type: DataTypes.STRING,
      allowNull: false,
    },
    content: {
      type: DataTypes.TEXT,
      allowNull: true,
    },
    published: {
      type: DataTypes.BOOLEAN,
      allowNull: false,
      defaultValue: false,
    },
    authorId: {
      type: DataTypes.UUID,
      allowNull: false,
      field: 'author_id',
    },
  },
  {
    sequelize,
    tableName: 'posts',
    timestamps: true,
    createdAt: 'created_at',
    updatedAt: false,
  }
);

// Define associations
User.hasMany(Post, { foreignKey: 'authorId', as: 'posts' });
Post.belongsTo(User, { foreignKey: 'authorId', as: 'author' });
```

### 3. Sync Schema

```typescript
await sequelize.sync();
```

---

## Basic Queries

### Find All

```typescript
const users = await User.findAll();
```

### Find with Conditions

```typescript
import { Op } from 'sequelize';

// Equality
const activeUsers = await User.findAll({
  where: { status: 'active' },
});

// Not equal
const nonBanned = await User.findAll({
  where: { status: { [Op.ne]: 'banned' } },
});

// Greater than
const adults = await User.findAll({
  where: { age: { [Op.gt]: 18 } },
});

// LIKE
const gmailUsers = await User.findAll({
  where: { email: { [Op.like]: '%@gmail.com' } },
});

// IN
const multiStatus = await User.findAll({
  where: { status: { [Op.in]: ['active', 'pending'] } },
});

// IS NULL
const noEmail = await User.findAll({
  where: { email: { [Op.is]: null } },
});
```

### Compound Conditions

```typescript
// AND (implicit)
const activeAdults = await User.findAll({
  where: {
    status: 'active',
    age: { [Op.gte]: 18 },
  },
});

// OR
const adminsOrMods = await User.findAll({
  where: {
    [Op.or]: [{ role: 'admin' }, { role: 'moderator' }],
  },
});

// Complex
const complex = await User.findAll({
  where: {
    [Op.and]: [
      { status: 'active' },
      {
        [Op.or]: [{ role: 'admin' }, { age: { [Op.gte]: 21 } }],
      },
    ],
  },
});
```

### Ordering

```typescript
const users = await User.findAll({
  order: [
    ['name', 'ASC'],
    ['createdAt', 'DESC'],
  ],
});
```

### Pagination

```typescript
const users = await User.findAll({
  limit: 10,
  offset: 20,
});
```

### Find One

```typescript
// Find by primary key
const user = await User.findByPk('user-123');

// Find one with condition
const user = await User.findOne({
  where: { email: 'alice@example.com' },
});
```

---

## Relations

### Include (Eager Loading)

```typescript
// Single relation
const users = await User.findAll({
  include: ['posts'],
});

// Multiple relations
const users = await User.findAll({
  include: ['posts', 'profile'],
});

// Nested
const users = await User.findAll({
  include: [
    {
      model: Post,
      as: 'posts',
      include: ['comments'],
    },
  ],
});
```

### Filtered Include

```typescript
const users = await User.findAll({
  include: [
    {
      model: Post,
      as: 'posts',
      where: { published: true },
      required: false,  // LEFT JOIN
    },
  ],
});
```

### Attributes on Include

```typescript
const users = await User.findAll({
  include: [
    {
      model: Post,
      as: 'posts',
      attributes: ['id', 'title'],
    },
  ],
});
```

---

## Select Attributes

```typescript
// Include specific
const users = await User.findAll({
  attributes: ['id', 'name', 'email'],
});

// Exclude specific
const users = await User.findAll({
  attributes: { exclude: ['password'] },
});
```

---

## Mutations

### Create

```typescript
// Single create
const user = await User.create({
  name: 'Alice',
  email: 'alice@example.com',
});

// Bulk create
const users = await User.bulkCreate([
  { name: 'Alice', email: 'alice@example.com' },
  { name: 'Bob', email: 'bob@example.com' },
]);
```

### Update

```typescript
// Update instance
const user = await User.findByPk('user-123');
user.name = 'Alice Smith';
await user.save();

// Update with object
await user.update({ name: 'Alice Smith' });

// Update specific fields
await user.update({ name: 'Alice Smith' }, { fields: ['name'] });

// Bulk update
await User.update(
  { status: 'active' },
  { where: { status: 'pending' } }
);
```

### Delete

```typescript
// Delete instance
const user = await User.findByPk('user-123');
await user.destroy();

// Bulk delete
await User.destroy({
  where: { status: 'banned' },
});
```

### Upsert

```typescript
const [user, created] = await User.upsert({
  id: 'user-123',
  name: 'Alice',
  email: 'alice@example.com',
});
```

---

## Transactions

```typescript
// Managed transaction
await sequelize.transaction(async (t) => {
  const user = await User.create(
    { name: 'Alice', email: 'alice@example.com' },
    { transaction: t }
  );

  await Post.create(
    { title: 'Welcome', authorId: user.id },
    { transaction: t }
  );
});

// Manual transaction
const t = await sequelize.transaction();
try {
  await User.create({ ... }, { transaction: t });
  await Post.create({ ... }, { transaction: t });
  await t.commit();
} catch (error) {
  await t.rollback();
  throw error;
}
```

---

## Aggregations

```typescript
// Count
const count = await User.count({
  where: { status: 'active' },
});

// Sum
const total = await Post.sum('views');

// Min/Max
const oldest = await User.min('createdAt');
const newest = await User.max('createdAt');

// Group by
const stats = await User.findAll({
  attributes: [
    'status',
    [sequelize.fn('COUNT', sequelize.col('id')), 'count'],
  ],
  group: ['status'],
});
```

---

## Hooks

```typescript
User.beforeCreate((user) => {
  user.email = user.email.toLowerCase();
});

User.afterCreate((user) => {
  console.log(`User ${user.id} created`);
});

User.beforeUpdate((user) => {
  user.updatedAt = new Date();
});
```

---

## Scopes

```typescript
User.init(
  { /* ... */ },
  {
    sequelize,
    scopes: {
      active: {
        where: { status: 'active' },
      },
      withPosts: {
        include: ['posts'],
      },
    },
  }
);

// Use scopes
const activeUsers = await User.scope('active').findAll();
const usersWithPosts = await User.scope(['active', 'withPosts']).findAll();
```

---

## ORMDB-Specific Features

### Access Native Client

```typescript
import { getOrmdbClient } from '@ormdb/sequelize-adapter';

const ormdb = getOrmdbClient(sequelize);

// Use native ORMDB features
const result = await ormdb.query(/* ... */);
```

### Include Soft-Deleted

```typescript
// ORMDB soft deletes by default
const users = await User.findAll({
  paranoid: false,  // Include deleted
});
```

---

## Limitations

| Feature | Status | Notes |
|---------|--------|-------|
| Raw queries | Not supported | Use native client |
| Migrations | Partial | Use ORMDB migrations |
| Associations | Partial | Basic support |
| Paranoid mode | Built-in | ORMDB default |

---

## Next Steps

- **[Kysely Adapter](kysely.md)** - Type-safe query builder
- **[Native Client](../clients/typescript.md)** - Direct ORMDB access
