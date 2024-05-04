use std::{
    fmt::Display,
    sync::atomic::{AtomicI64, Ordering},
};

use mlua::Value;

pub use from_lua::*;
pub use generate_lua::*;
pub use visit_lua::*;

use crate::{bounds::Bounds, Nodes};

mod from_lua;
mod generate_lua;
mod visit_lua;

const TABLE_ID_KEY: &str = "_node_table_id";
pub static TABLE_ID_SOURCE: TableIdSource = TableIdSource::new();

pub struct Context<'lua, B: Bounds> {
    lua: &'lua mlua::Lua,
    nodes: &'lua mut Nodes<TableId, B>,
}

impl<'lua, B: Bounds> Context<'lua, B> {
    pub fn new(lua: &'lua mlua::Lua, nodes: &'lua mut Nodes<TableId, B>) -> Self {
        Self { lua, nodes }
    }
}

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TableId(mlua::Integer);

impl TableId {
    pub fn get_or_assign<'lua>(table: &mlua::Table<'lua>) -> mlua::Result<Self> {
        match table.raw_get(TABLE_ID_KEY)? {
            Value::Integer(id) => Ok(TableId(id)),
            current_id => {
                debug_assert_eq!(
                    current_id,
                    Value::Nil,
                    "the table ID should either be an integer or nil"
                );

                let id = TABLE_ID_SOURCE.next_table_id();
                table.raw_set(TABLE_ID_KEY, Value::Integer(id.0))?;
                Ok(id)
            }
        }
    }
}

pub struct TableIdSource(AtomicI64);

impl TableIdSource {
    pub const fn new() -> Self {
        Self(AtomicI64::new(0))
    }

    pub fn next_table_id(&self) -> TableId {
        TableId(self.0.fetch_add(1, Ordering::Relaxed))
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
