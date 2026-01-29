//! ORMDB Client - Client library for connecting to ORMDB servers.
//!
//! This crate provides async client functionality for ORMDB.

pub mod error;

pub use error::Error;

/// Re-export protocol types.
pub use ormdb_proto as proto;
