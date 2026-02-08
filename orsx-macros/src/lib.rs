use proc_macro::TokenStream;
use quote::quote;
use sha2::Digest;
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

#[proc_macro_derive(
    OrsxFlatten,
    attributes(orsx_table, orsx_column, orsx_family, orsx_processor_id)
)]
pub fn derive_orsx_flatten(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let processor_id = match parse_orsx_processor_id(&input.attrs) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return syn::Error::new_spanned(
                &input.ident,
                "OrsxFlatten requires #[orsx_processor_id(\"...\")] on the root type",
            )
            .to_compile_error()
            .into();
        }
        Err(e) => return e.to_compile_error().into(),
    };

    if let Err(e) = validate_id_bytes(processor_id.as_bytes(), "processor_id") {
        return e.to_compile_error().into();
    }

    // Proof-of-assumption: stable Rust proc macros cannot read the invoking source file via `proc_macro::Span`.
    // Recursive flatten is implemented by the module-level macro `#[orsx_flatten_module]` instead.
    syn::Error::new_spanned(
        &input.ident,
        "OrsxFlatten derive does not support recursive flatten on stable Rust. Use #[orsx_flatten_module] on a module containing the related structs (see orsx_FLATTENED_WIDE_SCHEMA_PROTOCOL_SPEC).",
    )
    .to_compile_error()
    .into()
}

#[proc_macro_attribute]
pub fn orsx_flatten_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module = parse_macro_input!(item as syn::ItemMod);

    let Some((_brace, items)) = &module.content else {
        return syn::Error::new_spanned(
            &module,
            "orsx_flatten_module requires an inline module body (e.g. `mod outputs { ... }`), not `mod outputs;`",
        )
        .to_compile_error()
        .into();
    };

    // Phase 3: generate schema/constants from module-contained structs (stable recursion).
    let file_ast = syn::File {
        shebang: None,
        attrs: Vec::new(),
        items: items.clone(),
    };
    let struct_map = collect_structs(&file_ast);

    let mut generated: Vec<proc_macro2::TokenStream> = Vec::new();

    for st in struct_map.values() {
        let processor_id = match parse_orsx_processor_id(&st.attrs) {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };
        let Some(processor_id) = processor_id else {
            continue;
        };

        if let Err(e) = validate_id_bytes(processor_id.as_bytes(), "processor_id") {
            return e.to_compile_error().into();
        }

        let table = parse_orsx_table(&st.attrs);
        let table_name = table
            .as_ref()
            .and_then(|t| t.table_name.as_ref())
            .map(|s| s.value())
            .unwrap_or_else(|| st.ident.to_string().to_lowercase());

        let root_fields = match &st.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return syn::Error::new_spanned(
                    &st.ident,
                    "OrsxFlatten only supports named-field structs",
                )
                .to_compile_error()
                .into();
            }
        };

        let mut columns_in_order: Vec<String> = Vec::new();
        let mut provenance_specs: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut columnar_fields_in_order: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut visit_calls_in_order: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut metric_cols: Vec<MetricCol> = Vec::new();

        for field in root_fields {
            if parse_skip(&field.attrs) {
                continue;
            }
            let family_prefix = match parse_orsx_family_prefix(&field.attrs) {
                Ok(v) => v,
                Err(e) => return e.to_compile_error().into(),
            };
            let Some(prefix) = family_prefix else {
                let Some(ident) = &field.ident else {
                    continue;
                };
                let field_name = ident.to_string();
                let (nullable, inner_ty) = unwrap_option(&field.ty);
                let field_type_ts = rust_type_to_field_type(&inner_ty);
                let columnar_type_ts = match rust_type_to_columnar_type(&inner_ty) {
                    Ok(v) => v,
                    Err(e) => return e.to_compile_error().into(),
                };

                let is_pk = has_flag(&field.attrs, "primary_key");
                let is_unique = has_flag(&field.attrs, "unique");
                let default_sql = parse_default_sql(&field.attrs);
                let rename_from = parse_rename_from(&field.attrs);

                provenance_specs.push(quote! {
                    orsx::ColumnSpec {
                        name: #field_name,
                        rename_from: #rename_from,
                        ty: #field_type_ts,
                        nullable: #nullable,
                        primary_key: #is_pk,
                        unique: #is_unique,
                        default_sql: #default_sql,
                    }
                });

                columns_in_order.push(field_name.clone());
                columnar_fields_in_order.push(quote! {
                    orsx::columnar::ColumnarField {
                        name: Some(#field_name.to_string()),
                        ty: #columnar_type_ts,
                    }
                });
                let access_ts = quote! { self.#ident };
                match visitor_call_ts_for_flatten_leaf(&field_name, nullable, &inner_ty, &access_ts) {
                    Ok(ts) => visit_calls_in_order.push(ts),
                    Err(e) => return e.to_compile_error().into(),
                }
                continue;
            };

            let Some(ident) = &field.ident else {
                continue;
            };

            if let Err(e) = validate_id_bytes(prefix.as_bytes(), "family_prefix") {
                return e.to_compile_error().into();
            }

            let (nullable, inner_ty) = unwrap_option(&field.ty);
            if nullable {
                return syn::Error::new_spanned(
                    &field.ty,
                    "OrsxFlatten does not support Option<Struct> for family nodes",
                )
                .to_compile_error()
                .into();
            }

            let family_type_name = match type_name(&inner_ty) {
                Some(v) => v,
                None => {
                    return syn::Error::new_spanned(
                        &field.ty,
                        "OrsxFlatten family field must have a concrete named type",
                    )
                    .to_compile_error()
                    .into();
                }
            };

            let Some(family_struct) = struct_map.get(&family_type_name) else {
                return syn::Error::new_spanned(
                    &field.ty,
                    format!(
                        "OrsxFlatten could not locate struct definition for `{family_type_name}` in the same module; recursive flatten is module-scoped"
                    ),
                )
                .to_compile_error()
                .into();
            };

            let access_prefix = quote! { self.#ident };
            if let Err(e) = flatten_metrics_from_struct(
                &struct_map,
                family_struct,
                &prefix,
                &access_prefix,
                &mut Vec::new(),
                &mut metric_cols,
            ) {
                return e.to_compile_error().into();
            }
        }

        if columns_in_order.is_empty() && metric_cols.is_empty() {
            return syn::Error::new_spanned(
                &st.ident,
                "OrsxFlatten produced an empty schema (no provenance columns and no metric columns)",
            )
            .to_compile_error()
            .into();
        }

        metric_cols.sort_by(|a, b| a.field_id.cmp(&b.field_id));
        for w in metric_cols.windows(2) {
            if w[0].field_id == w[1].field_id {
                return syn::Error::new_spanned(
                    &st.ident,
                    format!("OrsxFlatten metric id collision: `{}`", w[0].field_id),
                )
                .to_compile_error()
                .into();
            }
        }

        // Collision checks (metrics vs provenance).
        {
            let mut all_names: Vec<String> =
                Vec::with_capacity(columns_in_order.len() + metric_cols.len());
            all_names.extend(columns_in_order.iter().cloned());
            all_names.extend(metric_cols.iter().map(|m| m.field_id.clone()));
            all_names.sort();
            for w in all_names.windows(2) {
                if w[0] == w[1] {
                    return syn::Error::new_spanned(
                        &st.ident,
                        format!("OrsxFlatten column name collision: `{}`", w[0]),
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }

        // Deterministic ordering: provenance decl order (already in columns_in_order),
        // then metrics sorted by canonical field_id.
        let metric_columns_in_order: Vec<String> =
            metric_cols.iter().map(|m| m.field_id.clone()).collect();
        for m in &metric_cols {
            let field_id = m.field_id.as_str();
            let columnar_ty_ts = &m.columnar_type_ts;
            columns_in_order.push(field_id.to_string());
            columnar_fields_in_order.push(quote! {
                orsx::columnar::ColumnarField {
                    name: Some(#field_id.to_string()),
                    ty: #columnar_ty_ts,
                }
            });
            visit_calls_in_order.push(m.visit_call_ts.clone());
        }

        // Build ColumnSpec list in exactly COLUMNS_IN_ORDER order.
        let mut columns_spec_ts: Vec<proc_macro2::TokenStream> = Vec::new();
        columns_spec_ts.extend(provenance_specs.into_iter());
        for m in &metric_cols {
            let field_id = m.field_id.as_str();
            let nullable = m.nullable;
            let ty_ts = &m.field_type_ts;
            columns_spec_ts.push(quote! {
                orsx::ColumnSpec {
                    name: #field_id,
                    rename_from: None,
                    ty: #ty_ts,
                    nullable: #nullable,
                    primary_key: false,
                    unique: false,
                    default_sql: None,
                }
            });
        }

        // Indexes: provenance field-level indexes + table-level indexes.
        let mut indexes_ts: Vec<proc_macro2::TokenStream> = Vec::new();
        for field in root_fields {
            let family_prefix = match parse_orsx_family_prefix(&field.attrs) {
                Ok(v) => v,
                Err(e) => return e.to_compile_error().into(),
            };
            if family_prefix.is_some() || parse_skip(&field.attrs) {
                continue;
            }
            let Some(ident) = &field.ident else {
                continue;
            };
            let field_name = ident.to_string();
            if let Some(idx) = parse_index(&field.attrs, &field_name) {
                indexes_ts.push(idx);
            }
        }
        if let Some(table) = &table {
            for idx in &table.indexes {
                indexes_ts.push(idx.to_index_info_tokens());
            }
        }

        // Schema hash: metric-only columns in order + type/nullability + processor_id + version constant.
        let generation_version = "orsx_flatten_module_v1";
        let mut hasher = sha2::Sha256::new();
        hasher.update(generation_version.as_bytes());
        hasher.update(b"\n");
        hasher.update(processor_id.as_bytes());
        hasher.update(b"\n");
        for m in &metric_cols {
            hasher.update(m.field_id.as_bytes());
            hasher.update(b":");
            hasher.update(m.field_type_id.as_bytes());
            hasher.update(b":");
            hasher.update(if m.nullable { b"1" } else { b"0" });
            hasher.update(b"\n");
        }
        let digest = hasher.finalize();
        let digest_bytes: Vec<proc_macro2::TokenStream> =
            digest.iter().map(|b| quote! { #b }).collect();

        let columns_in_order_ts: Vec<proc_macro2::TokenStream> = columns_in_order
            .iter()
            .map(|c| {
                let s = c.as_str();
                quote! { #s }
            })
            .collect();
        let metric_columns_in_order_ts: Vec<proc_macro2::TokenStream> = metric_columns_in_order
            .iter()
            .map(|c| {
                let s = c.as_str();
                quote! { #s }
            })
            .collect();

        let root_ident = &st.ident;
        let (impl_generics, ty_generics, where_clause) = st.generics.split_for_impl();

        generated.push(quote! {
            impl #impl_generics orsx::OrsxMigrate for #root_ident #ty_generics #where_clause {
                fn spec() -> orsx::TableSpec {
                    const COLUMNS: &[orsx::ColumnSpec] = &[
                        #(#columns_spec_ts),*
                    ];
                    const INDEXES: &[orsx::IndexInfo] = &[
                        #(#indexes_ts),*
                    ];

                    orsx::TableSpec {
                        table_name: #table_name,
                        columns: COLUMNS,
                        indexes: INDEXES,
                    }
                }
            }

            impl #impl_generics orsx::columnar::OrsxColumnar for #root_ident #ty_generics #where_clause {
                fn columnar_schema() -> orsx::Result<orsx::columnar::ColumnarSchema> {
                    orsx::columnar::ColumnarSchema::new(vec![
                        #(#columnar_fields_in_order),*
                    ])
                }
            }

            impl #impl_generics #root_ident #ty_generics #where_clause {
                pub const ORSX_FLATTEN_GENERATION_VERSION: &'static str = #generation_version;
                pub const COLUMNS_IN_ORDER: &'static [&'static str] = &[
                    #(#columns_in_order_ts),*
                ];
                pub const METRIC_COLUMNS_IN_ORDER: &'static [&'static str] = &[
                    #(#metric_columns_in_order_ts),*
                ];
                pub const SCHEMA_HASH: [u8; 32] = [
                    #(#digest_bytes),*
                ];

                pub fn visit_values_in_order<'q>(
                    &'q self,
                    visitor: &mut impl orsx::OrsxValueVisitor<'q>,
                ) -> orsx::Result<()> {
                    #(#visit_calls_in_order)*
                    Ok(())
                }
            }
        });
    }

    let clean_items = strip_orsx_flatten_marker_attrs(items);

    let attrs = &module.attrs;
    let vis = &module.vis;
    let unsafety = &module.unsafety;
    let ident = &module.ident;

    TokenStream::from(quote! {
        #(#attrs)*
        #vis #unsafety mod #ident {
            #(#clean_items)*
            #(#generated)*
        }
    })
}

fn strip_orsx_flatten_marker_attrs(items: &[syn::Item]) -> Vec<syn::Item> {
    fn strip_attrs(attrs: &mut Vec<syn::Attribute>) {
        attrs.retain(|a| {
            !a.path().is_ident("orsx_table")
                && !a.path().is_ident("orsx_column")
                && !a.path().is_ident("orsx_family")
                && !a.path().is_ident("orsx_processor_id")
        });
    }

    fn strip_item(item: &syn::Item) -> syn::Item {
        match item {
            syn::Item::Struct(s) => {
                let mut s = s.clone();
                strip_attrs(&mut s.attrs);
                match &mut s.fields {
                    syn::Fields::Named(fields) => {
                        for f in fields.named.iter_mut() {
                            strip_attrs(&mut f.attrs);
                        }
                    }
                    syn::Fields::Unnamed(fields) => {
                        for f in fields.unnamed.iter_mut() {
                            strip_attrs(&mut f.attrs);
                        }
                    }
                    syn::Fields::Unit => {}
                }
                syn::Item::Struct(s)
            }
            syn::Item::Mod(m) => {
                let mut m = m.clone();
                strip_attrs(&mut m.attrs);
                if let Some((_brace, inner)) = &mut m.content {
                    let stripped: Vec<syn::Item> = inner.iter().map(strip_item).collect();
                    *inner = stripped;
                }
                syn::Item::Mod(m)
            }
            _ => item.clone(),
        }
    }

    items.iter().map(strip_item).collect()
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

fn parse_orsx_processor_id(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    for attr in attrs {
        if !attr.path().is_ident("orsx_processor_id") {
            continue;
        }
        let meta_list = attr.meta.require_list()?;
        let args = meta_list.tokens.clone();
        // Expect: #[orsx_processor_id("...")]
        let lit: LitStr = syn::parse2(args)?;
        return Ok(Some(lit.value()));
    }
    Ok(None)
}

fn parse_orsx_family_prefix(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    for attr in attrs {
        if !attr.path().is_ident("orsx_family") {
            continue;
        }
        let mut found: Option<String> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let v: LitStr = meta.value()?.parse()?;
                found = Some(v.value());
            }
            Ok(())
        })?;
        return Ok(found);
    }
    Ok(None)
}

fn parse_skip(attrs: &[syn::Attribute]) -> bool {
    has_flag(attrs, "skip")
}

fn parse_id_override(attrs: &[syn::Attribute]) -> syn::Result<Option<String>> {
    for attr in attrs {
        if !attr.path().is_ident("orsx_column") {
            continue;
        }
        let mut found: Option<String> = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                let v: LitStr = meta.value()?.parse()?;
                found = Some(v.value());
            }
            Ok(())
        });
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
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

fn validate_id_bytes(bytes: &[u8], label: &str) -> syn::Result<()> {
    if bytes.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("{label} must not be empty"),
        ));
    }
    if bytes.len() > 63 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("{label} exceeds Postgres identifier length (63 bytes)"),
        ));
    }
    let first = bytes[0];
    if !(b'a'..=b'z').contains(&first) {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("{label} must start with [a-z]"),
        ));
    }
    for &b in &bytes[1..] {
        let ok = (b'a'..=b'z').contains(&b) || (b'0'..=b'9').contains(&b) || b == b'_';
        if !ok {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("{label} must match [a-z][a-z0-9_]*"),
            ));
        }
    }
    Ok(())
}

#[derive(Clone)]
struct MetricCol {
    field_id: String,
    field_type_id: String,
    field_type_ts: proc_macro2::TokenStream,
    columnar_type_ts: proc_macro2::TokenStream,
    nullable: bool,
    visit_call_ts: proc_macro2::TokenStream,
}

fn collect_structs(file: &syn::File) -> std::collections::BTreeMap<String, syn::ItemStruct> {
    fn walk_items(
        items: &[syn::Item],
        out: &mut std::collections::BTreeMap<String, syn::ItemStruct>,
    ) {
        for item in items {
            match item {
                syn::Item::Struct(s) => {
                    out.entry(s.ident.to_string()).or_insert_with(|| s.clone());
                }
                syn::Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        walk_items(inner, out);
                    }
                }
                _ => {}
            }
        }
    }

    let mut out = std::collections::BTreeMap::new();
    walk_items(&file.items, &mut out);
    out
}

fn flatten_metrics_from_struct(
    struct_map: &std::collections::BTreeMap<String, syn::ItemStruct>,
    st: &syn::ItemStruct,
    family_prefix: &str,
    access_prefix: &proc_macro2::TokenStream,
    path: &mut Vec<String>,
    out: &mut Vec<MetricCol>,
) -> syn::Result<()> {
    let fields = match &st.fields {
        Fields::Named(f) => &f.named,
        _ => {
            return Err(syn::Error::new_spanned(
                &st.ident,
                "OrsxFlatten only supports named-field structs for flattening",
            ));
        }
    };

    for field in fields {
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;
        if parse_skip(&field.attrs) {
            continue;
        }

        let field_name = ident.to_string();
        let (nullable, inner_ty) = unwrap_option(&field.ty);
        let access_ts = quote! { #access_prefix.#ident };

        // If this field is a nested struct we can flatten, recurse.
        if let Some(inner_name) = type_name(&inner_ty) {
            if let Some(nested) = struct_map.get(&inner_name) {
                if nullable {
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        "OrsxFlatten does not support Option<Struct> for nested flattening; use a non-optional struct or wrap as JSON",
                    ));
                }
                path.push(field_name);
                flatten_metrics_from_struct(
                    struct_map,
                    nested,
                    family_prefix,
                    &access_ts,
                    path,
                    out,
                )?;
                path.pop();
                continue;
            }
        }

        // Leaf field: build id override or derived id.
        let field_id = match parse_id_override(&field.attrs)? {
            Some(v) => v,
            None => {
                let mut parts: Vec<&str> = Vec::with_capacity(path.len() + 1);
                for p in path.iter() {
                    parts.push(p.as_str());
                }
                parts.push(field_name.as_str());
                let joined = parts.join("_");
                format!("{family_prefix}{joined}")
            }
        };

        validate_id_bytes(field_id.as_bytes(), "field_id")?;

        // Enforce supported leaf types for OrsxFlatten (both migrations and columnar).
        let (field_type_id, field_type_ts) = rust_type_to_field_type_strict_for_flatten(&inner_ty)?;
        let columnar_type_ts = rust_type_to_columnar_type(&inner_ty)?;

        let visit_call_ts = visitor_call_ts_for_flatten_leaf(&field_id, nullable, &inner_ty, &access_ts)?;

        out.push(MetricCol {
            field_id,
            field_type_id,
            field_type_ts,
            columnar_type_ts,
            nullable,
            visit_call_ts,
        });
    }

    Ok(())
}

fn visitor_call_ts_for_flatten_leaf(
    col_name: &str,
    nullable: bool,
    inner_ty: &Type,
    access_ts: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let Some(visit_method) = visitor_method_for_type(inner_ty)? else {
        return Err(syn::Error::new_spanned(
            inner_ty,
            "unsupported OrsxFlatten leaf type for visitor binding; use a supported scalar, Vec<u8>, sqlx::types::JsonValue, sqlx::types::Json<T>, or mark the field #[orsx_column(skip)]",
        ));
    };

    let col_name_lit = col_name;

    let value_ts = if nullable {
        match visit_method {
            VisitorMethod::Text => quote! { #access_ts.as_deref() },
            VisitorMethod::Bytes => quote! { #access_ts.as_deref() },
            VisitorMethod::Uuid
            | VisitorMethod::SqlxTimestamp
            | VisitorMethod::JsonValue
            | VisitorMethod::Json => {
                quote! { #access_ts.as_ref() }
            }
            _ => quote! { #access_ts },
        }
    } else {
        match visit_method {
            VisitorMethod::Text => quote! { Some(#access_ts.as_str()) },
            VisitorMethod::Bytes => quote! { Some(#access_ts.as_slice()) },
            VisitorMethod::Uuid
            | VisitorMethod::SqlxTimestamp
            | VisitorMethod::JsonValue
            | VisitorMethod::Json => {
                quote! { Some(&#access_ts) }
            }
            _ => quote! { Some(#access_ts) },
        }
    };

    Ok(quote! { visitor.#visit_method(#col_name_lit, #value_ts)?; })
}

#[derive(Copy, Clone)]
enum VisitorMethod {
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Text,
    Uuid,
    SqlxTimestamp,
    Bytes,
    JsonValue,
    Json,
}

impl quote::ToTokens for VisitorMethod {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = match self {
            VisitorMethod::I16 => "visit_i16",
            VisitorMethod::I32 => "visit_i32",
            VisitorMethod::I64 => "visit_i64",
            VisitorMethod::F32 => "visit_f32",
            VisitorMethod::F64 => "visit_f64",
            VisitorMethod::Bool => "visit_bool",
            VisitorMethod::Text => "visit_text",
            VisitorMethod::Uuid => "visit_uuid",
            VisitorMethod::SqlxTimestamp => "visit_sqlx_timestamp",
            VisitorMethod::Bytes => "visit_bytes",
            VisitorMethod::JsonValue => "visit_json_value",
            VisitorMethod::Json => "visit_json",
        };
        let ident = syn::Ident::new(ident, proc_macro2::Span::call_site());
        ident.to_tokens(tokens);
    }
}

fn visitor_method_for_type(ty: &Type) -> syn::Result<Option<VisitorMethod>> {
    match type_name(ty).as_deref() {
        Some("i16") => Ok(Some(VisitorMethod::I16)),
        Some("i32") => Ok(Some(VisitorMethod::I32)),
        Some("i64") => Ok(Some(VisitorMethod::I64)),
        Some("f32") => Ok(Some(VisitorMethod::F32)),
        Some("f64") => Ok(Some(VisitorMethod::F64)),
        Some("bool") => Ok(Some(VisitorMethod::Bool)),
        Some("String") | Some("str") => Ok(Some(VisitorMethod::Text)),
        Some("Uuid") => Ok(Some(VisitorMethod::Uuid)),
        Some("SqlxTimestamp") => Ok(Some(VisitorMethod::SqlxTimestamp)),
        Some("Timestamp") => Err(syn::Error::new_spanned(
            ty,
            "OrsxFlatten uses SQL types; use `orsx::SqlxTimestamp` (not `orsx::Timestamp`) for timestamptz fields",
        )),
        Some("Vec") => match vec_inner_name(ty).as_deref() {
            Some("u8") => Ok(Some(VisitorMethod::Bytes)),
            _ => Ok(None),
        },
        Some("JsonValue") => Ok(Some(VisitorMethod::JsonValue)),
        Some("Json") => Ok(Some(VisitorMethod::Json)),
        _ => Ok(None),
    }
}

fn rust_type_to_field_type_strict_for_flatten(
    ty: &Type,
) -> syn::Result<(String, proc_macro2::TokenStream)> {
    match type_name(ty).as_deref() {
        Some("String") | Some("str") => Ok(("Text".to_string(), quote! { orsx::FieldType::Text })),
        Some("Json") | Some("JsonValue") => Ok(("Jsonb".to_string(), quote! { orsx::FieldType::Jsonb })),
        Some("Uuid") => Ok(("Uuid".to_string(), quote! { orsx::FieldType::Uuid })),
        Some("i16") => Ok(("Integer".to_string(), quote! { orsx::FieldType::Integer })),
        Some("i32") => Ok(("Integer".to_string(), quote! { orsx::FieldType::Integer })),
        Some("i64") => Ok(("BigInt".to_string(), quote! { orsx::FieldType::BigInt })),
        Some("f32") => Ok(("Real".to_string(), quote! { orsx::FieldType::Real })),
        Some("f64") => Ok(("DoublePrecision".to_string(), quote! { orsx::FieldType::DoublePrecision })),
        Some("bool") => Ok(("Boolean".to_string(), quote! { orsx::FieldType::Boolean })),
        Some("SqlxTimestamp") => Ok(("TimestampTz".to_string(), quote! { orsx::FieldType::TimestampTz })),
        Some("Timestamp") => Err(syn::Error::new_spanned(
            ty,
            "OrsxFlatten uses SQL types; use `orsx::SqlxTimestamp` (not `orsx::Timestamp`) for timestamptz fields",
        )),
        Some("Vec") => match vec_inner_name(ty).as_deref() {
            Some("u8") => Ok(("Bytea".to_string(), quote! { orsx::FieldType::Bytea })),
            _ => Err(syn::Error::new_spanned(
                ty,
                "OrsxFlatten leaf Vec<T> is unsupported unless T=u8; wrap structured payloads as sqlx::types::Json<T> or skip",
            )),
        },
        _ => Err(syn::Error::new_spanned(
            ty,
            "unsupported OrsxFlatten leaf type; use a supported scalar, Vec<u8>, or sqlx::types::Json<T>, or mark the field #[orsx_column(skip)]",
        )),
    }
}

fn rust_type_to_field_type(ty: &Type) -> proc_macro2::TokenStream {
    match type_name(ty).as_deref() {
        Some("String") | Some("str") => quote! { orsx::FieldType::Text },
        // SQLx JSON wrappers: store as JSONB.
        Some("Json") | Some("JsonValue") => quote! { orsx::FieldType::Jsonb },
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
        // JSONB/JSON: require SQLx json feature and use `sqlx::types::Json*` types in Rust.
        "Json" | "JsonValue" => quote! { orsx::columnar::ColumnarType::JsonbText },
        "bool" => quote! { orsx::columnar::ColumnarType::Bool },
        "i16" => quote! { orsx::columnar::ColumnarType::I16 },
        "i32" => quote! { orsx::columnar::ColumnarType::I32 },
        "i64" => quote! { orsx::columnar::ColumnarType::I64 },
        "f32" => quote! { orsx::columnar::ColumnarType::F32 },
        "f64" => quote! { orsx::columnar::ColumnarType::F64 },
        // Support both `uuid::Uuid` and `sqlx::types::Uuid` (last segment is still `Uuid`).
        "Uuid" => quote! { orsx::columnar::ColumnarType::Uuid },
        // ORSX uses `SqlxTimestamp` for timestamptz binding.
        "SqlxTimestamp" => quote! { orsx::columnar::ColumnarType::TimestampTzMicros },
        "Timestamp" => {
            return Err(syn::Error::new_spanned(
                ty,
                "OrsxFlatten uses SQL types; use `orsx::SqlxTimestamp` (not `orsx::Timestamp`) for timestamptz fields",
            ));
        }
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
