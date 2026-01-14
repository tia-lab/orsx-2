use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, LitStr, Meta, PathArguments,
    Type,
};

#[proc_macro_derive(OrsxMigrate, attributes(orsx_table, orsx_column))]
pub fn derive_orsx_migrate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let table_name = extract_table_name(&input.attrs)
        .unwrap_or_else(|| name.to_string().to_lowercase());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut columns = Vec::new();
    let mut indexes = Vec::new();

    if let Data::Struct(s) = &input.data {
        if let Fields::Named(fields) = &s.fields {
            for field in &fields.named {
                let ident = field.ident.as_ref().expect("named field");
                let field_name = ident.to_string();

                let (nullable, inner_ty) = unwrap_option(&field.ty);
                let field_type = rust_type_to_field_type(&inner_ty);

                let is_pk = has_flag(&field.attrs, "primary_key");
                let is_unique = has_flag(&field.attrs, "unique");
                let default_sql = parse_default_sql(&field.attrs);

                columns.push(quote! {
                    orsx::ColumnSpec {
                        name: #field_name,
                        ty: #field_type,
                        nullable: #nullable,
                        primary_key: #is_pk,
                        unique: #is_unique,
                        default_sql: #default_sql,
                    }
                });

                if let Some(idx) = parse_index(&field.attrs, &field_name) {
                    indexes.push(idx);
                }
            }
        }
    }

    let expanded = quote! {
        impl #impl_generics orsx::OrsxMigrate for #name #ty_generics #where_clause {
            fn spec() -> orsx::TableSpec {
                const COLUMNS: &[orsx::ColumnSpec] = &[
                    #(#columns),*
                ];
                const INDEXES: &[orsx::IndexInfo] = &[
                    #(#indexes),*
                ];

                orsx::TableSpec {
                    table_name: #table_name,
                    columns: COLUMNS,
                    indexes: INDEXES,
                }
            }
        }
    };

    TokenStream::from(expanded)
}

fn extract_table_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("orsx_table") {
            if let Ok(meta_list) = attr.meta.require_list() {
                if let Ok(lit_str) = meta_list.parse_args::<LitStr>() {
                    return Some(lit_str.value());
                }
            }
        }
    }
    None
}

fn unwrap_option(ty: &Type) -> (bool, Type) {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return (true, inner.clone());
                    }
                }
            }
        }
    }
    (false, ty.clone())
}

fn rust_type_to_field_type(ty: &Type) -> proc_macro2::TokenStream {
    match type_name(ty).as_deref() {
        Some("String") | Some("str") => quote! { orsx::FieldType::Text },
        Some("i32") => quote! { orsx::FieldType::Integer },
        Some("i64") => quote! { orsx::FieldType::BigInt },
        Some("f32") => quote! { orsx::FieldType::Real },
        Some("f64") => quote! { orsx::FieldType::DoublePrecision },
        Some("bool") => quote! { orsx::FieldType::Boolean },
        Some("Vec") => match vec_inner_name(ty).as_deref() {
            Some("i32") | Some("i16") | Some("i8") | Some("u32") | Some("u16") | Some("u8") => {
                quote! { orsx::FieldType::IntegerArray }
            }
            Some("i64") | Some("u64") => quote! { orsx::FieldType::BigIntArray },
            Some("f32") | Some("f64") => quote! { orsx::FieldType::DoublePrecisionArray },
            Some("String") | Some("str") => quote! { orsx::FieldType::TextArray },
            _ => quote! { orsx::FieldType::Text },
        },
        Some("Compressed") => quote! { orsx::FieldType::Bytea },
        Some("Timestamp") => quote! { orsx::FieldType::TimestampTz },
        Some("Vector") => quote! { orsx::FieldType::Vector(384) },
        _ => quote! { orsx::FieldType::Text },
    }
}

fn type_name(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        return tp.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

fn vec_inner_name(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        let seg = tp.path.segments.last()?;
        if seg.ident != "Vec" {
            return None;
        }
        if let PathArguments::AngleBracketed(args) = &seg.arguments {
            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                return type_name(inner);
            }
        }
    }
    None
}

fn has_flag(attrs: &[syn::Attribute], flag: &str) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("orsx_column") {
            continue;
        }
        if let Ok(meta_list) = attr.meta.require_list() {
            if let Ok(meta) = syn::parse2::<Meta>(meta_list.tokens.clone()) {
                if meta.path().is_ident(flag) {
                    return true;
                }
            }
            // Fallback: any nested meta that matches the flag.
            if meta_list.tokens.to_string().contains(flag) {
                return true;
            }
        }
    }
    false
}

fn parse_index(
    attrs: &[syn::Attribute],
    field_name: &str,
) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if !attr.path().is_ident("orsx_column") {
            continue;
        }
        let meta_list = attr.meta.require_list().ok()?;
        // Accept forms:
        // - #[orsx_column(index)]
        // - #[orsx_column(index(unique))]
        // - #[orsx_column(index(type = "gin"))]
        let tokens = meta_list.tokens.to_string();
        if !tokens.contains("index") {
            continue;
        }

        let unique = tokens.contains("unique");
        let index_type = if tokens.contains("gin") {
            quote! { orsx::IndexType::Gin }
        } else if tokens.contains("gist") {
            quote! { orsx::IndexType::Gist }
        } else if tokens.contains("hash") {
            quote! { orsx::IndexType::Hash }
        } else {
            quote! { orsx::IndexType::BTree }
        };

        let index_name = format!("idx_{field_name}");
        return Some(quote! {
            orsx::IndexInfo {
                name: #index_name,
                columns: &[#field_name],
                unique: #unique,
                index_type: #index_type,
            }
        });
    }
    None
}

fn parse_default_sql(attrs: &[syn::Attribute]) -> proc_macro2::TokenStream {
    for attr in attrs {
        if !attr.path().is_ident("orsx_column") {
            continue;
        }
        let mut found: Option<String> = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default_sql") {
                let v: LitStr = meta.value()?.parse()?;
                found = Some(v.value());
            }
            Ok(())
        });
        if let Some(v) = found {
            return quote! { Some(#v) };
        }
    }
    quote! { None }
}
