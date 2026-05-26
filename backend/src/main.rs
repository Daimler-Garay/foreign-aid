use axum::{Router, routing::get};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod db;

#[tokio::main]
async fn main() {
    // Tracing config
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "axum_web=trace".into());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_target(false)
        .with_file(true)
        .with_line_number(true);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    tracing::info!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // build our application with a route
    let app = Router::new().route("/health", get(health));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}

async fn health() -> &'static str {
    "Ok"
}
