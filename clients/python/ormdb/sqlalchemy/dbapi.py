"""DBAPI 2.0 compatible interface for ORMDB.

This module provides a PEP 249 (DBAPI 2.0) compatible interface
that SQLAlchemy can use to communicate with ORMDB.
"""

from typing import Any, Iterator

import httpx

# DBAPI 2.0 module-level globals
apilevel = "2.0"
threadsafety = 1  # Threads may share the module, but not connections
paramstyle = "pyformat"  # Python format codes, e.g. %(name)s


class Error(Exception):
    """Base exception for DBAPI errors."""

    pass


class Warning(Exception):
    """Exception for important warnings."""

    pass


class InterfaceError(Error):
    """Exception for interface errors."""

    pass


class DatabaseError(Error):
    """Exception for database errors."""

    pass


class DataError(DatabaseError):
    """Exception for data errors."""

    pass


class OperationalError(DatabaseError):
    """Exception for operational errors."""

    pass


class IntegrityError(DatabaseError):
    """Exception for integrity errors."""

    pass


class InternalError(DatabaseError):
    """Exception for internal errors."""

    pass


class ProgrammingError(DatabaseError):
    """Exception for programming errors."""

    pass


class NotSupportedError(DatabaseError):
    """Exception for unsupported operations."""

    pass


def connect(
    host: str = "localhost",
    port: int = 8080,
    timeout: float = 30.0,
    **kwargs: Any,
) -> "Connection":
    """Create a new database connection.

    Args:
        host: ORMDB gateway host.
        port: ORMDB gateway port.
        timeout: Request timeout in seconds.

    Returns:
        A new Connection object.
    """
    return Connection(host=host, port=port, timeout=timeout)


class Connection:
    """DBAPI 2.0 connection object."""

    def __init__(
        self,
        host: str = "localhost",
        port: int = 8080,
        timeout: float = 30.0,
    ):
        self.host = host
        self.port = port
        self.timeout = timeout
        self.base_url = f"http://{host}:{port}"
        self._client = httpx.Client(timeout=timeout)
        self._closed = False
        self._in_transaction = False

    def close(self) -> None:
        """Close the connection."""
        if not self._closed:
            self._client.close()
            self._closed = True

    def commit(self) -> None:
        """Commit any pending transaction.

        ORMDB auto-commits each mutation, so this is a no-op.
        """
        self._in_transaction = False

    def rollback(self) -> None:
        """Roll back any pending transaction.

        Note: ORMDB doesn't support transactions yet, so this is a no-op.
        """
        self._in_transaction = False

    def cursor(self) -> "Cursor":
        """Create a new cursor."""
        if self._closed:
            raise InterfaceError("Connection is closed")
        return Cursor(self)

    def __enter__(self) -> "Connection":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()


class Cursor:
    """DBAPI 2.0 cursor object."""

    def __init__(self, connection: Connection):
        self.connection = connection
        self.description: list[tuple[str, Any, None, None, None, None, None]] | None = (
            None
        )
        self.rowcount: int = -1
        self.arraysize: int = 1
        self._rows: list[tuple[Any, ...]] = []
        self._row_index: int = 0
        self._closed = False
        self._last_inserted_id: str | None = None

    @property
    def lastrowid(self) -> str | None:
        """Return the ID of the last inserted row."""
        return self._last_inserted_id

    def close(self) -> None:
        """Close the cursor."""
        self._closed = True
        self._rows = []

    def execute(self, operation: str, parameters: dict[str, Any] | None = None) -> None:
        """Execute a database operation (query or mutation).

        Args:
            operation: SQL-like statement or ORMDB operation.
            parameters: Parameters for the operation.
        """
        if self._closed:
            raise InterfaceError("Cursor is closed")

        # Reset state
        self._rows = []
        self._row_index = 0
        self.description = None
        self.rowcount = -1
        self._last_inserted_id = None

        # Parse and execute the operation
        operation = operation.strip()
        if parameters:
            operation = operation % parameters

        try:
            if operation.upper().startswith("SELECT"):
                self._execute_query(operation)
            elif operation.upper().startswith("INSERT"):
                self._execute_insert(operation)
            elif operation.upper().startswith("UPDATE"):
                self._execute_update(operation)
            elif operation.upper().startswith("DELETE"):
                self._execute_delete(operation)
            elif operation.upper().startswith("--"):
                # Comment, ignore
                pass
            else:
                raise ProgrammingError(f"Unsupported operation: {operation}")
        except httpx.HTTPError as e:
            raise OperationalError(f"HTTP error: {e}")

    def _execute_query(self, sql: str) -> None:
        """Execute a SELECT query."""
        # Parse simple SELECT queries
        # Format: SELECT fields FROM entity [WHERE conditions] [ORDER BY ...] [LIMIT n]
        entity, fields, filter_expr, order_by, limit, offset = self._parse_select(sql)

        payload: dict[str, Any] = {"root_entity": entity}

        if fields and fields != ["*"]:
            payload["fields"] = fields
        if filter_expr:
            payload["filter"] = {"expression": filter_expr}
        if order_by:
            payload["order_by"] = order_by
        if limit is not None or offset is not None:
            payload["pagination"] = {
                "limit": limit or 100,
                "offset": offset or 0,
            }

        response = self.connection._client.post(
            f"{self.connection.base_url}/query",
            json=payload,
        )
        response.raise_for_status()
        result = response.json()

        # Build description and rows from response
        data = result.get("data", {})
        entities = []
        for block in data.get("entities", []):
            entities.extend(block.get("rows", []))

        if entities:
            # Build description from first row
            first_row = entities[0]
            self.description = [
                (key, None, None, None, None, None, None) for key in first_row.keys()
            ]
            # Convert rows to tuples
            self._rows = [tuple(row.values()) for row in entities]
        else:
            self._rows = []

        self.rowcount = len(self._rows)

    def _execute_insert(self, sql: str) -> None:
        """Execute an INSERT statement."""
        # Parse: INSERT INTO entity (fields) VALUES (values)
        entity, data = self._parse_insert(sql)

        payload = {
            "Insert": {
                "entity": entity,
                "data": [
                    {"field": k, "value": self._convert_value(v)}
                    for k, v in data.items()
                ],
            }
        }

        response = self.connection._client.post(
            f"{self.connection.base_url}/mutate",
            json=payload,
        )
        response.raise_for_status()
        result = response.json()

        self.rowcount = result.get("affected", 0)
        inserted_ids = result.get("inserted_ids", [])
        if inserted_ids:
            self._last_inserted_id = inserted_ids[0]

    def _execute_update(self, sql: str) -> None:
        """Execute an UPDATE statement."""
        # Parse: UPDATE entity SET field=value WHERE id=...
        entity, entity_id, data = self._parse_update(sql)

        payload = {
            "Update": {
                "entity": entity,
                "id": self._hex_to_uuid(entity_id),
                "data": [
                    {"field": k, "value": self._convert_value(v)}
                    for k, v in data.items()
                ],
            }
        }

        response = self.connection._client.post(
            f"{self.connection.base_url}/mutate",
            json=payload,
        )
        response.raise_for_status()
        result = response.json()

        self.rowcount = result.get("affected", 0)

    def _execute_delete(self, sql: str) -> None:
        """Execute a DELETE statement."""
        # Parse: DELETE FROM entity WHERE id=...
        entity, entity_id = self._parse_delete(sql)

        payload = {
            "Delete": {
                "entity": entity,
                "id": self._hex_to_uuid(entity_id),
            }
        }

        response = self.connection._client.post(
            f"{self.connection.base_url}/mutate",
            json=payload,
        )
        response.raise_for_status()
        result = response.json()

        self.rowcount = result.get("affected", 0)

    def _parse_select(
        self, sql: str
    ) -> tuple[
        str,
        list[str],
        dict[str, Any] | None,
        list[dict[str, str]] | None,
        int | None,
        int | None,
    ]:
        """Parse a SELECT statement into ORMDB query components."""
        import re

        # Remove SELECT keyword
        sql = sql[6:].strip()

        # Extract fields (before FROM)
        from_idx = sql.upper().find(" FROM ")
        if from_idx == -1:
            raise ProgrammingError("Invalid SELECT: missing FROM clause")

        fields_str = sql[:from_idx].strip()
        fields = [f.strip() for f in fields_str.split(",")]

        # Extract entity (after FROM)
        remaining = sql[from_idx + 6 :].strip()

        # Find WHERE, ORDER BY, LIMIT
        where_idx = remaining.upper().find(" WHERE ")
        order_idx = remaining.upper().find(" ORDER BY ")
        limit_idx = remaining.upper().find(" LIMIT ")
        offset_idx = remaining.upper().find(" OFFSET ")

        # Determine entity end position
        end_positions = [
            i for i in [where_idx, order_idx, limit_idx] if i != -1
        ]
        entity_end = min(end_positions) if end_positions else len(remaining)
        entity = remaining[:entity_end].strip()

        # Parse WHERE clause
        filter_expr = None
        if where_idx != -1:
            where_end = min(
                [i for i in [order_idx, limit_idx] if i != -1]
                or [len(remaining)]
            )
            where_clause = remaining[where_idx + 7 : where_end].strip()
            filter_expr = self._parse_where(where_clause)

        # Parse ORDER BY clause
        order_by = None
        if order_idx != -1:
            order_end = limit_idx if limit_idx != -1 else len(remaining)
            order_clause = remaining[order_idx + 10 : order_end].strip()
            order_by = self._parse_order_by(order_clause)

        # Parse LIMIT and OFFSET
        limit = None
        offset = None
        if limit_idx != -1:
            limit_str = remaining[limit_idx + 7 :].strip()
            # Check for OFFSET in limit string
            offset_in_limit = limit_str.upper().find(" OFFSET ")
            if offset_in_limit != -1:
                limit = int(limit_str[:offset_in_limit].strip())
                offset = int(limit_str[offset_in_limit + 8 :].strip())
            else:
                limit = int(limit_str.split()[0])

        if offset_idx != -1 and offset is None:
            offset_str = remaining[offset_idx + 8 :].strip()
            offset = int(offset_str.split()[0])

        return entity, fields, filter_expr, order_by, limit, offset

    def _parse_where(self, where_clause: str) -> dict[str, Any]:
        """Parse a WHERE clause into a filter expression."""
        import re

        # Simple single condition: field op value
        # Supported: =, !=, <, >, <=, >=, LIKE, IN
        match = re.match(
            r"(\w+)\s*(=|!=|<>|<=|>=|<|>|LIKE|IN)\s*(.+)",
            where_clause,
            re.IGNORECASE,
        )
        if not match:
            raise ProgrammingError(f"Cannot parse WHERE clause: {where_clause}")

        field = match.group(1)
        op = match.group(2).upper()
        value_str = match.group(3).strip()

        # Convert operator
        op_map = {
            "=": "eq",
            "!=": "ne",
            "<>": "ne",
            "<": "lt",
            ">": "gt",
            "<=": "le",
            ">=": "ge",
            "LIKE": "like",
            "IN": "in",
        }
        ormdb_op = op_map.get(op, "eq")

        # Parse value
        value = self._parse_sql_value(value_str)

        return {
            "field": field,
            "op": ormdb_op,
            "value": value,
        }

    def _parse_order_by(self, order_clause: str) -> list[dict[str, str]]:
        """Parse an ORDER BY clause."""
        result = []
        for part in order_clause.split(","):
            part = part.strip()
            if " DESC" in part.upper():
                field = part.upper().replace(" DESC", "").strip()
                result.append({"field": field, "direction": "desc"})
            elif " ASC" in part.upper():
                field = part.upper().replace(" ASC", "").strip()
                result.append({"field": field, "direction": "asc"})
            else:
                result.append({"field": part, "direction": "asc"})
        return result

    def _parse_insert(self, sql: str) -> tuple[str, dict[str, Any]]:
        """Parse an INSERT statement."""
        import re

        # INSERT INTO entity (fields) VALUES (values)
        match = re.match(
            r"INSERT\s+INTO\s+(\w+)\s*\(([^)]+)\)\s*VALUES\s*\(([^)]+)\)",
            sql,
            re.IGNORECASE,
        )
        if not match:
            raise ProgrammingError(f"Cannot parse INSERT: {sql}")

        entity = match.group(1)
        fields = [f.strip() for f in match.group(2).split(",")]
        values_str = match.group(3)

        # Parse values (handling quoted strings)
        values = self._parse_value_list(values_str)

        if len(fields) != len(values):
            raise ProgrammingError("Field count doesn't match value count")

        return entity, dict(zip(fields, values))

    def _parse_update(self, sql: str) -> tuple[str, str, dict[str, Any]]:
        """Parse an UPDATE statement."""
        import re

        # UPDATE entity SET field=value, ... WHERE id=...
        match = re.match(
            r"UPDATE\s+(\w+)\s+SET\s+(.+)\s+WHERE\s+id\s*=\s*['\"]?([^'\"]+)['\"]?",
            sql,
            re.IGNORECASE,
        )
        if not match:
            raise ProgrammingError(f"Cannot parse UPDATE: {sql}")

        entity = match.group(1)
        set_clause = match.group(2)
        entity_id = match.group(3).strip()

        # Parse SET clause
        data = {}
        for assignment in set_clause.split(","):
            field, value_str = assignment.split("=", 1)
            data[field.strip()] = self._parse_sql_value(value_str.strip())

        return entity, entity_id, data

    def _parse_delete(self, sql: str) -> tuple[str, str]:
        """Parse a DELETE statement."""
        import re

        # DELETE FROM entity WHERE id=...
        match = re.match(
            r"DELETE\s+FROM\s+(\w+)\s+WHERE\s+id\s*=\s*['\"]?([^'\"]+)['\"]?",
            sql,
            re.IGNORECASE,
        )
        if not match:
            raise ProgrammingError(f"Cannot parse DELETE: {sql}")

        entity = match.group(1)
        entity_id = match.group(2).strip()

        return entity, entity_id

    def _parse_sql_value(self, value_str: str) -> Any:
        """Parse a SQL value string into a Python value."""
        value_str = value_str.strip()

        # Quoted string
        if (value_str.startswith("'") and value_str.endswith("'")) or (
            value_str.startswith('"') and value_str.endswith('"')
        ):
            return value_str[1:-1]

        # NULL
        if value_str.upper() == "NULL":
            return None

        # Boolean
        if value_str.upper() == "TRUE":
            return True
        if value_str.upper() == "FALSE":
            return False

        # Number
        try:
            if "." in value_str:
                return float(value_str)
            return int(value_str)
        except ValueError:
            pass

        # Default to string
        return value_str

    def _parse_value_list(self, values_str: str) -> list[Any]:
        """Parse a comma-separated list of SQL values."""
        values = []
        current = ""
        in_quotes = False
        quote_char = None

        for char in values_str:
            if char in ("'", '"') and not in_quotes:
                in_quotes = True
                quote_char = char
                current += char
            elif char == quote_char and in_quotes:
                in_quotes = False
                current += char
                quote_char = None
            elif char == "," and not in_quotes:
                values.append(self._parse_sql_value(current.strip()))
                current = ""
            else:
                current += char

        if current.strip():
            values.append(self._parse_sql_value(current.strip()))

        return values

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
        else:
            return {"String": str(value)}

    def _hex_to_uuid(self, hex_str: str) -> list[int]:
        """Convert hex string to UUID byte array."""
        hex_str = hex_str.replace("-", "")
        if len(hex_str) != 32:
            raise ProgrammingError(f"Invalid UUID hex string: {hex_str}")
        return [int(hex_str[i : i + 2], 16) for i in range(0, 32, 2)]

    def executemany(
        self, operation: str, seq_of_parameters: list[dict[str, Any]]
    ) -> None:
        """Execute a database operation multiple times."""
        for parameters in seq_of_parameters:
            self.execute(operation, parameters)

    def fetchone(self) -> tuple[Any, ...] | None:
        """Fetch the next row of a query result set."""
        if self._row_index < len(self._rows):
            row = self._rows[self._row_index]
            self._row_index += 1
            return row
        return None

    def fetchmany(self, size: int | None = None) -> list[tuple[Any, ...]]:
        """Fetch the next set of rows of a query result."""
        if size is None:
            size = self.arraysize

        rows = self._rows[self._row_index : self._row_index + size]
        self._row_index += len(rows)
        return rows

    def fetchall(self) -> list[tuple[Any, ...]]:
        """Fetch all remaining rows of a query result."""
        rows = self._rows[self._row_index :]
        self._row_index = len(self._rows)
        return rows

    def setinputsizes(self, sizes: list[Any]) -> None:
        """Set input sizes (no-op for ORMDB)."""
        pass

    def setoutputsize(self, size: int, column: int | None = None) -> None:
        """Set output size (no-op for ORMDB)."""
        pass

    def __iter__(self) -> Iterator[tuple[Any, ...]]:
        return self

    def __next__(self) -> tuple[Any, ...]:
        row = self.fetchone()
        if row is None:
            raise StopIteration
        return row

    def __enter__(self) -> "Cursor":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
