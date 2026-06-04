use std::{fmt, net::SocketAddr};

use thiserror::Error;

use crate::db::{DatabaseOptions, options::PostgresOptions};

#[derive(Clone)]
pub struct Config {
    pub app_env: String,
    pub app_host: String,
    pub app_port: u16,
    pub database: PostgresOptions,
}

impl Config {
    pub fn service_socket_addr(&self) -> Result<SocketAddr, ConfigError> {
        format!("{}:{}", self.app_host, self.app_port)
            .parse()
            .map_err(ConfigError::InvalidSocketAddress)
    }

    fn load_from_lookup<F>(lookup: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let app_env = lookup("APP_ENV").unwrap_or_else(|| "development".to_owned());
        let app_host = get_any(&lookup, &["APP_HOST", "SERVICE_HOST"])?;
        let app_port = parse_any(&lookup, &["APP_PORT", "SERVICE_PORT"])?;
        let max_connections = parse_any_optional(
            &lookup,
            &["DATABASE_MAX_CONNECTIONS", "POSTGRES_CONNECTION_POOL"],
        )?
        .unwrap_or(5);

        let database = if let Some(database_url) = lookup("DATABASE_URL") {
            PostgresOptions {
                database_url: Some(database_url),
                db: String::new(),
                host: String::new(),
                port: 0,
                user: String::new(),
                password: String::new(),
                max_connections,
            }
        } else {
            PostgresOptions {
                database_url: None,
                db: get_any(&lookup, &["POSTGRES_DB"])?,
                host: get_any(&lookup, &["POSTGRES_HOST"])?,
                port: parse_any(&lookup, &["POSTGRES_PORT"])?,
                user: get_any(&lookup, &["POSTGRES_USER"])?,
                password: get_any(&lookup, &["POSTGRES_PASSWORD"])?,
                max_connections,
            }
        };

        Ok(Self {
            app_env,
            app_host,
            app_port,
            database,
        })
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("app_env", &self.app_env)
            .field("app_host", &self.app_host)
            .field("app_port", &self.app_port)
            .field("database", &self.database)
            .finish()
    }
}

pub fn load() -> Result<Config, ConfigError> {
    let env_file = if env_get_or("ENV_TEST", "0") == "1" {
        ".env_test"
    } else {
        ".env"
    };

    if dotenvy::from_filename(env_file).is_ok() {
        tracing::info!(env_file, "environment file loaded");
    } else {
        tracing::info!(
            env_file,
            "environment file not found, using existing environment"
        );
    }

    let config = Config::load_from_lookup(|key| std::env::var(key).ok())?;
    tracing::debug!(?config, "configuration loaded");
    Ok(config)
}

impl From<Config> for DatabaseOptions {
    fn from(config: Config) -> Self {
        Self {
            postgres: config.database,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable; tried one of: {0}")]
    MissingRequired(String),
    #[error("failed to parse environment variable '{key}'")]
    Parse { key: String },
    #[error("invalid application socket address")]
    InvalidSocketAddress(#[source] std::net::AddrParseError),
}

fn get_any<F>(lookup: &F, keys: &[&str]) -> Result<String, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    keys.iter()
        .find_map(|key| lookup(key))
        .ok_or_else(|| ConfigError::MissingRequired(keys.join(" or ")))
}

fn parse_any<F, T>(lookup: &F, keys: &[&str]) -> Result<T, ConfigError>
where
    F: Fn(&str) -> Option<String>,
    T: std::str::FromStr,
{
    let value = get_any(lookup, keys)?;
    value.parse().map_err(|_| ConfigError::Parse {
        key: keys.join(" or "),
    })
}

fn parse_any_optional<F, T>(lookup: &F, keys: &[&str]) -> Result<Option<T>, ConfigError>
where
    F: Fn(&str) -> Option<String>,
    T: std::str::FromStr,
{
    let Some((key, value)) = keys
        .iter()
        .find_map(|key| lookup(key).map(|value| ((*key).to_owned(), value)))
    else {
        return Ok(None);
    };

    value
        .parse()
        .map(Some)
        .map_err(|_| ConfigError::Parse { key })
}

#[inline]
fn env_get_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_from(values: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let values: HashMap<&str, &str> = values.iter().copied().collect();
        Config::load_from_lookup(|key| values.get(key).map(|value| (*value).to_owned()))
    }

    #[test]
    fn loads_planned_environment_keys() {
        let config = config_from(&[
            ("APP_HOST", "127.0.0.1"),
            ("APP_PORT", "3000"),
            ("DATABASE_URL", "postgres://user:secret@localhost:5432/app"),
            ("DATABASE_MAX_CONNECTIONS", "7"),
        ])
        .expect("config should load");

        assert_eq!(config.app_host, "127.0.0.1");
        assert_eq!(config.app_port, 3000);
        assert_eq!(config.database.max_connections(), 7);
        assert_eq!(
            config.database.connection_url(),
            "postgres://user:secret@localhost:5432/app"
        );
    }

    #[test]
    fn supports_legacy_environment_keys() {
        let config = config_from(&[
            ("SERVICE_HOST", "127.0.0.1"),
            ("SERVICE_PORT", "3000"),
            ("POSTGRES_USER", "admin"),
            ("POSTGRES_PASSWORD", "admin"),
            ("POSTGRES_HOST", "localhost"),
            ("POSTGRES_PORT", "5433"),
            ("POSTGRES_DB", "foreign_aid"),
            ("POSTGRES_CONNECTION_POOL", "5"),
        ])
        .expect("config should load");

        assert_eq!(config.app_host, "127.0.0.1");
        assert_eq!(config.app_port, 3000);
        assert_eq!(
            config.database.connection_url(),
            "postgres://admin:admin@localhost:5433/foreign_aid"
        );
    }

    #[test]
    fn rejects_missing_required_keys() {
        let error = config_from(&[]).expect_err("config should fail");

        assert!(matches!(error, ConfigError::MissingRequired(_)));
    }

    #[test]
    fn debug_output_redacts_database_secret() {
        let config = config_from(&[
            ("APP_HOST", "127.0.0.1"),
            ("APP_PORT", "3000"),
            ("DATABASE_URL", "postgres://user:secret@localhost:5432/app"),
        ])
        .expect("config should load");

        let debug = format!("{config:?}");

        assert!(!debug.contains("secret"));
        assert!(debug.contains("<redacted>"));
    }
}
