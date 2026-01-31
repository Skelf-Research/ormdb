"""Database operations for ORMDB Django backend."""

from typing import Any

from django.db.backends.base.operations import BaseDatabaseOperations


class DatabaseOperations(BaseDatabaseOperations):
    """Database operations for ORMDB."""

    compiler_module = "django.db.models.sql.compiler"

    # Caching
    cast_char_field_without_max_length = "string"
    cast_data_types = {
        "AutoField": "uuid",
        "BigAutoField": "uuid",
        "SmallAutoField": "uuid",
    }

    def quote_name(self, name: str) -> str:
        """Quote an identifier name."""
        if name.startswith('"') and name.endswith('"'):
            return name
        return f'"{name}"'

    def no_limit_value(self) -> int:
        """Return the value to use for 'no limit'."""
        return -1

    def limit_offset_sql(self, low_mark: int, high_mark: int | None) -> str:
        """Return LIMIT/OFFSET SQL clause."""
        sql = ""
        if high_mark is not None:
            sql = f" LIMIT {high_mark - low_mark}"
        if low_mark:
            if high_mark is None:
                sql = f" LIMIT -1"
            sql += f" OFFSET {low_mark}"
        return sql

    def last_executed_query(
        self, cursor: Any, sql: str, params: tuple[Any, ...] | None
    ) -> str:
        """Return the last executed query."""
        if params:
            return sql % params
        return sql

    def last_insert_id(
        self, cursor: Any, table_name: str, pk_name: str
    ) -> Any:
        """Return the ID of the last inserted row."""
        return cursor.lastrowid

    def pk_default_value(self) -> str:
        """Return the default value for primary keys."""
        return "DEFAULT"

    def date_extract_sql(self, lookup_type: str, sql: str, params: tuple) -> tuple[str, tuple]:
        """Extract a date component."""
        return f"EXTRACT({lookup_type.upper()} FROM {sql})", params

    def date_trunc_sql(
        self, lookup_type: str, sql: str, params: tuple, tzname: str | None = None
    ) -> tuple[str, tuple]:
        """Truncate date to a specific precision."""
        return f"DATE_TRUNC('{lookup_type}', {sql})", params

    def datetime_extract_sql(
        self, lookup_type: str, sql: str, params: tuple, tzname: str | None = None
    ) -> tuple[str, tuple]:
        """Extract a datetime component."""
        return f"EXTRACT({lookup_type.upper()} FROM {sql})", params

    def datetime_trunc_sql(
        self, lookup_type: str, sql: str, params: tuple, tzname: str | None = None
    ) -> tuple[str, tuple]:
        """Truncate datetime to a specific precision."""
        return f"DATE_TRUNC('{lookup_type}', {sql})", params

    def time_extract_sql(self, lookup_type: str, sql: str, params: tuple) -> tuple[str, tuple]:
        """Extract a time component."""
        return f"EXTRACT({lookup_type.upper()} FROM {sql})", params

    def time_trunc_sql(
        self, lookup_type: str, sql: str, params: tuple, tzname: str | None = None
    ) -> tuple[str, tuple]:
        """Truncate time to a specific precision."""
        return f"TIME_TRUNC('{lookup_type}', {sql})", params

    def sql_flush(
        self,
        style: Any,
        tables: list[str],
        *,
        reset_sequences: bool = False,
        allow_cascade: bool = False,
    ) -> list[str]:
        """Return a list of SQL statements to flush the database."""
        # ORMDB would need a specific flush mechanism
        # For now, return DELETE statements
        return [f"DELETE FROM {self.quote_name(table)}" for table in tables]

    def sequence_reset_sql(self, style: Any, model_list: list[Any]) -> list[str]:
        """Return SQL to reset sequences (not supported)."""
        return []

    def adapt_datetimefield_value(self, value: Any) -> Any:
        """Adapt datetime value for the database."""
        if value is None:
            return None
        return value.isoformat()

    def adapt_datefield_value(self, value: Any) -> Any:
        """Adapt date value for the database."""
        if value is None:
            return None
        return value.isoformat()

    def adapt_timefield_value(self, value: Any) -> Any:
        """Adapt time value for the database."""
        if value is None:
            return None
        return value.isoformat()

    def adapt_decimalfield_value(
        self, value: Any, max_digits: int | None = None, decimal_places: int | None = None
    ) -> Any:
        """Adapt decimal value for the database."""
        if value is None:
            return None
        return float(value)

    def adapt_ipaddressfield_value(self, value: Any) -> Any:
        """Adapt IP address value for the database."""
        return value

    def convert_uuidfield_value(
        self, value: Any, expression: Any, connection: Any
    ) -> Any:
        """Convert UUID field value from database."""
        return value

    def bulk_insert_sql(self, fields: list[Any], placeholder_rows: list[str]) -> str:
        """Return bulk insert SQL."""
        # ORMDB doesn't support traditional bulk insert
        # This would need to be handled differently
        return ""

    def prep_for_like_query(self, x: str) -> str:
        """Prepare a value for use in a LIKE query."""
        return x.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")

    def lookup_cast(self, lookup_type: str, internal_type: str | None = None) -> str:
        """Return the cast for a lookup."""
        return "%s"

    def max_name_length(self) -> int:
        """Return maximum identifier length."""
        return 128

    def distinct_sql(
        self, fields: list[str], params: list[Any]
    ) -> tuple[list[str], list[Any]]:
        """Return DISTINCT SQL."""
        return ["DISTINCT"], []

    def fetch_returned_insert_columns(
        self, cursor: Any, returning_params: tuple[Any, ...]
    ) -> tuple[Any, ...]:
        """Fetch columns returned from an INSERT."""
        return (cursor.lastrowid,)

    def return_insert_columns(
        self, fields: list[Any]
    ) -> tuple[str, tuple[Any, ...]]:
        """Return INSERT ... RETURNING clause."""
        # ORMDB returns inserted ID automatically
        return "", ()

    def random_function_sql(self) -> str:
        """Return SQL for random function."""
        return "RANDOM()"

    def regex_lookup(self, lookup_type: str) -> str:
        """Return regex lookup SQL."""
        if lookup_type == "iregex":
            return "REGEXP"
        return "REGEXP"
