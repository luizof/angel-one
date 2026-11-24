use axum::extract::multipart::{MultipartError, MultipartRejection};
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Erro padrão da API: status HTTP + `{"error": "mensagem"}`.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
    pub fn bad_request(m: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, m)
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, m)
    }
    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "Somente tripulação")
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, m)
    }
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, m)
    }
    pub fn unavailable(m: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, m)
    }
    pub fn bad_gateway(m: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, m)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status.as_u16(), self.message)
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        tracing::error!("erro no SQLite: {e}");
        ApiError::internal("Erro no banco de dados")
    }
}

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        let status = r.status();
        let msg = match &r {
            JsonRejection::MissingJsonContentType(_) => {
                "Envie o corpo como JSON (Content-Type: application/json)".to_string()
            }
            JsonRejection::BytesRejection(_) if status == StatusCode::PAYLOAD_TOO_LARGE => {
                "Corpo da requisição grande demais".to_string()
            }
            _ => format!("JSON inválido: {}", r.body_text()),
        };
        ApiError::new(status, msg)
    }
}

impl From<PathRejection> for ApiError {
    fn from(r: PathRejection) -> Self {
        ApiError::bad_request(format!("Parâmetro de rota inválido: {}", r.body_text()))
    }
}

impl From<QueryRejection> for ApiError {
    fn from(r: QueryRejection) -> Self {
        ApiError::bad_request(format!("Parâmetros inválidos: {}", r.body_text()))
    }
}

impl From<MultipartRejection> for ApiError {
    fn from(r: MultipartRejection) -> Self {
        ApiError::new(r.status(), format!("Envio multipart inválido: {}", r.body_text()))
    }
}

impl From<MultipartError> for ApiError {
    fn from(e: MultipartError) -> Self {
        let status = e.status();
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            return ApiError::new(status, "Arquivo grande demais (máx. 20 MB)");
        }
        ApiError::new(status, format!("Falha ao ler o arquivo enviado: {}", e.body_text()))
    }
}
