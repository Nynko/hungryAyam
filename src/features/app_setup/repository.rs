use anyhow::Result;
use sqlx::PgPool;

use crate::{
    features::app_setup::{
    domain::AppSetup,
    db_model::AppSetupRow,
    dto::AppSetupRequest
    },
    types::utils::option_to_string,
    traits::domain_traits::IntoDomain
};

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
        let setup_row = sqlx::query_as!(
            AppSetupRow,
            r#"
            INSERT INTO app_settings (id,title,image_url)
            VALUES (1,$1, $2)
            RETURNING id, title, image_url, created_at, updated_at
            "#,
            request.app_name,
            option_to_string(request.image_url)
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(setup_row.into_domain())
    }

}
