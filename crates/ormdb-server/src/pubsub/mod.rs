//! Pub-sub infrastructure for change notifications.
//!
//! This module provides the infrastructure for subscribing to and publishing
//! change events. Full change data capture (CDC) will be implemented in Phase 6.

mod manager;
mod subscription;

pub use manager::PubSubManager;
pub use subscription::{SubscriptionEntry, SubscriptionFilter};
