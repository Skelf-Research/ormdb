# Search

Advanced search capabilities including vector similarity, geographic, and full-text search.

## Overview

ORMDB provides three types of advanced search beyond standard filters:

| Search Type | Index | Use Case |
|-------------|-------|----------|
| Vector Search | HNSW | Semantic similarity, embeddings, recommendations |
| Geo Search | R-tree | Location-based queries, proximity search |
| Full-Text Search | Inverted Index | Text content search, document retrieval |

---

## Vector Search

Find similar items using HNSW (Hierarchical Navigable Small World) k-nearest neighbor search.

### Basic Usage

=== "Rust"

    ```rust
    use ormdb_proto::{GraphQuery, FilterExpr};

    // Find 10 most similar products by embedding
    let query = GraphQuery::new("Product")
        .with_filter(FilterExpr::vector_nearest_neighbor(
            "embedding",
            query_vector,
            10,  // k nearest neighbors
        ));
    ```

=== "TypeScript"

    ```typescript
    // Find similar products
    const similar = await client.vectorSearch(
      "Product",
      "embedding",
      [0.1, 0.2, 0.3, ...],  // query vector
      10,                     // k nearest neighbors
      { maxDistance: 0.5 }    // optional distance threshold
    );

    console.log(`Found ${similar.entities.length} similar products`);
    ```

=== "Python"

    ```python
    # Find similar products
    similar = client.vector_search(
        "Product",
        "embedding",
        query_vector=[0.1, 0.2, 0.3, ...],
        k=10,
        max_distance=0.5,  # optional
    )

    print(f"Found {len(similar.entities)} similar products")
    ```

### With Distance Threshold

Limit results to vectors within a maximum distance:

=== "TypeScript"

    ```typescript
    const results = await client.vectorSearch(
      "Product",
      "embedding",
      queryVector,
      100,  // search up to 100 neighbors
      { maxDistance: 0.3 }  // but only return those within distance 0.3
    );
    ```

=== "Python"

    ```python
    results = client.vector_search(
        "Product",
        "embedding",
        query_vector=query_vector,
        k=100,
        max_distance=0.3,
    )
    ```

### Using SearchFilter Types

=== "TypeScript"

    ```typescript
    import { VectorSearchFilter } from "@ormdb/client";

    const filter: VectorSearchFilter = {
      vector_nearest_neighbor: {
        field: "embedding",
        query_vector: queryVector,
        k: 10,
        max_distance: 0.5,
      },
    };

    const results = await client.query("Product", { filter });
    ```

=== "Python"

    ```python
    from ormdb.types import VectorSearchFilter

    filter = VectorSearchFilter(
        field="embedding",
        query_vector=query_vector,
        k=10,
        max_distance=0.5,
    )

    results = client.search("Product", filter)
    ```

---

## Geographic Search

Find entities based on geographic location using R-tree spatial indexing.

### Radius Search

Find entities within a radius from a center point:

=== "Rust"

    ```rust
    // Find restaurants within 5km of San Francisco
    let query = GraphQuery::new("Restaurant")
        .with_filter(FilterExpr::geo_within_radius(
            "location",
            37.7749,   // latitude
            -122.4194, // longitude
            5.0,       // radius in km
        ));
    ```

=== "TypeScript"

    ```typescript
    // Find restaurants within 5km
    const nearby = await client.geoSearch(
      "Restaurant",
      "location",
      37.7749,   // latitude
      -122.4194, // longitude
      5.0        // radius in km
    );
    ```

=== "Python"

    ```python
    # Find restaurants within 5km
    nearby = client.geo_search(
        "Restaurant",
        "location",
        center_lat=37.7749,
        center_lon=-122.4194,
        radius_km=5.0,
    )
    ```

### Bounding Box Search

Find entities within a rectangular region:

=== "TypeScript"

    ```typescript
    // Find restaurants in San Francisco area
    const results = await client.geoBoxSearch(
      "Restaurant",
      "location",
      37.7,    // min latitude
      -122.5,  // min longitude
      37.85,   // max latitude
      -122.35  // max longitude
    );
    ```

=== "Python"

    ```python
    # Find restaurants in bounding box
    results = client.geo_box_search(
        "Restaurant",
        "location",
        min_lat=37.7,
        min_lon=-122.5,
        max_lat=37.85,
        max_lon=-122.35,
    )
    ```

### Polygon Search

Find entities within an arbitrary polygon:

=== "TypeScript"

    ```typescript
    // Find restaurants within a polygon
    const vertices: [number, number][] = [
      [37.7, -122.5],
      [37.8, -122.5],
      [37.85, -122.4],
      [37.75, -122.35],
    ];

    const results = await client.geoPolygonSearch(
      "Restaurant",
      "location",
      vertices
    );
    ```

=== "Python"

    ```python
    # Find restaurants within a polygon
    vertices = [
        (37.7, -122.5),
        (37.8, -122.5),
        (37.85, -122.4),
        (37.75, -122.35),
    ]

    results = client.geo_polygon_search(
        "Restaurant",
        "location",
        vertices=vertices,
    )
    ```

### K-Nearest Geographic Search

Find the k nearest entities to a point:

=== "TypeScript"

    ```typescript
    // Find 10 closest restaurants
    const closest = await client.geoNearest(
      "Restaurant",
      "location",
      37.7749,   // latitude
      -122.4194, // longitude
      10         // k nearest
    );
    ```

=== "Python"

    ```python
    # Find 10 closest restaurants
    closest = client.geo_nearest(
        "Restaurant",
        "location",
        center_lat=37.7749,
        center_lon=-122.4194,
        k=10,
    )
    ```

---

## Full-Text Search

Search text content using BM25 ranking with an inverted index.

### Basic Text Search

Find documents matching search terms:

=== "Rust"

    ```rust
    // Search articles for "rust programming"
    let query = GraphQuery::new("Article")
        .with_filter(FilterExpr::text_match(
            "content",
            "rust programming",
        ));
    ```

=== "TypeScript"

    ```typescript
    // Search articles
    const results = await client.textSearch(
      "Article",
      "content",
      "rust programming"
    );

    console.log(`Found ${results.entities.length} matching articles`);
    ```

=== "Python"

    ```python
    # Search articles
    results = client.text_search(
        "Article",
        "content",
        "rust programming",
    )

    print(f"Found {len(results.entities)} matching articles")
    ```

### With Minimum Score

Filter results by relevance score:

=== "TypeScript"

    ```typescript
    const results = await client.textSearch(
      "Article",
      "content",
      "rust programming",
      { minScore: 0.5 }  // only results with score >= 0.5
    );
    ```

=== "Python"

    ```python
    results = client.text_search(
        "Article",
        "content",
        "rust programming",
        min_score=0.5,
    )
    ```

### Phrase Search

Search for exact phrases:

=== "TypeScript"

    ```typescript
    // Find articles containing exact phrase
    const results = await client.textPhraseSearch(
      "Article",
      "content",
      "quick brown fox"
    );
    ```

=== "Python"

    ```python
    # Find articles containing exact phrase
    results = client.text_phrase_search(
        "Article",
        "content",
        "quick brown fox",
    )
    ```

### Boolean Search

Advanced search with must/should/must_not terms:

=== "TypeScript"

    ```typescript
    // Complex boolean search
    const results = await client.textBooleanSearch(
      "Article",
      "content",
      {
        must: ["rust"],           // must contain "rust"
        should: ["performance", "safety"],  // preferably these too
        mustNot: ["deprecated"],  // must not contain "deprecated"
      }
    );
    ```

=== "Python"

    ```python
    # Complex boolean search
    results = client.text_boolean_search(
        "Article",
        "content",
        must=["rust"],
        should=["performance", "safety"],
        must_not=["deprecated"],
    )
    ```

---

## Combining Search with Other Filters

Search filters can be combined with regular filters using AND:

=== "TypeScript"

    ```typescript
    // Find nearby active restaurants
    const results = await client.query("Restaurant", {
      filter: {
        and: [
          {
            geo_within_radius: {
              field: "location",
              center_lat: 37.7749,
              center_lon: -122.4194,
              radius_km: 5.0,
            },
          },
          { field: "status", op: "eq", value: "active" },
          { field: "rating", op: "ge", value: 4.0 },
        ],
      },
    });
    ```

=== "Python"

    ```python
    # Find nearby active restaurants
    results = client.query("Restaurant",
        filter={
            "and": [
                {
                    "geo_within_radius": {
                        "field": "location",
                        "center_lat": 37.7749,
                        "center_lon": -122.4194,
                        "radius_km": 5.0,
                    }
                },
                {"field": "status", "op": "eq", "value": "active"},
                {"field": "rating", "op": "ge", "value": 4.0},
            ]
        })
    ```

---

## ORM Adapter Examples

### Prisma

```typescript
// Vector search
const similar = await prisma.product.findMany({
  where: {
    embedding: { vectorSearch: { queryVector: [...], k: 10 } },
  },
});

// Geo search
const nearby = await prisma.restaurant.findMany({
  where: {
    location: { geoRadius: { lat: 37.7749, lon: -122.4194, radiusKm: 5 } },
  },
});

// Text search
const articles = await prisma.article.findMany({
  where: {
    content: { textMatch: { query: "rust programming" } },
  },
});
```

### Drizzle

```typescript
import { vectorSearch, geoWithinRadius, textMatch } from "@ormdb/client/drizzle";

// Vector search
const similar = await db
  .select()
  .from(products)
  .where(vectorSearch(products.embedding, queryVector, 10));

// Geo search
const nearby = await db
  .select()
  .from(restaurants)
  .where(geoWithinRadius(restaurants.location, 37.7749, -122.4194, 5.0));

// Text search
const articles = await db
  .select()
  .from(articles)
  .where(textMatch(articles.content, "rust programming"));
```

### SQLAlchemy

```python
from ormdb.sqlalchemy import vector_search, geo_within_radius, text_match

# Vector search
stmt = select(Product).where(
    vector_search(Product.embedding, query_vector, k=10)
)

# Geo search
stmt = select(Restaurant).where(
    geo_within_radius(Restaurant.location, 37.7749, -122.4194, 5.0)
)

# Text search
stmt = select(Article).where(
    text_match(Article.content, "rust programming")
)
```

### Django

```python
from ormdb.django import register_lookups
register_lookups()

# Vector search
Product.objects.filter(
    embedding__vector_search={'query_vector': [...], 'k': 10}
)

# Geo search
Restaurant.objects.filter(
    location__geo_radius={'lat': 37.7749, 'lon': -122.4194, 'radius_km': 5}
)

# Text search
Article.objects.filter(content__text_match='rust programming')
```

---

## Best Practices

1. **Index appropriate fields** - Ensure vector, geo, and text fields have the correct index type
2. **Use distance thresholds** - For vector search, set `max_distance` to filter irrelevant results
3. **Limit k appropriately** - Start with smaller k values and increase as needed
4. **Combine with filters** - Use standard filters to narrow down search space before expensive operations
5. **Consider pagination** - Use `limit` and `offset` for large result sets

---

## Next Steps

- **[Filtering](../tutorials/filtering.md)** - Standard filter operators
- **[Query API](../reference/query-api.md)** - Complete query reference
- **[Performance](performance.md)** - Query optimization tips
