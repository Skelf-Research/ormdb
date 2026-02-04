# Django Adapter

Use ORMDB with Django's built-in ORM.

---

## Installation

```bash
pip install django-ormdb
```

---

## Setup

### 1. Configure Database

```python
# settings.py
DATABASES = {
    'default': {
        'ENGINE': 'django_ormdb',
        'HOST': 'localhost',
        'PORT': 8080,
        'OPTIONS': {
            'pool_size': 10,
        },
    }
}
```

### 2. Define Models

```python
# models.py
from django.db import models
import uuid

class User(models.Model):
    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)
    name = models.CharField(max_length=255)
    email = models.EmailField(unique=True)
    status = models.CharField(max_length=50, default='active')
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = 'users'


class Post(models.Model):
    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)
    title = models.CharField(max_length=255)
    content = models.TextField(blank=True, null=True)
    published = models.BooleanField(default=False)
    author = models.ForeignKey(User, on_delete=models.CASCADE, related_name='posts')
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = 'posts'


class Comment(models.Model):
    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)
    text = models.TextField()
    post = models.ForeignKey(Post, on_delete=models.CASCADE, related_name='comments')
    author = models.ForeignKey(User, on_delete=models.CASCADE)

    class Meta:
        db_table = 'comments'
```

### 3. Sync Schema

```bash
python manage.py migrate
```

---

## Basic Queries

### All Objects

```python
users = User.objects.all()
```

### Filtering

```python
# Equality
active_users = User.objects.filter(status='active')

# Not equal
non_banned = User.objects.exclude(status='banned')

# Greater than
adults = User.objects.filter(age__gt=18)

# Greater than or equal
adults = User.objects.filter(age__gte=18)

# Less than
young = User.objects.filter(age__lt=30)

# Contains
gmail = User.objects.filter(email__contains='@gmail')

# Starts with
admins = User.objects.filter(email__startswith='admin')

# Ends with
example = User.objects.filter(email__endswith='@example.com')

# IN
active_or_pending = User.objects.filter(status__in=['active', 'pending'])

# IS NULL
no_bio = User.objects.filter(bio__isnull=True)

# IS NOT NULL
has_bio = User.objects.filter(bio__isnull=False)
```

### Compound Filters

```python
from django.db.models import Q

# AND (chaining)
active_adults = User.objects.filter(status='active').filter(age__gte=18)

# AND (Q object)
active_adults = User.objects.filter(Q(status='active') & Q(age__gte=18))

# OR
admins_or_mods = User.objects.filter(Q(role='admin') | Q(role='moderator'))

# NOT
not_banned = User.objects.filter(~Q(status='banned'))
```

### Ordering

```python
# Ascending
users = User.objects.order_by('name')

# Descending
users = User.objects.order_by('-created_at')

# Multiple
users = User.objects.order_by('status', 'name')
```

### Pagination

```python
users = User.objects.all()[20:30]  # Skip 20, take 10
```

### Get Single Object

```python
# Get or raise DoesNotExist
user = User.objects.get(id=user_id)

# Get or None
user = User.objects.filter(email=email).first()

# Get or create
user, created = User.objects.get_or_create(
    email=email,
    defaults={'name': 'New User'}
)
```

---

## Relations

### Select Related (Foreign Keys)

```python
# Single relation
posts = Post.objects.select_related('author').all()

# Multiple
posts = Post.objects.select_related('author', 'category').all()

# Access without extra query
for post in posts:
    print(post.author.name)
```

### Prefetch Related (Reverse/Many)

```python
# Reverse relation
users = User.objects.prefetch_related('posts').all()

# Nested
users = User.objects.prefetch_related('posts__comments').all()

# Access without N+1
for user in users:
    for post in user.posts.all():
        print(post.title)
```

### Custom Prefetch

```python
from django.db.models import Prefetch

# With filter
users = User.objects.prefetch_related(
    Prefetch(
        'posts',
        queryset=Post.objects.filter(published=True).order_by('-created_at')[:5]
    )
).all()
```

### Filtering on Relations

```python
# Users with published posts
users = User.objects.filter(posts__published=True).distinct()

# Posts by active users
posts = Post.objects.filter(author__status='active')
```

---

## Values and Annotations

### Select Specific Fields

```python
# Returns dictionaries
users = User.objects.values('id', 'name')

# Returns tuples
users = User.objects.values_list('id', 'name')

# Flat list of single field
emails = User.objects.values_list('email', flat=True)
```

### Only/Defer

```python
# Only load specific fields
users = User.objects.only('id', 'name')

# Exclude specific fields
users = User.objects.defer('bio', 'avatar')
```

---

## Mutations

### Create

```python
# Create and save
user = User.objects.create(name='Alice', email='alice@example.com')

# Create instance then save
user = User(name='Bob', email='bob@example.com')
user.save()

# Bulk create
users = User.objects.bulk_create([
    User(name='Alice', email='alice@example.com'),
    User(name='Bob', email='bob@example.com'),
])
```

### Update

```python
# Update single
user = User.objects.get(id=user_id)
user.name = 'Alice Smith'
user.save()

# Update specific fields only
user.save(update_fields=['name'])

# Bulk update
User.objects.filter(status='pending').update(status='active')

# Update or create
user, created = User.objects.update_or_create(
    email=email,
    defaults={'name': 'Updated Name'}
)
```

### Delete

```python
# Delete single
user = User.objects.get(id=user_id)
user.delete()

# Bulk delete
User.objects.filter(status='banned').delete()
```

---

## Aggregations

```python
from django.db.models import Count, Sum, Avg, Min, Max

# Count
count = User.objects.filter(status='active').count()

# Aggregate
stats = Post.objects.aggregate(
    total_views=Sum('views'),
    avg_views=Avg('views'),
    min_views=Min('views'),
    max_views=Max('views'),
)

# Annotate (per object)
users = User.objects.annotate(post_count=Count('posts'))
for user in users:
    print(f"{user.name}: {user.post_count} posts")
```

### Group By

```python
# Group by field
stats = User.objects.values('status').annotate(count=Count('id'))
# [{'status': 'active', 'count': 50}, {'status': 'inactive', 'count': 10}]
```

---

## Transactions

```python
from django.db import transaction

# Atomic block
with transaction.atomic():
    user = User.objects.create(name='Alice', email='alice@example.com')
    Post.objects.create(title='Welcome', author=user)
    # Both committed or both rolled back

# Decorator
@transaction.atomic
def create_user_with_post(name, email, title):
    user = User.objects.create(name=name, email=email)
    Post.objects.create(title=title, author=user)
    return user

# Savepoint
with transaction.atomic():
    user = User.objects.create(name='Alice', email='alice@example.com')

    sid = transaction.savepoint()
    try:
        # Risky operation
        Post.objects.create(title='Risky', author=user)
    except:
        transaction.savepoint_rollback(sid)

    # User is still saved
```

---

## Managers

```python
class ActiveManager(models.Manager):
    def get_queryset(self):
        return super().get_queryset().filter(status='active')


class User(models.Model):
    # ... fields

    objects = models.Manager()  # Default
    active = ActiveManager()  # Custom

# Usage
active_users = User.active.all()
```

---

## ORMDB-Specific Features

### Access Native Client

```python
from django_ormdb import get_ormdb_client

ormdb = get_ormdb_client()

# Use native ORMDB features
result = ormdb.query(GraphQuery(...))
```

### Query Budget

```python
from django_ormdb import with_budget

# Set budget for query
users = User.objects.with_budget(max_entities=100).prefetch_related('posts')
```

### Include Soft-Deleted

```python
from django_ormdb import include_deleted

# ORMDB soft deletes by default
users = User.objects.include_deleted().all()
```

---

## Django Admin

The adapter integrates with Django admin:

```python
# admin.py
from django.contrib import admin
from .models import User, Post

@admin.register(User)
class UserAdmin(admin.ModelAdmin):
    list_display = ['id', 'name', 'email', 'status', 'created_at']
    list_filter = ['status']
    search_fields = ['name', 'email']


@admin.register(Post)
class PostAdmin(admin.ModelAdmin):
    list_display = ['id', 'title', 'author', 'published', 'created_at']
    list_filter = ['published']
    raw_id_fields = ['author']
```

---

## Migration from PostgreSQL

### 1. Update Database Settings

```python
# Before
DATABASES = {
    'default': {
        'ENGINE': 'django.db.backends.postgresql',
        'NAME': 'mydb',
        'USER': 'myuser',
        'PASSWORD': 'mypassword',
        'HOST': 'localhost',
        'PORT': '5432',
    }
}

# After
DATABASES = {
    'default': {
        'ENGINE': 'django_ormdb',
        'HOST': 'localhost',
        'PORT': 8080,
    }
}
```

### 2. Run Migrations

```bash
python manage.py migrate
```

### 3. Import Data

```bash
# Export from PostgreSQL
python manage.py dumpdata --format=json > data.json

# Import to ORMDB
python manage.py loaddata data.json
```

---

## Limitations

| Feature | Status | Notes |
|---------|--------|-------|
| Raw SQL | Not supported | Use native client |
| `raw()` | Not supported | |
| `extra()` | Not supported | |
| Database functions | Partial | Basic support |
| F expressions | Partial | Basic support |
| Subqueries | Partial | Basic support |
| Window functions | Not supported | |

---

## Next Steps

- **[SQLAlchemy Adapter](sqlalchemy.md)** - Alternative Python ORM
- **[Python Client](../clients/python.md)** - Direct ORMDB access
- **[Migration Guide](../guides/schema-migrations.md)** - Schema management
