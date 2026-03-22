# SQLAlchemy Adapter

Use ORMDB with SQLAlchemy, Python's most powerful ORM.

---

## Installation

```bash
pip install ormdb-sqlalchemy
```

---

## Setup

### 1. Create Engine

```python
from sqlalchemy import create_engine

engine = create_engine('ormdb://localhost:8080')
```

### 2. Define Models

```python
from sqlalchemy import Column, String, Boolean, DateTime, ForeignKey
from sqlalchemy.orm import declarative_base, relationship
from sqlalchemy.dialects.postgresql import UUID
from datetime import datetime
import uuid

Base = declarative_base()

class User(Base):
    __tablename__ = 'users'

    id = Column(UUID(as_uuid=True), primary_key=True, default=uuid.uuid4)
    name = Column(String, nullable=False)
    email = Column(String, nullable=False, unique=True)
    status = Column(String, nullable=False, default='active')
    created_at = Column(DateTime, nullable=False, default=datetime.utcnow)

    posts = relationship('Post', back_populates='author')


class Post(Base):
    __tablename__ = 'posts'

    id = Column(UUID(as_uuid=True), primary_key=True, default=uuid.uuid4)
    title = Column(String, nullable=False)
    content = Column(String, nullable=True)
    published = Column(Boolean, nullable=False, default=False)
    author_id = Column(UUID(as_uuid=True), ForeignKey('users.id'), nullable=False)
    created_at = Column(DateTime, nullable=False, default=datetime.utcnow)

    author = relationship('User', back_populates='posts')
    comments = relationship('Comment', back_populates='post')


class Comment(Base):
    __tablename__ = 'comments'

    id = Column(UUID(as_uuid=True), primary_key=True, default=uuid.uuid4)
    text = Column(String, nullable=False)
    post_id = Column(UUID(as_uuid=True), ForeignKey('posts.id'), nullable=False)
    author_id = Column(UUID(as_uuid=True), ForeignKey('users.id'), nullable=False)

    post = relationship('Post', back_populates='comments')
    author = relationship('User')
```

### 3. Create Tables

```python
Base.metadata.create_all(engine)
```

### 4. Create Session

```python
from sqlalchemy.orm import Session

with Session(engine) as session:
    # Use session
    pass
```

---

## Basic Queries

### Query All

```python
with Session(engine) as session:
    users = session.query(User).all()
```

### Filter

```python
# Equality
active_users = session.query(User).filter(User.status == 'active').all()

# Not equal
non_banned = session.query(User).filter(User.status != 'banned').all()

# Greater than
adults = session.query(User).filter(User.age > 18).all()

# LIKE
gmail_users = session.query(User).filter(User.email.like('%@gmail.com')).all()

# IN
statuses = session.query(User).filter(User.status.in_(['active', 'pending'])).all()

# IS NULL
no_email = session.query(User).filter(User.email.is_(None)).all()

# IS NOT NULL
has_email = session.query(User).filter(User.email.isnot(None)).all()
```

### Compound Filters

```python
from sqlalchemy import and_, or_, not_

# AND
active_adults = session.query(User).filter(
    and_(User.status == 'active', User.age >= 18)
).all()

# OR
admins_or_mods = session.query(User).filter(
    or_(User.role == 'admin', User.role == 'moderator')
).all()

# NOT
not_banned = session.query(User).filter(
    not_(User.status == 'banned')
).all()
```

### Ordering

```python
# Ascending
users = session.query(User).order_by(User.name.asc()).all()

# Descending
users = session.query(User).order_by(User.created_at.desc()).all()

# Multiple
users = session.query(User).order_by(User.status, User.name.asc()).all()
```

### Pagination

```python
users = session.query(User).limit(10).offset(20).all()
```

### First/One

```python
# First result or None
user = session.query(User).filter(User.status == 'active').first()

# Exactly one or raise
user = session.query(User).filter(User.id == user_id).one()

# One or None
user = session.query(User).filter(User.email == email).one_or_none()
```

---

## Relations

### Eager Loading

```python
from sqlalchemy.orm import joinedload, selectinload

# Load posts with users
users = session.query(User).options(
    joinedload(User.posts)
).all()

# Nested loading
users = session.query(User).options(
    joinedload(User.posts).joinedload(Post.comments)
).all()

# Select-in loading (better for many)
users = session.query(User).options(
    selectinload(User.posts)
).all()
```

### Lazy Loading

```python
# Default behavior - loads on access
user = session.query(User).first()
posts = user.posts  # Triggers additional query
```

### Filtering Related

```python
# Users with published posts
users = session.query(User).join(User.posts).filter(
    Post.published == True
).distinct().all()
```

---

## Select Specific Columns

```python
# Specific columns
results = session.query(User.id, User.name).all()

# As tuples
for id, name in results:
    print(f"{id}: {name}")
```

---

## Mutations

### Insert

```python
# Single insert
user = User(name='Alice', email='alice@example.com')
session.add(user)
session.commit()

# Bulk insert
users = [
    User(name='Alice', email='alice@example.com'),
    User(name='Bob', email='bob@example.com'),
]
session.add_all(users)
session.commit()
```

### Update

```python
# Update instance
user = session.query(User).filter(User.id == user_id).one()
user.name = 'Alice Smith'
session.commit()

# Bulk update
session.query(User).filter(User.status == 'pending').update(
    {'status': 'active'}
)
session.commit()
```

### Delete

```python
# Delete instance
user = session.query(User).filter(User.id == user_id).one()
session.delete(user)
session.commit()

# Bulk delete
session.query(User).filter(User.status == 'banned').delete()
session.commit()
```

---

## Transactions

### Automatic Transaction

```python
with Session(engine) as session:
    user = User(name='Alice', email='alice@example.com')
    session.add(user)

    post = Post(title='Hello', author=user)
    session.add(post)

    session.commit()  # Both saved atomically
```

### Manual Transaction

```python
with Session(engine) as session:
    try:
        session.begin()
        # ... operations
        session.commit()
    except:
        session.rollback()
        raise
```

### Savepoints

```python
with Session(engine) as session:
    user = User(name='Alice', email='alice@example.com')
    session.add(user)

    savepoint = session.begin_nested()
    try:
        # Risky operation
        session.commit()
    except:
        savepoint.rollback()

    session.commit()
```

---

## Aggregations

```python
from sqlalchemy import func

# Count
count = session.query(func.count(User.id)).filter(
    User.status == 'active'
).scalar()

# Sum
total = session.query(func.sum(Post.views)).scalar()

# Avg
avg = session.query(func.avg(Post.views)).scalar()

# Min/Max
oldest = session.query(func.min(User.created_at)).scalar()
newest = session.query(func.max(User.created_at)).scalar()

# Group by
stats = session.query(
    User.status,
    func.count(User.id)
).group_by(User.status).all()
```

---

## Async Support (SQLAlchemy 2.0)

```python
from sqlalchemy.ext.asyncio import create_async_engine, AsyncSession

# Async engine
engine = create_async_engine('ormdb+async://localhost:8080')

async with AsyncSession(engine) as session:
    result = await session.execute(
        select(User).where(User.status == 'active')
    )
    users = result.scalars().all()
```

---

## ORMDB-Specific Features

### Access Native Client

```python
from ormdb_sqlalchemy import get_ormdb_client

ormdb = get_ormdb_client(engine)

# Use native ORMDB features
result = ormdb.query(GraphQuery(...))
```

### Query Budget

```python
from ormdb_sqlalchemy import with_budget

# Set budget for query
users = session.query(User).options(
    with_budget(max_entities=100)
).all()
```

### Include Soft-Deleted

```python
from ormdb_sqlalchemy import include_deleted

users = session.query(User).options(
    include_deleted()
).all()
```

---

## Event Hooks

```python
from sqlalchemy import event

@event.listens_for(User, 'before_insert')
def before_insert(mapper, connection, target):
    target.created_at = datetime.utcnow()

@event.listens_for(User, 'after_update')
def after_update(mapper, connection, target):
    print(f"User {target.id} updated")
```

---

## Migration from PostgreSQL

### 1. Update Connection String

```python
# Before
engine = create_engine('postgresql://user:pass@localhost/db')

# After
engine = create_engine('ormdb://localhost:8080')
```

### 2. Create Schema

```python
# Sync models to ORMDB
Base.metadata.create_all(engine)
```

### 3. Migrate Data

```bash
# Export from PostgreSQL
pg_dump -Fc mydb > backup.dump

# Import to ORMDB
ormdb import --from-pg backup.dump
```

---

## Limitations

| Feature | Status | Notes |
|---------|--------|-------|
| Raw SQL | Not supported | Use native client |
| text() constructs | Not supported | |
| Reflection | Partial | Limited introspection |
| Alembic | Partial | Use ORMDB migrations |
| Hybrid properties | Works | Client-side only |

---

## Next Steps

- **[Django Adapter](django.md)** - Django ORM integration
- **[Python Client](../clients/python.md)** - Direct ORMDB access
- **[Migration Guide](../guides/schema-migrations.md)** - Schema management
