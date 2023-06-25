use mlua::{Table, Value};

use super::{Error, FromLuaContext};

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

/// A helper visitor for tables.
pub struct VisitTable<F>(F);

impl<F> VisitTable<F> {
    #[inline(always)]
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

    #[inline(always)]
    fn expected(&self) -> String {
        format!("a table")
    }

    #[inline(always)]
    fn visit_table(&mut self, value: Table<'lua>, context: &mut C) -> Result<T, C::Error> {
        self.0(value, context)
    }
}

/// A helper visitor for integers.
pub struct VisitInteger<F>(F);

impl<F> VisitInteger<F> {
    #[inline(always)]
    pub fn visit<'lua, T, C>(
        value: mlua::Value<'lua>,
        context: &mut C,
        visit: F,
    ) -> Result<T, C::Error>
    where
        C: FromLuaContext<'lua>,
        F: FnMut(mlua::Integer, &mut C) -> Result<T, C::Error>,
    {
        let mut visitor = Self(visit);
        visitor.visit_lua(value, context)
    }
}

impl<'lua, C, T, F> VisitLua<'lua, C> for VisitInteger<F>
where
    C: FromLuaContext<'lua>,
    F: FnMut(mlua::Integer, &mut C) -> Result<T, C::Error>,
{
    type Output = T;

    #[inline(always)]
    fn expected(&self) -> String {
        format!("an integer")
    }

    #[inline(always)]
    fn visit_integer(&mut self, value: mlua::Integer, context: &mut C) -> Result<T, C::Error> {
        self.0(value, context)
    }
}
