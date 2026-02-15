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
        auth::AuthMethod,
        role::UserRole,
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

    /// Create a new user.
    ///
    /// The `password_hash` parameter is provided separately because it is
    /// computed by the service layer (not sent directly in the API request).
    pub async fn create(
        &self,
        create_user: CreateUser,
        password_hash: Option<String>,
    ) -> Result<User> {
        let user = sqlx::query_as!(
            UserRow,
            r#"
            INSERT INTO users (name, email, auth_method, password_hash, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                password_hash,
                role as "role?: UserRole",
                created_at,
                updated_at
            "#,
            create_user.name.as_ref(),
            create_user.email.as_ref().map(|e| e.to_string()),
            create_user.auth_method.to_string(),
            password_hash,
            create_user.role.as_ref().map(|r| r.to_string())
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
                password_hash,
                role as "role?: UserRole",
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
                password_hash,
                role as "role?: UserRole",
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
                password_hash,
                role as "role?: UserRole",
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

    pub async fn get_all(&self) -> Result<Vec<User>> {
        let users = sqlx::query_as!(
            UserRow,
            r#"
            SELECT
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                password_hash,
                role as "role?: UserRole",
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

    /// General-purpose update (name, email).
    ///
    /// Does NOT update auth_method, password_hash, or role — those are
    /// changed through dedicated admin endpoints.
    pub async fn update(&self, update_user: UpdateUser) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            UserRow,
            r#"
            UPDATE users
            SET name = COALESCE($1, name),
                email = COALESCE($2, email),
                updated_at = NOW()
            WHERE id = $3
            RETURNING
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                password_hash,
                role as "role?: UserRole",
                created_at,
                updated_at
            "#,
            update_user.name.as_ref().map(|n| n.as_ref()),
            update_user.email.as_ref().map(|e| e.to_string()),
            update_user.id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Upgrade a NameWithCookie user to a Password user.
    ///
    /// Sets auth_method, password_hash, email, and role in a single UPDATE.
    pub async fn upgrade_to_password(
        &self,
        user_id: Uuid,
        email: Email,
        password_hash: String,
        role: UserRole,
    ) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            UserRow,
            r#"
            UPDATE users
            SET auth_method = $1,
                email = $2,
                password_hash = $3,
                role = $4,
                updated_at = NOW()
            WHERE id = $5
              AND auth_method = 'NameWithCookie'
            RETURNING
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                password_hash,
                role as "role?: UserRole",
                created_at,
                updated_at
            "#,
            AuthMethod::Password.to_string(),
            email.to_string(),
            password_hash,
            role.to_string(),
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Change a user's role.
    pub async fn update_role(
        &self,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            UserRow,
            r#"
            UPDATE users
            SET role = $1,
                updated_at = NOW()
            WHERE id = $2
              AND auth_method = 'Password'
            RETURNING
                id,
                name as "name: Name",
                email as "email?: Email",
                auth_method as "auth_method: AuthMethod",
                password_hash,
                role as "role?: UserRole",
                created_at,
                updated_at
            "#,
            role.to_string(),
            user_id
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