use crate::validated_enum;

validated_enum!(
    /// The role of a user stored in the database.
    ///
    /// - `User` — regular user who has site access (entered the shared password).
    /// - `Admin` — full control: manage restaurants, menus, users, and promote others.
    ///
    /// Note: "Viewer" is NOT a stored role. It is the implicit access level for
    /// anyone who has NOT entered the site access password. Viewers can only see
    /// statistics and have no database record of their role.
    pub UserRole {
        /// Regular authenticated user, can browse menus, place orders, etc.
        User,
        /// Full control: manage restaurants, menus, users, and promote others.
        Admin,
    }
);