//! # Hungry Ayam Derive Macros
//!
//! Custom derive macros for domain-driven design patterns.
//!
//! - [`domain_struct`] — Generate derived structs from a domain struct
//! - [`IntoDomain`] — Convert DTOs/DB models into domain objects
//!
//! ```text
//! DTO/DB Model ──IntoDomain──▶ Domain ──domain_struct──▶ CreateX / UpdateX / {Name}X
//! ```
//!
//! ## `domain_struct`
//!
//! Each variant name produces a `{PascalName}{StructName}` struct.
//! Snake_case names are converted to PascalCase (e.g. `unit_create` → `UnitCreateItem`).
//!
//! ```rust,ignore
//! #[domain_struct(create, update, unit_create)]
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct Item {
//!     #[create_ignore]
//!     #[update_required]
//!     pub id: Uuid,
//!     pub name: String,
//!     #[create_ignore]
//!     pub active: bool,
//!     #[derived_type(Vec<TagInput>)]
//!     pub tags: Vec<Tag>,
//! }
//! // Generates: CreateItem, UpdateItem, UnitCreateItem
//! ```
//!
//! ### Variant modifiers
//!
//! - `name` — fields are required by default
//! - `name(all_optional)` — fields are wrapped in `Option<T>` by default
//! - Bare `update` is treated as `update(all_optional)` for backward compatibility
//!
//! ### Field attributes
//!
//! **Global** (all variants):
//!
//! | Attribute | Effect |
//! |-----------|--------|
//! | `#[derived_domain_ignore]` | Exclude from all variants |
//! | `#[derived_domain_optional]` | Wrap in `Option` in all variants |
//! | `#[derived_type(Type)]` | Override type in all variants |
//! | `#[derived_nested]` | Auto-prefix inner types with the variant name (composition) |
//!
//! **Per-variant** (`{name}` = variant name):
//!
//! | Attribute | Effect |
//! |-----------|--------|
//! | `#[{name}_ignore]` | Exclude from this variant |
//! | `#[{name}_optional]` | Wrap in `Option` |
//! | `#[{name}_required]` | Keep as-is (no `Option` wrap) |
//! | `#[{name}_type(Type)]` | Override type |
//! | `#[{name}_nested]` | Auto-prefix inner types for this variant only |
//!
//! `Option<T>` fields are never double-wrapped.
//!
//! Type resolution priority: `{name}_type` > `derived_type` > `{name}_nested` / `derived_nested` > original.
//! `{name}_ignore` / `derived_domain_ignore` is always checked first.
//!
//! ## `IntoDomain`
//!
//! | Attribute | Description |
//! |-----------|-------------|
//! | `#[into_domain(Type)]` | Target domain type (required) |
//! | `#[into_domain_name = "field"]` | Rename field in target |
//! | `#[into_domain_ignored]` | Skip field |
//! | `#[into_domain_with(fn)]` | Custom conversion (fn must return `Result<T, E>`) |

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

use crate::domain_struct::impl_domain_struct;
use crate::into_domain::impl_into_domain_derive;

mod domain_struct;
mod into_domain;

/// Generate derived variant structs from a domain struct.
///
/// # Example
///
/// ```rust,ignore
/// #[domain_struct(create, update, unit_create)]
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct Item {
///     #[create_ignore]
///     #[update_required]
///     #[unit_create_required]
///     pub id: Uuid,
///     pub name: String,
///     #[derived_type(Vec<TagInput>)]
///     pub tags: Vec<Tag>,
/// }
/// ```
#[proc_macro_attribute]
pub fn domain_struct(args: TokenStream, input: TokenStream) -> TokenStream {
    impl_domain_struct(args, input)
}

/// Derive `IntoDomain<T>` for converting structs to domain objects.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(IntoDomain)]
/// #[into_domain(User)]
/// pub struct UserRow {
///     pub id: Uuid,
///     #[into_domain_name = "user_email"]
///     #[into_domain_with(parse_email)]
///     pub email: Option<String>,
///     #[into_domain_ignored]
///     pub internal: String,
/// }
/// ```
#[proc_macro_derive(IntoDomain, attributes(into_domain, into_domain_name, into_domain_ignored, into_domain_with))]
pub fn into_domain_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_into_domain_derive(input)
}
