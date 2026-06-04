use axum::{
    http::header,
    response::{Html, IntoResponse, Redirect},
};

pub async fn index() -> impl IntoResponse {
    Redirect::temporary("/leaderboard")
}

pub async fn login_page() -> impl IntoResponse {
    Html(include_str!("../../static/login.html"))
}

pub async fn leaderboard_page() -> impl IntoResponse {
    Html(include_str!("../../static/leaderboard.html"))
}

pub async fn players_page() -> impl IntoResponse {
    Html(include_str!("../../static/players.html"))
}

pub async fn matches_page() -> impl IntoResponse {
    Html(include_str!("../../static/matches.html"))
}

pub async fn match_detail_page() -> impl IntoResponse {
    Html(include_str!("../../static/match-detail.html"))
}

pub async fn submit_match_page() -> impl IntoResponse {
    Html(include_str!("../../static/submit-match.html"))
}

pub async fn audit_log_page() -> impl IntoResponse {
    Html(include_str!("../../static/audit-log.html"))
}

pub async fn app_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/app.css"),
    )
}

pub async fn app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../static/app.js"),
    )
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::*;

    #[tokio::test]
    async fn login_page_renders_html() {
        let response = login_page().await.into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let body = String::from_utf8(body.to_vec()).expect("body should be utf8");

        assert!(body.contains("Coup Leaderboard"));
        assert!(body.contains("login-form"));
    }

    #[tokio::test]
    async fn app_javascript_contains_page_initializers() {
        let response = app_js().await.into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let body = String::from_utf8(body.to_vec()).expect("body should be utf8");

        assert!(body.contains("initLeaderboard"));
        assert!(body.contains("initSubmitMatch"));
        assert!(body.contains("initAuditLog"));
    }
}
