//! ORMDB - An embedded graph database with multi-writer support.
//!
//! ORMDB is an embedded database that provides a graph-aware ORM experience.
//! Unlike SQLite, ORMDB supports concurrent writes from multiple threads using
//! optimistic concurrency control.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use ormdb::{Database, ScalarType};
//!
//! // Open a database
//! let db = Database::open("./my_data").unwrap();
//!
//! // Define schema
//! db.schema()
//!     .entity("User")
//!         .field("id", ScalarType::Uuid).primary_key()
//!         .field("name", ScalarType::String)
//!         .field("email", ScalarType::String).unique()
//!     .apply()
//!     .unwrap();
//!
//! // Insert data
//! let user_id = db.insert("User")
//!     .set("name", "Alice")
//!     .set("email", "alice@example.com")
//!     .execute()
//!     .unwrap();
//!
//! // Query data
//! let users = db.query("User")
//!     .filter("name", "Alice")
//!     .execute()
//!     .unwrap();
//!
//! for user in users {
//!     println!("{}", user.get_string("name").unwrap_or("unknown"));
//! }
//! ```
//!
//! # Multi-Writer Concurrency
//!
//! ORMDB supports concurrent writes from multiple threads. If two transactions
//! modify the same entity, one will fail with `Error::TransactionConflict`.
//!
//! ```rust,no_run
//! use ormdb::Database;
//! use std::thread;
//!
//! let db = Database::open_memory().unwrap();
//!
//! // Clone is cheap - uses Arc internally
//! let handles: Vec<_> = (0..10).map(|i| {
//!     let db = db.clone();
//!     thread::spawn(move || {
//!         db.insert("User")
//!             .set("name", format!("User {}", i))
//!             .execute()
//!     })
//! }).collect();
//!
//! for h in handles {
//!     h.join().unwrap().unwrap();
//! }
//! ```
//!
//! # Transactions
//!
//! Use transactions for atomic operations:
//!
//! ```rust,no_run
//! use ormdb::Database;
//!
//! let db = Database::open_memory().unwrap();
//!
//! // Using with_transaction (auto-commit/rollback)
//! db.with_transaction(|tx| {
//!     tx.insert("User").set("name", "Alice").execute()?;
//!     tx.insert("User").set("name", "Bob").execute()?;
//!     Ok(())
//! }).unwrap();
//!
//! // Manual transaction control
//! let mut tx = db.transaction();
//! tx.insert("User").set("name", "Charlie").execute().unwrap();
//! tx.commit().unwrap();
//! ```
//!
//! # Feature Flags
//!
//! - `async` - Enable async API (adds tokio dependency)
//! - `btree-index` - Enable B-tree indexes for range queries
//! - `query-lang` - Enable query language parser
//! - `full` - Enable all features

// Module declarations
mod database;
mod entity;
mod error;
mod mutation;
mod query;
mod schema;
mod transaction;

// Public exports

// Database
pub use database::{Config, Database};

// Error handling
pub use error::{Error, Result};

// Query
pub use query::{Query, QueryResult};

// Mutations
pub use mutation::{Delete, Insert, Update};

// Schema
pub use schema::{EntityBuilder, FieldBuilder, RelationFieldBuilder, ScalarType, SchemaBuilder};

// Transactions
pub use transaction::{Transaction, TransactionalInsert, TransactionalUpdate};

// Entity results
pub use entity::Entity;

// Re-exports from ormdb-proto for user convenience
pub use ormdb_proto::{FilterExpr, GraphQuery, Value};

// Re-export storage types that users might need
pub use ormdb_core::storage::{CompactionResult, RetentionPolicy};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_usage() {
        // Open in-memory database
        let db = Database::open_memory().unwrap();

        // Define schema
        db.schema()
            .entity("User")
            .field("id", ScalarType::Uuid)
            .primary_key()
            .field("name", ScalarType::String)
            .apply()
            .unwrap();

        // Insert a user
        let user_id = db.insert("User").set("name", "Alice").execute().unwrap();

        assert_ne!(user_id, [0; 16]);
    }

    #[test]
    fn test_transaction() {
        let db = Database::open_memory().unwrap();

        db.with_transaction(|tx| {
            tx.insert("TestEntity").set("value", 1).execute()?;
            tx.insert("TestEntity").set("value", 2).execute()?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_query_builder() {
        let db = Database::open_memory().unwrap();

        // Define schema first
        db.schema()
            .entity("User")
            .field("id", ScalarType::Uuid)
            .primary_key()
            .field("name", ScalarType::String)
            .apply()
            .unwrap();

        // Insert some data
        db.insert("User").set("name", "Alice").execute().unwrap();
        db.insert("User").set("name", "Bob").execute().unwrap();

        // Query
        let users = db.query("User").execute().unwrap();

        // We should have 2 users
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn test_clone_is_cheap() {
        let db = Database::open_memory().unwrap();
        let db2 = db.clone();

        // Both point to the same underlying data
        assert_eq!(db.schema_version(), db2.schema_version());

        // Define schema
        db.schema()
            .entity("Test")
            .field("id", ScalarType::Uuid)
            .primary_key()
            .field("x", ScalarType::Int32)
            .apply()
            .unwrap();

        // Insert via one, see via other
        db.insert("Test").set("x", 1).execute().unwrap();
        let results = db2.query("Test").execute().unwrap();
        assert_eq!(results.len(), 1);
    }
}
