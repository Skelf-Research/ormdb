//! Metrics collection infrastructure.
//!
//! This module provides centralized metrics collection for observability,
//! including query latency histograms, mutation counts, and cache statistics.
//!
//! # Usage
//!
//! ```ignore
//! use ormdb_core::metrics::{MetricsRegistry, MutationType, new_shared_registry};
//!
//! // Create a shared registry
//! let registry = new_shared_registry();
//!
//! // Record query execution
//! registry.record_query("User", 1500); // 1.5ms
//!
//! // Record mutations
//! registry.record_mutation(MutationType::Insert, 1);
//!
//! // Record cache activity
//! registry.record_cache_hit();
//!
//! // Export to Prometheus format
//! let prometheus_text = registry.to_prometheus();
//! ```

mod histogram;
mod registry;

pub use histogram::Histogram;
pub use registry::{
    new_shared_registry, MetricsRegistry, MutationType, SharedMetricsRegistry,
};
