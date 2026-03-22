//! Test data generation for benchmarks.
//!
//! This module provides consistent data generators for benchmark reproducibility.

use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle};
use ormdb_proto::Value;
use rand::distributions::Alphanumeric;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Scale factor for benchmark data generation.
#[derive(Clone, Copy, Debug)]
pub enum Scale {
    /// Tiny scale: ~30 entities total (10 users, 20 posts, 20 comments)
    /// Use for quick tests and development iteration.
    Tiny,
    /// Small scale: ~100 entities per type
    Small,
    /// Medium scale: ~10,000 entities per type
    Medium,
    /// Large scale: ~100,000 entities per type
    Large,
}

impl Scale {
    /// Get the entity count for this scale.
    pub fn count(&self) -> usize {
        match self {
            Scale::Tiny => 10,
            Scale::Small => 100,
            Scale::Medium => 2_000,  // Reduced from 10,000 for faster tests
            Scale::Large => 100_000,
        }
    }

    /// Get the posts per user ratio.
    pub fn posts_per_user(&self) -> usize {
        match self {
            Scale::Tiny => 2,
            Scale::Small => 5,
            Scale::Medium => 5,  // Reduced from 10
            Scale::Large => 10,
        }
    }

    /// Get the comments per post ratio.
    pub fn comments_per_post(&self) -> usize {
        match self {
            Scale::Tiny => 1,
            Scale::Small => 3,
            Scale::Medium => 2,  // Reduced from 5
            Scale::Large => 5,
        }
    }
}

impl Default for Scale {
    fn default() -> Self {
        Scale::Medium
    }
}

/// User entity data for benchmarks.
pub struct UserData {
    pub id: [u8; 16],
    pub name: String,
    pub email: String,
    pub age: i32,
    pub status: String,
}

/// Post entity data for benchmarks.
pub struct PostData {
    pub id: [u8; 16],
    pub title: String,
    pub content: String,
    pub author_id: [u8; 16],
    pub views: i64,
    pub published: bool,
}

/// Comment entity data for benchmarks.
pub struct CommentData {
    pub id: [u8; 16],
    pub text: String,
    pub post_id: [u8; 16],
    pub author_id: [u8; 16],
}

/// Generate a deterministic UUID from seed and index.
fn generate_uuid(seed: u64, index: usize) -> [u8; 16] {
    let mut rng = StdRng::seed_from_u64(seed.wrapping_add(index as u64));
    let mut id = [0u8; 16];
    rng.fill(&mut id);
    id
}

/// Generate a random string of specified length.
fn random_string(rng: &mut StdRng, len: usize) -> String {
    (0..len).map(|_| rng.sample(Alphanumeric) as char).collect()
}

/// Generate User entities with realistic field distribution.
pub fn generate_users(count: usize) -> Vec<UserData> {
    const SEED: u64 = 12345;
    let mut rng = StdRng::seed_from_u64(SEED);

    let statuses = ["active", "inactive", "pending", "admin"];
    let name_prefixes = [
        "Alice", "Bob", "Charlie", "David", "Eve", "Frank", "Grace", "Henry", "Ivy", "Jack",
    ];

    (0..count)
        .map(|i| {
            let id = generate_uuid(SEED, i);
            let name_prefix = name_prefixes[i % name_prefixes.len()];
            let name = format!("{}_{}", name_prefix, i);
            let email = format!("user{}@example{}.com", i, i % 10);
            let age = 18 + (rng.gen::<u32>() % 60) as i32;
            let status = statuses[i % statuses.len()].to_string();

            UserData {
                id,
                name,
                email,
                age,
                status,
            }
        })
        .collect()
}

/// Generate Post entities with foreign keys to Users.
pub fn generate_posts(count: usize, user_ids: &[[u8; 16]]) -> Vec<PostData> {
    const SEED: u64 = 54321;
    let mut rng = StdRng::seed_from_u64(SEED);

    (0..count)
        .map(|i| {
            let id = generate_uuid(SEED, i);
            let title = format!("Post Title {}: {}", i, random_string(&mut rng, 20));
            let content = random_string(&mut rng, 200);
            let author_id = user_ids[i % user_ids.len()];
            let views = rng.gen::<i64>().abs() % 1_000_000;
            let published = rng.gen_bool(0.8);

            PostData {
                id,
                title,
                content,
                author_id,
                views,
                published,
            }
        })
        .collect()
}

/// Generate Comment entities with foreign keys to Posts and Users.
pub fn generate_comments(
    count: usize,
    post_ids: &[[u8; 16]],
    user_ids: &[[u8; 16]],
) -> Vec<CommentData> {
    const SEED: u64 = 98765;
    let mut rng = StdRng::seed_from_u64(SEED);

    (0..count)
        .map(|i| {
            let id = generate_uuid(SEED, i);
            let text = random_string(&mut rng, 100);
            let post_id = post_ids[i % post_ids.len()];
            let author_id = user_ids[i % user_ids.len()];

            CommentData {
                id,
                text,
                post_id,
                author_id,
            }
        })
        .collect()
}

/// Convert UserData to field values for storage.
pub fn user_to_fields(user: &UserData) -> Vec<(String, Value)> {
    vec![
        ("id".to_string(), Value::Uuid(user.id)),
        ("name".to_string(), Value::String(user.name.clone())),
        ("email".to_string(), Value::String(user.email.clone())),
        ("age".to_string(), Value::Int32(user.age)),
        ("status".to_string(), Value::String(user.status.clone())),
    ]
}

/// Convert PostData to field values for storage.
pub fn post_to_fields(post: &PostData) -> Vec<(String, Value)> {
    vec![
        ("id".to_string(), Value::Uuid(post.id)),
        ("title".to_string(), Value::String(post.title.clone())),
        ("content".to_string(), Value::String(post.content.clone())),
        ("author_id".to_string(), Value::Uuid(post.author_id)),
        ("views".to_string(), Value::Int64(post.views)),
        ("published".to_string(), Value::Bool(post.published)),
    ]
}

/// Convert CommentData to field values for storage.
pub fn comment_to_fields(comment: &CommentData) -> Vec<(String, Value)> {
    vec![
        ("id".to_string(), Value::Uuid(comment.id)),
        ("text".to_string(), Value::String(comment.text.clone())),
        ("post_id".to_string(), Value::Uuid(comment.post_id)),
        ("author_id".to_string(), Value::Uuid(comment.author_id)),
    ]
}

/// Create the blog schema (User -> Posts -> Comments).
pub fn blog_schema() -> SchemaBundle {
    let user = EntityDef::new("User", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("age", FieldType::Scalar(ScalarType::Int32)))
        .with_field(FieldDef::new(
            "status",
            FieldType::Scalar(ScalarType::String),
        ));

    let post = EntityDef::new("Post", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new(
            "title",
            FieldType::Scalar(ScalarType::String),
        ))
        .with_field(FieldDef::new(
            "content",
            FieldType::Scalar(ScalarType::String),
        ))
        .with_field(FieldDef::new(
            "author_id",
            FieldType::Scalar(ScalarType::Uuid),
        ))
        .with_field(FieldDef::new("views", FieldType::Scalar(ScalarType::Int64)))
        .with_field(FieldDef::new(
            "published",
            FieldType::Scalar(ScalarType::Bool),
        ));

    let comment = EntityDef::new("Comment", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("text", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new(
            "post_id",
            FieldType::Scalar(ScalarType::Uuid),
        ))
        .with_field(FieldDef::new(
            "author_id",
            FieldType::Scalar(ScalarType::Uuid),
        ));

    let user_posts = RelationDef::one_to_many("posts", "User", "id", "Post", "author_id");
    let post_comments = RelationDef::one_to_many("comments", "Post", "id", "Comment", "post_id");

    SchemaBundle::new(1)
        .with_entity(user)
        .with_entity(post)
        .with_entity(comment)
        .with_relation(user_posts)
        .with_relation(post_comments)
}

/// Generate record data of specified size for raw storage benchmarks.
pub fn generate_record_data(size: usize) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(42);
    (0..size).map(|_| rng.gen()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_users() {
        let users = generate_users(100);
        assert_eq!(users.len(), 100);

        // Check deterministic generation
        let users2 = generate_users(100);
        assert_eq!(users[0].id, users2[0].id);
        assert_eq!(users[0].name, users2[0].name);
    }

    #[test]
    fn test_generate_posts() {
        let users = generate_users(10);
        let user_ids: Vec<_> = users.iter().map(|u| u.id).collect();
        let posts = generate_posts(50, &user_ids);

        assert_eq!(posts.len(), 50);
        // All posts should reference valid users
        for post in &posts {
            assert!(user_ids.contains(&post.author_id));
        }
    }

    #[test]
    fn test_scale_counts() {
        assert_eq!(Scale::Tiny.count(), 10);
        assert_eq!(Scale::Small.count(), 100);
        assert_eq!(Scale::Medium.count(), 2_000);
        assert_eq!(Scale::Large.count(), 100_000);
    }
}
