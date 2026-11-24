use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::extract::{Crew, JsonBody, PathParam};
use crate::state::AppState;
use crate::util::{limited_text, now_str, required_text};

#[derive(Serialize)]
pub struct CrewOut {
    pub id: i64,
    pub name: String,
    pub emoji: String,
    pub created_at: String,
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<CrewOut> {
    Ok(CrewOut { id: r.get(0)?, name: r.get(1)?, emoji: r.get(2)?, created_at: r.get(3)? })
}

fn get_one(c: &Connection, id: i64) -> ApiResult<CrewOut> {
    c.query_row("SELECT id, name, emoji, created_at FROM crew WHERE id = ?1", [id], row)
        .optional()?
        .ok_or_else(|| ApiError::not_found("Tripulante não encontrado"))
}

/// `GET /api/crew`
pub async fn list(State(st): State<AppState>) -> ApiResult<Json<Vec<CrewOut>>> {
    let v = st
        .db
        .call(|c| {
            let mut s = c.prepare_cached("SELECT id, name, emoji, created_at FROM crew ORDER BY id")?;
            let v = s.query_map([], row)?.collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct CrewIn {
    name: String,
    #[serde(default)]
    emoji: Option<String>,
}

/// `POST /api/crew` (sem auth: cadastro pela entrada oculta)
pub async fn create(
    State(st): State<AppState>,
    JsonBody(b): JsonBody<CrewIn>,
) -> ApiResult<(StatusCode, Json<CrewOut>)> {
    let name = required_text(&b.name, "name", 40)?;
    let emoji = limited_text(b.emoji.as_deref().unwrap_or(""), "emoji", 16)?;
    let out = st
        .db
        .call(move |c| {
            c.execute(
                "INSERT INTO crew(name, emoji, created_at) VALUES (?1, ?2, ?3)",
                params![name, emoji, now_str()],
            )?;
            get_one(c, c.last_insert_rowid())
        })
        .await?;
    events::publish_reload(&st, "crew");
    Ok((StatusCode::CREATED, Json(out)))
}

#[derive(Deserialize)]
pub struct CrewPatch {
    name: Option<String>,
    emoji: Option<String>,
}

/// `PATCH /api/crew/:id` (crew)
pub async fn update(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<CrewPatch>,
) -> ApiResult<Json<CrewOut>> {
    let name = b.name.as_deref().map(|n| required_text(n, "name", 40)).transpose()?;
    let emoji = b.emoji.as_deref().map(|e| limited_text(e, "emoji", 16)).transpose()?;
    let out = st
        .db
        .call(move |c| {
            get_one(c, id)?;
            if let Some(n) = name {
                c.execute("UPDATE crew SET name = ?1 WHERE id = ?2", params![n, id])?;
            }
            if let Some(e) = emoji {
                c.execute("UPDATE crew SET emoji = ?1 WHERE id = ?2", params![e, id])?;
            }
            get_one(c, id)
        })
        .await?;
    events::publish_reload(&st, "crew");
    Ok(Json(out))
}

/// `DELETE /api/crew/:id` (crew)
pub async fn delete(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
) -> ApiResult<StatusCode> {
    st.db
        .call(move |c| {
            let tx = c.transaction()?;
            let n = tx.execute("DELETE FROM crew WHERE id = ?1", [id])?;
            if n == 0 {
                return Err(ApiError::not_found("Tripulante não encontrado"));
            }
            // Inscrições de push desse tripulante deixam de receber avisos da tripulação.
            tx.execute("UPDATE push_subs SET crew_id = NULL WHERE crew_id = ?1", [id])?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    events::publish_reload(&st, "crew");
    Ok(StatusCode::NO_CONTENT)
}
