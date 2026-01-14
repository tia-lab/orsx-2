use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type, TypePath};

#[proc_macro_derive(OrsxMigrate, attributes(orsx_table, orsx_column))]
pub fn derive_orsx_migrate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract table name from attributes or use default (lowercase struct name)
    let table_name =
        extract_table_name(&input.attrs).unwrap_or_else(|| name.to_string().to_lowercase());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Extract field metadata
    let (
        field_names,
        field_types,
        nullable_flags,
        primary_key_field,
        column_defs,
        indexes,
        field_idents,
    ) = if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            extract_field_metadata(&fields.named)
        } else {
            (vec![], vec![], vec![], None, vec![], vec![], vec![])
        }
    } else {
        (vec![], vec![], vec![], None, vec![], vec![], vec![])
    };

    // Split field_idents into non-PK and PK for UPDATE binding
    let (non_pk_idents, pk_ident): (Vec<_>, Vec<_>) = if let Some(ref pk_field) = primary_key_field
    {
        field_idents.iter().partition(|ident| *pk_field != **ident)
    } else {
        (field_idents.iter().collect(), vec![])
    };

    // Generate primary key field name
    let pk_field_name = if let Some(ref pk) = primary_key_field {
        quote! { stringify!(#pk) }
    } else {
        quote! { "id" }
    };

    // Generate migration SQL
    let migration_sql = {
        let columns_sql = column_defs.join(",\n    ");
        format!(
            "CREATE TABLE IF NOT EXISTS {} (\n    {}\n)",
            table_name, columns_sql
        )
    };

    // Generate trait implementation
    let expanded = quote! {
        impl #impl_generics orsx::OrsxMigrate for #name #ty_generics #where_clause {
            fn table_name() -> &'static str {
                #table_name
            }

            fn primary_key_field() -> &'static str {
                #pk_field_name
            }

            fn field_names() -> Vec<&'static str> {
                vec![#(#field_names),*]
            }

            fn field_types() -> Vec<orsx::FieldType> {
                vec![#(#field_types),*]
            }

            fn field_nullable() -> Vec<bool> {
                vec![#(#nullable_flags),*]
            }

            fn migration_sql() -> String {
                #migration_sql.to_string()
            }

            fn table_indexes() -> Vec<orsx::indexes::IndexInfo> {
                vec![#(#indexes),*]
            }

            fn bind_values_to_query<'q>(
                &'q self,
                mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
            ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
                #(
                    query = query.bind(&self.#field_idents);
                )*
                query
            }

            fn bind_values_for_update<'q>(
                &'q self,
                mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
            ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
                // Bind non-PK fields first (for SET clause)
                #(
                    query = query.bind(&self.#non_pk_idents);
                )*
                // Bind PK field last (for WHERE clause)
                #(
                    query = query.bind(&self.#pk_ident);
                )*
                query
            }
        }
    };

    TokenStream::from(expanded)
}

// Extract table name from #[orsx_table("name")] attribute
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

// Extract field metadata from struct fields
#[allow(clippy::type_complexity)]
fn extract_field_metadata(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> (
    Vec<proc_macro2::TokenStream>, // field names
    Vec<proc_macro2::TokenStream>, // field types (FieldType enum)
    Vec<bool>,                     // nullable flags
    Option<proc_macro2::Ident>,    // primary key field
    Vec<String>,                   // column definitions for SQL
    Vec<proc_macro2::TokenStream>, // index definitions
    Vec<proc_macro2::Ident>,       // field identifiers for binding
) {
    let mut field_names = vec![];
    let mut field_types = vec![];
    let mut nullable_flags = vec![];
    let mut primary_key_field = None;
    let mut column_defs = vec![];
    let mut field_idents = vec![];
    let mut indexes = vec![];

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Check for primary key attribute
        let is_primary_key = has_attribute(&field.attrs, "orsx_column", "primary_key");
        if is_primary_key {
            primary_key_field = Some(field_name.clone());
        }

        // Extract index information from field attributes
        if let Some(index_info) = extract_index_from_field(&field.attrs, field_name) {
            indexes.push(index_info);
        }

        // Determine if field is nullable (Option<T>)
        let is_nullable = is_option_type(field_type);

        // Map Rust type to FieldType
        let rust_type_info = extract_inner_type(field_type);
        let field_type_enum = rust_type_to_field_type(&rust_type_info);

        // Generate column definition for migration SQL
        let column_def =
            generate_column_def(field_name, &field_type_enum, is_nullable, is_primary_key);

        field_names.push(quote! { stringify!(#field_name) });
        field_types.push(field_type_enum);
        nullable_flags.push(is_nullable);
        column_defs.push(column_def);
        field_idents.push(field_name.clone());
    }

    (
        field_names,
        field_types,
        nullable_flags,
        primary_key_field,
        column_defs,
        indexes,
        field_idents,
    )
}

// Extract index information from field attributes
// Supports: #[orsx_column(index)], #[orsx_column(index(unique))], #[orsx_column(index(type = "gin"))]
fn extract_index_from_field(
    attrs: &[syn::Attribute],
    field_name: &proc_macro2::Ident,
) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if attr.path().is_ident("orsx_column") {
            if let Ok(meta_list) = attr.meta.require_list() {
                let tokens = meta_list.tokens.to_string();

                // Check if field has index attribute
                if tokens.contains("index") {
                    let is_unique = tokens.contains("unique");

                    // Determine index type from attributes
                    let index_type = if tokens.contains("gin") {
                        quote! { orsx::indexes::IndexType::Gin }
                    } else if tokens.contains("gist") {
                        quote! { orsx::indexes::IndexType::Gist }
                    } else if tokens.contains("hash") {
                        quote! { orsx::indexes::IndexType::Hash }
                    } else {
                        quote! { orsx::indexes::IndexType::BTree }
                    };

                    let index_name = format!("idx_{}", field_name);
                    let field_name_str = field_name.to_string();

                    return Some(quote! {
                        orsx::indexes::IndexInfo {
                            name: #index_name.to_string(),
                            columns: vec![#field_name_str.to_string()],
                            unique: #is_unique,
                            index_type: #index_type,
                        }
                    });
                }
            }
        }
    }

    None
}

// Check if field has specific attribute
fn has_attribute(attrs: &[syn::Attribute], attr_name: &str, arg: &str) -> bool {
    for attr in attrs {
        if attr.path().is_ident(attr_name) {
            if let Ok(meta_list) = attr.meta.require_list() {
                let tokens = meta_list.tokens.to_string();
                if tokens.contains(arg) {
                    return true;
                }
            }
        }
    }
    false
}

// Check if type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

// Extract inner type from Option<T> or Compressed<T>
fn extract_inner_type(ty: &Type) -> String {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            let type_name = segment.ident.to_string();

            // Handle Option<T>
            if type_name == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return extract_inner_type(inner_ty);
                    }
                }
            }

            // Handle Vec<T>
            if type_name == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        let inner_type_str = extract_inner_type(inner_ty);
                        return format!("Vec<{}>", inner_type_str);
                    }
                }
                return "Vec".to_string();
            }

            // Handle Compressed<Vec<T>>
            if type_name == "Compressed" {
                return "Bytea".to_string(); // Compressed types stored as bytea
            }

            // Handle Vector (pgvector)
            if type_name == "Vector" {
                return "Vector".to_string();
            }

            return type_name;
        }
    }
    "Text".to_string()
}

// Map Rust type string to FieldType enum
fn rust_type_to_field_type(rust_type: &str) -> proc_macro2::TokenStream {
    // Handle Vec<T> types first
    if rust_type.starts_with("Vec<") {
        if let Some(inner) = rust_type
            .strip_prefix("Vec<")
            .and_then(|s| s.strip_suffix(">"))
        {
            return match inner {
                "i32" | "i16" | "i8" | "u32" | "u16" | "u8" => {
                    quote! { orsx::FieldType::IntegerArray }
                }
                "i64" | "u64" => quote! { orsx::FieldType::BigIntArray },
                "f32" | "f64" => quote! { orsx::FieldType::DoublePrecisionArray },
                "String" | "str" => quote! { orsx::FieldType::TextArray },
                _ => quote! { orsx::FieldType::Text },
            };
        }
    }

    // Handle scalar types
    match rust_type {
        "String" | "str" => quote! { orsx::FieldType::Text },
        "i32" => quote! { orsx::FieldType::Integer },
        "i64" => quote! { orsx::FieldType::BigInt },
        "f32" => quote! { orsx::FieldType::Real },
        "f64" => quote! { orsx::FieldType::DoublePrecision },
        "bool" => quote! { orsx::FieldType::Boolean },
        "Timestamp" => quote! { orsx::FieldType::Timestamp },
        "Bytea" => quote! { orsx::FieldType::Bytea },
        "Value" => quote! { orsx::FieldType::Jsonb },
        "Vector" => quote! { orsx::FieldType::Vector(384) }, // Default dimension
        _ => quote! { orsx::FieldType::Text },
    }
}

// Generate SQL column definition
fn generate_column_def(
    field_name: &proc_macro2::Ident,
    field_type: &proc_macro2::TokenStream,
    is_nullable: bool,
    is_primary_key: bool,
) -> String {
    let field_name_str = field_name.to_string();

    // Extract the FieldType variant to get SQL type
    let sql_type = match field_type.to_string().as_str() {
        s if s.contains("IntegerArray") => "INTEGER[]",
        s if s.contains("BigIntArray") => "BIGINT[]",
        s if s.contains("DoublePrecisionArray") => "DOUBLE PRECISION[]",
        s if s.contains("TextArray") => "TEXT[]",
        s if s.contains("Text") => "TEXT",
        s if s.contains("Integer") => "INTEGER",
        s if s.contains("BigInt") => "BIGINT",
        s if s.contains("Real") => "REAL",
        s if s.contains("DoublePrecision") => "DOUBLE PRECISION",
        s if s.contains("Boolean") => "BOOLEAN",
        s if s.contains("Timestamp") => "TIMESTAMPTZ",
        s if s.contains("Bytea") => "BYTEA",
        s if s.contains("Jsonb") => "JSONB",
        s if s.contains("Vector") => "vector(384)",
        _ => "TEXT",
    };

    let mut parts = vec![field_name_str, sql_type.to_string()];

    if is_primary_key {
        parts.push("PRIMARY KEY".to_string());
        // Primary keys typically have a default UUID generator
        if sql_type == "TEXT" {
            parts.push("DEFAULT gen_random_uuid()::text".to_string());
        }
    }

    if !is_nullable && !is_primary_key {
        parts.push("NOT NULL".to_string());
    }

    parts.join(" ")
}
