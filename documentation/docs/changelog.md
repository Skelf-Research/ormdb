# Changelog

All notable changes to ORMDB are documented here.

## [Unreleased]

### Added
- Initial public release
- Core database engine with ACID transactions
- GraphQuery API for efficient data fetching
- Relation includes with configurable depth
- Row-level security (RLS) support
- Change Data Capture (CDC) streaming
- Rust, TypeScript, and Python clients

---

## Version History

### [1.0.0] - Initial Release

#### Core Features

- **Graph Queries**
  - Fetch entities with related data in single query
  - Configurable include depth and limits
  - Query budgets to prevent runaway queries

- **Type-Safe Protocol**
  - Strongly typed GraphQuery and Mutation types
  - Value enum with all scalar types
  - Compile-time validation in Rust client

- **Relations**
  - One-to-one, one-to-many, many-to-many support
  - Configurable delete behaviors (cascade, set null, restrict)
  - Foreign key constraints

- **Filtering**
  - Comparison operators (eq, ne, lt, le, gt, ge)
  - Pattern matching (like, ilike)
  - Set operations (in, not_in)
  - Null checks (is_null, is_not_null)
  - Logical operators (and, or, not)

- **Mutations**
  - Insert with auto-generated IDs
  - Update by ID or filter
  - Delete with cascade support
  - Upsert operations
  - Batch mutations

- **Transactions**
  - ACID compliance
  - Optimistic concurrency control
  - Transaction isolation

#### Storage

- Embedded storage engine based on Sled
- Efficient B+ tree indexes
- Write-ahead logging (WAL)
- Automatic compaction
- Configurable cache size

#### Security

- Row-level security policies
- Capability tokens for fine-grained access
- Field masking for sensitive data
- Audit logging

#### Operations

- Prometheus metrics endpoint
- Health and readiness checks
- CLI for administration
- Backup and restore
- Schema migrations

#### Clients

- **Rust Client**
  - Native async/await support
  - Connection pooling
  - Type-safe queries

- **TypeScript Client**
  - Direct client API
  - Prisma-style adapter
  - Drizzle adapter
  - TypeORM adapter

- **Python Client**
  - Sync and async support
  - SQLAlchemy integration
  - Django integration

---

## Migration Guides

### Upgrading from Pre-release

If upgrading from a pre-release version:

1. Backup your data
   ```bash
   ormdb backup create pre-release-backup.ormdb
   ```

2. Stop the server
   ```bash
   ormdb server stop
   ```

3. Update the binary
   ```bash
   cargo install ormdb-cli --version 1.0.0
   ```

4. Run migrations
   ```bash
   ormdb migrate run
   ```

5. Start the server
   ```bash
   ormdb server start
   ```

---

## Versioning Policy

ORMDB follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Incompatible API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

### Stability Guarantees

| Component | Stability |
|-----------|-----------|
| Wire protocol | Stable after 1.0 |
| Rust client API | Stable after 1.0 |
| TypeScript client API | Stable after 1.0 |
| Python client API | Stable after 1.0 |
| CLI interface | Stable after 1.0 |
| Configuration | May change with migration path |
| Internal APIs | No guarantees |

---

## Release Schedule

- **Patch releases**: As needed for bug fixes
- **Minor releases**: Approximately monthly
- **Major releases**: As needed for breaking changes

---

## Reporting Issues

Found a bug or have a feature request?

- [GitHub Issues](https://github.com/Skelf-Research/ormdb/issues)
- [Discussions](https://github.com/Skelf-Research/ormdb/discussions)

When reporting bugs, please include:

1. ORMDB version (`ormdb --version`)
2. Operating system and version
3. Steps to reproduce
4. Expected vs actual behavior
5. Relevant logs or error messages
