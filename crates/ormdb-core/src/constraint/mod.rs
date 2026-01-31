//! Constraint enforcement module.
//!
//! This module provides constraint validation and enforcement for ORMDB:
//! - Unique constraints (single and composite)
//! - Foreign key constraints
//! - Check constraints (expression-based)

mod check_evaluator;
mod unique_index;
mod validator;

pub use check_evaluator::{CheckEvaluator, EvaluationError};
pub use unique_index::UniqueIndex;
pub use validator::ConstraintValidator;
