//! Lexer for the ORMDB query language using logos.

use crate::span::Span;
use logos::Logos;

/// Token types for the query language.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
pub enum Token {
    // Query method keywords
    #[token("findMany")]
    FindMany,
    #[token("findUnique")]
    FindUnique,
    #[token("findFirst")]
    FindFirst,
    #[token("count")]
    Count,

    // Mutation method keywords
    #[token("create")]
    Create,
    #[token("update")]
    Update,
    #[token("delete")]
    Delete,
    #[token("upsert")]
    Upsert,

    // Chain method keywords
    #[token("where")]
    Where,
    #[token("include")]
    Include,
    #[token("orderBy")]
    OrderBy,
    #[token("limit")]
    Limit,
    #[token("offset")]
    Offset,
    #[token("set")]
    Set,

    // Sort direction
    #[token("asc")]
    Asc,
    #[token("desc")]
    Desc,

    // Comparison operators
    #[token("==")]
    Eq,
    #[token("!=")]
    Ne,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,

    // Logical operators
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("!")]
    Bang,
    #[token("not")]
    Not,

    // Keyword operators
    #[token("in")]
    In,
    #[token("like")]
    Like,
    #[token("is")]
    Is,

    // Literals
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("null")]
    Null,

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // String literal (double-quoted)
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        // Remove quotes and handle escapes
        let inner = &s[1..s.len()-1];
        unescape_string(inner)
    })]
    String(String),

    // String literal (single-quoted)
    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        unescape_string(inner)
    })]
    StringSingle(String),

    // Integer literal
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    // Float literal
    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    // Punctuation
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,

    // Schema commands (start with dot)
    #[token(".schema")]
    DotSchema,
    #[token(".describe")]
    DotDescribe,
    #[token(".help")]
    DotHelp,
}

/// Unescape a string literal, handling common escape sequences.
fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// A token with its span in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// Lexer that produces spanned tokens.
pub struct Lexer<'source> {
    inner: logos::Lexer<'source, Token>,
    peeked: Option<Option<SpannedToken>>,
}

impl<'source> Lexer<'source> {
    /// Create a new lexer for the given source.
    pub fn new(source: &'source str) -> Self {
        Self {
            inner: Token::lexer(source),
            peeked: None,
        }
    }

    /// Peek at the next token without consuming it.
    pub fn peek(&mut self) -> Option<&SpannedToken> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_inner());
        }
        self.peeked.as_ref().and_then(|o| o.as_ref())
    }

    /// Get the next token.
    pub fn next_token(&mut self) -> Option<SpannedToken> {
        if let Some(peeked) = self.peeked.take() {
            peeked
        } else {
            self.next_inner()
        }
    }

    fn next_inner(&mut self) -> Option<SpannedToken> {
        loop {
            match self.inner.next() {
                Some(Ok(token)) => {
                    return Some(SpannedToken {
                        token,
                        span: self.inner.span().into(),
                    });
                }
                Some(Err(())) => {
                    // Skip invalid tokens (lexer error)
                    // In a real implementation, we'd collect these as errors
                    continue;
                }
                None => return None,
            }
        }
    }

    /// Get the current position in the source.
    pub fn span(&self) -> Span {
        self.inner.span().into()
    }

    /// Get the source string.
    pub fn source(&self) -> &'source str {
        self.inner.source()
    }
}

impl Iterator for Lexer<'_> {
    type Item = SpannedToken;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

/// Tokenize a source string into a vector of spanned tokens.
pub fn tokenize(source: &str) -> Vec<SpannedToken> {
    Lexer::new(source).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let tokens = tokenize("User.findMany()");
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].token, Token::Ident("User".to_string()));
        assert_eq!(tokens[1].token, Token::Dot);
        assert_eq!(tokens[2].token, Token::FindMany);
        assert_eq!(tokens[3].token, Token::LParen);
        assert_eq!(tokens[4].token, Token::RParen);
    }

    #[test]
    fn test_query_with_where() {
        let tokens = tokenize(r#"User.findMany().where(status == "active")"#);
        assert!(tokens.iter().any(|t| t.token == Token::Where));
        assert!(tokens.iter().any(|t| t.token == Token::Eq));
        assert!(tokens
            .iter()
            .any(|t| t.token == Token::String("active".to_string())));
    }

    #[test]
    fn test_operators() {
        let tokens = tokenize("a == b && c != d || e < f");
        assert!(tokens.iter().any(|t| t.token == Token::Eq));
        assert!(tokens.iter().any(|t| t.token == Token::And));
        assert!(tokens.iter().any(|t| t.token == Token::Ne));
        assert!(tokens.iter().any(|t| t.token == Token::Or));
        assert!(tokens.iter().any(|t| t.token == Token::Lt));
    }

    #[test]
    fn test_numbers() {
        let tokens = tokenize("123 -456 3.14 -2.5");
        assert_eq!(tokens[0].token, Token::Int(123));
        assert_eq!(tokens[1].token, Token::Int(-456));
        assert_eq!(tokens[2].token, Token::Float(3.14));
        assert_eq!(tokens[3].token, Token::Float(-2.5));
    }

    #[test]
    fn test_string_escapes() {
        let tokens = tokenize(r#""hello\nworld" "tab\there""#);
        assert_eq!(tokens[0].token, Token::String("hello\nworld".to_string()));
        assert_eq!(tokens[1].token, Token::String("tab\there".to_string()));
    }

    #[test]
    fn test_create_mutation() {
        let tokens = tokenize(r#"User.create({ name: "Alice" })"#);
        assert!(tokens.iter().any(|t| t.token == Token::Create));
        assert!(tokens.iter().any(|t| t.token == Token::LBrace));
        assert!(tokens.iter().any(|t| t.token == Token::Colon));
        assert!(tokens.iter().any(|t| t.token == Token::RBrace));
    }

    #[test]
    fn test_schema_commands() {
        let tokens = tokenize(".schema User");
        assert_eq!(tokens[0].token, Token::DotSchema);
        assert_eq!(tokens[1].token, Token::Ident("User".to_string()));
    }

    #[test]
    fn test_orderby() {
        let tokens = tokenize("User.findMany().orderBy(createdAt.desc)");
        assert!(tokens.iter().any(|t| t.token == Token::OrderBy));
        assert!(tokens.iter().any(|t| t.token == Token::Desc));
    }

    #[test]
    fn test_array_syntax() {
        let tokens = tokenize("[1, 2, 3]");
        assert_eq!(tokens[0].token, Token::LBracket);
        assert_eq!(tokens[1].token, Token::Int(1));
        assert_eq!(tokens[2].token, Token::Comma);
        assert_eq!(tokens[5].token, Token::Int(3));
        assert_eq!(tokens[6].token, Token::RBracket);
    }

    #[test]
    fn test_lexer_peek() {
        let mut lexer = Lexer::new("a.b");

        // Peek should not consume
        assert_eq!(
            lexer.peek().map(|t| &t.token),
            Some(&Token::Ident("a".to_string()))
        );
        assert_eq!(
            lexer.peek().map(|t| &t.token),
            Some(&Token::Ident("a".to_string()))
        );

        // Now consume
        assert_eq!(
            lexer.next_token().map(|t| t.token),
            Some(Token::Ident("a".to_string()))
        );
        assert_eq!(lexer.next_token().map(|t| t.token), Some(Token::Dot));
    }
}
