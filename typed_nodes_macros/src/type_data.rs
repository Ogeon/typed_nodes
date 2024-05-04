use proc_macro2::Ident;
use syn::{Attribute, Generics, Type};

use crate::attribute_options::{EnumOptions, FieldOptions, StructOptions, VariantOptions};

pub(crate) struct StructData {
    pub options: StructOptions,
    pub name: Ident,
    pub generics: Generics,
    pub fields: Fields,
}

impl StructData {
    pub fn new(
        attributes: Vec<Attribute>,
        name: Ident,
        generics: Generics,
        struct_data: syn::DataStruct,
    ) -> syn::Result<Self> {
        Ok(Self {
            options: StructOptions::from_attributes(&attributes)?,
            name,
            generics,
            fields: Fields::new(struct_data.fields)?,
        })
    }
}

pub(crate) struct EnumData {
    pub options: EnumOptions,
    pub name: Ident,
    pub generics: Generics,
    pub variants: Vec<Variant>,
}

impl EnumData {
    pub fn new(
        attributes: Vec<Attribute>,
        name: Ident,
        generics: Generics,
        enum_data: syn::DataEnum,
    ) -> syn::Result<Self> {
        Ok(Self {
            options: EnumOptions::from_attributes(&attributes)?,
            name,
            generics,
            variants: enum_data
                .variants
                .into_iter()
                .map(|variant| Variant::new(variant))
                .collect::<syn::Result<_>>()?,
        })
    }
}

pub(crate) struct Variant {
    pub options: VariantOptions,
    pub name: Ident,
    pub fields: Fields,
}

impl Variant {
    fn new(variant: syn::Variant) -> syn::Result<Self> {
        Ok(Self {
            options: VariantOptions::from_attributes(&variant.attrs)?,
            name: variant.ident,
            fields: Fields::new(variant.fields)?,
        })
    }
}

#[derive(Clone)]
pub(crate) enum Fields {
    Named { fields: Vec<(Ident, Field)> },
    Unnamed { fields: Vec<Field> },
    Unit,
}

impl Fields {
    fn new(fields: syn::Fields) -> syn::Result<Self> {
        let result = match fields {
            syn::Fields::Named(fields) => Self::Named {
                fields: fields
                    .named
                    .into_iter()
                    .map(|field| Ok((field.ident.unwrap(), Field::new(field.attrs, field.ty)?)))
                    .collect::<syn::Result<_>>()?,
            },
            syn::Fields::Unnamed(fields) => Self::Unnamed {
                fields: fields
                    .unnamed
                    .into_iter()
                    .map(|field| Field::new(field.attrs, field.ty))
                    .collect::<syn::Result<_>>()?,
            },
            syn::Fields::Unit => Self::Unit,
        };

        Ok(result)
    }

    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Fields::Named { fields } => fields.is_empty(),
            Fields::Unnamed { fields } => fields.is_empty(),
            Fields::Unit => true,
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Fields::Named { fields } => fields.len(),
            Fields::Unnamed { fields } => fields.len(),
            Fields::Unit => 0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Field {
    pub options: FieldOptions,
    pub ty: Type,
}

impl Field {
    fn new(attributes: Vec<Attribute>, ty: Type) -> syn::Result<Self> {
        Ok(Field {
            options: FieldOptions::from_attributes(&attributes)?,
            ty,
        })
    }
}
