# ORMDB Python Client

A Python client library for ORMDB with support for SQLAlchemy and Django ORM.

## Installation

```bash
pip install ormdb
```

For SQLAlchemy support:
```bash
pip install ormdb[sqlalchemy]
```

For Django support:
```bash
pip install ormdb[django]
```

## Quick Start

### Direct Client Usage

```python
from ormdb import OrmdbClient

# Create a client
client = OrmdbClient("http://localhost:8080")

# Query entities
result = client.query(
    "User",
    fields=["id", "name", "email"],
    filter={"field": "status", "op": "eq", "value": "active"},
    order_by=[{"field": "name", "direction": "asc"}],
    limit=10,
)
print(f"Found {len(result.entities)} users")

# Insert an entity
result = client.insert("User", {
    "name": "Alice",
    "email": "alice@example.com",
    "status": "active",
})
print(f"Inserted user: {result.inserted_ids[0]}")

# Update an entity
client.update("User", result.inserted_ids[0], {"status": "inactive"})

# Delete an entity
client.delete("User", result.inserted_ids[0])
```

### Async Client

```python
import asyncio
from ormdb import AsyncOrmdbClient

async def main():
    async with AsyncOrmdbClient("http://localhost:8080") as client:
        users = await client.query("User", limit=10)
        print(f"Found {len(users.entities)} users")

asyncio.run(main())
```

### SQLAlchemy Integration

```python
from sqlalchemy import create_engine, text
from sqlalchemy.orm import sessionmaker

# Create engine
engine = create_engine("ormdb://localhost:8080")

# Execute queries
with engine.connect() as conn:
    result = conn.execute(text("SELECT * FROM User LIMIT 10"))
    for row in result:
        print(row)

# Use ORM
Session = sessionmaker(bind=engine)
session = Session()

# Reflect existing tables
from sqlalchemy import MetaData
metadata = MetaData()
metadata.reflect(bind=engine)
```

### Django Integration

Add to your Django settings:

```python
DATABASES = {
    'default': {
        'ENGINE': 'ormdb.django',
        'HOST': 'localhost',
        'PORT': 8080,
    }
}
```

Then use Django models as usual:

```python
from django.db import models

class User(models.Model):
    name = models.CharField(max_length=255)
    email = models.EmailField()
    status = models.CharField(max_length=50)

    class Meta:
        db_table = 'User'
```

## API Reference

### OrmdbClient

#### Constructor

```python
OrmdbClient(base_url: str = "http://localhost:8080", timeout: float = 30.0)
```

#### Methods

- `query(entity, *, fields=None, filter=None, includes=None, order_by=None, limit=None, offset=None)` - Execute a query
- `insert(entity, data)` - Insert a new entity
- `update(entity, id, data)` - Update an existing entity
- `delete(entity, id)` - Delete an entity
- `upsert(entity, data, id=None)` - Insert or update an entity
- `health()` - Check gateway health
- `get_schema()` - Get database schema
- `get_replication_status()` - Get replication status
- `stream_changes(from_lsn=0, limit=1000, entities=None)` - Stream change log entries

### Filters

Filters use the following format:

```python
# Simple equality
{"field": "status", "op": "eq", "value": "active"}

# Comparison operators: eq, ne, lt, gt, le, ge
{"field": "age", "op": "ge", "value": 18}

# Like pattern matching
{"field": "name", "op": "like", "value": "Alice%"}

# In clause
{"field": "status", "op": "in", "value": ["active", "pending"]}

# Logical AND
{
    "and": [
        {"field": "status", "op": "eq", "value": "active"},
        {"field": "age", "op": "ge", "value": 18}
    ]
}

# Logical OR
{
    "or": [
        {"field": "status", "op": "eq", "value": "active"},
        {"field": "status", "op": "eq", "value": "pending"}
    ]
}
```

### Includes (Relations)

```python
result = client.query(
    "User",
    includes=[
        {
            "relation": "posts",
            "fields": ["id", "title"],
            "filter": {"field": "published", "op": "eq", "value": True},
            "limit": 5
        }
    ]
)
```

## Error Handling

```python
from ormdb import OrmdbClient, ConnectionError, QueryError, MutationError

client = OrmdbClient()

try:
    result = client.query("NonExistent")
except QueryError as e:
    print(f"Query failed: {e.message} (code: {e.code})")
except ConnectionError as e:
    print(f"Connection failed: {e}")
```

## License

MIT
