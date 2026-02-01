//! PostgreSQL backend for comparison benchmarks.
//!
//! Requires a running PostgreSQL instance and DATABASE_URL environment variable.
//! Enable with `--features postgres`.

use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::runtime::Runtime;

use crate::fixtures::{generate_comments, generate_posts, generate_users, Scale, UserData};

use super::rows::{PostRow, UserRow};

/// PostgreSQL backend for benchmarks.
pub struct PostgresBackend {
    pool: PgPool,
    rt: Runtime,
}

impl PostgresBackend {
    /// Create a new PostgreSQL backend.
    ///
    /// Requires DATABASE_URL environment variable to be set.
    pub fn new(database_url: &str) -> Self {
        let rt = Runtime::new().expect("Failed to create Tokio runtime");

        let pool = rt.block_on(async {
            PgPoolOptions::new()
                .max_connections(10)
                .connect(database_url)
                .await
                .expect("Failed to connect to PostgreSQL")
        });

        Self { pool, rt }
    }

    /// Create from DATABASE_URL environment variable.
    pub fn from_env() -> Self {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
        Self::new(&database_url)
    }

    /// Set up the blog schema.
    pub fn setup_schema(&self) {
        self.rt.block_on(async {
            sqlx::query(
                r#"
                DROP TABLE IF EXISTS comment CASCADE;
                DROP TABLE IF EXISTS post CASCADE;
                DROP TABLE IF EXISTS "user" CASCADE;

                CREATE TABLE IF NOT EXISTS "user" (
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
                    author_id TEXT NOT NULL REFERENCES "user"(id),
                    views BIGINT NOT NULL,
                    published BOOLEAN NOT NULL
                );

                CREATE TABLE IF NOT EXISTS comment (
                    id TEXT PRIMARY KEY,
                    text TEXT NOT NULL,
                    post_id TEXT NOT NULL REFERENCES post(id),
                    author_id TEXT NOT NULL REFERENCES "user"(id)
                );

                CREATE INDEX IF NOT EXISTS idx_user_status ON "user"(status);
                CREATE INDEX IF NOT EXISTS idx_user_age ON "user"(age);
                CREATE INDEX IF NOT EXISTS idx_post_author ON post(author_id);
                CREATE INDEX IF NOT EXISTS idx_post_published ON post(published);
                CREATE INDEX IF NOT EXISTS idx_comment_post ON comment(post_id);
                CREATE INDEX IF NOT EXISTS idx_comment_author ON comment(author_id);
                "#,
            )
            .execute(&self.pool)
            .await
            .expect("Failed to create schema")
        });
    }

    /// Populate the database with benchmark data.
    pub fn populate(&self, scale: Scale) {
        let user_count = scale.count();
        let posts_per_user = scale.posts_per_user();
        let comments_per_post = scale.comments_per_post();

        let users = generate_users(user_count);
        let user_ids: Vec<_> = users.iter().map(|u| u.id).collect();

        self.rt.block_on(async {
            // Insert users
            for user in &users {
                sqlx::query(
                    r#"INSERT INTO "user" (id, name, email, age, status) VALUES ($1, $2, $3, $4, $5)"#,
                )
                .bind(uuid_to_string(&user.id))
                .bind(&user.name)
                .bind(&user.email)
                .bind(user.age)
                .bind(&user.status)
                .execute(&self.pool)
                .await
                .expect("Failed to insert user");
            }

            // Insert posts
            let post_count = user_count * posts_per_user;
            let posts = generate_posts(post_count, &user_ids);
            let post_ids: Vec<_> = posts.iter().map(|p| p.id).collect();

            for post in &posts {
                sqlx::query(
                    r#"INSERT INTO post (id, title, content, author_id, views, published) VALUES ($1, $2, $3, $4, $5, $6)"#,
                )
                .bind(uuid_to_string(&post.id))
                .bind(&post.title)
                .bind(&post.content)
                .bind(uuid_to_string(&post.author_id))
                .bind(post.views)
                .bind(post.published)
                .execute(&self.pool)
                .await
                .expect("Failed to insert post");
            }

            // Insert comments
            let comment_count = post_count * comments_per_post;
            let comments = generate_comments(comment_count, &post_ids, &user_ids);

            for comment in &comments {
                sqlx::query(
                    r#"INSERT INTO comment (id, text, post_id, author_id) VALUES ($1, $2, $3, $4)"#,
                )
                .bind(uuid_to_string(&comment.id))
                .bind(&comment.text)
                .bind(uuid_to_string(&comment.post_id))
                .bind(uuid_to_string(&comment.author_id))
                .execute(&self.pool)
                .await
                .expect("Failed to insert comment");
            }
        });
    }

    /// Create a backend with schema and data.
    pub fn with_scale(scale: Scale) -> Self {
        let backend = Self::from_env();
        backend.setup_schema();
        backend.populate(scale);
        backend
    }

    // -------------------------------------------------------------------------
    // Query Operations
    // -------------------------------------------------------------------------

    /// Scan all users.
    pub fn scan_users(&self) -> Vec<UserRow> {
        self.rt.block_on(async {
            sqlx::query(r#"SELECT id, name, email, age, status FROM "user""#)
                .fetch_all(&self.pool)
                .await
                .expect("Failed to scan users")
                .into_iter()
                .map(|row| UserRow {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    age: row.get("age"),
                    status: row.get("status"),
                })
                .collect()
        })
    }

    /// Scan users with a limit.
    pub fn scan_users_limit(&self, limit: usize) -> Vec<UserRow> {
        self.rt.block_on(async {
            sqlx::query(r#"SELECT id, name, email, age, status FROM "user" LIMIT $1"#)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .expect("Failed to scan users")
                .into_iter()
                .map(|row| UserRow {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    age: row.get("age"),
                    status: row.get("status"),
                })
                .collect()
        })
    }

    /// Filter users by status.
    pub fn filter_users_by_status(&self, status: &str) -> Vec<UserRow> {
        self.rt.block_on(async {
            sqlx::query(r#"SELECT id, name, email, age, status FROM "user" WHERE status = $1"#)
                .bind(status)
                .fetch_all(&self.pool)
                .await
                .expect("Failed to filter users")
                .into_iter()
                .map(|row| UserRow {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    age: row.get("age"),
                    status: row.get("status"),
                })
                .collect()
        })
    }

    /// Filter users by age (greater than).
    pub fn filter_users_by_age_gt(&self, age: i32) -> Vec<UserRow> {
        self.rt.block_on(async {
            sqlx::query(r#"SELECT id, name, email, age, status FROM "user" WHERE age > $1"#)
                .bind(age)
                .fetch_all(&self.pool)
                .await
                .expect("Failed to filter users")
                .into_iter()
                .map(|row| UserRow {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    age: row.get("age"),
                    status: row.get("status"),
                })
                .collect()
        })
    }

    /// Filter users by name pattern.
    pub fn filter_users_by_name_like(&self, pattern: &str) -> Vec<UserRow> {
        self.rt.block_on(async {
            sqlx::query(r#"SELECT id, name, email, age, status FROM "user" WHERE name LIKE $1"#)
                .bind(pattern)
                .fetch_all(&self.pool)
                .await
                .expect("Failed to filter users")
                .into_iter()
                .map(|row| UserRow {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    age: row.get("age"),
                    status: row.get("status"),
                })
                .collect()
        })
    }

    // -------------------------------------------------------------------------
    // N+1 vs Batched vs JOIN Comparisons
    // -------------------------------------------------------------------------

    /// Get users with posts using N+1 pattern.
    pub fn get_users_with_posts_n_plus_1(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        self.rt.block_on(async {
            let users = sqlx::query(
                r#"SELECT id, name, email, age, status FROM "user" LIMIT $1"#,
            )
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .expect("Failed to fetch users");

            let mut results = Vec::with_capacity(users.len());

            for row in users {
                let user = UserRow {
                    id: row.get("id"),
                    name: row.get("name"),
                    email: row.get("email"),
                    age: row.get("age"),
                    status: row.get("status"),
                };

                let posts: Vec<PostRow> = sqlx::query(
                    r#"SELECT id, title, content, author_id, views, published FROM post WHERE author_id = $1"#,
                )
                .bind(&user.id)
                .fetch_all(&self.pool)
                .await
                .expect("Failed to fetch posts")
                .into_iter()
                .map(|row| PostRow {
                    id: row.get("id"),
                    title: row.get("title"),
                    content: row.get("content"),
                    author_id: row.get("author_id"),
                    views: row.get("views"),
                    published: row.get("published"),
                })
                .collect();

                results.push((user, posts));
            }

            results
        })
    }

    /// Get users with posts using batched IN query.
    pub fn get_users_with_posts_batched(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        self.rt.block_on(async {
            let users: Vec<UserRow> = sqlx::query(
                r#"SELECT id, name, email, age, status FROM "user" LIMIT $1"#,
            )
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .expect("Failed to fetch users")
            .into_iter()
            .map(|row| UserRow {
                id: row.get("id"),
                name: row.get("name"),
                email: row.get("email"),
                age: row.get("age"),
                status: row.get("status"),
            })
            .collect();

            if users.is_empty() {
                return vec![];
            }

            let user_ids: Vec<&str> = users.iter().map(|u| u.id.as_str()).collect();

            // Use ANY for PostgreSQL
            let posts: Vec<PostRow> = sqlx::query(
                r#"SELECT id, title, content, author_id, views, published FROM post WHERE author_id = ANY($1)"#,
            )
            .bind(&user_ids)
            .fetch_all(&self.pool)
            .await
            .expect("Failed to fetch posts")
            .into_iter()
            .map(|row| PostRow {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                author_id: row.get("author_id"),
                views: row.get("views"),
                published: row.get("published"),
            })
            .collect();

            // Group posts by author
            let mut post_map: std::collections::HashMap<String, Vec<PostRow>> =
                std::collections::HashMap::new();
            for post in posts {
                post_map.entry(post.author_id.clone()).or_default().push(post);
            }

            users
                .into_iter()
                .map(|user| {
                    let posts = post_map.remove(&user.id).unwrap_or_default();
                    (user, posts)
                })
                .collect()
        })
    }

    /// Get users with posts using a JOIN.
    pub fn get_users_with_posts_join(&self, limit: usize) -> Vec<(UserRow, Vec<PostRow>)> {
        self.rt.block_on(async {
            let rows = sqlx::query(
                r#"
                SELECT
                    u.id as user_id, u.name, u.email, u.age, u.status,
                    p.id as post_id, p.title, p.content, p.author_id, p.views, p.published
                FROM "user" u
                LEFT JOIN post p ON u.id = p.author_id
                WHERE u.id IN (SELECT id FROM "user" LIMIT $1)
                ORDER BY u.id
                "#,
            )
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .expect("Failed to fetch users with posts");

            let mut results: Vec<(UserRow, Vec<PostRow>)> = vec![];
            let mut current_user_id: Option<String> = None;

            for row in rows {
                let user_id: String = row.get("user_id");

                if current_user_id.as_ref() != Some(&user_id) {
                    current_user_id = Some(user_id.clone());
                    let user = UserRow {
                        id: user_id,
                        name: row.get("name"),
                        email: row.get("email"),
                        age: row.get("age"),
                        status: row.get("status"),
                    };
                    results.push((user, vec![]));
                }

                if let Ok(post_id) = row.try_get::<String, _>("post_id") {
                    let post = PostRow {
                        id: post_id,
                        title: row.get("title"),
                        content: row.get("content"),
                        author_id: row.get("author_id"),
                        views: row.get("views"),
                        published: row.get("published"),
                    };
                    if let Some((_, posts)) = results.last_mut() {
                        posts.push(post);
                    }
                }
            }

            results
        })
    }

    // -------------------------------------------------------------------------
    // Mutation Operations
    // -------------------------------------------------------------------------

    /// Insert a single user.
    pub fn insert_user(&self, user: &UserData) {
        self.rt.block_on(async {
            sqlx::query(
                r#"INSERT INTO "user" (id, name, email, age, status) VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(uuid_to_string(&user.id))
            .bind(&user.name)
            .bind(&user.email)
            .bind(user.age)
            .bind(&user.status)
            .execute(&self.pool)
            .await
            .expect("Failed to insert user");
        });
    }

    /// Insert multiple users in a batch.
    pub fn insert_users_batch(&self, users: &[UserData]) {
        self.rt.block_on(async {
            let mut tx = self.pool.begin().await.expect("Failed to begin transaction");

            for user in users {
                sqlx::query(
                    r#"INSERT INTO "user" (id, name, email, age, status) VALUES ($1, $2, $3, $4, $5)"#,
                )
                .bind(uuid_to_string(&user.id))
                .bind(&user.name)
                .bind(&user.email)
                .bind(user.age)
                .bind(&user.status)
                .execute(&mut *tx)
                .await
                .expect("Failed to insert user");
            }

            tx.commit().await.expect("Failed to commit transaction");
        });
    }

    /// Update a user's name.
    pub fn update_user(&self, id: &str, name: &str) {
        self.rt.block_on(async {
            sqlx::query(r#"UPDATE "user" SET name = $1 WHERE id = $2"#)
                .bind(name)
                .bind(id)
                .execute(&self.pool)
                .await
                .expect("Failed to update user");
        });
    }

    /// Delete a user.
    pub fn delete_user(&self, id: &str) {
        self.rt.block_on(async {
            sqlx::query(r#"DELETE FROM "user" WHERE id = $1"#)
                .bind(id)
                .execute(&self.pool)
                .await
                .expect("Failed to delete user");
        });
    }
}

/// Convert a UUID byte array to a hex string.
fn uuid_to_string(id: &[u8; 16]) -> String {
    hex::encode(id)
}
