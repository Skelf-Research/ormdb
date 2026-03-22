# Troubleshooting Guide

Common issues and solutions for ORMDB.

## Quick Diagnostics

```bash
# Check server status
ormdb admin status

# View recent logs
ormdb admin logs --tail 100

# Check connectivity
ormdb admin ping

# Verify data integrity
ormdb admin verify
```

## Connection Issues

### Cannot Connect to Server

**Symptoms:**
- Connection refused
- Connection timeout
- "Server unavailable" error

**Solutions:**

1. **Check if server is running**
   ```bash
   ormdb server status
   # or
   ps aux | grep ormdb
   ```

2. **Verify port binding**
   ```bash
   netstat -tlnp | grep 8080
   # or
   ss -tlnp | grep 8080
   ```

3. **Check firewall rules**
   ```bash
   # Linux
   sudo iptables -L -n | grep 8080

   # macOS
   sudo pfctl -s rules | grep 8080
   ```

4. **Verify configuration**
   ```bash
   ormdb config show | grep -E "host|port"
   ```

### Connection Drops

**Symptoms:**
- Intermittent disconnections
- "Connection reset" errors

**Solutions:**

1. **Check connection limits**
   ```bash
   ormdb admin stats | grep connections
   ```

2. **Increase connection limit**
   ```toml
   # ormdb.toml
   [server]
   max_connections = 2000
   ```

3. **Check for idle timeout**
   ```toml
   # ormdb.toml
   [server]
   idle_timeout_seconds = 300
   ```

4. **Check client connection pooling**
   ```rust
   // Ensure proper pool configuration
   let config = ClientConfig {
       max_connections: 20,
       idle_timeout: Duration::from_secs(300),
       ..Default::default()
   };
   ```

### Too Many Connections

**Symptoms:**
- "Connection limit exceeded" error
- New connections rejected

**Solutions:**

1. **Check current connections**
   ```bash
   ormdb admin connections
   ```

2. **Identify connection leaks**
   ```bash
   ormdb admin connections --by-client
   ```

3. **Increase limit (if appropriate)**
   ```toml
   [server]
   max_connections = 5000
   ```

4. **Fix connection leaks in application**
   ```rust
   // Ensure connections are properly returned to pool
   // Use connection pooling, not individual connections
   ```

## Query Issues

### Slow Queries

**Symptoms:**
- Queries taking >1 second
- Timeouts on large queries

**Solutions:**

1. **Check slow query log**
   ```bash
   ormdb admin slow-queries --limit 10
   ```

2. **Analyze query plan**
   ```bash
   ormdb query User "status = 'active'" --explain
   ```

3. **Add missing indexes**
   ```bash
   ormdb schema add-index User user_status_idx status
   ```

4. **Optimize includes**
   ```rust
   // Bad: Deep unbounded includes
   .include(RelationInclude::new("posts")
       .include(RelationInclude::new("comments")))

   // Good: Limited includes
   .include(RelationInclude::new("posts")
       .with_limit(10)
       .with_fields(vec!["id", "title"]))
   ```

5. **Use query budgets**
   ```rust
   .with_budget(FanoutBudget {
       max_entities: 1000,
       max_edges: 5000,
       max_depth: 3,
   })
   ```

### Query Budget Exceeded

**Symptoms:**
- "Budget exceeded" error
- Queries returning partial results

**Solutions:**

1. **Add pagination**
   ```rust
   .with_pagination(Pagination::new(100, 0))
   ```

2. **Reduce include depth**
   ```rust
   // Limit nesting
   .include(RelationInclude::new("posts")
       .with_limit(10))  // Don't nest further
   ```

3. **Increase budget (carefully)**
   ```toml
   [query]
   max_entities = 20000
   max_edges = 100000
   max_depth = 6
   ```

### Query Returns Wrong Results

**Symptoms:**
- Missing expected data
- Incorrect filter results

**Solutions:**

1. **Verify filter syntax**
   ```rust
   // Check operator
   FilterExpr::eq("status", Value::String("active".into()))
   // Not
   FilterExpr::eq("status", "active")  // Wrong type!
   ```

2. **Check RLS policies**
   ```bash
   ormdb admin policies --entity User
   ```

3. **Test without RLS**
   ```rust
   // Temporarily bypass RLS for debugging
   let ctx = SecurityContext::admin();
   client.query_with_context(query, ctx).await?;
   ```

4. **Verify data exists**
   ```bash
   ormdb query User --filter "id = '...'" --context admin
   ```

## Mutation Issues

### Insert Fails

**Symptoms:**
- "Constraint violation" error
- Insert returns error

**Solutions:**

1. **Check unique constraints**
   ```bash
   ormdb schema show User | grep unique
   ```

2. **Verify required fields**
   ```rust
   // Ensure all non-nullable fields are provided
   EntityData::new()
       .set("name", Value::String("Alice".into()))
       .set("email", Value::String("alice@example.com".into()))
       // Don't forget required fields!
   ```

3. **Check foreign key references**
   ```bash
   # Verify referenced entity exists
   ormdb query User --filter "id = 'referenced-id'"
   ```

### Update Conflicts

**Symptoms:**
- "Version conflict" error
- Optimistic locking failures

**Solutions:**

1. **Implement retry logic**
   ```rust
   loop {
       let entity = client.get("User", id).await?;
       let result = client.update("User", id,
           data.clone(),
           Some(entity.version)
       ).await;

       match result {
           Ok(_) => break,
           Err(Error::TransactionConflict { .. }) => {
               // Retry with fresh data
               continue;
           }
           Err(e) => return Err(e),
       }
   }
   ```

2. **Use transactions for complex updates**
   ```rust
   let tx = client.begin_transaction().await?;
   // ... perform multiple operations
   tx.commit().await?;
   ```

### Delete Blocked

**Symptoms:**
- "Restrict violation" error
- Cannot delete entity

**Solutions:**

1. **Check referencing entities**
   ```bash
   ormdb admin references User 'user-id'
   ```

2. **Delete or reassign references first**
   ```rust
   // Delete child entities
   client.delete_many("Post", FilterExpr::eq("author_id", user_id)).await?;
   // Then delete parent
   client.delete("User", user_id).await?;
   ```

3. **Use cascade delete**
   ```rust
   let mutation = DeleteMutation::new("User")
       .with_id(user_id)
       .cascade();
   client.mutate(Mutation::Delete(mutation)).await?;
   ```

## Storage Issues

### High Disk Usage

**Symptoms:**
- Disk space warnings
- Write failures

**Solutions:**

1. **Check storage usage**
   ```bash
   ormdb admin stats --storage
   ```

2. **Run compaction**
   ```bash
   ormdb admin compact
   ```

3. **Trim WAL**
   ```bash
   ormdb admin wal trim --keep-days 7
   ```

4. **Archive old data**
   ```bash
   # Export old data
   ormdb query Event "created_at < '2023-01-01'" --format ndjson > old-events.ndjson

   # Delete from database
   ormdb delete Event "created_at < '2023-01-01'"
   ```

### Low Cache Hit Rate

**Symptoms:**
- Cache hit rate below 80%
- Increased disk I/O

**Solutions:**

1. **Check cache statistics**
   ```bash
   ormdb admin stats | grep cache
   ```

2. **Increase cache size**
   ```toml
   [storage]
   cache_size_mb = 1024  # Increase from 256
   ```

3. **Analyze query patterns**
   ```bash
   # Look for queries scanning large amounts of data
   ormdb admin slow-queries
   ```

### Data Corruption

**Symptoms:**
- Checksum errors
- Inconsistent data
- Server crashes

**Solutions:**

1. **Run verification**
   ```bash
   ormdb admin verify
   ```

2. **Repair if possible**
   ```bash
   ormdb admin repair
   ```

3. **Restore from backup**
   ```bash
   ormdb server stop
   ormdb backup restore latest.ormdb --target /var/lib/ormdb/data
   ormdb server start
   ```

## Performance Issues

### High Memory Usage

**Symptoms:**
- OOM kills
- Swap usage
- Slow performance

**Solutions:**

1. **Check memory breakdown**
   ```bash
   ormdb admin stats --memory
   ```

2. **Reduce cache size**
   ```toml
   [storage]
   cache_size_mb = 256  # Reduce
   ```

3. **Limit query results**
   ```toml
   [query]
   max_entities = 5000  # Reduce default
   ```

4. **Check for memory leaks**
   ```bash
   # Monitor over time
   watch -n 60 'ormdb admin stats | grep memory'
   ```

### High CPU Usage

**Symptoms:**
- CPU constantly at 100%
- Slow response times

**Solutions:**

1. **Identify expensive queries**
   ```bash
   ormdb admin slow-queries
   ```

2. **Add indexes**
   ```bash
   ormdb query User "email LIKE '%@example.com'" --explain
   # If full scan, add index
   ormdb schema add-index User user_email_idx email
   ```

3. **Reduce parallelism**
   ```toml
   [query]
   parallel_scan = false
   ```

## Common Error Messages

### "Entity not found"

```rust
// Entity doesn't exist
Err(Error::NotFound)
```

**Cause:** Querying non-existent entity ID.

**Solution:** Check ID validity, handle NotFound gracefully.

### "Unique constraint violation"

```rust
Err(Error::ConstraintViolation(ConstraintError::UniqueViolation { .. }))
```

**Cause:** Duplicate value for unique field.

**Solution:** Use upsert or check existence first.

### "Foreign key violation"

```rust
Err(Error::ConstraintViolation(ConstraintError::ForeignKeyViolation { .. }))
```

**Cause:** Referenced entity doesn't exist.

**Solution:** Create referenced entity first, or check reference validity.

### "Schema mismatch"

**Cause:** Client schema doesn't match server.

**Solution:** Update client, run migrations, or sync schema.

### "Budget exceeded"

**Cause:** Query would return too much data.

**Solution:** Add pagination, limits, or filters.

## Getting Help

### Collect Diagnostics

```bash
# Generate diagnostic report
ormdb admin diagnostics > diagnostics.txt

# Include:
# - Server version
# - Configuration (sanitized)
# - Recent errors
# - Statistics
# - Slow queries
```

### Log Levels

```toml
# Enable debug logging temporarily
[logging]
level = "debug"
```

### Support Channels

- **GitHub Issues:** Bug reports and feature requests
- **Discussions:** Questions and community help
- **Documentation:** API reference and guides

---

## Next Steps

- **[Error Reference](../reference/errors.md)** - Complete error documentation
- **[CLI Reference](../reference/cli.md)** - All admin commands
- **[Monitoring](monitoring.md)** - Set up proactive alerts
