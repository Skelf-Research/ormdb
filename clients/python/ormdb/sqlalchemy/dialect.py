"""SQLAlchemy dialect for ORMDB."""

from typing import Any

from sqlalchemy import types as sqltypes
from sqlalchemy.engine import default
from sqlalchemy.engine.interfaces import ReflectedColumn
from sqlalchemy.engine.url import URL


class OrmdbDialect(default.DefaultDialect):
    """SQLAlchemy dialect for ORMDB.

    Allows using SQLAlchemy's ORM and expression language with ORMDB.

    Usage:
        from sqlalchemy import create_engine

        engine = create_engine("ormdb://localhost:8080")

        # Use with Core
        with engine.connect() as conn:
            result = conn.execute(text("SELECT * FROM User"))

        # Use with ORM
        Session = sessionmaker(bind=engine)
        session = Session()
        users = session.query(User).all()
    """

    name = "ormdb"
    driver = "ormdb"

    # Feature flags
    supports_native_boolean = True
    supports_unicode_statements = True
    supports_unicode_binds = True
    supports_alter = False
    supports_native_enum = False
    supports_sequences = False
    supports_sane_rowcount = True
    supports_sane_multi_rowcount = False
    preexecute_autoincrement_sequences = False
    postfetch_lastrowid = True

    # Default parameter style
    default_paramstyle = "pyformat"

    # Statement compiler
    statement_compiler = default.DefaultDialect.statement_compiler
    ddl_compiler = default.DefaultDialect.ddl_compiler
    type_compiler = default.DefaultDialect.type_compiler
    preparer = default.DefaultDialect.preparer

    @classmethod
    def dbapi(cls) -> Any:
        """Return the DBAPI module."""
        from . import dbapi

        return dbapi

    @classmethod
    def import_dbapi(cls) -> Any:
        """Import and return the DBAPI module."""
        from . import dbapi

        return dbapi

    def create_connect_args(self, url: URL) -> tuple[list[Any], dict[str, Any]]:
        """Create connection arguments from URL.

        Args:
            url: SQLAlchemy URL object.

        Returns:
            Tuple of (args, kwargs) for dbapi.connect().
        """
        kwargs = {
            "host": url.host or "localhost",
            "port": url.port or 8080,
        }

        # Optional timeout from query string
        if url.query:
            if "timeout" in url.query:
                kwargs["timeout"] = float(url.query["timeout"])

        return [], kwargs

    def do_ping(self, dbapi_connection: Any) -> bool:
        """Check if the connection is still alive."""
        try:
            cursor = dbapi_connection.cursor()
            cursor.execute("SELECT 1 FROM _health")
            cursor.close()
            return True
        except Exception:
            return False

    def get_schema_names(
        self, connection: Any, **kw: Any
    ) -> list[str]:
        """Return list of schema names.

        ORMDB doesn't have traditional schemas, return default.
        """
        return ["default"]

    def get_table_names(
        self, connection: Any, schema: str | None = None, **kw: Any
    ) -> list[str]:
        """Return list of table (entity) names."""
        try:
            # Use the schema endpoint
            import httpx

            raw_conn = connection.connection.dbapi_connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            entities = schema_data.get("entities", [])
            return [e.get("name", "") for e in entities if e.get("name")]
        except Exception:
            return []

    def has_table(
        self,
        connection: Any,
        table_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> bool:
        """Check if a table exists."""
        return table_name in self.get_table_names(connection, schema)

    def get_columns(
        self,
        connection: Any,
        table_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> list[ReflectedColumn]:
        """Return column information for a table."""
        try:
            import httpx

            raw_conn = connection.connection.dbapi_connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            # Find the entity
            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    columns = []
                    for field in entity.get("fields", []):
                        col_type = self._map_ormdb_type(field.get("field_type", "string"))
                        columns.append(
                            {
                                "name": field.get("name", ""),
                                "type": col_type,
                                "nullable": not field.get("required", False),
                                "default": field.get("default"),
                                "autoincrement": False,
                            }
                        )
                    return columns

            return []
        except Exception:
            return []

    def get_pk_constraint(
        self,
        connection: Any,
        table_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> dict[str, Any]:
        """Return the primary key constraint for a table."""
        try:
            import httpx

            raw_conn = connection.connection.dbapi_connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    # ORMDB always has 'id' as primary key
                    return {
                        "constrained_columns": ["id"],
                        "name": f"{table_name}_pkey",
                    }

            return {"constrained_columns": [], "name": None}
        except Exception:
            return {"constrained_columns": [], "name": None}

    def get_foreign_keys(
        self,
        connection: Any,
        table_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> list[dict[str, Any]]:
        """Return foreign key information for a table."""
        try:
            import httpx

            raw_conn = connection.connection.dbapi_connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    fks = []
                    for rel in entity.get("relations", []):
                        if rel.get("type") in ("many_to_one", "one_to_one"):
                            fks.append(
                                {
                                    "name": f"fk_{table_name}_{rel.get('name', '')}",
                                    "constrained_columns": [
                                        f"{rel.get('name', '')}_id"
                                    ],
                                    "referred_schema": None,
                                    "referred_table": rel.get("target", ""),
                                    "referred_columns": ["id"],
                                }
                            )
                    return fks

            return []
        except Exception:
            return []

    def get_indexes(
        self,
        connection: Any,
        table_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> list[dict[str, Any]]:
        """Return index information for a table."""
        try:
            import httpx

            raw_conn = connection.connection.dbapi_connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    indexes = []
                    for field in entity.get("fields", []):
                        if field.get("indexed"):
                            indexes.append(
                                {
                                    "name": f"idx_{table_name}_{field.get('name', '')}",
                                    "column_names": [field.get("name", "")],
                                    "unique": field.get("unique", False),
                                }
                            )
                    return indexes

            return []
        except Exception:
            return []

    def get_unique_constraints(
        self,
        connection: Any,
        table_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> list[dict[str, Any]]:
        """Return unique constraint information for a table."""
        try:
            import httpx

            raw_conn = connection.connection.dbapi_connection
            response = httpx.get(f"{raw_conn.base_url}/schema")
            response.raise_for_status()
            schema_data = response.json()

            for entity in schema_data.get("entities", []):
                if entity.get("name") == table_name:
                    constraints = []
                    for field in entity.get("fields", []):
                        if field.get("unique"):
                            constraints.append(
                                {
                                    "name": f"uq_{table_name}_{field.get('name', '')}",
                                    "column_names": [field.get("name", "")],
                                }
                            )
                    return constraints

            return []
        except Exception:
            return []

    def get_view_names(
        self, connection: Any, schema: str | None = None, **kw: Any
    ) -> list[str]:
        """Return list of view names (ORMDB doesn't support views)."""
        return []

    def _map_ormdb_type(self, ormdb_type: str) -> sqltypes.TypeEngine:
        """Map ORMDB type to SQLAlchemy type."""
        type_map = {
            "uuid": sqltypes.String(32),
            "string": sqltypes.String,
            "text": sqltypes.Text,
            "int32": sqltypes.Integer,
            "int64": sqltypes.BigInteger,
            "float32": sqltypes.Float,
            "float64": sqltypes.Float,
            "bool": sqltypes.Boolean,
            "bytes": sqltypes.LargeBinary,
            "timestamp": sqltypes.DateTime,
            "date": sqltypes.Date,
            "time": sqltypes.Time,
            "json": sqltypes.JSON,
        }
        return type_map.get(ormdb_type.lower(), sqltypes.String)

    def has_sequence(
        self,
        connection: Any,
        sequence_name: str,
        schema: str | None = None,
        **kw: Any,
    ) -> bool:
        """Check if a sequence exists (ORMDB doesn't support sequences)."""
        return False

    def _get_default_schema_name(self, connection: Any) -> str:
        """Return the default schema name."""
        return "default"

    def get_isolation_level(self, dbapi_connection: Any) -> str:
        """Return the isolation level."""
        return "AUTOCOMMIT"

    def set_isolation_level(
        self, dbapi_connection: Any, level: str
    ) -> None:
        """Set the isolation level (no-op for ORMDB)."""
        pass


# Register dialect
def register() -> None:
    """Register the ORMDB dialect with SQLAlchemy."""
    from sqlalchemy.dialects import registry

    registry.register("ormdb", "ormdb.sqlalchemy", "OrmdbDialect")
