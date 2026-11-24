use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::util::now_str;

const TTL: Duration = Duration::from_secs(30 * 60);
/// Ponto mais ao mar, usado se a API marine devolver só nulos no ponto configurado.
const MARINE_FALLBACK: (f64, f64) = (-27.38, -48.45);
pub const JURERE: (f64, f64) = (-27.425, -48.495);

pub struct WeatherCache {
    at: Instant,
    key: (i64, i64),
    value: Value,
}

fn key(lat: f64, lon: f64) -> (i64, i64) {
    ((lat * 1e4).round() as i64, (lon * 1e4).round() as i64)
}

/// `GET /api/weather`
pub async fn get(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    // Nunca usar o GPS do aparelho: a previsão é sempre do mar ao norte
    // de Jurerê Internacional, inclusive quando o passageiro está em casa.
    let (lat, lon) = JURERE;
    let k = key(lat, lon);
    let mut cache = st.weather.lock().await;
    if let Some(c) = cache.as_ref().filter(|c| c.key == k && c.at.elapsed() < TTL) {
        return Ok(Json(c.value.clone()));
    }
    match fetch(&st, lat, lon).await {
        Ok(v) => {
            *cache = Some(WeatherCache { at: Instant::now(), key: k, value: v.clone() });
            Ok(Json(v))
        }
        Err(e) => {
            tracing::warn!("previsão indisponível: {e}");
            match cache.as_ref().filter(|c| c.key == k) {
                Some(c) => Ok(Json(c.value.clone())),
                None => Err(ApiError::bad_gateway("Previsão do tempo indisponível no momento")),
            }
        }
    }
}

async fn get_json(st: &AppState, url: &str, query: &[(&str, String)]) -> Result<Value, String> {
    let res = st
        .http
        .get(url)
        .query(query)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    let status = res.status();
    let body: Value = res.json().await.map_err(|e| format!("{url}: resposta inválida: {e}"))?;
    if !status.is_success() || body.get("error").and_then(Value::as_bool) == Some(true) {
        let reason = body.get("reason").and_then(Value::as_str).unwrap_or("erro");
        return Err(format!("{url}: HTTP {status}: {reason}"));
    }
    Ok(body)
}

fn series<'a>(hourly: &'a Value, name: &str) -> &'a [Value] {
    hourly.get(name).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

type Marine = HashMap<String, [Value; 4]>;

async fn marine_at(st: &AppState, lat: f64, lon: f64) -> Result<Marine, String> {
    let v = get_json(
        st,
        "https://marine-api.open-meteo.com/v1/marine",
        &[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("hourly", "wave_height,wave_direction,swell_wave_height,swell_wave_direction".into()),
            ("timezone", st.cfg.tz_name.clone()),
            ("forecast_days", "2".into()),
        ],
    )
    .await?;
    let h = v.get("hourly").ok_or("marine: sem 'hourly'")?;
    let times = series(h, "time");
    let cols = [
        series(h, "wave_height"),
        series(h, "wave_direction"),
        series(h, "swell_wave_height"),
        series(h, "swell_wave_direction"),
    ];
    let mut out = Marine::new();
    let mut any = false;
    for (i, t) in times.iter().enumerate() {
        let Some(t) = t.as_str() else { continue };
        let vals = cols.map(|c| c.get(i).cloned().unwrap_or(Value::Null));
        any |= vals.iter().any(|v| !v.is_null());
        out.insert(t.to_string(), vals);
    }
    if !any {
        return Err("marine: só valores nulos".into());
    }
    Ok(out)
}

async fn marine(st: &AppState, lat: f64, lon: f64) -> Marine {
    match marine_at(st, lat, lon).await {
        Ok(m) => m,
        Err(e) => {
            tracing::info!("ondas no ponto configurado indisponíveis ({e}); tentando ponto ao mar");
            marine_at(st, MARINE_FALLBACK.0, MARINE_FALLBACK.1).await.unwrap_or_else(|e| {
                tracing::warn!("ondas indisponíveis: {e}");
                Marine::new()
            })
        }
    }
}

async fn fetch(st: &AppState, lat: f64, lon: f64) -> Result<Value, String> {
    let query = [
        ("latitude", lat.to_string()),
        ("longitude", lon.to_string()),
        ("hourly", "wind_speed_10m,wind_direction_10m,wind_gusts_10m".into()),
        ("wind_speed_unit", "kn".into()),
        ("timezone", st.cfg.tz_name.clone()),
        ("forecast_days", "2".into()),
    ];
    let forecast = get_json(st, "https://api.open-meteo.com/v1/forecast", &query);
    let (forecast, marine) = tokio::join!(forecast, marine(st, lat, lon));
    let forecast = forecast?;
    let h = forecast.get("hourly").ok_or("forecast: sem 'hourly'")?;
    let times = series(h, "time");
    let ws = series(h, "wind_speed_10m");
    let wd = series(h, "wind_direction_10m");
    let wg = series(h, "wind_gusts_10m");
    let at = |s: &[Value], i: usize| s.get(i).cloned().unwrap_or(Value::Null);
    let null4 = [Value::Null, Value::Null, Value::Null, Value::Null];
    let hours: Vec<Value> = times
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let t = t.as_str()?;
            let m = marine.get(t).unwrap_or(&null4);
            Some(json!({
                "time": t,
                "wind_kn": at(ws, i),
                "gust_kn": at(wg, i),
                "wind_dir": at(wd, i),
                "wave_m": m[0],
                "wave_dir": m[1],
                "swell_m": m[2],
                "swell_dir": m[3],
            }))
        })
        .collect();
    if hours.is_empty() {
        return Err("forecast: sem horas".into());
    }
    let tz = forecast.get("timezone").and_then(Value::as_str).unwrap_or(&st.cfg.tz_name).to_string();
    Ok(json!({ "updated_at": now_str(), "timezone": tz, "location": "Jurerê Internacional",
        "lat": lat, "lon": lon, "hours": hours }))
}
