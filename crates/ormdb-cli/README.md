# ormdb-cli

[![Crates.io](https://img.shields.io/crates/v/ormdb-cli.svg)](https://crates.io/crates/ormdb-cli)
[![Documentation](https://docs.rs/ormdb-cli/badge.svg)](https://docs.rs/ormdb-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

Command-line interface for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-cli` provides an interactive REPL and scripting interface for ORMDB. Features include:

- **Interactive REPL** - Query and explore your database
- **Syntax Highlighting** - Colored output for readability
- **History** - Command history with search
- **Scripting** - Execute query files
- **Table Output** - Formatted table display

## Installation

```bash
# Install from crates.io
cargo install ormdb-cli

# Or build from source
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build --release -p ormdb-cli
```

## Usage

### Interactive Mode

```bash
# Connect to local server
ormdb

# Connect to specific server
ormdb --host localhost --port 5432

# Connect with URL
ormdb --url ormdb://localhost:5432/mydb
```

### REPL Commands

```
ormdb> FETCH User WHERE id = 1 INCLUDE posts

┌────┬─────────┬─────────────────────┐
│ id │ name    │ email               │
├────┼─────────┼─────────────────────┤
│ 1  │ Alice   │ alice@example.com   │
└────┴─────────┴─────────────────────┘

ormdb> .tables          # List all entities
ormdb> .schema User     # Show entity schema
ormdb> .indexes User    # Show indexes
ormdb> .help            # Show help
ormdb> .quit            # Exit
```

### Script Mode

```bash
# Execute a query file
ormdb --file queries.oql

# Execute inline query
ormdb --execute "FETCH User LIMIT 10"

# Output as JSON
ormdb --execute "FETCH User" --format json
```

## Configuration

Configuration file: `~/.config/ormdb/config.toml`

```toml
[connection]
default_url = "ormdb://localhost:5432"

[display]
format = "table"  # table, json, csv
color = true

[history]
max_entries = 1000
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
