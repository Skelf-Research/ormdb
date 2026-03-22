# Blog Platform Example

A multi-author blog demonstrating complex relations, pagination, and row-level security.

---

## Overview

This example builds a complete blog platform with:
- Users, posts, comments, and tags
- One-to-many and many-to-many relations
- Graph queries for efficient data loading
- Cursor-based pagination
- Row-level security for author permissions

---

## Schema

```ormdb
// schema.ormdb

entity User {
    id: uuid @id @default(uuid())
    username: string @unique
    email: string @unique
    display_name: string
    bio: string?
    avatar_url: string?
    role: string @default("author")  // admin, author, reader
    created_at: timestamp @default(now())

    posts: Post[] @relation(field: author_id)
    comments: Comment[] @relation(field: author_id)

    @index(role)
}

entity Post {
    id: uuid @id @default(uuid())
    title: string
    slug: string @unique
    content: string
    excerpt: string?
    published: bool @default(false)
    featured: bool @default(false)
    view_count: int32 @default(0)
    author_id: uuid
    created_at: timestamp @default(now())
    updated_at: timestamp @default(now())
    published_at: timestamp?

    author: User @relation(field: author_id, references: id)
    comments: Comment[] @relation(field: post_id)
    tags: PostTag[] @relation(field: post_id)

    @index(published)
    @index(author_id)
    @index(created_at)
}

entity Comment {
    id: uuid @id @default(uuid())
    content: string
    post_id: uuid
    author_id: uuid
    parent_id: uuid?  // For nested comments
    created_at: timestamp @default(now())
    updated_at: timestamp @default(now())

    post: Post @relation(field: post_id, references: id)
    author: User @relation(field: author_id, references: id)
    parent: Comment? @relation(field: parent_id, references: id)
    replies: Comment[] @relation(field: parent_id)

    @index(post_id)
    @index(author_id)
}

entity Tag {
    id: uuid @id @default(uuid())
    name: string @unique
    slug: string @unique
    description: string?

    posts: PostTag[] @relation(field: tag_id)
}

// Junction table for many-to-many
entity PostTag {
    id: uuid @id @default(uuid())
    post_id: uuid
    tag_id: uuid

    post: Post @relation(field: post_id, references: id)
    tag: Tag @relation(field: tag_id, references: id)

    @unique([post_id, tag_id])
}

// Row-Level Security
policy AuthorCanEditOwnPosts {
    entity: Post
    action: [update, delete]
    condition: author_id == $user_id OR $user_role == "admin"
}

policy AuthorCanEditOwnComments {
    entity: Comment
    action: [update, delete]
    condition: author_id == $user_id OR $user_role == "admin"
}

policy OnlyPublishedPostsVisible {
    entity: Post
    action: read
    condition: published == true OR author_id == $user_id OR $user_role == "admin"
}
```

---

## Graph Queries

### Fetch Post with Author and Comments

```rust
use ormdb_proto::{GraphQuery, FilterExpr, Value, Include};

// Single query fetches post + author + comments + comment authors
let query = GraphQuery::new("Post")
    .filter(FilterExpr::eq("slug", Value::String("my-first-post".into())))
    .include(Include::new("author").fields(["id", "username", "display_name", "avatar_url"]))
    .include(
        Include::new("comments")
            .filter(FilterExpr::is_null("parent_id", false))  // Top-level only
            .order_by(OrderSpec::desc("created_at"))
            .limit(20)
            .include(Include::new("author").fields(["id", "username", "avatar_url"]))
            .include(
                Include::new("replies")
                    .limit(5)
                    .include(Include::new("author").fields(["id", "username"]))
            )
    )
    .include(
        Include::new("tags")
            .include(Include::new("tag").fields(["id", "name", "slug"]))
    );

let result = db.query(query).await?;
```

This single query replaces what would be 6+ queries in a traditional ORM:
1. Fetch post
2. Fetch author
3. Fetch comments
4. Fetch comment authors
5. Fetch replies
6. Fetch tags

### List Posts with Pagination

```rust
use ormdb_proto::{GraphQuery, FilterExpr, Value, Include, OrderSpec, Pagination};

pub async fn list_posts(
    db: &Database,
    cursor: Option<String>,
    limit: usize,
    tag_slug: Option<&str>,
) -> (Vec<Post>, Option<String>) {
    let mut query = GraphQuery::new("Post")
        .filter(FilterExpr::eq("published", Value::Bool(true)))
        .order_by(OrderSpec::desc("published_at"))
        .include(Include::new("author").fields(["id", "username", "display_name", "avatar_url"]))
        .include(
            Include::new("tags")
                .include(Include::new("tag").fields(["id", "name", "slug"]))
        );

    // Filter by tag if specified
    if let Some(slug) = tag_slug {
        query = query.filter(FilterExpr::exists(
            "tags",
            FilterExpr::field_eq("tag.slug", slug)
        ));
    }

    // Cursor-based pagination
    if let Some(cursor) = cursor {
        let (timestamp, id) = decode_cursor(&cursor);
        query = query.filter(FilterExpr::or(vec![
            FilterExpr::lt("published_at", Value::Timestamp(timestamp)),
            FilterExpr::and(vec![
                FilterExpr::eq("published_at", Value::Timestamp(timestamp)),
                FilterExpr::lt("id", Value::Uuid(id)),
            ]),
        ]));
    }

    query = query.limit(limit + 1);  // Fetch one extra to check for more

    let result = db.query(query).await.unwrap();
    let mut posts: Vec<Post> = result.entities().map(Into::into).collect();

    // Determine next cursor
    let next_cursor = if posts.len() > limit {
        posts.pop();  // Remove extra
        let last = posts.last().unwrap();
        Some(encode_cursor(last.published_at, &last.id))
    } else {
        None
    };

    (posts, next_cursor)
}

fn encode_cursor(timestamp: i64, id: &Uuid) -> String {
    base64::encode(format!("{}:{}", timestamp, id))
}

fn decode_cursor(cursor: &str) -> (i64, Uuid) {
    let decoded = base64::decode(cursor).unwrap();
    let s = String::from_utf8(decoded).unwrap();
    let parts: Vec<&str> = s.split(':').collect();
    (parts[0].parse().unwrap(), parts[1].parse().unwrap())
}
```

---

## Row-Level Security

### Setting Up User Context

```rust
use ormdb_core::{Database, SecurityContext};

pub async fn with_user_context<T>(
    db: &Database,
    user: &User,
    f: impl FnOnce(&Database) -> T,
) -> T {
    let ctx = SecurityContext::new()
        .set("user_id", user.id)
        .set("user_role", &user.role);

    db.with_security_context(ctx, || f(db)).await
}
```

### Using RLS in Handlers

```rust
pub async fn update_post(
    State(db): State<Db>,
    user: AuthenticatedUser,
    Path(post_id): Path<Uuid>,
    Json(input): Json<UpdatePostInput>,
) -> Result<Json<Post>, StatusCode> {
    // RLS automatically enforces that user can only update their own posts
    // (or any post if they're an admin)
    let result = with_user_context(&db, &user, |db| async {
        let mutation = Mutation::update("Post")
            .filter(FilterExpr::eq("id", Value::Uuid(post_id.into_bytes())))
            .set("title", Value::String(input.title))
            .set("content", Value::String(input.content))
            .set("updated_at", Value::Timestamp(Utc::now().timestamp_micros()));

        db.mutate(mutation).await
    }).await;

    match result {
        Ok(_) => {
            let post = get_post(&db, post_id).await?;
            Ok(Json(post))
        }
        Err(OrmdbError::PermissionDenied) => Err(StatusCode::FORBIDDEN),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

---

## Full API Implementation

### Post Handlers

```rust
// src/handlers/posts.rs
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

#[derive(Deserialize)]
pub struct ListPostsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    tag: Option<String>,
}

/// GET /posts
pub async fn list_posts(
    State(db): State<Db>,
    Query(params): Query<ListPostsQuery>,
) -> Json<PostListResponse> {
    let limit = params.limit.unwrap_or(10).min(50);
    let (posts, next_cursor) = queries::list_posts(
        &db,
        params.cursor,
        limit,
        params.tag.as_deref(),
    ).await;

    Json(PostListResponse { posts, next_cursor })
}

/// GET /posts/:slug
pub async fn get_post(
    State(db): State<Db>,
    Path(slug): Path<String>,
) -> Result<Json<PostDetail>, StatusCode> {
    // Increment view count
    db.mutate(
        Mutation::update("Post")
            .filter(FilterExpr::eq("slug", Value::String(slug.clone())))
            .increment("view_count", 1)
    ).await.ok();

    // Fetch post with all relations
    let query = GraphQuery::new("Post")
        .filter(FilterExpr::eq("slug", Value::String(slug)))
        .include(Include::new("author").fields(["id", "username", "display_name", "avatar_url", "bio"]))
        .include(
            Include::new("comments")
                .filter(FilterExpr::is_null("parent_id"))
                .order_by(OrderSpec::desc("created_at"))
                .limit(50)
                .include(Include::new("author").fields(["id", "username", "avatar_url"]))
                .include(Include::new("replies").limit(10).include(Include::new("author")))
        )
        .include(Include::new("tags").include(Include::new("tag")));

    let result = db.query(query).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    result.entities()
        .next()
        .map(|e| Json(PostDetail::from(e)))
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /posts
pub async fn create_post(
    State(db): State<Db>,
    user: AuthenticatedUser,
    Json(input): Json<CreatePostInput>,
) -> Result<(StatusCode, Json<Post>), StatusCode> {
    let id = Uuid::new_v4();
    let slug = slugify(&input.title);

    let mutation = Mutation::create("Post")
        .set("id", Value::Uuid(id.into_bytes()))
        .set("title", Value::String(input.title))
        .set("slug", Value::String(slug))
        .set("content", Value::String(input.content))
        .set_opt("excerpt", input.excerpt.map(Value::String))
        .set("author_id", Value::Uuid(user.id.into_bytes()));

    db.mutate(mutation).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Add tags
    if let Some(tag_ids) = input.tag_ids {
        for tag_id in tag_ids {
            db.mutate(
                Mutation::create("PostTag")
                    .set("id", Value::Uuid(Uuid::new_v4().into_bytes()))
                    .set("post_id", Value::Uuid(id.into_bytes()))
                    .set("tag_id", Value::Uuid(tag_id.into_bytes()))
            ).await.ok();
        }
    }

    let post = queries::get_post_by_id(&db, id).await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(post)))
}

/// PUT /posts/:id/publish
pub async fn publish_post(
    State(db): State<Db>,
    user: AuthenticatedUser,
    Path(post_id): Path<Uuid>,
) -> Result<Json<Post>, StatusCode> {
    with_user_context(&db, &user, |db| async {
        db.mutate(
            Mutation::update("Post")
                .filter(FilterExpr::eq("id", Value::Uuid(post_id.into_bytes())))
                .set("published", Value::Bool(true))
                .set("published_at", Value::Timestamp(Utc::now().timestamp_micros()))
        ).await
    }).await.map_err(|_| StatusCode::FORBIDDEN)?;

    let post = queries::get_post_by_id(&db, post_id).await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(post))
}
```

### Comment Handlers

```rust
// src/handlers/comments.rs

/// POST /posts/:post_id/comments
pub async fn create_comment(
    State(db): State<Db>,
    user: AuthenticatedUser,
    Path(post_id): Path<Uuid>,
    Json(input): Json<CreateCommentInput>,
) -> Result<(StatusCode, Json<Comment>), StatusCode> {
    // Verify post exists and is published
    let post = queries::get_post_by_id(&db, post_id).await
        .ok_or(StatusCode::NOT_FOUND)?;

    if !post.published {
        return Err(StatusCode::FORBIDDEN);
    }

    let id = Uuid::new_v4();

    let mutation = Mutation::create("Comment")
        .set("id", Value::Uuid(id.into_bytes()))
        .set("content", Value::String(input.content))
        .set("post_id", Value::Uuid(post_id.into_bytes()))
        .set("author_id", Value::Uuid(user.id.into_bytes()))
        .set_opt("parent_id", input.parent_id.map(|id| Value::Uuid(id.into_bytes())));

    db.mutate(mutation).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let comment = queries::get_comment(&db, id).await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(comment)))
}

/// DELETE /comments/:id
pub async fn delete_comment(
    State(db): State<Db>,
    user: AuthenticatedUser,
    Path(comment_id): Path<Uuid>,
) -> StatusCode {
    let result = with_user_context(&db, &user, |db| async {
        db.mutate(
            Mutation::delete("Comment")
                .filter(FilterExpr::eq("id", Value::Uuid(comment_id.into_bytes())))
        ).await
    }).await;

    match result {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(OrmdbError::PermissionDenied) => StatusCode::FORBIDDEN,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
```

---

## TypeScript Frontend

### API Client

```typescript
// frontend/src/api/client.ts
import { OrmdbClient } from '@ormdb/client';

const client = new OrmdbClient({
  host: 'localhost',
  port: 8080,
});

export interface Post {
  id: string;
  title: string;
  slug: string;
  content: string;
  excerpt?: string;
  published: boolean;
  viewCount: number;
  publishedAt?: Date;
  author: User;
  comments: Comment[];
  tags: Tag[];
}

export interface PostListResponse {
  posts: Post[];
  nextCursor?: string;
}

export async function listPosts(params: {
  cursor?: string;
  limit?: number;
  tag?: string;
}): Promise<PostListResponse> {
  const query = new GraphQuery('Post')
    .filter({ published: true })
    .orderBy('publishedAt', 'desc')
    .include('author', { fields: ['id', 'username', 'displayName', 'avatarUrl'] })
    .include('tags', { include: { tag: { fields: ['id', 'name', 'slug'] } } })
    .limit(params.limit || 10);

  if (params.cursor) {
    // Apply cursor filter
  }

  if (params.tag) {
    query.filter({ 'tags.tag.slug': params.tag });
  }

  return client.query(query);
}

export async function getPost(slug: string): Promise<Post | null> {
  const result = await client.query(
    new GraphQuery('Post')
      .filter({ slug })
      .include('author')
      .include('comments', {
        filter: { parentId: null },
        orderBy: { createdAt: 'desc' },
        limit: 50,
        include: {
          author: true,
          replies: { limit: 10, include: { author: true } },
        },
      })
      .include('tags', { include: { tag: true } })
  );

  return result.first();
}
```

### React Components

```tsx
// frontend/src/components/PostList.tsx
import { useInfiniteQuery } from '@tanstack/react-query';
import { listPosts } from '../api/client';

export function PostList({ tag }: { tag?: string }) {
  const {
    data,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useInfiniteQuery({
    queryKey: ['posts', tag],
    queryFn: ({ pageParam }) => listPosts({ cursor: pageParam, tag }),
    getNextPageParam: (lastPage) => lastPage.nextCursor,
  });

  const posts = data?.pages.flatMap(page => page.posts) ?? [];

  return (
    <div className="post-list">
      {posts.map(post => (
        <PostCard key={post.id} post={post} />
      ))}

      {hasNextPage && (
        <button
          onClick={() => fetchNextPage()}
          disabled={isFetchingNextPage}
        >
          {isFetchingNextPage ? 'Loading...' : 'Load More'}
        </button>
      )}
    </div>
  );
}

function PostCard({ post }: { post: Post }) {
  return (
    <article className="post-card">
      <h2>
        <Link to={`/posts/${post.slug}`}>{post.title}</Link>
      </h2>
      <p className="excerpt">{post.excerpt}</p>
      <div className="meta">
        <img src={post.author.avatarUrl} alt="" />
        <span>{post.author.displayName}</span>
        <time>{formatDate(post.publishedAt)}</time>
      </div>
      <div className="tags">
        {post.tags.map(({ tag }) => (
          <Link key={tag.id} to={`/tags/${tag.slug}`}>{tag.name}</Link>
        ))}
      </div>
    </article>
  );
}
```

---

## Key Takeaways

1. **Graph queries eliminate N+1** - Fetch posts with authors, comments, and tags in one query
2. **Cursor pagination scales** - Works efficiently with large datasets
3. **RLS simplifies authorization** - Declare policies in schema, enforced automatically
4. **Relations are first-class** - Define once in schema, use everywhere
5. **Type safety throughout** - From schema to Rust to TypeScript

---

## Next Steps

- Add full-text search with [Performance Guide](../guides/performance.md)
- Implement webhooks with [CDC Guide](../guides/cdc.md)
- Scale to multiple tenants with [Multi-Tenant SaaS](multi-tenant-saas.md)

