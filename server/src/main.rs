//! Angel One — backend do webapp do barco (Axum + SQLite). Contrato em `app/API.md`.

mod ai;
mod crew;
mod db;
mod destinations;
mod error;
mod events;
mod extract;
mod inventory;
mod media;
mod menu;
mod push;
mod routes;
mod settings;
mod state;
mod status;
mod util;
mod weather;
mod web;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Request, State, FromRequestParts};
use axum::http::header::{CACHE_CONTROL, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde_json::json;
use tokio::sync::{broadcast, watch};
use tower_http::set_header::SetResponseHeaderLayer;
use tracing_subscriber::EnvFilter;

use crate::error::ApiError;
use crate::state::{AppState, Config, Inner};

async fn api_not_found() -> ApiError {
    ApiError::not_found("Rota não encontrada")
}

async fn api_method_not_allowed() -> ApiError {
    ApiError::new(StatusCode::METHOD_NOT_ALLOWED, "Método não permitido")
}

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let started = Instant::now();
    let res = next.run(req).await;
    let ms = started.elapsed().as_millis();
    let status = res.status().as_u16();
    if path.starts_with("/api/") {
        tracing::info!("{method} {path} → {status} ({ms} ms)");
    } else {
        tracing::debug!("{method} {path} → {status} ({ms} ms)");
    }
    res
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/crew", get(crew::list).post(crew::create))
        .route("/crew/{id}", patch(crew::update).delete(crew::delete))
        .route("/settings", get(settings::get).put(settings::put))
        .route(
            "/upload",
            post(media::upload).layer(DefaultBodyLimit::max(media::MAX_UPLOAD + 1024 * 1024)),
        )
        .route("/ai/models", get(ai::models))
        .route("/ai/jobs/{id}", get(ai::job))
        .route("/ai/menu-text", post(ai::menu_text))
        .route("/ai/menu-photo", post(ai::menu_photo))
        .route("/ai/clean-photo", post(ai::clean_photo))
        .route("/ai/enhance-photo", post(ai::enhance_photo))
        .route("/destinations", get(destinations::list).post(destinations::create))
        .route("/destinations/{id}", patch(destinations::update).delete(destinations::delete))
        .route("/destinations/{id}/vote", post(destinations::vote))
        .route("/votes/mine", delete(destinations::unvote))
        .route("/votes/reset", post(destinations::reset))
        .route("/routes", get(routes::get))
        .route("/weather", get(weather::get))
        .route("/position", post(status::post))
        .route("/status", get(status::get))
        .route("/inventory", get(inventory::get))
        .route("/items", post(inventory::create_item))
        .route("/items/{id}", patch(inventory::update_item).delete(inventory::delete_item))
        .route("/items/{id}/adjust", post(inventory::adjust))
        .route("/items/{id}/history", get(inventory::history))
        .route("/tags", post(inventory::create_tag))
        .route("/tags/{id}", patch(inventory::update_tag).delete(inventory::delete_tag))
        .route("/menu", get(menu::list).post(menu::create))
        .route("/menu/{id}", patch(menu::update).delete(menu::delete))
        .route("/orders", get(menu::list_orders).post(menu::create_order))
        .route("/orders/{id}", patch(menu::update_order))
        .route("/push/subscribe", post(push::subscribe))
        .route("/push/unsubscribe", post(push::unsubscribe))
        .route("/events", get(events::sse))
        .fallback(api_not_found)
        .method_not_allowed_fallback(api_method_not_allowed)
        .layer(SetResponseHeaderLayer::if_not_present(CACHE_CONTROL, HeaderValue::from_static("no-store")))
}

// Preserva o nome usado na operação mesmo se o tripulante for renomeado depois.
async fn crew_activity(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let method = req.method().clone();
    let record = !matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS")
        && !matches!(path.as_str(), "/api/position" | "/api/push/subscribe" | "/api/push/unsubscribe")
        && req.headers().contains_key("x-crew-id");
    if !record { return next.run(req).await; }
    let (mut parts, body) = req.into_parts();
    let actor = extract::OptCrew::from_request_parts(&mut parts, &st).await;
    let res = next.run(Request::from_parts(parts, body)).await;
    if res.status().is_success() && let Ok(extract::OptCrew(Some(actor))) = actor {
        let saved = st.db.call(move |c| {
            c.execute("INSERT INTO crew_activity(at,crew_id,crew_name,method,path) VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![util::now_str(), actor.id, actor.name, method.as_str(), path])?;
            Ok(())
        }).await;
        if let Err(e) = saved { tracing::error!("histórico da tripulação: {e}"); }
    }
    res
}

async fn shutdown_signal(tx: watch::Sender<bool>) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("encerrando...");
    let _ = tx.send(true);
    // Se alguma conexão travar o encerramento, sai mesmo assim.
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        std::process::exit(0);
    });
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

    let cfg = Config::from_env();
    if let Err(e) = std::fs::create_dir_all(&cfg.media_dir) {
        tracing::error!("não foi possível criar {}: {e}", cfg.media_dir.display());
        std::process::exit(1);
    }
    let db = match db::Db::open(&cfg.data_dir.join("angel-one.sqlite3")) {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("não foi possível abrir o banco: {e}");
            std::process::exit(1);
        }
    };
    let vapid = match push::load_or_create(&cfg.data_dir) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("chaves VAPID: {e}");
            std::process::exit(1);
        }
    };
    let http = reqwest::Client::builder()
        .user_agent(concat!("angel-one/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cliente HTTP");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (events_tx, _) = broadcast::channel(256);
    let port = cfg.port;
    let st: AppState = Arc::new(Inner {
        db,
        events: events_tx,
        http,
        jobs: Mutex::new(HashMap::new()),
        lists: tokio::sync::Mutex::new(HashMap::new()),
        weather: tokio::sync::Mutex::new(None),
        status: Mutex::new(status::StatusMem::default()),
        vapid,
        shutdown: shutdown_rx,
        cfg,
    });
    status::load_from_db(&st);
    destinations::spawn_auto_reset(st.clone());
    ai::spawn_janitor(st.clone());
    {
        // Pré-carrega a malha de água fora do caminho das requisições.
        let path = st.cfg.water_grid.clone();
        tokio::task::spawn_blocking(move || {
            routes::grid(path.as_deref());
        });
    }

    tracing::info!(
        "dados em {}, web em {}, chave OpenRouter no ambiente: {}",
        st.cfg.data_dir.display(),
        st.cfg.web_dir.display(),
        if st.cfg.env_openrouter_key.is_some() { "sim" } else { "não" }
    );
    if !st.cfg.web_dir.join("index.html").is_file() {
        tracing::warn!("{} não tem index.html (o front ainda não foi compilado?)", st.cfg.web_dir.display());
    }

    let app = Router::new()
        .nest("/api", api_router())
        .nest("/media", web::media(&st.cfg.media_dir))
        .nest("/assets", web::assets(&st.cfg.web_dir))
        .merge(web::spa(&st.cfg.web_dir))
        .layer(middleware::from_fn_with_state(st.clone(), crew_activity))
        .layer(middleware::from_fn(log_requests))
        .layer(SetResponseHeaderLayer::if_not_present(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .with_state(st.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("não foi possível escutar em {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("Angel One ouvindo em http://{addr}");
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(shutdown_tx)).await {
        tracing::error!("servidor: {e}");
    }
}
