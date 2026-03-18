"""Database creation for ORMDB Django backend."""

from typing import Any

from django.db.backends.base.creation import BaseDatabaseCreation


class DatabaseCreation(BaseDatabaseCreation):
    """Database creation operations for ORMDB."""

    def create_test_db(
        self,
        verbosity: int = 1,
        autoclobber: bool = False,
        serialize: bool = True,
        keepdb: bool = False,
    ) -> str:
        """Create a test database.

        ORMDB doesn't have traditional database creation.
        Tests run against the same instance.
        """
        test_database_name = self._get_test_db_name()

        if verbosity >= 1:
            print(f"Using ORMDB instance for testing: {test_database_name}")

        self.connection.close()
        return test_database_name

    def destroy_test_db(
        self,
        old_database_name: str | None = None,
        verbosity: int = 1,
        keepdb: bool = False,
        suffix: str | None = None,
    ) -> None:
        """Destroy a test database.

        ORMDB doesn't have traditional database destruction.
        This is a no-op.
        """
        if verbosity >= 1:
            print("Cleaning up ORMDB test data...")

        self.connection.close()

    def _get_test_db_name(self) -> str:
        """Return the test database name."""
        return self.connection.settings_dict.get("TEST", {}).get(
            "NAME", "ormdb_test"
        )

    def _clone_test_db(
        self,
        suffix: str,
        verbosity: int = 1,
        keepdb: bool = False,
    ) -> None:
        """Clone a test database (not supported)."""
        pass

    def sql_table_creation_suffix(self) -> str:
        """Return SQL suffix for table creation."""
        return ""

    def _create_test_db(
        self,
        verbosity: int = 1,
        autoclobber: bool = False,
        keepdb: bool = False,
    ) -> str:
        """Create test database internals."""
        return self._get_test_db_name()

    def _destroy_test_db(
        self, test_database_name: str, verbosity: int = 1
    ) -> None:
        """Destroy test database internals."""
        pass
