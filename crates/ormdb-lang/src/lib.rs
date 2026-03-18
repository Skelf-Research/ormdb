//! ORMDB Query Language
//!
//! This crate provides a parser and compiler for the ORMDB query language,
//! an ORM-style DSL for querying and mutating data.
//!
//! # Query Language Syntax
//!
//! ## Queries
//!
//! ```text
//! User.findMany()
//! User.findMany().where(status == "active")
//! User.findMany().where(age > 18 && status == "active")
//! User.findMany().include(posts).include(posts.comments)
//! User.findMany().orderBy(createdAt.desc).limit(10).offset(20)
//! User.findUnique().where(id == "uuid-here")
//! User.findFirst().where(email == "test@example.com")
//! ```
//!
//! ## Mutations
//!
//! ```text
//! User.create({ name: "Alice", email: "alice@example.com" })
//! User.update().where(id == "uuid").set({ status: "inactive" })
//! User.delete().where(id == "uuid")
//! User.upsert().where(id == "uuid").set({ name: "Bob" })
//! ```
//!
//! ## Schema Commands
//!
//! ```text
//! .schema
//! .schema User
//! .describe posts
//! .help
//! ```
//!
//! # Usage
//!
//! ```rust
//! use ormdb_lang::{parse, compile, parse_and_compile};
//!
//! // Parse and compile in one step
//! let result = parse_and_compile(r#"User.findMany().where(status == "active")"#);
//!
//! // Or parse and compile separately
//! let ast = parse(r#"User.findMany()"#).unwrap();
//! let ir = compile(ast).unwrap();
//! ```

pub mod ast;
pub mod compiler;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;

// Re-export main types
pub use ast::{
    ComparisonOp, FilterCondition, IncludeClause, Literal, Mutation, MutationClause, MutationKind,
    ObjectField, ObjectLiteral, OrderByClause, Query, QueryClause, QueryKind, SchemaCommand,
    SchemaCommandKind, SortDirection, Statement, WhereClause,
};
pub use compiler::{CompiledMutation, CompiledSchemaCommand, CompiledStatement};
pub use error::{CompileError, CompileErrorKind, LangError, ParseError};
pub use span::{Span, Spanned};

/// Parse a source string into an AST.
///
/// # Example
///
/// ```rust
/// use ormdb_lang::parse;
///
/// let stmt = parse("User.findMany()").unwrap();
/// ```
pub fn parse(source: &str) -> Result<Statement, ParseError> {
    parser::parse(source)
}

/// Compile an AST statement to IR.
///
/// # Example
///
/// ```rust
/// use ormdb_lang::{parse, compile};
///
/// let stmt = parse("User.findMany()").unwrap();
/// let ir = compile(stmt).unwrap();
/// ```
pub fn compile(stmt: Statement) -> Result<CompiledStatement, CompileError> {
    compiler::compile(stmt)
}

/// Parse and compile a source string in one step.
///
/// # Example
///
/// ```rust
/// use ormdb_lang::parse_and_compile;
///
/// let ir = parse_and_compile(r#"User.findMany().where(status == "active")"#).unwrap();
/// ```
pub fn parse_and_compile(source: &str) -> Result<CompiledStatement, LangError> {
    let stmt = parse(source)?;
    let compiled = compile(stmt)?;
    Ok(compiled)
}

/// Tokenize a source string (for debugging/testing).
///
/// # Example
///
/// ```rust
/// use ormdb_lang::tokenize;
///
/// let tokens = tokenize("User.findMany()");
/// assert!(!tokens.is_empty());
/// ```
pub fn tokenize(source: &str) -> Vec<lexer::SpannedToken> {
    lexer::tokenize(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_compile_query() {
        let result = parse_and_compile("User.findMany()").unwrap();
        assert!(matches!(result, CompiledStatement::Query(_)));
    }

    #[test]
    fn test_parse_and_compile_mutation() {
        let result =
            parse_and_compile(r#"User.create({ name: "Alice", email: "alice@example.com" })"#)
                .unwrap();
        assert!(matches!(
            result,
            CompiledStatement::Mutation(CompiledMutation::Insert(_))
        ));
    }

    #[test]
    fn test_parse_and_compile_schema_command() {
        let result = parse_and_compile(".schema").unwrap();
        assert!(matches!(
            result,
            CompiledStatement::SchemaCommand(CompiledSchemaCommand::ListEntities)
        ));
    }

    #[test]
    fn test_error_with_source_context() {
        let source = "User.findMany().where(status = \"active\")";
        let result = parse_and_compile(source);
        assert!(result.is_err());
        if let Err(e) = result {
            let formatted = e.format_with_source(source);
            assert!(formatted.contains("line 1"));
            assert!(formatted.contains("error"));
        }
    }

    #[test]
    fn test_complex_query() {
        let source = r#"
            User.findMany()
                .where(status == "active" && age > 18)
                .include(posts)
                .include(posts.comments)
                .orderBy(createdAt.desc)
                .limit(10)
                .offset(0)
        "#;
        let result = parse_and_compile(source).unwrap();
        if let CompiledStatement::Query(q) = result {
            assert_eq!(q.root_entity, "User");
            assert!(q.filter.is_some());
            assert_eq!(q.includes.len(), 2);
            assert_eq!(q.order_by.len(), 1);
            assert!(q.pagination.is_some());
        } else {
            panic!("expected Query");
        }
    }

    #[test]
    fn test_multiple_where_clauses() {
        // Multiple where clauses should be AND'd together
        let source = r#"User.findMany().where(status == "active").where(age > 18)"#;
        let result = parse_and_compile(source).unwrap();
        if let CompiledStatement::Query(q) = result {
            assert!(q.filter.is_some());
            // The filter should combine both conditions
            let filter = q.filter.unwrap();
            assert!(matches!(
                filter.expression,
                ormdb_proto::query::FilterExpr::And(_)
            ));
        }
    }

    #[test]
    fn test_all_comparison_operators() {
        let operators = [
            ("==", "Eq"),
            ("!=", "Ne"),
            ("<", "Lt"),
            ("<=", "Le"),
            (">", "Gt"),
            (">=", "Ge"),
        ];

        for (op, _name) in operators {
            let source = format!("User.findMany().where(age {} 18)", op);
            let result = parse_and_compile(&source);
            assert!(
                result.is_ok(),
                "Failed to parse operator {}: {:?}",
                op,
                result
            );
        }
    }

    #[test]
    fn test_update_with_where_and_set() {
        let source =
            r#"User.update().where(id == "123e4567-e89b-12d3-a456-426614174000").set({ name: "Bob", age: 30 })"#;
        let result = parse_and_compile(source).unwrap();
        if let CompiledStatement::Mutation(CompiledMutation::UpdateWithFilter {
            entity,
            filter,
            data,
        }) = result
        {
            assert_eq!(entity, "User");
            assert!(filter.is_some());
            assert_eq!(data.len(), 2);
        } else {
            panic!("expected UpdateWithFilter");
        }
    }
}
