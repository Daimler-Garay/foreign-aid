use std::fmt::{Display, Formatter, Result};

use axum::{
    Form, Json,
    http::{StatusCode, status},
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
#[serde[rename_all = "snake_case"]]
pub enum ApiErrorCode {
    PlayerNotFound,
    ResourceNotFound,
    ApiVersionError,
    DatabaseError,
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

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.to_owned());
        self
    }

    pub fn detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn reason(mut self, reason: &str) -> Self {
        self.reason = Some(reason.to_owned());
        self
    }

    pub fn instance(mut self, instance: &str) -> Self {
        self.instance = Some(instance.to_owned());
        self
    }

    pub fn trace_id(mut self) -> Self {
        let mut trace_id = uuid::Uuid::new_v4().to_string();
        trace_id.retain(|c| c != '-');
        self.trace_id = Some(trace_id);
        self
    }

    pub fn help(mut self, help: &str) -> Self {
        self.help = Some(help.to_owned());
        self
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
        if cfg!(debug_assertions) {
            let (code, kind) = match e {
                sqlx::Error::RowNotFound => (
                    ApiErrorCode::ResourceNotFound,
                    ApiErrorKind::ResourceNotFound,
                ),
                _ => (ApiErrorCode::DatabaseError, ApiErrorKind::DatabaseError),
            };
            Self::new(&e.to_string()).code(code).kind(kind).trace_id()
        } else {
            // build the entry with trace id to for locating purposes
            let error_entry = Self::from(StatusCode::INTERNAL_SERVER_ERROR).trace_id();
            let trace_id = error_entry.trace_id.as_deref().unwrap_or("");
            // the error has to be logged so we dont fkn lose it
            tracing::error!("SQLX error: {}, trace_id: {}", e.to_string(), trace_id);
            error_entry
        }
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
        tracing::error!("Error response: {:?}", self);
        let status_code =
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status_code, Json(self)).into_response()
    }
}
