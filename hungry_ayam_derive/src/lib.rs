//! # Hungry Ayam Derive Macros
//!
//! Custom derive macros for domain-driven design patterns.
//!
//! ## Macros
//!
//! - [`domain_struct`] - Attribute macro to generate `CreateX` and `UpdateX` structs
//! - [`IntoDomain`] - Convert DTOs/DB models → Domain objects
//!
//! ## Quick Reference
//!
//! ```text
//! DTO/DB Model ──IntoDomain──▶ Domain ──domain_struct──▶ CreateX/UpdateX
//! ```
//!
//! ### `domain_struct` Attribute
//!
//! Automatically forwards all derives and struct-level attributes to generated structs.
//!
//! ```rust,ignore
//! #[domain_struct(create, update)]
//! #[derive(Debug, Clone, Serialize, Deserialize, TS)]
//! #[ts(export)]
//! pub struct Item {
//!     #[derived_domain_ignore]
//!     pub id: Uuid,
//!     pub name: String,
//!     #[create_ignore]
//!     pub active: bool,
//! }
//! // Generates CreateItem and UpdateItem with same derives and #[ts(export)]
//! ```
//!
//! ### `IntoDomain` Attributes
//!
//! | Attribute | Description |
//! |-----------|-------------|
//! | `#[into_domain(Type)]` | Target domain type (required) |
//! | `#[into_domain_name = "field"]` | Rename field in target |
//! | `#[into_domain_ignored]` | Skip field |
//! | `#[into_domain_with(fn)]` | Use custom function for conversion (fn must return `Result<T, E>`) |
//!
//! ### Field Attributes for `domain_struct`
//!
//! | Attribute | Create | Update |
//! |-----------|--------|--------|
//! | `#[derived_domain_ignore]` | Exclude | Exclude |
//! | `#[create_ignore]` | Exclude | - |
//! | `#[update_ignore]` | - | Exclude |
//! | `#[create_optional]` | Wrap in Option | - |
//! | `#[derived_domain_optional]` | Wrap in Option | (no-op) |
//! | `#[update_required]` | - | Keep as-is (no Option wrap) |
//!
//! **Note:** `UpdateX` structs wrap all non-Option fields in `Option<T>` automatically.
//! Fields already `Option<T>` stay as `Option<T>` (no double-wrapping).

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

use crate::into_domain::impl_into_domain_derive;
use crate::domain_struct::impl_domain_struct;

mod into_domain;
mod domain_struct;

/// Attribute macro to generate `Create{StructName}` and/or `Update{StructName}` structs.
///
/// This macro automatically forwards all derives and struct-level attributes
/// (like `#[ts(export)]`, `#[serde(...)]`) to the generated structs.
///
/// # Arguments
///
/// - `create` - Generate a `Create{StructName}` struct
/// - `update` - Generate an `Update{StructName}` struct
///
/// # Example
///
/// ```rust,ignore
/// #[domain_struct(create, update)]
/// #[derive(Debug, Clone, Serialize, Deserialize, TS)]
/// #[ts(export)]
/// pub struct Item {
///     #[derived_domain_ignore]
///     pub id: Uuid,
///     pub name: String,
///     pub description: Option<String>,
///     #[create_ignore]
///     pub active: bool,
/// }
/// // Generates:
/// // #[derive(Debug, Clone, Serialize, Deserialize, TS)]
/// // #[ts(export)]
/// // pub struct CreateItem { pub name: String, pub description: Option<String> }
/// //
/// // #[derive(Debug, Clone, Serialize, Deserialize, TS)]
/// // #[ts(export)]
/// // pub struct UpdateItem { pub name: Option<String>, pub description: Option<String>, pub active: Option<bool> }
/// ```
#[proc_macro_attribute]
pub fn domain_struct(args: TokenStream, input: TokenStream) -> TokenStream {
    impl_domain_struct(args, input)
}

/// Derive `IntoDomain<T>` trait for converting structs to domain objects.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(IntoDomain)]
/// #[into_domain(User)]
/// pub struct UserRow {
///     pub id: Uuid,
///     #[into_domain_name = "user_email"]
///     #[into_domain_with(parse_email)]  // custom conversion function
///     pub email: Option<String>,
///     #[into_domain_with(parse_name)]
///     pub name: String,
///     #[into_domain_ignored]
///     pub internal: String,
/// }
/// ```
#[proc_macro_derive(IntoDomain, attributes(into_domain, into_domain_name, into_domain_ignored, into_domain_with))]
pub fn into_domain_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_into_domain_derive(input)
}
