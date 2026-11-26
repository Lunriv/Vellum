use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{str::FromStr, time::Duration};

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}
impl Database {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;
        sqlx::raw_sql(include_str!("../../migrations/0001_initial.sql"))
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }
    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        Self::connect("sqlite::memory:").await
    }
}
