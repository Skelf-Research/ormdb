"""Django database wrapper for ORMDB.

This module provides the main DatabaseWrapper class that Django uses
to interact with ORMDB.

Usage in Django settings:
    DATABASES = {
        'default': {
            'ENGINE': 'ormdb.django',
            'HOST': 'localhost',
            'PORT': 8080,
        }
    }
"""

from typing import Any

from django.db.backends.base.base import BaseDatabaseWrapper
from django.utils.asyncio import async_unsafe

from .client import DatabaseClient
from .creation import DatabaseCreation
from .features import DatabaseFeatures
from .introspection import DatabaseIntrospection
from .operations import DatabaseOperations
from .schema import DatabaseSchemaEditor


class DatabaseWrapper(BaseDatabaseWrapper):
    """Django database backend for ORMDB."""

    vendor = "ormdb"
    display_name = "ORMDB"

    # Database operators
    operators = {
        "exact": "= %s",
        "iexact": "= UPPER(%s)",
        "contains": "LIKE %s",
        "icontains": "LIKE UPPER(%s)",
        "gt": "> %s",
        "gte": ">= %s",
        "lt": "< %s",
        "lte": "<= %s",
        "startswith": "LIKE %s",
        "endswith": "LIKE %s",
        "istartswith": "LIKE UPPER(%s)",
        "iendswith": "LIKE UPPER(%s)",
        "regex": "REGEXP %s",
        "iregex": "REGEXP %s",
    }

    # Data type mapping
    data_types = {
        "AutoField": "uuid",
        "BigAutoField": "uuid",
        "BinaryField": "bytes",
        "BooleanField": "bool",
        "CharField": "string",
        "DateField": "date",
        "DateTimeField": "timestamp",
        "DecimalField": "float64",
        "DurationField": "int64",
        "FileField": "string",
        "FilePathField": "string",
        "FloatField": "float64",
        "IntegerField": "int32",
        "BigIntegerField": "int64",
        "IPAddressField": "string",
        "GenericIPAddressField": "string",
        "JSONField": "json",
        "NullBooleanField": "bool",
        "OneToOneField": "uuid",
        "PositiveIntegerField": "int32",
        "PositiveSmallIntegerField": "int32",
        "PositiveBigIntegerField": "int64",
        "SlugField": "string",
        "SmallAutoField": "uuid",
        "SmallIntegerField": "int32",
        "TextField": "text",
        "TimeField": "time",
        "UUIDField": "uuid",
    }

    # Data type check constraints (empty - ORMDB handles validation)
    data_type_check_constraints = {}

    def __init__(self, settings_dict: dict[str, Any], alias: str = "default"):
        super().__init__(settings_dict, alias)

        self.features = DatabaseFeatures(self)
        self.ops = DatabaseOperations(self)
        self.client = DatabaseClient(self)
        self.creation = DatabaseCreation(self)
        self.introspection = DatabaseIntrospection(self)

    def get_connection_params(self) -> dict[str, Any]:
        """Return connection parameters."""
        settings = self.settings_dict
        return {
            "host": settings.get("HOST", "localhost"),
            "port": int(settings.get("PORT", 8080)),
            "timeout": float(settings.get("TIMEOUT", 30.0)),
        }

    @async_unsafe
    def get_new_connection(self, conn_params: dict[str, Any]) -> Any:
        """Create a new database connection."""
        from ..sqlalchemy import dbapi

        return dbapi.connect(**conn_params)

    def init_connection_state(self) -> None:
        """Initialize the connection state."""
        pass

    @async_unsafe
    def create_cursor(self, name: str | None = None) -> Any:
        """Create a new cursor."""
        return self.connection.cursor()

    def _set_autocommit(self, autocommit: bool) -> None:
        """Set autocommit mode (no-op - ORMDB auto-commits)."""
        pass

    def is_usable(self) -> bool:
        """Check if the connection is usable."""
        try:
            self.connection.cursor().execute("SELECT 1 FROM _health")
            return True
        except Exception:
            return False

    def schema_editor(
        self, *args: Any, **kwargs: Any
    ) -> "DatabaseSchemaEditor":
        """Return a new schema editor instance."""
        return DatabaseSchemaEditor(self, *args, **kwargs)

    @property
    def _nodb_cursor(self) -> Any:
        """Return a cursor not connected to a specific database.

        ORMDB doesn't have multiple databases, so return a normal cursor.
        """
        return self.cursor()
