use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::Display,
    hash::{BuildHasher, Hash},
};

use mlua::{Table, Value};

use crate::{BoundedBy, Key};

pub use typed_nodes_macros::FromLua;

const TABLE_ID_KEY: &str = "_node_table_id";

pub trait FromLua<'lua, C: FromLuaContext<'lua>>: Sized {
    /// Try to convert from any Lua value.
    fn from_lua(value: mlua::Value<'lua>, context: &mut C) -> Result<Self, C::Error>;
}

pub trait VisitLua<'lua, C: FromLuaContext<'lua>> {
    type Output;

    fn expected(&self) -> String;

    fn visit_nil(&mut self, _context: &mut C) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(&Value::Nil, &self.expected()))
    }

    fn visit_boolean(&mut self, value: bool, _context: &mut C) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Boolean(value),
            &self.expected(),
        ))
    }

    fn visit_light_user_data(
        &mut self,
        value: mlua::LightUserData,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::LightUserData(value),
            &self.expected(),
        ))
    }

    fn visit_integer(
        &mut self,
        value: mlua::Integer,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Integer(value),
            &self.expected(),
        ))
    }

    fn visit_number(
        &mut self,
        value: mlua::Number,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Number(value),
            &self.expected(),
        ))
    }

    fn visit_string(
        &mut self,
        value: mlua::String<'lua>,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::String(value),
            &self.expected(),
        ))
    }

    fn visit_table(
        &mut self,
        value: mlua::Table<'lua>,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Table(value),
            &self.expected(),
        ))
    }

    fn visit_function(
        &mut self,
        value: mlua::Function<'lua>,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Function(value),
            &self.expected(),
        ))
    }

    fn visit_thread(
        &mut self,
        value: mlua::Thread<'lua>,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Thread(value),
            &self.expected(),
        ))
    }

    fn visit_user_data(
        &mut self,
        value: mlua::AnyUserData<'lua>,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::UserData(value),
            &self.expected(),
        ))
    }

    fn visit_error(
        &mut self,
        value: mlua::Error,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Error(value),
            &self.expected(),
        ))
    }

    #[cfg(feature = "luau")]
    fn visit_vector(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        _context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        Err(C::Error::invalid_type(
            &Value::Vector(x, y, z),
            &self.expected(),
        ))
    }

    fn visit_lua(
        &mut self,
        value: mlua::Value<'lua>,
        context: &mut C,
    ) -> Result<Self::Output, C::Error> {
        match value {
            Value::Nil => self.visit_nil(context),
            Value::Boolean(value) => self.visit_boolean(value, context),
            Value::LightUserData(value) => self.visit_light_user_data(value, context),
            Value::Integer(value) => self.visit_integer(value, context),
            Value::Number(value) => self.visit_number(value, context),
            Value::String(value) => self.visit_string(value, context),
            Value::Table(value) => self.visit_table(value, context),
            Value::Function(value) => self.visit_function(value, context),
            Value::Thread(value) => self.visit_thread(value, context),
            Value::UserData(value) => self.visit_user_data(value, context),
            Value::Error(value) => self.visit_error(value, context),
            #[cfg(feature = "luau")]
            Value::Vector(x, y, z) => self.visit_vector(x, y, z, context),
        }
    }
}

impl<'lua, T, C> FromLua<'lua, C> for Key<T>
where
    T: FromLua<'lua, C> + BoundedBy<C::NodeId, C::Bounds>,
    C: FromLuaContext<'lua>,
{
    fn from_lua(value: mlua::Value<'lua>, context: &mut C) -> Result<Self, C::Error> {
        VisitTable::visit(value, context, |value, context| {
            let id = TableId::get_or_assign(&value, context)?;
            let id = context.table_id_to_node_id(id);

            //let id = TableId::of(&value).into();

            if let Some(key) = context.get_nodes_mut().get_key(&id) {
                return Ok(key);
            }

            // Reserve a slot in case of circular references.
            let (reserved_key, _) = context.get_nodes_mut().reserve_with_id(id);
            let node = T::from_lua(Value::Table(value), &mut *context)?;

            Ok(context.get_nodes_mut().insert_reserved(reserved_key, node))
        })
    }
}

impl<'lua, T, C> FromLua<'lua, C> for Vec<T>
where
    T: FromLua<'lua, C>,
    C: FromLuaContext<'lua>,
{
    fn from_lua(value: mlua::Value<'lua>, context: &mut C) -> Result<Self, C::Error> {
        VisitTable::visit(value, context, |value, context| {
            value
                .sequence_values()
                .map(|value| T::from_lua(value?, context))
                .collect()
        })
    }
}

impl<'lua, K, V, S, C> FromLua<'lua, C> for HashMap<K, V, S>
where
    C: FromLuaContext<'lua>,
    K: FromLua<'lua, C> + Eq + Hash,
    V: FromLua<'lua, C>,
    S: BuildHasher + Default + Send + Sync,
{
    fn from_lua(value: mlua::Value<'lua>, context: &mut C) -> Result<Self, C::Error> {
        VisitTable::visit(value, context, |value, context| {
            value
                .pairs()
                .map(|pair| {
                    let (key, value) = pair?;
                    Ok((K::from_lua(key, context)?, V::from_lua(value, context)?))
                })
                .collect()
        })
    }
}

impl<'lua, T, C> FromLua<'lua, C> for Option<T>
where
    T: FromLua<'lua, C>,
    C: FromLuaContext<'lua>,
{
    fn from_lua(value: Value<'lua>, context: &mut C) -> Result<Self, C::Error> {
        match value {
            Value::Nil => Ok(None),
            value => T::from_lua(value, context).map(Some),
        }
    }
}

impl<'a, 'lua, T, C> FromLua<'lua, C> for Cow<'a, T>
where
    T: ToOwned + ?Sized + Send + Sync,
    T::Owned: FromLua<'lua, C>,
    C: FromLuaContext<'lua>,
{
    fn from_lua(value: Value<'lua>, context: &mut C) -> Result<Self, C::Error> {
        T::Owned::from_lua(value, context).map(Cow::Owned)
    }
}

macro_rules! impl_from_lua_tuples {
    ($first:ident $(,$ty:ident)* ) => {
        impl_from_lua_tuples!($($ty),*);

        impl<'lua, $first $(,$ty)*, Context> FromLua<'lua, Context> for ($first $(,$ty)*,)
        where
            $first: FromLua<'lua, Context>,
            $(
                $ty: FromLua<'lua, Context>,
            )*
            Context: FromLuaContext<'lua>,
            Self: BoundedBy<Context::NodeId, Context::Bounds>
        {

            fn from_lua(value: mlua::Value<'lua>, context: &mut Context) -> Result<Self, Context::Error> {
                VisitTable::visit(value, context, |value, context| {
                    const EXPECTED_LENGTH: usize = {
                        // Maybe weird to be const, but it works well with the uppercase names :)
                        const $first: usize = 1;
                        $(const $ty: usize = 1;)*

                        $first $(+$ty)*
                    };

                    let mut values = value.sequence_values();
                    #[allow(unused_mut)]
                    let mut index: usize = 0;

                    Ok((
                        $first::from_lua(values.next().ok_or_else(||Context::Error::invalid_length(EXPECTED_LENGTH, index))??, context)?,
                        $({
                            index += 1;
                            $ty::from_lua(values.next().ok_or_else(||Context::Error::invalid_length(EXPECTED_LENGTH, index))??, context)?
                        },)*
                    ))
                })
            }
        }
    };

    () => {};
}

impl_from_lua_tuples!(A, B, C, D, E, F, G, H);

macro_rules! impl_from_lua_delegate {
    ($($self_ty:ty),+) => {$(
        impl<'lua, C> FromLua<'lua, C> for $self_ty
        where
            C: FromLuaContext<'lua>,
        {
            fn from_lua(value: Value<'lua>, context: &mut C) -> Result<Self, C::Error> {
                mlua::FromLua::from_lua(value, context.get_lua()).map_err(C::Error::from)
            }
        }
    )+};
}

impl_from_lua_delegate!(
    bool, String, f32, f64, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

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

pub trait Error: Sized + From<mlua::Error> {
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

pub struct VisitTable<F>(F);

impl<F> VisitTable<F> {
    pub fn visit<'lua, T, C>(
        value: mlua::Value<'lua>,
        context: &mut C,
        visit: F,
    ) -> Result<T, C::Error>
    where
        C: FromLuaContext<'lua>,
        F: FnMut(mlua::Table<'lua>, &mut C) -> Result<T, C::Error>,
    {
        let mut visitor = Self(visit);
        visitor.visit_lua(value, context)
    }
}

impl<'lua, C, T, F> VisitLua<'lua, C> for VisitTable<F>
where
    C: FromLuaContext<'lua>,
    F: FnMut(mlua::Table<'lua>, &mut C) -> Result<T, C::Error>,
{
    type Output = T;

    fn expected(&self) -> String {
        format!("a table")
    }

    fn visit_table(&mut self, value: Table<'lua>, context: &mut C) -> Result<T, C::Error> {
        self.0(value, context)
    }
}
