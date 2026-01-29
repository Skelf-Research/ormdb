//! ORMDB Server library.
//!
//! This crate provides the core server functionality for ORMDB,
//! including database management, mutation execution, and request handling.

pub mod config;
pub mod database;
pub mod error;
pub mod handler;
pub mod mutation;
pub mod transport;

pub use config::{Args, ServerConfig};
pub use database::{Database, SharedDatabase};
pub use error::Error;
pub use handler::RequestHandler;
pub use mutation::MutationExecutor;
pub use transport::{create_transport, Transport};
