use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Path;

use crate::{
    iter_ext::IterExt as _,
    lua_type::LuaType,
    type_data::{Field, Fields},
};

pub(crate) fn make_fields_parsing_code(
    self_path: Path,
    fields: Fields,
    lua_type: LuaType,
    always_flatten: bool,
) -> TokenStream {
    match fields {
        Fields::Named { fields } => {
            make_named_fields_parsing_code(self_path, fields, lua_type, always_flatten)
        }
        Fields::Unnamed { fields } => {
            make_unnamed_fields_parsing_code(self_path, fields, lua_type, always_flatten)
        }
        Fields::Unit => quote!(Ok(#self_path)),
    }
}

fn make_named_fields_parsing_code(
    self_path: Path,
    fields: Vec<(Ident, Field)>,
    lua_type: LuaType,
    always_flatten: bool,
) -> TokenStream {
    let mut field_names = Vec::with_capacity(fields.len());
    let mut parse_exprs = Vec::with_capacity(fields.len());

    for (is_last, (ident, field)) in fields.into_iter().with_is_last() {
        let field_options = field.options;
        let lua_name = ident.to_string();

        let get_from_lua = if always_flatten || field_options.flatten {
            lua_type.wrap_value_expression(!is_last)
        } else {
            quote!(value.get(#lua_name)?)
        };

        let expr = if let Some(parse_fn) = field_options.parse_with {
            quote!(#parse_fn(#get_from_lua, context))
        } else {
            quote!(typed_nodes::mlua::FromLua::from_lua(
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
        Ok(#self_path {
            #(#field_names: #parse_exprs,)*
        })
    }
}

fn make_unnamed_fields_parsing_code(
    self_path: Path,
    fields: Vec<Field>,
    lua_type: LuaType,
    always_flatten: bool,
) -> TokenStream {
    let parse_exprs =
        fields
            .into_iter()
            .enumerate()
            .with_is_last()
            .map(|(is_last, (index, field))| {
                let index = index + 1;

                let field_options = field.options;

                let get_from_lua = if always_flatten || field_options.flatten {
                    lua_type.wrap_value_expression(!is_last)
                } else {
                    quote!(value.get(#index)?)
                };

                let expr = if let Some(parse_fn) = field_options.parse_with {
                    quote! {
                        #parse_fn(#get_from_lua, context)
                    }
                } else {
                    quote! {
                        typed_nodes::mlua::FromLua::from_lua(#get_from_lua, context)
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
