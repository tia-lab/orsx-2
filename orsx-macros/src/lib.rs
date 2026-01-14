use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(OrsxMigrate, attributes(orsx_table))]
pub fn derive_orsx_migrate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let table_name = extract_table_name(&input.attrs)
        .unwrap_or_else(|| name.to_string().to_lowercase());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics orsx::OrsxMigrate for #name #ty_generics #where_clause {
            fn table_name() -> &'static str {
                #table_name
            }
        }
    };

    TokenStream::from(expanded)
}

fn extract_table_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("orsx_table") {
            if let Ok(meta_list) = attr.meta.require_list() {
                if let Ok(lit_str) = meta_list.parse_args::<syn::LitStr>() {
                    return Some(lit_str.value());
                }
            }
        }
    }
    None
}

