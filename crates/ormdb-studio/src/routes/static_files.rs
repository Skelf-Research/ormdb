use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
struct Assets;

/// Serve embedded static files
pub async fn serve_static(req: Request<Body>) -> Response {
    let path = req.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => {
            // SPA fallback: serve index.html for client-side routing
            if let Some(content) = Assets::get("index.html") {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(content.data.into_owned()))
                    .unwrap()
            } else {
                // No frontend built yet - serve a placeholder
                serve_placeholder().into_response()
            }
        }
    }
}

fn serve_placeholder() -> Response {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>ORMDB Studio</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            color: #e8e8e8;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .container {
            text-align: center;
            padding: 2rem;
        }
        h1 {
            font-size: 3rem;
            margin-bottom: 1rem;
            background: linear-gradient(90deg, #00d4ff, #7c3aed);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        p { color: #a0a0a0; margin-bottom: 2rem; font-size: 1.1rem; }
        .status {
            background: rgba(255,255,255,0.05);
            border-radius: 12px;
            padding: 1.5rem;
            margin-bottom: 2rem;
        }
        .status h3 { color: #00d4ff; margin-bottom: 0.5rem; }
        code {
            background: rgba(0,0,0,0.3);
            padding: 0.5rem 1rem;
            border-radius: 6px;
            font-family: 'Fira Code', monospace;
            display: block;
            margin-top: 1rem;
        }
        .api-link {
            color: #00d4ff;
            text-decoration: none;
        }
        .api-link:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <div class="container">
        <h1>ORMDB Studio</h1>
        <p>Web-based database management for ORMDB</p>

        <div class="status">
            <h3>Frontend Not Built</h3>
            <p>The Vue.js frontend needs to be built first:</p>
            <code>cd crates/ormdb-studio/frontend && npm install && npm run build</code>
        </div>

        <p>
            API is available at
            <a href="/health" class="api-link">/health</a> and
            <a href="/api/session" class="api-link">/api/session</a>
        </p>
    </div>
</body>
</html>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}
