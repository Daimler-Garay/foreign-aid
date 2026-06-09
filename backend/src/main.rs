use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use backend::application::app;

#[tokio::main]
async fn main() {
    init_tracing();

    tracing::info!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    if let Err(error) = run_command().await {
        tracing::error!(%error, "application startup failed");
        std::process::exit(1);
    }
}

async fn run_command() -> Result<(), app::StartupError> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    match args.as_slice() {
        [] => app::run().await,
        [command, username, password] if command == "seed-admin" => {
            let username = app::seed_admin(username, password).await?;
            tracing::info!(username, "admin user seeded");
            Ok(())
        }
        [command, ..] if command == "seed-admin" => Err(app::StartupError::SeedAdmin(
            "usage: cargo run -- seed-admin <username> <password>".to_owned(),
        )),
        [command, ..] => Err(app::StartupError::SeedAdmin(format!(
            "unknown command '{command}'"
        ))),
    }
}

fn init_tracing() {
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "backend=info,tower_http=info".into());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_target(false)
        .with_file(true)
        .with_line_number(true);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
