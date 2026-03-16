//! Query engine for ORMDB.
//!
//! This module implements the query executor that compiles GraphQuery IR into
//! execution plans and returns EntityBlock/EdgeBlock results.

mod cache;
mod cost;
mod executor;
mod filter;
mod join;
mod planner;
mod statistics;
mod value_codec;

pub use cache::{CacheStats, CachedPlan, PlanCache, QueryFingerprint};
pub use cost::{CostEstimate, CostModel};
pub use executor::QueryExecutor;
pub use filter::FilterEvaluator;
pub use join::{execute_join, EntityRow, HashJoinExecutor, JoinStrategy, NestedLoopExecutor};
pub use planner::{estimate_fanout, FanoutBudget, IncludePlan, QueryPlan, QueryPlanner};
pub use statistics::TableStatistics;
pub use value_codec::{decode_entity, decode_fields, encode_entity, get_field};
