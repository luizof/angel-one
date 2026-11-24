use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer};

use crate::error::{ApiError, ApiResult};

/// RFC 3339 UTC com segundos e sufixo `Z` (`2026-11-19T02:10:00Z`).
pub fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn now_str() -> String {
    ts(Utc::now())
}

pub fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

pub fn round_to(x: f64, decimals: i32) -> f64 {
    let m = 10f64.powi(decimals);
    (x * m).round() / m
}

const EARTH_R: f64 = 6_371_008.8;

/// Distância em metros entre dois pontos (lat, lon) em graus.
pub fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1) = (a.0.to_radians(), a.1.to_radians());
    let (la2, lo2) = (b.0.to_radians(), b.1.to_radians());
    let dla = la2 - la1;
    let dlo = lo2 - lo1;
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * EARTH_R * h.sqrt().min(1.0).asin()
}

/// Rumo inicial (graus, 0 = norte) de `a` para `b`.
pub fn bearing_deg(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1) = (a.0.to_radians(), a.1.to_radians());
    let (la2, lo2) = (b.0.to_radians(), b.1.to_radians());
    let dlo = lo2 - lo1;
    let y = dlo.sin() * la2.cos();
    let x = la1.cos() * la2.sin() - la1.sin() * la2.cos() * dlo.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

/// Para PATCH: distingue campo ausente (`None`) de `null` (`Some(None)`).
pub fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Texto obrigatório: aparado, não vazio e com no máximo `max` caracteres.
pub fn required_text(v: &str, field: &str, max: usize) -> ApiResult<String> {
    let t = v.trim();
    if t.is_empty() {
        return Err(ApiError::bad_request(format!("Campo '{field}' é obrigatório")));
    }
    if t.chars().count() > max {
        return Err(ApiError::bad_request(format!(
            "Campo '{field}' muito longo (máx. {max} caracteres)"
        )));
    }
    Ok(t.to_string())
}

/// Texto opcional: aparado e limitado; mantém vazio como vazio.
pub fn limited_text(v: &str, field: &str, max: usize) -> ApiResult<String> {
    let t = v.trim();
    if t.chars().count() > max {
        return Err(ApiError::bad_request(format!(
            "Campo '{field}' muito longo (máx. {max} caracteres)"
        )));
    }
    Ok(t.to_string())
}

/// Texto anulável: `None`/vazio viram `None`.
pub fn nullable_text(v: Option<&str>, field: &str, max: usize) -> ApiResult<Option<String>> {
    match v {
        None => Ok(None),
        Some(s) => {
            let t = limited_text(s, field, max)?;
            Ok(if t.is_empty() { None } else { Some(t) })
        }
    }
}

pub fn check_lat_lon(lat: f64, lon: f64) -> ApiResult<()> {
    if !lat.is_finite() || !lon.is_finite() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(ApiError::bad_request("Coordenadas inválidas"));
    }
    Ok(())
}

pub fn check_non_negative(v: f64, field: &str) -> ApiResult<f64> {
    if !v.is_finite() || v < 0.0 {
        return Err(ApiError::bad_request(format!("Campo '{field}' deve ser um número ≥ 0")));
    }
    Ok(v)
}
