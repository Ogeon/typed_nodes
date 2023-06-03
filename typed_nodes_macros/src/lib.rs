use std::collections::{btree_map, BTreeMap};

use convert_case::{Case, Casing};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Attribute, DeriveInput, Error, Generics, Ident, LitByteStr,
    Path,
};

use crate::{
    attribute_options::{EnumOptions, FieldOptions, StructOptions, VariantOptions},
    field_parsing::make_fields_parsing_code,
    lua_type::LuaType,
    where_clause::add_where_clauses,
};

mod attribute_options;
mod field_parsing;
mod iter_ext;
mod lua_type;
mod where_clause;

const DEFAULT_TAG_NAME: &str = "type";

#[proc_macro_derive(FromLua, attributes(typed_nodes))]
pub fn from_lua(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match input.data {
        syn::Data::Struct(struct_data) => {
            derive_for_struct(input.attrs, input.ident, input.generics, struct_data).into()
        }
        syn::Data::Enum(enum_data) => {
            derive_for_enum(input.attrs, input.ident, input.generics, enum_data).into()
        }
        syn::Data::Union(_) => Error::new(Span::call_site(), "unions are not supported")
            .into_compile_error()
            .into(),
    }
}

fn derive_for_struct(
    attributes: Vec<Attribute>,
    name: Ident,
    generics: Generics,
    struct_data: syn::DataStruct,
) -> TokenStream {
    let struct_options = match StructOptions::from_attributes(&attributes) {
        Ok(options) => options,
        Err(error) => return error.into_compile_error(),
    };

    let mut impl_generics = generics.clone();
    impl_generics.params.push(parse_quote!('lua));
    impl_generics.params.push(parse_quote!(__C));

    {
        let where_clause = impl_generics.make_where_clause();
        let fields = struct_data.fields.iter().map(|field| {
            (
                &field.ty,
                FieldOptions::from_attributes(&field.attrs).unwrap_or_default(),
            )
        });

        add_where_clauses(
            where_clause,
            struct_options.type_options,
            &name,
            &generics,
            fields,
        );
    }

    let function_body = make_fields_parsing_code(
        Path::from(Ident::new("Self", Span::call_site())),
        struct_data.fields,
        LuaType::Table,
        false,
    );
    let where_clause = impl_generics.where_clause.take();
    let (_, generics, _) = generics.split_for_impl();
    quote! {
        impl #impl_generics typed_nodes::FromLua<'lua, __C> for #name #generics #where_clause {
            fn from_lua(value: mlua::Value<'lua>, context: &mut __C) -> Result<Self, __C::Error> {
                typed_nodes::VisitTable::visit(value, context, |value, context|{
                    #function_body
                })
            }
        }
    }
}

fn derive_for_enum(
    attributes: Vec<Attribute>,
    name: Ident,
    generics: Generics,
    enum_data: syn::DataEnum,
) -> TokenStream {
    let enum_options = match EnumOptions::from_attributes(&attributes) {
        Ok(options) => options,
        Err(error) => return error.into_compile_error(),
    };

    let mut impl_generics = generics.clone();
    impl_generics.params.push(parse_quote!('lua));
    impl_generics.params.push(parse_quote!(__C));

    {
        let where_clause = impl_generics.make_where_clause();
        let fields = enum_data
            .variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .map(|field| {
                (
                    &field.ty,
                    FieldOptions::from_attributes(&field.attrs).unwrap_or_default(),
                )
            });

        add_where_clauses(
            where_clause,
            enum_options.type_options,
            &name,
            &generics,
            fields,
        );
    }

    let mut variant_names_bytes = Vec::with_capacity(enum_data.variants.len());
    let mut variant_names_str = Vec::with_capacity(enum_data.variants.len());
    let mut variant_bodies = Vec::with_capacity(enum_data.variants.len());
    let mut untagged_bodies = BTreeMap::new();
    let mut all_are_empty = true;

    for variant in enum_data.variants {
        let variant_options = match VariantOptions::from_attributes(&variant.attrs) {
            Ok(options) => options,
            Err(error) => return error.into_compile_error(),
        };

        let snake_case_name = variant.ident.to_string().to_case(Case::Snake);

        all_are_empty &= variant.fields.is_empty();

        let variant_name_span = variant.ident.span();
        let mut self_path = Path::from(name.clone());
        self_path.segments.push(variant.ident.into());

        if variant_options.untagged_as.is_empty() {
            variant_names_bytes.push(LitByteStr::new(
                snake_case_name.as_bytes(),
                variant_name_span,
            ));
            variant_names_str.push(snake_case_name);
            variant_bodies.push(make_fields_parsing_code(
                self_path,
                variant.fields,
                LuaType::Table,
                false,
            ));
        } else if variant.fields.len() <= 1 {
            for lua_type in variant_options.untagged_as {
                if let btree_map::Entry::Vacant(entry) = untagged_bodies.entry(lua_type) {
                    entry.insert(make_fields_parsing_code(
                        self_path.clone(),
                        variant.fields.clone(),
                        lua_type,
                        true,
                    ));
                } else {
                    return Error::new(
                        variant_name_span,
                        format!("more than one untagged {lua_type} variant"),
                    )
                    .into_compile_error();
                }
            }
        } else {
            return Error::new(
                variant_name_span,
                "only variants with no or one field can be untagged",
            )
            .into_compile_error();
        }
    }

    let where_clause = impl_generics.where_clause.take();
    let (visitor_generics, generics, _) = generics.split_for_impl();

    let table_visitor = make_enum_table_visitor_fn(
        enum_options.tag_name.as_deref().unwrap_or(DEFAULT_TAG_NAME),
        &variant_bodies,
        &variant_names_bytes,
        &variant_names_str,
        untagged_bodies.remove(&LuaType::Table),
    );
    let string_visitor = make_enum_string_visitor_fn(
        &variant_bodies,
        &variant_names_bytes,
        &variant_names_str,
        untagged_bodies.remove(&LuaType::String),
        all_are_empty,
    );

    let mut expected_types: Vec<_> = untagged_bodies
        .keys()
        .chain(table_visitor.is_some().then(|| &LuaType::Table))
        .chain(string_visitor.is_some().then(|| &LuaType::String))
        .map(|lua_type| lua_type.to_string())
        .collect();

    let expected = match &mut *expected_types {
        [] => "no attempt to parse any value".to_owned(),
        [name] => std::mem::take(name),
        names => {
            let names = names.join(", ");
            format!("one of: {names}")
        }
    };

    let untagged_visitors = untagged_bodies
        .into_iter()
        .map(|(lua_type, body)| lua_type.make_delegating_visitor_fn(&body));

    quote! {
        impl #impl_generics typed_nodes::FromLua<'lua, __C> for #name #generics #where_clause {
            fn from_lua(value: mlua::Value<'lua>, context: &mut __C) -> Result<Self, __C::Error> {
                struct __Visitor #visitor_generics (std::marker::PhantomData<fn() -> #name #generics>);

                impl #impl_generics typed_nodes::VisitLua<'lua, __C> for __Visitor #generics #where_clause {
                    type Output = #name #generics;

                    fn expected(&self) -> String {
                        #expected.into()
                    }

                    #table_visitor

                    #string_visitor

                    #(#untagged_visitors)*
                }

                typed_nodes::VisitLua::visit_lua(&mut __Visitor(std::marker::PhantomData), value, context)
            }
        }
    }
}

fn make_enum_table_visitor_fn(
    tag_name: &str,
    variant_bodies: &[TokenStream],
    variant_names_bytes: &[LitByteStr],
    variant_names_str: &[String],
    untagged_body: Option<TokenStream>,
) -> Option<TokenStream> {
    if !variant_bodies.is_empty() {
        let default_table_body = if let Some(body) = untagged_body {
            body
        } else {
            quote!(Err(typed_nodes::Error::invalid_variant(&*variant.to_string_lossy(), &[#(#variant_names_str),*])))
        };

        Some(quote! {
            fn visit_table(&mut self, value: mlua::Table<'lua>, context: &mut __C) -> Result<Self::Output, __C::Error> {
                let variant = value.get::<_, mlua::String>(#tag_name)?;
                match variant.as_bytes() {
                    #(#variant_names_bytes => {#variant_bodies},)*
                    _ => #default_table_body,
                }
            }
        })
    } else if let Some(body) = untagged_body {
        Some(LuaType::Table.make_delegating_visitor_fn(&body))
    } else {
        None
    }
}

fn make_enum_string_visitor_fn(
    variant_bodies: &[TokenStream],
    variant_names_bytes: &[LitByteStr],
    variant_names_str: &[String],
    untagged_body: Option<TokenStream>,
    all_are_empty: bool,
) -> Option<TokenStream> {
    if all_are_empty {
        let default_string_body = if let Some(body) = untagged_body {
            body
        } else {
            quote!(Err(typed_nodes::Error::invalid_variant(&*value.to_string_lossy(), &[#(#variant_names_str),*])))
        };

        Some(quote! {
            fn visit_string(&mut self, value: mlua::String<'lua>, context: &mut __C) -> Result<Self::Output, __C::Error> {
                match value.as_bytes() {
                    #(#variant_names_bytes => {#variant_bodies},)*
                    _ => #default_string_body,
                }
            }
        })
    } else if let Some(body) = untagged_body {
        Some(LuaType::String.make_delegating_visitor_fn(&body))
    } else {
        None
    }
}
