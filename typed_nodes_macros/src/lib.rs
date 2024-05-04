use proc_macro2::Span;
use syn::{parse_macro_input, DeriveInput, Error};
use type_data::{EnumData, StructData};

mod attribute_options;
mod field_parsing;
mod from_lua;
mod generate_lua;
mod iter_ext;
mod lua_type;
mod type_data;

const DEFAULT_TAG_NAME: &str = "type";

#[proc_macro_derive(FromLua, attributes(typed_nodes))]
pub fn from_lua(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match input.data {
        syn::Data::Struct(struct_data) => {
            let struct_data =
                match StructData::new(input.attrs, input.ident, input.generics, struct_data) {
                    Ok(data) => data,
                    Err(error) => return error.into_compile_error().into(),
                };

            from_lua::derive_for_struct(struct_data).into()
        }
        syn::Data::Enum(enum_data) => {
            let enum_data = match EnumData::new(input.attrs, input.ident, input.generics, enum_data)
            {
                Ok(data) => data,
                Err(error) => return error.into_compile_error().into(),
            };

            from_lua::derive_for_enum(enum_data).into()
        }
        syn::Data::Union(_) => Error::new(Span::call_site(), "unions are not supported")
            .into_compile_error()
            .into(),
    }
}

#[proc_macro_derive(GenerateLua, attributes(typed_nodes))]
pub fn generate_lua(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match input.data {
        syn::Data::Struct(struct_data) => {
            let struct_data =
                match StructData::new(input.attrs, input.ident, input.generics, struct_data) {
                    Ok(data) => data,
                    Err(error) => return error.into_compile_error().into(),
                };

            generate_lua::derive_for_struct(struct_data).into()
        }
        syn::Data::Enum(enum_data) => {
            let enum_data = match EnumData::new(input.attrs, input.ident, input.generics, enum_data)
            {
                Ok(data) => data,
                Err(error) => return error.into_compile_error().into(),
            };

            generate_lua::derive_for_enum(enum_data).into()
        }
        syn::Data::Union(_) => Error::new(Span::call_site(), "unions are not supported")
            .into_compile_error()
            .into(),
    }
}
