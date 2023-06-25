use std::collections::{btree_map, BTreeMap};

use convert_case::{Case, Casing};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, Error, Ident, LitByteStr, Path, Type};

use crate::{
    field_parsing::make_fields_parsing_code,
    lua_type::LuaType,
    type_data::{EnumData, StructData},
    where_clause::add_where_clauses,
    DEFAULT_TAG_NAME,
};

pub(crate) fn derive_for_struct(struct_data: StructData) -> TokenStream {
    let StructData {
        options: struct_options,
        name,
        generics,
        fields,
    } = struct_data;

    let context_type_param = struct_options
        .type_options
        .from_lua_context
        .is_none()
        .then(|| parse_quote!(__C));

    let mut impl_generics = generics.clone();
    impl_generics.params.push(parse_quote!('lua));

    if let Some(parm) = context_type_param {
        impl_generics.params.push(parm);
    }

    {
        let where_clause = impl_generics.make_where_clause();

        add_where_clauses(
            where_clause,
            &struct_options.type_options,
            &name,
            &generics,
            struct_options.type_options.from_lua_context.as_ref(),
            fields.fields().map(|field| (&field.ty, &field.options)),
        );
    }

    let function_body = make_fields_parsing_code(
        Path::from(Ident::new("Self", Span::call_site())),
        fields,
        LuaType::Table,
        false,
    );
    let where_clause = impl_generics.where_clause.take();
    let (_, generics, _) = generics.split_for_impl();
    let context_type = struct_options
        .type_options
        .from_lua_context
        .unwrap_or_else(|| parse_quote!(__C));

    quote! {
        impl #impl_generics typed_nodes::mlua::FromLua<'lua, #context_type> for #name #generics #where_clause {
            fn from_lua(value: mlua::Value<'lua>, context: &mut #context_type) -> Result<Self, <#context_type as typed_nodes::mlua::FromLuaContext<'lua>>::Error> {
                use typed_nodes::mlua::Error as _;

                typed_nodes::mlua::VisitTable::visit(value, context, |value, context|{
                    #function_body
                })
            }
        }
    }
}

pub(crate) fn derive_for_enum(enum_data: EnumData) -> TokenStream {
    let EnumData {
        options: enum_options,
        name,
        generics,
        variants,
    } = enum_data;

    let context_type_param = enum_options
        .type_options
        .from_lua_context
        .is_none()
        .then(|| parse_quote!(__C));

    let mut impl_generics = generics.clone();
    impl_generics.params.push(parse_quote!('lua));

    if let Some(parm) = context_type_param {
        impl_generics.params.push(parm);
    }

    {
        let where_clause = impl_generics.make_where_clause();
        let fields = variants
            .iter()
            .filter(|variant| !variant.options.skip)
            .flat_map(|variant| variant.fields.fields())
            .map(|field| (&field.ty, &field.options));

        add_where_clauses(
            where_clause,
            &enum_options.type_options,
            &name,
            &generics,
            enum_options.type_options.from_lua_context.as_ref(),
            fields,
        );
    }

    let mut variant_names_bytes = Vec::with_capacity(variants.len());
    let mut variant_names_str = Vec::with_capacity(variants.len());
    let mut variant_bodies = Vec::with_capacity(variants.len());
    let mut untagged_bodies = BTreeMap::new();
    let mut default_body = None;
    let mut all_are_empty = true;

    for variant in variants {
        let variant_options = variant.options;

        if variant_options.skip {
            continue;
        }

        let snake_case_name = variant.name.to_string().to_case(Case::Snake);

        all_are_empty &= variant.fields.is_empty();

        let variant_name_span = variant.name.span();
        let mut self_path = Path::from(name.clone());
        self_path.segments.push(variant.name.into());

        if variant_options.default {
            if default_body.is_some() {
                return Error::new(variant_name_span, format!("more than one default variant"))
                    .into_compile_error();
            }

            default_body = Some(make_fields_parsing_code(
                self_path.clone(),
                variant.fields,
                LuaType::Table,
                true,
            ))
        } else if variant_options.untagged_as.is_empty() {
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
    let context_type = enum_options
        .type_options
        .from_lua_context
        .unwrap_or_else(|| parse_quote!(__C));

    let table_visitor = make_enum_table_visitor_fn(
        enum_options.tag_name.as_deref().unwrap_or(DEFAULT_TAG_NAME),
        &variant_bodies,
        &variant_names_bytes,
        &variant_names_str,
        untagged_bodies.remove(&LuaType::Table),
        default_body,
        &context_type,
    );
    let string_visitor = make_enum_string_visitor_fn(
        &variant_bodies,
        &variant_names_bytes,
        &variant_names_str,
        untagged_bodies.remove(&LuaType::String),
        all_are_empty,
        &context_type,
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
        .map(|(lua_type, body)| lua_type.make_delegating_visitor_fn(&context_type, &body));

    quote! {
        impl #impl_generics typed_nodes::mlua::FromLua<'lua, #context_type> for #name #generics #where_clause {
            fn from_lua(value: mlua::Value<'lua>, context: &mut #context_type) -> Result<Self, <#context_type as typed_nodes::mlua::FromLuaContext<'lua>>::Error> {
                use typed_nodes::mlua::Error as _;

                struct __Visitor #visitor_generics (std::marker::PhantomData<fn() -> #name #generics>);

                impl #impl_generics typed_nodes::mlua::VisitLua<'lua, #context_type> for __Visitor #generics #where_clause {
                    type Output = #name #generics;

                    fn expected(&self) -> String {
                        #expected.into()
                    }

                    #table_visitor

                    #string_visitor

                    #(#untagged_visitors)*
                }

                typed_nodes::mlua::VisitLua::visit_lua(&mut __Visitor(std::marker::PhantomData), value, context)
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
    default_body: Option<TokenStream>,
    context_type: &Type,
) -> Option<TokenStream> {
    if !variant_bodies.is_empty() {
        let untagged_arm = if let Some(body) = untagged_body {
            Some(quote!(None => #body,))
        } else {
            None
        };

        let default_body = if let Some(body) = default_body {
            body
        } else {
            quote!(Err(typed_nodes::mlua::Error::invalid_variant(
                variant.as_ref().map(mlua::String::to_string_lossy).as_deref().unwrap_or("<nil>"),
                &[#(#variant_names_str),*]
            )))
        };

        Some(quote! {
            fn visit_table(&mut self, value: mlua::Table<'lua>, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::mlua::FromLuaContext<'lua>>::Error> {
                let variant = value.get::<_, Option<mlua::String>>(#tag_name)?;
                match variant.as_ref().map(mlua::String::as_bytes) {
                    #(Some(#variant_names_bytes) => {#variant_bodies},)*
                    #untagged_arm
                    _ => #default_body,
                }
            }
        })
    } else if let Some(body) = untagged_body {
        Some(LuaType::Table.make_delegating_visitor_fn(context_type, &body))
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
    context_type: &Type,
) -> Option<TokenStream> {
    if all_are_empty {
        let default_string_body = if let Some(body) = untagged_body {
            body
        } else {
            quote!(Err(typed_nodes::mlua::Error::invalid_variant(&*value.to_string_lossy(), &[#(#variant_names_str),*])))
        };

        Some(quote! {
            fn visit_string(&mut self, value: mlua::String<'lua>, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::mlua::FromLuaContext<'lua>>::Error> {
                match value.as_bytes() {
                    #(#variant_names_bytes => {#variant_bodies},)*
                    _ => #default_string_body,
                }
            }
        })
    } else if let Some(body) = untagged_body {
        Some(LuaType::String.make_delegating_visitor_fn(context_type, &body))
    } else {
        None
    }
}
