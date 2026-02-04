# ORMDB vs SQLite: Developer Ergonomics Demo

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg)]()

> **ORMDB: N+1 queries are a bug. We made them impossible.**

An interactive terminal demo comparing ORMDB and SQLite side-by-side. See how ORMDB's graph-native queries, type-safe API, and automatic relation loading eliminate entire classes of bugs that plague traditional database access.

---

## Quick Start

```bash
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb/documentation/demos
cargo run --release
```

That's it. No database setup required.

---

## What You'll See

```
┌───────────────────────────────────────────────────────────────────────────┐
│  ORMDB  vs  SQLite   Developer Ergonomics                                 │
│  Compare code side-by-side. See why ORMDB improves developer experience.  │
├───────────────────────────────────────────────────────────────────────────┤
│  [1] N+1 Problem | [2] Type Safety | [3] Errors | [4] Schema | [5] Joins  │
├─────────────────────────────────┬─────────────────────────────────────────┤
│          ORMDB (Rust)           │            SQLite (Rust)                │
│  ───────────────────────────    │   ───────────────────────────           │
│                                 │                                         │
│  // Single query fetches all    │   // N+1 queries — one per user!        │
│  let query = GraphQuery::new()  │   let users = conn.prepare(             │
│      .include("posts");         │       "SELECT * FROM user")?;           │
│                                 │                                         │
│  let result = executor          │   for user in users {                   │
│      .execute(&query)?;         │       let posts = conn.prepare(         │
│                                 │           "SELECT * FROM post           │
│  // Done! All data loaded.      │            WHERE author_id = ?")?;      │
│                                 │   }                                     │
│                                 │   // 101 queries later...               │
│                                 │                                         │
├─────────────────────────────────┴─────────────────────────────────────────┤
│  ORMDB: 1 query   |   SQLite: 101 queries   |   ORMDB eliminates N+1      │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## Who Is This For?

### Evaluating ORMDB for your project?
Run the demo to see real code comparisons. Understand the API differences before committing to a technology choice.

### Explaining ORMDB to your team?
Use this as a presentation tool. Walk through each scenario and discuss the trade-offs.

### Learning about database ergonomics?
See concrete examples of common pitfalls (N+1 queries, runtime errors) and how modern APIs solve them.

### Writing a tech blog or documentation?
The demo provides ready-made code examples showing best practices vs anti-patterns.

---

## Scenarios Covered

| # | Scenario | What You'll Learn |
|---|----------|-------------------|
| 1 | **N+1 Query Problem** | How ORMDB loads related data in 1 query vs 101 |
| 2 | **Type Safety** | Catch typos at compile-time, not in production |
| 3 | **Error Handling** | Typed errors vs parsing `"UNIQUE constraint failed"` strings |
| 4 | **Schema Definition** | Declarative DSL vs manual DDL and triggers |
| 5 | **Relations & Joins** | Graph queries vs complex JOIN reconstruction |

---

## Controls

| Key | Action |
|-----|--------|
| `←` `→` | Navigate scenarios |
| `h` `l` | Navigate (vim-style) |
| `n` `p` | Next / Previous |
| `1`-`5` | Jump to scenario |
| `q` `Esc` | Quit |

---

## The Bottom Line

| Aspect | ORMDB | SQLite |
|--------|-------|--------|
| N+1 Prevention | Automatic | Manual optimization |
| Error Detection | Compile-time | Runtime |
| Error Types | Structured enums | String parsing |
| Schema | Type-safe DSL | DDL strings |
| Relations | Graph queries | Manual JOINs |

**ORMDB: Graph-native queries. Type-safe by default. N+1 eliminated.**

---

## Requirements

- **Rust 1.75+** — Install via [rustup](https://rustup.rs/)
- **Terminal** — Any modern terminal with Unicode support
- **Size** — 80x24 minimum (larger recommended)

## Tech Stack

Built with excellent Rust crates:

- [**ratatui**](https://ratatui.rs) — Modern terminal UI framework
- [**crossterm**](https://github.com/crossterm-rs/crossterm) — Cross-platform terminal handling

---

## Contributing

Found a bug? Want to add a scenario? PRs welcome!

```bash
# Run in development
cargo run

# Run optimized
cargo run --release
```

---

## Related Links

- [ORMDB Documentation](https://docs.skelfresearch.com/ormdb)
- [ORMDB vs SQLite (Performance)](../docs/comparisons/vs-sqlite.md)
- [Getting Started Guide](../docs/getting-started/quickstart.md)
- [GitHub Repository](https://github.com/Skelf-Research/ormdb)

---

<p align="center">
  <b>One query. All your data. Zero N+1.</b>
  <br><br>
  <code>cargo run --release</code>
</p>
