use convert_case::{Case, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{parse_quote_spanned, spanned::Spanned, Type};

use crate::{
    attribute_options::TypeOptions,
    type_data::{EnumData, Fields, StructData, Variant},
    DEFAULT_TAG_NAME,
};

pub(crate) fn derive_for_struct(struct_data: StructData) -> TokenStream {
    let StructData {
        options,
        name,
        mut generics,
        fields,
    } = struct_data;

    let metatable_name = metatable_name_expr(&options.type_options, &name);
    let base_type_delegate =
        base_type_delegate_expr(options.type_options.lua_base_type.as_ref(), &mut generics);

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let new_method = method_expr(fields, None);

    quote! {
        impl #impl_generics typed_nodes::mlua::GenerateLua for #name #type_generics #where_clause {
            fn metatable_name() -> &'static str {
                #metatable_name
            }

            fn generate_lua(module: &mut typed_nodes::mlua::LuaModule) {
                #base_type_delegate;

                let new_method = #new_method;
                module.add_method(Self::metatable_name(), "new", new_method);
            }
        }
    }
}

pub(crate) fn derive_for_enum(enum_data: EnumData) -> TokenStream {
    fn include_variant(variant: &Variant) -> bool {
        !variant.options.skip && !variant.options.skip_method
    }

    let EnumData {
        options,
        name,
        mut generics,
        variants,
    } = enum_data;

    let metatable_name = metatable_name_expr(&options.type_options, &name);
    let base_type_delegates: Vec<_> =
        base_type_delegate_expr(options.type_options.lua_base_type.as_ref(), &mut generics)
            .into_iter()
            .chain(
                variants
                    .iter()
                    .filter(|&variant| include_variant(variant))
                    .filter_map(|variant| {
                        base_type_delegate_expr(
                            variant.options.lua_base_type.as_ref(),
                            &mut generics,
                        )
                    }),
            )
            .collect();

    let variant_code = variants
        .into_iter()
        .filter(include_variant)
        .map(|variant| {
            let method_name = if let Some(method_name) = variant.options.lua_method {
                method_name
            } else {
                let name_str = variant.name.to_string().to_case(Case::Snake);
                parse_quote_spanned! {variant.name.span() => #name_str}
            };

            let set_tag = if !variant.options.default && variant.options.untagged_as.is_empty() {
                let tag_name = options.tag_name.as_deref().unwrap_or(DEFAULT_TAG_NAME);
                let tag = variant.name.to_string().to_case(Case::Snake);
                Some(quote!((#tag_name, Box::new(typed_nodes::mlua::LuaExpression::String{value: #tag}))))
            } else {
                None
            };

            let get_metatable = if let Some(base) = variant.options.lua_base_type {
                quote_spanned!(base.span() => #base::metatable_name())
            } else {
                quote!(Self::metatable_name())
            };

            let method = method_expr(variant.fields, set_tag);

            quote! {
                let method = #method;
                module.add_method(#get_metatable, #method_name, method);
            }
        });

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics typed_nodes::mlua::GenerateLua for #name #type_generics #where_clause {
            fn metatable_name() -> &'static str {
                #metatable_name
            }

            fn generate_lua(module: &mut typed_nodes::mlua::LuaModule) {
                #(#base_type_delegates;)*

                #(#variant_code)*
            }
        }
    }
}

fn method_expr(fields: Fields, set_tag: Option<TokenStream>) -> TokenStream {
    match fields {
        crate::type_data::Fields::Named { fields } => {
            let method_constructor = if fields.iter().any(|(_, field)| field.options.lua_self) {
                Ident::new("new", Span::call_site())
            } else {
                Ident::new("new_static", Span::call_site())
            };
            let argument_names = fields
                .iter()
                .filter(|(_, field)| !field.options.lua_self && !field.options.lua_arguments)
                .map(|(name, _)| name.to_string());
            let variable_arguments = if fields.iter().any(|(_, field)| field.options.lua_arguments)
            {
                Some(quote!(method.set_variable_arguments()))
            } else {
                None
            };

            let lua_fields = fields
                .iter()
                .map(|(name, field)| {
                    let name = name.to_string();

                    let value = if field.options.lua_self {
                        quote!(typed_nodes::mlua::LuaExpression::Identifier { name: "self" })
                    } else if field.options.lua_arguments {
                        quote!(typed_nodes::mlua::LuaExpression::MakeArgumentsTable)
                    } else {
                        quote!(typed_nodes::mlua::LuaExpression::Identifier{name: #name})
                    };

                    quote!((#name, Box::new(#value)))
                })
                .chain(set_tag.clone());

            quote! {{
                let mut method = typed_nodes::mlua::Method::#method_constructor(
                    vec![#(#argument_names),*]
                );
                #variable_arguments;
                method.add_statement(typed_nodes::mlua::LuaStatement::Assign {
                    variable: "__self",
                    expression: typed_nodes::mlua::LuaExpression::MakeTable {
                        fields: vec![#(#lua_fields),*]
                    }
                });
                method.add_statement(typed_nodes::mlua::LuaStatement::Return{
                    expression: typed_nodes::mlua::LuaExpression::SetMetatable {
                        variable: "__self",
                        metatable: Self::metatable_name(),
                    }
                });

                method
            }}
        }
        crate::type_data::Fields::Unnamed { .. } => {
            quote! {{
                let mut method = typed_nodes::mlua::Method::new_static(vec!["items"]);
                method.add_statement(typed_nodes::mlua::LuaStatement::Return{
                    expression: typed_nodes::mlua::LuaExpression::SetMetatable {
                        variable: "items",
                        metatable: Self::metatable_name(),
                    }
                });

                method
            }}
        }
        crate::type_data::Fields::Unit => {
            quote! {{
                let mut method = typed_nodes::mlua::Method::new_static(
                    vec![]
                );
                method.add_statement(typed_nodes::mlua::LuaStatement::Assign {
                    variable: "__self",
                    expression: typed_nodes::mlua::LuaExpression::MakeTable {
                        fields: vec![]
                    }
                });
                method.add_statement(typed_nodes::mlua::LuaStatement::Return{
                    expression: typed_nodes::mlua::LuaExpression::SetMetatable {
                        variable: "__self",
                        metatable: Self::metatable_name(),
                    }
                });

                method
            }}
        }
    }
}

fn metatable_name_expr(options: &TypeOptions, name: &Ident) -> syn::Expr {
    options
        .lua_metatable
        .clone()
        .or_else(|| {
            options
                .lua_base_type
                .as_ref()
                .map(|base| parse_quote_spanned! {base.span() => #base::metatable_name()})
        })
        .unwrap_or_else(|| {
            let name_str = name.to_string();
            parse_quote_spanned! {name.span() => #name_str}
        })
}

fn base_type_delegate_expr(
    base: Option<&Type>,
    generics: &mut syn::Generics,
) -> Option<TokenStream> {
    if let Some(base) = base {
        let where_clause = generics.make_where_clause();
        where_clause
            .predicates
            .push(parse_quote_spanned! {base.span() => #base: typed_nodes::mlua::GenerateLua});

        Some(quote!(#base::generate_lua(module);))
    } else {
        None
    }
}
