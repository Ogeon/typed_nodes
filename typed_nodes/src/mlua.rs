use std::fmt::Display;

use mlua::Value;

pub use from_lua::*;
pub use generate_lua::*;
pub use visit_lua::*;

mod from_lua;
mod generate_lua;
mod visit_lua;

const TABLE_ID_KEY: &str = "_node_table_id";

pub trait FromLuaContext<'lua>: crate::Context {
    type Error: Error;
    fn get_lua(&self) -> &'lua mlua::Lua;
    fn table_id_to_node_id(&self, id: TableId) -> Self::NodeId;
    fn next_table_id(&mut self) -> TableId;
}

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TableId(mlua::Integer);

impl TableId {
    pub fn get_or_assign<'lua, C>(
        table: &mlua::Table<'lua>,
        context: &mut C,
    ) -> Result<Self, C::Error>
    where
        C: FromLuaContext<'lua>,
    {
        match table.raw_get(TABLE_ID_KEY)? {
            Value::Integer(id) => Ok(TableId(id)),
            current_id => {
                debug_assert_eq!(
                    current_id,
                    Value::Nil,
                    "the table ID should either be an integer or nil"
                );

                let id = context.next_table_id();
                table.raw_set(TABLE_ID_KEY, Value::Integer(id.0))?;
                Ok(id)
            }
        }
    }
}

pub struct TableIdSource(mlua::Integer);

impl TableIdSource {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn next_table_id(&mut self) -> TableId {
        let id = TableId(self.0);
        self.0 = self.0.checked_add(1).expect("out of table IDs");

        id
    }
}

impl Default for TableIdSource {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Error: Sized + From<mlua::Error> + Display {
    fn custom<T>(message: T) -> Self
    where
        T: Display;

    fn invalid_length(length: usize, expected: usize) -> Self {
        Self::custom(format_args!("invalid length {length}, expected {expected}"))
    }

    fn invalid_type(value: &mlua::Value, expected: &str) -> Self {
        let name = value.type_name();
        Self::custom(format_args!("unexpected {name}, expected {expected}"))
    }

    fn invalid_variant(variant: &str, expected: &[&str]) -> Self {
        if expected.is_empty() {
            Self::custom(format_args!(
                "unexpected enumeration variant \"{variant}\", none where expected"
            ))
        } else {
            let expected = expected
                .into_iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ");

            Self::custom(format_args!(
                "unexpected enumeration variant \"{variant}\", expected one of {expected}"
            ))
        }
    }

    fn add_context_field_name(&mut self, name: &str) {
        *self = Self::custom(format_args!("in {name}, {self}"))
    }

    fn add_context_index(&mut self, index: usize) {
        *self = Self::custom(format_args!("in [{index}], {self}"))
    }
}

impl Error for Box<dyn std::error::Error> {
    fn custom<T>(message: T) -> Self
    where
        T: Display,
    {
        message.to_string().into()
    }
}

impl Error for mlua::Error {
    fn custom<T>(message: T) -> Self
    where
        T: Display,
    {
        mlua::Error::RuntimeError(message.to_string())
    }
}
