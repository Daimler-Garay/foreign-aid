use std::fmt;

// Database config
#[derive(Clone)]
pub struct PostgresOptions {
    pub database_url: Option<String>,
    pub db: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub max_connections: u32,
}

impl PostgresOptions {
    pub fn connection_url(&self) -> String {
        if let Some(database_url) = &self.database_url {
            return database_url.clone();
        }

        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.db
        )
    }

    #[cfg(test)]
    pub fn set_db(&mut self, postgres_db: &str) {
        self.database_url = None;
        self.db = postgres_db.to_owned()
    }

    #[cfg(test)]
    pub const fn set_max_connections(&mut self, max_connections: u32) {
        self.max_connections = max_connections
    }

    pub const fn max_connections(&self) -> u32 {
        self.max_connections
    }
}

impl fmt::Debug for PostgresOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresOptions")
            .field(
                "database_url",
                &self.database_url.as_ref().map(|_| "<redacted>"),
            )
            .field("db", &self.db)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("password", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .finish()
    }
}
