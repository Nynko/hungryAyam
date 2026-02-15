use crate::validated_enum;

validated_enum!(
    /// How a user authenticates.
    pub AuthMethod {
        NameWithCookie,
        Password,
    }
);
