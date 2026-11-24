use std::path::Path;

use axum::extract::Request;
use axum::http::header::CACHE_CONTROL;
use axum::http::HeaderValue;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

const IMMUTABLE: &str = "public, max-age=31536000, immutable";

async fn immutable(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let v = if res.status().is_success() || res.status().as_u16() == 304 { IMMUTABLE } else { "no-cache" };
    res.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static(v));
    res
}

async fn no_cache(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    res.headers_mut().insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res
}

/// `/media/*` → `DATA_DIR/media` (cache longo, imutável).
pub fn media(media_dir: &Path) -> Router<AppState> {
    Router::new()
        .fallback_service(ServeDir::new(media_dir).append_index_html_on_directories(false))
        .layer(middleware::from_fn(immutable))
}

/// `/assets/*` → `WEB_DIR/assets` (nomes com hash do Vite: cache imutável; ausente = 404).
pub fn assets(web_dir: &Path) -> Router<AppState> {
    Router::new()
        .fallback_service(ServeDir::new(web_dir.join("assets")).append_index_html_on_directories(false))
        .layer(middleware::from_fn(immutable))
}

/// Demais rotas → arquivo de `WEB_DIR` ou `index.html` (SPA), sempre `no-cache`.
pub fn spa(web_dir: &Path) -> Router<AppState> {
    let index = web_dir.join("index.html");
    Router::new()
        .fallback_service(ServeDir::new(web_dir).fallback(ServeFile::new(index)))
        .layer(middleware::from_fn(no_cache))
}
