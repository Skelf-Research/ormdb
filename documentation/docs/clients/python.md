# Python Client

Python client for ORMDB with SQLAlchemy and Django support.

## Installation

```bash
pip install ormdb

# With SQLAlchemy support
pip install ormdb[sqlalchemy]

# With Django support
pip install ormdb[django]
```

## Direct Client

### Basic Usage

```python
from ormdb import OrmdbClient

client = OrmdbClient("http://localhost:8080")

# Query
result = client.query(
    "User",
    fields=["id", "name", "email"],
    filter={"field": "status", "op": "eq", "value": "active"},
    order_by=[{"field": "name", "direction": "asc"}],
    limit=10,
)

for user in result.entities:
    print(f"{user['name']} <{user['email']}>")

# Insert
result = client.insert("User", {
    "name": "Alice",
    "email": "alice@example.com",
})
user_id = result.inserted_ids[0]

# Update
client.update("User", user_id, {"name": "Alice Smith"})

# Delete
client.delete("User", user_id)
```

### Async Client

```python
import asyncio
from ormdb import AsyncOrmdbClient

async def main():
    async with AsyncOrmdbClient("http://localhost:8080") as client:
        result = await client.query("User", limit=10)
        for user in result.entities:
            print(user["name"])

asyncio.run(main())
```

### Configuration

```python
client = OrmdbClient(
    base_url="http://localhost:8080",
    timeout=30.0,
    headers={"Authorization": "Bearer token"},
)
```

## Query Features

### Filtering

```python
# Simple filter
result = client.query("User",
    filter={"field": "status", "op": "eq", "value": "active"})

# AND filter
result = client.query("User",
    filter={
        "and": [
            {"field": "status", "op": "eq", "value": "active"},
            {"field": "age", "op": "ge", "value": 18},
        ]
    })

# OR filter
result = client.query("User",
    filter={
        "or": [
            {"field": "role", "op": "eq", "value": "admin"},
            {"field": "role", "op": "eq", "value": "moderator"},
        ]
    })

# Pattern matching
result = client.query("User",
    filter={"field": "email", "op": "like", "value": "%@example.com"})

# IN operator
result = client.query("User",
    filter={"field": "status", "op": "in", "value": ["active", "pending"]})
```

### Including Relations

```python
# Single include
result = client.query("User",
    includes=[{"relation": "posts"}])

for user in result.entities:
    print(f"{user['name']} has {len(user.get('posts', []))} posts")

# Nested includes
result = client.query("User",
    includes=[{
        "relation": "posts",
        "fields": ["id", "title"],
        "filter": {"field": "published", "op": "eq", "value": True},
        "includes": [{"relation": "comments"}],
    }])
```

### Pagination

```python
# First page
result = client.query("User", limit=10, offset=0)

# Iterate through pages
offset = 0
while True:
    result = client.query("User", limit=100, offset=offset)
    for user in result.entities:
        process(user)

    if not result.has_more:
        break
    offset += 100
```

## SQLAlchemy Integration

```python
from sqlalchemy import create_engine, text, MetaData
from sqlalchemy.orm import sessionmaker

# Create engine
engine = create_engine("ormdb://localhost:8080")

# Raw SQL-like queries
with engine.connect() as conn:
    result = conn.execute(text("SELECT * FROM User LIMIT 10"))
    for row in result:
        print(row)

# Reflect tables
metadata = MetaData()
metadata.reflect(bind=engine)

# Use ORM
Session = sessionmaker(bind=engine)
session = Session()

# Query using reflected tables
User = metadata.tables["User"]
users = session.query(User).filter(User.c.status == "active").all()
```

## Django Integration

### Settings

```python
# settings.py
DATABASES = {
    'default': {
        'ENGINE': 'ormdb.django',
        'HOST': 'localhost',
        'PORT': 8080,
    }
}
```

### Models

```python
# models.py
from django.db import models

class User(models.Model):
    name = models.CharField(max_length=255)
    email = models.EmailField()
    status = models.CharField(max_length=50)

    class Meta:
        db_table = 'User'

class Post(models.Model):
    title = models.CharField(max_length=255)
    content = models.TextField()
    author = models.ForeignKey(User, on_delete=models.CASCADE)

    class Meta:
        db_table = 'Post'
```

### Queries

```python
# views.py
from .models import User, Post

# Query
active_users = User.objects.filter(status="active").order_by("name")[:10]

# Create
user = User.objects.create(name="Alice", email="alice@example.com")

# Update
User.objects.filter(id=user.id).update(status="inactive")

# Delete
User.objects.filter(id=user.id).delete()

# Relations
user = User.objects.prefetch_related("post_set").get(id=user_id)
for post in user.post_set.all():
    print(post.title)
```

## Error Handling

```python
from ormdb import OrmdbClient, ConnectionError, QueryError, MutationError

client = OrmdbClient()

try:
    result = client.insert("User", {"email": "existing@example.com"})
except MutationError as e:
    if e.code == "UNIQUE_VIOLATION":
        print("Email already exists")
    elif e.code == "FOREIGN_KEY_VIOLATION":
        print("Invalid reference")
    else:
        print(f"Mutation failed: {e.message}")
except QueryError as e:
    print(f"Query failed: {e.message}")
except ConnectionError as e:
    print(f"Connection failed: {e}")
```

## Change Data Capture

```python
# Stream changes
for change in client.stream_changes(from_lsn=0, entities=["User", "Post"]):
    if change["type"] == "insert":
        print(f"New {change['entity']}: {change['id']}")
    elif change["type"] == "update":
        print(f"Updated {change['entity']}: {change['id']}")
    elif change["type"] == "delete":
        print(f"Deleted {change['entity']}: {change['id']}")
```

## Best Practices

1. **Use context managers** for async client
2. **Handle pagination** for large datasets
3. **Use appropriate ORM integration** for existing projects
4. **Set timeouts** for production use

## Next Steps

- **[Query API Reference](../reference/query-api.md)**
- **[Error Reference](../reference/errors.md)**
