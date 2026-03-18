//! Recursive descent parser for the query language.

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Lexer, SpannedToken, Token};
use crate::span::{Span, Spanned};

/// Parser for the ORMDB query language.
pub struct Parser<'source> {
    lexer: Lexer<'source>,
    source: &'source str,
}

impl<'source> Parser<'source> {
    /// Create a new parser for the given source.
    pub fn new(source: &'source str) -> Self {
        Self {
            lexer: Lexer::new(source),
            source,
        }
    }

    /// Parse a complete statement.
    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        // Check for schema commands first
        if let Some(tok) = self.lexer.peek() {
            match &tok.token {
                Token::DotSchema => return self.parse_schema_command(),
                Token::DotDescribe => return self.parse_schema_command(),
                Token::DotHelp => return self.parse_schema_command(),
                _ => {}
            }
        }

        // Parse entity name
        let entity = self.expect_ident()?;
        self.expect_token(Token::Dot)?;

        // Parse operation
        let op_token = self.next_token()?;
        let start_span = entity.span;

        match op_token.token {
            Token::FindMany | Token::FindUnique | Token::FindFirst | Token::Count => {
                self.parse_query(entity, op_token)
            }
            Token::Create => self.parse_create_mutation(entity, start_span),
            Token::Update | Token::Delete | Token::Upsert => {
                self.parse_mutation(entity, op_token)
            }
            _ => Err(ParseError::new(
                format!("expected query or mutation method, found {:?}", op_token.token),
                op_token.span,
            )),
        }
    }

    /// Parse a query (findMany, findUnique, findFirst).
    fn parse_query(
        &mut self,
        entity: Spanned<String>,
        op_token: SpannedToken,
    ) -> Result<Statement, ParseError> {
        let kind = match op_token.token {
            Token::FindMany => QueryKind::FindMany,
            Token::FindUnique => QueryKind::FindUnique,
            Token::FindFirst => QueryKind::FindFirst,
            Token::Count => QueryKind::Count,
            _ => unreachable!(),
        };

        // Expect opening paren
        self.expect_token(Token::LParen)?;
        self.expect_token(Token::RParen)?;

        // Parse chained clauses
        let mut clauses = Vec::new();
        let mut end_span = op_token.span;
        let start_span = entity.span;

        while let Some(tok) = self.lexer.peek() {
            if tok.token != Token::Dot {
                break;
            }
            self.next_token()?; // consume dot

            let clause_tok = self.next_token()?;
            let clause = match clause_tok.token {
                Token::Where => QueryClause::Where(self.parse_where_clause(clause_tok.span)?),
                Token::Include => QueryClause::Include(self.parse_include_clause(clause_tok.span)?),
                Token::OrderBy => QueryClause::OrderBy(self.parse_orderby_clause(clause_tok.span)?),
                Token::Limit => QueryClause::Limit(self.parse_limit_clause()?),
                Token::Offset => QueryClause::Offset(self.parse_offset_clause()?),
                _ => {
                    return Err(ParseError::new(
                        format!("unexpected clause {:?}", clause_tok.token),
                        clause_tok.span,
                    ))
                }
            };
            end_span = clause.span();
            clauses.push(clause);
        }

        Ok(Statement::Query(Query {
            entity,
            kind,
            clauses,
            span: start_span.merge(end_span),
        }))
    }

    /// Parse a where clause.
    fn parse_where_clause(&mut self, start_span: Span) -> Result<WhereClause, ParseError> {
        self.expect_token(Token::LParen)?;
        let condition = self.parse_filter_condition()?;
        let end = self.expect_token(Token::RParen)?;

        Ok(WhereClause {
            condition,
            span: start_span.merge(end.span),
        })
    }

    /// Parse a filter condition (with support for && and ||).
    fn parse_filter_condition(&mut self) -> Result<FilterCondition, ParseError> {
        self.parse_or_condition()
    }

    /// Parse OR conditions.
    fn parse_or_condition(&mut self) -> Result<FilterCondition, ParseError> {
        let mut left = self.parse_and_condition()?;

        while let Some(tok) = self.lexer.peek() {
            if tok.token != Token::Or {
                break;
            }
            self.next_token()?; // consume ||

            let right = self.parse_and_condition()?;
            left = match left {
                FilterCondition::Or(mut conditions) => {
                    conditions.push(right);
                    FilterCondition::Or(conditions)
                }
                _ => FilterCondition::Or(vec![left, right]),
            };
        }

        Ok(left)
    }

    /// Parse AND conditions.
    fn parse_and_condition(&mut self) -> Result<FilterCondition, ParseError> {
        let mut left = self.parse_primary_condition()?;

        while let Some(tok) = self.lexer.peek() {
            if tok.token != Token::And {
                break;
            }
            self.next_token()?; // consume &&

            let right = self.parse_primary_condition()?;
            left = match left {
                FilterCondition::And(mut conditions) => {
                    conditions.push(right);
                    FilterCondition::And(conditions)
                }
                _ => FilterCondition::And(vec![left, right]),
            };
        }

        Ok(left)
    }

    /// Parse a primary filter condition (comparison, is null, in, like).
    fn parse_primary_condition(&mut self) -> Result<FilterCondition, ParseError> {
        let field = self.expect_ident()?;

        // Check for the operator
        let op_tok = self.lexer.peek().ok_or_else(|| {
            ParseError::new("unexpected end of input, expected operator", field.span)
        })?;

        match &op_tok.token {
            // Comparison operators
            Token::Eq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge => {
                let op = match self.next_token()?.token {
                    Token::Eq => ComparisonOp::Eq,
                    Token::Ne => ComparisonOp::Ne,
                    Token::Lt => ComparisonOp::Lt,
                    Token::Le => ComparisonOp::Le,
                    Token::Gt => ComparisonOp::Gt,
                    Token::Ge => ComparisonOp::Ge,
                    _ => unreachable!(),
                };
                let value = self.parse_literal()?;
                Ok(FilterCondition::Comparison { field, op, value })
            }

            // IS NULL / IS NOT NULL
            Token::Is => {
                self.next_token()?; // consume 'is'

                let negated = if let Some(tok) = self.lexer.peek() {
                    if tok.token == Token::Not {
                        self.next_token()?;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                let null_tok = self.next_token()?;
                if null_tok.token != Token::Null {
                    return Err(ParseError::new(
                        format!("expected 'null' after 'is', found {:?}", null_tok.token),
                        null_tok.span,
                    ));
                }

                Ok(FilterCondition::IsNull { field, negated })
            }

            // IN / NOT IN
            Token::In => {
                self.next_token()?; // consume 'in'
                let values = self.parse_array_literal()?;
                Ok(FilterCondition::In {
                    field,
                    values,
                    negated: false,
                })
            }

            Token::Not => {
                self.next_token()?; // consume 'not'
                let next = self.next_token()?;
                match next.token {
                    Token::In => {
                        let values = self.parse_array_literal()?;
                        Ok(FilterCondition::In {
                            field,
                            values,
                            negated: true,
                        })
                    }
                    Token::Like => {
                        let pattern = self.parse_string_literal()?;
                        Ok(FilterCondition::Like {
                            field,
                            pattern,
                            negated: true,
                        })
                    }
                    _ => Err(ParseError::new(
                        format!("expected 'in' or 'like' after 'not', found {:?}", next.token),
                        next.span,
                    )),
                }
            }

            // LIKE
            Token::Like => {
                self.next_token()?; // consume 'like'
                let pattern = self.parse_string_literal()?;
                Ok(FilterCondition::Like {
                    field,
                    pattern,
                    negated: false,
                })
            }

            _ => Err(ParseError::new(
                format!("expected comparison operator, found {:?}", op_tok.token),
                op_tok.span,
            )),
        }
    }

    /// Parse an array literal [value, ...].
    fn parse_array_literal(&mut self) -> Result<Vec<Spanned<Literal>>, ParseError> {
        self.expect_token(Token::LBracket)?;
        let mut values = Vec::new();

        // Handle empty array
        if let Some(tok) = self.lexer.peek() {
            if tok.token == Token::RBracket {
                self.next_token()?;
                return Ok(values);
            }
        }

        // Parse first value
        values.push(self.parse_literal()?);

        // Parse remaining values
        while let Some(tok) = self.lexer.peek() {
            if tok.token == Token::RBracket {
                break;
            }
            self.expect_token(Token::Comma)?;
            values.push(self.parse_literal()?);
        }

        self.expect_token(Token::RBracket)?;
        Ok(values)
    }

    /// Parse an include clause.
    fn parse_include_clause(&mut self, start_span: Span) -> Result<IncludeClause, ParseError> {
        self.expect_token(Token::LParen)?;

        // Parse the relation path (can include dots)
        let first_ident = self.expect_ident()?;
        let mut path = first_ident.value;
        let mut end_span = first_ident.span;

        // Handle dot-separated paths like posts.comments
        while let Some(tok) = self.lexer.peek() {
            if tok.token != Token::Dot {
                break;
            }
            // Check if next after dot is an ident (could be method call)
            let dot_span = self.next_token()?.span;
            if let Some(next) = self.lexer.peek() {
                if let Token::Ident(name) = &next.token {
                    // Check it's not a method name
                    if !is_method_name(name) {
                        let ident = self.next_token()?;
                        if let Token::Ident(name) = ident.token {
                            path.push('.');
                            path.push_str(&name);
                            end_span = ident.span;
                            continue;
                        }
                    }
                }
            }
            // Put the dot back conceptually - we need to stop here
            // Actually we can't put it back, so let's handle this differently
            return Err(ParseError::new(
                "unexpected dot in include path",
                dot_span,
            ));
        }

        let close = self.expect_token(Token::RParen)?;

        Ok(IncludeClause {
            path: Spanned::new(path, first_ident.span.merge(end_span)),
            span: start_span.merge(close.span),
        })
    }

    /// Parse an orderBy clause.
    fn parse_orderby_clause(&mut self, start_span: Span) -> Result<OrderByClause, ParseError> {
        self.expect_token(Token::LParen)?;

        let field = self.expect_ident()?;

        // Check for .asc or .desc
        let direction = if let Some(tok) = self.lexer.peek() {
            if tok.token == Token::Dot {
                self.next_token()?; // consume dot
                let dir_tok = self.next_token()?;
                match dir_tok.token {
                    Token::Asc => SortDirection::Asc,
                    Token::Desc => SortDirection::Desc,
                    _ => {
                        return Err(ParseError::new(
                            format!("expected 'asc' or 'desc', found {:?}", dir_tok.token),
                            dir_tok.span,
                        ))
                    }
                }
            } else {
                SortDirection::default()
            }
        } else {
            SortDirection::default()
        };

        let close = self.expect_token(Token::RParen)?;

        Ok(OrderByClause {
            field,
            direction,
            span: start_span.merge(close.span),
        })
    }

    /// Parse a limit clause.
    fn parse_limit_clause(&mut self) -> Result<Spanned<u32>, ParseError> {
        self.expect_token(Token::LParen)?;
        let value = self.expect_int()?;
        self.expect_token(Token::RParen)?;

        if value.value < 0 {
            return Err(ParseError::new("limit must be non-negative", value.span));
        }

        Ok(Spanned::new(value.value as u32, value.span))
    }

    /// Parse an offset clause.
    fn parse_offset_clause(&mut self) -> Result<Spanned<u32>, ParseError> {
        self.expect_token(Token::LParen)?;
        let value = self.expect_int()?;
        self.expect_token(Token::RParen)?;

        if value.value < 0 {
            return Err(ParseError::new("offset must be non-negative", value.span));
        }

        Ok(Spanned::new(value.value as u32, value.span))
    }

    /// Parse a create mutation.
    fn parse_create_mutation(
        &mut self,
        entity: Spanned<String>,
        start_span: Span,
    ) -> Result<Statement, ParseError> {
        self.expect_token(Token::LParen)?;
        let data = self.parse_object_literal()?;
        let close = self.expect_token(Token::RParen)?;

        Ok(Statement::Mutation(Mutation {
            entity,
            kind: MutationKind::Create { data },
            span: start_span.merge(close.span),
        }))
    }

    /// Parse update/delete/upsert mutation.
    fn parse_mutation(
        &mut self,
        entity: Spanned<String>,
        op_token: SpannedToken,
    ) -> Result<Statement, ParseError> {
        // Expect opening paren for the method call
        self.expect_token(Token::LParen)?;
        self.expect_token(Token::RParen)?;

        let mut clauses = Vec::new();
        let mut end_span = op_token.span;
        let start_span = entity.span;

        // Parse chained clauses
        while let Some(tok) = self.lexer.peek() {
            if tok.token != Token::Dot {
                break;
            }
            self.next_token()?; // consume dot

            let clause_tok = self.next_token()?;
            let clause = match clause_tok.token {
                Token::Where => MutationClause::Where(self.parse_where_clause(clause_tok.span)?),
                Token::Set => {
                    self.expect_token(Token::LParen)?;
                    let obj = self.parse_object_literal()?;
                    self.expect_token(Token::RParen)?;
                    MutationClause::Set(obj)
                }
                _ => {
                    return Err(ParseError::new(
                        format!("unexpected mutation clause {:?}", clause_tok.token),
                        clause_tok.span,
                    ))
                }
            };
            end_span = clause.span();
            clauses.push(clause);
        }

        let kind = match op_token.token {
            Token::Update => MutationKind::Update { clauses },
            Token::Delete => MutationKind::Delete { clauses },
            Token::Upsert => MutationKind::Upsert { clauses },
            _ => unreachable!(),
        };

        Ok(Statement::Mutation(Mutation {
            entity,
            kind,
            span: start_span.merge(end_span),
        }))
    }

    /// Parse an object literal { key: value, ... }.
    fn parse_object_literal(&mut self) -> Result<ObjectLiteral, ParseError> {
        let open = self.expect_token(Token::LBrace)?;
        let mut fields = Vec::new();

        // Handle empty object
        if let Some(tok) = self.lexer.peek() {
            if tok.token == Token::RBrace {
                let close = self.next_token()?;
                return Ok(ObjectLiteral {
                    fields,
                    span: open.span.merge(close.span),
                });
            }
        }

        // Parse first field
        fields.push(self.parse_object_field()?);

        // Parse remaining fields
        while let Some(tok) = self.lexer.peek() {
            if tok.token == Token::RBrace {
                break;
            }
            self.expect_token(Token::Comma)?;

            // Allow trailing comma
            if let Some(tok) = self.lexer.peek() {
                if tok.token == Token::RBrace {
                    break;
                }
            }

            fields.push(self.parse_object_field()?);
        }

        let close = self.expect_token(Token::RBrace)?;

        Ok(ObjectLiteral {
            fields,
            span: open.span.merge(close.span),
        })
    }

    /// Parse a single object field: key: value.
    fn parse_object_field(&mut self) -> Result<ObjectField, ParseError> {
        let name = self.expect_ident()?;
        self.expect_token(Token::Colon)?;
        let value = self.parse_literal()?;

        Ok(ObjectField { name, value })
    }

    /// Parse a schema command.
    fn parse_schema_command(&mut self) -> Result<Statement, ParseError> {
        let cmd_tok = self.next_token()?;
        let start_span = cmd_tok.span;

        let kind = match cmd_tok.token {
            Token::DotSchema => {
                // Check for optional entity name
                if let Some(tok) = self.lexer.peek() {
                    if let Token::Ident(_) = &tok.token {
                        let entity = self.expect_ident()?;
                        SchemaCommandKind::DescribeEntity(entity)
                    } else {
                        SchemaCommandKind::ListEntities
                    }
                } else {
                    SchemaCommandKind::ListEntities
                }
            }
            Token::DotDescribe => {
                let name = self.expect_ident()?;
                SchemaCommandKind::DescribeRelation(name)
            }
            Token::DotHelp => SchemaCommandKind::Help,
            _ => unreachable!(),
        };

        let end_span = match &kind {
            SchemaCommandKind::ListEntities | SchemaCommandKind::Help => start_span,
            SchemaCommandKind::DescribeEntity(e) => e.span,
            SchemaCommandKind::DescribeRelation(r) => r.span,
        };

        Ok(Statement::SchemaCommand(SchemaCommand {
            kind,
            span: start_span.merge(end_span),
        }))
    }

    /// Parse a literal value.
    fn parse_literal(&mut self) -> Result<Spanned<Literal>, ParseError> {
        let tok = self.next_token()?;
        let literal = match tok.token {
            Token::Null => Literal::Null,
            Token::True => Literal::Bool(true),
            Token::False => Literal::Bool(false),
            Token::Int(i) => Literal::Int(i),
            Token::Float(f) => Literal::Float(f),
            Token::String(s) | Token::StringSingle(s) => Literal::String(s),
            _ => {
                return Err(ParseError::new(
                    format!("expected literal value, found {:?}", tok.token),
                    tok.span,
                ))
            }
        };

        Ok(Spanned::new(literal, tok.span))
    }

    /// Parse a string literal specifically.
    fn parse_string_literal(&mut self) -> Result<Spanned<String>, ParseError> {
        let tok = self.next_token()?;
        match tok.token {
            Token::String(s) | Token::StringSingle(s) => Ok(Spanned::new(s, tok.span)),
            _ => Err(ParseError::new(
                format!("expected string literal, found {:?}", tok.token),
                tok.span,
            )),
        }
    }

    /// Expect and consume an identifier.
    fn expect_ident(&mut self) -> Result<Spanned<String>, ParseError> {
        let tok = self.next_token()?;
        match tok.token {
            Token::Ident(name) => Ok(Spanned::new(name, tok.span)),
            _ => Err(ParseError::new(
                format!("expected identifier, found {:?}", tok.token),
                tok.span,
            )),
        }
    }

    /// Expect and consume an integer.
    fn expect_int(&mut self) -> Result<Spanned<i64>, ParseError> {
        let tok = self.next_token()?;
        match tok.token {
            Token::Int(i) => Ok(Spanned::new(i, tok.span)),
            _ => Err(ParseError::new(
                format!("expected integer, found {:?}", tok.token),
                tok.span,
            )),
        }
    }

    /// Expect and consume a specific token.
    fn expect_token(&mut self, expected: Token) -> Result<SpannedToken, ParseError> {
        let tok = self.next_token()?;
        if std::mem::discriminant(&tok.token) == std::mem::discriminant(&expected) {
            Ok(tok)
        } else {
            Err(ParseError::new(
                format!("expected {:?}, found {:?}", expected, tok.token),
                tok.span,
            ))
        }
    }

    /// Get the next token or error if EOF.
    fn next_token(&mut self) -> Result<SpannedToken, ParseError> {
        self.lexer.next_token().ok_or_else(|| {
            ParseError::new(
                "unexpected end of input",
                Span::new(self.source.len(), self.source.len()),
            )
        })
    }
}

/// Check if a name is a method name (not a relation path segment).
fn is_method_name(name: &str) -> bool {
    matches!(
        name,
        "findMany"
            | "findUnique"
            | "findFirst"
            | "count"
            | "create"
            | "update"
            | "delete"
            | "upsert"
            | "where"
            | "include"
            | "orderBy"
            | "limit"
            | "offset"
            | "set"
            | "asc"
            | "desc"
    )
}

/// Parse a source string into a statement.
pub fn parse(source: &str) -> Result<Statement, ParseError> {
    let mut parser = Parser::new(source);
    parser.parse_statement()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_query() {
        let stmt = parse("User.findMany()").unwrap();
        if let Statement::Query(q) = stmt {
            assert_eq!(q.entity.value, "User");
            assert_eq!(q.kind, QueryKind::FindMany);
            assert!(q.clauses.is_empty());
        } else {
            panic!("expected Query");
        }
    }

    #[test]
    fn test_parse_query_with_where() {
        let stmt = parse(r#"User.findMany().where(status == "active")"#).unwrap();
        if let Statement::Query(q) = stmt {
            assert_eq!(q.clauses.len(), 1);
            if let QueryClause::Where(w) = &q.clauses[0] {
                if let FilterCondition::Comparison { field, op, value } = &w.condition {
                    assert_eq!(field.value, "status");
                    assert_eq!(*op, ComparisonOp::Eq);
                    assert_eq!(value.value, Literal::String("active".to_string()));
                } else {
                    panic!("expected Comparison");
                }
            } else {
                panic!("expected Where clause");
            }
        } else {
            panic!("expected Query");
        }
    }

    #[test]
    fn test_parse_query_with_and_filter() {
        let stmt = parse(r#"User.findMany().where(status == "active" && age > 18)"#).unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::Where(w) = &q.clauses[0] {
                if let FilterCondition::And(conditions) = &w.condition {
                    assert_eq!(conditions.len(), 2);
                } else {
                    panic!("expected And");
                }
            }
        }
    }

    #[test]
    fn test_parse_query_with_include() {
        let stmt = parse("User.findMany().include(posts)").unwrap();
        if let Statement::Query(q) = stmt {
            assert_eq!(q.clauses.len(), 1);
            if let QueryClause::Include(i) = &q.clauses[0] {
                assert_eq!(i.path.value, "posts");
            }
        }
    }

    #[test]
    fn test_parse_nested_include() {
        let stmt = parse("User.findMany().include(posts.comments)").unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::Include(i) = &q.clauses[0] {
                assert_eq!(i.path.value, "posts.comments");
            }
        }
    }

    #[test]
    fn test_parse_orderby_desc() {
        let stmt = parse("User.findMany().orderBy(createdAt.desc)").unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::OrderBy(o) = &q.clauses[0] {
                assert_eq!(o.field.value, "createdAt");
                assert_eq!(o.direction, SortDirection::Desc);
            }
        }
    }

    #[test]
    fn test_parse_limit_offset() {
        let stmt = parse("User.findMany().limit(10).offset(20)").unwrap();
        if let Statement::Query(q) = stmt {
            assert_eq!(q.clauses.len(), 2);
            if let QueryClause::Limit(l) = &q.clauses[0] {
                assert_eq!(l.value, 10);
            }
            if let QueryClause::Offset(o) = &q.clauses[1] {
                assert_eq!(o.value, 20);
            }
        }
    }

    #[test]
    fn test_parse_create_mutation() {
        let stmt = parse(r#"User.create({ name: "Alice", email: "alice@example.com" })"#).unwrap();
        if let Statement::Mutation(m) = stmt {
            assert_eq!(m.entity.value, "User");
            if let MutationKind::Create { data } = &m.kind {
                assert_eq!(data.fields.len(), 2);
                assert_eq!(data.fields[0].name.value, "name");
            } else {
                panic!("expected Create");
            }
        } else {
            panic!("expected Mutation");
        }
    }

    #[test]
    fn test_parse_update_mutation() {
        let stmt = parse(r#"User.update().where(id == "uuid").set({ status: "inactive" })"#).unwrap();
        if let Statement::Mutation(m) = stmt {
            assert_eq!(m.entity.value, "User");
            if let MutationKind::Update { clauses } = &m.kind {
                assert_eq!(clauses.len(), 2);
            }
        }
    }

    #[test]
    fn test_parse_delete_mutation() {
        let stmt = parse(r#"User.delete().where(id == "uuid")"#).unwrap();
        if let Statement::Mutation(m) = stmt {
            if let MutationKind::Delete { clauses } = &m.kind {
                assert_eq!(clauses.len(), 1);
            }
        }
    }

    #[test]
    fn test_parse_schema_command() {
        let stmt = parse(".schema").unwrap();
        if let Statement::SchemaCommand(s) = stmt {
            assert!(matches!(s.kind, SchemaCommandKind::ListEntities));
        }

        let stmt = parse(".schema User").unwrap();
        if let Statement::SchemaCommand(s) = stmt {
            if let SchemaCommandKind::DescribeEntity(e) = s.kind {
                assert_eq!(e.value, "User");
            }
        }
    }

    #[test]
    fn test_parse_in_filter() {
        let stmt = parse(r#"User.findMany().where(status in ["active", "pending"])"#).unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::Where(w) = &q.clauses[0] {
                if let FilterCondition::In { field, values, negated } = &w.condition {
                    assert_eq!(field.value, "status");
                    assert_eq!(values.len(), 2);
                    assert!(!negated);
                }
            }
        }
    }

    #[test]
    fn test_parse_is_null() {
        let stmt = parse("User.findMany().where(deletedAt is null)").unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::Where(w) = &q.clauses[0] {
                if let FilterCondition::IsNull { field, negated } = &w.condition {
                    assert_eq!(field.value, "deletedAt");
                    assert!(!negated);
                }
            }
        }
    }

    #[test]
    fn test_parse_is_not_null() {
        let stmt = parse("User.findMany().where(deletedAt is not null)").unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::Where(w) = &q.clauses[0] {
                if let FilterCondition::IsNull { field, negated } = &w.condition {
                    assert!(negated);
                }
            }
        }
    }

    #[test]
    fn test_parse_like() {
        let stmt = parse(r#"User.findMany().where(name like "Al%")"#).unwrap();
        if let Statement::Query(q) = stmt {
            if let QueryClause::Where(w) = &q.clauses[0] {
                if let FilterCondition::Like { field, pattern, negated } = &w.condition {
                    assert_eq!(field.value, "name");
                    assert_eq!(pattern.value, "Al%");
                    assert!(!negated);
                }
            }
        }
    }

    #[test]
    fn test_error_formatting() {
        let result = parse("User.findMany().where(status = \"active\")");
        assert!(result.is_err());
        let err = result.unwrap_err();
        let formatted = err.format_with_source("User.findMany().where(status = \"active\")");
        assert!(formatted.contains("line 1"));
    }
}
