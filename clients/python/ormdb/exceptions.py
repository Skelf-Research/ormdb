"""ORMDB client exceptions."""


class OrmdbError(Exception):
    """Base exception for ORMDB errors."""

    def __init__(self, message: str, code: str | None = None):
        super().__init__(message)
        self.message = message
        self.code = code


class ConnectionError(OrmdbError):
    """Failed to connect to ORMDB gateway."""

    pass


class QueryError(OrmdbError):
    """Query execution failed."""

    pass


class MutationError(OrmdbError):
    """Mutation execution failed."""

    pass


class SchemaError(OrmdbError):
    """Schema-related error."""

    pass


class ValidationError(OrmdbError):
    """Request validation error."""

    pass
