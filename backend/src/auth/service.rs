use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::{
    auth::{
        password::{hash_password, verify_password},
        session::SessionRepository,
    },
    features::user::{
        domain::{CreateUser, User},
        repository::UserRepository,
    },
    types::{
        auth::AuthMethod,
        email::Email,
        name::Name,
        role::UserRole,
    },
};

/// Minimum password length enforced during registration/upgrade.
const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Clone)]
pub struct AuthService {
    user_repository: UserRepository,
    session_repository: SessionRepository,
}

/// Response returned after successful authentication (login or guest creation).
/// Contains the user and the session token to send to the client.
pub struct AuthResponse {
    pub user: User,
    pub token: String,
}

impl AuthService {
    pub fn new(
        user_repository: UserRepository,
        session_repository: SessionRepository,
    ) -> Self {
        Self {
            user_repository,
            session_repository,
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Public (unauthenticated) operations
    // ═══════════════════════════════════════════════════════════════

    /// Create a new guest (NameWithCookie) user and a session.
    ///
    /// The guest only needs to provide a name. They get a long-lived session
    /// cookie so they can be remembered across visits.
    pub async fn create_guest(&self, name: Name) -> Result<AuthResponse> {
        let create = CreateUser {
            name,
            email: None,
            auth_method: AuthMethod::NameWithCookie,
            role: Some(UserRole::User),
        };

        let user = self
            .user_repository
            .create(create, None)
            .await?;

        let session = self
            .session_repository
            .create_guest_session(user.id)
            .await?;

        Ok(AuthResponse {
            user,
            token: session.token,
        })
    }

    /// Authenticate a password user with email and password.
    ///
    /// Returns the user and a new session token on success.
    pub async fn login(&self, email: &str, password: &str) -> Result<AuthResponse> {
        // Parse the email to validate format
        let parsed_email = crate::types::email::parse_email(email.to_string())
            .map_err(|_| anyhow!("Invalid email format"))?;

        // Look up user by email
        let user = self
            .user_repository
            .get_by_email(&parsed_email)
            .await?
            .ok_or_else(|| anyhow!("Invalid email or password"))?;

        // Must be a Password user
        if user.auth_method != AuthMethod::Password {
            return Err(anyhow!("Invalid email or password"));
        }

        // Verify the password against stored hash
        let password_hash = user
            .password_hash
            .as_deref()
            .ok_or_else(|| anyhow!("Account has no password set"))?;

        if !verify_password(password, password_hash)? {
            return Err(anyhow!("Invalid email or password"));
        }

        // Create a session
        let session = self
            .session_repository
            .create_password_session(user.id)
            .await?;

        Ok(AuthResponse {
            user,
            token: session.token,
        })
    }

    /// Destroy a session (logout).
    pub async fn logout(&self, token: &str) -> Result<()> {
        self.session_repository.delete_by_token(token).await?;
        Ok(())
    }

    /// Get the current user from a session token.
    ///
    /// Returns `None` if the token is invalid or expired.
    pub async fn get_current_user(&self, token: &str) -> Result<Option<User>> {
        let session = match self.session_repository.get_by_token(token).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        self.user_repository.get_by_id(session.user_id).await
    }

    // ═══════════════════════════════════════════════════════════════
    // Admin-only operations
    // ═══════════════════════════════════════════════════════════════
    //
    // These methods do NOT check permissions — that's done by the
    // `AdminUser` extractor in the route handler. The service layer
    // only enforces business rules.

    /// Register a new password-authenticated user (admin action).
    ///
    /// The admin provides the name, email, plaintext password, and role.
    /// The password is hashed before storage.
    pub async fn register_password_user(
        &self,
        name: Name,
        email: Email,
        password: &str,
        role: UserRole,
    ) -> Result<User> {
        validate_password(password)?;

        // Check email uniqueness
        if self
            .user_repository
            .get_by_email(&email)
            .await?
            .is_some()
        {
            return Err(anyhow!("A user with this email already exists"));
        }

        let password_hash = hash_password(password)?;

        let create = CreateUser {
            name,
            email: Some(email),
            auth_method: AuthMethod::Password,
            role: Some(role),
        };

        self.user_repository
            .create(create, Some(password_hash))
            .await
    }

    /// Upgrade a NameWithCookie (guest) user to a Password user (admin action).
    ///
    /// Sets the email, password, and role. The user's ID is preserved so
    /// their order history and other data remains linked.
    pub async fn upgrade_to_password(
        &self,
        user_id: Uuid,
        email: Email,
        password: &str,
        role: UserRole,
    ) -> Result<User> {
        validate_password(password)?;

        // Verify user exists and is currently a guest
        let existing = self
            .user_repository
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        if existing.auth_method != AuthMethod::NameWithCookie {
            return Err(anyhow!(
                "Only NameWithCookie users can be upgraded to Password"
            ));
        }

        // Check email uniqueness
        if let Some(other) = self.user_repository.get_by_email(&email).await? {
            if other.id != user_id {
                return Err(anyhow!("A user with this email already exists"));
            }
        }

        let password_hash = hash_password(password)?;

        // Update the user
        let user = self
            .user_repository
            .upgrade_to_password(user_id, email, password_hash, role)
            .await?
            .ok_or_else(|| anyhow!("User not found during upgrade"))?;

        // Invalidate all existing sessions (force re-login with password)
        self.session_repository.delete_by_user_id(user_id).await?;

        Ok(user)
    }

    /// Change a user's role (admin action).
    ///
    /// Only password-authenticated users can have roles.
    pub async fn change_role(&self, user_id: Uuid, role: UserRole) -> Result<User> {
        let existing = self
            .user_repository
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        if existing.auth_method != AuthMethod::Password {
            return Err(anyhow!(
                "Only Password users can have roles assigned"
            ));
        }

        self.user_repository
            .update_role(user_id, role)
            .await?
            .ok_or_else(|| anyhow!("User not found during role change"))
    }

    // ═══════════════════════════════════════════════════════════════
    // Setup (bootstrap first admin)
    // ═══════════════════════════════════════════════════════════════

    /// Create the first admin user during initial app setup.
    ///
    /// This bypasses normal admin-only checks since no admin exists yet.
    pub async fn create_first_admin(
        &self,
        name: Name,
        email: Email,
        password: &str,
    ) -> Result<User> {
        validate_password(password)?;

        // Check email uniqueness
        if self
            .user_repository
            .get_by_email(&email)
            .await?
            .is_some()
        {
            return Err(anyhow!("A user with this email already exists"));
        }

        let password_hash = hash_password(password)?;

        let create = CreateUser {
            name,
            email: Some(email),
            auth_method: AuthMethod::Password,
            role: Some(UserRole::Admin),
        };

        self.user_repository
            .create(create, Some(password_hash))
            .await
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

fn validate_password(password: &str) -> Result<()> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(anyhow!(
            "Password must be at least {} characters",
            MIN_PASSWORD_LENGTH
        ));
    }
    Ok(())
}