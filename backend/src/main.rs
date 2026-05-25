use axum::{Router, routing::get};

mod db;

#[tokio::main]
async fn main() {
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
