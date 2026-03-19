//! Database backends for comparison benchmarks.
//!
//! This module provides a common interface for ORMDB, SQLite, and PostgreSQL
//! to enable fair performance comparisons.

pub mod ormdb;
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

pub use ormdb::OrmdbBackend;
pub use sqlite::SqliteBackend;

#[cfg(feature = "postgres")]
pub use postgres::PostgresBackend;

/// Common row types for cross-backend benchmarks.
pub mod rows {
    /// User row returned by all backends.
    #[derive(Debug, Clone)]
    pub struct UserRow {
        pub id: String,
        pub name: String,
        pub email: String,
        pub age: i32,
        pub status: String,
    }

    /// Post row returned by all backends.
    #[derive(Debug, Clone)]
    pub struct PostRow {
        pub id: String,
        pub title: String,
        pub content: String,
        pub author_id: String,
        pub views: i64,
        pub published: bool,
    }

    /// Comment row returned by all backends.
    #[derive(Debug, Clone)]
    pub struct CommentRow {
        pub id: String,
        pub text: String,
        pub post_id: String,
        pub author_id: String,
    }
}
