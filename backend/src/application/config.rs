use std::net::SocketAddr;

use crate::db::{DatabaseOptions, options::PostgresOptions};

#[derive(Debug, Clone)]
pub struct Config {
    // API Config
    pub service_host: String,
    pub service_port: u16,

    // PostgreSQL config
    pub postgres_user: String,
    pub postgres_password: String,
    pub postgres_host: String,
    pub postgres_port: u16,
    pub postgres_db: String,
    pub postgres_connection_pool: u32,
}

impl Config {
    pub fn service_http_addr(&self) -> String {
        format!("{}://{}:{}", "http", self.service_host, self.service_port)
    }

    pub fn service_socket_addr(&self) -> SocketAddr {
        use std::str::FromStr;
        SocketAddr::from_str(&format!("{}:{}", self.service_host, self.service_port)).unwrap()
    }
    pub fn postgres_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.postgres_user,
            self.postgres_password,
            self.postgres_host,
            self.postgres_port,
            self.postgres_db,
        )
    }
}

pub fn load() -> Config {
    let env_file = if env_get_or("ENV_TEST", "0") == "1" {
        ".env_test"
    } else {
        ".env"
    };

    // try to load env variables from file
    if dotenvy::from_filename(env_file).is_ok() {
        tracing::info!("{} file loaded", env_file);
    } else {
        tracing::info!("{} file not found, using existing environment", env_file);
    }

    let config = Config {
        service_host: env_get("SERVICE_HOST"),
        service_port: env_parse("SERVICE_PORT"),
        postgres_user: env_get("POSTGRES_USER"),
        postgres_password: env_get("POSTGRES_PASSWORD"),
        postgres_host: env_get("POSTGRES_HOST"),
        postgres_port: env_parse("POSTGRES_PORT"),
        postgres_db: env_get("POSTGRES_DB"),
        postgres_connection_pool: env_parse("POSTGRES_CONNECTION_POOL"),
    };

    tracing::trace!("configuration: {:#?}", config);
    config
}

impl From<Config> for PostgresOptions {
    fn from(config: Config) -> Self {
        Self {
            db: config.postgres_db,
            host: config.postgres_host,
            port: config.postgres_port,
            user: config.postgres_user,
            password: config.postgres_password,
            max_connections: config.postgres_connection_pool,
        }
    }
}

impl From<Config> for DatabaseOptions {
    fn from(config: Config) -> Self {
        Self {
            postgres: config.into(),
        }
    }
}

#[inline]
fn env_get_or(key: &str, default: &str) -> String {
    if let Ok(v) = std::env::var(key) {
        return v;
    }
    default.to_owned()
}

#[inline]
fn env_parse<T: std::str::FromStr>(key: &str) -> T {
    env_get(key).parse().map_or_else(
        |_| {
            let msg = format!("Failed to parse: {}", key);
            tracing::error!(msg);
            panic!("{msg}");
        },
        |v| v,
    )
}

#[inline]
fn env_get(key: &str) -> String {
    match std::env::var(key) {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("{} {}", key, e);
            tracing::error!(msg);
            panic!("{msg}");
        }
    }
}
