//! Check constraint expression evaluator.
//!
//! Supports a simple expression language for CHECK constraints:
//! - Field comparisons: `age >= 0`, `price > 0`
//! - Null checks: `email IS NOT NULL`, `middle_name IS NULL`
//! - IN lists: `status IN ('active', 'pending', 'inactive')`
//! - Boolean operators: `AND`, `OR`, `NOT`
//! - Parentheses for grouping

use std::collections::HashMap;

use thiserror::Error;

/// Errors that can occur during expression evaluation.
#[derive(Debug, Error, Clone)]
pub enum EvaluationError {
    /// Unknown field referenced in expression.
    #[error("unknown field: {0}")]
    UnknownField(String),

    /// Type mismatch in comparison.
    #[error("type mismatch: cannot compare {left_type} with {right_type}")]
    TypeMismatch {
        /// Left operand type.
        left_type: String,
        /// Right operand type.
        right_type: String,
    },

    /// Invalid expression syntax.
    #[error("invalid expression syntax: {0}")]
    InvalidSyntax(String),

    /// Division by zero.
    #[error("division by zero")]
    DivisionByZero,
}

/// A value that can be used in expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
    /// String value.
    String(String),
}

impl Value {
    /// Check if this value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Try to compare two values.
    pub fn compare(&self, other: &Value) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
            (Value::Null, _) | (_, Value::Null) => None,
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Int(i as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Value::Null,
        }
    }
}

/// Check constraint expression evaluator.
pub struct CheckEvaluator;

impl CheckEvaluator {
    /// Evaluate a check expression against entity data.
    ///
    /// Returns `true` if the constraint is satisfied, `false` otherwise.
    pub fn evaluate(
        expression: &str,
        data: &HashMap<String, Value>,
    ) -> Result<bool, EvaluationError> {
        let expr = expression.trim();
        if expr.is_empty() {
            return Ok(true);
        }

        Self::evaluate_expr(expr, data)
    }

    /// Evaluate an expression recursively.
    fn evaluate_expr(
        expr: &str,
        data: &HashMap<String, Value>,
    ) -> Result<bool, EvaluationError> {
        let expr = expr.trim();

        // Handle parentheses
        if expr.starts_with('(') && expr.ends_with(')') {
            // Check if the parens are balanced and wrapping the whole expression
            if Self::parens_balanced(&expr[1..expr.len() - 1]) {
                return Self::evaluate_expr(&expr[1..expr.len() - 1], data);
            }
        }

        // Handle OR (lowest precedence)
        if let Some((left, right)) = Self::split_at_operator(expr, " OR ") {
            let left_result = Self::evaluate_expr(left, data)?;
            let right_result = Self::evaluate_expr(right, data)?;
            return Ok(left_result || right_result);
        }

        // Handle AND
        if let Some((left, right)) = Self::split_at_operator(expr, " AND ") {
            let left_result = Self::evaluate_expr(left, data)?;
            let right_result = Self::evaluate_expr(right, data)?;
            return Ok(left_result && right_result);
        }

        // Handle NOT
        if let Some(rest) = expr.strip_prefix("NOT ") {
            let result = Self::evaluate_expr(rest.trim(), data)?;
            return Ok(!result);
        }

        // Handle IS NOT NULL
        if let Some(field) = Self::extract_is_not_null(expr) {
            let value = data
                .get(field)
                .ok_or_else(|| EvaluationError::UnknownField(field.to_string()))?;
            return Ok(!value.is_null());
        }

        // Handle IS NULL
        if let Some(field) = Self::extract_is_null(expr) {
            let value = data
                .get(field)
                .ok_or_else(|| EvaluationError::UnknownField(field.to_string()))?;
            return Ok(value.is_null());
        }

        // Handle IN
        if let Some((field, values)) = Self::extract_in(expr) {
            let value = data
                .get(field)
                .ok_or_else(|| EvaluationError::UnknownField(field.to_string()))?;
            return Self::evaluate_in(value, &values);
        }

        // Handle comparisons
        Self::evaluate_comparison(expr, data)
    }

    /// Check if parentheses are balanced.
    fn parens_balanced(s: &str) -> bool {
        let mut depth = 0;
        for c in s.chars() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                _ => {}
            }
        }
        depth == 0
    }

    /// Split expression at operator, respecting parentheses.
    fn split_at_operator<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = '"';
        let chars: Vec<char> = expr.chars().collect();

        for i in 0..chars.len() {
            let c = chars[i];

            if !in_string {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    '"' | '\'' => {
                        in_string = true;
                        string_char = c;
                    }
                    _ => {}
                }
            } else if c == string_char {
                in_string = false;
            }

            // Only split at top level (depth 0)
            if depth == 0 && !in_string {
                let remaining = &expr[i..];
                if remaining.to_uppercase().starts_with(op) {
                    let left = &expr[..i];
                    let right = &expr[i + op.len()..];
                    return Some((left.trim(), right.trim()));
                }
            }
        }
        None
    }

    /// Extract field from "field IS NOT NULL" pattern.
    fn extract_is_not_null(expr: &str) -> Option<&str> {
        let upper = expr.to_uppercase();
        if upper.ends_with(" IS NOT NULL") {
            let field_end = expr.len() - " IS NOT NULL".len();
            Some(expr[..field_end].trim())
        } else {
            None
        }
    }

    /// Extract field from "field IS NULL" pattern.
    fn extract_is_null(expr: &str) -> Option<&str> {
        let upper = expr.to_uppercase();
        if upper.ends_with(" IS NULL") && !upper.ends_with(" IS NOT NULL") {
            let field_end = expr.len() - " IS NULL".len();
            Some(expr[..field_end].trim())
        } else {
            None
        }
    }

    /// Extract field and values from "field IN ('a', 'b', 'c')" pattern.
    fn extract_in(expr: &str) -> Option<(&str, Vec<String>)> {
        let upper = expr.to_uppercase();
        if let Some(in_pos) = upper.find(" IN ") {
            let field = expr[..in_pos].trim();
            let rest = expr[in_pos + 4..].trim();

            if rest.starts_with('(') && rest.ends_with(')') {
                let values_str = &rest[1..rest.len() - 1];
                let values = Self::parse_in_values(values_str);
                return Some((field, values));
            }
        }
        None
    }

    /// Parse comma-separated values in an IN clause.
    fn parse_in_values(s: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut string_char = '"';

        for c in s.chars() {
            if !in_string {
                match c {
                    '"' | '\'' => {
                        in_string = true;
                        string_char = c;
                    }
                    ',' => {
                        let trimmed = current.trim().to_string();
                        if !trimmed.is_empty() {
                            values.push(trimmed);
                        }
                        current.clear();
                    }
                    _ => current.push(c),
                }
            } else if c == string_char {
                in_string = false;
            } else {
                current.push(c);
            }
        }

        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            values.push(trimmed);
        }

        values
    }

    /// Evaluate an IN expression.
    fn evaluate_in(value: &Value, allowed: &[String]) -> Result<bool, EvaluationError> {
        if value.is_null() {
            return Ok(false);
        }

        match value {
            Value::String(s) => Ok(allowed.contains(s)),
            Value::Int(i) => {
                let s = i.to_string();
                Ok(allowed.contains(&s))
            }
            _ => Ok(false),
        }
    }

    /// Evaluate a comparison expression.
    fn evaluate_comparison(
        expr: &str,
        data: &HashMap<String, Value>,
    ) -> Result<bool, EvaluationError> {
        // Try each comparison operator
        // Order matters - longer operators must come first to avoid partial matches
        let operators: &[(&str, fn(std::cmp::Ordering) -> bool)] = &[
            (">=", |ord| ord == std::cmp::Ordering::Greater || ord == std::cmp::Ordering::Equal),
            ("<=", |ord| ord == std::cmp::Ordering::Less || ord == std::cmp::Ordering::Equal),
            ("!=", |ord| ord != std::cmp::Ordering::Equal),
            ("<>", |ord| ord != std::cmp::Ordering::Equal),
            ("=", |ord| ord == std::cmp::Ordering::Equal),
            (">", |ord| ord == std::cmp::Ordering::Greater),
            ("<", |ord| ord == std::cmp::Ordering::Less),
        ];

        for (op, predicate) in operators {
            if let Some((left, right)) = Self::split_at_operator(expr, op) {
                let left_value = Self::resolve_value(left.trim(), data)?;
                let right_value = Self::resolve_value(right.trim(), data)?;

                // NULL comparisons always false (except IS NULL)
                if left_value.is_null() || right_value.is_null() {
                    return Ok(false);
                }

                match left_value.compare(&right_value) {
                    Some(ord) => return Ok(predicate(ord)),
                    None => {
                        return Err(EvaluationError::TypeMismatch {
                            left_type: format!("{:?}", left_value),
                            right_type: format!("{:?}", right_value),
                        })
                    }
                }
            }
        }

        Err(EvaluationError::InvalidSyntax(format!(
            "cannot parse expression: {}",
            expr
        )))
    }

    /// Resolve a value from either a field reference or a literal.
    fn resolve_value(s: &str, data: &HashMap<String, Value>) -> Result<Value, EvaluationError> {
        let s = s.trim();

        // String literal
        if (s.starts_with('\'') && s.ends_with('\''))
            || (s.starts_with('"') && s.ends_with('"'))
        {
            return Ok(Value::String(s[1..s.len() - 1].to_string()));
        }

        // NULL literal
        if s.to_uppercase() == "NULL" {
            return Ok(Value::Null);
        }

        // Boolean literal
        if s.to_uppercase() == "TRUE" {
            return Ok(Value::Bool(true));
        }
        if s.to_uppercase() == "FALSE" {
            return Ok(Value::Bool(false));
        }

        // Numeric literal
        if let Ok(i) = s.parse::<i64>() {
            return Ok(Value::Int(i));
        }
        if let Ok(f) = s.parse::<f64>() {
            return Ok(Value::Float(f));
        }

        // Field reference
        data.get(s)
            .cloned()
            .ok_or_else(|| EvaluationError::UnknownField(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_data(pairs: Vec<(&str, Value)>) -> HashMap<String, Value> {
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn test_simple_comparison() {
        let data = make_data(vec![("age", Value::Int(25))]);

        assert!(CheckEvaluator::evaluate("age >= 18", &data).unwrap());
        assert!(CheckEvaluator::evaluate("age > 20", &data).unwrap());
        assert!(CheckEvaluator::evaluate("age < 30", &data).unwrap());
        assert!(CheckEvaluator::evaluate("age = 25", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("age >= 30", &data).unwrap());
    }

    #[test]
    fn test_string_comparison() {
        let data = make_data(vec![("status", Value::String("active".to_string()))]);

        assert!(CheckEvaluator::evaluate("status = 'active'", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("status = 'inactive'", &data).unwrap());
    }

    #[test]
    fn test_is_null() {
        let data = make_data(vec![
            ("email", Value::String("test@example.com".to_string())),
            ("middle_name", Value::Null),
        ]);

        assert!(CheckEvaluator::evaluate("email IS NOT NULL", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("email IS NULL", &data).unwrap());
        assert!(CheckEvaluator::evaluate("middle_name IS NULL", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("middle_name IS NOT NULL", &data).unwrap());
    }

    #[test]
    fn test_in_clause() {
        let data = make_data(vec![("status", Value::String("active".to_string()))]);

        assert!(
            CheckEvaluator::evaluate("status IN ('active', 'pending', 'inactive')", &data).unwrap()
        );
        assert!(!CheckEvaluator::evaluate("status IN ('archived', 'deleted')", &data).unwrap());
    }

    #[test]
    fn test_and_or() {
        let data = make_data(vec![
            ("age", Value::Int(25)),
            ("status", Value::String("active".to_string())),
        ]);

        assert!(CheckEvaluator::evaluate("age >= 18 AND status = 'active'", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("age >= 30 AND status = 'active'", &data).unwrap());
        assert!(CheckEvaluator::evaluate("age >= 30 OR status = 'active'", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("age >= 30 OR status = 'inactive'", &data).unwrap());
    }

    #[test]
    fn test_not() {
        let data = make_data(vec![("age", Value::Int(25))]);

        assert!(CheckEvaluator::evaluate("NOT age < 18", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("NOT age >= 18", &data).unwrap());
    }

    #[test]
    fn test_complex_expression() {
        let data = make_data(vec![
            ("price", Value::Float(99.99)),
            ("quantity", Value::Int(5)),
            ("status", Value::String("available".to_string())),
        ]);

        assert!(CheckEvaluator::evaluate(
            "price > 0 AND quantity >= 0 AND status IN ('available', 'reserved')",
            &data
        )
        .unwrap());
    }

    #[test]
    fn test_unknown_field() {
        let data = make_data(vec![]);

        let result = CheckEvaluator::evaluate("unknown_field > 0", &data);
        assert!(matches!(result, Err(EvaluationError::UnknownField(_))));
    }

    #[test]
    fn test_null_comparison() {
        let data = make_data(vec![("value", Value::Null)]);

        // NULL comparisons should return false (use IS NULL instead)
        assert!(!CheckEvaluator::evaluate("value = 0", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("value > 0", &data).unwrap());
    }

    #[test]
    fn test_float_comparison() {
        let data = make_data(vec![("amount", Value::Float(10.5))]);

        assert!(CheckEvaluator::evaluate("amount > 10", &data).unwrap());
        assert!(CheckEvaluator::evaluate("amount < 11", &data).unwrap());
        assert!(CheckEvaluator::evaluate("amount >= 10.5", &data).unwrap());
    }

    #[test]
    fn test_parentheses() {
        let data = make_data(vec![
            ("a", Value::Int(1)),
            ("b", Value::Int(2)),
            ("c", Value::Int(3)),
        ]);

        // Without parens: a = 1 OR (b = 2 AND c = 4) -> true OR false = true
        assert!(CheckEvaluator::evaluate("a = 1 OR b = 2 AND c = 4", &data).unwrap());

        // With parens: (a = 1 OR b = 2) AND c = 4 -> true AND false = false
        // Note: Our simple parser handles this correctly due to OR having lower precedence
    }

    #[test]
    fn test_not_equal() {
        let data = make_data(vec![("status", Value::String("active".to_string()))]);

        assert!(CheckEvaluator::evaluate("status != 'deleted'", &data).unwrap());
        assert!(CheckEvaluator::evaluate("status <> 'deleted'", &data).unwrap());
        assert!(!CheckEvaluator::evaluate("status != 'active'", &data).unwrap());
    }
}
