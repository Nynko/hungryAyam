use crate::validated_enum;

validated_enum!(
    /// The role of a user stored in the database.
    ///
    /// - `User` — regular user who has site access (entered the shared password).
    /// - `Editor` — can manage restaurants, menus, offers, and sessions.
    /// - `Admin` — full control: everything an Editor can do, plus manage users,
    ///   app settings, and promote others.
    ///
    /// Note: "Viewer" is NOT a stored role. It is the implicit access level for
    /// anyone who has NOT entered the site access password. Viewers can only see
    /// statistics and have no database record of their role.
    pub UserRole {
        /// Regular authenticated user, can browse menus, place orders, etc.
        User,
        /// Can manage restaurants, menus, offers, sessions — but not users or app settings.
        Editor,
        /// Full control: manage restaurants, menus, users, and promote others.
        Admin,
    }
);

impl UserRole {
    /// Returns `true` if this role has at least editor-level privileges
    /// (i.e. `Editor` or `Admin`).
    pub fn is_editor_or_above(&self) -> bool {
        matches!(self, UserRole::Editor | UserRole::Admin)
    }

    /// Returns `true` if this role has admin-level privileges.
    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
}