//! ORMDB backend wrapper for comparison benchmarks.
//!
//! Provides the same interface as SqliteBackend for fair comparison.

use ormdb_proto::{EntityBlock, FilterExpr, GraphQuery, Pagination, RelationInclude, Value};

use crate::fixtures::{Scale, UserData};
use crate::harness::{insert_entity, TestContext};

use super::rows::{PostRow, UserRow};

/// ORMDB backend for benchmarks.
pub struct OrmdbBackend {
    ctx: TestContext,
}

impl OrmdbBackend {
    /// Create a new ORMDB backend with the specified scale.
    pub fn new(scale: Scale) -> Self {
        let ctx = TestContext::with_scale(scale);
        Self { ctx }
    }

    /// Get the underlying test context.
    pub fn context(&self) -> &TestContext {
        &self.ctx
    }

    // -------------------------------------------------------------------------
    // Query Operations
    // -------------------------------------------------------------------------

    /// Scan all users.
    pub fn scan_users(&self) -> Vec<UserRow> {
        let query = GraphQuery::new("User");
        let result = self.ctx.executor().execute(&query).unwrap();
        if result.entities.is_empty() {
            return vec![];
        }
        entity_block_to_user_rows(&result.entities[0])
    }

    /// Scan users with a limit.
    pub fn scan_users_limit(&self, limit: usize) -> Vec<UserRow> {
        let query = GraphQuery::new("User").with_pagination(Pagination::limit(limit as u32));
        let result = self.ctx.executor().execute(&query).unwrap();
        if result.entities.is_empty() {
            return vec![];
        }
        entity_block_to_user_rows(&result.entities[0])
    }

    /// Filter users by status.
    pub fn filter_users_by_status(&self, status: &str) -> Vec<UserRow> {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String(status.to_string())).into());
        let result = self.ctx.executor().execute(&query).unwrap();
        if result.entities.is_empty() {
            return vec![];
        }
        entity_block_to_user_rows(&result.entities[0])
    }

    /// Filter users by age (greater than).
    pub fn filter_users_by_age_gt(&self, age: i32) -> Vec<UserRow> {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(age)).into());
        let result = self.ctx.executor().execute(&query).unwrap();
        if result.entities.is_empty() {
            return vec![];
        }
        entity_block_to_user_rows(&result.entities[0])
    }

    /// Filter users by name pattern (LIKE).
    pub fn filter_users_by_name_like(&self, pattern: &str) -> Vec<UserRow> {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::like("name", pattern).into());
        let result = self.ctx.executor().execute(&query).unwrap();
        if result.entities.is_empty() {
            return vec![];
        }
        entity_block_to_user_rows(&result.entities[0])
    }

    // -------------------------------------------------------------------------
    // Row-Oriented Query Operations (faster path, skips column transposition)
    // -------------------------------------------------------------------------

    /// Scan all users using the row-oriented path (faster, no transposition).
    pub fn scan_users_rows(&self) -> Vec<UserRow> {
        let query = GraphQuery::new("User");
        let (rows, _has_more) = self.ctx.executor().execute_rows(&query).unwrap();
        entity_rows_to_user_rows(&rows)
    }

    /// Scan users with a limit using the row-oriented path.
    pub fn scan_users_limit_rows(&self, limit: usize) -> Vec<UserRow> {
        let query = GraphQuery::new("User").with_pagination(Pagination::limit(limit as u32));
        let (rows, _has_more) = self.ctx.executor().execute_rows(&query).unwrap();
        entity_rows_to_user_rows(&rows)
    }

    /// Filter users by status using the row-oriented path.
    pub fn filter_users_by_status_rows(&self, status: &str) -> Vec<UserRow> {
        let query = GraphQuery::new("User")
            .with_filter(FilterExpr::eq("status", Value::String(status.to_string())).into());
        let (rows, _has_more) = self.ctx.executor().execute_rows(&query).unwrap();
        entity_rows_to_user_rows(&rows)
    }

    /// Filter users by age (greater than) using the row-oriented path.
    pub fn filter_users_by_age_gt_rows(&self, age: i32) -> Vec<UserRow> {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(age)).into());
        let (rows, _has_more) = self.ctx.executor().execute_rows(&query).unwrap();
        entity_rows_to_user_rows(&rows)
    }

    /// Filter users by name pattern (LIKE) using the row-oriented path.
    pub fn filter_users_by_name_like_rows(&self, pattern: &str) -> Vec<UserRow> {
        let query =
            GraphQuery::new("User").with_filter(FilterExpr::like("name", pattern).into());
        let (rows, _has_more) = self.ctx.executor().execute_rows(&query).unwrap();
        entity_rows_to_user_rows(&rows)
    }

    // -------------------------------------------------------------------------
    // Graph Query Operations (ORMDB's N+1 elimination)
    // -------------------------------------------------------------------------

    /// Get users with their posts (automatic N+1 elimination).
    pub fn get_users_with_posts(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        let query = GraphQuery::new("User")
            .include(RelationInclude::new("posts"))
            .with_pagination(Pagination::limit(limit as u32));

        let result = self.ctx.executor().execute(&query).unwrap();

        if result.entities.is_empty() {
            return vec![];
        }

        let users = entity_block_to_user_rows(&result.entities[0]);

        // Posts are in the second entity block (if present)
        let posts_by_user = if result.entities.len() > 1 {
            group_posts_by_author(&result.entities[1])
        } else {
            std::collections::HashMap::new()
        };

        users
            .into_iter()
            .map(|user| {
                let posts = posts_by_user.get(&user.id).cloned().unwrap_or_default();
                (user, posts)
            })
            .collect()
    }

    // -------------------------------------------------------------------------
    // Mutation Operations
    // -------------------------------------------------------------------------

    /// Insert a single user.
    pub fn insert_user(&self, user: &UserData) {
        let fields = crate::fixtures::user_to_fields(user);
        insert_entity(&self.ctx.storage, "User", fields);
    }

    /// Insert multiple users (uses ORMDB transactions).
    pub fn insert_users_batch(&self, users: &[UserData]) {
        let mut txn = self.ctx.storage.transaction();
        for user in users {
            let fields = crate::fixtures::user_to_fields(user);
            let data = ormdb_core::query::encode_entity(&fields).unwrap();
            let key = ormdb_core::storage::VersionedKey::now(user.id);
            txn.put_typed("User", key, ormdb_core::storage::Record::new(data));
        }
        txn.commit().unwrap();
    }
}

/// Convert EntityBlock (column-oriented) to UserRow vector.
fn entity_block_to_user_rows(block: &EntityBlock) -> Vec<UserRow> {
    let get_column = |name: &str| -> Option<&Vec<Value>> {
        block.column(name).map(|c| &c.values)
    };

    let names = get_column("name");
    let emails = get_column("email");
    let ages = get_column("age");
    let statuses = get_column("status");

    block
        .ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            UserRow {
                id: hex::encode(id),
                name: names
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
                email: emails
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
                age: ages
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::Int32(n) => *n,
                        _ => 0,
                    })
                    .unwrap_or(0),
                status: statuses
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}

/// Convert EntityBlock (column-oriented) to PostRow vector.
fn entity_block_to_post_rows(block: &EntityBlock) -> Vec<PostRow> {
    let get_column = |name: &str| -> Option<&Vec<Value>> {
        block.column(name).map(|c| &c.values)
    };

    let titles = get_column("title");
    let contents = get_column("content");
    let author_ids = get_column("author_id");
    let views = get_column("views");
    let published = get_column("published");

    block
        .ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            PostRow {
                id: hex::encode(id),
                title: titles
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
                content: contents
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
                author_id: author_ids
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::Uuid(id) => hex::encode(id),
                        _ => String::new(),
                    })
                    .unwrap_or_default(),
                views: views
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::Int64(n) => *n,
                        _ => 0,
                    })
                    .unwrap_or(0),
                published: published
                    .and_then(|v| v.get(i))
                    .map(|v| match v {
                        Value::Bool(b) => *b,
                        _ => false,
                    })
                    .unwrap_or(false),
            }
        })
        .collect()
}

/// Group posts by author_id.
fn group_posts_by_author(block: &EntityBlock) -> std::collections::HashMap<String, Vec<PostRow>> {
    let posts = entity_block_to_post_rows(block);
    let mut map: std::collections::HashMap<String, Vec<PostRow>> = std::collections::HashMap::new();
    for post in posts {
        map.entry(post.author_id.clone()).or_default().push(post);
    }
    map
}

// =============================================================================
// Row-oriented conversion (faster path, skips column transposition)
// =============================================================================

use ormdb_core::query::EntityRow;

/// Convert EntityRow (row-oriented) to UserRow.
/// This is faster than going through EntityBlock as it skips transposition.
fn entity_row_to_user_row(row: &EntityRow) -> UserRow {
    let get_str = |name: &str| -> String {
        row.get_field(name)
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    let get_i32 = |name: &str| -> i32 {
        row.get_field(name)
            .and_then(|v| match v {
                Value::Int32(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0)
    };

    UserRow {
        id: hex::encode(row.id),
        name: get_str("name"),
        email: get_str("email"),
        age: get_i32("age"),
        status: get_str("status"),
    }
}

/// Convert a Vec<EntityRow> to Vec<UserRow> using the row-oriented path.
fn entity_rows_to_user_rows(rows: &[EntityRow]) -> Vec<UserRow> {
    rows.iter().map(entity_row_to_user_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ormdb_backend_basic() {
        let backend = OrmdbBackend::new(Scale::Small);
        let users = backend.scan_users();
        assert_eq!(users.len(), 100);
    }

    #[test]
    fn test_ormdb_filter_status() {
        let backend = OrmdbBackend::new(Scale::Small);
        let active_users = backend.filter_users_by_status("active");
        assert!(!active_users.is_empty());
        assert!(active_users.iter().all(|u| u.status == "active"));
    }
}
