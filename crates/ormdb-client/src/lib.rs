//! ORMDB Client - Client library for connecting to ORMDB servers.
//!
//! This crate provides async client functionality for ORMDB.
//!
//! # Quick Start
//!
//! ```ignore
//! use ormdb_client::{Client, ClientConfig};
//! use ormdb_proto::GraphQuery;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to the server
//!     let client = Client::connect(ClientConfig::localhost()).await?;
//!
//!     // Ping to check connectivity
//!     client.ping().await?;
//!
//!     // Execute a query
//!     let query = GraphQuery::new("User")
//!         .with_fields(vec!["id".into(), "name".into()]);
//!     let result = client.query(query).await?;
//!
//!     println!("Found {} entities", result.total_entities());
//!
//!     // Close the connection
//!     client.close().await;
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod config;
pub mod connection;
pub mod error;
pub mod pool;

pub use client::Client;
pub use config::ClientConfig;
pub use connection::{Connection, ConnectionState};
pub use error::Error;
pub use pool::{ConnectionPool, PoolConfig, PooledConnection};

/// Re-export protocol types.
pub use ormdb_proto as proto;
