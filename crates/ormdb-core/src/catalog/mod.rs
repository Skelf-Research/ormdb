//! Semantic catalog for ORMDB.
//!
//! The catalog stores metadata about entities, relations, constraints, and schema versions.

mod catalog;
mod constraint;
mod entity;
mod field;
mod relation;
mod schema;
mod types;

pub use catalog::Catalog;
pub use constraint::ConstraintDef;
pub use entity::{EntityDef, LifecycleRules, OrderBy, OrderDirection};
pub use field::{ComputedField, DefaultValue, FieldDef};
pub use relation::{Cardinality, DeleteBehavior, RelationDef};
pub use schema::SchemaBundle;
pub use types::{FieldType, ScalarType};
