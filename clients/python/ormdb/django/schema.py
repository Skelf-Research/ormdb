"""Database schema editor for ORMDB Django backend."""

from typing import Any

from django.db.backends.base.schema import BaseDatabaseSchemaEditor
from django.db.models import Field, Model


class DatabaseSchemaEditor(BaseDatabaseSchemaEditor):
    """Schema editor for ORMDB.

    Note: ORMDB manages schema through its own schema definition files,
    not through DDL statements. This editor provides basic compatibility
    but actual schema changes should be made through ORMDB's schema system.
    """

    sql_create_table = "-- CREATE TABLE not supported (use ORMDB schema)"
    sql_delete_table = "-- DROP TABLE not supported (use ORMDB schema)"
    sql_rename_table = "-- RENAME TABLE not supported (use ORMDB schema)"

    sql_create_column = "-- ADD COLUMN not supported (use ORMDB schema)"
    sql_alter_column = "-- ALTER COLUMN not supported (use ORMDB schema)"
    sql_delete_column = "-- DROP COLUMN not supported (use ORMDB schema)"
    sql_rename_column = "-- RENAME COLUMN not supported (use ORMDB schema)"

    sql_create_pk = "-- PRIMARY KEY managed by ORMDB"
    sql_delete_pk = "-- PRIMARY KEY managed by ORMDB"

    sql_create_fk = "-- FOREIGN KEY not supported (use ORMDB relations)"
    sql_delete_fk = "-- FOREIGN KEY not supported (use ORMDB relations)"

    sql_create_index = "-- CREATE INDEX not supported (use ORMDB schema)"
    sql_delete_index = "-- DROP INDEX not supported (use ORMDB schema)"

    sql_create_unique = "-- UNIQUE constraint not supported (use ORMDB schema)"
    sql_delete_unique = "-- UNIQUE constraint not supported (use ORMDB schema)"

    def create_model(self, model: type[Model]) -> None:
        """Create a model (entity) in the database.

        Note: Schema changes should be made through ORMDB's schema system.
        """
        # Log that we're attempting to create a model
        if self.connection.features.supports_transactions:
            self.deferred_sql.append(
                f"-- Model {model._meta.db_table} should be defined in ORMDB schema"
            )

    def delete_model(self, model: type[Model]) -> None:
        """Delete a model (entity) from the database.

        Note: Schema changes should be made through ORMDB's schema system.
        """
        pass

    def alter_unique_together(
        self,
        model: type[Model],
        old_unique_together: set[tuple[str, ...]],
        new_unique_together: set[tuple[str, ...]],
    ) -> None:
        """Alter unique together constraints (not supported)."""
        pass

    def alter_index_together(
        self,
        model: type[Model],
        old_index_together: set[tuple[str, ...]],
        new_index_together: set[tuple[str, ...]],
    ) -> None:
        """Alter index together constraints (not supported)."""
        pass

    def add_field(self, model: type[Model], field: Field) -> None:
        """Add a field to a model (not directly supported).

        Note: Schema changes should be made through ORMDB's schema system.
        """
        pass

    def remove_field(self, model: type[Model], field: Field) -> None:
        """Remove a field from a model (not directly supported).

        Note: Schema changes should be made through ORMDB's schema system.
        """
        pass

    def alter_field(
        self,
        model: type[Model],
        old_field: Field,
        new_field: Field,
        strict: bool = False,
    ) -> None:
        """Alter a field (not directly supported).

        Note: Schema changes should be made through ORMDB's schema system.
        """
        pass

    def add_index(self, model: type[Model], index: Any) -> None:
        """Add an index (should be done through ORMDB schema)."""
        pass

    def remove_index(self, model: type[Model], index: Any) -> None:
        """Remove an index (should be done through ORMDB schema)."""
        pass

    def add_constraint(self, model: type[Model], constraint: Any) -> None:
        """Add a constraint (should be done through ORMDB schema)."""
        pass

    def remove_constraint(self, model: type[Model], constraint: Any) -> None:
        """Remove a constraint (should be done through ORMDB schema)."""
        pass

    def _alter_field(
        self,
        model: type[Model],
        old_field: Field,
        new_field: Field,
        old_type: str,
        new_type: str,
        old_db_params: dict[str, Any],
        new_db_params: dict[str, Any],
        strict: bool = False,
    ) -> None:
        """Internal field alteration (not supported)."""
        pass

    def quote_value(self, value: Any) -> str:
        """Quote a value for SQL."""
        if isinstance(value, str):
            return f"'{value}'"
        elif value is None:
            return "NULL"
        elif isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        else:
            return str(value)

    def prepare_default(self, field: Field) -> Any:
        """Prepare a default value for SQL."""
        return self.quote_value(field.get_default())
