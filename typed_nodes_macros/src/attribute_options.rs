use std::collections::BTreeSet;

use proc_macro2::Ident;
use syn::{punctuated::Punctuated, Attribute, Error, Expr, Meta, MetaNameValue, Path, Token, Type};

use crate::lua_type::LuaType;

#[derive(Default)]
pub(crate) struct TypeOptions {
    pub(crate) is_node: bool,
    pub(crate) sync: bool,
    pub(crate) lua_metatable: Option<Expr>,
    pub(crate) lua_base_type: Option<Type>,
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
            Some("sync") => {
                let Meta::Path(_) = &option else {
                    return Err(Error::new_spanned(
                        option,
                        "expected `sync` without arguments or value",
                    ));
                };

                self.sync = true;

                Ok(true)
            }
            Some("lua_metatable") => {
                if self.lua_metatable.is_some() {
                    return Err(Error::new_spanned(
                        option,
                        "multiple `lua_metatable` attributes",
                    ));
                }

                let Meta::NameValue(value) = &option else {
                    return Err(Error::new_spanned(
                        option,
                        "expected `lua_metatable = \"MyMetatable\"`",
                    ));
                };

                self.lua_metatable = Some(value.value.clone());

                Ok(true)
            }
            Some("lua_base_type") => {
                if self.lua_base_type.is_some() {
                    return Err(Error::new_spanned(
                        option,
                        "multiple `lua_base_type` attributes",
                    ));
                }

                let Meta::List(list) = &option else {
                    return Err(Error::new_spanned(
                        option,
                        "expected `lua_base_type(MyType)`",
                    ));
                };

                self.lua_base_type = Some(list.parse_args()?);

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

                        let Meta::NameValue(MetaNameValue {
                            value: Expr::Path(path),
                            ..
                        }) = &option
                        else {
                            return Err(Error::new_spanned(
                                option,
                                "expected `tag = property_name`",
                            ));
                        };

                        let Some(ident) = path.path.get_ident() else {
                            return Err(Error::new_spanned(
                                option,
                                "expected `tag = property_name`",
                            ));
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
    pub(crate) skip_method: bool,
    pub(crate) lua_base_type: Option<Type>,
    pub(crate) lua_method: Option<Expr>,
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
                    Some("skip_method") => options.skip_method = true,
                    Some("lua_base_type") => {
                        if options.lua_base_type.is_some() {
                            return Err(Error::new_spanned(
                                option,
                                "multiple `lua_base_type` attributes",
                            ));
                        }

                        let Meta::List(list) = &option else {
                            return Err(Error::new_spanned(
                                option,
                                "expected `lua_base_type(MyType)`",
                            ));
                        };

                        options.lua_base_type = Some(list.parse_args()?);
                    }
                    Some("lua_method") => {
                        if options.lua_method.is_some() {
                            return Err(Error::new_spanned(
                                option,
                                "multiple `lua_method` attributes",
                            ));
                        }

                        let Meta::NameValue(value) = &option else {
                            return Err(Error::new_spanned(
                                option,
                                "expected `lua_method = \"my_method\"`",
                            ));
                        };

                        options.lua_method = Some(value.value.clone());
                    }
                    _ => return Err(Error::new_spanned(option, "unexpected variant attribute")),
                }
            }
        }

        Ok(options)
    }
}

#[derive(Clone, Default)]
pub(crate) struct FieldOptions {
    pub(crate) flatten: bool,
    pub(crate) parse_with: Option<Path>,
    pub(crate) is_optional: bool,
    pub(crate) lua_self: bool,
    pub(crate) lua_arguments: bool,
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
                    Some("parse_with") => {
                        if options.parse_with.is_some() {
                            return Err(Error::new_spanned(
                                option,
                                "multiple `parse_with` attributes",
                            ));
                        }

                        let Meta::NameValue(MetaNameValue {
                            value: Expr::Path(path),
                            ..
                        }) = option
                        else {
                            return Err(Error::new_spanned(
                                option,
                                "expected `parse_with = path::to::function`",
                            ));
                        };

                        options.parse_with = Some(path.path);
                    }
                    Some("optional") => {
                        options.is_optional = true;
                    }
                    Some("lua_self") => options.lua_self = true,
                    Some("lua_arguments") => options.lua_arguments = true,
                    _ => {
                        return Err(Error::new_spanned(option, "unexpected field attribute"));
                    }
                }
            }
        }

        Ok(options)
    }
}
