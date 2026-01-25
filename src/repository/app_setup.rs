use anyhow::Result;
use sqlx::PgPool;

use crate::domain::app_setup::AppSetup;
use crate::api::dtos::app_setup::AppSetupRequest;

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
        let restaurant = sqlx::query_as!(
            AppSetup,
            r#"
            INSERT INTO app_settings (title)
            VALUES ($1)
            RETURNING id, title, created_at, updated_at
            "#,
            request.app_name
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(restaurant)
    }

}
