use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use chrono_tz::America::Sao_Paulo;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::extract::{Crew, Device, JsonBody, PathParam};
use crate::state::AppState;
use crate::util::{check_lat_lon, double_option, now_str, nullable_text, required_text};

pub const WINDS: [&str; 8] = ["N", "NE", "L", "SE", "S", "SO", "O", "NO"];

#[derive(Serialize)]
pub struct DestOut {
    pub id: i64,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub photo: Option<String>,
    pub photo_original: Option<String>,
    pub good_winds: Vec<String>,
    pub votes: i64,
    pub my_vote: bool,
    pub created_by: Option<String>,
    pub created_at: String,
}

const SELECT: &str = "SELECT d.id, d.name, d.lat, d.lon, d.photo, d.photo_original, d.good_winds,
        (SELECT COUNT(*) FROM votes v WHERE v.destination_id = d.id AND v.created_at >= ?2),
        EXISTS(SELECT 1 FROM votes v WHERE v.destination_id = d.id AND v.device_id = ?1 AND v.created_at >= ?2),
        d.created_by, d.created_at
    FROM destinations d";

// Turnos em Florianópolis: [00:00,12:00) e [12:00,00:00).
// A comparação no banco é UTC em RFC3339, sem depender do fuso do host.
fn slot_start(at: DateTime<Utc>) -> String {
    let local = at.with_timezone(&Sao_Paulo);
    let hour = if local.hour() < 12 { 0 } else { 12 };
    let start = Sao_Paulo
        .with_ymd_and_hms(local.year(), local.month(), local.day(), hour, 0, 0)
        .single()
        .expect("turno válido de São Paulo");
    crate::util::ts(start.with_timezone(&Utc))
}

fn current_slot() -> String { slot_start(Utc::now()) }

pub fn spawn_auto_reset(st: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let cutoff = current_slot();
            let changed = st.db.call(move |c| {
                let removed = c.execute("DELETE FROM votes WHERE created_at < ?1", [cutoff])?;
                Ok(if removed > 0 { Some(vote_counts(c)?) } else { None })
            }).await;
            match changed {
                Ok(Some(counts)) => publish_votes(&st, &counts),
                Err(e) => tracing::warn!("limpeza dos votos: {e}"),
                _ => {}
            }
        }
    });
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<DestOut> {
    let winds: String = r.get(6)?;
    Ok(DestOut {
        id: r.get(0)?,
        name: r.get(1)?,
        lat: r.get(2)?,
        lon: r.get(3)?,
        photo: r.get(4)?,
        photo_original: r.get(5)?,
        good_winds: serde_json::from_str(&winds).unwrap_or_default(),
        votes: r.get(7)?,
        my_vote: r.get(8)?,
        created_by: r.get(9)?,
        created_at: r.get(10)?,
    })
}

fn get_one(c: &Connection, id: i64, device: &Option<String>) -> ApiResult<DestOut> {
    c.query_row(&format!("{SELECT} WHERE d.id = ?3"), params![device, current_slot(), id], row)
        .optional()?
        .ok_or_else(|| ApiError::not_found("Destino não encontrado"))
}

/// Só `(id, lat, lon)` de todos os destinos — usado pelas rotas.
pub fn all_points(c: &Connection) -> rusqlite::Result<Vec<(i64, f64, f64)>> {
    let mut s = c.prepare_cached("SELECT id, lat, lon FROM destinations ORDER BY id")?;
    s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect()
}

fn normalize_winds(v: &[String]) -> ApiResult<Vec<String>> {
    let mut out = Vec::new();
    for w in v {
        let w = w.trim().to_uppercase();
        let w = match w.as_str() {
            "E" => "L".to_string(),
            "SW" => "SO".to_string(),
            "W" => "O".to_string(),
            "NW" => "NO".to_string(),
            _ => w,
        };
        if !WINDS.contains(&w.as_str()) {
            return Err(ApiError::bad_request(format!(
                "Direção de vento inválida: '{w}' (use N, NE, L, SE, S, SO, O, NO)"
            )));
        }
        if !out.contains(&w) {
            out.push(w);
        }
    }
    out.sort_by_key(|w| WINDS.iter().position(|x| x == w));
    Ok(out)
}

/// `GET /api/destinations`
pub async fn list(State(st): State<AppState>, Device(device): Device) -> ApiResult<Json<Vec<DestOut>>> {
    let v = st
        .db
        .call(move |c| {
            let mut s = c.prepare_cached(&format!("{SELECT} ORDER BY d.id"))?;
            let v = s.query_map(params![device, current_slot()], row)?.collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct DestIn {
    name: String,
    lat: f64,
    lon: f64,
    photo: Option<String>,
    photo_original: Option<String>,
    #[serde(default)]
    good_winds: Vec<String>,
}

/// `POST /api/destinations` (crew)
pub async fn create(
    State(st): State<AppState>,
    crew: Crew,
    Device(device): Device,
    JsonBody(b): JsonBody<DestIn>,
) -> ApiResult<(StatusCode, Json<DestOut>)> {
    let name = required_text(&b.name, "name", 80)?;
    check_lat_lon(b.lat, b.lon)?;
    let photo = nullable_text(b.photo.as_deref(), "photo", 300)?;
    let photo_original = nullable_text(b.photo_original.as_deref(), "photo_original", 300)?;
    let winds = serde_json::to_string(&normalize_winds(&b.good_winds)?).unwrap_or_else(|_| "[]".into());
    let out = st
        .db
        .call(move |c| {
            c.execute(
                "INSERT INTO destinations(name, lat, lon, photo, photo_original, good_winds, created_by, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![name, b.lat, b.lon, photo, photo_original, winds, crew.name, now_str()],
            )?;
            get_one(c, c.last_insert_rowid(), &device)
        })
        .await?;
    events::publish_reload(&st, "destinations");
    Ok((StatusCode::CREATED, Json(out)))
}

#[derive(Deserialize)]
pub struct DestPatch {
    name: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    #[serde(default, deserialize_with = "double_option")]
    photo: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    photo_original: Option<Option<String>>,
    good_winds: Option<Vec<String>>,
}

/// `PATCH /api/destinations/:id` (crew)
pub async fn update(
    State(st): State<AppState>,
    _crew: Crew,
    Device(device): Device,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<DestPatch>,
) -> ApiResult<Json<DestOut>> {
    let name = b.name.as_deref().map(|n| required_text(n, "name", 80)).transpose()?;
    let photo = b.photo.map(|p| nullable_text(p.as_deref(), "photo", 300)).transpose()?;
    let photo_original =
        b.photo_original.map(|p| nullable_text(p.as_deref(), "photo_original", 300)).transpose()?;
    let winds = b
        .good_winds
        .as_deref()
        .map(normalize_winds)
        .transpose()?
        .map(|w| serde_json::to_string(&w).unwrap_or_else(|_| "[]".into()));
    let out = st
        .db
        .call(move |c| {
            let cur = get_one(c, id, &device)?;
            let lat = b.lat.unwrap_or(cur.lat);
            let lon = b.lon.unwrap_or(cur.lon);
            check_lat_lon(lat, lon)?;
            c.execute(
                "UPDATE destinations SET name = ?1, lat = ?2, lon = ?3, photo = ?4, photo_original = ?5,
                    good_winds = ?6 WHERE id = ?7",
                params![
                    name.unwrap_or(cur.name),
                    lat,
                    lon,
                    photo.unwrap_or(cur.photo),
                    photo_original.unwrap_or(cur.photo_original),
                    winds.unwrap_or_else(|| serde_json::to_string(&cur.good_winds).unwrap_or_default()),
                    id
                ],
            )?;
            get_one(c, id, &device)
        })
        .await?;
    events::publish_reload(&st, "destinations");
    Ok(Json(out))
}

fn vote_counts(c: &Connection) -> rusqlite::Result<BTreeMap<String, i64>> {
    let mut s = c.prepare_cached(
        "SELECT d.id, (SELECT COUNT(*) FROM votes v WHERE v.destination_id = d.id AND v.created_at >= ?1) FROM destinations d",
    )?;
    let rows = s.query_map([current_slot()], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
    let mut m = BTreeMap::new();
    for row in rows {
        let (id, n) = row?;
        m.insert(id.to_string(), n);
    }
    Ok(m)
}

fn my_vote(c: &Connection, device: &str) -> rusqlite::Result<Option<i64>> {
    c.query_row("SELECT destination_id FROM votes WHERE device_id = ?1 AND created_at >= ?2", params![device, current_slot()], |r| r.get(0))
        .optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_noon_and_midnight_are_separate_slots() {
        let utc = |s| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        assert_eq!(slot_start(utc("2026-11-24T14:59:59Z")), "2026-11-24T03:00:00Z");
        assert_eq!(slot_start(utc("2026-11-24T15:00:00Z")), "2026-11-24T15:00:00Z");
        assert_eq!(slot_start(utc("2026-11-25T02:59:59Z")), "2026-11-24T15:00:00Z");
        assert_eq!(slot_start(utc("2026-11-25T03:00:00Z")), "2026-11-25T03:00:00Z");
    }
}

fn publish_votes(st: &AppState, votes: &BTreeMap<String, i64>) {
    events::publish(st, "votes", &json!({ "votes": votes }));
}

/// `DELETE /api/destinations/:id` (crew) — apaga também os votos.
pub async fn delete(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
) -> ApiResult<StatusCode> {
    let votes = st
        .db
        .call(move |c| {
            let tx = c.transaction()?;
            tx.execute("DELETE FROM votes WHERE destination_id = ?1", [id])?;
            let n = tx.execute("DELETE FROM destinations WHERE id = ?1", [id])?;
            if n == 0 {
                return Err(ApiError::not_found("Destino não encontrado"));
            }
            tx.commit()?;
            Ok(vote_counts(c)?)
        })
        .await?;
    events::publish_reload(&st, "destinations");
    publish_votes(&st, &votes);
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/destinations/:id/vote` — um voto por aparelho.
pub async fn vote(
    State(st): State<AppState>,
    device: Device,
    PathParam(id): PathParam<i64>,
) -> ApiResult<Json<Value>> {
    let device = device.require()?;
    let (votes, mine) = st
        .db
        .call(move |c| {
            let exists: bool =
                c.query_row("SELECT EXISTS(SELECT 1 FROM destinations WHERE id = ?1)", [id], |r| r.get(0))?;
            if !exists {
                return Err(ApiError::not_found("Destino não encontrado"));
            }
            c.execute(
                "INSERT INTO votes(device_id, destination_id, created_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(device_id) DO UPDATE SET destination_id = excluded.destination_id,
                    created_at = excluded.created_at",
                params![device, id, now_str()],
            )?;
            Ok((vote_counts(c)?, my_vote(c, &device)?))
        })
        .await?;
    publish_votes(&st, &votes);
    Ok(Json(json!({ "votes": votes, "my_vote": mine })))
}

/// `DELETE /api/votes/mine`
pub async fn unvote(State(st): State<AppState>, device: Device) -> ApiResult<Json<Value>> {
    let device = device.require()?;
    let votes = st
        .db
        .call(move |c| {
            c.execute("DELETE FROM votes WHERE device_id = ?1", [&device])?;
            Ok(vote_counts(c)?)
        })
        .await?;
    publish_votes(&st, &votes);
    Ok(Json(json!({ "votes": votes, "my_vote": null })))
}

/// `POST /api/votes/reset` (crew)
pub async fn reset(State(st): State<AppState>, _crew: Crew) -> ApiResult<Json<Value>> {
    let votes = st
        .db
        .call(|c| {
            c.execute("DELETE FROM votes", [])?;
            Ok(vote_counts(c)?)
        })
        .await?;
    publish_votes(&st, &votes);
    Ok(Json(json!({ "votes": votes, "my_vote": null })))
}
