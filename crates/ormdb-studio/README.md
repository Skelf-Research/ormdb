# ormdb-studio

[![Crates.io](https://img.shields.io/crates/v/ormdb-studio.svg)](https://crates.io/crates/ormdb-studio)
[![Documentation](https://docs.rs/ormdb-studio/badge.svg)](https://docs.rs/ormdb-studio)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

Web-based database management studio for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-studio` provides a browser-based interface for creating, exploring, and managing ORMDB databases. Features include:

- **Query REPL** - Execute queries with syntax highlighting and results display
- **Terminal Emulator** - Full xterm.js terminal for command-line interaction
- **Visual Query Builder** - Drag-and-drop interface for constructing queries
- **Schema Explorer** - Browse entities, fields, and relations
- **Session Isolation** - Each browser session gets its own temporary database

## Installation

```bash
# Install from crates.io
cargo install ormdb-studio

# Or build from source
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb
cargo build --release -p ormdb-studio
```

## Usage

```bash
# Start the studio (opens browser automatically)
ormdb-studio

# Specify port
ormdb-studio --port 8080

# Specify data directory for session databases
ormdb-studio --data-dir ./studio-data

# Configure session timeout (minutes)
ormdb-studio --session-timeout 120

# Don't open browser automatically
ormdb-studio --no-open
```

## Features

### Terminal Commands

#### Schema Definition
```bash
# Define an entity (id: Uuid is added automatically)
.entity User { name: String, email: String }

# With optional fields (suffix with ?)
.entity Post { title: String, content: String?, views: Int }

# With array fields (suffix with [])
.entity Tag { name: String, posts: String[] }

# View current schema
.schema
```

**Available Types:**
- `String` - UTF-8 text
- `Int` / `Int64` - 32/64-bit integers
- `Float` / `Float64` - 32/64-bit floating point
- `Bool` - Boolean
- `Uuid` - UUID identifier
- `Timestamp` - Date/time
- `Bytes` - Binary data

#### Other Commands
```bash
.help          # Show all commands
.schema        # Display current schema
.session       # Show session info
.clear         # Clear terminal
```

### Query Editor
- Syntax highlighting for ORMDB query language
- Auto-completion for entities and fields
- Query history
- Multiple result formats (table, JSON)

### Terminal
- Full terminal emulator powered by xterm.js
- Command history and auto-completion
- WebSocket connection for real-time interaction

### Visual Query Builder
- Drag entities from schema palette
- Connect relations visually
- Add filters with UI controls
- See generated query in real-time

### Schema Explorer
- Tree view of all entities
- View fields, types, and constraints
- See relations between entities

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `--port` | `3000` | HTTP server port |
| `--host` | `127.0.0.1` | Bind address (localhost only for security) |
| `--data-dir` | System temp | Directory for session databases |
| `--session-timeout` | `60` | Session timeout in minutes |
| `--max-sessions` | `10` | Maximum concurrent sessions |
| `--no-open` | `false` | Don't open browser on start |

## Architecture

```
Browser                    ormdb-studio
┌─────────────────┐       ┌─────────────────┐
│  Vue.js SPA     │◄─────►│  Axum Server    │
│  - Terminal     │  HTTP │  - REST API     │
│  - Query Editor │  WS   │  - WebSocket    │
│  - Query Builder│       │  - Static Files │
└─────────────────┘       └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │ Session Manager │
                          │ ┌─────────────┐ │
                          │ │ Session 1   │ │
                          │ │ (temp DB)   │ │
                          │ ├─────────────┤ │
                          │ │ Session 2   │ │
                          │ │ (temp DB)   │ │
                          │ └─────────────┘ │
                          └─────────────────┘
```

## Development

### Building the Frontend

```bash
cd crates/ormdb-studio/frontend
npm install
npm run build
```

### Running in Development

```bash
# Frontend dev server (with hot reload)
cd crates/ormdb-studio/frontend
npm run dev

# Backend (in another terminal)
cargo run -p ormdb-studio -- --port 3001
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
