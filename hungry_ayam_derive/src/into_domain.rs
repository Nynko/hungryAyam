//! `IntoDomain` derive macro implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Data, Fields, Path, Lit, Meta, Expr};

/// Implements the `IntoDomain` derive macro.
pub fn impl_into_domain_derive(input: DeriveInput) -> TokenStream {
    let struct_name = &input.ident;

    // Find #[into_domain(TargetType)]
    let mut domain_name = None;
    for attr in &input.attrs {
        if attr.path().is_ident("into_domain") {
            let path: Path = attr.parse_args().expect("Expected #[into_domain(Type)]");
            if let Some(ident) = path.get_ident() {
                domain_name = Some(ident.clone());
            }
        }
    }

    let domain_name = match domain_name {
        Some(name) => name,
        None => {
            return syn::Error::new_spanned(
                struct_name,
                "Missing #[into_domain(DomainType)] attribute."
            )
            .to_compile_error()
            .into();
        }
    };

    let fields = if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            &fields_named.named
        } else {
            panic!("IntoDomain only supports structs with named fields");
        }
    } else {
        panic!("IntoDomain only supports structs");
    };

    // Generate field mappings
    let field_mappings = fields.iter().map(|f| {
        let source_name = &f.ident;

        // Check for field attributes
        let is_ignored = f.attrs.iter().any(|attr| attr.path().is_ident("into_domain_ignored"));

        // Check for #[into_domain_with(function_path)]
        let with_fn = f.attrs.iter()
            .find(|attr| attr.path().is_ident("into_domain_with"))
            .and_then(|attr| {
                attr.parse_args::<Path>().ok()
            });

        // Check for #[into_domain_name = "target_field_name"]
        let target_name = f.attrs.iter()
            .find(|attr| attr.path().is_ident("into_domain_name"))
            .and_then(|attr| {
                if let Meta::NameValue(meta) = &attr.meta {
                    if let Expr::Lit(expr_lit) = &meta.value {
                        if let Lit::Str(lit_str) = &expr_lit.lit {
                            return Some(syn::Ident::new(&lit_str.value(), lit_str.span()));
                        }
                    }
                }
                None
            })
            .unwrap_or_else(|| source_name.clone().unwrap());

        if is_ignored {
            quote! {}
        } else if let Some(fn_path) = with_fn {
            // Use the specified function for conversion
            // The function should return Result<T, E> - we call .expect() for now
            // Or it can return T directly
            quote! { #target_name: #fn_path(self.#source_name).expect("Conversion failed"), }
        } else {
            quote! { #target_name: self.#source_name, }
        }
    });

    let expanded = quote! {
        impl IntoDomain<#domain_name> for #struct_name {
            fn into_domain(self) -> #domain_name {
                #domain_name {
                    #(#field_mappings)*
                }
            }
        }
    };

    TokenStream::from(expanded)
}