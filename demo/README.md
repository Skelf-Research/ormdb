# ORMDB Studio - Demo Deployment

This directory contains deployment configurations for running ORMDB Studio.

## Quick Start

### Local Docker

```bash
# Build and run with docker-compose
cd demo
docker-compose up --build

# Or build manually
docker build -f demo/Dockerfile -t ormdb-studio .
docker run -p 3000:3000 -v ormdb-data:/app/data ormdb-studio
```

Then open http://localhost:3000 in your browser.

### CapRover Deployment

1. Ensure you have CapRover CLI installed:
   ```bash
   npm install -g caprover
   ```

2. Login to your CapRover instance:
   ```bash
   caprover login
   ```

3. Create an app in CapRover dashboard or via CLI:
   ```bash
   caprover api --path /user/apps/appDefinitions --method POST --data '{"appName":"ormdb-studio"}'
   ```

4. Deploy from the repository root:
   ```bash
   caprover deploy -a ormdb-studio
   ```

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `ORMDB_STUDIO_HOST` | `0.0.0.0` | Bind address |
| `ORMDB_STUDIO_PORT` | `3000` | HTTP port |
| `ORMDB_STUDIO_DATA_DIR` | `/app/data` | Data directory for session databases |
| `ORMDB_STUDIO_SESSION_TIMEOUT` | `60` | Session timeout in minutes |
| `ORMDB_STUDIO_MAX_SESSIONS` | `10` | Maximum concurrent sessions |

## Architecture

```
┌─────────────────────────────────────────┐
│           CapRover / Docker             │
│  ┌───────────────────────────────────┐  │
│  │         ormdb-studio              │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │   Axum HTTP Server          │  │  │
│  │  │   - Vue.js SPA (embedded)   │  │  │
│  │  │   - REST API                │  │  │
│  │  │   - WebSocket Terminal      │  │  │
│  │  └─────────────────────────────┘  │  │
│  │              │                    │  │
│  │  ┌───────────▼───────────────┐   │  │
│  │  │   Session Manager         │   │  │
│  │  │   (isolated temp DBs)     │   │  │
│  │  └───────────────────────────┘   │  │
│  └───────────────────────────────────┘  │
│                  │                      │
│  ┌───────────────▼───────────────────┐  │
│  │         /app/data (volume)        │  │
│  │   Session databases persisted     │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## Features

- **Query REPL** - Execute queries with syntax highlighting
- **Terminal Emulator** - Full xterm.js terminal for command-line interaction
- **Visual Query Builder** - Drag-and-drop interface for constructing queries
- **Schema Explorer** - Browse entities, fields, and relations
- **Session Isolation** - Each browser session gets its own temporary database

## Usage Examples

Once deployed, access the terminal and try:

```bash
# Define entities
.entity User { name: String, email: String }
.entity Post { title: String, content: String, author_id: Uuid }

# Create data
User.create({ name: "Alice", email: "alice@example.com" })

# Query data
User.findMany()
User.findMany().where(name == "Alice")

# Define relations
.relation posts: Post.author_id -> User.id

# Query with includes
User.findMany().include(posts)
```

## Health Check

The container includes a health check endpoint at `/` that returns the SPA.

```bash
curl http://localhost:3000/
```

## Persistent Data

Session databases are stored in `/app/data`. Mount a volume to persist data across container restarts:

```bash
docker run -v /path/to/data:/app/data ormdb-studio
```
