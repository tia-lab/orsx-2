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

    let table = parse_orsx_table(&input.attrs);
    let table_name = table
        .as_ref()
        .and_then(|t| t.table_name.as_ref())
        .map(|s| s.value())
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
                let rename_from = parse_rename_from(&field.attrs);

                columns.push(quote! {
                    orsx::ColumnSpec {
                        name: #field_name,
                        rename_from: #rename_from,
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

    if let Some(table) = &table {
        for idx in &table.indexes {
            indexes.push(idx.to_index_info_tokens());
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

#[proc_macro_derive(OrsxColumnar, attributes(orsx_table, orsx_column))]
pub fn derive_orsx_columnar(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut fields_ts = Vec::new();
    let mut idx_consts = Vec::new();

    if let Data::Struct(s) = &input.data {
        if let Fields::Named(fields) = &s.fields {
            for (idx, field) in fields.named.iter().enumerate() {
                let ident = field.ident.as_ref().expect("named field");
                let field_name = ident.to_string();

                let (_, inner_ty) = unwrap_option(&field.ty);
                let columnar_type = match rust_type_to_columnar_type(&inner_ty) {
                    Ok(ts) => ts,
                    Err(err) => return TokenStream::from(err.to_compile_error()),
                };

                let const_ident = syn::Ident::new(
                    &format!("COL_{}", field_name.to_uppercase()),
                    ident.span(),
                );
                let idx_lit = idx;

                idx_consts.push(quote! {
                    pub const #const_ident: usize = #idx_lit;
                });

                fields_ts.push(quote! {
                    orsx::columnar::ColumnarField {
                        name: Some(#field_name.to_string()),
                        ty: #columnar_type,
                    }
                });
            }
        }
    }

    let expanded = quote! {
        impl #impl_generics orsx::columnar::OrsxColumnar for #name #ty_generics #where_clause {
            fn columnar_schema() -> orsx::Result<orsx::columnar::ColumnarSchema> {
                orsx::columnar::ColumnarSchema::new(vec![
                    #(#fields_ts),*
                ])
            }
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #(#idx_consts)*
        }
    };

    TokenStream::from(expanded)
}

struct OrsxTableSpec {
    table_name: Option<LitStr>,
    indexes: Vec<OrsxIndexDecl>,
}

#[derive(Clone)]
struct OrsxIndexDecl {
    name: Option<LitStr>,
    unique: bool,
    method: String,
    columns: Vec<LitStr>,
}

impl OrsxIndexDecl {
    fn to_index_info_tokens(&self) -> proc_macro2::TokenStream {
        let unique = self.unique;
        // If no explicit name is provided, emit an empty name and let the runtime derive a
        // deterministic, table-specific name (important for table-name overrides).
        let name = self.name.as_ref().map(|s| s.value()).unwrap_or_default();
        let method = self.method.as_str();
        let index_type = match method {
            "btree" => quote! { orsx::IndexType::BTree },
            "hash" => quote! { orsx::IndexType::Hash },
            "gin" => quote! { orsx::IndexType::Gin },
            "gist" => quote! { orsx::IndexType::Gist },
            _ => quote! { orsx::IndexType::BTree },
        };
        let cols: Vec<String> = self.columns.iter().map(|c| c.value()).collect();
        let cols_tokens: Vec<proc_macro2::TokenStream> = cols
            .iter()
            .map(|c| quote! { #c })
            .collect();
        quote! {
            orsx::IndexInfo {
                name: #name,
                columns: &[#(#cols_tokens),*],
                unique: #unique,
                index_type: #index_type,
            }
        }
    }
}

fn parse_orsx_table(attrs: &[syn::Attribute]) -> Option<OrsxTableSpec> {
    for attr in attrs {
        if !attr.path().is_ident("orsx_table") {
            continue;
        }
        let meta_list = attr.meta.require_list().ok()?;
        let parsed = syn::parse2::<OrsxTableArgs>(meta_list.tokens.clone()).ok()?;
        return Some(parsed.into_spec());
    }
    None
}

struct OrsxTableArgs {
    table_name: Option<LitStr>,
    indexes: Vec<OrsxIndexDecl>,
}

impl syn::parse::Parse for OrsxTableArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut table_name: Option<LitStr> = None;
        let mut indexes: Vec<OrsxIndexDecl> = Vec::new();

        while !input.is_empty() {
            if input.peek(LitStr) {
                if table_name.is_some() {
                    return Err(syn::Error::new(
                        input.span(),
                        "orsx_table: table name may only be specified once",
                    ));
                }
                table_name = Some(input.parse::<LitStr>()?);
            } else if input.peek(syn::Ident) {
                let ident = input.parse::<syn::Ident>()?;
                if ident == "index" {
                    let content;
                    syn::parenthesized!(content in input);
                    indexes.push(parse_index_decl(&content)?);
                } else {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "orsx_table: expected `index(...)`",
                    ));
                }
            } else {
                return Err(syn::Error::new(
                    input.span(),
                    "orsx_table: expected table name string or `index(...)`",
                ));
            }

            if input.peek(syn::Token![,]) {
                let _ = input.parse::<syn::Token![,]>()?;
            } else {
                break;
            }
        }

        Ok(Self { table_name, indexes })
    }
}

impl OrsxTableArgs {
    fn into_spec(self) -> OrsxTableSpec {
        OrsxTableSpec {
            table_name: self.table_name,
            indexes: self.indexes,
        }
    }
}

fn parse_index_decl(input: syn::parse::ParseStream) -> syn::Result<OrsxIndexDecl> {
    // index(columns("a","b"), unique, type="gin", name="...")
    let mut columns: Option<Vec<LitStr>> = None;
    let mut unique = false;
    let mut method: String = "btree".to_string();
    let mut name: Option<LitStr> = None;

    while !input.is_empty() {
        if input.peek(syn::Ident) {
            let ident = input.parse::<syn::Ident>()?;
            if ident == "unique" {
                unique = true;
            } else if ident == "columns" {
                let content;
                syn::parenthesized!(content in input);
                let mut cols: Vec<LitStr> = Vec::new();
                while !content.is_empty() {
                    cols.push(content.parse::<LitStr>()?);
                    if content.peek(syn::Token![,]) {
                        let _ = content.parse::<syn::Token![,]>()?;
                    } else {
                        break;
                    }
                }
                if cols.is_empty() {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "index(columns(...)): requires at least one column",
                    ));
                }
                columns = Some(cols);
            } else if ident == "type" {
                let _eq = input.parse::<syn::Token![=]>()?;
                let v = input.parse::<LitStr>()?.value();
                method = v.to_lowercase();
            } else if ident == "name" {
                let _eq = input.parse::<syn::Token![=]>()?;
                name = Some(input.parse::<LitStr>()?);
            } else {
                return Err(syn::Error::new_spanned(
                    ident,
                    "index(...): expected `columns(...)`, `unique`, `type=...`, or `name=...`",
                ));
            }
        } else {
            return Err(syn::Error::new(
                input.span(),
                "index(...): expected an identifier item",
            ));
        }

        if input.peek(syn::Token![,]) {
            let _ = input.parse::<syn::Token![,]>()?;
        } else {
            break;
        }
    }

    let cols = columns.ok_or_else(|| {
        syn::Error::new(input.span(), "index(...): missing required `columns(...)`")
    })?;

    let allowed = ["btree", "gin", "gist", "hash"];
    if !allowed.contains(&method.as_str()) {
        return Err(syn::Error::new(
            input.span(),
            format!("index(...): unsupported index type `{method}`"),
        ));
    }

    Ok(OrsxIndexDecl {
        name,
        unique,
        method,
        columns: cols,
    })
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
        Some("Uuid") => quote! { orsx::FieldType::Uuid },
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

fn rust_type_to_columnar_type(ty: &Type) -> syn::Result<proc_macro2::TokenStream> {
    let name = type_name(ty);
    let last = name.as_deref().unwrap_or_default();

    let ts = match last {
        "String" | "str" => quote! { orsx::columnar::ColumnarType::Utf8 },
        "bool" => quote! { orsx::columnar::ColumnarType::Bool },
        "i16" => quote! { orsx::columnar::ColumnarType::I16 },
        "i32" => quote! { orsx::columnar::ColumnarType::I32 },
        "i64" => quote! { orsx::columnar::ColumnarType::I64 },
        "f32" => quote! { orsx::columnar::ColumnarType::F32 },
        "f64" => quote! { orsx::columnar::ColumnarType::F64 },
        // Support both `uuid::Uuid` and `sqlx::types::Uuid` (last segment is still `Uuid`).
        "Uuid" => quote! { orsx::columnar::ColumnarType::Uuid },
        // ORSX exports `Timestamp` and `SqlxTimestamp` aliases; both end in `Timestamp`.
        "Timestamp" => quote! { orsx::columnar::ColumnarType::TimestampTzMicros },
        "Vec" => {
            match vec_inner_name(ty).as_deref() {
                Some("u8") => quote! { orsx::columnar::ColumnarType::Bytes },
                _ => {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "OrsxColumnar only supports Vec<u8> for varlen bytes; use Vec<u8> or add a custom mapping",
                    ));
                }
            }
        }
        _ => {
            return Err(syn::Error::new_spanned(
                ty,
                format!("unsupported OrsxColumnar field type `{last}`; add a supported type or extend the macro mapping"),
            ));
        }
    };

    Ok(ts)
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

fn parse_rename_from(attrs: &[syn::Attribute]) -> proc_macro2::TokenStream {
    for attr in attrs {
        if !attr.path().is_ident("orsx_column") {
            continue;
        }
        let mut found: Option<String> = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_from") {
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
