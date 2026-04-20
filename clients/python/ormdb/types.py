"""Type definitions for ORMDB client."""

from dataclasses import dataclass, field
from typing import Any


@dataclass
class QueryResult:
    """Result of a query operation."""

    entities: list[dict[str, Any]]
    edges: list[dict[str, Any]] = field(default_factory=list)
    total_entities: int = 0
    total_edges: int = 0
    has_more: bool = False

    @classmethod
    def from_response(cls, response: dict[str, Any]) -> "QueryResult":
        """Create QueryResult from gateway response."""
        data = response.get("data", {})
        meta = response.get("meta", {})

        # Flatten entity blocks into a single list
        entities = []
        for block in data.get("entities", []):
            entities.extend(block.get("rows", []))

        edges = []
        for block in data.get("edges", []):
            edges.extend(block.get("edges", []))

        return cls(
            entities=entities,
            edges=edges,
            total_entities=meta.get("total_entities", len(entities)),
            total_edges=meta.get("total_edges", len(edges)),
            has_more=meta.get("has_more", False),
        )


@dataclass
class MutationResult:
    """Result of a mutation operation."""

    success: bool
    affected: int
    inserted_ids: list[str] = field(default_factory=list)

    @classmethod
    def from_response(cls, response: dict[str, Any]) -> "MutationResult":
        """Create MutationResult from gateway response."""
        return cls(
            success=response.get("success", False),
            affected=response.get("affected", 0),
            inserted_ids=response.get("inserted_ids", []),
        )


@dataclass
class ReplicationStatus:
    """Replication status information."""

    role: str
    primary_addr: str | None
    current_lsn: int
    lag_entries: int
    lag_ms: int

    @classmethod
    def from_response(cls, response: dict[str, Any]) -> "ReplicationStatus":
        """Create ReplicationStatus from gateway response."""
        data = response.get("data", {})
        return cls(
            role=data.get("role", "unknown"),
            primary_addr=data.get("primary_addr"),
            current_lsn=data.get("current_lsn", 0),
            lag_entries=data.get("lag_entries", 0),
            lag_ms=data.get("lag_ms", 0),
        )


@dataclass
class ChangeLogEntry:
    """A single change log entry from CDC."""

    lsn: int
    timestamp: int
    entity_type: str
    entity_id: str
    change_type: str
    changed_fields: list[str]
    schema_version: int

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ChangeLogEntry":
        """Create ChangeLogEntry from dictionary."""
        return cls(
            lsn=data.get("lsn", 0),
            timestamp=data.get("timestamp", 0),
            entity_type=data.get("entity_type", ""),
            entity_id=data.get("entity_id", ""),
            change_type=data.get("change_type", ""),
            changed_fields=data.get("changed_fields", []),
            schema_version=data.get("schema_version", 0),
        )


@dataclass
class StreamChangesResult:
    """Result of streaming changes."""

    entries: list[ChangeLogEntry]
    next_lsn: int
    has_more: bool

    @classmethod
    def from_response(cls, response: dict[str, Any]) -> "StreamChangesResult":
        """Create StreamChangesResult from gateway response."""
        entries = [
            ChangeLogEntry.from_dict(e) for e in response.get("entries", [])
        ]
        return cls(
            entries=entries,
            next_lsn=response.get("next_lsn", 0),
            has_more=response.get("has_more", False),
        )


# ============================================================================
# Search Filter Types
# ============================================================================


@dataclass
class VectorSearchFilter:
    """Vector similarity search filter using HNSW index."""

    field: str
    query_vector: list[float]
    k: int
    max_distance: float | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        result: dict[str, Any] = {
            "vector_nearest_neighbor": {
                "field": self.field,
                "query_vector": self.query_vector,
                "k": self.k,
            }
        }
        if self.max_distance is not None:
            result["vector_nearest_neighbor"]["max_distance"] = self.max_distance
        return result


@dataclass
class GeoRadiusFilter:
    """Geographic radius search filter."""

    field: str
    center_lat: float
    center_lon: float
    radius_km: float

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        return {
            "geo_within_radius": {
                "field": self.field,
                "center_lat": self.center_lat,
                "center_lon": self.center_lon,
                "radius_km": self.radius_km,
            }
        }


@dataclass
class GeoBoxFilter:
    """Geographic bounding box search filter."""

    field: str
    min_lat: float
    min_lon: float
    max_lat: float
    max_lon: float

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        return {
            "geo_within_box": {
                "field": self.field,
                "min_lat": self.min_lat,
                "min_lon": self.min_lon,
                "max_lat": self.max_lat,
                "max_lon": self.max_lon,
            }
        }


@dataclass
class GeoPolygonFilter:
    """Geographic polygon containment search filter."""

    field: str
    vertices: list[tuple[float, float]]

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        return {
            "geo_within_polygon": {
                "field": self.field,
                "vertices": self.vertices,
            }
        }


@dataclass
class GeoNearestFilter:
    """Geographic k-nearest neighbor search filter."""

    field: str
    center_lat: float
    center_lon: float
    k: int

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        return {
            "geo_nearest_neighbor": {
                "field": self.field,
                "center_lat": self.center_lat,
                "center_lon": self.center_lon,
                "k": self.k,
            }
        }


@dataclass
class TextMatchFilter:
    """Full-text BM25 search filter."""

    field: str
    query: str
    min_score: float | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        result: dict[str, Any] = {
            "text_match": {
                "field": self.field,
                "query": self.query,
            }
        }
        if self.min_score is not None:
            result["text_match"]["min_score"] = self.min_score
        return result


@dataclass
class TextPhraseFilter:
    """Full-text phrase search filter."""

    field: str
    phrase: str

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        return {
            "text_phrase": {
                "field": self.field,
                "phrase": self.phrase,
            }
        }


@dataclass
class TextBooleanFilter:
    """Full-text boolean search filter with must/should/must_not terms."""

    field: str
    must: list[str] = field(default_factory=list)
    should: list[str] = field(default_factory=list)
    must_not: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to protocol format."""
        return {
            "text_boolean": {
                "field": self.field,
                "must": self.must,
                "should": self.should,
                "must_not": self.must_not,
            }
        }


# Union type for all search filters
SearchFilter = (
    VectorSearchFilter
    | GeoRadiusFilter
    | GeoBoxFilter
    | GeoPolygonFilter
    | GeoNearestFilter
    | TextMatchFilter
    | TextPhraseFilter
    | TextBooleanFilter
)
