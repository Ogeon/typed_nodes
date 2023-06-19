use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, FieldsNamed, FieldsUnnamed, Path};

use crate::{attribute_options::FieldOptions, iter_ext::IterExt as _, lua_type::LuaType};

pub(crate) fn make_fields_parsing_code(
    self_path: Path,
    fields: Fields,
    lua_type: LuaType,
    always_flatten: bool,
) -> TokenStream {
    match fields {
        syn::Fields::Named(fields) => {
            make_named_fields_parsing_code(self_path, fields, lua_type, always_flatten)
        }
        syn::Fields::Unnamed(fields) => {
            make_unnamed_fields_parsing_code(self_path, fields, lua_type, always_flatten)
        }
        syn::Fields::Unit => quote!(Ok(#self_path)),
    }
}

fn make_named_fields_parsing_code(
    self_path: Path,
    fields: FieldsNamed,
    lua_type: LuaType,
    always_flatten: bool,
) -> TokenStream {
    let mut field_names = Vec::with_capacity(fields.named.len());
    let mut parse_exprs = Vec::with_capacity(fields.named.len());
    let mut errors = Vec::new();

    for (is_last, field) in fields.named.into_iter().with_is_last() {
        let field_options = match FieldOptions::from_attributes(&field.attrs) {
            Ok(options) => options,
            Err(error) => {
                errors.push(error.to_compile_error());
                FieldOptions::default()
            }
        };
        let ident = field.ident.expect("all fields should be named");
        let lua_name = ident.to_string();

        let get_from_lua = if always_flatten || field_options.flatten {
            lua_type.wrap_value_expression(!is_last)
        } else {
            quote!(value.get(#lua_name)?)
        };

        let expr = if let Some(parse_fn) = field_options.parse_with {
            quote!(#parse_fn(#get_from_lua, context))
        } else {
            quote!(typed_nodes::FromLua::from_lua(
                #get_from_lua,
                context
            ))
        };

        let expr = if field_options.flatten {
            quote!(#expr?)
        } else {
            quote!(#expr.map_err(|mut error| {error.add_context_field_name(#lua_name); error})?)
        };

        parse_exprs.push(if field_options.is_optional {
            quote!({
                let maybe_value: Option<_> = #expr;
                maybe_value.unwrap_or_else(Default::default)
            })
        } else {
            expr
        });

        field_names.push(ident);
    }

    quote! {
        #(#errors)*
        Ok(#self_path {
            #(#field_names: #parse_exprs,)*
        })
    }
}

fn make_unnamed_fields_parsing_code(
    self_path: Path,
    fields: FieldsUnnamed,
    lua_type: LuaType,
    always_flatten: bool,
) -> TokenStream {
    let parse_exprs =
        fields
            .unnamed
            .into_iter()
            .enumerate()
            .with_is_last()
            .map(|(is_last, (index, field))| {
                let index = index + 1;

                let (field_options, error) = match FieldOptions::from_attributes(&field.attrs) {
                    Ok(options) => (options, None),
                    Err(error) => (FieldOptions::default(), Some(error.to_compile_error())),
                };

                let get_from_lua = if always_flatten || field_options.flatten {
                    lua_type.wrap_value_expression(!is_last)
                } else {
                    quote!(value.get(#index)?)
                };

                let expr = if let Some(parse_fn) = field_options.parse_with {
                    quote! {
                        #error
                        #parse_fn(#get_from_lua, context)
                    }
                } else {
                    quote! {
                        #error
                        typed_nodes::FromLua::from_lua(#get_from_lua, context)
                    }
                };

                let expr = if field_options.flatten {
                    quote!(#expr?)
                } else {
                    quote!(#expr.map_err(|mut error| {error.add_context_index(#index); error})?)
                };

                if field_options.is_optional {
                    quote!({
                        let maybe_value: Option<_> = #expr;
                        maybe_value.unwrap_or_else(Default::default)
                    })
                } else {
                    expr
                }
            });

    quote! {
        Ok(#self_path (
            #(#parse_exprs,)*
        ))
    }
}
