//! ORMDB vs SQLite: Developer Ergonomics Demo
//!
//! An interactive TUI showing side-by-side code comparisons between
//! ORMDB and SQLite, highlighting developer experience differences.
//!
//! Run with: cargo run --release

use std::io::{self, stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Tabs},
};

fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Run app
    let result = run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

struct App {
    scenarios: Vec<Scenario>,
    current: usize,
    running: bool,
}

struct Scenario {
    name: &'static str,
    key: char,
    ormdb_code: &'static str,
    sqlite_code: &'static str,
    ormdb_metric: &'static str,
    sqlite_metric: &'static str,
    summary: &'static str,
}

impl App {
    fn new() -> Self {
        Self {
            scenarios: vec![
                Scenario {
                    name: "N+1 Problem",
                    key: '1',
                    ormdb_code: r#"// Single query fetches users AND posts
let query = GraphQuery::new("User")
    .include(RelationInclude::new("posts"))
    .with_pagination(Pagination::limit(100));

let result = executor.execute(&query)?;

for user in result.entities("User") {
    for post in result.related(&user, "posts") {
        println!("{}: {}", user.name, post.title);
    }
}

// Total: 1 query
// All data loaded in single round-trip"#,
                    sqlite_code: r#"// N+1 queries - one per user!
let users: Vec<User> = conn.prepare(
    "SELECT * FROM user LIMIT 100"
)?.query_map([], |row| {
    Ok(User { id: row.get(0)?, name: row.get(1)? })
})?.collect();

for user in &users {
    // Extra query for EACH user!
    let posts: Vec<Post> = conn.prepare(
        "SELECT * FROM post WHERE author_id = ?"
    )?.query_map([&user.id], |row| {
        Ok(Post { title: row.get(1)? })
    })?.collect();

    for post in posts {
        println!("{}: {}", user.name, post.title);
    }
}
// Total: 101 queries!"#,
                    ormdb_metric: "1 query",
                    sqlite_metric: "101 queries",
                    summary: "ORMDB eliminates N+1 with automatic batching",
                },
                Scenario {
                    name: "Type Safety",
                    key: '2',
                    ormdb_code: r#"// Entity name validated before execution
let query = GraphQuery::new("Usres");
//                          ^^^^^^
// Error: Unknown entity 'Usres'
// Suggestion: Did you mean 'User'?


// Field types validated at query time
FilterExpr::eq("age", Value::String("thirty".into()))
// Error: Field 'age' expects Int32, got String


// Compile-time field validation in Rust
let query = GraphQuery::new("User")
    .with_fields(vec!["id", "naem", "email"]);
//                          ^^^^
// Error: Unknown field 'naem' on entity 'User'
// Available fields: id, name, email, age, status"#,
                    sqlite_code: r#"// Typo only discovered at RUNTIME!
let stmt = conn.prepare(
    "SELECT * FROM usres"  // Typo in table name
)?;
// Runtime error: no such table: usres
// Only caught when code runs in production!


// Type coercion surprises
conn.execute(
    "INSERT INTO user (age) VALUES (?)",
    ["thirty"]  // String stored in integer column!
)?;
// SQLite silently coerces - no error!
// Data corruption waiting to happen


// Column name typos
conn.prepare("SELECT naem FROM user")?;
// Runtime error: no such column: naem"#,
                    ormdb_metric: "Compile/validation time",
                    sqlite_metric: "Runtime errors",
                    summary: "ORMDB catches errors before code runs",
                },
                Scenario {
                    name: "Error Handling",
                    key: '3',
                    ormdb_code: r#"// Typed errors with structured data
match client.mutate(insert_user).await {
    Ok(result) => {
        println!("Created: {}", result.id);
    }

    Err(Error::ConstraintViolation(
        ConstraintError::UniqueViolation {
            fields,    // Vec<String>
            value,     // The duplicate value
            entity,    // Entity type
            ..
        }
    )) => {
        // Direct access to error details!
        println!("Duplicate {} in {}",
            fields.join(", "), entity);
    }

    Err(Error::ConstraintViolation(
        ConstraintError::ForeignKeyViolation {
            field,
            referenced_entity,
            ..
        }
    )) => {
        println!("Invalid ref to {}", referenced_entity);
    }

    Err(e) => return Err(e),
}"#,
                    sqlite_code: r#"// Must parse error strings!
match conn.execute(insert_sql, params) {
    Ok(_) => println!("Created user"),
    Err(e) => {
        let msg = e.to_string();

        // String matching - fragile!
        if msg.contains("UNIQUE constraint failed") {
            // Which field? Parse the string...
            if msg.contains("user.email") {
                println!("Duplicate email");
            } else if msg.contains("user.username") {
                println!("Duplicate username");
            }
            // Easy to miss cases!
        }
        else if msg.contains("FOREIGN KEY constraint") {
            // Which reference? More parsing...
            println!("Invalid reference");
        }
        else {
            return Err(e);
        }
    }
}

// Error format can change between SQLite versions!"#,
                    ormdb_metric: "Typed error enums",
                    sqlite_metric: "String parsing",
                    summary: "ORMDB provides structured, typed errors",
                },
                Scenario {
                    name: "Schema Definition",
                    key: '4',
                    ormdb_code: r#"// Declarative, type-safe schema
let user = EntityDef::new("User", "id")
    .with_field(
        FieldDef::new("id", ScalarType::Uuid)
            .with_default_auto_uuid()
    )
    .with_field(
        FieldDef::new("name", ScalarType::String)
            .required()
            .with_max_length(100)
    )
    .with_field(
        FieldDef::new("email", ScalarType::String)
            .required()
            .with_index()  // Automatic index
    )
    .with_field(
        FieldDef::new("created_at", ScalarType::Timestamp)
            .with_default_current_timestamp()
    );

// Relations defined with cardinality
let posts = RelationDef::one_to_many(
    "posts", "User", "id", "Post", "author_id"
).with_delete_behavior(DeleteBehavior::Cascade);

// Apply with safety grading
let grade = SafetyGrader::grade(&migration);
// A: Non-breaking, B: Careful, C: Downtime, D: Data loss"#,
                    sqlite_code: r#"// DDL strings - no compile-time checks
conn.execute(
    "CREATE TABLE user (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        email TEXT NOT NULL,
        created_at INTEGER
    )",
    []
)?;

// Indexes are separate statements
conn.execute(
    "CREATE UNIQUE INDEX user_email ON user(email)",
    []
)?;

// No built-in relation tracking
// Must manually maintain foreign keys
conn.execute(
    "CREATE TABLE post (
        id TEXT PRIMARY KEY,
        author_id TEXT REFERENCES user(id),
        title TEXT
    )",
    []
)?;

// Cascade delete? Write triggers manually
conn.execute(
    "CREATE TRIGGER delete_user_posts
     BEFORE DELETE ON user
     BEGIN
       DELETE FROM post WHERE author_id = OLD.id;
     END",
    []
)?;

// Migration safety? Hope and pray..."#,
                    ormdb_metric: "Type-safe DSL",
                    sqlite_metric: "DDL strings",
                    summary: "ORMDB schema is validated, migrations are graded",
                },
                Scenario {
                    name: "Relations & Joins",
                    key: '5',
                    ormdb_code: r#"// Load post with author and comments
let query = GraphQuery::new("Post")
    .with_filter(FilterExpr::eq("id", post_id))
    .include(RelationInclude::new("author"))
    .include(
        RelationInclude::new("comments")
            .with_order(OrderSpec::desc("created_at"))
            .with_limit(20)
            .include(RelationInclude::new("author"))
    );

let result = executor.execute(&query)?;

// Structured access to graph
let post = result.entities("Post").next()?;
let author = result.related_one(&post, "author")?;

for comment in result.related(&post, "comments") {
    let commenter = result.related_one(&comment, "author")?;
    println!("{}: {}", commenter.name, comment.text);
}

// 1 query, proper graph structure"#,
                    sqlite_code: r#"// Complex JOIN with denormalized results
let rows = conn.prepare(
    "SELECT
        p.id, p.title, p.content,
        u.id AS author_id, u.name AS author_name,
        c.id AS comment_id, c.text AS comment_text,
        cu.id AS commenter_id, cu.name AS commenter_name
     FROM post p
     JOIN user u ON p.author_id = u.id
     LEFT JOIN comment c ON c.post_id = p.id
     LEFT JOIN user cu ON c.author_id = cu.id
     WHERE p.id = ?
     ORDER BY c.created_at DESC
     LIMIT 21"  // +1 for post row
)?.query_map([post_id], |row| {
    // Manual row parsing...
})?;

// Results are FLAT - must reconstruct graph
let mut post = None;
let mut comments = Vec::new();
for row in rows {
    if post.is_none() {
        post = Some(Post { /* ... */ });
    }
    if let Some(comment_id) = row.comment_id {
        comments.push(Comment { /* ... */ });
    }
}
// Lots of boilerplate, easy to get wrong"#,
                    ormdb_metric: "Graph queries",
                    sqlite_metric: "Manual JOINs",
                    summary: "ORMDB returns structured graphs, not flat rows",
                },
            ],
            current: 0,
            running: true,
        }
    }

    fn next(&mut self) {
        self.current = (self.current + 1) % self.scenarios.len();
    }

    fn prev(&mut self) {
        self.current = if self.current == 0 {
            self.scenarios.len() - 1
        } else {
            self.current - 1
        };
    }

    fn jump(&mut self, index: usize) {
        if index < self.scenarios.len() {
            self.current = index;
        }
    }
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    while app.running {
        terminal.draw(|frame| ui(frame, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.running = false,
                        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('n') => app.next(),
                        KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('p') => app.prev(),
                        KeyCode::Char('1') => app.jump(0),
                        KeyCode::Char('2') => app.jump(1),
                        KeyCode::Char('3') => app.jump(2),
                        KeyCode::Char('4') => app.jump(3),
                        KeyCode::Char('5') => app.jump(4),
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}

fn ui(frame: &mut Frame, app: &App) {
    let scenario = &app.scenarios[app.current];

    // Main layout
    let [header_area, nav_area, main_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(3),
    ])
    .areas(frame.area());

    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                " ORMDB ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" vs "),
            Span::styled(
                " SQLite ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  Developer Ergonomics",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "  Compare code side-by-side. See why ORMDB improves developer experience.",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(header, header_area);

    // Navigation tabs
    let tab_titles: Vec<Line> = app
        .scenarios
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(format!("[{}] {}", s.key, s.name), style))
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(app.current)
        .divider(" | ")
        .padding(" ", " ")
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    " <- -> Navigate | 1-5 Jump | q Quit ",
                    Style::default().fg(Color::DarkGray),
                )),
        );
    frame.render_widget(tabs, nav_area);

    // Code panes (side by side)
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(main_area);

    // ORMDB code (left)
    let ormdb_block = Block::default()
        .title(Span::styled(
            " ORMDB (Rust) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let ormdb_code = Paragraph::new(syntax_highlight(scenario.ormdb_code, true))
        .block(ormdb_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(ormdb_code, left_area);

    // SQLite code (right)
    let sqlite_block = Block::default()
        .title(Span::styled(
            " SQLite (Rust) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let sqlite_code = Paragraph::new(syntax_highlight(scenario.sqlite_code, false))
        .block(sqlite_block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(sqlite_code, right_area);

    // Footer with metrics
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" ORMDB: ", Style::default().fg(Color::Cyan)),
        Span::styled(
            scenario.ormdb_metric,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled("SQLite: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            scenario.sqlite_metric,
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  "),
        Span::styled(
            scenario.summary,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::ITALIC),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(footer, footer_area);
}

/// Simple syntax highlighting for code display
fn syntax_highlight(code: &str, is_ormdb: bool) -> Vec<Line<'static>> {
    let keywords = [
        "let", "fn", "for", "in", "if", "else", "match", "Ok", "Err", "Some", "None", "return",
        "async", "await", "mut", "&", "?",
    ];
    let types = [
        "GraphQuery",
        "FilterExpr",
        "RelationInclude",
        "Pagination",
        "Value",
        "EntityDef",
        "FieldDef",
        "ScalarType",
        "RelationDef",
        "OrderSpec",
        "Error",
        "ConstraintError",
        "Vec",
        "String",
        "User",
        "Post",
        "Comment",
        "DeleteBehavior",
        "SafetyGrader",
    ];

    let accent_color = if is_ormdb { Color::Cyan } else { Color::Yellow };

    code.lines()
        .map(|line| {
            let line = line.to_string();

            // Comment lines
            if line.trim().starts_with("//") {
                return Line::from(Span::styled(line, Style::default().fg(Color::DarkGray)));
            }

            // String literals (simplified)
            if line.contains('"') {
                let mut spans = Vec::new();
                let mut in_string = false;
                let mut current = String::new();

                for ch in line.chars() {
                    if ch == '"' {
                        if in_string {
                            current.push(ch);
                            spans.push(Span::styled(
                                current.clone(),
                                Style::default().fg(Color::Green),
                            ));
                            current.clear();
                            in_string = false;
                        } else {
                            if !current.is_empty() {
                                spans.push(colorize_segment(&current, &keywords, &types, accent_color));
                                current.clear();
                            }
                            current.push(ch);
                            in_string = true;
                        }
                    } else {
                        current.push(ch);
                    }
                }
                if !current.is_empty() {
                    spans.push(colorize_segment(&current, &keywords, &types, accent_color));
                }
                return Line::from(spans);
            }

            // Regular code line
            Line::from(colorize_segment(&line, &keywords, &types, accent_color))
        })
        .collect()
}

fn colorize_segment(
    segment: &str,
    keywords: &[&str],
    types: &[&str],
    accent: Color,
) -> Span<'static> {
    let segment = segment.to_string();

    // Check if segment contains a keyword
    for kw in keywords {
        if segment.contains(kw) {
            return Span::styled(segment, Style::default().fg(Color::Magenta));
        }
    }

    // Check if segment contains a type
    for ty in types {
        if segment.contains(ty) {
            return Span::styled(segment, Style::default().fg(accent));
        }
    }

    // Default
    Span::styled(segment, Style::default().fg(Color::White))
}
