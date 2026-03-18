//! Query and mutation execution.

use crate::formatter::Formatter;
use ormdb_client::Client;
use ormdb_lang::{parse_and_compile, CompiledMutation, CompiledSchemaCommand, CompiledStatement};
use ormdb_proto::{ExplainResult, MetricsResult};
use thiserror::Error;

/// Execution errors.
#[derive(Debug, Error)]
pub enum ExecuteError {
    /// Parse or compile error.
    #[error("{0}")]
    Language(String),

    /// Client communication error.
    #[error("client error: {0}")]
    Client(#[from] ormdb_client::Error),

    /// Not connected to server.
    #[error("not connected to server")]
    NotConnected,
}

/// Execute a statement and return formatted output.
pub async fn execute(
    client: &Client,
    input: &str,
    formatter: &dyn Formatter,
) -> Result<String, ExecuteError> {
    // Parse and compile the input
    let compiled = parse_and_compile(input).map_err(|e| ExecuteError::Language(e.format_with_source(input)))?;

    match compiled {
        CompiledStatement::Query(query) => {
            let result = client.query(query).await?;
            Ok(formatter.format_query_result(&result))
        }
        CompiledStatement::Aggregate(query) => {
            let result = client.aggregate(query).await?;
            Ok(format_aggregate_result(&result))
        }
        CompiledStatement::Mutation(mutation) => execute_mutation(client, mutation, formatter).await,
        CompiledStatement::SchemaCommand(cmd) => {
            execute_schema_command(client, cmd, formatter).await
        }
    }
}

/// Format an aggregate result for display.
fn format_aggregate_result(result: &ormdb_proto::AggregateResult) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Aggregate: {}", result.entity));
    lines.push("-".repeat(40));

    for value in &result.values {
        let func_name = match value.function {
            ormdb_proto::AggregateFunction::Count => "COUNT",
            ormdb_proto::AggregateFunction::Sum => "SUM",
            ormdb_proto::AggregateFunction::Avg => "AVG",
            ormdb_proto::AggregateFunction::Min => "MIN",
            ormdb_proto::AggregateFunction::Max => "MAX",
        };

        let field_part = value.field.as_ref().map(|f| format!("({})", f)).unwrap_or_else(|| "".to_string());
        lines.push(format!("{}{}: {}", func_name, field_part, format_value(&value.value)));
    }

    lines.join("\n")
}

/// Format a value for display.
fn format_value(value: &ormdb_proto::Value) -> String {
    match value {
        ormdb_proto::Value::Null => "NULL".to_string(),
        ormdb_proto::Value::Bool(b) => b.to_string(),
        ormdb_proto::Value::Int32(n) => n.to_string(),
        ormdb_proto::Value::Int64(n) => n.to_string(),
        ormdb_proto::Value::Float32(n) => format!("{:.2}", n),
        ormdb_proto::Value::Float64(n) => format!("{:.2}", n),
        ormdb_proto::Value::String(s) => s.clone(),
        ormdb_proto::Value::Bytes(b) => format!("<{} bytes>", b.len()),
        ormdb_proto::Value::Timestamp(ts) => ts.to_string(),
        ormdb_proto::Value::Uuid(u) => format_uuid(u),
        _ => "<complex>".to_string(),
    }
}

/// Format a UUID for display.
fn format_uuid(bytes: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Execute a mutation.
async fn execute_mutation(
    client: &Client,
    mutation: CompiledMutation,
    formatter: &dyn Formatter,
) -> Result<String, ExecuteError> {
    match mutation {
        CompiledMutation::Insert(m) => {
            let result = client.mutate(m).await?;
            let message = if !result.inserted_ids.is_empty() {
                format!("inserted {} record(s)", result.inserted_ids.len())
            } else {
                String::new()
            };
            Ok(formatter.format_mutation_result(result.affected as usize, &message))
        }
        CompiledMutation::UpdateWithFilter { entity, filter: _, data } => {
            // For now, we need to build a query to find matching entities,
            // then update each one. This is a simplified implementation.
            // In a full implementation, the server would support filter-based mutations.

            // Build a mutation for the operation
            // Since we don't have the IDs, we'll report what would happen
            Ok(formatter.format_message(&format!(
                "Update {} with {} fields (filter-based updates require server support)",
                entity,
                data.len()
            )))
        }
        CompiledMutation::DeleteWithFilter { entity, filter: _ } => {
            // Similar to update, filter-based deletes need server support
            Ok(formatter.format_message(&format!(
                "Delete from {} (filter-based deletes require server support)",
                entity
            )))
        }
        CompiledMutation::UpsertWithFilter { entity, filter: _, data } => {
            Ok(formatter.format_message(&format!(
                "Upsert {} with {} fields (filter-based upserts require server support)",
                entity,
                data.len()
            )))
        }
    }
}

/// Execute a schema command.
async fn execute_schema_command(
    client: &Client,
    cmd: CompiledSchemaCommand,
    formatter: &dyn Formatter,
) -> Result<String, ExecuteError> {
    match cmd {
        CompiledSchemaCommand::ListEntities => {
            let (_version, schema_bytes) = client.get_schema().await?;
            // For now, we'll just report the schema version
            // A full implementation would deserialize the schema and list entities
            Ok(formatter.format_message(&format!(
                "Schema loaded ({} bytes). Entity listing requires schema deserialization.",
                schema_bytes.len()
            )))
        }
        CompiledSchemaCommand::DescribeEntity(entity) => {
            Ok(formatter.format_message(&format!(
                "Describe entity '{}' (requires schema deserialization)",
                entity
            )))
        }
        CompiledSchemaCommand::DescribeRelation(relation) => {
            Ok(formatter.format_message(&format!(
                "Describe relation '{}' (requires schema deserialization)",
                relation
            )))
        }
        CompiledSchemaCommand::Help => Ok(get_help_text()),
    }
}

/// Execute an EXPLAIN command for a query.
pub async fn explain(client: &Client, input: &str) -> Result<ExplainResult, ExecuteError> {
    // Parse and compile the input
    let compiled =
        parse_and_compile(input).map_err(|e| ExecuteError::Language(e.format_with_source(input)))?;

    match compiled {
        CompiledStatement::Query(query) => {
            let result = client.explain(query).await?;
            Ok(result)
        }
        _ => Err(ExecuteError::Language(
            "EXPLAIN only works with queries".to_string(),
        )),
    }
}

/// Get server metrics.
pub async fn get_metrics(client: &Client) -> Result<MetricsResult, ExecuteError> {
    let result = client.get_metrics().await?;
    Ok(result)
}

/// Format an explain result for display.
pub fn format_explain(result: &ExplainResult) -> String {
    // The result already contains a human-readable explanation
    result.explanation.clone()
}

/// Format metrics result for display.
pub fn format_metrics(result: &MetricsResult) -> String {
    let mut lines = Vec::new();

    lines.push("Server Metrics".to_string());
    lines.push("=".repeat(40));
    lines.push(format!("Uptime: {}s", result.uptime_secs));
    lines.push(String::new());

    // Query metrics
    lines.push("Queries:".to_string());
    lines.push(format!("  Total: {}", result.queries.total_count));
    if result.queries.total_count > 0 {
        lines.push(format!(
            "  Avg Latency: {:.2}ms",
            result.queries.avg_duration_us as f64 / 1000.0
        ));
        lines.push(format!(
            "  P50 Latency: {:.2}ms",
            result.queries.p50_duration_us as f64 / 1000.0
        ));
        lines.push(format!(
            "  P99 Latency: {:.2}ms",
            result.queries.p99_duration_us as f64 / 1000.0
        ));
    }

    // Show queries by entity if available
    if !result.queries.by_entity.is_empty() {
        lines.push("  By Entity:".to_string());
        for entity_count in &result.queries.by_entity {
            lines.push(format!("    {}: {}", entity_count.entity, entity_count.count));
        }
    }
    lines.push(String::new());

    // Mutation metrics
    lines.push("Mutations:".to_string());
    lines.push(format!("  Total: {}", result.mutations.total_count));
    lines.push(format!("  Inserts: {}", result.mutations.inserts));
    lines.push(format!("  Updates: {}", result.mutations.updates));
    lines.push(format!("  Deletes: {}", result.mutations.deletes));
    lines.push(format!("  Upserts: {}", result.mutations.upserts));
    lines.push(format!("  Rows Affected: {}", result.mutations.rows_affected));
    lines.push(String::new());

    // Cache metrics
    lines.push("Plan Cache:".to_string());
    lines.push(format!("  Hit Rate: {:.1}%", result.cache.hit_rate * 100.0));
    lines.push(format!("  Hits: {}", result.cache.hits));
    lines.push(format!("  Misses: {}", result.cache.misses));
    lines.push(format!(
        "  Size: {}/{}",
        result.cache.size, result.cache.capacity
    ));
    lines.push(format!("  Evictions: {}", result.cache.evictions));
    lines.push(String::new());

    // Storage metrics
    lines.push("Storage:".to_string());
    lines.push(format!("  Total Entities: {}", result.storage.total_entities));
    if !result.storage.entity_counts.is_empty() {
        lines.push("  By Type:".to_string());
        for ec in &result.storage.entity_counts {
            lines.push(format!("    {}: {}", ec.entity, ec.count));
        }
    }

    lines.join("\n")
}

/// Get help text for the query language.
fn get_help_text() -> String {
    r#"ORMDB Query Language Help
=========================

QUERIES
-------
Entity.findMany()                           Find all records
Entity.findMany().where(field == value)     Filter records
Entity.findMany().include(relation)         Include related records
Entity.findMany().orderBy(field.asc)        Sort ascending
Entity.findMany().orderBy(field.desc)       Sort descending
Entity.findMany().limit(10)                 Limit results
Entity.findMany().offset(20)                Skip results
Entity.findUnique().where(id == "...")      Find by unique field
Entity.findFirst().where(...)               Find first match

AGGREGATE QUERIES
-----------------
Entity.count()                              Count all records
Entity.count().where(field == value)        Count matching records

MUTATIONS
---------
Entity.create({ field: value, ... })        Create new record
Entity.update().where(...).set({...})       Update records
Entity.delete().where(...)                  Delete records
Entity.upsert().where(...).set({...})       Insert or update

FILTER OPERATORS
----------------
field == value          Equal
field != value          Not equal
field < value           Less than
field <= value          Less than or equal
field > value           Greater than
field >= value          Greater than or equal
field in [v1, v2]       In list
field not in [v1, v2]   Not in list
field is null           Is null
field is not null       Is not null
field like "pattern"    Pattern match (% = wildcard)
cond1 && cond2          Logical AND
cond1 || cond2          Logical OR

REPL COMMANDS
-------------
.connect <address>      Connect to server
.disconnect             Disconnect from server
.status                 Show connection status
.schema                 List all entities
.schema <Entity>        Describe entity
.format table|json|csv  Set output format
.history                Show query history
.clear                  Clear screen
.help                   Show this help
.exit / .quit           Exit REPL
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_text() {
        let help = get_help_text();
        assert!(help.contains("QUERIES"));
        assert!(help.contains("MUTATIONS"));
        assert!(help.contains("FILTER OPERATORS"));
    }
}
