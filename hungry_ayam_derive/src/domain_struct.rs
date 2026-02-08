//! `domain_struct` attribute macro implementation.
//!
//! This attribute macro generates Create/Update variants of domain structs,
//! automatically forwarding derives and struct-level attributes.

use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments,
    Meta, Attribute, Token, punctuated::Punctuated, Field,
};
use syn::parse::{Parse, ParseStream};

/// Helper attributes that should be stripped from the original struct
const HELPER_ATTRS: &[&str] = &[
    "derived_domain_ignore",
    "create_ignore",
    "update_ignore",
    "create_optional",
    "derived_domain_optional",
    "update_required",
];

/// Configuration for which structs to generate
#[derive(Default)]
pub struct DomainStructArgs {
    pub generate_create: bool,
    pub generate_update: bool,
}

impl Parse for DomainStructArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = DomainStructArgs::default();

        let idents = Punctuated::<syn::Ident, Token![,]>::parse_terminated(input)?;

        for ident in idents {
            match ident.to_string().as_str() {
                "create" => args.generate_create = true,
                "update" => args.generate_update = true,
                other => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("unknown option '{}', expected 'create' or 'update'", other),
                    ));
                }
            }
        }

        Ok(args)
    }
}

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
fn is_helper_attr(attr: &Attribute) -> bool {
    HELPER_ATTRS.iter().any(|name| attr.path().is_ident(name))
}

/// Extracts derive macros from `#[derive(...)]` attributes
fn extract_derives(attrs: &[Attribute]) -> Vec<proc_macro2::TokenStream> {
    let mut derives = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Meta::List(meta_list) = &attr.meta {
                let tokens = &meta_list.tokens;
                let token_str = tokens.to_string();

                // Split by comma and collect derive names
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

/// Extracts struct-level attributes to forward (like #[ts(export)], #[serde(...)])
/// Excludes derive attributes (handled separately)
fn extract_struct_attrs(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("derive"))
        .collect()
}

/// Filter out helper attributes from a field's attributes
fn filter_field_attrs(field: &Field) -> Vec<&Attribute> {
    field.attrs.iter().filter(|attr| !is_helper_attr(attr)).collect()
}

#[derive(Clone, Copy, PartialEq)]
pub enum DomainKind {
    Create,
    Update,
}

fn generate_domain_struct(
    input: &DeriveInput,
    kind: DomainKind,
    derives: &[proc_macro2::TokenStream],
    struct_attrs: &[&Attribute],
) -> proc_macro2::TokenStream {
    let struct_name = &input.ident;
    let prefix = match kind {
        DomainKind::Create => "Create",
        DomainKind::Update => "Update",
    };
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

    let mut generated_fields = Vec::new();

    for f in fields.iter() {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        let vis = &f.vis;

        // Check attributes
        let is_derived_domain_ignore = f.attrs.iter().any(|attr| attr.path().is_ident("derived_domain_ignore"));
        let is_create_ignore = f.attrs.iter().any(|attr| attr.path().is_ident("create_ignore"));
        let is_update_ignore = f.attrs.iter().any(|attr| attr.path().is_ident("update_ignore"));
        let is_create_optional = f.attrs.iter().any(|attr| attr.path().is_ident("create_optional"));
        let is_derived_domain_optional = f.attrs.iter().any(|attr| attr.path().is_ident("derived_domain_optional"));
        let is_update_required = f.attrs.iter().any(|attr| attr.path().is_ident("update_required"));

        // Should ignore?
        let should_ignore = is_derived_domain_ignore || match kind {
            DomainKind::Create => is_create_ignore,
            DomainKind::Update => is_update_ignore,
        };

        if should_ignore {
            continue;
        }

        // Should wrap in Option?
        let should_wrap_in_option = match kind {
            DomainKind::Create => is_create_optional || is_derived_domain_optional,
            DomainKind::Update => !is_update_required,
        };

        let generated_field_ty = if should_wrap_in_option && !is_option_type(ty) {
            quote! { Option<#ty> }
        } else {
            quote! { #ty }
        };

        generated_fields.push(quote! {
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
fn generate_original_struct(input: &DeriveInput) -> proc_macro2::TokenStream {
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
    let cleaned_fields: Vec<_> = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let vis = &f.vis;
        let filtered_attrs = filter_field_attrs(f);

        quote! {
            #(#filtered_attrs)*
            #vis #name: #ty
        }
    }).collect();

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
    let original = generate_original_struct(&input);

    // Generate Create struct if requested
    let create_struct = if args.generate_create {
        generate_domain_struct(&input, DomainKind::Create, &derives, &struct_attrs)
    } else {
        quote! {}
    };

    // Generate Update struct if requested
    let update_struct = if args.generate_update {
        generate_domain_struct(&input, DomainKind::Update, &derives, &struct_attrs)
    } else {
        quote! {}
    };

    let expanded = quote! {
        #original
        #create_struct
        #update_struct
    };

    TokenStream::from(expanded)
}
