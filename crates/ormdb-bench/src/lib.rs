//! ORMDB Benchmark Suite
//!
//! This crate provides comprehensive benchmarks for ORMDB components using Criterion.
//!
//! # Benchmark Categories
//!
//! - **Storage**: Key-value operations, versioned reads, scans
//! - **Query**: Filter evaluation, sorting, pagination, includes
//! - **Mutation**: Insert, update, delete operations
//! - **Serialization**: rkyv vs JSON performance
//! - **Filter**: Filter expression evaluation
//! - **Join**: Join strategy comparison (NestedLoop vs HashJoin)
//! - **Cache**: Plan cache effectiveness
//! - **E2E**: End-to-end latency through client/gateway
//! - **Comparison**: Cross-database comparisons (ORMDB vs SQLite vs PostgreSQL)

pub mod backends;
pub mod fixtures;
pub mod harness;

pub use backends::{OrmdbBackend, SqliteBackend};
#[cfg(feature = "postgres")]
pub use backends::PostgresBackend;
pub use fixtures::{generate_comments, generate_posts, generate_users, Scale};
pub use harness::TestContext;
