use chrono::Utc;
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::db::{DatabaseError, DatabaseOptions};
