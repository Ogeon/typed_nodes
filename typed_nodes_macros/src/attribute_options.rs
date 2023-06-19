use std::collections::BTreeSet;

use proc_macro2::Ident;
use syn::{punctuated::Punctuated, Attribute, Error, Expr, Meta, MetaNameValue, Path, Token, Type};

use crate::LuaType;

#[derive(Default)]
pub(crate) struct TypeOptions {
    pub(crate) is_node: bool,
    pub(crate) from_lua_context: Option<Type>,
}

impl TypeOptions {
    fn parse_attribute(&mut self, attribute: &Attribute) -> syn::Result<bool> {
        let Meta::List(ref list) = attribute.meta else {
            return Ok(false);
        };

        if !list.path.is_ident("typed_nodes") {
            return Ok(false);
        }

        let option: Meta = list.parse_args()?;
        match option.path().get_ident().map(Ident::to_string).as_deref() {
            Some("is_node") => {
                self.is_node = true;
                Ok(true)
            }
            Some("from_lua_context") => {
                if self.from_lua_context.is_some() {
                    return Err(Error::new_spanned(
                        option,
                        "multiple `from_lua_context` attributes",
                    ));
                }

                let Meta::List(list) = &option else {
                    return Err(
                        Error::new_spanned(option, "expected `from_lua_context(MyContext<'lua>)`")
                    );
                };

                self.from_lua_context = Some(list.parse_args()?);

                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[derive(Default)]
pub(crate) struct StructOptions {
    pub(crate) type_options: TypeOptions,
}

impl StructOptions {
    pub(crate) fn from_attributes(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut options = Self::default();

        for attribute in attrs {
            if options.type_options.parse_attribute(attribute)? {
                continue;
            }

            if let Meta::List(ref list) = attribute.meta {
                if !list.path.is_ident("typed_nodes") {
                    continue;
                }

                let option: Meta = list.parse_args()?;
                match option.path().get_ident().map(Ident::to_string).as_deref() {
                    _ => return Err(Error::new_spanned(option, "unexpected struct attribute")),
                }
            }
        }

        Ok(options)
    }
}

#[derive(Default)]
pub(crate) struct EnumOptions {
    pub(crate) type_options: TypeOptions,
    pub(crate) tag_name: Option<String>,
}

impl EnumOptions {
    pub(crate) fn from_attributes(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut options = Self::default();

        for attribute in attrs {
            if options.type_options.parse_attribute(attribute)? {
                continue;
            }

            if let Meta::List(ref list) = attribute.meta {
                if !list.path.is_ident("typed_nodes") {
                    continue;
                }

                let option: Meta = list.parse_args()?;
                match option.path().get_ident().map(Ident::to_string).as_deref() {
                    Some("tag") => {
                        if options.tag_name.is_some() {
                            return Err(Error::new_spanned(option, "multiple `tag` attributes"));
                        }

                        let Meta::NameValue(MetaNameValue{value: Expr::Path(path), ..}) = &option else {
                            return Err(
                                Error::new_spanned(option, "expected `tag = property_name`")
                            );
                        };

                        let Some(ident) = path.path.get_ident() else {
                            return Err(
                                Error::new_spanned(option, "expected `tag = property_name`")
                            );
                        };

                        options.tag_name = Some(ident.to_string());
                    }
                    _ => return Err(Error::new_spanned(option, "unexpected enum attribute")),
                }
            }
        }

        Ok(options)
    }
}

#[derive(Default)]
pub(crate) struct VariantOptions {
    pub(crate) untagged_as: BTreeSet<LuaType>,
    pub(crate) default: bool,
    pub(crate) skip: bool,
}

impl VariantOptions {
    pub(crate) fn from_attributes(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut options = Self::default();

        for attribute in attrs {
            if let Meta::List(ref list) = attribute.meta {
                if !list.path.is_ident("typed_nodes") {
                    continue;
                }

                let option: Meta = list.parse_args()?;
                match option.path().get_ident().map(Ident::to_string).as_deref() {
                    Some("untagged") => {
                        let Meta::List(list) = option else {
                            return Err(Error::new_spanned(
                                option,
                                "expected a list of lua type names, such as `untagged(number, integer)`",
                            ));
                        };

                        options.untagged_as.extend(
                            list.parse_args_with(
                                Punctuated::<LuaType, Token![,]>::parse_terminated,
                            )?,
                        );
                    }
                    Some("skip") => options.skip = true,
                    Some("default") => options.default = true,
                    _ => return Err(Error::new_spanned(option, "unexpected variant attribute")),
                }
            }
        }

        Ok(options)
    }
}

#[derive(Default)]
pub(crate) struct FieldOptions {
    pub(crate) flatten: bool,
    pub(crate) is_recursive: bool,
    pub(crate) parse_with: Option<Path>,
    pub(crate) is_optional: bool,
}

impl FieldOptions {
    pub(crate) fn from_attributes(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut options = Self::default();

        for attribute in attrs {
            if let Meta::List(ref list) = attribute.meta {
                if !list.path.is_ident("typed_nodes") {
                    continue;
                }

                let option: Meta = list.parse_args()?;
                match option.path().get_ident().map(Ident::to_string).as_deref() {
                    Some("flatten") => {
                        options.flatten = true;
                    }
                    Some("recursive") => {
                        options.is_recursive = true;
                    }
                    Some("parse_with") => {
                        if options.parse_with.is_some() {
                            return Err(Error::new_spanned(
                                option,
                                "multiple `parse_with` attributes",
                            ));
                        }

                        let Meta::NameValue(MetaNameValue{value: Expr::Path(path), ..}) = option else {
                            return Err(
                                Error::new_spanned(option, "expected `parse_with = path::to::function`")
                            );
                        };

                        options.parse_with = Some(path.path);
                    }
                    Some("optional") => {
                        options.is_optional = true;
                    }
                    _ => {
                        return Err(Error::new_spanned(option, "unexpected field attribute"));
                    }
                }
            }
        }

        Ok(options)
    }
}
