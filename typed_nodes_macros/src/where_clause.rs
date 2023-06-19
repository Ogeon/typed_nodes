use std::collections::{hash_map, HashMap};

use syn::{parse_quote, Generics, Ident, Type, WhereClause, WherePredicate};

use crate::attribute_options::{FieldOptions, TypeOptions};

pub(crate) fn add_where_clauses<'a>(
    where_clause: &mut WhereClause,
    type_options: &TypeOptions,
    name: &Ident,
    generics: &Generics,
    context_type: Option<&Type>,
    fields: impl Iterator<Item = (&'a Type, FieldOptions)>,
) {
    if context_type.is_none() {
        where_clause
            .predicates
            .push(parse_quote!(__C: typed_nodes::FromLuaContext<'lua>));
    }

    let context_type = context_type.cloned().unwrap_or_else(|| parse_quote!(__C));

    if type_options.is_node {
        where_clause.predicates.push(parse_quote!(
            #name #generics: typed_nodes::bounds::BoundedBy<
                <#context_type as typed_nodes::Context>::NodeId,
                <#context_type as typed_nodes::Context>::Bounds
            >
        ));
    }
    add_field_type_where_clauses(where_clause, &context_type, fields)
}

fn add_field_type_where_clauses<'a>(
    where_clause: &mut WhereClause,
    context_type: &Type,
    types: impl IntoIterator<Item = (&'a Type, FieldOptions)>,
) {
    struct TypeOptions {
        exclue: bool,
    }

    impl TypeOptions {
        fn join(&mut self, other: TypeOptions) {
            self.exclue |= other.exclue;
        }
    }

    impl From<FieldOptions> for TypeOptions {
        fn from(field_options: FieldOptions) -> Self {
            Self {
                exclue: field_options.is_recursive || field_options.parse_with.is_some(),
            }
        }
    }

    let mut type_options = HashMap::<&Type, TypeOptions>::new();

    for (field_type, options) in types {
        let options = TypeOptions::from(options);
        match type_options.entry(field_type) {
            hash_map::Entry::Occupied(mut entry) => entry.get_mut().join(options),
            hash_map::Entry::Vacant(entry) => {
                entry.insert(options);
            }
        }
    }

    where_clause
        .predicates
        .extend(type_options.into_iter().filter_map::<WherePredicate, _>(
            |(field_type, options)| {
                if options.exclue {
                    return None;
                }

                Some(parse_quote!(#field_type: typed_nodes::FromLua<'lua, #context_type>))
            },
        ));
}
