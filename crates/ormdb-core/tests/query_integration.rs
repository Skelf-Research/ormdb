//! Integration tests for the query engine.

use ormdb_core::catalog::{
    Catalog, EntityDef, FieldDef, FieldType, RelationDef, ScalarType, SchemaBundle,
};
use ormdb_core::query::{encode_entity, FanoutBudget, QueryExecutor};
use ormdb_core::storage::{Record, StorageConfig, StorageEngine, VersionedKey};
use ormdb_proto::{
    FilterExpr, GraphQuery, OrderSpec, Pagination, RelationInclude, SimpleFilter, Value,
};

struct TestContext {
    storage: StorageEngine,
    catalog: Catalog,
    _storage_dir: tempfile::TempDir,
    _catalog_db: sled::Db,
}

impl TestContext {
    fn new() -> Self {
        let storage_dir = tempfile::tempdir().unwrap();
        let storage = StorageEngine::open(StorageConfig::new(storage_dir.path())).unwrap();
        let catalog_db = sled::Config::new().temporary(true).open().unwrap();
        let catalog = Catalog::open(&catalog_db).unwrap();

        Self {
            storage,
            catalog,
            _storage_dir: storage_dir,
            _catalog_db: catalog_db,
        }
    }

    fn executor(&self) -> QueryExecutor<'_> {
        QueryExecutor::new(&self.storage, &self.catalog)
    }
}

fn setup_blog_schema(ctx: &TestContext) {
    let user = EntityDef::new("User", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("email", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new("age", FieldType::Scalar(ScalarType::Int32)));

    let post = EntityDef::new("Post", "id")
        .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
        .with_field(FieldDef::new("title", FieldType::Scalar(ScalarType::String)))
        .with_field(FieldDef::new(
            "content",
            FieldType::Scalar(ScalarType::String),
        ))
        .with_field(FieldDef::new(
            "author_id",
            FieldType::Scalar(ScalarType::Uuid),
        ))
        .with_field(FieldDef::new("views", FieldType::Scalar(ScalarType::Int64)));

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

    let schema = SchemaBundle::new(1)
        .with_entity(user)
        .with_entity(post)
        .with_entity(comment)
        .with_relation(user_posts)
        .with_relation(post_comments);

    ctx.catalog.apply_schema(schema).unwrap();
}

fn insert_entity(ctx: &TestContext, entity_type: &str, fields: Vec<(&str, Value)>) -> [u8; 16] {
    let id = StorageEngine::generate_id();
    let mut field_data: Vec<(String, Value)> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

    // Add id field if not present
    if !field_data.iter().any(|(k, _)| k == "id") {
        field_data.insert(0, ("id".to_string(), Value::Uuid(id)));
    }

    let data = encode_entity(&field_data).unwrap();
    let key = VersionedKey::now(id);
    ctx.storage
        .put_typed(entity_type, key, Record::new(data))
        .unwrap();
    id
}

// ============== Tests ==============

#[test]
fn test_simple_entity_query() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    // Insert users
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("email", Value::String("alice@example.com".to_string())),
            ("age", Value::Int32(30)),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Bob".to_string())),
            ("email", Value::String("bob@example.com".to_string())),
            ("age", Value::Int32(25)),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User");
    let result = executor.execute(&query).unwrap();

    assert_eq!(result.entities.len(), 1);
    assert_eq!(result.entities[0].entity, "User");
    assert_eq!(result.entities[0].len(), 2);
}

#[test]
fn test_filter_equality() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("email", Value::String("alice@example.com".to_string())),
            ("age", Value::Int32(30)),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Bob".to_string())),
            ("email", Value::String("bob@example.com".to_string())),
            ("age", Value::Int32(25)),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User")
        .with_filter(FilterExpr::eq("name", Value::String("Alice".to_string())).into());

    let result = executor.execute(&query).unwrap();

    assert_eq!(result.entities[0].len(), 1);
    let name_col = result.entities[0].column("name").unwrap();
    assert_eq!(name_col.values[0], Value::String("Alice".to_string()));
}

#[test]
fn test_filter_comparison() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    for i in 0..10 {
        insert_entity(
            &ctx,
            "User",
            vec![
                ("name", Value::String(format!("User{}", i))),
                ("email", Value::String(format!("user{}@example.com", i))),
                ("age", Value::Int32(20 + i)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();

    // Test greater than
    let query =
        GraphQuery::new("User").with_filter(FilterExpr::gt("age", Value::Int32(25)).into());
    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 4); // ages 26, 27, 28, 29

    // Test less than or equal
    let query =
        GraphQuery::new("User").with_filter(FilterExpr::le("age", Value::Int32(22)).into());
    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 3); // ages 20, 21, 22
}

#[test]
fn test_filter_compound() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("email", Value::String("alice@example.com".to_string())),
            ("age", Value::Int32(30)),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Bob".to_string())),
            ("email", Value::String("bob@test.com".to_string())),
            ("age", Value::Int32(25)),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Charlie".to_string())),
            ("email", Value::String("charlie@example.com".to_string())),
            ("age", Value::Int32(35)),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();

    // AND filter: age > 25 AND email contains example.com
    let query = GraphQuery::new("User").with_filter(
        FilterExpr::and(vec![
            SimpleFilter::Gt {
                field: "age".to_string(),
                value: Value::Int32(25),
            },
            SimpleFilter::Like {
                field: "email".to_string(),
                pattern: "%example.com".to_string(),
            },
        ])
        .into(),
    );

    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 2); // Alice (30) and Charlie (35)
}

#[test]
fn test_sorting_ascending() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Charlie".to_string())),
            ("age", Value::Int32(35)),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int32(30)),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Bob".to_string())),
            ("age", Value::Int32(25)),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").with_order(OrderSpec::asc("name"));

    let result = executor.execute(&query).unwrap();
    let name_col = result.entities[0].column("name").unwrap();

    assert_eq!(name_col.values[0], Value::String("Alice".to_string()));
    assert_eq!(name_col.values[1], Value::String("Bob".to_string()));
    assert_eq!(name_col.values[2], Value::String("Charlie".to_string()));
}

#[test]
fn test_sorting_descending() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    for i in 0..5 {
        insert_entity(
            &ctx,
            "User",
            vec![
                ("name", Value::String(format!("User{}", i))),
                ("age", Value::Int32(20 + i)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").with_order(OrderSpec::desc("age"));

    let result = executor.execute(&query).unwrap();
    let age_col = result.entities[0].column("age").unwrap();

    assert_eq!(age_col.values[0], Value::Int32(24));
    assert_eq!(age_col.values[4], Value::Int32(20));
}

#[test]
fn test_pagination() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    for i in 0..20 {
        insert_entity(
            &ctx,
            "User",
            vec![
                ("name", Value::String(format!("User{:02}", i))),
                ("age", Value::Int32(20 + i)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();

    // First page
    let query = GraphQuery::new("User")
        .with_order(OrderSpec::asc("name"))
        .with_pagination(Pagination::new(5, 0));

    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 5);
    assert!(result.has_more);

    // Second page
    let query = GraphQuery::new("User")
        .with_order(OrderSpec::asc("name"))
        .with_pagination(Pagination::new(5, 5));

    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 5);
    assert!(result.has_more);

    // Last page (only 5 remaining)
    let query = GraphQuery::new("User")
        .with_order(OrderSpec::asc("name"))
        .with_pagination(Pagination::new(5, 15));

    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 5);
    assert!(!result.has_more);
}

#[test]
fn test_field_projection() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("email", Value::String("alice@example.com".to_string())),
            ("age", Value::Int32(30)),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").with_fields(vec!["name".into(), "age".into()]);

    let result = executor.execute(&query).unwrap();

    assert_eq!(result.entities[0].columns.len(), 2);
    assert!(result.entities[0].column("name").is_some());
    assert!(result.entities[0].column("age").is_some());
    assert!(result.entities[0].column("email").is_none());
}

#[test]
fn test_single_level_include() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    let user_id = insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int32(30)),
        ],
    );

    for i in 0..3 {
        insert_entity(
            &ctx,
            "Post",
            vec![
                ("title", Value::String(format!("Post {}", i))),
                ("content", Value::String(format!("Content {}", i))),
                ("author_id", Value::Uuid(user_id)),
                ("views", Value::Int64(100 + i as i64)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").include(RelationInclude::new("posts"));

    let result = executor.execute(&query).unwrap();

    // Should have User and Post blocks
    assert_eq!(result.entities.len(), 2);
    assert_eq!(result.entities[0].entity, "User");
    assert_eq!(result.entities[0].len(), 1);
    assert_eq!(result.entities[1].entity, "Post");
    assert_eq!(result.entities[1].len(), 3);

    // Should have edges
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].relation, "posts");
    assert_eq!(result.edges[0].len(), 3);
}

#[test]
fn test_nested_includes() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    let user_id = insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int32(30)),
        ],
    );

    let post_id = insert_entity(
        &ctx,
        "Post",
        vec![
            ("title", Value::String("Test Post".to_string())),
            ("content", Value::String("Content".to_string())),
            ("author_id", Value::Uuid(user_id)),
            ("views", Value::Int64(100)),
        ],
    );

    for i in 0..2 {
        insert_entity(
            &ctx,
            "Comment",
            vec![
                ("text", Value::String(format!("Comment {}", i))),
                ("post_id", Value::Uuid(post_id)),
                ("author_id", Value::Uuid(user_id)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User")
        .include(RelationInclude::new("posts"))
        .include(RelationInclude::new("posts.comments"));

    let result = executor.execute(&query).unwrap();

    // Should have User, Post, and Comment blocks
    assert_eq!(result.entities.len(), 3);
    assert_eq!(result.entities[0].entity, "User");
    assert_eq!(result.entities[1].entity, "Post");
    assert_eq!(result.entities[2].entity, "Comment");
    assert_eq!(result.entities[2].len(), 2);

    // Should have edges for both relations
    assert_eq!(result.edges.len(), 2);
}

#[test]
fn test_include_with_filter() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    let user_id = insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int32(30)),
        ],
    );

    insert_entity(
        &ctx,
        "Post",
        vec![
            ("title", Value::String("Popular Post".to_string())),
            ("content", Value::String("Content".to_string())),
            ("author_id", Value::Uuid(user_id)),
            ("views", Value::Int64(1000)),
        ],
    );

    insert_entity(
        &ctx,
        "Post",
        vec![
            ("title", Value::String("Unpopular Post".to_string())),
            ("content", Value::String("Content".to_string())),
            ("author_id", Value::Uuid(user_id)),
            ("views", Value::Int64(10)),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").include(
        RelationInclude::new("posts")
            .with_filter(FilterExpr::gt("views", Value::Int64(500)).into()),
    );

    let result = executor.execute(&query).unwrap();

    // Should only include the popular post
    assert_eq!(result.entities[1].len(), 1);
    let title_col = result.entities[1].column("title").unwrap();
    assert_eq!(
        title_col.values[0],
        Value::String("Popular Post".to_string())
    );
}

#[test]
fn test_budget_entity_limit() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    // Insert many users
    for i in 0..100 {
        insert_entity(
            &ctx,
            "User",
            vec![
                ("name", Value::String(format!("User{}", i))),
                ("age", Value::Int32(20 + i)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();

    // Query with a low entity budget
    let budget = FanoutBudget::new(50, 1000, 5);
    let query = GraphQuery::new("User");

    let result = executor.execute_with_budget(&query, budget);

    // Should fail because we have 100 entities but budget is 50
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("budget"));
}

#[test]
fn test_budget_edge_limit() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    let user_id = insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int32(30)),
        ],
    );

    // Insert many posts
    for i in 0..50 {
        insert_entity(
            &ctx,
            "Post",
            vec![
                ("title", Value::String(format!("Post {}", i))),
                ("content", Value::String("Content".to_string())),
                ("author_id", Value::Uuid(user_id)),
                ("views", Value::Int64(100)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();

    // Query with a low edge budget
    let budget = FanoutBudget::new(100, 10, 5);
    let query = GraphQuery::new("User").include(RelationInclude::new("posts"));

    let result = executor.execute_with_budget(&query, budget);

    // Should fail because we have 50 edges but budget is 10
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("budget"));
}

#[test]
fn test_empty_result() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    let executor = ctx.executor();
    let query = GraphQuery::new("User");
    let result = executor.execute(&query).unwrap();

    assert_eq!(result.entities.len(), 1);
    assert!(result.entities[0].is_empty());
    assert!(!result.has_more);
}

#[test]
fn test_filter_in_values() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    for age in [20, 25, 30, 35, 40] {
        insert_entity(
            &ctx,
            "User",
            vec![
                ("name", Value::String(format!("User{}", age))),
                ("age", Value::Int32(age)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").with_filter(
        FilterExpr::in_values(
            "age",
            vec![Value::Int32(25), Value::Int32(30), Value::Int32(40)],
        )
        .into(),
    );

    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 3);
}

#[test]
fn test_filter_like_pattern() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("email", Value::String("alice@example.com".to_string())),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Bob".to_string())),
            ("email", Value::String("bob@test.org".to_string())),
        ],
    );
    insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Charlie".to_string())),
            ("email", Value::String("charlie@example.com".to_string())),
        ],
    );

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();

    // Match emails ending with example.com
    let query = GraphQuery::new("User")
        .with_filter(FilterExpr::like("email", "%@example.com").into());

    let result = executor.execute(&query).unwrap();
    assert_eq!(result.entities[0].len(), 2); // Alice and Charlie
}

#[test]
fn test_multiple_users_with_posts() {
    let ctx = TestContext::new();
    setup_blog_schema(&ctx);

    let alice_id = insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int32(30)),
        ],
    );

    let bob_id = insert_entity(
        &ctx,
        "User",
        vec![
            ("name", Value::String("Bob".to_string())),
            ("age", Value::Int32(25)),
        ],
    );

    // Alice has 3 posts
    for i in 0..3 {
        insert_entity(
            &ctx,
            "Post",
            vec![
                ("title", Value::String(format!("Alice Post {}", i))),
                ("content", Value::String("Content".to_string())),
                ("author_id", Value::Uuid(alice_id)),
                ("views", Value::Int64(100)),
            ],
        );
    }

    // Bob has 2 posts
    for i in 0..2 {
        insert_entity(
            &ctx,
            "Post",
            vec![
                ("title", Value::String(format!("Bob Post {}", i))),
                ("content", Value::String("Content".to_string())),
                ("author_id", Value::Uuid(bob_id)),
                ("views", Value::Int64(50)),
            ],
        );
    }

    ctx.storage.flush().unwrap();

    let executor = ctx.executor();
    let query = GraphQuery::new("User").include(RelationInclude::new("posts"));

    let result = executor.execute(&query).unwrap();

    assert_eq!(result.entities[0].len(), 2); // 2 users
    assert_eq!(result.entities[1].len(), 5); // 5 total posts
    assert_eq!(result.edges[0].len(), 5); // 5 edges
}
