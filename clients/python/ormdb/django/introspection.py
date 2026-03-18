"""Database introspection for ORMDB Django backend."""

from typing import Any

import httpx
from django.db.backends.base.introspection import (
    BaseDatabaseIntrospection,
    FieldInfo,
    TableInfo,
)


class DatabaseIntrospection(BaseDatabaseIntrospection):
    """Database introspection for ORMDB."""

    # Data type mapping from ORMDB to Django
    data_types_reverse = {
        "uuid": "UUIDField",
        "string": "CharField",
        "text": "TextField",
        "int32": "IntegerField",
        "int64": "BigIntegerField",
        "float32": "FloatField",
        "float64": "FloatField",
        "bool": "BooleanField",
        "bytes": "BinaryField",
        "timestamp": "DateTimeField",
        "date": "DateField",
        "time": "TimeField",
        "json": "JSONField",
    }

    def get_table_list(self, cursor: Any) -> list[TableInfo]:
        """Return a list of table/entity names."""
        try:
            raw_conn = cursor.connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            tables = []
            for entity in schema_data.get("entities", []):
                name = entity.get("name", "")
                if name:
                    tables.append(TableInfo(name, "t"))  # 't' for table

            return tables
        except Exception:
            return []

    def get_table_description(
        self, cursor: Any, table_name: str
    ) -> list[FieldInfo]:
        """Return field descriptions for a table."""
        try:
            raw_conn = cursor.connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    fields = []
                    for field in entity.get("fields", []):
                        field_type = field.get("field_type", "string")
                        fields.append(
                            FieldInfo(
                                name=field.get("name", ""),
                                type_code=field_type,
                                display_size=None,
                                internal_size=self._get_field_size(field_type),
                                precision=None,
                                scale=None,
                                null_ok=not field.get("required", False),
                                default=field.get("default"),
                                collation=None,
                            )
                        )
                    return fields

            return []
        except Exception:
            return []

    def _get_field_size(self, field_type: str) -> int | None:
        """Return the internal size for a field type."""
        sizes = {
            "uuid": 16,
            "int32": 4,
            "int64": 8,
            "float32": 4,
            "float64": 8,
            "bool": 1,
        }
        return sizes.get(field_type)

    def get_relations(
        self, cursor: Any, table_name: str
    ) -> dict[int, tuple[str, str]]:
        """Return relations for a table.

        Returns dict mapping column index to (referenced table, referenced column).
        """
        try:
            raw_conn = cursor.connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    relations = {}
                    fields = entity.get("fields", [])
                    for rel in entity.get("relations", []):
                        if rel.get("type") in ("many_to_one", "one_to_one"):
                            # Find the FK field index
                            fk_field = f"{rel.get('name', '')}_id"
                            for idx, field in enumerate(fields):
                                if field.get("name") == fk_field:
                                    relations[idx] = (rel.get("target", ""), "id")
                                    break
                    return relations

            return {}
        except Exception:
            return {}

    def get_primary_key_column(
        self, cursor: Any, table_name: str
    ) -> str | None:
        """Return the primary key column name."""
        # ORMDB always uses 'id' as primary key
        return "id"

    def get_primary_key_columns(
        self, cursor: Any, table_name: str
    ) -> list[str]:
        """Return primary key column names."""
        return ["id"]

    def get_key_columns(
        self, cursor: Any, table_name: str
    ) -> list[tuple[str, str, str]]:
        """Return foreign key columns.

        Returns list of (column_name, referenced_table, referenced_column).
        """
        try:
            raw_conn = cursor.connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    key_columns = []
                    for rel in entity.get("relations", []):
                        if rel.get("type") in ("many_to_one", "one_to_one"):
                            fk_field = f"{rel.get('name', '')}_id"
                            key_columns.append(
                                (fk_field, rel.get("target", ""), "id")
                            )
                    return key_columns

            return []
        except Exception:
            return []

    def get_constraints(
        self, cursor: Any, table_name: str
    ) -> dict[str, dict[str, Any]]:
        """Return constraints for a table."""
        constraints: dict[str, dict[str, Any]] = {}

        try:
            raw_conn = cursor.connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    # Primary key constraint
                    constraints[f"{table_name}_pkey"] = {
                        "columns": ["id"],
                        "primary_key": True,
                        "unique": True,
                        "foreign_key": None,
                        "check": False,
                        "index": True,
                    }

                    # Unique constraints
                    for field in entity.get("fields", []):
                        if field.get("unique"):
                            name = f"{table_name}_{field.get('name', '')}_key"
                            constraints[name] = {
                                "columns": [field.get("name", "")],
                                "primary_key": False,
                                "unique": True,
                                "foreign_key": None,
                                "check": False,
                                "index": True,
                            }

                    # Foreign key constraints
                    for rel in entity.get("relations", []):
                        if rel.get("type") in ("many_to_one", "one_to_one"):
                            fk_field = f"{rel.get('name', '')}_id"
                            name = f"{table_name}_{fk_field}_fkey"
                            constraints[name] = {
                                "columns": [fk_field],
                                "primary_key": False,
                                "unique": False,
                                "foreign_key": (rel.get("target", ""), "id"),
                                "check": False,
                                "index": True,
                            }

                    # Index constraints
                    for field in entity.get("fields", []):
                        if field.get("indexed") and not field.get("unique"):
                            name = f"{table_name}_{field.get('name', '')}_idx"
                            constraints[name] = {
                                "columns": [field.get("name", "")],
                                "primary_key": False,
                                "unique": False,
                                "foreign_key": None,
                                "check": False,
                                "index": True,
                            }

                    return constraints

            return constraints
        except Exception:
            return constraints

    def get_sequences(
        self, cursor: Any, table_name: str, table_fields: tuple[Any, ...]
    ) -> list[dict[str, str]]:
        """Return sequences (not supported in ORMDB)."""
        return []

    def identifier_converter(self, name: str) -> str:
        """Convert identifier for comparison."""
        return name
