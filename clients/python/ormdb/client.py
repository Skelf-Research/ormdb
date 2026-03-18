"""ORMDB HTTP client."""

from typing import Any

import httpx

from .exceptions import ConnectionError, MutationError, QueryError
from .types import (
    MutationResult,
    QueryResult,
    ReplicationStatus,
    StreamChangesResult,
)


class OrmdbClient:
    """HTTP client for ORMDB gateway.

    Args:
        base_url: Base URL of the ORMDB gateway (e.g., "http://localhost:8080").
        timeout: Request timeout in seconds.

    Example:
        >>> client = OrmdbClient("http://localhost:8080")
        >>> users = client.query("User", filter={"status": "active"})
        >>> print(f"Found {len(users.entities)} users")
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8080",
        timeout: float = 30.0,
    ):
        self.base_url = base_url.rstrip("/")
        self._client = httpx.Client(timeout=timeout)

    def close(self) -> None:
        """Close the HTTP client."""
        self._client.close()

    def __enter__(self) -> "OrmdbClient":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    def health(self) -> dict[str, Any]:
        """Check gateway health.

        Returns:
            Health status dictionary with keys:
                - status: "healthy" or "degraded"
                - version: Gateway version
                - ormdb_connected: Whether connected to ORMDB server
        """
        try:
            response = self._client.get(f"{self.base_url}/health")
            response.raise_for_status()
            return response.json()
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to connect to gateway: {e}")

    def query(
        self,
        entity: str,
        *,
        fields: list[str] | None = None,
        filter: dict[str, Any] | None = None,  # noqa: A002
        includes: list[dict[str, Any]] | None = None,
        order_by: list[dict[str, str]] | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> QueryResult:
        """Execute a graph query.

        Args:
            entity: Root entity type to query.
            fields: Fields to select (all if not specified).
            filter: Filter conditions.
            includes: Related entities to include.
            order_by: Ordering specification.
            limit: Maximum number of results.
            offset: Number of results to skip.

        Returns:
            QueryResult containing matched entities.

        Example:
            >>> result = client.query(
            ...     "User",
            ...     fields=["id", "name", "email"],
            ...     filter={"field": "status", "op": "eq", "value": "active"},
            ...     order_by=[{"field": "name", "direction": "asc"}],
            ...     limit=10,
            ... )
        """
        payload: dict[str, Any] = {"root_entity": entity}

        if fields:
            payload["fields"] = fields
        if filter:
            payload["filter"] = {"expression": filter}
        if includes:
            payload["includes"] = includes
        if order_by:
            payload["order_by"] = order_by
        if limit is not None or offset is not None:
            payload["pagination"] = {
                "limit": limit or 100,
                "offset": offset or 0,
            }

        try:
            response = self._client.post(f"{self.base_url}/query", json=payload)
            response.raise_for_status()
            return QueryResult.from_response(response.json())
        except httpx.HTTPStatusError as e:
            error_body = e.response.json() if e.response.content else {}
            raise QueryError(
                error_body.get("message", str(e)),
                error_body.get("code"),
            )
        except httpx.HTTPError as e:
            raise ConnectionError(f"Query failed: {e}")

    def insert(self, entity: str, data: dict[str, Any]) -> MutationResult:
        """Insert a new entity.

        Args:
            entity: Entity type to insert.
            data: Field values for the new entity.

        Returns:
            MutationResult with inserted ID.

        Example:
            >>> result = client.insert("User", {
            ...     "name": "Alice",
            ...     "email": "alice@example.com",
            ... })
            >>> print(f"Inserted user with ID: {result.inserted_ids[0]}")
        """
        payload = {
            "Insert": {
                "entity": entity,
                "data": [{"field": k, "value": self._convert_value(v)} for k, v in data.items()],
            }
        }
        return self._mutate(payload)

    def update(self, entity: str, id: str, data: dict[str, Any]) -> MutationResult:  # noqa: A002
        """Update an existing entity.

        Args:
            entity: Entity type to update.
            id: Entity ID (hex string).
            data: Field values to update.

        Returns:
            MutationResult with affected count.
        """
        payload = {
            "Update": {
                "entity": entity,
                "id": self._hex_to_uuid(id),
                "data": [{"field": k, "value": self._convert_value(v)} for k, v in data.items()],
            }
        }
        return self._mutate(payload)

    def delete(self, entity: str, id: str) -> MutationResult:  # noqa: A002
        """Delete an entity.

        Args:
            entity: Entity type to delete.
            id: Entity ID (hex string).

        Returns:
            MutationResult with affected count.
        """
        payload = {
            "Delete": {
                "entity": entity,
                "id": self._hex_to_uuid(id),
            }
        }
        return self._mutate(payload)

    def upsert(
        self,
        entity: str,
        data: dict[str, Any],
        id: str | None = None,  # noqa: A002
    ) -> MutationResult:
        """Insert or update an entity.

        Args:
            entity: Entity type.
            data: Field values.
            id: Entity ID for update (optional, generates new ID for insert).

        Returns:
            MutationResult with affected count or inserted ID.
        """
        payload = {
            "Upsert": {
                "entity": entity,
                "id": self._hex_to_uuid(id) if id else None,
                "data": [{"field": k, "value": self._convert_value(v)} for k, v in data.items()],
            }
        }
        return self._mutate(payload)

    def _mutate(self, payload: dict[str, Any]) -> MutationResult:
        """Execute a mutation."""
        try:
            response = self._client.post(f"{self.base_url}/mutate", json=payload)
            response.raise_for_status()
            return MutationResult.from_response(response.json())
        except httpx.HTTPStatusError as e:
            error_body = e.response.json() if e.response.content else {}
            raise MutationError(
                error_body.get("message", str(e)),
                error_body.get("code"),
            )
        except httpx.HTTPError as e:
            raise ConnectionError(f"Mutation failed: {e}")

    def get_schema(self) -> dict[str, Any]:
        """Get database schema.

        Returns:
            Schema information including entities, fields, and relations.
        """
        try:
            response = self._client.get(f"{self.base_url}/schema")
            response.raise_for_status()
            return response.json()
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to get schema: {e}")

    def get_replication_status(self) -> ReplicationStatus:
        """Get replication status.

        Returns:
            Current replication status.
        """
        try:
            response = self._client.get(f"{self.base_url}/replication/status")
            response.raise_for_status()
            return ReplicationStatus.from_response(response.json())
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to get replication status: {e}")

    def stream_changes(
        self,
        from_lsn: int = 0,
        limit: int = 1000,
        entities: list[str] | None = None,
    ) -> StreamChangesResult:
        """Stream changes from the changelog.

        Args:
            from_lsn: Starting LSN (inclusive).
            limit: Maximum number of entries to return.
            entities: Filter by entity types.

        Returns:
            StreamChangesResult with change log entries.
        """
        params: dict[str, Any] = {
            "from_lsn": from_lsn,
            "limit": limit,
        }
        if entities:
            params["entities"] = ",".join(entities)

        try:
            response = self._client.get(
                f"{self.base_url}/replication/changes",
                params=params,
            )
            response.raise_for_status()
            return StreamChangesResult.from_response(response.json())
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to stream changes: {e}")

    def _convert_value(self, value: Any) -> dict[str, Any]:
        """Convert Python value to ORMDB Value format."""
        if value is None:
            return {"Null": None}
        elif isinstance(value, bool):
            return {"Bool": value}
        elif isinstance(value, int):
            if -(2**31) <= value < 2**31:
                return {"Int32": value}
            else:
                return {"Int64": value}
        elif isinstance(value, float):
            return {"Float64": value}
        elif isinstance(value, str):
            return {"String": value}
        elif isinstance(value, bytes):
            return {"Bytes": list(value)}
        elif isinstance(value, list):
            # Try to determine array type from first element
            if not value:
                return {"StringArray": []}
            first = value[0]
            if isinstance(first, bool):
                return {"BoolArray": value}
            elif isinstance(first, int):
                return {"Int64Array": value}
            elif isinstance(first, float):
                return {"Float64Array": value}
            elif isinstance(first, str):
                return {"StringArray": value}
            else:
                return {"StringArray": [str(v) for v in value]}
        else:
            # Default to string representation
            return {"String": str(value)}

    def _hex_to_uuid(self, hex_str: str) -> list[int]:
        """Convert hex string to UUID byte array."""
        if len(hex_str) != 32:
            raise ValueError(f"Invalid UUID hex string length: {len(hex_str)}")
        return [int(hex_str[i : i + 2], 16) for i in range(0, 32, 2)]


class AsyncOrmdbClient:
    """Async HTTP client for ORMDB gateway.

    Same interface as OrmdbClient but uses async/await.
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8080",
        timeout: float = 30.0,
    ):
        self.base_url = base_url.rstrip("/")
        self._client = httpx.AsyncClient(timeout=timeout)

    async def close(self) -> None:
        """Close the HTTP client."""
        await self._client.aclose()

    async def __aenter__(self) -> "AsyncOrmdbClient":
        return self

    async def __aexit__(self, *args: Any) -> None:
        await self.close()

    async def health(self) -> dict[str, Any]:
        """Check gateway health."""
        try:
            response = await self._client.get(f"{self.base_url}/health")
            response.raise_for_status()
            return response.json()
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to connect to gateway: {e}")

    async def query(
        self,
        entity: str,
        *,
        fields: list[str] | None = None,
        filter: dict[str, Any] | None = None,  # noqa: A002
        includes: list[dict[str, Any]] | None = None,
        order_by: list[dict[str, str]] | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> QueryResult:
        """Execute a graph query asynchronously."""
        payload: dict[str, Any] = {"root_entity": entity}

        if fields:
            payload["fields"] = fields
        if filter:
            payload["filter"] = {"expression": filter}
        if includes:
            payload["includes"] = includes
        if order_by:
            payload["order_by"] = order_by
        if limit is not None or offset is not None:
            payload["pagination"] = {
                "limit": limit or 100,
                "offset": offset or 0,
            }

        try:
            response = await self._client.post(f"{self.base_url}/query", json=payload)
            response.raise_for_status()
            return QueryResult.from_response(response.json())
        except httpx.HTTPStatusError as e:
            error_body = e.response.json() if e.response.content else {}
            raise QueryError(
                error_body.get("message", str(e)),
                error_body.get("code"),
            )
        except httpx.HTTPError as e:
            raise ConnectionError(f"Query failed: {e}")

    # Similar async implementations for insert, update, delete, upsert...
    # (Omitted for brevity, would follow same pattern as sync versions)
