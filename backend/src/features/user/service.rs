use anyhow::Result;
use email_address::EmailAddress;
use uuid::Uuid;

use crate::{
    features::user::{
        domain::{CreateUser, User},
        dto::{CreateUserRequest, UpdateUserRequest},
        repository::UserRepository,
    },
};

#[derive(Clone)]
pub struct UserService {
    repository: UserRepository,
}

impl UserService {
    pub fn new(repository: UserRepository) -> Self {
        Self { repository }
    }

    /// Create a new user with business validation
    pub async fn create_user(&self, request: CreateUserRequest) -> Result<User> {
        // Check if email is already taken (if provided)
        if let Some(email) = &request.email {
            if self.repository.get_by_email(email).await?.is_some() {
                anyhow::bail!("A user with this email already exists");
            }
        }

        self.repository.create(request).await
    }

    /// Get a user by ID
    pub async fn get_user(&self, id: Uuid) -> Result<Option<User>> {
        self.repository.get_by_id(id).await
    }

    /// Get a user by email
    pub async fn get_user_by_email(&self, email: &EmailAddress) -> Result<Option<User>> {
        self.repository.get_by_email(email).await
    }

    /// Get a user by cookie (for guest users with NoneWithCookie auth)
    pub async fn get_user_by_cookie(&self, cookie: &str) -> Result<Option<User>> {
        self.repository.get_by_cookie(cookie).await
    }

    /// Get all users
    pub async fn list_users(&self) -> Result<Vec<User>> {
        self.repository.get_all().await
    }

    /// Update a user with business validation
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
