use anyhow::Result;
use sqlx::PgPool;

use crate::{
    features::app_setup::{
    domain::{AppSetup,CreateAppSetup},
    db_model::AppSetupRow,
    },
    types::utils::option_to_string
};

use crate::types::url::UrlString;

#[derive(Clone)]
pub struct AppSetupRepository {
    pool: PgPool,
}

impl AppSetupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new restaurant
    pub async fn create(&self, request: CreateAppSetup) -> Result<AppSetup> {
        let setup_row = sqlx::query_as!(
            AppSetupRow,
            r#"
            INSERT INTO app_settings (id,title,image_url)
            VALUES (1, $1, $2)
            RETURNING
                id,
                title,
                image_url as "image_url?: UrlString",
                created_at,
                updated_at
            "#,
            request.title,
            option_to_string(request.image_url)
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(setup_row)
    }

}
