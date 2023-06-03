use std::collections::{hash_map, HashMap};

use syn::{parse_quote, Generics, Ident, Type, WhereClause, WherePredicate};

use crate::attribute_options::{FieldOptions, TypeOptions};

pub(crate) fn add_where_clauses<'a>(
    where_clause: &mut WhereClause,
    type_options: TypeOptions,
    name: &Ident,
    generics: &Generics,
    fields: impl Iterator<Item = (&'a Type, FieldOptions)>,
) {
    where_clause
        .predicates
        .push(parse_quote!(__C: typed_nodes::FromLuaContext<'lua>));
    if type_options.is_node {
        where_clause.predicates.push(parse_quote!(
            #name #generics: typed_nodes::bounds::BoundedBy<__C::NodeId, __C::Bounds>
        ));
    }
    add_field_type_where_clauses(where_clause, fields)
}

fn add_field_type_where_clauses<'a>(
    where_clause: &mut WhereClause,
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

                Some(parse_quote!(#field_type: typed_nodes::FromLua<'lua, __C>))
            },
        ));
}
