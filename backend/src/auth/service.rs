use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::{
    auth::session::SessionRepository,
    features::user::{
        domain::{CreateUser, User},
        repository::UserRepository,
    },
    types::{
        auth::AuthMethod,
        email::Email,
        name::Name,
        password::ClearPassword,
        role::UserRole,
    },
    utils::password::{
        hash_password,
        verify_password
    },
};

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

    /// Create a new guest (NameWithCookie) user and a session, or log in
    /// as an existing guest with the same name.
    ///
    /// The guest only needs to provide a name. They get a long-lived session
    /// cookie so they can be remembered across visits.
    ///
    /// Returns `(AuthResponse, bool)` where the bool indicates if this was
    /// an existing user (`true`) or a newly created one (`false`).
    pub async fn create_guest(&self, name: Name) -> Result<(AuthResponse, bool)> {
        // Check if a user with this name already exists
        if let Some(existing) = self.user_repository.get_by_name(name.as_ref()).await? {
            // If they're a guest (NameWithCookie), reconnect them
            if existing.auth_method == AuthMethod::NameWithCookie {
                let session = self
                    .session_repository
                    .create_guest_session(existing.id)
                    .await?;

                return Ok((
                    AuthResponse {
                        user: existing,
                        token: session.token,
                    },
                    true, // existing user
                ));
            }

            // If they're a password user, they need to log in with their password
            if existing.auth_method == AuthMethod::Password {
                return Err(anyhow!(
                    "A user with this name already exists. Please log in with your password."
                ));
            }
        }

        // Create new guest user
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

        Ok((
            AuthResponse {
                user,
                token: session.token,
            },
            false, // new user
        ))
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
    // Self-service operations (authenticated user)
    // ═══════════════════════════════════════════════════════════════

    /// Upgrade the current guest to a password account.
    ///
    /// Role is always set to `User`. Sessions are invalidated so the
    /// caller must re-authenticate with the new credentials.
    pub async fn self_upgrade_to_password(
        &self,
        user_id: Uuid,
        email: Email,
        password: &ClearPassword,
    ) -> Result<User> {
        self.upgrade_to_password(user_id, email, password, UserRole::User)
            .await
    }

    /// Toggle the current user between `User` and `Editor` roles.
    ///
    /// Only password users whose email domain matches `editor_email_domain`
    /// are eligible. Admins cannot toggle (they stay Admin).
    /// If `editor_email_domain` is `"*"`, any email domain is accepted.
    pub async fn toggle_editor(
        &self,
        user_id: Uuid,
        editor_email_domain: &str,
    ) -> Result<User> {
        let user = self
            .user_repository
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        if user.auth_method != AuthMethod::Password {
            return Err(anyhow!("Only registered users can toggle editor role"));
        }

        let email = user
            .email
            .as_ref()
            .ok_or_else(|| anyhow!("User has no email"))?;

        // Compare domains (case-insensitive). "*" acts as a wildcard (any domain allowed).
        if editor_email_domain != "*" {
            let user_domain = email.as_ref().domain();
            if !user_domain.eq_ignore_ascii_case(editor_email_domain) {
                return Err(anyhow!("Your email domain is not eligible for editor access"));
            }
        }

        let new_role = match user.role.as_ref() {
            Some(UserRole::User) => UserRole::Editor,
            Some(UserRole::Editor) => UserRole::User,
            Some(UserRole::Admin) => return Err(anyhow!("Admin users cannot toggle editor role")),
            None => return Err(anyhow!("User has no role")),
        };

        self.user_repository
            .update_role(user_id, new_role)
            .await?
            .ok_or_else(|| anyhow!("User not found during role toggle"))
    }

    /// Change the current user's display name.
    ///
    /// Returns an error if the name is already taken by another user.
    pub async fn change_name(&self, user_id: Uuid, new_name: Name) -> Result<User> {
        // Check name availability
        if let Some(existing) = self.user_repository.get_by_name(new_name.as_ref()).await? {
            if existing.id != user_id {
                return Err(anyhow!("A user with this name already exists"));
            }
        }

        let update = crate::features::user::domain::UpdateUser {
            id: user_id,
            name: Some(new_name),
            email: None,
            auth_method: None,
            role: None,
        };

        self.user_repository
            .update(update)
            .await?
            .ok_or_else(|| anyhow!("User not found"))
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
    /// The admin provides the name, email, validated password, and role.
    /// The password is hashed before storage.
    pub async fn register_password_user(
        &self,
        name: Name,
        email: Email,
        password: &ClearPassword,
        role: UserRole,
    ) -> Result<User> {

        // Check email uniqueness
        if self
            .user_repository
            .get_by_email(&email)
            .await?
            .is_some()
        {
            return Err(anyhow!("A user with this email already exists"));
        }

        let password_hash = hash_password(password.as_ref())?;

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
        password: &ClearPassword,
        role: UserRole,
    ) -> Result<User> {

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

        let password_hash = hash_password(password.as_ref())?;

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
        password: &ClearPassword,
    ) -> Result<User> {

        // Check email uniqueness
        if self
            .user_repository
            .get_by_email(&email)
            .await?
            .is_some()
        {
            return Err(anyhow!("A user with this email already exists"));
        }

        let password_hash = hash_password(password.as_ref())?;

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
