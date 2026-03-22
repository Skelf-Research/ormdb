# Query Engine

The query engine transforms GraphQuery requests into executable plans and runs them against the storage engine.

---

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Query Engine                             │
│                                                                  │
│  GraphQuery ──► Planner ──► QueryPlan ──► Executor ──► Result   │
│                    │            │             │                  │
│                    ▼            ▼             ▼                  │
│               Catalog       Cache        Storage                │
│                                              │                   │
│                                    ┌─────────┼─────────┐        │
│                                    ▼         ▼         ▼        │
│                                 Filter    Join    Aggregate     │
└─────────────────────────────────────────────────────────────────┘
```

---

## Query Planner

The planner validates queries and produces execution plans.

### Planning Steps

1. **Resolve root entity** - Look up entity definition in catalog
2. **Validate fields** - Ensure requested fields exist
3. **Plan includes** - Resolve relations for each include
4. **Check depth** - Enforce maximum include depth
5. **Estimate fanout** - Calculate expected result size
6. **Optimize order** - Reorder includes by estimated cost

### QueryPlan Structure

```rust
pub struct QueryPlan {
    /// Root entity type
    pub root_entity: String,

    /// Resolved entity definition
    pub root_entity_def: EntityDef,

    /// Fields to project (empty = all)
    pub fields: Vec<String>,

    /// Filter expression
    pub filter: Option<FilterExpr>,

    /// Sort order
    pub order_by: Vec<OrderSpec>,

    /// Pagination
    pub pagination: Option<Pagination>,

    /// Nested includes
    pub includes: Vec<IncludePlan>,

    /// Resource limits
    pub budget: FanoutBudget,
}
```

### IncludePlan Structure

```rust
pub struct IncludePlan {
    /// Path from root (e.g., "posts.comments")
    pub path: String,

    /// Relation definition
    pub relation: RelationDef,

    /// Target entity definition
    pub target_entity_def: EntityDef,

    /// Fields to project
    pub fields: Vec<String>,

    /// Filter for related entities
    pub filter: Option<FilterExpr>,

    /// Sort order
    pub order_by: Vec<OrderSpec>,

    /// Pagination per parent
    pub pagination: Option<Pagination>,
}
```

### Include Depth

```rust
impl IncludePlan {
    /// Depth: 1 for "posts", 2 for "posts.comments"
    pub fn depth(&self) -> usize {
        self.path.matches('.').count() + 1
    }

    pub fn is_top_level(&self) -> bool {
        !self.path.contains('.')
    }

    pub fn parent_path(&self) -> Option<&str> {
        self.path.rsplit_once('.').map(|(parent, _)| parent)
    }
}
```

---

## Fanout Budget

Budgets prevent runaway queries:

```rust
pub struct FanoutBudget {
    /// Max entities across all blocks
    pub max_entities: usize,  // Default: 10,000

    /// Max edges (relationships)
    pub max_edges: usize,     // Default: 50,000

    /// Max include depth
    pub max_depth: usize,     // Default: 5
}
```

### Budget Enforcement

```rust
// Check depth at planning time
if include.depth() > budget.max_depth {
    return Err(Error::InvalidData(format!(
        "Query depth {} exceeds maximum {}",
        include.depth(), budget.max_depth
    )));
}
```

### Fanout Estimation

```rust
pub fn estimate_fanout(cardinality: Cardinality) -> usize {
    match cardinality {
        Cardinality::OneToOne => 1,
        Cardinality::OneToMany => 10,
        Cardinality::ManyToMany => 25,
    }
}
```

---

## Include Optimization

The planner optimizes include order for efficiency:

```rust
impl QueryPlan {
    pub fn optimize_include_order(&mut self) {
        // 1. Build dependency graph
        let dependencies = build_dependencies(&self.includes);

        // 2. Estimate cost per include
        let costs = estimate_costs(&self.includes);

        // 3. Topological sort with cost-based ordering
        // Process cheaper includes first while respecting dependencies
        self.includes = topological_sort_by_cost(
            &self.includes,
            &dependencies,
            &costs
        );
    }
}
```

Example:

```
Before optimization:
  posts.comments (depth 2, fanout 25)
  posts (depth 1, fanout 10)
  profile (depth 1, fanout 1)

After optimization:
  profile (depth 1, fanout 1)      ← Cheapest first
  posts (depth 1, fanout 10)       ← Required before posts.comments
  posts.comments (depth 2, fanout 25)
```

---

## Query Executor

The executor runs plans against storage.

### Execution Flow

```rust
pub async fn execute(&self, plan: &QueryPlan) -> Result<QueryResult> {
    // 1. Fetch root entities
    let root_entities = self.fetch_root_entities(plan).await?;

    // 2. Execute each include
    let mut edge_blocks = Vec::new();
    for include_plan in &plan.includes {
        let edges = self.execute_include(
            include_plan,
            &root_entities,
            &edge_blocks
        ).await?;
        edge_blocks.push(edges);
    }

    // 3. Assemble result
    Ok(QueryResult {
        entities: root_entities,
        edges: edge_blocks,
    })
}
```

### Root Entity Fetching

```rust
async fn fetch_root_entities(&self, plan: &QueryPlan) -> Result<EntityBlock> {
    // 1. Check for index-optimized path
    if let Some(filter) = &plan.filter {
        if let Some(entity_ids) = self.try_index_lookup(filter).await? {
            return self.fetch_by_ids(entity_ids).await;
        }
    }

    // 2. Fall back to scan with filter
    let entities = self.storage
        .scan_entity_type(&plan.root_entity)
        .filter(|e| self.evaluate_filter(&plan.filter, e))
        .collect();

    // 3. Apply ordering
    if !plan.order_by.is_empty() {
        entities.sort_by(|a, b| self.compare_by_order(&plan.order_by, a, b));
    }

    // 4. Apply pagination
    if let Some(pagination) = &plan.pagination {
        entities = entities
            .skip(pagination.offset)
            .take(pagination.limit)
            .collect();
    }

    Ok(EntityBlock::new(&plan.root_entity, entities))
}
```

---

## Filter Evaluation

Filter expressions are evaluated recursively:

```rust
pub fn evaluate(filter: &FilterExpr, entity: &Entity) -> bool {
    match filter {
        FilterExpr::Eq { field, value } => {
            entity.get(field) == Some(value)
        }
        FilterExpr::Ne { field, value } => {
            entity.get(field) != Some(value)
        }
        FilterExpr::Gt { field, value } => {
            entity.get(field).map(|v| v > value).unwrap_or(false)
        }
        FilterExpr::Like { field, pattern } => {
            entity.get_string(field)
                .map(|s| match_pattern(s, pattern))
                .unwrap_or(false)
        }
        FilterExpr::And { left, right } => {
            evaluate(left, entity) && evaluate(right, entity)
        }
        FilterExpr::Or { left, right } => {
            evaluate(left, entity) || evaluate(right, entity)
        }
        FilterExpr::Not { expr } => {
            !evaluate(expr, entity)
        }
        // ... other operators
    }
}
```

---

## Join Strategies

The executor supports multiple join strategies:

### Nested Loop Join

Simple O(N * M) algorithm for small datasets:

```rust
fn nested_loop_join(
    parent_ids: &[[u8; 16]],
    relation: &RelationDef,
    storage: &StorageEngine,
) -> Result<Vec<([u8; 16], [u8; 16])>> {
    let mut edges = Vec::new();

    for parent_id in parent_ids {
        // Fetch children for this parent
        let children = storage.scan_entity_type(&relation.to_entity)
            .filter(|c| c.get(&relation.to_field) == Some(parent_id))
            .collect::<Vec<_>>();

        for child in children {
            edges.push((*parent_id, child.id()));
        }
    }

    Ok(edges)
}
```

### Hash Join

O(N + M) algorithm for larger datasets:

```rust
fn hash_join(
    parent_ids: &[[u8; 16]],
    relation: &RelationDef,
    storage: &StorageEngine,
) -> Result<Vec<([u8; 16], [u8; 16])>> {
    // 1. Build phase: Create hash map of parent IDs
    let parent_set: HashSet<[u8; 16]> = parent_ids.iter().copied().collect();

    // 2. Probe phase: Scan children and match
    let edges = storage.scan_entity_type(&relation.to_entity)
        .filter_map(|child| {
            let fk = child.get(&relation.to_field)?;
            if parent_set.contains(fk) {
                Some((*fk, child.id()))
            } else {
                None
            }
        })
        .collect();

    Ok(edges)
}
```

### Strategy Selection

```rust
fn select_join_strategy(
    parent_count: usize,
    estimated_child_count: usize,
) -> JoinStrategy {
    if parent_count < 100 {
        JoinStrategy::NestedLoop
    } else {
        JoinStrategy::Hash
    }
}
```

---

## Plan Caching

Query plans are cached by fingerprint:

### Fingerprint Computation

```rust
fn compute_fingerprint(query: &GraphQuery) -> u64 {
    let mut hasher = DefaultHasher::new();

    // Hash structure, not values
    query.root_entity.hash(&mut hasher);
    query.fields.hash(&mut hasher);

    if let Some(filter) = &query.filter {
        // Hash filter shape, not parameter values
        hash_filter_shape(filter, &mut hasher);
    }

    for include in &query.includes {
        include.path.hash(&mut hasher);
        include.fields.hash(&mut hasher);
    }

    hasher.finish()
}
```

### Cache Usage

```rust
pub async fn execute_query(&self, query: &GraphQuery) -> Result<QueryResult> {
    let fingerprint = compute_fingerprint(query);

    // Try cache
    let plan = if let Some(cached) = self.plan_cache.get(&fingerprint) {
        cached.clone()
    } else {
        // Plan and cache
        let plan = self.planner.plan(query)?;
        self.plan_cache.insert(fingerprint, plan.clone());
        plan
    };

    // Execute with query-specific values
    self.executor.execute(&plan, query).await
}
```

---

## Aggregations

The aggregate executor handles COUNT, SUM, AVG, MIN, MAX:

```rust
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

pub fn execute_aggregate(
    function: AggregateFunction,
    field: &str,
    entities: &[Entity],
) -> Value {
    match function {
        AggregateFunction::Count => {
            Value::Int64(entities.len() as i64)
        }
        AggregateFunction::Sum => {
            let sum: f64 = entities
                .iter()
                .filter_map(|e| e.get_number(field))
                .sum();
            Value::Float64(sum)
        }
        AggregateFunction::Avg => {
            let values: Vec<f64> = entities
                .iter()
                .filter_map(|e| e.get_number(field))
                .collect();
            if values.is_empty() {
                Value::Null
            } else {
                Value::Float64(values.iter().sum::<f64>() / values.len() as f64)
            }
        }
        AggregateFunction::Min => {
            entities
                .iter()
                .filter_map(|e| e.get(field))
                .min()
                .cloned()
                .unwrap_or(Value::Null)
        }
        AggregateFunction::Max => {
            entities
                .iter()
                .filter_map(|e| e.get(field))
                .max()
                .cloned()
                .unwrap_or(Value::Null)
        }
    }
}
```

### Columnar Optimization

For large datasets, aggregations use the columnar store:

```rust
pub fn execute_aggregate_columnar(
    function: AggregateFunction,
    field: &str,
    entity_type: &str,
    columnar: &ColumnarStore,
) -> Result<Value> {
    let projection = columnar.projection(entity_type)?;
    let column = projection.get_column(field)?;

    // Operate directly on column data
    match function {
        AggregateFunction::Sum => column.sum(),
        AggregateFunction::Avg => column.avg(),
        // ...
    }
}
```

---

## Explain Service

Query plans can be explained for debugging:

```rust
pub fn explain(plan: &QueryPlan) -> ExplainResult {
    ExplainResult {
        root_entity: plan.root_entity.clone(),
        estimated_rows: estimate_rows(plan),
        access_path: determine_access_path(plan),
        includes: plan.includes.iter().map(|i| ExplainInclude {
            path: i.path.clone(),
            strategy: determine_join_strategy(&i.relation),
            estimated_fanout: estimate_fanout(i.relation.cardinality),
        }).collect(),
        warnings: collect_warnings(plan),
    }
}
```

Example output:

```
Query Explain
─────────────
Root: User
Estimated rows: 1,000
Access path: TypeIndex scan

Includes:
  1. posts (HashJoin, fanout ~10)
  2. posts.comments (HashJoin, fanout ~25)

Warnings:
  - No index on User.status, consider adding one
  - Include depth 2 may result in large result set
```

---

## Performance Tips

1. **Use indexes for filters** - Hash index for equality, B-tree for ranges
2. **Limit include depth** - Deep nesting multiplies result size
3. **Project only needed fields** - Reduces data transfer
4. **Paginate root entities** - Control result size
5. **Filter at the source** - Include filters reduce join work

---

## Next Steps

- **[Storage Engine](storage-engine.md)** - Data organization
- **[Index Internals](index-internals.md)** - Index implementation
- **[Performance Guide](../guides/performance.md)** - Optimization tips
