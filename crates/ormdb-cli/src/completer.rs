//! Tab completion for the REPL.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;

/// ORMDB REPL helper with completion support.
pub struct OrmdbHelper {
    /// Cached entity names from schema.
    pub entities: Vec<String>,
}

impl OrmdbHelper {
    /// Create a new helper with empty entity list.
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
        }
    }

    /// Update the entity list.
    pub fn set_entities(&mut self, entities: Vec<String>) {
        self.entities = entities;
    }
}

impl Default for OrmdbHelper {
    fn default() -> Self {
        Self::new()
    }
}

/// Dot-commands for completion.
const DOT_COMMANDS: &[&str] = &[
    ".connect",
    ".disconnect",
    ".status",
    ".schema",
    ".format",
    ".history",
    ".clear",
    ".help",
    ".exit",
    ".quit",
];

/// Query method keywords.
const QUERY_METHODS: &[&str] = &[
    "findMany",
    "findUnique",
    "findFirst",
    "create",
    "update",
    "delete",
    "upsert",
];

/// Chain method keywords.
const CHAIN_METHODS: &[&str] = &["where", "include", "orderBy", "limit", "offset", "set"];

/// Filter keywords.
const FILTER_KEYWORDS: &[&str] = &[
    "true", "false", "null", "in", "not", "like", "is", "asc", "desc",
];

impl Completer for OrmdbHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line_to_cursor = &line[..pos];

        // Find the start of the current word
        let word_start = line_to_cursor
            .rfind(|c: char| c.is_whitespace() || c == '(' || c == '.' || c == ',')
            .map(|i| i + 1)
            .unwrap_or(0);

        let word = &line_to_cursor[word_start..];

        let mut completions = Vec::new();

        // Dot commands at start of line
        if line_to_cursor.trim().starts_with('.') && !line_to_cursor.contains(' ') {
            for cmd in DOT_COMMANDS {
                if cmd.starts_with(line_to_cursor.trim()) {
                    completions.push(Pair {
                        display: cmd.to_string(),
                        replacement: cmd.to_string(),
                    });
                }
            }
            return Ok((0, completions));
        }

        // After a dot (method calls)
        if line_to_cursor.ends_with('.') || word.is_empty() && line_to_cursor.ends_with('.') {
            // Check if we're at the start (entity.method) or chaining
            let before_dot = &line_to_cursor[..line_to_cursor.len() - 1];
            if before_dot.contains('.') {
                // Chaining - suggest chain methods
                for method in CHAIN_METHODS {
                    completions.push(Pair {
                        display: method.to_string(),
                        replacement: method.to_string(),
                    });
                }
            } else {
                // First dot - suggest query/mutation methods
                for method in QUERY_METHODS {
                    completions.push(Pair {
                        display: method.to_string(),
                        replacement: method.to_string(),
                    });
                }
            }
            return Ok((pos, completions));
        }

        // At the start of the line - suggest entities
        if word_start == 0 && !word.starts_with('.') {
            for entity in &self.entities {
                if entity.to_lowercase().starts_with(&word.to_lowercase()) {
                    completions.push(Pair {
                        display: entity.clone(),
                        replacement: entity.clone(),
                    });
                }
            }

            // Also suggest some common entity names if we have no entities
            if self.entities.is_empty() {
                let common = ["User", "Post", "Comment", "Order", "Product", "Customer"];
                for entity in common {
                    if entity.to_lowercase().starts_with(&word.to_lowercase()) {
                        completions.push(Pair {
                            display: entity.to_string(),
                            replacement: entity.to_string(),
                        });
                    }
                }
            }
        }

        // Inside parentheses or after keywords - suggest filter keywords
        if line_to_cursor.contains('(') {
            for kw in FILTER_KEYWORDS {
                if kw.starts_with(&word.to_lowercase()) {
                    completions.push(Pair {
                        display: kw.to_string(),
                        replacement: kw.to_string(),
                    });
                }
            }
        }

        // Method name completion
        let all_methods: Vec<&str> = QUERY_METHODS
            .iter()
            .chain(CHAIN_METHODS.iter())
            .copied()
            .collect();

        for method in all_methods {
            if method.to_lowercase().starts_with(&word.to_lowercase()) && !word.is_empty() {
                completions.push(Pair {
                    display: method.to_string(),
                    replacement: method.to_string(),
                });
            }
        }

        Ok((word_start, completions))
    }
}

impl Hinter for OrmdbHelper {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        // Could provide inline hints here
        None
    }
}

impl Highlighter for OrmdbHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // Could add syntax highlighting here
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: rustyline::highlight::CmdKind) -> bool {
        false
    }
}

impl Validator for OrmdbHelper {}

impl Helper for OrmdbHelper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helper_creation() {
        let helper = OrmdbHelper::new();
        assert!(helper.entities.is_empty());
    }

    #[test]
    fn test_set_entities() {
        let mut helper = OrmdbHelper::new();
        helper.set_entities(vec!["User".to_string(), "Post".to_string()]);
        assert_eq!(helper.entities.len(), 2);
    }
}
