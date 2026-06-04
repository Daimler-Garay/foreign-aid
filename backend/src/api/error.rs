use std::fmt::{Display, Formatter, Result};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub status: u16,
    pub errors: Vec<ApiErrorEntry>,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let api_error = serde_json::to_string_pretty(&self).unwrap_or_default();
        write!(f, "{}", api_error)
    }
}

#[derive(Debug, Copy, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    MatchNotFound,
    DuplicateDisplayName,
    InvalidDisplayName,
    PlayerNotFound,
    ResourceNotFound,
    ApiVersionError,
    DatabaseError,
    ServiceUnavailable,
}

impl Display for ApiErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}",
            serde_json::json!(self).as_str().unwrap_or_default()
        )
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorKind {
    ResourceNotFound,
    ValidationError,
    DatabaseError,
}

impl Display for ApiErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}",
            serde_json::json!(self).as_str().unwrap_or_default()
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApiErrorEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

impl ApiErrorEntry {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_owned(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    pub fn code<S: ToString>(mut self, code: S) -> Self {
        self.code = Some(code.to_string());
        self
    }

    pub fn kind<S: ToString>(mut self, kind: S) -> Self {
        self.kind = Some(kind.to_string());
        self
    }

    pub fn trace_id(mut self) -> Self {
        let mut trace_id = uuid::Uuid::new_v4().to_string();
        trace_id.retain(|c| c != '-');
        self.trace_id = Some(trace_id);
        self
    }

    fn public_code(&self, fallback_status: StatusCode) -> String {
        self.code
            .clone()
            .unwrap_or_else(|| status_code_to_code(fallback_status))
    }
}

impl From<StatusCode> for ApiErrorEntry {
    fn from(status_code: StatusCode) -> Self {
        let error_message = status_code.to_string();
        let error_code = error_message.replace(' ', "_").to_lowercase();
        Self::new(&error_message).code(error_code)
    }
}

impl From<sqlx::Error> for ApiErrorEntry {
    fn from(e: sqlx::Error) -> Self {
        let error_entry = match e {
            sqlx::Error::RowNotFound => Self::new("Resource not found.")
                .code(ApiErrorCode::ResourceNotFound)
                .kind(ApiErrorKind::ResourceNotFound),
            _ => Self::new("A database error occurred.")
                .code(ApiErrorCode::DatabaseError)
                .kind(ApiErrorKind::DatabaseError),
        }
        .trace_id();

        let trace_id = error_entry.trace_id.as_deref().unwrap_or("");
        tracing::error!(%e, trace_id, "SQLx error");

        error_entry
    }
}

impl ApiError {
    pub fn new<S1, S2>(status_code: StatusCode, code: S1, message: S2) -> Self
    where
        S1: ToString,
        S2: AsRef<str>,
    {
        Self {
            status: status_code.as_u16(),
            errors: vec![ApiErrorEntry::new(message.as_ref()).code(code)],
        }
    }

    pub fn not_found<S1, S2>(code: S1, message: S2) -> Self
    where
        S1: ToString,
        S2: AsRef<str>,
    {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn service_unavailable<S1, S2>(code: S1, message: S2) -> Self
    where
        S1: ToString,
        S2: AsRef<str>,
    {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }
}

impl From<(StatusCode, Vec<ApiErrorEntry>)> for ApiError {
    fn from(error_from: (StatusCode, Vec<ApiErrorEntry>)) -> Self {
        let (status_code, errors) = error_from;
        Self {
            status: status_code.as_u16(),
            errors,
        }
    }
}
impl From<(StatusCode, ApiErrorEntry)> for ApiError {
    fn from(error_from: (StatusCode, ApiErrorEntry)) -> Self {
        let (status_code, error_entry) = error_from;
        Self {
            status: status_code.as_u16(),
            errors: vec![error_entry],
        }
    }
}

impl From<StatusCode> for ApiError {
    fn from(status_code: StatusCode) -> Self {
        Self {
            status: status_code.as_u16(),
            errors: vec![status_code.into()],
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        let status_code = match error {
            sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status: status_code.as_u16(),
            errors: vec![ApiErrorEntry::from(error)],
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code =
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let error_entry = self
            .errors
            .first()
            .cloned()
            .unwrap_or_else(|| ApiErrorEntry::from(status_code));
        let code = error_entry.public_code(status_code);

        if status_code.is_server_error() {
            tracing::error!(
                status = status_code.as_u16(),
                code,
                message = error_entry.message,
                "error response"
            );
        } else {
            tracing::debug!(
                status = status_code.as_u16(),
                code,
                message = error_entry.message,
                "error response"
            );
        }

        (
            status_code,
            Json(serde_json::json!({
                "error": {
                    "code": code,
                    "message": error_entry.message,
                }
            })),
        )
            .into_response()
    }
}

fn status_code_to_code(status_code: StatusCode) -> String {
    status_code.to_string().replace(' ', "_").to_lowercase()
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};

    use super::ApiError;

    #[tokio::test]
    async fn error_response_uses_public_envelope() {
        let response = ApiError::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Placements must be sequential.",
        )
        .into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("body should be json");

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["error"]["code"], "validation_error");
        assert_eq!(value["error"]["message"], "Placements must be sequential.");
    }
}
