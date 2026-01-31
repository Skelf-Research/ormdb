//! Compiler from AST to IR types.

use crate::ast::*;
use crate::error::{CompileError, CompileErrorKind};
use crate::span::Span;
use ormdb_proto::mutation::{FieldValue, Mutation};
use ormdb_proto::query::{
    AggregateQuery, Filter, FilterExpr, GraphQuery, OrderDirection, OrderSpec, Pagination,
    RelationInclude, SimpleFilter,
};
use ormdb_proto::value::Value;

/// A compiled statement ready for execution.
#[derive(Debug, Clone, PartialEq)]
pub enum CompiledStatement {
    /// A compiled query.
    Query(GraphQuery),
    /// A compiled mutation.
    Mutation(CompiledMutation),
    /// A compiled aggregate query.
    Aggregate(AggregateQuery),
    /// A schema command (these are executed directly, not compiled to IR).
    SchemaCommand(CompiledSchemaCommand),
}

/// A compiled mutation with filter support.
///
/// Note: The ormdb-proto Mutation type requires concrete IDs, but our
/// language supports filter-based mutations. This type bridges that gap.
#[derive(Debug, Clone, PartialEq)]
pub enum CompiledMutation {
    /// Insert a new entity (maps directly to Mutation::Insert).
    Insert(Mutation),
    /// Update with a filter to select entities.
    UpdateWithFilter {
        entity: String,
        filter: Option<Filter>,
        data: Vec<FieldValue>,
    },
    /// Delete with a filter to select entities.
    DeleteWithFilter {
        entity: String,
        filter: Option<Filter>,
    },
    /// Upsert with a filter for the update part.
    UpsertWithFilter {
        entity: String,
        filter: Option<Filter>,
        data: Vec<FieldValue>,
    },
}

/// A compiled schema command.
#[derive(Debug, Clone, PartialEq)]
pub enum CompiledSchemaCommand {
    /// List all entities.
    ListEntities,
    /// Describe a specific entity.
    DescribeEntity(String),
    /// Describe a relation.
    DescribeRelation(String),
    /// Show help.
    Help,
}

/// Compiler for the query language.
pub struct Compiler;

impl Compiler {
    /// Compile a statement to IR.
    pub fn compile(stmt: Statement) -> Result<CompiledStatement, CompileError> {
        match stmt {
            Statement::Query(q) => {
                // Check if this is a count query - compile to AggregateQuery
                if q.kind == QueryKind::Count {
                    Self::compile_count_query(q).map(CompiledStatement::Aggregate)
                } else {
                    Self::compile_query(q).map(CompiledStatement::Query)
                }
            }
            Statement::Mutation(m) => Self::compile_mutation(m).map(CompiledStatement::Mutation),
            Statement::SchemaCommand(s) => {
                Self::compile_schema_command(s).map(CompiledStatement::SchemaCommand)
            }
        }
    }

    /// Compile a count query to AggregateQuery IR.
    fn compile_count_query(query: Query) -> Result<AggregateQuery, CompileError> {
        let mut aq = AggregateQuery::new(&query.entity.value).count();

        let mut filter_conditions: Vec<FilterCondition> = Vec::new();

        for clause in query.clauses {
            match clause {
                QueryClause::Where(w) => {
                    filter_conditions.push(w.condition);
                }
                QueryClause::Include(_) | QueryClause::OrderBy(_) | QueryClause::Limit(_) | QueryClause::Offset(_) => {
                    // These clauses don't make sense for count queries, ignore them
                    // In a stricter implementation, we could return an error
                }
            }
        }

        // Combine all where conditions with AND
        if !filter_conditions.is_empty() {
            let combined = FilterCondition::and(filter_conditions);
            aq.filter = Some(Self::compile_filter(&combined)?);
        }

        Ok(aq)
    }

    /// Compile a query to GraphQuery IR.
    fn compile_query(query: Query) -> Result<GraphQuery, CompileError> {
        let mut gq = GraphQuery::new(&query.entity.value);

        let mut filter_conditions: Vec<FilterCondition> = Vec::new();

        for clause in query.clauses {
            match clause {
                QueryClause::Where(w) => {
                    filter_conditions.push(w.condition);
                }
                QueryClause::Include(i) => {
                    gq.includes.push(Self::compile_include(i)?);
                }
                QueryClause::OrderBy(o) => {
                    gq.order_by.push(Self::compile_order_by(o)?);
                }
                QueryClause::Limit(l) => {
                    let pagination = gq.pagination.get_or_insert(Pagination::new(l.value, 0));
                    pagination.limit = l.value;
                }
                QueryClause::Offset(o) => {
                    let pagination = gq.pagination.get_or_insert(Pagination::new(u32::MAX, 0));
                    pagination.offset = o.value;
                }
            }
        }

        // Combine all where conditions with AND
        if !filter_conditions.is_empty() {
            let combined = FilterCondition::and(filter_conditions);
            gq.filter = Some(Self::compile_filter(&combined)?);
        }

        Ok(gq)
    }

    /// Compile an include clause.
    fn compile_include(include: IncludeClause) -> Result<RelationInclude, CompileError> {
        Ok(RelationInclude::new(&include.path.value))
    }

    /// Compile an orderBy clause.
    fn compile_order_by(order: OrderByClause) -> Result<OrderSpec, CompileError> {
        let direction = match order.direction {
            SortDirection::Asc => OrderDirection::Asc,
            SortDirection::Desc => OrderDirection::Desc,
        };
        Ok(OrderSpec {
            field: order.field.value,
            direction,
        })
    }

    /// Compile a filter condition to Filter IR.
    fn compile_filter(condition: &FilterCondition) -> Result<Filter, CompileError> {
        let expr = Self::compile_filter_expr(condition)?;
        Ok(Filter::new(expr))
    }

    /// Compile a filter condition to FilterExpr.
    fn compile_filter_expr(condition: &FilterCondition) -> Result<FilterExpr, CompileError> {
        match condition {
            FilterCondition::Comparison { field, op, value } => {
                let v = Self::compile_literal(&value.value, value.span)?;
                let f = &field.value;
                Ok(match op {
                    ComparisonOp::Eq => FilterExpr::eq(f, v),
                    ComparisonOp::Ne => FilterExpr::ne(f, v),
                    ComparisonOp::Lt => FilterExpr::lt(f, v),
                    ComparisonOp::Le => FilterExpr::le(f, v),
                    ComparisonOp::Gt => FilterExpr::gt(f, v),
                    ComparisonOp::Ge => FilterExpr::ge(f, v),
                })
            }
            FilterCondition::In {
                field,
                values,
                negated,
            } => {
                let compiled_values: Result<Vec<Value>, _> = values
                    .iter()
                    .map(|v| Self::compile_literal(&v.value, v.span))
                    .collect();
                let compiled_values = compiled_values?;

                if *negated {
                    Ok(FilterExpr::not_in_values(&field.value, compiled_values))
                } else {
                    Ok(FilterExpr::in_values(&field.value, compiled_values))
                }
            }
            FilterCondition::IsNull { field, negated } => {
                if *negated {
                    Ok(FilterExpr::is_not_null(&field.value))
                } else {
                    Ok(FilterExpr::is_null(&field.value))
                }
            }
            FilterCondition::Like {
                field,
                pattern,
                negated,
            } => {
                if *negated {
                    Ok(FilterExpr::NotLike {
                        field: field.value.clone(),
                        pattern: pattern.value.clone(),
                    })
                } else {
                    Ok(FilterExpr::like(&field.value, &pattern.value))
                }
            }
            FilterCondition::And(conditions) => {
                let simple_filters: Result<Vec<SimpleFilter>, _> = conditions
                    .iter()
                    .map(|c| Self::compile_to_simple_filter(c))
                    .collect();
                Ok(FilterExpr::and(simple_filters?))
            }
            FilterCondition::Or(conditions) => {
                let simple_filters: Result<Vec<SimpleFilter>, _> = conditions
                    .iter()
                    .map(|c| Self::compile_to_simple_filter(c))
                    .collect();
                Ok(FilterExpr::or(simple_filters?))
            }
        }
    }

    /// Compile a filter condition to SimpleFilter (for And/Or expressions).
    fn compile_to_simple_filter(condition: &FilterCondition) -> Result<SimpleFilter, CompileError> {
        match condition {
            FilterCondition::Comparison { field, op, value } => {
                let v = Self::compile_literal(&value.value, value.span)?;
                let f = &field.value;
                Ok(match op {
                    ComparisonOp::Eq => SimpleFilter::eq(f, v),
                    ComparisonOp::Ne => SimpleFilter::ne(f, v),
                    ComparisonOp::Lt => SimpleFilter::Lt {
                        field: f.clone(),
                        value: v,
                    },
                    ComparisonOp::Le => SimpleFilter::Le {
                        field: f.clone(),
                        value: v,
                    },
                    ComparisonOp::Gt => SimpleFilter::Gt {
                        field: f.clone(),
                        value: v,
                    },
                    ComparisonOp::Ge => SimpleFilter::Ge {
                        field: f.clone(),
                        value: v,
                    },
                })
            }
            FilterCondition::In {
                field,
                values,
                negated,
            } => {
                let compiled_values: Result<Vec<Value>, _> = values
                    .iter()
                    .map(|v| Self::compile_literal(&v.value, v.span))
                    .collect();
                let compiled_values = compiled_values?;

                if *negated {
                    Ok(SimpleFilter::NotIn {
                        field: field.value.clone(),
                        values: compiled_values,
                    })
                } else {
                    Ok(SimpleFilter::In {
                        field: field.value.clone(),
                        values: compiled_values,
                    })
                }
            }
            FilterCondition::IsNull { field, negated } => {
                if *negated {
                    Ok(SimpleFilter::is_not_null(&field.value))
                } else {
                    Ok(SimpleFilter::is_null(&field.value))
                }
            }
            FilterCondition::Like {
                field,
                pattern,
                negated,
            } => {
                if *negated {
                    Ok(SimpleFilter::NotLike {
                        field: field.value.clone(),
                        pattern: pattern.value.clone(),
                    })
                } else {
                    Ok(SimpleFilter::Like {
                        field: field.value.clone(),
                        pattern: pattern.value.clone(),
                    })
                }
            }
            FilterCondition::And(_) | FilterCondition::Or(_) => {
                // Nested And/Or not supported in SimpleFilter
                Err(CompileError::new(
                    "nested AND/OR not supported in filter expressions",
                    Span::default(),
                    CompileErrorKind::InvalidQuery,
                ))
            }
        }
    }

    /// Compile a mutation.
    fn compile_mutation(mutation: crate::ast::Mutation) -> Result<CompiledMutation, CompileError> {
        let entity = mutation.entity.value;

        match mutation.kind {
            MutationKind::Create { data } => {
                let field_values = Self::compile_object_literal(&data)?;
                Ok(CompiledMutation::Insert(Mutation::insert(
                    entity,
                    field_values,
                )))
            }
            MutationKind::Update { clauses } => {
                let (filter, data) = Self::extract_mutation_clauses(clauses)?;
                Ok(CompiledMutation::UpdateWithFilter {
                    entity,
                    filter,
                    data,
                })
            }
            MutationKind::Delete { clauses } => {
                let (filter, _) = Self::extract_mutation_clauses(clauses)?;
                Ok(CompiledMutation::DeleteWithFilter { entity, filter })
            }
            MutationKind::Upsert { clauses } => {
                let (filter, data) = Self::extract_mutation_clauses(clauses)?;
                Ok(CompiledMutation::UpsertWithFilter {
                    entity,
                    filter,
                    data,
                })
            }
        }
    }

    /// Extract filter and data from mutation clauses.
    fn extract_mutation_clauses(
        clauses: Vec<MutationClause>,
    ) -> Result<(Option<Filter>, Vec<FieldValue>), CompileError> {
        let mut filter: Option<Filter> = None;
        let mut data = Vec::new();

        for clause in clauses {
            match clause {
                MutationClause::Where(w) => {
                    let f = Self::compile_filter(&w.condition)?;
                    filter = Some(f);
                }
                MutationClause::Set(obj) => {
                    data = Self::compile_object_literal(&obj)?;
                }
            }
        }

        Ok((filter, data))
    }

    /// Compile an object literal to field values.
    fn compile_object_literal(obj: &ObjectLiteral) -> Result<Vec<FieldValue>, CompileError> {
        obj.fields
            .iter()
            .map(|f| {
                let value = Self::compile_literal(&f.value.value, f.value.span)?;
                Ok(FieldValue::new(&f.name.value, value))
            })
            .collect()
    }

    /// Compile a literal to a Value.
    fn compile_literal(literal: &Literal, _span: Span) -> Result<Value, CompileError> {
        match literal {
            Literal::Null => Ok(Value::Null),
            Literal::Bool(b) => Ok(Value::Bool(*b)),
            Literal::Int(i) => {
                // Check if it fits in i32
                if *i >= i32::MIN as i64 && *i <= i32::MAX as i64 {
                    Ok(Value::Int32(*i as i32))
                } else {
                    Ok(Value::Int64(*i))
                }
            }
            Literal::Float(f) => Ok(Value::Float64(*f)),
            Literal::String(s) => {
                // Check if it looks like a UUID
                if Self::is_uuid_string(s) {
                    if let Some(uuid_bytes) = Self::parse_uuid(s) {
                        return Ok(Value::Uuid(uuid_bytes));
                    }
                }
                Ok(Value::String(s.clone()))
            }
        }
    }

    /// Check if a string looks like a UUID.
    fn is_uuid_string(s: &str) -> bool {
        // UUID format: 8-4-4-4-12 hex digits
        s.len() == 36 && s.chars().filter(|c| *c == '-').count() == 4
    }

    /// Try to parse a UUID string to bytes.
    fn parse_uuid(s: &str) -> Option<[u8; 16]> {
        let hex: String = s.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 {
            return None;
        }

        let mut bytes = [0u8; 16];
        for i in 0..16 {
            let byte_str = &hex[i * 2..i * 2 + 2];
            bytes[i] = u8::from_str_radix(byte_str, 16).ok()?;
        }

        Some(bytes)
    }

    /// Compile a schema command.
    fn compile_schema_command(
        cmd: SchemaCommand,
    ) -> Result<CompiledSchemaCommand, CompileError> {
        match cmd.kind {
            SchemaCommandKind::ListEntities => Ok(CompiledSchemaCommand::ListEntities),
            SchemaCommandKind::DescribeEntity(e) => {
                Ok(CompiledSchemaCommand::DescribeEntity(e.value))
            }
            SchemaCommandKind::DescribeRelation(r) => {
                Ok(CompiledSchemaCommand::DescribeRelation(r.value))
            }
            SchemaCommandKind::Help => Ok(CompiledSchemaCommand::Help),
        }
    }
}

/// Compile a statement to IR.
pub fn compile(stmt: Statement) -> Result<CompiledStatement, CompileError> {
    Compiler::compile(stmt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn parse_and_compile(source: &str) -> Result<CompiledStatement, CompileError> {
        let stmt = parse(source).map_err(|e| {
            CompileError::new(e.message, e.span, CompileErrorKind::InvalidQuery)
        })?;
        compile(stmt)
    }

    #[test]
    fn test_compile_simple_query() {
        let result = parse_and_compile("User.findMany()").unwrap();
        if let CompiledStatement::Query(q) = result {
            assert_eq!(q.root_entity, "User");
            assert!(q.filter.is_none());
            assert!(q.includes.is_empty());
        } else {
            panic!("expected Query");
        }
    }

    #[test]
    fn test_compile_query_with_filter() {
        let result = parse_and_compile(r#"User.findMany().where(status == "active")"#).unwrap();
        if let CompiledStatement::Query(q) = result {
            assert!(q.filter.is_some());
            let filter = q.filter.unwrap();
            if let FilterExpr::Eq { field, value } = filter.expression {
                assert_eq!(field, "status");
                assert_eq!(value, Value::String("active".to_string()));
            } else {
                panic!("expected Eq filter");
            }
        }
    }

    #[test]
    fn test_compile_query_with_and_filter() {
        let result =
            parse_and_compile(r#"User.findMany().where(status == "active" && age > 18)"#).unwrap();
        if let CompiledStatement::Query(q) = result {
            if let FilterExpr::And(conditions) = &q.filter.unwrap().expression {
                assert_eq!(conditions.len(), 2);
            } else {
                panic!("expected And filter");
            }
        }
    }

    #[test]
    fn test_compile_query_with_includes() {
        let result = parse_and_compile("User.findMany().include(posts).include(posts.comments)").unwrap();
        if let CompiledStatement::Query(q) = result {
            assert_eq!(q.includes.len(), 2);
            assert_eq!(q.includes[0].path, "posts");
            assert_eq!(q.includes[1].path, "posts.comments");
        }
    }

    #[test]
    fn test_compile_query_with_orderby() {
        let result = parse_and_compile("User.findMany().orderBy(createdAt.desc)").unwrap();
        if let CompiledStatement::Query(q) = result {
            assert_eq!(q.order_by.len(), 1);
            assert_eq!(q.order_by[0].field, "createdAt");
            assert_eq!(q.order_by[0].direction, OrderDirection::Desc);
        }
    }

    #[test]
    fn test_compile_query_with_pagination() {
        let result = parse_and_compile("User.findMany().limit(10).offset(20)").unwrap();
        if let CompiledStatement::Query(q) = result {
            let pagination = q.pagination.unwrap();
            assert_eq!(pagination.limit, 10);
            assert_eq!(pagination.offset, 20);
        }
    }

    #[test]
    fn test_compile_create_mutation() {
        let result =
            parse_and_compile(r#"User.create({ name: "Alice", email: "alice@example.com" })"#)
                .unwrap();
        if let CompiledStatement::Mutation(CompiledMutation::Insert(m)) = result {
            if let Mutation::Insert { entity, data } = m {
                assert_eq!(entity, "User");
                assert_eq!(data.len(), 2);
                assert_eq!(data[0].field, "name");
                assert_eq!(data[0].value, Value::String("Alice".to_string()));
            } else {
                panic!("expected Insert");
            }
        } else {
            panic!("expected Insert mutation");
        }
    }

    #[test]
    fn test_compile_update_mutation() {
        let result = parse_and_compile(
            r#"User.update().where(id == "123e4567-e89b-12d3-a456-426614174000").set({ status: "inactive" })"#,
        )
        .unwrap();
        if let CompiledStatement::Mutation(CompiledMutation::UpdateWithFilter {
            entity,
            filter,
            data,
        }) = result
        {
            assert_eq!(entity, "User");
            assert!(filter.is_some());
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].field, "status");
        } else {
            panic!("expected UpdateWithFilter");
        }
    }

    #[test]
    fn test_compile_delete_mutation() {
        let result = parse_and_compile(r#"User.delete().where(id == "uuid")"#).unwrap();
        if let CompiledStatement::Mutation(CompiledMutation::DeleteWithFilter { entity, filter }) =
            result
        {
            assert_eq!(entity, "User");
            assert!(filter.is_some());
        }
    }

    #[test]
    fn test_compile_schema_command() {
        let result = parse_and_compile(".schema").unwrap();
        assert!(matches!(
            result,
            CompiledStatement::SchemaCommand(CompiledSchemaCommand::ListEntities)
        ));

        let result = parse_and_compile(".schema User").unwrap();
        if let CompiledStatement::SchemaCommand(CompiledSchemaCommand::DescribeEntity(e)) = result {
            assert_eq!(e, "User");
        }
    }

    #[test]
    fn test_uuid_parsing() {
        let uuid_str = "123e4567-e89b-12d3-a456-426614174000";
        let bytes = Compiler::parse_uuid(uuid_str).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes[0], 0x12);
        assert_eq!(bytes[1], 0x3e);
    }

    #[test]
    fn test_compile_in_filter() {
        let result =
            parse_and_compile(r#"User.findMany().where(status in ["active", "pending"])"#).unwrap();
        if let CompiledStatement::Query(q) = result {
            if let FilterExpr::In { field, values } = &q.filter.unwrap().expression {
                assert_eq!(field, "status");
                assert_eq!(values.len(), 2);
            } else {
                panic!("expected In filter");
            }
        }
    }

    #[test]
    fn test_compile_is_null() {
        let result = parse_and_compile("User.findMany().where(deletedAt is null)").unwrap();
        if let CompiledStatement::Query(q) = result {
            assert!(matches!(
                q.filter.unwrap().expression,
                FilterExpr::IsNull { .. }
            ));
        }
    }

    #[test]
    fn test_compile_like() {
        let result = parse_and_compile(r#"User.findMany().where(name like "Al%")"#).unwrap();
        if let CompiledStatement::Query(q) = result {
            if let FilterExpr::Like { field, pattern } = &q.filter.unwrap().expression {
                assert_eq!(field, "name");
                assert_eq!(pattern, "Al%");
            }
        }
    }

    #[test]
    fn test_integer_types() {
        // Small integer should be Int32
        let result = parse_and_compile("User.findMany().where(age == 25)").unwrap();
        if let CompiledStatement::Query(q) = result {
            if let FilterExpr::Eq { value, .. } = &q.filter.unwrap().expression {
                assert_eq!(*value, Value::Int32(25));
            }
        }

        // Large integer should be Int64
        let result = parse_and_compile("User.findMany().where(bignum == 9999999999)").unwrap();
        if let CompiledStatement::Query(q) = result {
            if let FilterExpr::Eq { value, .. } = &q.filter.unwrap().expression {
                assert_eq!(*value, Value::Int64(9999999999));
            }
        }
    }

    #[test]
    fn test_compile_count_query() {
        let result = parse_and_compile("User.count()").unwrap();
        if let CompiledStatement::Aggregate(aq) = result {
            assert_eq!(aq.root_entity, "User");
            assert_eq!(aq.aggregations.len(), 1);
            assert!(aq.filter.is_none());
        } else {
            panic!("expected Aggregate");
        }
    }

    #[test]
    fn test_compile_count_query_with_filter() {
        let result = parse_and_compile(r#"User.count().where(status == "active")"#).unwrap();
        if let CompiledStatement::Aggregate(aq) = result {
            assert_eq!(aq.root_entity, "User");
            assert!(aq.filter.is_some());
        } else {
            panic!("expected Aggregate");
        }
    }
}
