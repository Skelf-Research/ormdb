//! Query builder for fluent query construction.
//!
//! The `Query` type provides a builder pattern for constructing queries.
//!
//! # Example
//!
//! ```rust,no_run
//! use ormdb::Database;
//!
//! let db = Database::open_memory().unwrap();
//!
//! // Simple query
//! let users = db.query("User")
//!     .filter("status", "active")
//!     .execute()
//!     .unwrap();
//!
//! // Query with includes and ordering
//! let users = db.query("User")
//!     .filter("status", "active")
//!     .include("posts")
//!     .order_by("name")
//!     .limit(10)
//!     .execute()
//!     .unwrap();
//! ```

use ormdb_core::query::QueryExecutor;
use ormdb_proto::{
    FilterExpr, GraphQuery, OrderSpec, Pagination, QueryResult as ProtoQueryResult,
    RelationInclude, Value,
};

use crate::database::Database;
use crate::entity::Entity;
use crate::error::Result;

/// A fluent query builder.
///
/// Use `Database::query()` to create a new query builder.
pub struct Query<'db> {
    db: &'db Database,
    inner: GraphQuery,
}

impl<'db> Query<'db> {
    /// Create a new query builder for an entity type.
    pub(crate) fn new(db: &'db Database, entity: &str) -> Self {
        Self {
            db,
            inner: GraphQuery::new(entity),
        }
    }

    /// Filter by field equality.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.query("User").filter("status", "active")
    /// ```
    pub fn filter(mut self, field: &str, value: impl Into<Value>) -> Self {
        let filter = FilterExpr::eq(field, value.into());
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field not equal.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.query("User").filter_ne("status", "deleted")
    /// ```
    pub fn filter_ne(mut self, field: &str, value: impl Into<Value>) -> Self {
        let filter = FilterExpr::ne(field, value.into());
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field greater than.
    pub fn filter_gt(mut self, field: &str, value: impl Into<Value>) -> Self {
        let filter = FilterExpr::gt(field, value.into());
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field greater than or equal.
    pub fn filter_gte(mut self, field: &str, value: impl Into<Value>) -> Self {
        let filter = FilterExpr::ge(field, value.into());
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field less than.
    pub fn filter_lt(mut self, field: &str, value: impl Into<Value>) -> Self {
        let filter = FilterExpr::lt(field, value.into());
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field less than or equal.
    pub fn filter_lte(mut self, field: &str, value: impl Into<Value>) -> Self {
        let filter = FilterExpr::le(field, value.into());
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field being null.
    pub fn filter_null(mut self, field: &str) -> Self {
        let filter = FilterExpr::is_null(field);
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field not being null.
    pub fn filter_not_null(mut self, field: &str) -> Self {
        let filter = FilterExpr::is_not_null(field);
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field matching a LIKE pattern.
    ///
    /// Use `%` as wildcard.
    pub fn filter_like(mut self, field: &str, pattern: &str) -> Self {
        let filter = FilterExpr::like(field, pattern);
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Filter by field value being in a list.
    pub fn filter_in(mut self, field: &str, values: Vec<Value>) -> Self {
        let filter = FilterExpr::in_values(field, values);
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Add a custom filter expression.
    ///
    /// For complex filters that can't be expressed with the convenience methods.
    pub fn filter_expr(mut self, filter: FilterExpr) -> Self {
        self.inner = self.inner.with_filter(filter.into());
        self
    }

    /// Include a relation in the results.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Include posts for each user
    /// db.query("User").include("posts")
    ///
    /// // Nested includes use dot notation
    /// db.query("User").include("posts.comments")
    /// ```
    pub fn include(mut self, relation: &str) -> Self {
        self.inner = self.inner.include(RelationInclude::new(relation));
        self
    }

    /// Include a relation with a filter.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ormdb::FilterExpr;
    ///
    /// db.query("User")
    ///     .include_filtered("posts", FilterExpr::eq("published", true))
    /// ```
    pub fn include_filtered(mut self, relation: &str, filter: FilterExpr) -> Self {
        let include = RelationInclude::new(relation).with_filter(filter.into());
        self.inner = self.inner.include(include);
        self
    }

    /// Select specific fields to return.
    ///
    /// If not called, all fields are returned.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.query("User").select(&["id", "name", "email"])
    /// ```
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.inner = self
            .inner
            .with_fields(fields.iter().map(|f| (*f).to_string()).collect());
        self
    }

    /// Order results by a field (ascending).
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.query("User").order_by("name")
    /// ```
    pub fn order_by(mut self, field: &str) -> Self {
        self.inner = self.inner.with_order(OrderSpec::asc(field));
        self
    }

    /// Order results by a field (descending).
    pub fn order_by_desc(mut self, field: &str) -> Self {
        self.inner = self.inner.with_order(OrderSpec::desc(field));
        self
    }

    /// Limit the number of results.
    pub fn limit(mut self, limit: u32) -> Self {
        let pagination = self.inner.pagination.take().unwrap_or_else(|| Pagination {
            limit: 0,
            offset: 0,
            cursor: None,
        });
        self.inner.pagination = Some(Pagination {
            limit,
            ..pagination
        });
        self
    }

    /// Skip a number of results (offset).
    pub fn offset(mut self, offset: u32) -> Self {
        let pagination = self.inner.pagination.take().unwrap_or_else(|| Pagination {
            limit: 0,
            offset: 0,
            cursor: None,
        });
        self.inner.pagination = Some(Pagination {
            offset,
            ..pagination
        });
        self
    }

    /// Execute the query and return all matching entities.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let users = db.query("User")
    ///     .filter("status", "active")
    ///     .execute()
    ///     .unwrap();
    ///
    /// for user in users {
    ///     println!("{}", user.get_string("name").unwrap_or("unknown"));
    /// }
    /// ```
    pub fn execute(self) -> Result<Vec<Entity>> {
        let executor = QueryExecutor::new(self.db.storage(), self.db.catalog());
        let result = executor.execute(&self.inner)?;
        Ok(QueryResult::from_proto(result).into_entities())
    }

    /// Execute the query and return the first matching entity.
    pub fn first(self) -> Result<Option<Entity>> {
        let result = self.limit(1).execute()?;
        Ok(result.into_iter().next())
    }

    /// Execute the query and return the count of matching entities.
    pub fn count(self) -> Result<u64> {
        // For counting, we use a minimal query
        let executor = QueryExecutor::new(self.db.storage(), self.db.catalog());
        let result = executor.execute(&self.inner)?;
        Ok(result.entities.first().map(|e| e.len() as u64).unwrap_or(0))
    }

    /// Get the underlying GraphQuery for advanced use.
    pub fn into_raw(self) -> GraphQuery {
        self.inner
    }
}

/// Result of a query execution.
pub struct QueryResult {
    inner: ProtoQueryResult,
}

impl QueryResult {
    /// Create from protocol result.
    pub(crate) fn from_proto(inner: ProtoQueryResult) -> Self {
        Self { inner }
    }

    /// Convert to a vector of entities.
    pub fn into_entities(self) -> Vec<Entity> {
        // The first entity block contains the root entities
        if self.inner.entities.is_empty() {
            return vec![];
        }

        let root_block = &self.inner.entities[0];
        let len = root_block.len();

        (0..len)
            .map(|i| Entity::from_block_index(root_block, i, &self.inner.entities))
            .collect()
    }

    /// Get the number of root entities.
    pub fn len(&self) -> usize {
        self.inner
            .entities
            .first()
            .map(|e| e.len())
            .unwrap_or(0)
    }

    /// Check if the result is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether there are more results (for pagination).
    pub fn has_more(&self) -> bool {
        self.inner.has_more
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder() {
        let db = Database::open_memory().unwrap();

        // Test building a query
        let query = db
            .query("User")
            .filter("status", "active")
            .order_by("name")
            .limit(10)
            .into_raw();

        assert_eq!(query.root_entity, "User");
        assert!(query.filter.is_some());
        assert!(!query.order_by.is_empty());
        assert!(query.pagination.is_some());
    }
}
