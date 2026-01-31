"""Database client for ORMDB Django backend."""

from typing import Any

from django.db.backends.base.client import BaseDatabaseClient


class DatabaseClient(BaseDatabaseClient):
    """Database client for shell access to ORMDB."""

    executable_name = "ormdb"

    def runshell(self, parameters: list[str] | None = None) -> None:
        """Run a shell command (not implemented for ORMDB)."""
        import subprocess
        import sys

        args = [self.executable_name]

        # Add connection parameters
        settings = self.connection.settings_dict
        if settings.get("HOST"):
            args.extend(["--host", settings["HOST"]])
        if settings.get("PORT"):
            args.extend(["--port", str(settings["PORT"])])

        if parameters:
            args.extend(parameters)

        try:
            subprocess.run(args, check=True)
        except FileNotFoundError:
            print(
                f"Error: {self.executable_name} not found. "
                "ORMDB CLI is not installed.",
                file=sys.stderr,
            )
        except subprocess.CalledProcessError as e:
            print(f"Error running ORMDB shell: {e}", file=sys.stderr)

    def settings_to_cmd_args_env(
        self,
        settings_dict: dict[str, Any] | None = None,
        parameters: list[str] | None = None,
    ) -> tuple[list[str], dict[str, str] | None]:
        """Convert settings to command arguments and environment."""
        if settings_dict is None:
            settings_dict = self.connection.settings_dict

        args = [self.executable_name]

        if settings_dict.get("HOST"):
            args.extend(["--host", settings_dict["HOST"]])
        if settings_dict.get("PORT"):
            args.extend(["--port", str(settings_dict["PORT"])])

        if parameters:
            args.extend(parameters)

        return args, None
