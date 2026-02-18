use anyhow::Result;
use sqlx::PgPool;

use crate::{
    features::app_setup::{
        domain::{AppSetup, CreateAppSetup, UpdateAppSetup},
        db_model::AppSetupRow,
    },
    types::password::HashedPassword
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
            INSERT INTO app_settings (id, access_hash)
            VALUES (1, $1)
            RETURNING
                id,
                max_menu_nesting_depth,
                access_hash as "access_hash: HashedPassword",
                created_at,
                updated_at
            "#,
            request.access_hash.as_ref()
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
                max_menu_nesting_depth,
                access_hash as "access_hash: HashedPassword",
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
            SET max_menu_nesting_depth = COALESCE($1, max_menu_nesting_depth),
                access_hash = COALESCE($2, access_hash),
                updated_at = NOW()
            WHERE id = 1
            RETURNING
                id,
                max_menu_nesting_depth,
                access_hash as "access_hash: HashedPassword",
                created_at,
                updated_at
            "#,
            request.max_menu_nesting_depth,
            request.access_hash.as_ref().map(|h| h.as_ref())
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
    pub async fn get_access_hash(&self) -> Result<Option<HashedPassword>> {
        let result: Option<HashedPassword> = sqlx::query_scalar!(
            r#"
            SELECT access_hash as "access_hash: HashedPassword"
            FROM app_settings
            WHERE id = 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }
}
