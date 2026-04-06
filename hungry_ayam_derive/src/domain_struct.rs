//! `domain_struct` attribute macro implementation.
//!
//! Generates derived structs from a domain struct. Each variant name produces
//! a `{PascalName}{StructName}` (snake_case is converted, e.g. `unit_create` → `UnitCreate`).
//!
//! Fields are required by default. Use `name(all_optional)` to wrap all fields in `Option`.
//! Bare `update` is treated as `update(all_optional)` for backward compatibility.
//!
//! Per-variant field attributes: `{name}_ignore`, `{name}_optional`, `{name}_required`, `{name}_type(T)`, `{name}_nested`.
//! Global field attributes: `derived_domain_ignore`, `derived_domain_optional`, `derived_type(T)`, `derived_nested`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, Attribute, Data, DeriveInput, Field, Fields,
    GenericArgument, Meta, PathArguments, Token, Type,
};
use syn::parse::{Parse, ParseStream};

/// Configuration for a single variant to generate
#[derive(Clone)]
pub struct VariantConfig {
    /// The variant name as written by the user (e.g. "create", "update", "unit")
    pub name: String,
    /// Whether fields default to optional (like old `update` behavior)
    pub all_optional: bool,
}

impl VariantConfig {
    /// Get the PascalCase prefix for the generated struct name.
    /// Converts snake_case to PascalCase (e.g. "unit_create" → "UnitCreate").
    fn prefix(&self) -> String {
        self.name
            .split('_')
            .map(|segment| {
                let mut chars = segment.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                }
            })
            .collect()
    }

    /// Returns the helper attribute names for this variant
    fn helper_attr_names(&self) -> Vec<String> {
        vec![
            format!("{}_ignore", self.name),
            format!("{}_optional", self.name),
            format!("{}_required", self.name),
            format!("{}_type", self.name),
            format!("{}_nested", self.name),
        ]
    }
}

/// Parsed arguments for the `domain_struct` attribute
pub struct DomainStructArgs {
    pub variants: Vec<VariantConfig>,
}

/// A single argument entry that can be either `name` or `name(modifier)`
enum ArgEntry {
    Simple(syn::Ident),
    WithModifier(syn::Ident, syn::Ident),
}

impl Parse for ArgEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let modifier: syn::Ident = content.parse()?;
            Ok(ArgEntry::WithModifier(name, modifier))
        } else {
            Ok(ArgEntry::Simple(name))
        }
    }
}

impl Parse for DomainStructArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let entries = Punctuated::<ArgEntry, Token![,]>::parse_terminated(input)?;
        let mut variants = Vec::new();

        for entry in entries {
            match entry {
                ArgEntry::Simple(ident) => {
                    let name = ident.to_string();
                    variants.push(VariantConfig { name, all_optional: false });
                }
                ArgEntry::WithModifier(ident, modifier) => {
                    let name = ident.to_string();
                    let modifier_str = modifier.to_string();
                    let all_optional = match modifier_str.as_str() {
                        "all_optional" => true,
                        "all_required" => false,
                        other => {
                            return Err(syn::Error::new_spanned(
                                modifier,
                                format!(
                                    "unknown modifier '{}', expected 'all_optional' or 'all_required'",
                                    other
                                ),
                            ));
                        }
                    };
                    variants.push(VariantConfig { name, all_optional });
                }
            }
        }

        Ok(DomainStructArgs { variants })
    }
}

/// Global helper attributes that are always recognized
const GLOBAL_HELPER_ATTRS: &[&str] = &[
    "derived_domain_ignore",
    "derived_domain_optional",
    "derived_type",
    "derived_nested",
];

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    return args.args.first().is_some();
                }
            }
        }
    }
    false
}

/// Check if an attribute is a helper attribute that should be stripped
fn is_helper_attr(attr: &Attribute, variants: &[VariantConfig]) -> bool {
    // Check global helper attrs
    if GLOBAL_HELPER_ATTRS
        .iter()
        .any(|name| attr.path().is_ident(name))
    {
        return true;
    }
    // Check per-variant helper attrs
    for variant in variants {
        for attr_name in variant.helper_attr_names() {
            if attr.path().is_ident(&attr_name) {
                return true;
            }
        }
    }
    false
}

/// Extract type from a type-override attribute like `#[create_type(Type)]` or `#[derived_type(Type)]`
fn extract_type_override(field: &Field, attr_name: &str) -> Option<Type> {
    for attr in &field.attrs {
        if attr.path().is_ident(attr_name) {
            if let Meta::List(meta_list) = &attr.meta {
                let tokens = &meta_list.tokens;
                if let Ok(ty) = syn::parse2::<Type>(tokens.clone()) {
                    return Some(ty);
                }
            }
        }
    }
    None
}

/// Check if a field has a specific marker attribute (no arguments)
fn has_attr(field: &Field, attr_name: &str) -> bool {
    field.attrs.iter().any(|attr| attr.path().is_ident(attr_name))
}

/// Walk a type tree and prefix all concrete (non-generic-wrapper) type identifiers.
///
/// For type paths with generic arguments (like `Vec<T>`, `Option<T>`, `Box<T>`, `HashMap<K, V>`),
/// we recurse into the generic arguments. For type paths without generic arguments
/// (like `MenuSection`, `Item`), we prepend the variant prefix to the identifier.
///
/// Examples with prefix "Create":
///   - `MenuSection`                → `CreateMenuSection`
///   - `Vec<MenuSection>`           → `Vec<CreateMenuSection>`
///   - `Option<MenuSection>`        → `Option<CreateMenuSection>`
///   - `Option<Vec<MenuSection>>`   → `Option<Vec<CreateMenuSection>>`
///   - `Box<MenuSection>`           → `Box<CreateMenuSection>`
///   - `HashMap<String, Item>`      → `HashMap<String, CreateItem>` (prefixes both concrete types)
fn prefix_concrete_types(ty: &Type, prefix: &str) -> Type {
    match ty {
        Type::Path(type_path) => {
            let mut new_path = type_path.clone();
            if let Some(last_segment) = new_path.path.segments.last_mut() {
                match &mut last_segment.arguments {
                    PathArguments::AngleBracketed(args) => {
                        // Generic type like Vec<T>, Option<T>, HashMap<K,V> etc.
                        // Recurse into each type argument
                        for arg in args.args.iter_mut() {
                            if let GenericArgument::Type(inner_ty) = arg {
                                *inner_ty = prefix_concrete_types(inner_ty, prefix);
                            }
                        }
                    }
                    PathArguments::None => {
                        // Concrete type with no generics — prefix the identifier.
                        // Skip primitive-looking types (lowercase first char) to avoid
                        // prefixing things like `String`, `bool`, `i32` etc.
                        // Actually, `String` starts uppercase too, but it's a std type.
                        // The user opts in with `derived_nested`, so we trust them.
                        let ident = &last_segment.ident;
                        last_segment.ident = format_ident!("{}{}", prefix, ident);
                    }
                    PathArguments::Parenthesized(_) => {
                        // Fn types — leave as-is
                    }
                }
            }
            Type::Path(new_path)
        }
        // For any other type form (references, tuples, etc.), return as-is
        _ => ty.clone(),
    }
}

/// Extracts derive macros from `#[derive(...)]` attributes
fn extract_derives(attrs: &[Attribute]) -> Vec<proc_macro2::TokenStream> {
    let mut derives = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Meta::List(meta_list) = &attr.meta {
                let tokens = &meta_list.tokens;
                let token_str = tokens.to_string();

                let filtered: Vec<&str> = token_str
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();

                for derive_name in filtered {
                    let ident: proc_macro2::TokenStream = derive_name.parse().unwrap();
                    derives.push(ident);
                }
            }
        }
    }

    derives
}

/// Extracts struct-level attributes to forward (like `#[ts(export)]`, `#[serde(...)]`)
/// Excludes derive attributes (handled separately)
fn extract_struct_attrs(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("derive"))
        .collect()
}

/// Filter out helper attributes from a field's attributes
fn filter_field_attrs<'a>(field: &'a Field, variants: &[VariantConfig]) -> Vec<&'a Attribute> {
    field
        .attrs
        .iter()
        .filter(|attr| !is_helper_attr(attr, variants))
        .collect()
}

fn generate_domain_struct(
    input: &DeriveInput,
    variant: &VariantConfig,
    derives: &[proc_macro2::TokenStream],
    struct_attrs: &[&Attribute],
) -> proc_macro2::TokenStream {
    let struct_name = &input.ident;
    let prefix = variant.prefix();
    let generated_struct_name = format_ident!("{}{}", prefix, struct_name);
    let visibility = &input.vis;

    let fields = if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            &fields_named.named
        } else {
            panic!("domain_struct only supports structs with named fields");
        }
    } else {
        panic!("domain_struct only supports structs");
    };

    let ignore_attr = format!("{}_ignore", variant.name);
    let optional_attr = format!("{}_optional", variant.name);
    let required_attr = format!("{}_required", variant.name);
    let type_attr = format!("{}_type", variant.name);
    let nested_attr = format!("{}_nested", variant.name);

    let mut generated_fields = Vec::new();

    for f in fields.iter() {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        let vis = &f.vis;

        // ── 1. Ignore check (highest priority) ──
        let is_derived_domain_ignore = has_attr(f, "derived_domain_ignore");
        let is_variant_ignore = has_attr(f, &ignore_attr);

        if is_derived_domain_ignore || is_variant_ignore {
            continue;
        }

        // ── 2. Resolve the base type ──
        // Priority: {name}_type > derived_type > {name}_nested / derived_nested > original
        let is_variant_nested = has_attr(f, &nested_attr);
        let is_derived_nested = has_attr(f, "derived_nested");

        let base_ty: Type = if let Some(explicit) = extract_type_override(f, &type_attr) {
            // Per-variant explicit type override — highest priority
            explicit
        } else if let Some(explicit) = extract_type_override(f, "derived_type") {
            // Global explicit type override
            explicit
        } else if is_variant_nested || is_derived_nested {
            // Nested composition: prefix concrete types in the original type
            prefix_concrete_types(ty, &prefix)
        } else {
            // No transformation — use original type
            ty.clone()
        };

        // ── 3. Determine Option wrapping ──
        let is_derived_domain_optional = has_attr(f, "derived_domain_optional");
        let is_variant_optional = has_attr(f, &optional_attr);
        let is_variant_required = has_attr(f, &required_attr);

        let should_wrap_in_option = if variant.all_optional {
            !is_variant_required
        } else {
            is_variant_optional || is_derived_domain_optional
        };

        let generated_field_ty = if should_wrap_in_option && !is_option_type(&base_ty) {
            quote! { Option<#base_ty> }
        } else {
            quote! { #base_ty }
        };

        // ── 4. Collect non-helper attributes to forward ──
        let field_attrs: Vec<&Attribute> = f
            .attrs
            .iter()
            .filter(|attr| !is_helper_attr(attr, &[variant.clone()]))
            // Strip global helper attrs
            .filter(|attr| {
                !GLOBAL_HELPER_ATTRS
                    .iter()
                    .any(|name| attr.path().is_ident(name))
            })
            // Strip other variants' helper attrs (they shouldn't leak)
            .filter(|attr| {
                !attr.path().get_ident().map_or(false, |ident| {
                    let s = ident.to_string();
                    s.ends_with("_ignore")
                        || s.ends_with("_optional")
                        || s.ends_with("_required")
                        || s.ends_with("_type")
                        || s.ends_with("_nested")
                })
            })
            .collect();

        generated_fields.push(quote! {
            #(#field_attrs)*
            #vis #name: #generated_field_ty
        });
    }

    // Build the derive attribute
    let derive_attr = if derives.is_empty() {
        quote! {}
    } else {
        quote! { #[derive(#(#derives),*)] }
    };

    quote! {
        #derive_attr
        #(#struct_attrs)*
        #visibility struct #generated_struct_name {
            #(#generated_fields),*
        }
    }
}

/// Generate the original struct with helper attributes stripped from fields
fn generate_original_struct(
    input: &DeriveInput,
    variants: &[VariantConfig],
) -> proc_macro2::TokenStream {
    let struct_name = &input.ident;
    let visibility = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;

    let fields = if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            &fields_named.named
        } else {
            panic!("domain_struct only supports structs with named fields");
        }
    } else {
        panic!("domain_struct only supports structs");
    };

    // Generate fields with helper attributes stripped
    let cleaned_fields: Vec<_> = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            let vis = &f.vis;
            let filtered_attrs = filter_field_attrs(f, variants);

            quote! {
                #(#filtered_attrs)*
                #vis #name: #ty
            }
        })
        .collect();

    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        #(#attrs)*
        #visibility struct #struct_name #impl_generics #where_clause {
            #(#cleaned_fields),*
        }
    }
}

pub fn impl_domain_struct(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as DomainStructArgs);
    let input = parse_macro_input!(input as DeriveInput);

    // Extract derives and attributes to forward
    let derives = extract_derives(&input.attrs);
    let struct_attrs = extract_struct_attrs(&input.attrs);

    // Generate the original struct with helper attributes stripped
    let original = generate_original_struct(&input, &args.variants);

    // Generate a struct for each variant
    let variant_structs: Vec<_> = args
        .variants
        .iter()
        .map(|variant| generate_domain_struct(&input, variant, &derives, &struct_attrs))
        .collect();

    let expanded = quote! {
        #original
        #(#variant_structs)*
    };

    TokenStream::from(expanded)
}
