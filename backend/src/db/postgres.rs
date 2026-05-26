use chrono::Utc;
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::db::{DatabaseError, DatabaseOptions};

#[non_exhaustive]
pub struct PostgresDatabase {
    pool: PgPool,
    options: DatabaseOptions,
    test_db_to_drop: Option<String>,
}

impl PostgresDatabase {
    pub async fn connect(options: DatabaseOptions) -> Result<Self, DatabaseError> {
        // Get postgres config
        let connection_url = options.postgres.connection_url();
        let max_connections = options.postgres.max_connections();

        // Connect to DB and get a pool
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(&connection_url)
            .await?;

        tracing::info!("Connected to PostgreSQL database.");

        Ok(Self {
            pool,
            options,
            test_db_to_drop: None,
        })
    }

    pub async fn connect_test(options: DatabaseOptions) -> Result<Self, DatabaseError> {
        // Generate a temporary name for the test ddatabase.
        let nanos_since_epoch = Utc::now().timestamp_nanos_opt().unwrap();
        let test_db_name = format!("tmp_{:x}", nanos_since_epoch);

        // create a temp db
        {
            let mut options = options.clone();
            options.postgres.set_max_connections(1);
            let db = Self::connect(options).await?;
            let pool = db.pool();
            sqlx::query(r#"CREATE DATABASE {}"#)
                .bind(&test_db_name)
                .execute(pool)
                .await?;
        }

        // Connect to the temp db
        let mut test_options = options.clone();
        test_options.postgres.set_db(&test_db_name);
        let mut test_db = Self::connect(test_options).await?;

        // Prepare for the drop on close
        test_db.test_db_to_drop = Some(test_db_name.to_owned());
        test_db.options = options;

        Ok(test_db)
    }

    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn drop(&self) -> Result<(), DatabaseError> {
        if let Some(test_db_to_drop) = self.test_db_to_drop.as_ref() {
            // Close connections
            self.pool.close().await;

            // Drop temp DB
            let db = Self::connect(self.options.clone()).await?;
            let pool = db.pool();
            sqlx::query(r#"DROP DATABASE IF EXISTS {} WITH (FORCE)"#)
                .bind(test_db_to_drop)
                .execute(pool)
                .await?;
        }
        Ok(())
    }
}
