use anyhow::Result;
use email_address::EmailAddress;
use uuid::Uuid;

use crate::{
    features::user::{
        domain::User,
        dto::{CreateUserRequest, UpdateUserRequest},
        repository::UserRepository,
    },
    types::auth::AuthMethod,
};

#[derive(Clone)]
pub struct UserService {
    repository: UserRepository,
}

impl UserService {
    pub fn new(repository: UserRepository) -> Self {
        Self { repository }
    }

    /// Create a new guest (NameWithCookie) user.
    ///
    /// This is the public-facing user creation — it always creates a
    /// NameWithCookie user with no password and no role.
    /// For Password user creation, use `AuthService::register_password_user`.
    pub async fn create_user(&self, request: CreateUserRequest) -> Result<User> {
        // Enforce NameWithCookie for this endpoint
        if request.auth_method != AuthMethod::NameWithCookie {
            anyhow::bail!(
                "Only NameWithCookie users can be created through this endpoint. \
                 Use the admin registration endpoint for Password users."
            );
        }

        // Check if email is already taken (if provided)
        if let Some(email) = &request.email {
            if self.repository.get_by_email(email).await?.is_some() {
                anyhow::bail!("A user with this email already exists");
            }
        }

        // No password hash for guest users
        self.repository.create(request, None).await
    }

    /// Get a user by ID
    pub async fn get_user(&self, id: Uuid) -> Result<Option<User>> {
        self.repository.get_by_id(id).await
    }

    /// Get a user by email
    pub async fn get_user_by_email(&self, email: &EmailAddress) -> Result<Option<User>> {
        self.repository.get_by_email(email).await
    }

    /// Get all users
    pub async fn list_users(&self) -> Result<Vec<User>> {
        self.repository.get_all().await
    }

    /// Update a user's name and/or email.
    ///
    /// Does NOT update auth_method, password_hash, or role — those are
    /// changed through dedicated admin endpoints in AuthService.
    pub async fn update_user(&self, request: UpdateUserRequest) -> Result<Option<User>> {
        if self.repository.get_by_id(request.id).await?.is_none() {
            return Ok(None);
        }

        // Check if email is already taken by another user (if provided)
        if let Some(email) = &request.email {
            if let Some(existing_user) = self.repository.get_by_email(email).await? {
                if existing_user.id != request.id {
                    anyhow::bail!("A user with this email already exists");
                }
            }
        }

        self.repository.update(request).await
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<User>> {
        self.repository.get_by_name(name).await
    }

    /// Delete a user
    pub async fn delete_user(&self, id: Uuid) -> Result<bool> {
        self.repository.delete(id).await
    }
}