use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Path};

#[proc_macro_derive(IntoDomain, attributes(into_domain, domain_with_urlstring))]
pub fn into_domain_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    impl_into_domain_derive(input)
}


fn impl_into_domain_derive(input : DeriveInput) -> TokenStream{

    let struct_name = &input.ident;
    // Try to find #[into_domain(SomeType)]
    let mut domain_name = None;
    for attr in &input.attrs {
        if attr.path().is_ident("into_domain") {
            // Parse the attribute tokens as a Path, e.g. #[into_domain(Item)]
            let path: Path = attr.parse_args().expect("Expected a type name in #[into_domain(Type)]");
            if let Some(ident) = path.get_ident() {
                domain_name = Some(ident.clone());
            }
        }
    }


    let domain_name = match domain_name {
        Some(name) => name,
        None => {
            // Emit a compile error if the attribute is missing
            return syn::Error::new_spanned(
                struct_name,
                "Missing #[into_domain(DomainType)] attribute. Please specify the domain struct."
            )
            .to_compile_error()
            .into();
        }
    };

    let fields = if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            &fields_named.named
        } else {
            panic!("IntoDomain can only be derived for structs with named fields");
        }
    } else {
        panic!("IntoDomain can only be derived for structs");
    };

    // Generate field mappings
    let field_mappings = fields.iter().map(|f| {
        let name = &f.ident;
        // Check for #[domain_with_urlstring] attribute
        let is_urlstring = f.attrs.iter().any(|attr| attr.path().is_ident("domain_with_urlstring"));
        if is_urlstring {
            quote! { #name: self.#name.and_then(|s| url::Url::parse(&s).ok().map(UrlString)), }
        } else {
            quote! { #name: self.#name, }
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
