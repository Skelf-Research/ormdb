"""ORMDB Python Client.

A Python client for interacting with ORMDB via its HTTP/JSON gateway.

Usage:
    from ormdb import OrmdbClient

    client = OrmdbClient("http://localhost:8080")

    # Query entities
    users = client.query("User", filter={"status": "active"})

    # Insert entity
    result = client.insert("User", {"name": "Alice", "email": "alice@example.com"})

    # Update entity
    client.update("User", result["id"], {"status": "inactive"})

    # Delete entity
    client.delete("User", result["id"])
"""

from .client import OrmdbClient
from .exceptions import OrmdbError, ConnectionError, QueryError, MutationError
from .types import QueryResult, MutationResult

__version__ = "0.1.0"
__all__ = [
    "OrmdbClient",
    "OrmdbError",
    "ConnectionError",
    "QueryError",
    "MutationError",
    "QueryResult",
    "MutationResult",
]
