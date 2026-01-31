//! Output formatters for query results.

use clap::ValueEnum;
use comfy_table::{Cell, Table};
use ormdb_proto::result::{EntityBlock, QueryResult};
use ormdb_proto::value::Value;

/// Output format for results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// ASCII table format
    Table,
    /// JSON format
    Json,
    /// CSV format
    Csv,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Csv => write!(f, "csv"),
        }
    }
}

/// Trait for formatting output.
pub trait Formatter: Send + Sync {
    /// Format a query result.
    fn format_query_result(&self, result: &QueryResult) -> String;

    /// Format a mutation result.
    fn format_mutation_result(&self, affected: usize, message: &str) -> String;

    /// Format an error message.
    fn format_error(&self, error: &str) -> String;

    /// Format a simple message.
    fn format_message(&self, message: &str) -> String;

    /// Format schema information.
    fn format_schema(&self, entities: &[String]) -> String;
}

/// Create a formatter for the given output format.
pub fn create_formatter(format: OutputFormat) -> Box<dyn Formatter> {
    match format {
        OutputFormat::Table => Box::new(TableFormatter),
        OutputFormat::Json => Box::new(JsonFormatter),
        OutputFormat::Csv => Box::new(CsvFormatter),
    }
}

/// Table formatter using comfy-table.
pub struct TableFormatter;

impl Formatter for TableFormatter {
    fn format_query_result(&self, result: &QueryResult) -> String {
        let mut output = String::new();

        // Format each entity block
        for block in &result.entities {
            if !output.is_empty() {
                output.push_str("\n\n");
            }
            output.push_str(&format_entity_block_as_table(block));
        }

        if output.is_empty() {
            output = "No results".to_string();
        }

        output
    }

    fn format_mutation_result(&self, affected: usize, message: &str) -> String {
        if message.is_empty() {
            format!("{} row(s) affected", affected)
        } else {
            format!("{} row(s) affected: {}", affected, message)
        }
    }

    fn format_error(&self, error: &str) -> String {
        format!("Error: {}", error)
    }

    fn format_message(&self, message: &str) -> String {
        message.to_string()
    }

    fn format_schema(&self, entities: &[String]) -> String {
        let mut table = Table::new();
        table.set_header(vec!["Entity"]);

        for entity in entities {
            table.add_row(vec![entity]);
        }

        table.to_string()
    }
}

/// JSON formatter.
pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn format_query_result(&self, result: &QueryResult) -> String {
        // Build a JSON object with each entity type
        let mut obj = serde_json::Map::new();

        for block in &result.entities {
            let rows = entity_block_to_json_array(block);
            obj.insert(block.entity.clone(), serde_json::Value::Array(rows));
        }

        if obj.is_empty() {
            "[]".to_string()
        } else if obj.len() == 1 {
            // If only one entity type, return just the array
            let (_, arr) = obj.into_iter().next().unwrap();
            serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
        } else {
            serde_json::to_string_pretty(&serde_json::Value::Object(obj))
                .unwrap_or_else(|_| "{}".to_string())
        }
    }

    fn format_mutation_result(&self, affected: usize, message: &str) -> String {
        serde_json::json!({
            "affected": affected,
            "message": message
        })
        .to_string()
    }

    fn format_error(&self, error: &str) -> String {
        serde_json::json!({
            "error": error
        })
        .to_string()
    }

    fn format_message(&self, message: &str) -> String {
        serde_json::json!({
            "message": message
        })
        .to_string()
    }

    fn format_schema(&self, entities: &[String]) -> String {
        serde_json::to_string_pretty(entities).unwrap_or_else(|_| "[]".to_string())
    }
}

/// CSV formatter.
pub struct CsvFormatter;

impl Formatter for CsvFormatter {
    fn format_query_result(&self, result: &QueryResult) -> String {
        let mut output = String::new();

        // CSV format: first entity block only (CSV doesn't support multiple tables well)
        if let Some(block) = result.entities.first() {
            output = format_entity_block_as_csv(block);
        }

        output
    }

    fn format_mutation_result(&self, affected: usize, message: &str) -> String {
        format!("affected,message\n{},\"{}\"", affected, escape_csv(message))
    }

    fn format_error(&self, error: &str) -> String {
        format!("error\n\"{}\"", escape_csv(error))
    }

    fn format_message(&self, message: &str) -> String {
        message.to_string()
    }

    fn format_schema(&self, entities: &[String]) -> String {
        let mut output = String::from("entity\n");
        for entity in entities {
            output.push_str(&format!("{}\n", entity));
        }
        output
    }
}

/// Format an entity block as a table.
fn format_entity_block_as_table(block: &EntityBlock) -> String {
    let mut table = Table::new();

    // Build header: id + column names
    let mut headers: Vec<Cell> = vec![Cell::new("id")];
    for col in &block.columns {
        headers.push(Cell::new(&col.name));
    }
    table.set_header(headers);

    // Add rows
    for (id, fields) in block.rows() {
        let mut cells: Vec<Cell> = vec![Cell::new(format_uuid(id))];
        for (_, value) in fields {
            cells.push(Cell::new(format_value(value)));
        }
        table.add_row(cells);
    }

    let row_count = block.len();
    format!("{}\n{} row(s)", table, row_count)
}

/// Convert an entity block to a JSON array.
fn entity_block_to_json_array(block: &EntityBlock) -> Vec<serde_json::Value> {
    block
        .rows()
        .map(|(id, fields)| {
            let mut obj = serde_json::Map::new();
            obj.insert("id".to_string(), serde_json::Value::String(format_uuid(id)));
            for (name, value) in fields {
                obj.insert(name.to_string(), value_to_json(value));
            }
            serde_json::Value::Object(obj)
        })
        .collect()
}

/// Format an entity block as CSV.
fn format_entity_block_as_csv(block: &EntityBlock) -> String {
    let mut output = String::new();

    // Header: id + column names
    let mut headers: Vec<&str> = vec!["id"];
    for col in &block.columns {
        headers.push(&col.name);
    }
    output.push_str(&headers.join(","));
    output.push('\n');

    // Rows
    for (id, fields) in block.rows() {
        let mut cells: Vec<String> = vec![format_uuid(id)];
        for (_, value) in fields {
            cells.push(format_value_csv(value));
        }
        output.push_str(&cells.join(","));
        output.push('\n');
    }

    output
}

/// Format a Value as a display string.
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int32(i) => i.to_string(),
        Value::Int64(i) => i.to_string(),
        Value::Float32(f) => f.to_string(),
        Value::Float64(f) => f.to_string(),
        Value::String(s) => s.clone(),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
        Value::Timestamp(ts) => {
            // Format as ISO 8601
            let secs = ts / 1_000_000;
            let micros = (ts % 1_000_000) as u32;
            if let Some(dt) = chrono_from_timestamp(secs, micros) {
                dt
            } else {
                format!("{}", ts)
            }
        }
        Value::Uuid(bytes) => format_uuid(bytes),
        Value::BoolArray(arr) => format!("{:?}", arr),
        Value::Int32Array(arr) => format!("{:?}", arr),
        Value::Int64Array(arr) => format!("{:?}", arr),
        Value::Float32Array(arr) => format!("{:?}", arr),
        Value::Float64Array(arr) => format!("{:?}", arr),
        Value::StringArray(arr) => format!("{:?}", arr),
        Value::UuidArray(arr) => {
            let uuids: Vec<String> = arr.iter().map(format_uuid).collect();
            format!("{:?}", uuids)
        }
    }
}

/// Format a Value for CSV output.
fn format_value_csv(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => format!("\"{}\"", escape_csv(s)),
        _ => format_value(value),
    }
}

/// Convert a Value to JSON.
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int32(i) => serde_json::Value::Number((*i).into()),
        Value::Int64(i) => serde_json::Value::Number((*i).into()),
        Value::Float32(f) => {
            serde_json::Number::from_f64(*f as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::Float64(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Bytes(b) => {
            // Encode as base64
            use std::io::Write;
            let mut encoded = Vec::new();
            let _ = write!(encoded, "{}", base64_encode(b));
            serde_json::Value::String(String::from_utf8_lossy(&encoded).to_string())
        }
        Value::Timestamp(ts) => serde_json::Value::Number((*ts).into()),
        Value::Uuid(bytes) => serde_json::Value::String(format_uuid(bytes)),
        Value::BoolArray(arr) => serde_json::Value::Array(
            arr.iter().map(|b| serde_json::Value::Bool(*b)).collect(),
        ),
        Value::Int32Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|i| serde_json::Value::Number((*i).into()))
                .collect(),
        ),
        Value::Int64Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|i| serde_json::Value::Number((*i).into()))
                .collect(),
        ),
        Value::Float32Array(arr) => serde_json::Value::Array(
            arr.iter()
                .filter_map(|f| serde_json::Number::from_f64(*f as f64))
                .map(serde_json::Value::Number)
                .collect(),
        ),
        Value::Float64Array(arr) => serde_json::Value::Array(
            arr.iter()
                .filter_map(|f| serde_json::Number::from_f64(*f))
                .map(serde_json::Value::Number)
                .collect(),
        ),
        Value::StringArray(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        ),
        Value::UuidArray(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|u| serde_json::Value::String(format_uuid(u)))
                .collect(),
        ),
    }
}

/// Format a UUID as a string.
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

/// Escape a string for CSV.
fn escape_csv(s: &str) -> String {
    s.replace('"', "\"\"")
}

/// Simple base64 encoding.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let chunks = data.chunks(3);

    for chunk in chunks {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[(b0 >> 2) & 0x3f] as char);
        result.push(ALPHABET[((b0 << 4) | (b1 >> 4)) & 0x3f] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 << 2) | (b2 >> 6)) & 0x3f] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Format a timestamp as ISO 8601 (simple implementation).
fn chrono_from_timestamp(secs: i64, _micros: u32) -> Option<String> {
    // Simple implementation without chrono crate
    // Just return the Unix timestamp for now
    Some(format!("{}", secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_uuid() {
        let bytes = [
            0x12, 0x3e, 0x45, 0x67, 0xe8, 0x9b, 0x12, 0xd3, 0xa4, 0x56, 0x42, 0x66, 0x14, 0x17,
            0x40, 0x00,
        ];
        let uuid = format_uuid(&bytes);
        assert_eq!(uuid, "123e4567-e89b-12d3-a456-426614174000");
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv("hello, world"), "hello, world");
        assert_eq!(escape_csv("say \"hi\""), "say \"\"hi\"\"");
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }
}
