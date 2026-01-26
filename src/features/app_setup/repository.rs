use anyhow::Result;
use sqlx::PgPool;

use crate::features::app_setup::{
    domain::AppSetup,
    db_model::AppSetupRow,
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
        let image_url_string : Option<String> = request.image_url.map(|url| (*url).to_string());
        let setup_row = sqlx::query_as!(
            AppSetupRow,
            r#"
            INSERT INTO app_settings (id,title,image_url)
            VALUES (1,$1, $2)
            RETURNING id, title, image_url, created_at, updated_at
            "#,
            request.app_name,
            image_url_string
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(setup_row.into_domain())
    }

}
