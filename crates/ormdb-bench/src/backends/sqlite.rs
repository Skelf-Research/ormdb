//! SQLite backend for comparison benchmarks.
//!
//! Provides an SQLite implementation with equivalent operations to ORMDB
//! for fair performance comparison.

use rusqlite::{params, Connection};

use crate::fixtures::{generate_comments, generate_posts, generate_users, Scale, UserData};

use super::rows::{PostRow, UserRow};

/// SQLite backend for benchmarks.
pub struct SqliteBackend {
    conn: Connection,
}

impl SqliteBackend {
    /// Create a new in-memory SQLite database.
    pub fn new() -> Self {
        let conn = Connection::open_in_memory().expect("Failed to open SQLite in-memory database");
        Self { conn }
    }

    /// Create a new SQLite database with a temp file (for larger datasets).
    pub fn new_temp_file() -> Self {
        let conn = Connection::open("").expect("Failed to open SQLite temp file database");
        Self { conn }
    }

    /// Set up the blog schema (User, Post, Comment tables).
    pub fn setup_schema(&self) {
        self.conn
            .execute_batch(
                r#"
            CREATE TABLE IF NOT EXISTS user (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                age INTEGER NOT NULL,
                status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS post (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                author_id TEXT NOT NULL,
                views INTEGER NOT NULL,
                published INTEGER NOT NULL,
                FOREIGN KEY (author_id) REFERENCES user(id)
            );

            CREATE TABLE IF NOT EXISTS comment (
                id TEXT PRIMARY KEY,
                text TEXT NOT NULL,
                post_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                FOREIGN KEY (post_id) REFERENCES post(id),
                FOREIGN KEY (author_id) REFERENCES user(id)
            );

            CREATE INDEX IF NOT EXISTS idx_user_status ON user(status);
            CREATE INDEX IF NOT EXISTS idx_user_age ON user(age);
            CREATE INDEX IF NOT EXISTS idx_post_author ON post(author_id);
            CREATE INDEX IF NOT EXISTS idx_post_published ON post(published);
            CREATE INDEX IF NOT EXISTS idx_comment_post ON comment(post_id);
            CREATE INDEX IF NOT EXISTS idx_comment_author ON comment(author_id);
            "#,
            )
            .expect("Failed to create schema");
    }

    /// Populate the database with benchmark data at the specified scale.
    pub fn populate(&self, scale: Scale) {
        let user_count = scale.count();
        let posts_per_user = scale.posts_per_user();
        let comments_per_post = scale.comments_per_post();

        let users = generate_users(user_count);
        let user_ids: Vec<_> = users.iter().map(|u| u.id).collect();

        // Insert users in a transaction
        self.conn.execute("BEGIN TRANSACTION", []).unwrap();
        {
            let mut stmt = self
                .conn
                .prepare("INSERT INTO user (id, name, email, age, status) VALUES (?1, ?2, ?3, ?4, ?5)")
                .unwrap();

            for user in &users {
                stmt.execute(params![
                    uuid_to_string(&user.id),
                    &user.name,
                    &user.email,
                    user.age,
                    &user.status
                ])
                .unwrap();
            }
        }
        self.conn.execute("COMMIT", []).unwrap();

        // Insert posts
        let post_count = user_count * posts_per_user;
        let posts = generate_posts(post_count, &user_ids);
        let post_ids: Vec<_> = posts.iter().map(|p| p.id).collect();

        self.conn.execute("BEGIN TRANSACTION", []).unwrap();
        {
            let mut stmt = self
                .conn
                .prepare("INSERT INTO post (id, title, content, author_id, views, published) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
                .unwrap();

            for post in &posts {
                stmt.execute(params![
                    uuid_to_string(&post.id),
                    &post.title,
                    &post.content,
                    uuid_to_string(&post.author_id),
                    post.views,
                    post.published as i32
                ])
                .unwrap();
            }
        }
        self.conn.execute("COMMIT", []).unwrap();

        // Insert comments
        let comment_count = post_count * comments_per_post;
        let comments = generate_comments(comment_count, &post_ids, &user_ids);

        self.conn.execute("BEGIN TRANSACTION", []).unwrap();
        {
            let mut stmt = self
                .conn
                .prepare("INSERT INTO comment (id, text, post_id, author_id) VALUES (?1, ?2, ?3, ?4)")
                .unwrap();

            for comment in &comments {
                stmt.execute(params![
                    uuid_to_string(&comment.id),
                    &comment.text,
                    uuid_to_string(&comment.post_id),
                    uuid_to_string(&comment.author_id)
                ])
                .unwrap();
            }
        }
        self.conn.execute("COMMIT", []).unwrap();
    }

    /// Create a new backend with schema and data populated.
    pub fn with_scale(scale: Scale) -> Self {
        let backend = Self::new();
        backend.setup_schema();
        backend.populate(scale);
        backend
    }

    // -------------------------------------------------------------------------
    // Query Operations
    // -------------------------------------------------------------------------

    /// Scan all users (equivalent to GraphQuery::new("User")).
    pub fn scan_users(&self) -> Vec<UserRow> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, email, age, status FROM user")
            .unwrap();

        stmt.query_map([], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                age: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    /// Scan users with a limit.
    pub fn scan_users_limit(&self, limit: usize) -> Vec<UserRow> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, email, age, status FROM user LIMIT ?1")
            .unwrap();

        stmt.query_map([limit as i64], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                age: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    /// Filter users by status (equality filter).
    pub fn filter_users_by_status(&self, status: &str) -> Vec<UserRow> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, email, age, status FROM user WHERE status = ?1")
            .unwrap();

        stmt.query_map([status], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                age: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    /// Filter users by age (range filter).
    pub fn filter_users_by_age_gt(&self, age: i32) -> Vec<UserRow> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, email, age, status FROM user WHERE age > ?1")
            .unwrap();

        stmt.query_map([age], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                age: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    /// Filter users by name pattern (LIKE filter).
    pub fn filter_users_by_name_like(&self, pattern: &str) -> Vec<UserRow> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, email, age, status FROM user WHERE name LIKE ?1")
            .unwrap();

        stmt.query_map([pattern], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                age: row.get(3)?,
                status: row.get(4)?,
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    // -------------------------------------------------------------------------
    // N+1 vs Batched vs JOIN Comparisons
    // -------------------------------------------------------------------------

    /// Get users with their posts using N+1 pattern (1 query per user).
    /// This is the anti-pattern that ORMDB eliminates.
    pub fn get_users_with_posts_n_plus_1(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        let users = self.scan_users_limit(limit);
        let mut results = Vec::with_capacity(users.len());

        let mut post_stmt = self
            .conn
            .prepare("SELECT id, title, content, author_id, views, published FROM post WHERE author_id = ?1")
            .unwrap();

        for user in users {
            let posts: Vec<PostRow> = post_stmt
                .query_map([&user.id], |row| {
                    Ok(PostRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        author_id: row.get(3)?,
                        views: row.get(4)?,
                        published: row.get::<_, i32>(5)? != 0,
                    })
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();

            results.push((user, posts));
        }

        results
    }

    /// Get users with their posts using batched IN query.
    /// This is similar to what ORMDB does automatically.
    pub fn get_users_with_posts_batched(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        let users = self.scan_users_limit(limit);
        if users.is_empty() {
            return vec![];
        }

        // Build IN clause
        let user_ids: Vec<&str> = users.iter().map(|u| u.id.as_str()).collect();
        let placeholders: Vec<String> = (1..=user_ids.len()).map(|i| format!("?{}", i)).collect();
        let in_clause = placeholders.join(", ");

        let query = format!(
            "SELECT id, title, content, author_id, views, published FROM post WHERE author_id IN ({})",
            in_clause
        );

        let mut stmt = self.conn.prepare(&query).unwrap();

        // Bind parameters
        let params: Vec<&dyn rusqlite::ToSql> =
            user_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

        let posts: Vec<PostRow> = stmt
            .query_map(params.as_slice(), |row| {
                Ok(PostRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    author_id: row.get(3)?,
                    views: row.get(4)?,
                    published: row.get::<_, i32>(5)? != 0,
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // Group posts by author
        let mut post_map: std::collections::HashMap<String, Vec<PostRow>> =
            std::collections::HashMap::new();
        for post in posts {
            post_map
                .entry(post.author_id.clone())
                .or_default()
                .push(post);
        }

        // Build results
        users
            .into_iter()
            .map(|user| {
                let posts = post_map.remove(&user.id).unwrap_or_default();
                (user, posts)
            })
            .collect()
    }

    /// Get users with their posts using a JOIN query.
    /// Most efficient for SQL databases.
    pub fn get_users_with_posts_join(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        let query = r#"
            SELECT
                u.id, u.name, u.email, u.age, u.status,
                p.id, p.title, p.content, p.author_id, p.views, p.published
            FROM user u
            LEFT JOIN post p ON u.id = p.author_id
            WHERE u.id IN (SELECT id FROM user LIMIT ?1)
            ORDER BY u.id
        "#;

        let mut stmt = self.conn.prepare(query).unwrap();

        let rows: Vec<(UserRow, Option<PostRow>)> = stmt
            .query_map([limit as i64], |row| {
                let user = UserRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    age: row.get(3)?,
                    status: row.get(4)?,
                };

                let post = if let Ok(post_id) = row.get::<_, String>(5) {
                    Some(PostRow {
                        id: post_id,
                        title: row.get(6)?,
                        content: row.get(7)?,
                        author_id: row.get(8)?,
                        views: row.get(9)?,
                        published: row.get::<_, i32>(10)? != 0,
                    })
                } else {
                    None
                };

                Ok((user, post))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // Group by user
        let mut results: Vec<(UserRow, Vec<PostRow>)> = vec![];
        let mut current_user_id: Option<String> = None;

        for (user, post) in rows {
            if current_user_id.as_ref() != Some(&user.id) {
                current_user_id = Some(user.id.clone());
                results.push((user, vec![]));
            }

            if let Some(post) = post {
                if let Some((_, posts)) = results.last_mut() {
                    posts.push(post);
                }
            }
        }

        results
    }

    // -------------------------------------------------------------------------
    // Mutation Operations
    // -------------------------------------------------------------------------

    /// Insert a single user (uses INSERT OR REPLACE for upsert behavior).
    pub fn insert_user(&self, user: &UserData) {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO user (id, name, email, age, status) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    uuid_to_string(&user.id),
                    &user.name,
                    &user.email,
                    user.age,
                    &user.status
                ],
            )
            .unwrap();
    }

    /// Insert multiple users in a batch transaction.
    pub fn insert_users_batch(&self, users: &[UserData]) {
        self.conn.execute("BEGIN TRANSACTION", []).unwrap();
        {
            let mut stmt = self
                .conn
                .prepare("INSERT OR REPLACE INTO user (id, name, email, age, status) VALUES (?1, ?2, ?3, ?4, ?5)")
                .unwrap();

            for user in users {
                stmt.execute(params![
                    uuid_to_string(&user.id),
                    &user.name,
                    &user.email,
                    user.age,
                    &user.status
                ])
                .unwrap();
            }
        }
        self.conn.execute("COMMIT", []).unwrap();
    }

    /// Update a user's name.
    pub fn update_user(&self, id: &str, name: &str) {
        self.conn
            .execute(
                "UPDATE user SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .unwrap();
    }

    /// Delete a user.
    pub fn delete_user(&self, id: &str) {
        self.conn
            .execute("DELETE FROM user WHERE id = ?1", params![id])
            .unwrap();
    }
}

impl Default for SqliteBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a UUID byte array to a hex string.
fn uuid_to_string(id: &[u8; 16]) -> String {
    hex::encode(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_backend_basic() {
        let backend = SqliteBackend::with_scale(Scale::Small);
        let users = backend.scan_users();
        assert_eq!(users.len(), 100);
    }

    #[test]
    fn test_sqlite_filter_status() {
        let backend = SqliteBackend::with_scale(Scale::Small);
        let active_users = backend.filter_users_by_status("active");
        assert!(!active_users.is_empty());
        assert!(active_users.iter().all(|u| u.status == "active"));
    }

    #[test]
    fn test_sqlite_n_plus_1_vs_batched() {
        let backend = SqliteBackend::with_scale(Scale::Small);

        let n_plus_1_results = backend.get_users_with_posts_n_plus_1(10);
        let batched_results = backend.get_users_with_posts_batched(10);
        let join_results = backend.get_users_with_posts_join(10);

        // All should return the same number of users
        assert_eq!(n_plus_1_results.len(), batched_results.len());
        assert_eq!(batched_results.len(), join_results.len());
    }
}
