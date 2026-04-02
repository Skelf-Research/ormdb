//! ORMDB Server library.
//!
//! This crate provides the core server functionality for ORMDB,
//! including database management, mutation execution, and request handling.

pub mod auth;
pub mod cascade;
pub mod cdc;
pub mod config;
pub mod database;
pub mod error;
pub mod handler;
pub mod mutation;
pub mod pubsub;
pub mod replication;
pub mod transport;

pub use auth::{ApiKeyAuthenticator, CapabilityAuthenticator, JwtAuthenticator, TokenAuthenticator};
pub use cascade::{CascadeExecutor, CascadeResult};
pub use cdc::{CDCHandle, CDCProcessor, CDCSender};
pub use config::{
    Args, AuthMethod, ConnectionLimits, RateLimitConfig, ServerConfig, TlsConfig,
};
pub use database::{CompactionTask, Database, SharedDatabase};
pub use error::Error;
pub use handler::RequestHandler;
pub use mutation::MutationExecutor;
pub use pubsub::{PubSubManager, SubscriptionEntry, SubscriptionFilter};
pub use replication::{ReplicationManager, SharedReplicationManager};
pub use transport::{create_transport, Transport, TransportMetrics};
