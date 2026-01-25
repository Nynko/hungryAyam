use anyhow::Result;
use sqlx::PgPool;

use crate::features::app_setup::{
    domain::AppSetup,
    dto::AppSetupRequest};

#[derive(Clone)]
pub struct AppSetupRepository {
    pool: PgPool,
}

impl AppSetupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new restaurant
    pub async fn create(&self, request: AppSetupRequest) -> Result<AppSetup> {
        let setup = sqlx::query_as!(
            AppSetup,
            r#"
            INSERT INTO app_settings (id,title)
            VALUES (1,$1)
            RETURNING id, title, created_at, updated_at
            "#,
            request.app_name
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(setup)
    }

}
