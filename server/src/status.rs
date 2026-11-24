use std::collections::VecDeque;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiResult;
use crate::events;
use crate::extract::{Device, JsonBody};
use crate::settings;
use crate::state::AppState;
use crate::util::{bearing_deg, check_lat_lon, haversine_m, parse_ts, round_to, ts};

const MPS_TO_KN: f64 = 3600.0 / 1852.0;
const MAX_ACCURACY_M: f64 = 150.0;
const MIN_DT_S: i64 = 20;
const MAX_DT_S: i64 = 600;
const STALE_MIN: i64 = 30;
const KEEP_DAYS: i64 = 30;

#[derive(Clone, Debug)]
pub struct PosRec {
    pub lat: f64,
    pub lon: f64,
    pub at: DateTime<Utc>,
    pub speed_kn: Option<f64>,
    pub heading: Option<f64>,
}

/// Últimas posições aceitas (janela de 10 min + a mais recente).
#[derive(Default)]
pub struct StatusMem {
    recent: VecDeque<PosRec>,
}

impl StatusMem {
    fn latest(&self) -> Option<&PosRec> {
        self.recent.back()
    }

    fn push(&mut self, p: PosRec) {
        let cutoff = p.at - Duration::seconds(MAX_DT_S);
        self.recent.push_back(p);
        while self.recent.len() > 1 && self.recent.front().is_some_and(|f| f.at < cutoff) {
            self.recent.pop_front();
        }
    }

    /// Posição mais recente que seja de 20 s a 10 min mais antiga que `at`.
    fn reference(&self, at: DateTime<Utc>) -> Option<&PosRec> {
        self.recent.iter().rev().find(|p| (at - p.at).num_seconds() >= MIN_DT_S).filter(|p| (at - p.at).num_seconds() <= MAX_DT_S)
    }
}

/// Carrega do banco as posições recentes (após reiniciar).
pub fn load_from_db(st: &AppState) {
    let res: rusqlite::Result<Vec<PosRec>> = st.db.blocking(|c| {
        let mut s = c.prepare(
            "SELECT lat, lon, at, speed_kn, heading FROM positions ORDER BY id DESC LIMIT 200",
        )?;
        let rows = s.query_map([], |r| {
            Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?, r.get(3)?, r.get(4)?))
        })?;
        let mut v = Vec::new();
        for row in rows {
            let (lat, lon, at, speed_kn, heading) = row?;
            if let Some(at) = parse_ts(&at) {
                v.push(PosRec { lat, lon, at, speed_kn, heading });
            }
        }
        v.reverse();
        Ok(v)
    });
    match res {
        Ok(v) => {
            let mut mem = st.status.lock().unwrap_or_else(|e| e.into_inner());
            for p in v {
                mem.push(p);
            }
        }
        Err(e) => tracing::error!("falha ao carregar posições: {e}"),
    }
}

pub fn status_json(p: Option<&PosRec>, threshold_kn: f64) -> Value {
    match p {
        None => json!({
            "lat": null, "lon": null, "speed_kn": null, "moving": false,
            "heading": null, "updated_at": null, "stale": true
        }),
        Some(p) => {
            let stale = Utc::now() - p.at > Duration::minutes(STALE_MIN);
            json!({
                "lat": p.lat,
                "lon": p.lon,
                "speed_kn": p.speed_kn,
                "moving": p.speed_kn.is_some_and(|s| s > threshold_kn),
                "heading": p.heading,
                "updated_at": ts(p.at),
                "stale": stale,
            })
        }
    }
}

/// `GET /api/status`
pub async fn get(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let s = settings::get_settings(&st).await?;
    let mem = st.status.lock().unwrap_or_else(|e| e.into_inner());
    Ok(Json(status_json(mem.latest(), s.moving_threshold_kn)))
}

#[derive(Deserialize)]
pub struct PositionIn {
    lat: f64,
    lon: f64,
    accuracy_m: Option<f64>,
    speed_mps: Option<f64>,
    heading: Option<f64>,
}

/// `POST /api/position` → 204
pub async fn post(
    State(st): State<AppState>,
    Device(device): Device,
    JsonBody(b): JsonBody<PositionIn>,
) -> ApiResult<StatusCode> {
    check_lat_lon(b.lat, b.lon)?;
    let accuracy = b.accuracy_m.filter(|a| a.is_finite());
    if accuracy.is_some_and(|a| a > MAX_ACCURACY_M) {
        return Ok(StatusCode::NO_CONTENT);
    }
    let s = settings::get_settings(&st).await?;
    let now = Utc::now();
    let here = (b.lat, b.lon);
    let given_speed = b.speed_mps.filter(|v| v.is_finite() && *v >= 0.0);
    let given_heading = b.heading.filter(|h| h.is_finite()).map(|h| h.rem_euclid(360.0));

    let (rec, status) = {
        let mut mem = st.status.lock().unwrap_or_else(|e| e.into_inner());
        let reference = mem.reference(now).map(|p| ((p.lat, p.lon), p.at));
        let speed_kn = match (given_speed, reference) {
            (Some(mps), _) => Some(mps * MPS_TO_KN),
            (None, Some((pt, at))) => {
                let dt = (now - at).num_milliseconds() as f64 / 1000.0;
                Some(haversine_m(pt, here) / dt * MPS_TO_KN)
            }
            (None, None) => None,
        }
        .map(|v| round_to(v, 1));
        let moving = speed_kn.is_some_and(|v| v > s.moving_threshold_kn);
        let heading = given_heading
            .or_else(|| reference.filter(|_| moving).map(|(pt, _)| bearing_deg(pt, here)))
            .map(|h| round_to(h, 0));
        let rec = PosRec { lat: round_to(b.lat, 6), lon: round_to(b.lon, 6), at: now, speed_kn, heading };
        mem.push(rec.clone());
        (rec, status_json(mem.latest(), s.moving_threshold_kn))
    };

    let speed_mps = given_speed;
    st.db
        .call(move |c| {
            c.execute(
                "INSERT INTO positions(device_id, lat, lon, accuracy_m, speed_mps, heading, speed_kn, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![device, rec.lat, rec.lon, accuracy, speed_mps, rec.heading, rec.speed_kn, ts(rec.at)],
            )?;
            let cutoff = ts(Utc::now() - Duration::days(KEEP_DAYS));
            c.execute("DELETE FROM positions WHERE at < ?1", [cutoff])?;
            Ok(())
        })
        .await?;
    events::publish(&st, "status", &status);
    Ok(StatusCode::NO_CONTENT)
}
