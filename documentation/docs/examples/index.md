# Examples

Learn ORMDB through complete, real-world application examples.

---

## Example Applications

### [Todo App](todo-app.md)

**Level:** Beginner

A classic todo list demonstrating CRUD operations and basic queries.

**You'll learn:**
- Schema design basics
- Create, read, update, delete operations
- Simple filtering and sorting
- Basic Rust backend setup

**Tech stack:** Rust + ORMDB

---

### [Blog Platform](blog-platform.md)

**Level:** Intermediate

A multi-author blog with posts, comments, and tags.

**You'll learn:**
- Complex relations (one-to-many, many-to-many)
- Graph queries for efficient data loading
- Cursor-based pagination
- Row-level security for author permissions

**Tech stack:** Rust + TypeScript + ORMDB

---

### [Multi-Tenant SaaS](multi-tenant-saas.md)

**Level:** Advanced

A complete SaaS template with tenant isolation and user management.

**You'll learn:**
- Tenant isolation patterns
- Capability-based security
- Field masking for sensitive data
- Schema migrations in production

**Tech stack:** Rust + TypeScript + ORMDB

---

### [Real-time Dashboard](realtime-dashboard.md)

**Level:** Advanced

An analytics dashboard with live updates and aggregations.

**You'll learn:**
- Change Data Capture (CDC)
- Aggregation queries
- Columnar storage for analytics
- Performance optimization

**Tech stack:** Rust + TypeScript + WebSockets + ORMDB

---

## Quick Reference

| Example | Difficulty | Key Concepts |
|---------|------------|--------------|
| Todo App | Beginner | CRUD, filtering |
| Blog Platform | Intermediate | Relations, pagination, RLS |
| Multi-Tenant SaaS | Advanced | Isolation, capabilities |
| Real-time Dashboard | Advanced | CDC, aggregations |

---

## Running Examples

All examples are available in the [ormdb-examples](https://github.com/Skelf-Research/ormdb-examples) repository.

### Clone and Run

```bash
# Clone examples repository
git clone https://github.com/Skelf-Research/ormdb-examples.git
cd ormdb-examples

# Choose an example
cd todo-app

# Run with cargo
cargo run
```

### Prerequisites

- Rust 1.75+
- Node.js 18+ (for TypeScript examples)
- ORMDB CLI (optional, for schema management)

```bash
# Install ORMDB CLI
cargo install ormdb-cli

# Verify installation
ormdb --version
```

---

## Code Structure

Each example follows a consistent structure:

```
example-name/
├── Cargo.toml           # Rust dependencies
├── schema.ormdb         # ORMDB schema definition
├── src/
│   ├── main.rs          # Application entry point
│   ├── db.rs            # Database setup
│   ├── models.rs        # Entity definitions
│   ├── handlers.rs      # Request handlers
│   └── queries.rs       # Query builders
├── frontend/            # (if applicable)
│   ├── package.json
│   └── src/
└── README.md            # Example-specific documentation
```

---

## Contributing Examples

We welcome community examples! To contribute:

1. Fork the examples repository
2. Create a new directory for your example
3. Follow the structure above
4. Include a comprehensive README
5. Submit a pull request

See the [contribution guide](https://github.com/Skelf-Research/ormdb-examples/blob/main/CONTRIBUTING.md) for details.

