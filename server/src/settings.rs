use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::extract::{Crew, JsonBody};
use crate::state::AppState;

#[derive(Clone, Debug, Serialize)]
pub struct Settings {
    pub text_model: String,
    pub image_model: String,
    pub edit_model: String,
    pub cruise_speed_kn: f64,
    pub moving_threshold_kn: f64,
    pub weather_lat: f64,
    pub weather_lon: f64,
}

const KEY_OPENROUTER: &str = "openrouter_key";

fn raw(conn: &Connection) -> rusqlite::Result<HashMap<String, Value>> {
    let mut stmt = conn.prepare_cached("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut map = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        if let Ok(v) = serde_json::from_str::<Value>(&v) {
            map.insert(k, v);
        }
    }
    Ok(map)
}

/// Configurações efetivas + chave do OpenRouter salva no banco (se houver).
pub fn load(conn: &Connection, _st: &AppState) -> rusqlite::Result<(Settings, Option<String>)> {
    let m = raw(conn)?;
    let s = |k: &str, d: &str| m.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let n = |k: &str, d: f64| m.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).unwrap_or(d);
    let settings = Settings {
        text_model: s("text_model", "google/gemini-3.8-flash"),
        image_model: s("image_model", "google/gemini-3.1-flash-image"),
        edit_model: s("edit_model", "google/gemini-3.1-flash-image"),
        cruise_speed_kn: n("cruise_speed_kn", 18.0),
        moving_threshold_kn: n("moving_threshold_kn", 1.5),
        weather_lat: crate::weather::JURERE.0,
        weather_lon: crate::weather::JURERE.1,
    };
    Ok((settings, None))
}

/// Carrega as configurações (assíncrono).
pub async fn get_settings(st: &AppState) -> ApiResult<Settings> {
    let st2 = st.clone();
    st.db.call(move |c| Ok(load(c, &st2)?.0)).await
}

/// Chave efetiva do OpenRouter: variável de ambiente tem prioridade.
pub async fn openrouter_key(st: &AppState) -> ApiResult<Option<String>> {
    Ok(st.cfg.env_openrouter_key.clone())
}

fn to_json(st: &AppState, s: &Settings, db_key: &Option<String>) -> Value {
    let mut v = serde_json::to_value(s).unwrap_or_else(|_| json!({}));
    if let Value::Object(o) = &mut v {
        o.insert(
            "openrouter_key_set".into(),
            json!(st.cfg.env_openrouter_key.is_some() || db_key.is_some()),
        );
        o.insert("vapid_public_key".into(), json!(st.vapid.public_b64));
    }
    v
}

/// `GET /api/settings`
pub async fn get(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let st2 = st.clone();
    let (s, key) = st.db.call(move |c| Ok(load(c, &st2)?)).await?;
    Ok(Json(to_json(&st, &s, &key)))
}

fn validate(key: &str, v: &Value) -> ApiResult<Option<Value>> {
    let num = |min: f64, max: f64, min_inclusive: bool| -> ApiResult<Option<Value>> {
        let n = v
            .as_f64()
            .filter(|n| n.is_finite() && *n <= max && (if min_inclusive { *n >= min } else { *n > min }))
            .ok_or_else(|| ApiError::bad_request(format!("Valor inválido para '{key}'")))?;
        Ok(Some(json!(n)))
    };
    match key {
        "text_model" | "image_model" | "edit_model" => {
            let s = v
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.len() <= 200)
                .ok_or_else(|| ApiError::bad_request(format!("Valor inválido para '{key}'")))?;
            Ok(Some(json!(s)))
        }
        "cruise_speed_kn" => num(0.0, 100.0, false),
        "moving_threshold_kn" => num(0.0, 50.0, true),
        "weather_lat" | "weather_lon" => Err(ApiError::bad_request("Vento fixo em Jurerê Internacional")),
        _ => Ok(None),
    }
}

/// `PUT /api/settings` (crew) — atualização parcial.
pub async fn put(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(body): JsonBody<Map<String, Value>>,
) -> ApiResult<Json<Value>> {
    let mut updates: Vec<(String, Option<String>)> = Vec::new();
    for (k, v) in &body {
        if k == KEY_OPENROUTER {
            return Err(ApiError::bad_request("Configure OPENROUTER_API_KEY no servidor"));
        }
        if let Some(val) = validate(k, v)? {
            updates.push((k.clone(), Some(val.to_string())));
        }
    }
    let st2 = st.clone();
    let (s, key) = st
        .db
        .call(move |c| {
            let tx = c.transaction()?;
            for (k, v) in &updates {
                match v {
                    Some(v) => tx.execute(
                        "INSERT INTO settings(key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        params![k, v],
                    )?,
                    None => tx.execute("DELETE FROM settings WHERE key = ?1", [k])?,
                };
            }
            tx.commit()?;
            Ok(load(c, &st2)?)
        })
        .await?;
    events::publish_reload(&st, "settings");
    Ok(Json(to_json(&st, &s, &key)))
}
