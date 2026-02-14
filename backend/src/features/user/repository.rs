use anyhow::Result;
use email_address::EmailAddress;
use sqlx::PgPool;
use uuid::Uuid;
use crate::{
    features::user::{
        db_model::UserRow,
        domain::{CreateUser, UpdateUser, User},
    },
    types::{
        email::Email,
        name::Name,
        auth::AuthMethod
    }
};

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, create_user: CreateUser) -> Result<User> {
        let user = sqlx::query_as!(
            UserRow,
            r#"
            INSERT INTO users (name, email, auth_method, auth_value)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            "#,
            create_user.name.as_ref(),
            create_user.email.as_ref().map(|e| e.to_string()),
            create_user.auth_method.to_string(),
            create_user.auth_value
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let user_row = sqlx::query_as!(
            UserRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            FROM users WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_row)
    }

    pub async fn get_by_email(&self, email: &EmailAddress) -> Result<Option<User>> {
        let user_row = sqlx::query_as!(
            UserRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            FROM users WHERE email = $1
            "#,
            email.to_string()
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_row)
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<User>> {
        let user_row = sqlx::query_as!(
            UserRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            FROM users
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_row)
    }

    /// Look up a guest user by their cookie value.
    /// Only matches users with auth_method = 'NoneWithCookie'.
    pub async fn get_by_cookie(&self, cookie: &str) -> Result<Option<User>> {
        let user_row = sqlx::query_as!(
            UserRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            FROM users
            WHERE auth_method = 'NoneWithCookie'
              AND auth_value = $1
            "#,
            cookie
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_row)
    }

    pub async fn get_all(&self) -> Result<Vec<User>> {
        let users = sqlx::query_as!(
            UserRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            FROM users
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(users.into_iter().collect())
    }

    pub async fn update(&self, update_user: UpdateUser) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            UserRow,
            r#"
            UPDATE users
            SET name = COALESCE($1, name),
                email = COALESCE($2, email),
                auth_method = COALESCE($3, auth_method),
                auth_value = COALESCE($4, auth_value),
                updated_at = NOW()
            WHERE id = $5
            RETURNING
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                auth_value,
                created_at,
                updated_at
            "#,
            update_user.name.as_ref().map(|n| n.as_ref()),
            update_user.email.as_ref().map(|e| e.to_string()),
            update_user.auth_method.as_ref().map(|a| a.to_string()),
            update_user.auth_value,
            update_user.id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM users WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
