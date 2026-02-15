use anyhow::Result;
use sqlx::PgPool;

use crate::{
    features::app_setup::{
        domain::{AppSetup, CreateAppSetup, UpdateAppSetup},
        db_model::AppSetupRow,
    },
    types::url::UrlString,
};

#[derive(Clone)]
pub struct AppSetupRepository {
    pool: PgPool,
}

impl AppSetupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create app settings (initial setup)
    pub async fn create(&self, request: CreateAppSetup) -> Result<AppSetup> {
        let setup = sqlx::query_as!(
            AppSetupRow,
            r#"
            INSERT INTO app_settings (id, title, image_url, access_hash)
            VALUES (1, $1, $2, $3)
            RETURNING
                id,
                title,
                image_url as "image_url?: UrlString",
                max_menu_nesting_depth,
                access_hash,
                created_at,
                updated_at
            "#,
            request.title,
            request.image_url.as_ref().map(|u| u.to_string()),
            request.access_hash
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(setup)
    }

    /// Get app settings
    pub async fn get(&self) -> Result<Option<AppSetup>> {
        let setup = sqlx::query_as!(
            AppSetupRow,
            r#"
            SELECT
                id,
                title,
                image_url as "image_url?: UrlString",
                max_menu_nesting_depth,
                access_hash,
                created_at,
                updated_at
            FROM app_settings
            WHERE id = 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(setup)
    }

    /// Update app settings
    pub async fn update(&self, request: UpdateAppSetup) -> Result<Option<AppSetup>> {
        let setup = sqlx::query_as!(
            AppSetupRow,
            r#"
            UPDATE app_settings
            SET title = COALESCE($1, title),
                image_url = COALESCE($2, image_url),
                max_menu_nesting_depth = COALESCE($3, max_menu_nesting_depth),
                access_hash = COALESCE($4, access_hash),
                updated_at = NOW()
            WHERE id = 1
            RETURNING
                id,
                title,
                image_url as "image_url?: UrlString",
                max_menu_nesting_depth,
                access_hash,
                created_at,
                updated_at
            "#,
            request.title,
            request.image_url.as_ref().map(|u| u.to_string()),
            request.max_menu_nesting_depth,
            request.access_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(setup)
    }

    /// Get the max menu nesting depth setting
    pub async fn get_max_menu_nesting_depth(&self) -> Result<i16> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT max_menu_nesting_depth
            FROM app_settings
            WHERE id = 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        // Default to 2 if not found
        Ok(result.unwrap_or(2))
    }

    /// Get the access hash for site-access verification.
    ///
    /// This is the SHA-256 hex hash of the shared site access code.
    /// Returns `None` if app settings have not been configured yet.
    pub async fn get_access_hash(&self) -> Result<Option<String>> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT access_hash
            FROM app_settings
            WHERE id = 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }
}
