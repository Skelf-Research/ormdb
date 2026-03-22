//! Aggregate query executor.
//!
//! This module provides execution of aggregate queries using the columnar store.

use crate::error::Error;
use crate::storage::{ColumnarStore, StorageEngine};
use ormdb_proto::{AggregateFunction, AggregateQuery, AggregateResult, AggregateValue, Value};

use super::join::EntityRow;
use super::FilterEvaluator;

/// Executor for aggregate queries.
///
/// Uses the columnar store for efficient aggregate computation when available.
pub struct AggregateExecutor<'a> {
    storage: &'a StorageEngine,
    columnar: &'a ColumnarStore,
}

impl<'a> AggregateExecutor<'a> {
    /// Create a new aggregate executor.
    pub fn new(storage: &'a StorageEngine, columnar: &'a ColumnarStore) -> Self {
        Self { storage, columnar }
    }

    /// Execute an aggregate query.
    pub fn execute(&self, query: &AggregateQuery) -> Result<AggregateResult, Error> {
        // If there's a filter, we need to apply it first
        if query.filter.is_some() {
            return self.execute_with_filter(query);
        }

        // No filter - use columnar aggregates directly
        self.execute_unfiltered(query)
    }

    /// Execute an aggregate query without filters using columnar store.
    fn execute_unfiltered(&self, query: &AggregateQuery) -> Result<AggregateResult, Error> {
        let projection = self.columnar.projection(&query.root_entity)?;
        let mut values = Vec::with_capacity(query.aggregations.len());

        for agg in &query.aggregations {
            let value = match agg.function {
                AggregateFunction::Count => {
                    let count = if let Some(field) = &agg.field {
                        // COUNT(field) - count non-null values in column
                        projection.count_column(field)?
                    } else {
                        // COUNT(*) - count all entities
                        self.count_entities(&query.root_entity)?
                    };
                    Value::Int64(count as i64)
                }
                AggregateFunction::Sum => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("SUM requires a field".into())
                    })?;
                    Value::Float64(projection.sum_column(field)?)
                }
                AggregateFunction::Avg => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("AVG requires a field".into())
                    })?;
                    let sum = projection.sum_column(field)?;
                    let count = projection.count_column(field)?;
                    if count > 0 {
                        Value::Float64(sum / count as f64)
                    } else {
                        Value::Null
                    }
                }
                AggregateFunction::Min => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("MIN requires a field".into())
                    })?;
                    projection.min_column(field)?.unwrap_or(Value::Null)
                }
                AggregateFunction::Max => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("MAX requires a field".into())
                    })?;
                    projection.max_column(field)?.unwrap_or(Value::Null)
                }
            };

            values.push(AggregateValue::new(
                agg.function,
                agg.field.clone(),
                value,
            ));
        }

        Ok(AggregateResult::new(query.root_entity.clone(), values))
    }

    /// Execute an aggregate query with a filter.
    ///
    /// This requires scanning entities and applying the filter before aggregation.
    /// Uses batch lookups for better performance (single batch read vs N individual reads).
    fn execute_with_filter(&self, query: &AggregateQuery) -> Result<AggregateResult, Error> {
        let filter = query.filter.as_ref().unwrap();

        // First, collect all entity IDs
        let all_ids: Vec<[u8; 16]> = self
            .storage
            .list_entity_ids(&query.root_entity)
            .filter_map(|r| r.ok())
            .collect();

        if all_ids.is_empty() {
            return self.compute_filtered_aggregates(query, &[]);
        }

        // Batch fetch all records (much faster than N individual get_latest calls)
        let batch_results = self.storage.get_latest_batch(&all_ids)?;

        // Filter the results in memory
        let matching_ids: Vec<[u8; 16]> = all_ids
            .into_iter()
            .zip(batch_results.into_iter())
            .filter_map(|(id, result)| {
                let (_, record) = result?;
                // Check if record is deleted (tombstone)
                if record.deleted {
                    return None;
                }
                // Deserialize and check filter using EntityRow with O(1) field lookups
                let fields = super::decode_entity(&record.data).ok()?;
                let entity = EntityRow::with_index(id, fields);
                if FilterEvaluator::evaluate(&filter.expression, &entity).ok()? {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        // Now compute aggregates over the filtered set
        self.compute_filtered_aggregates(query, &matching_ids)
    }

    /// Compute aggregates over a specific set of entity IDs.
    fn compute_filtered_aggregates(
        &self,
        query: &AggregateQuery,
        entity_ids: &[[u8; 16]],
    ) -> Result<AggregateResult, Error> {
        let projection = self.columnar.projection(&query.root_entity)?;
        let mut values = Vec::with_capacity(query.aggregations.len());

        for agg in &query.aggregations {
            let value = match agg.function {
                AggregateFunction::Count => {
                    if agg.field.is_some() {
                        // For COUNT(field), count entities that have non-null field
                        let field = agg.field.as_ref().unwrap();
                        let count = self.count_non_null_in_set(&projection, field, entity_ids)?;
                        Value::Int64(count as i64)
                    } else {
                        // COUNT(*) is just the number of matching entities
                        Value::Int64(entity_ids.len() as i64)
                    }
                }
                AggregateFunction::Sum => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("SUM requires a field".into())
                    })?;
                    let sum = self.sum_in_set(&projection, field, entity_ids)?;
                    Value::Float64(sum)
                }
                AggregateFunction::Avg => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("AVG requires a field".into())
                    })?;
                    let (sum, count) = self.sum_and_count_in_set(&projection, field, entity_ids)?;
                    if count > 0 {
                        Value::Float64(sum / count as f64)
                    } else {
                        Value::Null
                    }
                }
                AggregateFunction::Min => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("MIN requires a field".into())
                    })?;
                    self.min_in_set(&projection, field, entity_ids)?.unwrap_or(Value::Null)
                }
                AggregateFunction::Max => {
                    let field = agg.field.as_ref().ok_or_else(|| {
                        Error::InvalidData("MAX requires a field".into())
                    })?;
                    self.max_in_set(&projection, field, entity_ids)?.unwrap_or(Value::Null)
                }
            };

            values.push(AggregateValue::new(
                agg.function,
                agg.field.clone(),
                value,
            ));
        }

        Ok(AggregateResult::new(query.root_entity.clone(), values))
    }

    /// Count all entities of a type.
    fn count_entities(&self, entity_type: &str) -> Result<u64, Error> {
        let count = self.storage.list_entity_ids(entity_type).count();
        Ok(count as u64)
    }

    /// Count non-null values for a field in a set of entity IDs.
    fn count_non_null_in_set(
        &self,
        projection: &crate::storage::ColumnarProjection,
        field: &str,
        entity_ids: &[[u8; 16]],
    ) -> Result<u64, Error> {
        let mut count = 0u64;
        for id in entity_ids {
            if let Some(value) = projection.get_column(id, field)? {
                if !matches!(value, Value::Null) {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Sum values for a field in a set of entity IDs.
    fn sum_in_set(
        &self,
        projection: &crate::storage::ColumnarProjection,
        field: &str,
        entity_ids: &[[u8; 16]],
    ) -> Result<f64, Error> {
        let mut sum = 0.0;
        for id in entity_ids {
            if let Some(value) = projection.get_column(id, field)? {
                sum += value_to_f64(&value);
            }
        }
        Ok(sum)
    }

    /// Sum and count values for a field in a set of entity IDs.
    fn sum_and_count_in_set(
        &self,
        projection: &crate::storage::ColumnarProjection,
        field: &str,
        entity_ids: &[[u8; 16]],
    ) -> Result<(f64, u64), Error> {
        let mut sum = 0.0;
        let mut count = 0u64;
        for id in entity_ids {
            if let Some(value) = projection.get_column(id, field)? {
                if !matches!(value, Value::Null) {
                    sum += value_to_f64(&value);
                    count += 1;
                }
            }
        }
        Ok((sum, count))
    }

    /// Find minimum value for a field in a set of entity IDs.
    fn min_in_set(
        &self,
        projection: &crate::storage::ColumnarProjection,
        field: &str,
        entity_ids: &[[u8; 16]],
    ) -> Result<Option<Value>, Error> {
        let mut min_val: Option<Value> = None;
        for id in entity_ids {
            if let Some(value) = projection.get_column(id, field)? {
                if matches!(value, Value::Null) {
                    continue;
                }
                min_val = Some(match min_val {
                    None => value,
                    Some(current) => {
                        if compare_values(&value, &current) == std::cmp::Ordering::Less {
                            value
                        } else {
                            current
                        }
                    }
                });
            }
        }
        Ok(min_val)
    }

    /// Find maximum value for a field in a set of entity IDs.
    fn max_in_set(
        &self,
        projection: &crate::storage::ColumnarProjection,
        field: &str,
        entity_ids: &[[u8; 16]],
    ) -> Result<Option<Value>, Error> {
        let mut max_val: Option<Value> = None;
        for id in entity_ids {
            if let Some(value) = projection.get_column(id, field)? {
                if matches!(value, Value::Null) {
                    continue;
                }
                max_val = Some(match max_val {
                    None => value,
                    Some(current) => {
                        if compare_values(&value, &current) == std::cmp::Ordering::Greater {
                            value
                        } else {
                            current
                        }
                    }
                });
            }
        }
        Ok(max_val)
    }
}

/// Convert a Value to f64 for numeric aggregation.
fn value_to_f64(value: &Value) -> f64 {
    match value {
        Value::Int32(n) => *n as f64,
        Value::Int64(n) => *n as f64,
        Value::Float32(n) => *n as f64,
        Value::Float64(n) => *n,
        _ => 0.0,
    }
}

/// Compare two values for ordering.
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int32(a), Value::Int32(b)) => a.cmp(b),
        (Value::Int64(a), Value::Int64(b)) => a.cmp(b),
        (Value::Float32(a), Value::Float32(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float64(a), Value::Float64(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Uuid(a), Value::Uuid(b)) => a.cmp(b),
        // Mixed numeric types - convert to f64
        (a, b) => {
            let a_f64 = value_to_f64(a);
            let b_f64 = value_to_f64(b);
            a_f64.partial_cmp(&b_f64).unwrap_or(std::cmp::Ordering::Equal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_proto::{Aggregation, AggregateQuery};

    #[test]
    fn test_value_to_f64() {
        assert_eq!(value_to_f64(&Value::Int32(42)), 42.0);
        assert_eq!(value_to_f64(&Value::Int64(100)), 100.0);
        assert_eq!(value_to_f64(&Value::Float32(3.14)), 3.14f32 as f64);
        assert_eq!(value_to_f64(&Value::Float64(2.718)), 2.718);
        assert_eq!(value_to_f64(&Value::String("hello".into())), 0.0);
    }

    #[test]
    fn test_compare_values() {
        use std::cmp::Ordering;

        assert_eq!(
            compare_values(&Value::Int32(10), &Value::Int32(20)),
            Ordering::Less
        );
        assert_eq!(
            compare_values(&Value::Int64(30), &Value::Int64(30)),
            Ordering::Equal
        );
        assert_eq!(
            compare_values(&Value::Float64(5.0), &Value::Float64(3.0)),
            Ordering::Greater
        );
        assert_eq!(
            compare_values(&Value::String("abc".into()), &Value::String("xyz".into())),
            Ordering::Less
        );
    }

    #[test]
    fn test_aggregate_query_builder() {
        let query = AggregateQuery::new("User")
            .count()
            .sum("balance")
            .avg("age");

        assert_eq!(query.root_entity, "User");
        assert_eq!(query.aggregations.len(), 3);
        assert_eq!(query.aggregations[0].function, AggregateFunction::Count);
        assert!(query.aggregations[0].field.is_none());
        assert_eq!(query.aggregations[1].function, AggregateFunction::Sum);
        assert_eq!(query.aggregations[1].field, Some("balance".to_string()));
    }
}
