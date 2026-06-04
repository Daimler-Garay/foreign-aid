use axum::{
    extract::FromRequestParts,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{COOKIE, SET_COOKIE},
        request::Parts,
    },
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{
    api::error::ApiError,
    application::{repositories::session_repo, state::SharedState},
    domain::models::auth::AuthenticatedUser,
};

pub const SESSION_COOKIE_NAME: &str = "coup_session";
pub const SESSION_TTL_HOURS: i64 = 24 * 30;

pub fn new_session_id() -> Uuid {
    Uuid::new_v4()
}

pub fn session_expires_at() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::hours(SESSION_TTL_HOURS)
}

pub fn session_cookie(session_id: Uuid, secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    let value = format!(
        "{SESSION_COOKIE_NAME}={session_id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure_attr}",
        SESSION_TTL_HOURS * 60 * 60
    );

    HeaderValue::from_str(&value).expect("session cookie header should be valid")
}

pub fn expired_session_cookie(secure: bool) -> HeaderValue {
    let secure_attr = if secure { "; Secure" } else { "" };
    let value =
        format!("{SESSION_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure_attr}");

    HeaderValue::from_str(&value).expect("expired session cookie header should be valid")
}

pub fn append_session_cookie(headers: &mut HeaderMap, session_id: Uuid, secure: bool) {
    headers.append(SET_COOKIE, session_cookie(session_id, secure));
}

pub fn append_expired_session_cookie(headers: &mut HeaderMap, secure: bool) {
    headers.append(SET_COOKIE, expired_session_cookie(secure));
}

pub fn session_id_from_headers(headers: &HeaderMap) -> Option<Uuid> {
    let cookies = headers.get(COOKIE)?.to_str().ok()?;

    cookies.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == SESSION_COOKIE_NAME)
            .then(|| Uuid::parse_str(value).ok())
            .flatten()
    })
}

impl FromRequestParts<SharedState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let session_id = session_id_from_headers(&parts.headers).ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication is required.",
            )
        })?;

        session_repo::find_authenticated_user_by_session_id(&state.db_pool, session_id)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "unauthorized",
                    "Authentication is required.",
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use axum::http::header::COOKIE;

    use super::*;

    #[test]
    fn session_cookie_uses_secure_http_only_settings() {
        let session_id = Uuid::from_u128(1);
        let cookie = session_cookie(session_id, true);
        let cookie = cookie.to_str().expect("cookie should be visible in test");

        assert!(cookie.contains("coup_session=00000000-0000-0000-0000-000000000001"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Max-Age="));
    }

    #[test]
    fn expired_cookie_clears_session() {
        let cookie = expired_session_cookie(true);
        let cookie = cookie.to_str().expect("cookie should be visible in test");

        assert!(cookie.contains("coup_session="));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
    }

    #[test]
    fn extracts_session_id_from_cookie_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            "theme=dark; coup_session=00000000-0000-0000-0000-000000000001"
                .parse()
                .expect("cookie header should parse"),
        );

        assert_eq!(session_id_from_headers(&headers), Some(Uuid::from_u128(1)));
    }

    #[test]
    fn ignores_invalid_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            "coup_session=not-a-uuid"
                .parse()
                .expect("cookie header should parse"),
        );

        assert_eq!(session_id_from_headers(&headers), None);
    }
}
