"""ORMDB HTTP client."""

from typing import Any

import httpx

from .exceptions import ConnectionError, MutationError, QueryError
from .types import (
    MutationResult,
    QueryResult,
    ReplicationStatus,
    SearchFilter,
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

    # ========================================================================
    # Search Methods
    # ========================================================================

    def vector_search(
        self,
        entity: str,
        field: str,
        query_vector: list[float],
        k: int,
        *,
        max_distance: float | None = None,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
    ) -> QueryResult:
        """Perform vector similarity search using HNSW index.

        Args:
            entity: Entity type to search.
            field: Vector field to search in.
            query_vector: Query vector for similarity comparison.
            k: Number of nearest neighbors to return.
            max_distance: Maximum distance threshold (optional).
            fields: Fields to include in results.
            includes: Related entities to include.

        Returns:
            QueryResult containing k-nearest entities by vector similarity.

        Example:
            >>> result = client.vector_search(
            ...     "Product",
            ...     "embedding",
            ...     query_vector=[0.1, 0.2, 0.3, ...],
            ...     k=10,
            ...     max_distance=0.5,
            ... )
        """
        filter_expr: dict[str, Any] = {
            "vector_nearest_neighbor": {
                "field": field,
                "query_vector": query_vector,
                "k": k,
            }
        }
        if max_distance is not None:
            filter_expr["vector_nearest_neighbor"]["max_distance"] = max_distance

        return self.query(entity, fields=fields, filter=filter_expr, includes=includes)

    def geo_search(
        self,
        entity: str,
        field: str,
        center_lat: float,
        center_lon: float,
        radius_km: float,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Search for entities within a geographic radius.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            center_lat: Latitude of the center point.
            center_lon: Longitude of the center point.
            radius_km: Search radius in kilometers.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities within the radius.

        Example:
            >>> result = client.geo_search(
            ...     "Restaurant",
            ...     "location",
            ...     center_lat=37.7749,
            ...     center_lon=-122.4194,
            ...     radius_km=5.0,
            ... )
        """
        filter_expr = {
            "geo_within_radius": {
                "field": field,
                "center_lat": center_lat,
                "center_lon": center_lon,
                "radius_km": radius_km,
            }
        }
        return self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    def geo_box_search(
        self,
        entity: str,
        field: str,
        min_lat: float,
        min_lon: float,
        max_lat: float,
        max_lon: float,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Search for entities within a geographic bounding box.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            min_lat: Minimum latitude of the bounding box.
            min_lon: Minimum longitude of the bounding box.
            max_lat: Maximum latitude of the bounding box.
            max_lon: Maximum longitude of the bounding box.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities within the bounding box.
        """
        filter_expr = {
            "geo_within_box": {
                "field": field,
                "min_lat": min_lat,
                "min_lon": min_lon,
                "max_lat": max_lat,
                "max_lon": max_lon,
            }
        }
        return self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    def geo_polygon_search(
        self,
        entity: str,
        field: str,
        vertices: list[tuple[float, float]],
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Search for entities within a geographic polygon.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            vertices: List of (lat, lon) tuples defining the polygon.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities within the polygon.
        """
        filter_expr = {
            "geo_within_polygon": {
                "field": field,
                "vertices": vertices,
            }
        }
        return self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    def geo_nearest(
        self,
        entity: str,
        field: str,
        center_lat: float,
        center_lon: float,
        k: int,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
    ) -> QueryResult:
        """Find k-nearest entities by geographic distance.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            center_lat: Latitude of the center point.
            center_lon: Longitude of the center point.
            k: Number of nearest neighbors to return.
            fields: Fields to include in results.
            includes: Related entities to include.

        Returns:
            QueryResult containing k-nearest entities by distance.
        """
        filter_expr = {
            "geo_nearest_neighbor": {
                "field": field,
                "center_lat": center_lat,
                "center_lon": center_lon,
                "k": k,
            }
        }
        return self.query(entity, fields=fields, filter=filter_expr, includes=includes)

    def text_search(
        self,
        entity: str,
        field: str,
        query: str,
        *,
        min_score: float | None = None,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Perform full-text search with BM25 scoring.

        Args:
            entity: Entity type to search.
            field: Text field to search in.
            query: Search query (will be tokenized and stemmed).
            min_score: Minimum relevance score threshold (0.0 to 1.0).
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing matching entities ranked by relevance.

        Example:
            >>> result = client.text_search(
            ...     "Article",
            ...     "content",
            ...     "rust programming language",
            ...     min_score=0.5,
            ... )
        """
        filter_expr: dict[str, Any] = {
            "text_match": {
                "field": field,
                "query": query,
            }
        }
        if min_score is not None:
            filter_expr["text_match"]["min_score"] = min_score

        return self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    def text_phrase_search(
        self,
        entity: str,
        field: str,
        phrase: str,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Perform exact phrase search.

        Args:
            entity: Entity type to search.
            field: Text field to search in.
            phrase: Exact phrase to match.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities with the exact phrase.
        """
        filter_expr = {
            "text_phrase": {
                "field": field,
                "phrase": phrase,
            }
        }
        return self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    def text_boolean_search(
        self,
        entity: str,
        field: str,
        *,
        must: list[str] | None = None,
        should: list[str] | None = None,
        must_not: list[str] | None = None,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Perform boolean text search with must/should/must_not terms.

        Args:
            entity: Entity type to search.
            field: Text field to search in.
            must: Terms that must appear.
            should: Terms that should appear (increases relevance).
            must_not: Terms that must not appear.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing matching entities.

        Example:
            >>> result = client.text_boolean_search(
            ...     "Article",
            ...     "content",
            ...     must=["rust"],
            ...     should=["performance", "safety"],
            ...     must_not=["deprecated"],
            ... )
        """
        filter_expr = {
            "text_boolean": {
                "field": field,
                "must": must or [],
                "should": should or [],
                "must_not": must_not or [],
            }
        }
        return self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    def search(
        self,
        entity: str,
        filter: SearchFilter,  # noqa: A002
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Execute a search using a SearchFilter object.

        Args:
            entity: Entity type to search.
            filter: A SearchFilter instance (VectorSearchFilter, GeoRadiusFilter, etc.).
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing matching entities.

        Example:
            >>> from ormdb.types import VectorSearchFilter
            >>> filter = VectorSearchFilter("embedding", [0.1, 0.2, ...], k=10)
            >>> result = client.search("Product", filter)
        """
        return self.query(
            entity, fields=fields, filter=filter.to_dict(), includes=includes, limit=limit
        )

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

    async def insert(self, entity: str, data: dict[str, Any]) -> MutationResult:
        """Insert a new entity asynchronously."""
        payload = {
            "Insert": {
                "entity": entity,
                "data": [{"field": k, "value": self._convert_value(v)} for k, v in data.items()],
            }
        }
        return await self._mutate(payload)

    async def update(self, entity: str, id: str, data: dict[str, Any]) -> MutationResult:  # noqa: A002
        """Update an existing entity asynchronously."""
        payload = {
            "Update": {
                "entity": entity,
                "id": self._hex_to_uuid(id),
                "data": [{"field": k, "value": self._convert_value(v)} for k, v in data.items()],
            }
        }
        return await self._mutate(payload)

    async def delete(self, entity: str, id: str) -> MutationResult:  # noqa: A002
        """Delete an entity asynchronously."""
        payload = {
            "Delete": {
                "entity": entity,
                "id": self._hex_to_uuid(id),
            }
        }
        return await self._mutate(payload)

    async def upsert(
        self,
        entity: str,
        data: dict[str, Any],
        id: str | None = None,  # noqa: A002
    ) -> MutationResult:
        """Insert or update an entity asynchronously."""
        payload = {
            "Upsert": {
                "entity": entity,
                "id": self._hex_to_uuid(id) if id else None,
                "data": [{"field": k, "value": self._convert_value(v)} for k, v in data.items()],
            }
        }
        return await self._mutate(payload)

    async def _mutate(self, payload: dict[str, Any]) -> MutationResult:
        """Execute a mutation asynchronously."""
        try:
            response = await self._client.post(f"{self.base_url}/mutate", json=payload)
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

    async def get_schema(self) -> dict[str, Any]:
        """Get database schema asynchronously."""
        try:
            response = await self._client.get(f"{self.base_url}/schema")
            response.raise_for_status()
            return response.json()
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to get schema: {e}")

    async def get_replication_status(self) -> ReplicationStatus:
        """Get replication status asynchronously."""
        try:
            response = await self._client.get(f"{self.base_url}/replication/status")
            response.raise_for_status()
            return ReplicationStatus.from_response(response.json())
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to get replication status: {e}")

    async def stream_changes(
        self,
        from_lsn: int = 0,
        limit: int = 1000,
        entities: list[str] | None = None,
    ) -> StreamChangesResult:
        """Stream changes from the changelog asynchronously."""
        params: dict[str, Any] = {
            "from_lsn": from_lsn,
            "limit": limit,
        }
        if entities:
            params["entities"] = ",".join(entities)

        try:
            response = await self._client.get(
                f"{self.base_url}/replication/changes",
                params=params,
            )
            response.raise_for_status()
            return StreamChangesResult.from_response(response.json())
        except httpx.HTTPError as e:
            raise ConnectionError(f"Failed to stream changes: {e}")

    # ========================================================================
    # Async Search Methods
    # ========================================================================

    async def vector_search(
        self,
        entity: str,
        field: str,
        query_vector: list[float],
        k: int,
        *,
        max_distance: float | None = None,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
    ) -> QueryResult:
        """Perform vector similarity search using HNSW index asynchronously.

        Args:
            entity: Entity type to search.
            field: Vector field to search in.
            query_vector: Query vector for similarity comparison.
            k: Number of nearest neighbors to return.
            max_distance: Maximum distance threshold (optional).
            fields: Fields to include in results.
            includes: Related entities to include.

        Returns:
            QueryResult containing k-nearest entities by vector similarity.
        """
        filter_expr: dict[str, Any] = {
            "vector_nearest_neighbor": {
                "field": field,
                "query_vector": query_vector,
                "k": k,
            }
        }
        if max_distance is not None:
            filter_expr["vector_nearest_neighbor"]["max_distance"] = max_distance

        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes)

    async def geo_search(
        self,
        entity: str,
        field: str,
        center_lat: float,
        center_lon: float,
        radius_km: float,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Search for entities within a geographic radius asynchronously.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            center_lat: Latitude of the center point.
            center_lon: Longitude of the center point.
            radius_km: Search radius in kilometers.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities within the radius.
        """
        filter_expr = {
            "geo_within_radius": {
                "field": field,
                "center_lat": center_lat,
                "center_lon": center_lon,
                "radius_km": radius_km,
            }
        }
        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    async def geo_box_search(
        self,
        entity: str,
        field: str,
        min_lat: float,
        min_lon: float,
        max_lat: float,
        max_lon: float,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Search for entities within a geographic bounding box asynchronously.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            min_lat: Minimum latitude of the bounding box.
            min_lon: Minimum longitude of the bounding box.
            max_lat: Maximum latitude of the bounding box.
            max_lon: Maximum longitude of the bounding box.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities within the bounding box.
        """
        filter_expr = {
            "geo_within_box": {
                "field": field,
                "min_lat": min_lat,
                "min_lon": min_lon,
                "max_lat": max_lat,
                "max_lon": max_lon,
            }
        }
        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    async def geo_polygon_search(
        self,
        entity: str,
        field: str,
        vertices: list[tuple[float, float]],
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Search for entities within a geographic polygon asynchronously.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            vertices: List of (lat, lon) tuples defining the polygon.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities within the polygon.
        """
        filter_expr = {
            "geo_within_polygon": {
                "field": field,
                "vertices": vertices,
            }
        }
        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    async def geo_nearest(
        self,
        entity: str,
        field: str,
        center_lat: float,
        center_lon: float,
        k: int,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
    ) -> QueryResult:
        """Find k-nearest entities by geographic distance asynchronously.

        Args:
            entity: Entity type to search.
            field: GeoPoint field to search in.
            center_lat: Latitude of the center point.
            center_lon: Longitude of the center point.
            k: Number of nearest neighbors to return.
            fields: Fields to include in results.
            includes: Related entities to include.

        Returns:
            QueryResult containing k-nearest entities by distance.
        """
        filter_expr = {
            "geo_nearest_neighbor": {
                "field": field,
                "center_lat": center_lat,
                "center_lon": center_lon,
                "k": k,
            }
        }
        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes)

    async def text_search(
        self,
        entity: str,
        field: str,
        query: str,
        *,
        min_score: float | None = None,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Perform full-text search with BM25 scoring asynchronously.

        Args:
            entity: Entity type to search.
            field: Text field to search in.
            query: Search query (will be tokenized and stemmed).
            min_score: Minimum relevance score threshold (0.0 to 1.0).
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing matching entities ranked by relevance.
        """
        filter_expr: dict[str, Any] = {
            "text_match": {
                "field": field,
                "query": query,
            }
        }
        if min_score is not None:
            filter_expr["text_match"]["min_score"] = min_score

        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    async def text_phrase_search(
        self,
        entity: str,
        field: str,
        phrase: str,
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Perform exact phrase search asynchronously.

        Args:
            entity: Entity type to search.
            field: Text field to search in.
            phrase: Exact phrase to match.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing entities with the exact phrase.
        """
        filter_expr = {
            "text_phrase": {
                "field": field,
                "phrase": phrase,
            }
        }
        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    async def text_boolean_search(
        self,
        entity: str,
        field: str,
        *,
        must: list[str] | None = None,
        should: list[str] | None = None,
        must_not: list[str] | None = None,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Perform boolean text search with must/should/must_not terms asynchronously.

        Args:
            entity: Entity type to search.
            field: Text field to search in.
            must: Terms that must appear.
            should: Terms that should appear (increases relevance).
            must_not: Terms that must not appear.
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing matching entities.
        """
        filter_expr = {
            "text_boolean": {
                "field": field,
                "must": must or [],
                "should": should or [],
                "must_not": must_not or [],
            }
        }
        return await self.query(entity, fields=fields, filter=filter_expr, includes=includes, limit=limit)

    async def search(
        self,
        entity: str,
        filter: SearchFilter,  # noqa: A002
        *,
        fields: list[str] | None = None,
        includes: list[dict[str, Any]] | None = None,
        limit: int | None = None,
    ) -> QueryResult:
        """Execute a search using a SearchFilter object asynchronously.

        Args:
            entity: Entity type to search.
            filter: A SearchFilter instance (VectorSearchFilter, GeoRadiusFilter, etc.).
            fields: Fields to include in results.
            includes: Related entities to include.
            limit: Maximum number of results.

        Returns:
            QueryResult containing matching entities.
        """
        return await self.query(
            entity, fields=fields, filter=filter.to_dict(), includes=includes, limit=limit
        )

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
            return {"String": str(value)}

    def _hex_to_uuid(self, hex_str: str) -> list[int]:
        """Convert hex string to UUID byte array."""
        if len(hex_str) != 32:
            raise ValueError(f"Invalid UUID hex string length: {len(hex_str)}")
        return [int(hex_str[i : i + 2], 16) for i in range(0, 32, 2)]
