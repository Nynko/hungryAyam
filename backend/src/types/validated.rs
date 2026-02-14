//! Validated types and enum macros.
//!
//! This module provides:
//! - `validated_type!` macro for creating newtype wrappers that validate/convert
//!   on deserialization using a custom function.
//! - `validated_enum!` macro for creating string-backed enums that store as TEXT
//!   in Postgres and integrate with Serde, SQLx, and ts-rs.
//!
//! Both macros integrate with:
//! - **Serde**: Validates during JSON deserialization
//! - **SQLx**: Validates when decoding from database rows
//! - **ts-rs**: Exports to TypeScript
//!
//! # Syntax
//!
//! ```rust,ignore
//! // Simple case: inner type = external type (e.g., String, i32)
//! // Inner type must implement sqlx::Encode, sqlx::Type, serde::Serialize
//! validated_type!(
//!     pub TypeName(InnerType) => ExternalType, parse_fn
//! );
//!
//! // Complex case: inner type ≠ external type (e.g., url::Url stored as String)
//! // Provide a to_external function to convert inner → external for encoding
//! validated_type!(
//!     pub TypeName(InnerType) => ExternalType, parse_fn, to_external_fn
//! );
//! ```
//!
//! # Example - Simple inner type (String, i32, etc.)
//!
//! When the inner type is the same as the external type, no `to_external` is needed:
//!
//! ```rust,ignore
//! use crate::validated_type;
//!
//! fn parse_name(value: String) -> anyhow::Result<Name> {
//!     if value.trim().is_empty() {
//!         anyhow::bail!("Name cannot be empty");
//!     }
//!     Ok(Name(value))
//! }
//!
//! validated_type!(
//!     /// A validated name (non-empty).
//!     pub Name(String) => String, parse_name
//! );
//! ```
//!
//! # Example - Complex inner type (Url, EmailAddress, etc.)
//!
//! When the inner type differs from the external type, provide `to_external`:
//!
//! ```rust,ignore
//! use crate::validated_type;
//!
//! fn parse_url(value: String) -> anyhow::Result<UrlString> {
//!     let url = url::Url::parse(&value)?;
//!     Ok(UrlString(url))
//! }
//!
//! fn url_to_string(url: &url::Url) -> String {
//!     url.to_string()
//! }
//!
//! validated_type!(
//!     /// A validated URL.
//!     pub UrlString(url::Url) => String, parse_url, url_to_string
//! );
//! ```
//!
//! # How It Works
//!
//! The macro generates different code paths based on whether `to_external` is provided:
//!
//! | Operation | Without `to_external` | With `to_external` |
//! |-----------|----------------------|-------------------|
//! | **Decode** (DB → Rust) | Decode as `$external`, call `$parse` | Same |
//! | **Deserialize** (JSON → Rust) | Deserialize as `$external`, call `$parse` | Same |
//! | **Encode** (Rust → DB) | Encode `$inner` directly | Call `$to_external(&inner)`, encode result |
//! | **Serialize** (Rust → JSON) | Serialize `$inner` directly | Call `$to_external(&inner)`, serialize result |
//! | **Display** | Display `$inner` | Display `$inner` (assumes inner: Display) |
//! | **SQLx Type** | Use `$inner`'s type info | Use `$external`'s type info |
//!
//! # Using with SQLx `query_as!` Macro
//!
//! When using the compile-time checked `sqlx::query_as!` macro, SQLx doesn't
//! automatically know about your custom validated types. You must use **type
//! overrides** in your SQL column aliases to tell SQLx which type to use.
//!
//! ## Type Override Syntax
//!
//! ```sql
//! SELECT
//!     column as "column: Type",      -- Non-nullable, use Type
//!     column as "column?: Type",     -- Nullable, returns Option<Type>
//!     column as "column!: Type"      -- Force non-null (override inference)
//! FROM table
//! ```
//!
//! | Syntax | When to Use | Rust Result Type |
//! |--------|-------------|------------------|
//! | `"col: Type"` | Column is `NOT NULL` | `Type` |
//! | `"col?: Type"` | Column is nullable | `Option<Type>` |
//! | `"col!: Type"` | Override SQLx's nullability inference | `Type` |
//!
//! ## Example Repository
//!
//! ```rust,ignore
//! use crate::features::restaurant::domain::{Name, Restaurant};
//! use crate::types::url::UrlString;
//!
//! // In your domain:
//! // - `name` is NOT NULL in the DB
//! // - `image_url` is nullable in the DB
//!
//! pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Restaurant>> {
//!     let restaurant = sqlx::query_as!(
//!         Restaurant,
//!         r#"
//!         SELECT
//!             id,
//!             name as "name: Name",                  -- NOT NULL → Name
//!             image_url as "image_url?: UrlString", -- NULL → Option<UrlString>
//!             created_at,
//!             created_by,
//!             updated_at
//!         FROM restaurants
//!         WHERE id = $1
//!         "#,
//!         id
//!     )
//!     .fetch_optional(&self.pool)
//!     .await?;
//!
//!     Ok(restaurant)
//! }
//! ```
//!
//! ## Why the `?` Matters for Nullable Columns
//!
//! Without `?`, SQLx sees "nullable column" + "custom type" and generates
//! `Option<YourType>`, then tries to convert that, causing type mismatches.
//!
//! With `?`, you explicitly tell SQLx: "This column is nullable. Decode the
//! inner value using `UrlString::decode()`, then wrap the result in `Option`."
//!
//! ```text
//! DB NULL           → None
//! DB "https://..."  → UrlString::decode() → Some(UrlString(Url {...}))
//! ```
//!
//! ## Alternative: `query_as` (Runtime, No Type Overrides)
//!
//! If you prefer not to write type overrides, use `sqlx::query_as` (no `!`)
//! with `#[derive(sqlx::FromRow)]` on your struct. This checks at runtime
//! instead of compile time, but uses the `Decode` implementations directly.
//!
//! ```rust,ignore
//! #[derive(sqlx::FromRow)]
//! pub struct Restaurant {
//!     pub id: Uuid,
//!     #[sqlx(try_from = "String")]  // Tell FromRow to use TryFrom
//!     pub name: Name,
//!     pub image_url: Option<UrlString>,  // Decode impl handles this
//!     // ...
//! }
//!
//! // No type overrides needed, but no compile-time SQL checking
//! let restaurant: Restaurant = sqlx::query_as(
//!     "SELECT id, name, image_url, ... FROM restaurants WHERE id = $1"
//! )
//! .bind(id)
//! .fetch_one(&pool)
//! .await?;
//! ```
//!
//! # Implemented Traits
//!
//! The macro implements the following traits for the generated type:
//!
//! - `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
//! - `Deref<Target = InnerType>` - Access inner value via `*my_value`
//! - `AsRef<InnerType>` - Borrow inner value
//! - `From<MyType> for InnerType` - Convert back to inner type
//! - `TryFrom<ExternalType>` - Fallible conversion (for SQLx FromRow)
//! - `Display` - Format as string (delegates to inner type)
//! - `Serialize` / `Deserialize` - Serde support with validation
//! - `TS` - TypeScript type generation
//! - `sqlx::Type`, `Encode`, `Decode` - Database support

/// Creates a validated newtype wrapper.
///
/// # Two Forms
///
/// ## Simple (inner = external, inner implements Encode/Serialize)
/// ```rust,ignore
/// validated_type!(pub Name(String) => String, parse_name);
/// ```
///
/// ## Complex (inner ≠ external, need conversion for encoding)
/// ```rust,ignore
/// validated_type!(pub UrlString(url::Url) => String, parse_url, url_to_string);
/// ```
///
/// The `to_external` function signature: `fn(&InnerType) -> ExternalType`
#[macro_export]
macro_rules! validated_type {
    // =========================================================================
    // ARM 1: Simple case - inner type = external type (or inner implements SQLx/Serde traits)
    // No to_external function needed
    // =========================================================================
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident($inner:ty) => $external:ty, $parse:path
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $vis struct $name(pub $inner);

        // === Core Methods ===
        impl $name {
            /// Returns a reference to the inner value.
            pub fn inner(&self) -> &$inner {
                &self.0
            }

            /// Consumes self and returns the inner value.
            pub fn into_inner(self) -> $inner {
                self.0
            }
        }

        // === Deref & AsRef (transparent access to inner) ===
        impl ::std::ops::Deref for $name {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::convert::AsRef<$inner> for $name {
            fn as_ref(&self) -> &$inner {
                &self.0
            }
        }

        // === From: Convert back to inner type ===
        impl ::std::convert::From<$name> for $inner {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        // === TryFrom: Fallible conversion from external (for SQLx FromRow) ===
        impl ::std::convert::TryFrom<$external> for $name {
            type Error = ::anyhow::Error;

            fn try_from(value: $external) -> Result<Self, Self::Error> {
                $parse(value)
            }
        }

        // === Display: Delegates to inner type ===
        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        // === Serde Serialize: Delegates to inner type ===
        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        // === Serde Deserialize: Deserialize as external, then validate ===
        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let value = <$external>::deserialize(deserializer)?;
                $parse(value).map_err(::serde::de::Error::custom)
            }
        }

        // === ts-rs: TypeScript type generation ===
        impl ::ts_rs::TS for $name {
            const EXPORT_TO: Option<&'static str> = None;

            fn decl() -> String {
                String::new()
            }

            fn name() -> String {
                <$external as ::ts_rs::TS>::name()
            }

            fn inline() -> String {
                <$external as ::ts_rs::TS>::inline()
            }

            fn dependencies() -> Vec<::ts_rs::Dependency> {
                <$external as ::ts_rs::TS>::dependencies()
            }

            fn transparent() -> bool {
                true
            }
        }

        // === SQLx Type: Use inner type's Postgres type info ===
        impl ::sqlx::Type<::sqlx::Postgres> for $name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <$inner as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &::sqlx::postgres::PgTypeInfo) -> bool {
                <$inner as ::sqlx::Type<::sqlx::Postgres>>::compatible(ty)
            }
        }

        // === SQLx Encode: Encode inner type directly ===
        impl ::sqlx::Encode<'_, ::sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer,
            ) -> Result<::sqlx::encode::IsNull, ::sqlx::error::BoxDynError> {
                <$inner as ::sqlx::Encode<'_, ::sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            }
        }

        // === SQLx Decode: Decode as external, then validate ===
        impl ::sqlx::Decode<'_, ::sqlx::Postgres> for $name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'_>,
            ) -> Result<Self, ::sqlx::error::BoxDynError> {
                let external = <$external as ::sqlx::Decode<'_, ::sqlx::Postgres>>::decode(value)?;
                $parse(external).map_err(|e| e.to_string().into())
            }
        }
    };

    // =========================================================================
    // ARM 2: Complex case - inner type ≠ external type
    // Requires to_external function for encoding/serializing
    // =========================================================================
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident($inner:ty) => $external:ty, $parse:path, $to_external:path
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $vis struct $name(pub $inner);

        // === Core Methods ===
        impl $name {
            /// Returns a reference to the inner value.
            pub fn inner(&self) -> &$inner {
                &self.0
            }

            /// Consumes self and returns the inner value.
            pub fn into_inner(self) -> $inner {
                self.0
            }
        }

        // === Deref & AsRef (transparent access to inner) ===
        impl ::std::ops::Deref for $name {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::convert::AsRef<$inner> for $name {
            fn as_ref(&self) -> &$inner {
                &self.0
            }
        }

        // === From: Convert back to inner type ===
        impl ::std::convert::From<$name> for $inner {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        // === TryFrom: Fallible conversion from external (for SQLx FromRow) ===
        impl ::std::convert::TryFrom<$external> for $name {
            type Error = ::anyhow::Error;

            fn try_from(value: $external) -> Result<Self, Self::Error> {
                $parse(value)
            }
        }

        // === Display: Delegates to inner type ===
        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }

        // === Serde Serialize: Convert to external first, then serialize ===
        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                let external: $external = $to_external(&self.0);
                external.serialize(serializer)
            }
        }

        // === Serde Deserialize: Deserialize as external, then validate ===
        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                let value = <$external>::deserialize(deserializer)?;
                $parse(value).map_err(::serde::de::Error::custom)
            }
        }

        // === ts-rs: TypeScript type generation (uses external type) ===
        impl ::ts_rs::TS for $name {
            const EXPORT_TO: Option<&'static str> = None;

            fn decl() -> String {
                String::new()
            }

            fn name() -> String {
                <$external as ::ts_rs::TS>::name()
            }

            fn inline() -> String {
                <$external as ::ts_rs::TS>::inline()
            }

            fn dependencies() -> Vec<::ts_rs::Dependency> {
                <$external as ::ts_rs::TS>::dependencies()
            }

            fn transparent() -> bool {
                true
            }
        }

        // === SQLx Type: Use external type's Postgres type info ===
        impl ::sqlx::Type<::sqlx::Postgres> for $name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <$external as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &::sqlx::postgres::PgTypeInfo) -> bool {
                <$external as ::sqlx::Type<::sqlx::Postgres>>::compatible(ty)
            }
        }

        // === SQLx Encode: Convert to external first, then encode ===
        impl ::sqlx::Encode<'_, ::sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer,
            ) -> Result<::sqlx::encode::IsNull, ::sqlx::error::BoxDynError> {
                let external: $external = $to_external(&self.0);
                <$external as ::sqlx::Encode<'_, ::sqlx::Postgres>>::encode_by_ref(&external, buf)
            }
        }

        // === SQLx Decode: Decode as external, then validate ===
        impl ::sqlx::Decode<'_, ::sqlx::Postgres> for $name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'_>,
            ) -> Result<Self, ::sqlx::error::BoxDynError> {
                let external = <$external as ::sqlx::Decode<'_, ::sqlx::Postgres>>::decode(value)?;
                $parse(external).map_err(|e| e.to_string().into())
            }
        }
    };
}

// =============================================================================
// validated_enum! macro
// =============================================================================

/// Creates a string-backed enum with full Serde, SQLx (Postgres TEXT), and ts-rs integration.
///
/// The enum uses `EnumStringify` (from the `enum_stringify` crate) to derive
/// `Display` and `FromStr`, which are then used for database encoding/decoding.
///
/// # Syntax
///
/// ```rust,ignore
/// validated_enum!(
///     /// Doc comment for the enum.
///     pub EnumName {
///         Variant1,
///         Variant2,
///         Variant3,
///     }
/// );
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use crate::validated_enum;
///
/// validated_enum!(
///     /// How a user authenticates.
///     pub AuthMethod {
///         NoneWithCookie,
///         Password,
///     }
/// );
/// ```
///
/// # Generated Traits
///
/// The macro derives/implements:
/// - `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
/// - `EnumStringify` → `Display` + `FromStr`
/// - `Serialize` / `Deserialize` (serde)
/// - `TS` (ts-rs, exported)
/// - `sqlx::Type<Postgres>` — maps to TEXT
/// - `sqlx::Encode<Postgres>` — encodes via `Display`
/// - `sqlx::Decode<Postgres>` — decodes via `FromStr`
///
/// # Using with SQLx `query_as!`
///
/// Use type overrides just like with `validated_type!`:
///
/// ```sql
/// SELECT auth_method as "auth_method: AuthMethod" FROM users WHERE id = $1
/// ```
#[macro_export]
macro_rules! validated_enum {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash,
            ::enum_stringify::EnumStringify,
            ::serde::Serialize, ::serde::Deserialize,
            ::ts_rs::TS,
        )]
        #[ts(export)]
        $vis enum $name {
            $( $(#[$variant_meta])* $variant ),+
        }

        // === SQLx Type: stored as TEXT in Postgres ===
        impl ::sqlx::Type<::sqlx::Postgres> for $name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <String as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &::sqlx::postgres::PgTypeInfo) -> bool {
                <String as ::sqlx::Type<::sqlx::Postgres>>::compatible(ty)
            }
        }

        // === SQLx Encode: serialize to string via Display ===
        impl ::sqlx::Encode<'_, ::sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer,
            ) -> Result<::sqlx::encode::IsNull, ::sqlx::error::BoxDynError> {
                let s = self.to_string();
                <String as ::sqlx::Encode<'_, ::sqlx::Postgres>>::encode_by_ref(&s, buf)
            }
        }

        // === SQLx Decode: parse from string via FromStr ===
        impl ::sqlx::Decode<'_, ::sqlx::Postgres> for $name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'_>,
            ) -> Result<Self, ::sqlx::error::BoxDynError> {
                let s = <String as ::sqlx::Decode<'_, ::sqlx::Postgres>>::decode(value)?;
                ::std::str::FromStr::from_str(&s)
                    .map_err(|e: String|
                        format!("Invalid {} '{}': {}", stringify!($name), s, e).into()
                    )
            }
        }
    };
}
