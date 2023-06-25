use std::{
    borrow::Cow,
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

use mlua::Value;

use crate::{BoundedBy, Key};

pub use typed_nodes_macros::FromLua;

use super::{Error, FromLuaContext, TableId, VisitTable};

pub trait FromLua<'lua, C: FromLuaContext<'lua>>: Sized {
    /// Try to convert from any Lua value.
    fn from_lua(value: mlua::Value<'lua>, context: &mut C) -> Result<Self, C::Error>;
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
                .enumerate()
                .map(|(index, value)| {
                    T::from_lua(value?, context).map_err(|mut error| {
                        error.add_context_index(index + 1);
                        error
                    })
                })
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
                .pairs::<mlua::Value<'lua>, _>()
                .map(|pair| {
                    let (key, value) = pair?;
                    Ok((
                        K::from_lua(key.clone(), context)?,
                        V::from_lua(value, context).map_err(|mut error| {
                            if let Ok(key) =
                                <String as mlua::FromLua>::from_lua(key.clone(), context.get_lua())
                            {
                                error.add_context_field_name(&key);
                            } else if let Ok(index) =
                                <usize as mlua::FromLua>::from_lua(key, context.get_lua())
                            {
                                error.add_context_index(index);
                            }
                            error
                        })?,
                    ))
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

                    fn add_context<T, E: Error>(index: usize, function: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
                        match function() {
                            Ok(value) => Ok(value),
                            Err(mut error) => {
                                error.add_context_index(index);
                                Err(error)
                            }
                        }
                    }

                    let mut values = value.sequence_values();
                    #[allow(unused_mut)]
                    let mut index: usize = 0;

                    Ok((
                        add_context(index + 1, || $first::from_lua(values.next().ok_or_else(||Context::Error::invalid_length(EXPECTED_LENGTH, index))??, context))?,
                        $({
                            index += 1;
                            add_context(index + 1, || $ty::from_lua(values.next().ok_or_else(||Context::Error::invalid_length(EXPECTED_LENGTH, index))??, context))?
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
