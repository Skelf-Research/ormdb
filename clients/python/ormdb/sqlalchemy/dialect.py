"""SQLAlchemy dialect for ORMDB."""

from typing import Any

from sqlalchemy import types as sqltypes
from sqlalchemy.engine import default
from sqlalchemy.engine.interfaces import ReflectedColumn
from sqlalchemy.engine.url import URL
from sqlalchemy.sql.expression import ColumnElement, FunctionElement
from sqlalchemy.ext.compiler import compiles


# ============================================================================
# Search Expression Types
# ============================================================================


class VectorSearch(FunctionElement):
    """Vector similarity search expression.

    Usage:
        from ormdb.sqlalchemy import vector_search

        # Find similar products by embedding
        stmt = select(Product).where(
            vector_search(Product.embedding, [0.1, 0.2, 0.3], k=10)
        )
    """

    inherit_cache = True
    name = "vector_search"

    def __init__(
        self,
        column: ColumnElement[Any],
        query_vector: list[float],
        k: int,
        max_distance: float | None = None,
    ):
        self.column = column
        self.query_vector = query_vector
        self.k = k
        self.max_distance = max_distance
        super().__init__(column)


@compiles(VectorSearch, "ormdb")
def compile_vector_search(element: VectorSearch, compiler: Any, **kw: Any) -> str:
    """Compile VectorSearch to ORMDB filter format."""
    # Return a placeholder that gets converted to filter in dbapi
    return f"__VECTOR_SEARCH__({element.column.name},{element.k},{element.max_distance})"


class GeoWithinRadius(FunctionElement):
    """Geographic radius search expression.

    Usage:
        from ormdb.sqlalchemy import geo_within_radius

        # Find restaurants within 5km
        stmt = select(Restaurant).where(
            geo_within_radius(Restaurant.location, 37.7749, -122.4194, 5.0)
        )
    """

    inherit_cache = True
    name = "geo_within_radius"

    def __init__(
        self,
        column: ColumnElement[Any],
        center_lat: float,
        center_lon: float,
        radius_km: float,
    ):
        self.column = column
        self.center_lat = center_lat
        self.center_lon = center_lon
        self.radius_km = radius_km
        super().__init__(column)


@compiles(GeoWithinRadius, "ormdb")
def compile_geo_within_radius(element: GeoWithinRadius, compiler: Any, **kw: Any) -> str:
    """Compile GeoWithinRadius to ORMDB filter format."""
    return f"__GEO_WITHIN_RADIUS__({element.column.name},{element.center_lat},{element.center_lon},{element.radius_km})"


class GeoWithinBox(FunctionElement):
    """Geographic bounding box search expression."""

    inherit_cache = True
    name = "geo_within_box"

    def __init__(
        self,
        column: ColumnElement[Any],
        min_lat: float,
        min_lon: float,
        max_lat: float,
        max_lon: float,
    ):
        self.column = column
        self.min_lat = min_lat
        self.min_lon = min_lon
        self.max_lat = max_lat
        self.max_lon = max_lon
        super().__init__(column)


@compiles(GeoWithinBox, "ormdb")
def compile_geo_within_box(element: GeoWithinBox, compiler: Any, **kw: Any) -> str:
    """Compile GeoWithinBox to ORMDB filter format."""
    return f"__GEO_WITHIN_BOX__({element.column.name},{element.min_lat},{element.min_lon},{element.max_lat},{element.max_lon})"


class GeoWithinPolygon(FunctionElement):
    """Geographic polygon containment search expression."""

    inherit_cache = True
    name = "geo_within_polygon"

    def __init__(
        self,
        column: ColumnElement[Any],
        vertices: list[tuple[float, float]],
    ):
        self.column = column
        self.vertices = vertices
        super().__init__(column)


@compiles(GeoWithinPolygon, "ormdb")
def compile_geo_within_polygon(element: GeoWithinPolygon, compiler: Any, **kw: Any) -> str:
    """Compile GeoWithinPolygon to ORMDB filter format."""
    return f"__GEO_WITHIN_POLYGON__({element.column.name})"


class GeoNearest(FunctionElement):
    """Geographic k-nearest neighbor search expression."""

    inherit_cache = True
    name = "geo_nearest"

    def __init__(
        self,
        column: ColumnElement[Any],
        center_lat: float,
        center_lon: float,
        k: int,
    ):
        self.column = column
        self.center_lat = center_lat
        self.center_lon = center_lon
        self.k = k
        super().__init__(column)


@compiles(GeoNearest, "ormdb")
def compile_geo_nearest(element: GeoNearest, compiler: Any, **kw: Any) -> str:
    """Compile GeoNearest to ORMDB filter format."""
    return f"__GEO_NEAREST__({element.column.name},{element.center_lat},{element.center_lon},{element.k})"


class TextMatch(FunctionElement):
    """Full-text search expression with BM25 scoring.

    Usage:
        from ormdb.sqlalchemy import text_match

        # Search articles
        stmt = select(Article).where(
            text_match(Article.content, "rust programming", min_score=0.5)
        )
    """

    inherit_cache = True
    name = "text_match"

    def __init__(
        self,
        column: ColumnElement[Any],
        query: str,
        min_score: float | None = None,
    ):
        self.column = column
        self.query = query
        self.min_score = min_score
        super().__init__(column)


@compiles(TextMatch, "ormdb")
def compile_text_match(element: TextMatch, compiler: Any, **kw: Any) -> str:
    """Compile TextMatch to ORMDB filter format."""
    return f"__TEXT_MATCH__({element.column.name},{element.min_score})"


class TextPhrase(FunctionElement):
    """Full-text phrase search expression."""

    inherit_cache = True
    name = "text_phrase"

    def __init__(
        self,
        column: ColumnElement[Any],
        phrase: str,
    ):
        self.column = column
        self.phrase = phrase
        super().__init__(column)


@compiles(TextPhrase, "ormdb")
def compile_text_phrase(element: TextPhrase, compiler: Any, **kw: Any) -> str:
    """Compile TextPhrase to ORMDB filter format."""
    return f"__TEXT_PHRASE__({element.column.name})"


class TextBoolean(FunctionElement):
    """Full-text boolean search expression with must/should/must_not terms."""

    inherit_cache = True
    name = "text_boolean"

    def __init__(
        self,
        column: ColumnElement[Any],
        must: list[str] | None = None,
        should: list[str] | None = None,
        must_not: list[str] | None = None,
    ):
        self.column = column
        self.must = must or []
        self.should = should or []
        self.must_not = must_not or []
        super().__init__(column)


@compiles(TextBoolean, "ormdb")
def compile_text_boolean(element: TextBoolean, compiler: Any, **kw: Any) -> str:
    """Compile TextBoolean to ORMDB filter format."""
    return f"__TEXT_BOOLEAN__({element.column.name})"


# Convenience functions
def vector_search(
    column: ColumnElement[Any],
    query_vector: list[float],
    k: int,
    max_distance: float | None = None,
) -> VectorSearch:
    """Create a vector similarity search expression."""
    return VectorSearch(column, query_vector, k, max_distance)


def geo_within_radius(
    column: ColumnElement[Any],
    center_lat: float,
    center_lon: float,
    radius_km: float,
) -> GeoWithinRadius:
    """Create a geographic radius search expression."""
    return GeoWithinRadius(column, center_lat, center_lon, radius_km)


def geo_within_box(
    column: ColumnElement[Any],
    min_lat: float,
    min_lon: float,
    max_lat: float,
    max_lon: float,
) -> GeoWithinBox:
    """Create a geographic bounding box search expression."""
    return GeoWithinBox(column, min_lat, min_lon, max_lat, max_lon)


def geo_within_polygon(
    column: ColumnElement[Any],
    vertices: list[tuple[float, float]],
) -> GeoWithinPolygon:
    """Create a geographic polygon search expression."""
    return GeoWithinPolygon(column, vertices)


def geo_nearest(
    column: ColumnElement[Any],
    center_lat: float,
    center_lon: float,
    k: int,
) -> GeoNearest:
    """Create a geographic k-nearest neighbor search expression."""
    return GeoNearest(column, center_lat, center_lon, k)


def text_match(
    column: ColumnElement[Any],
    query: str,
    min_score: float | None = None,
) -> TextMatch:
    """Create a full-text search expression."""
    return TextMatch(column, query, min_score)


def text_phrase(
    column: ColumnElement[Any],
    phrase: str,
) -> TextPhrase:
    """Create a full-text phrase search expression."""
    return TextPhrase(column, phrase)


def text_boolean(
    column: ColumnElement[Any],
    must: list[str] | None = None,
    should: list[str] | None = None,
    must_not: list[str] | None = None,
) -> TextBoolean:
    """Create a full-text boolean search expression."""
    return TextBoolean(column, must, should, must_not)


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
