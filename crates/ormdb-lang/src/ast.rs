//! Abstract Syntax Tree types for the query language.

use crate::span::{Span, Spanned};

/// A top-level statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// A query (findMany, findUnique, findFirst).
    Query(Query),
    /// A mutation (create, update, delete, upsert).
    Mutation(Mutation),
    /// A schema introspection command.
    SchemaCommand(SchemaCommand),
}

impl Statement {
    /// Get the span of this statement.
    pub fn span(&self) -> Span {
        match self {
            Statement::Query(q) => q.span,
            Statement::Mutation(m) => m.span,
            Statement::SchemaCommand(s) => s.span,
        }
    }
}

/// A query statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// The entity being queried.
    pub entity: Spanned<String>,
    /// The query kind (findMany, findUnique, findFirst).
    pub kind: QueryKind,
    /// Chained clauses (where, include, orderBy, etc.).
    pub clauses: Vec<QueryClause>,
    /// The full span of the query.
    pub span: Span,
}

/// Kind of query operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    /// Find multiple records.
    FindMany,
    /// Find a unique record (by unique fields).
    FindUnique,
    /// Find the first matching record.
    FindFirst,
    /// Count matching records.
    Count,
}

/// A clause in a query chain.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryClause {
    /// A where filter clause.
    Where(WhereClause),
    /// An include clause for relations.
    Include(IncludeClause),
    /// An orderBy clause.
    OrderBy(OrderByClause),
    /// A limit clause.
    Limit(Spanned<u32>),
    /// An offset clause.
    Offset(Spanned<u32>),
}

impl QueryClause {
    /// Get the span of this clause.
    pub fn span(&self) -> Span {
        match self {
            QueryClause::Where(w) => w.span,
            QueryClause::Include(i) => i.span,
            QueryClause::OrderBy(o) => o.span,
            QueryClause::Limit(l) => l.span,
            QueryClause::Offset(o) => o.span,
        }
    }
}

/// A where clause with a filter condition.
#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    /// The filter condition.
    pub condition: FilterCondition,
    /// Span of the where clause.
    pub span: Span,
}

/// A filter condition in a where clause.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterCondition {
    /// Comparison: field op value.
    Comparison {
        field: Spanned<String>,
        op: ComparisonOp,
        value: Spanned<Literal>,
    },
    /// IN check: field in [values].
    In {
        field: Spanned<String>,
        values: Vec<Spanned<Literal>>,
        negated: bool,
    },
    /// NULL check: field is null.
    IsNull {
        field: Spanned<String>,
        negated: bool,
    },
    /// LIKE pattern match.
    Like {
        field: Spanned<String>,
        pattern: Spanned<String>,
        negated: bool,
    },
    /// Logical AND of conditions.
    And(Vec<FilterCondition>),
    /// Logical OR of conditions.
    Or(Vec<FilterCondition>),
}

impl FilterCondition {
    /// Create an equality comparison.
    pub fn eq(field: Spanned<String>, value: Spanned<Literal>) -> Self {
        FilterCondition::Comparison {
            field,
            op: ComparisonOp::Eq,
            value,
        }
    }

    /// Create an AND of conditions.
    pub fn and(conditions: Vec<FilterCondition>) -> Self {
        if conditions.len() == 1 {
            conditions.into_iter().next().unwrap()
        } else {
            FilterCondition::And(conditions)
        }
    }

    /// Create an OR of conditions.
    pub fn or(conditions: Vec<FilterCondition>) -> Self {
        if conditions.len() == 1 {
            conditions.into_iter().next().unwrap()
        } else {
            FilterCondition::Or(conditions)
        }
    }
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    /// Equal (==).
    Eq,
    /// Not equal (!=).
    Ne,
    /// Less than (<).
    Lt,
    /// Less than or equal (<=).
    Le,
    /// Greater than (>).
    Gt,
    /// Greater than or equal (>=).
    Ge,
}

/// An include clause for loading relations.
#[derive(Debug, Clone, PartialEq)]
pub struct IncludeClause {
    /// The relation path (e.g., "posts" or "posts.comments").
    pub path: Spanned<String>,
    /// Span of the include clause.
    pub span: Span,
}

/// An orderBy clause.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByClause {
    /// The field to order by.
    pub field: Spanned<String>,
    /// The sort direction.
    pub direction: SortDirection,
    /// Span of the orderBy clause.
    pub span: Span,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order (default when not specified).
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Asc
    }
}

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
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

impl Literal {
    /// Get a description of the literal type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Literal::Null => "null",
            Literal::Bool(_) => "bool",
            Literal::Int(_) => "int",
            Literal::Float(_) => "float",
            Literal::String(_) => "string",
        }
    }
}

/// A mutation statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Mutation {
    /// The entity being mutated.
    pub entity: Spanned<String>,
    /// The mutation kind and data.
    pub kind: MutationKind,
    /// The full span of the mutation.
    pub span: Span,
}

/// Kind of mutation operation.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationKind {
    /// Create a new record.
    Create {
        /// The data to create with.
        data: ObjectLiteral,
    },
    /// Update existing records.
    Update {
        /// Clauses for the update (where, set).
        clauses: Vec<MutationClause>,
    },
    /// Delete records.
    Delete {
        /// Clauses for the delete (where).
        clauses: Vec<MutationClause>,
    },
    /// Upsert (update or insert).
    Upsert {
        /// Clauses for the upsert (where, set).
        clauses: Vec<MutationClause>,
    },
}

/// A clause in a mutation chain.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationClause {
    /// A where filter clause.
    Where(WhereClause),
    /// A set clause (for update/upsert).
    Set(ObjectLiteral),
}

impl MutationClause {
    /// Get the span of this clause.
    pub fn span(&self) -> Span {
        match self {
            MutationClause::Where(w) => w.span,
            MutationClause::Set(o) => o.span,
        }
    }
}

/// An object literal { key: value, ... }.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectLiteral {
    /// The fields of the object.
    pub fields: Vec<ObjectField>,
    /// Span of the object literal.
    pub span: Span,
}

/// A field in an object literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectField {
    /// The field name.
    pub name: Spanned<String>,
    /// The field value.
    pub value: Spanned<Literal>,
}

/// A schema introspection command.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaCommand {
    /// The kind of schema command.
    pub kind: SchemaCommandKind,
    /// Span of the command.
    pub span: Span,
}

/// Kind of schema command.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaCommandKind {
    /// List all entities (.schema).
    ListEntities,
    /// Describe a specific entity (.schema EntityName).
    DescribeEntity(Spanned<String>),
    /// Describe a relation (.describe relationName).
    DescribeRelation(Spanned<String>),
    /// Show help (.help).
    Help,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_condition_and_simplification() {
        let cond = FilterCondition::Comparison {
            field: Spanned::new("status".to_string(), Span::new(0, 6)),
            op: ComparisonOp::Eq,
            value: Spanned::new(Literal::String("active".to_string()), Span::new(10, 18)),
        };

        // Single condition should not wrap in And
        let and = FilterCondition::and(vec![cond.clone()]);
        assert!(matches!(and, FilterCondition::Comparison { .. }));

        // Multiple conditions should wrap
        let and = FilterCondition::and(vec![cond.clone(), cond]);
        assert!(matches!(and, FilterCondition::And(_)));
    }

    #[test]
    fn test_literal_type_names() {
        assert_eq!(Literal::Null.type_name(), "null");
        assert_eq!(Literal::Bool(true).type_name(), "bool");
        assert_eq!(Literal::Int(42).type_name(), "int");
        assert_eq!(Literal::Float(3.14).type_name(), "float");
        assert_eq!(Literal::String("hello".into()).type_name(), "string");
    }
}
