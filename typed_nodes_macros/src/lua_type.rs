use std::fmt;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Parse, Error, Ident, Type};

macro_rules! make_lua_type {
    (
        $(#[$meta:meta])*
        $visibility:vis enum LuaType {
            $($variant:ident => $token:ident),*
            $(,)?
        }
    ) => {
        $(#[$meta])*
        $visibility enum LuaType {
            $($variant,)*
        }

        impl TryFrom<Ident> for LuaType {
            type Error = Error;

            fn try_from(ident: Ident) -> syn::Result<Self> {
                match &*ident.to_string() {
                    $(stringify!($token) => Ok(Self::$variant),)*
                    ident => {
                        let types = [$(stringify!($token)),*].join(", ");
                        return Err(Error::new_spanned(
                            ident,
                            format!("unexpected Lua value type, expected one of: {types}")
                        ))
                    },
                }
            }
        }

        impl Parse for LuaType {
            fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
                let ident: Ident = input.parse()?;
                Self::try_from(ident)
            }
        }

        impl fmt::Display for LuaType {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match *self {
                    $(Self::$variant => f.write_str(stringify!($token))),*
                }
            }
        }
    };
}

make_lua_type! {
    #[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
    pub(crate) enum LuaType {
        Nil => nil,
        Table => table,
        Number => number,
        Integer => integer,
        String => string,
        Boolean => boolean,
    }
}

impl LuaType {
    pub(crate) fn wrap_value_expression(&self, clone_value: bool) -> TokenStream {
        let value = if clone_value {
            quote!(value.clone())
        } else {
            quote!(value)
        };

        match self {
            LuaType::Nil => quote!(mlua::Value::Nil),
            LuaType::Table => quote!(mlua::Value::Table(#value)),
            LuaType::Number => quote!(mlua::Value::Number(#value)),
            LuaType::Integer => quote!(mlua::Value::Integer(#value)),
            LuaType::String => quote!(mlua::Value::String(#value)),
            LuaType::Boolean => quote!(mlua::Value::Boolean(#value)),
        }
    }

    pub(crate) fn make_delegating_visitor_fn(
        &self,
        context_type: &Type,
        body: &TokenStream,
    ) -> TokenStream {
        match self {
            LuaType::Nil => {
                quote! {
                    fn visit_nil(&mut self, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::FromLuaContext<'lua>>::Error> {
                        #body
                    }
                }
            }
            LuaType::Table => {
                quote! {
                    fn visit_table(&mut self, value: mlua::Table<'lua>, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::FromLuaContext<'lua>>::Error> {
                        #body
                    }
                }
            }
            LuaType::Number => {
                quote! {
                    fn visit_number(&mut self, value: f64, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::FromLuaContext<'lua>>::Error> {
                        #body
                    }
                }
            }
            LuaType::Integer => {
                quote! {
                    fn visit_integer(&mut self, value: i64, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::FromLuaContext<'lua>>::Error> {
                        #body
                    }
                }
            }
            LuaType::String => {
                quote! {
                    fn visit_string(&mut self, value: mlua::String<'lua>, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::FromLuaContext<'lua>>::Error> {
                        #body
                    }
                }
            }
            LuaType::Boolean => {
                quote! {
                    fn visit_boolean(&mut self, value: bool, context: &mut #context_type) -> Result<Self::Output, <#context_type as typed_nodes::FromLuaContext<'lua>>::Error> {
                        #body
                    }
                }
            }
        }
    }
}
