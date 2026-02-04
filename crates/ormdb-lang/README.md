# ormdb-lang

[![Crates.io](https://img.shields.io/crates/v/ormdb-lang.svg)](https://crates.io/crates/ormdb-lang)
[![Documentation](https://docs.rs/ormdb-lang/badge.svg)](https://docs.rs/ormdb-lang)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../../LICENSE)

ORM-style query language parser and compiler for [ORMDB](https://github.com/Skelf-Research/ormdb).

## Overview

`ormdb-lang` provides a human-readable query language that compiles to ORMDB's typed protocol IR. It offers:

- **Lexer** - Fast tokenization with `logos`
- **Parser** - Recursive descent parser for query syntax
- **Compiler** - Transforms AST to protocol IR
- **Error Reporting** - Clear, actionable error messages

## Query Syntax

```
// Fetch users with their posts
FETCH User
WHERE id = 1
INCLUDE posts {
    INCLUDE comments
}

// Create a new user
CREATE User {
    name: "Alice",
    email: "alice@example.com"
}

// Update with filter
UPDATE User
WHERE active = false
SET { archived: true }

// Delete with cascade
DELETE Post
WHERE author.id = 1
```

## Usage

```rust
use ormdb_lang::{Parser, Compiler};
use ormdb_proto::Query;

// Parse query text
let ast = Parser::parse("FETCH User WHERE id = 1")?;

// Compile to protocol IR
let query: Query = Compiler::compile(ast)?;
```

## Architecture

```
ormdb-lang/
├── src/
│   ├── lexer.rs      # Token definitions (logos)
│   ├── parser.rs     # Recursive descent parser
│   ├── ast.rs        # Abstract syntax tree
│   ├── compiler.rs   # AST to IR compilation
│   └── error.rs      # Error types and reporting
```

## License

MIT License - see [LICENSE](../../LICENSE) for details.

Part of the [ORMDB](https://github.com/Skelf-Research/ormdb) project.
