use std::convert::Infallible;

use axum::extract::{FromRequest, FromRequestParts};
use axum::http::request::Parts;
use rusqlite::OptionalExtension;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// `Json<T>` com erro no formato da API.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct JsonBody<T>(pub T);

/// `Path<T>` com erro no formato da API.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct PathParam<T>(pub T);

/// `Query<T>` com erro no formato da API.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct QueryParams<T>(pub T);

/// Cabeçalho `X-Device-Id` (opcional).
pub struct Device(pub Option<String>);

impl Device {
    pub fn require(&self) -> ApiResult<String> {
        self.0
            .clone()
            .ok_or_else(|| ApiError::bad_request("Cabeçalho X-Device-Id ausente"))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Device {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let v = parts
            .headers
            .get("x-device-id")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .map(String::from);
        Ok(Device(v))
    }
}

/// Tripulante identificado por `X-Crew-Id`; rejeita com 403 se ausente ou inexistente.
#[derive(Clone, Debug)]
pub struct Crew {
    pub id: i64,
    pub name: String,
}

/// Tripulante opcional (cabeçalho ausente ou inválido → `None`).
pub struct OptCrew(pub Option<Crew>);

async fn lookup_crew(parts: &Parts, state: &AppState) -> ApiResult<Option<Crew>> {
    let Some(id) = parts
        .headers
        .get("x-crew-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<i64>().ok())
    else {
        return Ok(None);
    };
    state
        .db
        .call(move |c| {
            Ok(c.query_row("SELECT id, name FROM crew WHERE id = ?1", [id], |r| {
                Ok(Crew { id: r.get(0)?, name: r.get(1)? })
            })
            .optional()?)
        })
        .await
}

impl FromRequestParts<AppState> for Crew {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        lookup_crew(parts, state).await?.ok_or_else(ApiError::forbidden)
    }
}

impl FromRequestParts<AppState> for OptCrew {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(OptCrew(lookup_crew(parts, state).await?))
    }
}
