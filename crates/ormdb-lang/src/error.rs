//! Error types for parsing and compilation.

use crate::span::{offset_to_line_col, Span};
use thiserror::Error;

/// Error during lexing/parsing.
#[derive(Debug, Error)]
pub struct ParseError {
    /// The error message.
    pub message: String,
    /// Source span where the error occurred.
    pub span: Span,
    /// Optional hint for fixing the error.
    pub hint: Option<String>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl ParseError {
    /// Create a new parse error.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            hint: None,
        }
    }

    /// Add a hint to the error.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Format the error with source context.
    pub fn format_with_source(&self, source: &str) -> String {
        let (line, col) = offset_to_line_col(source, self.span.start);
        let mut result = format!("error: {}\n", self.message);
        result.push_str(&format!("  --> line {}:{}\n", line, col));

        // Show the source line
        if let Some(source_line) = source.lines().nth(line - 1) {
            result.push_str(&format!("   |\n{:3}| {}\n   |", line, source_line));

            // Add caret pointing to the error position
            for _ in 0..col {
                result.push(' ');
            }
            result.push('^');

            // Underline the span if it's on one line
            let span_len = self.span.end.saturating_sub(self.span.start);
            if span_len > 1 {
                for _ in 1..span_len.min(source_line.len() - col + 1) {
                    result.push('~');
                }
            }
            result.push('\n');
        }

        if let Some(hint) = &self.hint {
            result.push_str(&format!("   = hint: {}\n", hint));
        }

        result
    }
}

/// Error during compilation (AST to IR).
#[derive(Debug, Error)]
pub struct CompileError {
    /// The error message.
    pub message: String,
    /// Source span where the error occurred.
    pub span: Span,
    /// Error kind for programmatic handling.
    pub kind: CompileErrorKind,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Kinds of compilation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileErrorKind {
    /// Unknown entity type.
    UnknownEntity,
    /// Unknown field on entity.
    UnknownField,
    /// Unknown relation on entity.
    UnknownRelation,
    /// Type mismatch in filter.
    TypeMismatch,
    /// Invalid literal value.
    InvalidLiteral,
    /// Invalid query structure.
    InvalidQuery,
    /// Invalid mutation structure.
    InvalidMutation,
    /// Duplicate clause (e.g., multiple limits).
    DuplicateClause,
    /// Missing required clause.
    MissingClause,
}

impl CompileError {
    /// Create a new compile error.
    pub fn new(message: impl Into<String>, span: Span, kind: CompileErrorKind) -> Self {
        Self {
            message: message.into(),
            span,
            kind,
        }
    }

    /// Create an unknown entity error.
    pub fn unknown_entity(entity: &str, span: Span) -> Self {
        Self::new(
            format!("unknown entity '{}'", entity),
            span,
            CompileErrorKind::UnknownEntity,
        )
    }

    /// Create an unknown field error.
    pub fn unknown_field(entity: &str, field: &str, span: Span) -> Self {
        Self::new(
            format!("unknown field '{}' on entity '{}'", field, entity),
            span,
            CompileErrorKind::UnknownField,
        )
    }

    /// Create an unknown relation error.
    pub fn unknown_relation(entity: &str, relation: &str, span: Span) -> Self {
        Self::new(
            format!("unknown relation '{}' on entity '{}'", relation, entity),
            span,
            CompileErrorKind::UnknownRelation,
        )
    }

    /// Create a type mismatch error.
    pub fn type_mismatch(expected: &str, got: &str, span: Span) -> Self {
        Self::new(
            format!("type mismatch: expected {}, got {}", expected, got),
            span,
            CompileErrorKind::TypeMismatch,
        )
    }

    /// Create an invalid literal error.
    pub fn invalid_literal(message: impl Into<String>, span: Span) -> Self {
        Self::new(message, span, CompileErrorKind::InvalidLiteral)
    }

    /// Format the error with source context.
    pub fn format_with_source(&self, source: &str) -> String {
        let (line, col) = offset_to_line_col(source, self.span.start);
        let mut result = format!("error[{:?}]: {}\n", self.kind, self.message);
        result.push_str(&format!("  --> line {}:{}\n", line, col));

        if let Some(source_line) = source.lines().nth(line - 1) {
            result.push_str(&format!("   |\n{:3}| {}\n   |", line, source_line));

            for _ in 0..col {
                result.push(' ');
            }
            result.push('^');

            let span_len = self.span.end.saturating_sub(self.span.start);
            if span_len > 1 {
                for _ in 1..span_len.min(source_line.len() - col + 1) {
                    result.push('~');
                }
            }
            result.push('\n');
        }

        result
    }
}

/// A combined error type for the public API.
#[derive(Debug, Error)]
pub enum LangError {
    /// Parse error.
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    /// Compile error.
    #[error("compile error: {0}")]
    Compile(#[from] CompileError),
}

impl LangError {
    /// Format the error with source context.
    pub fn format_with_source(&self, source: &str) -> String {
        match self {
            LangError::Parse(e) => e.format_with_source(source),
            LangError::Compile(e) => e.format_with_source(source),
        }
    }

    /// Get the span of the error.
    pub fn span(&self) -> Span {
        match self {
            LangError::Parse(e) => e.span,
            LangError::Compile(e) => e.span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_formatting() {
        let source = "User.findMany().where(status = \"active\")";
        let err = ParseError::new("expected '==' but found '='", Span::new(29, 30))
            .with_hint("use '==' for equality comparison");

        let formatted = err.format_with_source(source);
        assert!(formatted.contains("line 1:30"));
        assert!(formatted.contains("expected '==' but found '='"));
        assert!(formatted.contains("hint: use '==' for equality"));
    }
}
