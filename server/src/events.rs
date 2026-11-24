use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::http::header::{HeaderName, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use futures_util::StreamExt as _;
use serde::Serialize;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::AppState;

/// Evento SSE já serializado.
#[derive(Clone, Debug)]
pub struct Ev {
    pub name: &'static str,
    pub data: String,
}

/// Publica um evento para todos os clientes conectados (ignora se não houver ninguém).
pub fn publish<T: Serialize>(st: &AppState, name: &'static str, data: &T) {
    match serde_json::to_string(data) {
        Ok(data) => {
            let _ = st.events.send(Ev { name, data });
        }
        Err(e) => tracing::error!("falha ao serializar evento {name}: {e}"),
    }
}

/// Atalho para eventos `{}` (o front só recarrega).
pub fn publish_reload(st: &AppState, name: &'static str) {
    let _ = st.events.send(Ev { name, data: "{}".into() });
}

/// `GET /api/events`
pub async fn sse(State(st): State<AppState>) -> impl IntoResponse {
    let rx = st.events.subscribe();
    let mut shutdown = st.shutdown.clone();
    let hello = tokio_stream::once(Ok::<Event, Infallible>(
        Event::default().retry(Duration::from_secs(3)).comment("conectado"),
    ));
    let live = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(ev) => Some(Ok::<Event, Infallible>(Event::default().event(ev.name).data(ev.data))),
            // Cliente lento perdeu eventos: segue em frente (o front recarrega no próximo evento).
            Err(_) => None,
        }
    });
    let stream = hello.chain(live).take_until(async move {
        let _ = shutdown.wait_for(|v| *v).await;
    });
    (
        [(HeaderName::from_static("x-accel-buffering"), HeaderValue::from_static("no"))],
        Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("ping")),
    )
}
