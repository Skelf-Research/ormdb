# Todo App Example

A classic todo list application demonstrating ORMDB fundamentals.

---

## Overview

This example builds a complete todo list with:
- Create, read, update, delete operations
- Filtering by status
- Sorting by date/priority
- Simple REST API

---

## Schema

```ormdb
// schema.ormdb
entity Todo {
    id: uuid @id @default(uuid())
    title: string
    description: string?
    completed: bool @default(false)
    priority: int32 @default(0)
    due_date: timestamp?
    created_at: timestamp @default(now())
    updated_at: timestamp @default(now())

    @index(completed)
    @index(priority)
}
```

---

## Project Setup

### Cargo.toml

```toml
[package]
name = "todo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
ormdb-core = "0.1"
ormdb-proto = "0.1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
axum = "0.7"
```

---

## Database Setup

```rust
// src/db.rs
use ormdb_core::{Database, StorageConfig};
use std::sync::Arc;

pub type Db = Arc<Database>;

pub async fn init_database() -> Db {
    let config = StorageConfig::default()
        .path("./data/todos.db");

    let db = Database::open(config)
        .await
        .expect("Failed to open database");

    // Apply schema
    db.apply_schema(include_str!("../schema.ormdb"))
        .await
        .expect("Failed to apply schema");

    Arc::new(db)
}
```

---

## Models

```rust
// src/models.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    pub priority: i32,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTodo {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTodo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub completed: Option<bool>,
    pub priority: Option<i32>,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct TodoFilter {
    pub completed: Option<bool>,
    pub priority_gte: Option<i32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
```

---

## Query Functions

```rust
// src/queries.rs
use crate::db::Db;
use crate::models::{CreateTodo, Todo, TodoFilter, UpdateTodo};
use ormdb_proto::{FilterExpr, GraphQuery, Mutation, OrderSpec, Value};
use uuid::Uuid;

/// List all todos with optional filtering
pub async fn list_todos(db: &Db, filter: TodoFilter) -> Vec<Todo> {
    let mut query = GraphQuery::new("Todo");

    // Apply filters
    if let Some(completed) = filter.completed {
        query = query.filter(FilterExpr::eq("completed", Value::Bool(completed)));
    }

    if let Some(priority) = filter.priority_gte {
        query = query.filter(FilterExpr::gte("priority", Value::Int32(priority)));
    }

    // Apply sorting
    if let Some(sort_by) = filter.sort_by {
        let direction = match filter.sort_order.as_deref() {
            Some("desc") => ormdb_proto::SortDirection::Desc,
            _ => ormdb_proto::SortDirection::Asc,
        };
        query = query.order_by(OrderSpec::new(&sort_by, direction));
    } else {
        // Default: newest first
        query = query.order_by(OrderSpec::desc("created_at"));
    }

    let result = db.query(query).await.expect("Query failed");
    result.entities().map(|e| e.into()).collect()
}

/// Get a single todo by ID
pub async fn get_todo(db: &Db, id: Uuid) -> Option<Todo> {
    let query = GraphQuery::new("Todo")
        .filter(FilterExpr::eq("id", Value::Uuid(id.into_bytes())));

    let result = db.query(query).await.expect("Query failed");
    result.entities().next().map(|e| e.into())
}

/// Create a new todo
pub async fn create_todo(db: &Db, input: CreateTodo) -> Todo {
    let id = Uuid::new_v4();

    let mutation = Mutation::create("Todo")
        .set("id", Value::Uuid(id.into_bytes()))
        .set("title", Value::String(input.title))
        .set_opt("description", input.description.map(Value::String))
        .set("priority", Value::Int32(input.priority.unwrap_or(0)))
        .set_opt("due_date", input.due_date.map(|d| Value::Timestamp(d.timestamp_micros())));

    db.mutate(mutation).await.expect("Mutation failed");

    get_todo(db, id).await.expect("Todo not found after create")
}

/// Update an existing todo
pub async fn update_todo(db: &Db, id: Uuid, input: UpdateTodo) -> Option<Todo> {
    // Check if exists
    if get_todo(db, id).await.is_none() {
        return None;
    }

    let mut mutation = Mutation::update("Todo")
        .filter(FilterExpr::eq("id", Value::Uuid(id.into_bytes())));

    if let Some(title) = input.title {
        mutation = mutation.set("title", Value::String(title));
    }
    if let Some(description) = input.description {
        mutation = mutation.set("description", Value::String(description));
    }
    if let Some(completed) = input.completed {
        mutation = mutation.set("completed", Value::Bool(completed));
    }
    if let Some(priority) = input.priority {
        mutation = mutation.set("priority", Value::Int32(priority));
    }
    if let Some(due_date) = input.due_date {
        mutation = mutation.set("due_date", Value::Timestamp(due_date.timestamp_micros()));
    }

    // Always update updated_at
    mutation = mutation.set("updated_at", Value::Timestamp(chrono::Utc::now().timestamp_micros()));

    db.mutate(mutation).await.expect("Mutation failed");

    get_todo(db, id).await
}

/// Delete a todo
pub async fn delete_todo(db: &Db, id: Uuid) -> bool {
    if get_todo(db, id).await.is_none() {
        return false;
    }

    let mutation = Mutation::delete("Todo")
        .filter(FilterExpr::eq("id", Value::Uuid(id.into_bytes())));

    db.mutate(mutation).await.expect("Mutation failed");
    true
}

/// Toggle todo completion status
pub async fn toggle_todo(db: &Db, id: Uuid) -> Option<Todo> {
    let todo = get_todo(db, id).await?;

    let mutation = Mutation::update("Todo")
        .filter(FilterExpr::eq("id", Value::Uuid(id.into_bytes())))
        .set("completed", Value::Bool(!todo.completed))
        .set("updated_at", Value::Timestamp(chrono::Utc::now().timestamp_micros()));

    db.mutate(mutation).await.expect("Mutation failed");

    get_todo(db, id).await
}

/// Get todo statistics
pub async fn get_stats(db: &Db) -> TodoStats {
    let all = list_todos(db, TodoFilter::default()).await;

    TodoStats {
        total: all.len(),
        completed: all.iter().filter(|t| t.completed).count(),
        pending: all.iter().filter(|t| !t.completed).count(),
        high_priority: all.iter().filter(|t| t.priority >= 5).count(),
    }
}

#[derive(Debug, Serialize)]
pub struct TodoStats {
    pub total: usize,
    pub completed: usize,
    pub pending: usize,
    pub high_priority: usize,
}
```

---

## HTTP Handlers

```rust
// src/handlers.rs
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::db::Db;
use crate::models::{CreateTodo, Todo, TodoFilter, UpdateTodo};
use crate::queries::{self, TodoStats};

/// GET /todos
pub async fn list_todos(
    State(db): State<Db>,
    Query(filter): Query<TodoFilter>,
) -> Json<Vec<Todo>> {
    let todos = queries::list_todos(&db, filter).await;
    Json(todos)
}

/// GET /todos/:id
pub async fn get_todo(
    State(db): State<Db>,
    Path(id): Path<Uuid>,
) -> Result<Json<Todo>, StatusCode> {
    queries::get_todo(&db, id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /todos
pub async fn create_todo(
    State(db): State<Db>,
    Json(input): Json<CreateTodo>,
) -> (StatusCode, Json<Todo>) {
    let todo = queries::create_todo(&db, input).await;
    (StatusCode::CREATED, Json(todo))
}

/// PUT /todos/:id
pub async fn update_todo(
    State(db): State<Db>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTodo>,
) -> Result<Json<Todo>, StatusCode> {
    queries::update_todo(&db, id, input)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// DELETE /todos/:id
pub async fn delete_todo(
    State(db): State<Db>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    if queries::delete_todo(&db, id).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// POST /todos/:id/toggle
pub async fn toggle_todo(
    State(db): State<Db>,
    Path(id): Path<Uuid>,
) -> Result<Json<Todo>, StatusCode> {
    queries::toggle_todo(&db, id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// GET /todos/stats
pub async fn get_stats(State(db): State<Db>) -> Json<TodoStats> {
    let stats = queries::get_stats(&db).await;
    Json(stats)
}
```

---

## Main Application

```rust
// src/main.rs
mod db;
mod handlers;
mod models;
mod queries;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Initialize database
    let db = db::init_database().await;

    // Build router
    let app = Router::new()
        .route("/todos", get(handlers::list_todos).post(handlers::create_todo))
        .route("/todos/stats", get(handlers::get_stats))
        .route(
            "/todos/:id",
            get(handlers::get_todo)
                .put(handlers::update_todo)
                .delete(handlers::delete_todo),
        )
        .route("/todos/:id/toggle", post(handlers::toggle_todo))
        .with_state(db);

    // Run server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

---

## API Usage

### Create a Todo

```bash
curl -X POST http://localhost:3000/todos \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Learn ORMDB",
    "description": "Read the documentation",
    "priority": 5
  }'
```

### List All Todos

```bash
# All todos
curl http://localhost:3000/todos

# Only pending
curl "http://localhost:3000/todos?completed=false"

# High priority, sorted
curl "http://localhost:3000/todos?priority_gte=5&sort_by=priority&sort_order=desc"
```

### Update a Todo

```bash
curl -X PUT http://localhost:3000/todos/550e8400-e29b-41d4-a716-446655440000 \
  -H "Content-Type: application/json" \
  -d '{"completed": true}'
```

### Toggle Completion

```bash
curl -X POST http://localhost:3000/todos/550e8400-e29b-41d4-a716-446655440000/toggle
```

### Delete a Todo

```bash
curl -X DELETE http://localhost:3000/todos/550e8400-e29b-41d4-a716-446655440000
```

### Get Statistics

```bash
curl http://localhost:3000/todos/stats
```

Response:
```json
{
  "total": 10,
  "completed": 3,
  "pending": 7,
  "high_priority": 2
}
```

---

## Key Takeaways

1. **Schema-first design** - Define your data model in `.ormdb` files
2. **Type-safe queries** - Use `GraphQuery` and `FilterExpr` instead of SQL strings
3. **Automatic indexing** - Declare `@index` for frequently filtered fields
4. **Simple CRUD** - `Mutation::create`, `update`, `delete` cover all basics
5. **Flexible filtering** - Combine filters with AND/OR logic

---

## Next Steps

- Add authentication with [Security](../guides/security.md)
- Implement pagination with [Pagination Guide](../guides/pagination.md)
- Try the more complex [Blog Platform](blog-platform.md) example

