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
