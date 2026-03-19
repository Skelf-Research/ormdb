//! Filter evaluation for query execution.
//!
//! This module provides the `FilterEvaluator` that evaluates filter expressions
//! from the query IR against entity field values.

use std::collections::HashSet;

use crate::error::Error;
use ormdb_proto::{FilterExpr, SimpleFilter, Value};

/// Extract all field names referenced in a filter expression.
pub fn extract_filter_fields(filter: &FilterExpr) -> HashSet<String> {
    let mut fields = HashSet::new();
    extract_filter_fields_inner(filter, &mut fields);
    fields
}

fn extract_filter_fields_inner(filter: &FilterExpr, fields: &mut HashSet<String>) {
    match filter {
        FilterExpr::Eq { field, .. }
        | FilterExpr::Ne { field, .. }
        | FilterExpr::Lt { field, .. }
        | FilterExpr::Le { field, .. }
        | FilterExpr::Gt { field, .. }
        | FilterExpr::Ge { field, .. }
        | FilterExpr::In { field, .. }
        | FilterExpr::NotIn { field, .. }
        | FilterExpr::IsNull { field }
        | FilterExpr::IsNotNull { field }
        | FilterExpr::Like { field, .. }
        | FilterExpr::NotLike { field, .. } => {
            fields.insert(field.clone());
        }
        FilterExpr::And(simple_filters) | FilterExpr::Or(simple_filters) => {
            for sf in simple_filters {
                extract_simple_filter_fields(sf, fields);
            }
        }
    }
}

fn extract_simple_filter_fields(filter: &SimpleFilter, fields: &mut HashSet<String>) {
    match filter {
        SimpleFilter::Eq { field, .. }
        | SimpleFilter::Ne { field, .. }
        | SimpleFilter::Lt { field, .. }
        | SimpleFilter::Le { field, .. }
        | SimpleFilter::Gt { field, .. }
        | SimpleFilter::Ge { field, .. }
        | SimpleFilter::In { field, .. }
        | SimpleFilter::NotIn { field, .. }
        | SimpleFilter::IsNull { field }
        | SimpleFilter::IsNotNull { field }
        | SimpleFilter::Like { field, .. }
        | SimpleFilter::NotLike { field, .. } => {
            fields.insert(field.clone());
        }
    }
}

/// Evaluates filter expressions against entity data.
pub struct FilterEvaluator;

impl FilterEvaluator {
    /// Evaluate a filter expression against a row of field values.
    ///
    /// Returns `true` if the row matches the filter, `false` otherwise.
    pub fn evaluate(filter: &FilterExpr, row: &[(String, Value)]) -> Result<bool, Error> {
        match filter {
            FilterExpr::Eq { field, value } => {
                Self::compare_field(row, field, value, Self::values_equal)
            }
            FilterExpr::Ne { field, value } => {
                Self::compare_field(row, field, value, |a, b| !Self::values_equal(a, b))
            }
            FilterExpr::Lt { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_lt()).unwrap_or(false)
                })
            }
            FilterExpr::Le { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_le()).unwrap_or(false)
                })
            }
            FilterExpr::Gt { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_gt()).unwrap_or(false)
                })
            }
            FilterExpr::Ge { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_ge()).unwrap_or(false)
                })
            }
            FilterExpr::In { field, values } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(fv) => Ok(values.iter().any(|v| Self::values_equal(fv, v))),
                    None => Ok(false),
                }
            }
            FilterExpr::NotIn { field, values } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(fv) => Ok(!values.iter().any(|v| Self::values_equal(fv, v))),
                    None => Ok(true), // NULL is not in any set
                }
            }
            FilterExpr::IsNull { field } => {
                let field_value = Self::get_field_value(row, field);
                Ok(matches!(field_value, None | Some(Value::Null)))
            }
            FilterExpr::IsNotNull { field } => {
                let field_value = Self::get_field_value(row, field);
                Ok(!matches!(field_value, None | Some(Value::Null)))
            }
            FilterExpr::Like { field, pattern } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(Value::String(s)) => Ok(Self::like_match(s, pattern)),
                    _ => Ok(false),
                }
            }
            FilterExpr::NotLike { field, pattern } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(Value::String(s)) => Ok(!Self::like_match(s, pattern)),
                    _ => Ok(true),
                }
            }
            FilterExpr::And(filters) => {
                for f in filters {
                    if !Self::evaluate_simple(f, row)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            FilterExpr::Or(filters) => {
                for f in filters {
                    if Self::evaluate_simple(f, row)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// Evaluate a simple (non-compound) filter.
    fn evaluate_simple(filter: &SimpleFilter, row: &[(String, Value)]) -> Result<bool, Error> {
        match filter {
            SimpleFilter::Eq { field, value } => {
                Self::compare_field(row, field, value, Self::values_equal)
            }
            SimpleFilter::Ne { field, value } => {
                Self::compare_field(row, field, value, |a, b| !Self::values_equal(a, b))
            }
            SimpleFilter::Lt { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_lt()).unwrap_or(false)
                })
            }
            SimpleFilter::Le { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_le()).unwrap_or(false)
                })
            }
            SimpleFilter::Gt { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_gt()).unwrap_or(false)
                })
            }
            SimpleFilter::Ge { field, value } => {
                Self::compare_field(row, field, value, |a, b| {
                    Self::compare_values(a, b).map(|ord| ord.is_ge()).unwrap_or(false)
                })
            }
            SimpleFilter::In { field, values } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(fv) => Ok(values.iter().any(|v| Self::values_equal(fv, v))),
                    None => Ok(false),
                }
            }
            SimpleFilter::NotIn { field, values } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(fv) => Ok(!values.iter().any(|v| Self::values_equal(fv, v))),
                    None => Ok(true),
                }
            }
            SimpleFilter::IsNull { field } => {
                let field_value = Self::get_field_value(row, field);
                Ok(matches!(field_value, None | Some(Value::Null)))
            }
            SimpleFilter::IsNotNull { field } => {
                let field_value = Self::get_field_value(row, field);
                Ok(!matches!(field_value, None | Some(Value::Null)))
            }
            SimpleFilter::Like { field, pattern } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(Value::String(s)) => Ok(Self::like_match(s, pattern)),
                    _ => Ok(false),
                }
            }
            SimpleFilter::NotLike { field, pattern } => {
                let field_value = Self::get_field_value(row, field);
                match field_value {
                    Some(Value::String(s)) => Ok(!Self::like_match(s, pattern)),
                    _ => Ok(true),
                }
            }
        }
    }

    /// Get a field value from a row by name.
    fn get_field_value<'a>(row: &'a [(String, Value)], field: &str) -> Option<&'a Value> {
        row.iter().find(|(name, _)| name == field).map(|(_, v)| v)
    }

    /// Compare a field value with a comparator function.
    fn compare_field<F>(
        row: &[(String, Value)],
        field: &str,
        value: &Value,
        comparator: F,
    ) -> Result<bool, Error>
    where
        F: FnOnce(&Value, &Value) -> bool,
    {
        let field_value = Self::get_field_value(row, field);
        match field_value {
            Some(fv) => Ok(comparator(fv, value)),
            None => Ok(false), // Missing field doesn't match
        }
    }

    /// Check if two values are equal.
    fn values_equal(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int32(a), Value::Int32(b)) => a == b,
            (Value::Int64(a), Value::Int64(b)) => a == b,
            (Value::Int32(a), Value::Int64(b)) => (*a as i64) == *b,
            (Value::Int64(a), Value::Int32(b)) => *a == (*b as i64),
            (Value::Float32(a), Value::Float32(b)) => a == b,
            (Value::Float64(a), Value::Float64(b)) => a == b,
            (Value::Float32(a), Value::Float64(b)) => (*a as f64) == *b,
            (Value::Float64(a), Value::Float32(b)) => *a == (*b as f64),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Timestamp(a), Value::Timestamp(b)) => a == b,
            (Value::Uuid(a), Value::Uuid(b)) => a == b,
            _ => false,
        }
    }

    /// Compare two values, returning their ordering if comparable.
    fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Value::Int32(a), Value::Int32(b)) => Some(a.cmp(b)),
            (Value::Int64(a), Value::Int64(b)) => Some(a.cmp(b)),
            (Value::Int32(a), Value::Int64(b)) => Some((*a as i64).cmp(b)),
            (Value::Int64(a), Value::Int32(b)) => Some(a.cmp(&(*b as i64))),
            (Value::Float32(a), Value::Float32(b)) => a.partial_cmp(b),
            (Value::Float64(a), Value::Float64(b)) => a.partial_cmp(b),
            (Value::Float32(a), Value::Float64(b)) => (*a as f64).partial_cmp(b),
            (Value::Float64(a), Value::Float32(b)) => a.partial_cmp(&(*b as f64)),
            (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
            (Value::Timestamp(a), Value::Timestamp(b)) => Some(a.cmp(b)),
            (Value::Bytes(a), Value::Bytes(b)) => Some(a.cmp(b)),
            (Value::Uuid(a), Value::Uuid(b)) => Some(a.cmp(b)),
            _ => None, // Incompatible types
        }
    }

    /// Match a string against a SQL LIKE pattern.
    ///
    /// Supports:
    /// - `%` matches zero or more characters
    /// - `_` matches exactly one character
    /// - `\\%` matches literal `%`
    /// - `\\_` matches literal `_`
    pub fn like_match(value: &str, pattern: &str) -> bool {
        let mut chars = value.chars().peekable();
        let mut pattern_chars = pattern.chars().peekable();

        Self::like_match_recursive(&mut chars, &mut pattern_chars)
    }

    fn like_match_recursive(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        pattern: &mut std::iter::Peekable<std::str::Chars>,
    ) -> bool {
        loop {
            match (pattern.peek().copied(), chars.peek().copied()) {
                // End of both
                (None, None) => return true,
                // End of pattern but not value
                (None, Some(_)) => return false,
                // Percent matches zero or more characters
                (Some('%'), _) => {
                    pattern.next(); // consume %

                    // If % is at end of pattern, match rest of string
                    if pattern.peek().is_none() {
                        return true;
                    }

                    // Try matching % with 0, 1, 2, ... characters
                    loop {
                        // Clone iterators for backtracking
                        let mut pattern_clone = pattern.clone();
                        let mut chars_clone = chars.clone();

                        if Self::like_match_recursive(&mut chars_clone, &mut pattern_clone) {
                            return true;
                        }

                        // Consume one more character from value
                        if chars.next().is_none() {
                            return false;
                        }
                    }
                }
                // Underscore matches exactly one character
                (Some('_'), Some(_)) => {
                    pattern.next();
                    chars.next();
                }
                // Underscore but no character left
                (Some('_'), None) => return false,
                // Escape sequence
                (Some('\\'), _) => {
                    pattern.next(); // consume backslash
                    match (pattern.peek().copied(), chars.peek().copied()) {
                        (Some(p), Some(c)) if p == c => {
                            pattern.next();
                            chars.next();
                        }
                        _ => return false,
                    }
                }
                // Regular character match
                (Some(p), Some(c)) => {
                    if p == c {
                        pattern.next();
                        chars.next();
                    } else {
                        return false;
                    }
                }
                // Pattern character but no value character
                (Some(_), None) => return false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(fields: Vec<(&str, Value)>) -> Vec<(String, Value)> {
        fields.into_iter().map(|(n, v)| (n.to_string(), v)).collect()
    }

    #[test]
    fn test_eq_filter() {
        let row = make_row(vec![
            ("name", Value::String("Alice".into())),
            ("age", Value::Int32(30)),
        ]);

        // String equality
        let filter = FilterExpr::Eq {
            field: "name".into(),
            value: Value::String("Alice".into()),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Eq {
            field: "name".into(),
            value: Value::String("Bob".into()),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Integer equality
        let filter = FilterExpr::Eq {
            field: "age".into(),
            value: Value::Int32(30),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_ne_filter() {
        let row = make_row(vec![("age", Value::Int32(30))]);

        let filter = FilterExpr::Ne {
            field: "age".into(),
            value: Value::Int32(25),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Ne {
            field: "age".into(),
            value: Value::Int32(30),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_comparison_filters() {
        let row = make_row(vec![("score", Value::Int32(75))]);

        // Greater than
        let filter = FilterExpr::Gt {
            field: "score".into(),
            value: Value::Int32(50),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Gt {
            field: "score".into(),
            value: Value::Int32(75),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Greater or equal
        let filter = FilterExpr::Ge {
            field: "score".into(),
            value: Value::Int32(75),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Less than
        let filter = FilterExpr::Lt {
            field: "score".into(),
            value: Value::Int32(100),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Less or equal
        let filter = FilterExpr::Le {
            field: "score".into(),
            value: Value::Int32(75),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_in_filter() {
        let row = make_row(vec![("status", Value::String("active".into()))]);

        let filter = FilterExpr::In {
            field: "status".into(),
            values: vec![
                Value::String("active".into()),
                Value::String("pending".into()),
            ],
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::In {
            field: "status".into(),
            values: vec![
                Value::String("deleted".into()),
                Value::String("archived".into()),
            ],
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_not_in_filter() {
        let row = make_row(vec![("status", Value::String("active".into()))]);

        let filter = FilterExpr::NotIn {
            field: "status".into(),
            values: vec![
                Value::String("deleted".into()),
                Value::String("archived".into()),
            ],
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_is_null_filter() {
        let row_with_null = make_row(vec![("value", Value::Null)]);
        let row_without_null = make_row(vec![("value", Value::Int32(42))]);
        let row_missing_field = make_row(vec![("other", Value::Int32(1))]);

        let filter = FilterExpr::IsNull { field: "value".into() };
        assert!(FilterEvaluator::evaluate(&filter, &row_with_null).unwrap());
        assert!(!FilterEvaluator::evaluate(&filter, &row_without_null).unwrap());
        assert!(FilterEvaluator::evaluate(&filter, &row_missing_field).unwrap());
    }

    #[test]
    fn test_is_not_null_filter() {
        let row_with_value = make_row(vec![("value", Value::Int32(42))]);
        let row_with_null = make_row(vec![("value", Value::Null)]);

        let filter = FilterExpr::IsNotNull { field: "value".into() };
        assert!(FilterEvaluator::evaluate(&filter, &row_with_value).unwrap());
        assert!(!FilterEvaluator::evaluate(&filter, &row_with_null).unwrap());
    }

    #[test]
    fn test_like_filter_basic() {
        let row = make_row(vec![("name", Value::String("Alice".into()))]);

        // Exact match
        let filter = FilterExpr::Like {
            field: "name".into(),
            pattern: "Alice".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        // No match
        let filter = FilterExpr::Like {
            field: "name".into(),
            pattern: "Bob".into(),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_like_filter_percent() {
        let row = make_row(vec![("email", Value::String("alice@example.com".into()))]);

        // Prefix match
        let filter = FilterExpr::Like {
            field: "email".into(),
            pattern: "alice%".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Suffix match
        let filter = FilterExpr::Like {
            field: "email".into(),
            pattern: "%example.com".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Contains match
        let filter = FilterExpr::Like {
            field: "email".into(),
            pattern: "%@%".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        // Multiple %
        let filter = FilterExpr::Like {
            field: "email".into(),
            pattern: "%ice%exam%".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_like_filter_underscore() {
        let row = make_row(vec![("code", Value::String("A1B".into()))]);

        let filter = FilterExpr::Like {
            field: "code".into(),
            pattern: "A_B".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Like {
            field: "code".into(),
            pattern: "___".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Like {
            field: "code".into(),
            pattern: "__".into(),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_like_filter_escape() {
        let row = make_row(vec![("text", Value::String("100%".into()))]);

        let filter = FilterExpr::Like {
            field: "text".into(),
            pattern: "100\\%".into(),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_and_filter() {
        let row = make_row(vec![
            ("age", Value::Int32(25)),
            ("active", Value::Bool(true)),
        ]);

        let filter = FilterExpr::And(vec![
            SimpleFilter::Gt { field: "age".into(), value: Value::Int32(18) },
            SimpleFilter::Eq { field: "active".into(), value: Value::Bool(true) },
        ]);
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::And(vec![
            SimpleFilter::Gt { field: "age".into(), value: Value::Int32(30) },
            SimpleFilter::Eq { field: "active".into(), value: Value::Bool(true) },
        ]);
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_or_filter() {
        let row = make_row(vec![
            ("status", Value::String("pending".into())),
        ]);

        let filter = FilterExpr::Or(vec![
            SimpleFilter::Eq { field: "status".into(), value: Value::String("active".into()) },
            SimpleFilter::Eq { field: "status".into(), value: Value::String("pending".into()) },
        ]);
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Or(vec![
            SimpleFilter::Eq { field: "status".into(), value: Value::String("active".into()) },
            SimpleFilter::Eq { field: "status".into(), value: Value::String("archived".into()) },
        ]);
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_numeric_type_coercion() {
        let row = make_row(vec![("value", Value::Int64(100))]);

        // Int64 field, Int32 filter value
        let filter = FilterExpr::Eq {
            field: "value".into(),
            value: Value::Int32(100),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Gt {
            field: "value".into(),
            value: Value::Int32(50),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_missing_field() {
        let row = make_row(vec![("name", Value::String("Alice".into()))]);

        // Comparing against missing field returns false
        let filter = FilterExpr::Eq {
            field: "age".into(),
            value: Value::Int32(30),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_empty_and() {
        let row = make_row(vec![("x", Value::Int32(1))]);

        // Empty AND is true (all zero conditions are met)
        let filter = FilterExpr::And(vec![]);
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_empty_or() {
        let row = make_row(vec![("x", Value::Int32(1))]);

        // Empty OR is false (no conditions are met)
        let filter = FilterExpr::Or(vec![]);
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }

    #[test]
    fn test_uuid_comparison() {
        let uuid1 = [1u8; 16];
        let uuid2 = [2u8; 16];
        let row = make_row(vec![("id", Value::Uuid(uuid1))]);

        let filter = FilterExpr::Eq {
            field: "id".into(),
            value: Value::Uuid(uuid1),
        };
        assert!(FilterEvaluator::evaluate(&filter, &row).unwrap());

        let filter = FilterExpr::Eq {
            field: "id".into(),
            value: Value::Uuid(uuid2),
        };
        assert!(!FilterEvaluator::evaluate(&filter, &row).unwrap());
    }
}
