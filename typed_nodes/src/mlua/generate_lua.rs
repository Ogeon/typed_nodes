use std::{collections::BTreeMap, io::Write};

use mlua::Table;

pub use typed_nodes_macros::GenerateLua;

pub trait GenerateLua {
    fn metatable_name() -> &'static str;

    fn generate_lua(module: &mut LuaModule);
}

pub struct LuaModule {
    metatables: BTreeMap<&'static str, Metatable>,
}

impl LuaModule {
    pub fn new() -> Self {
        Self {
            metatables: BTreeMap::new(),
        }
    }

    pub fn add_method(
        &mut self,
        metatable: &'static str,
        method_name: &'static str,
        method: Method,
    ) {
        self.metatables
            .entry(metatable)
            .or_insert_with(Metatable::new)
            .methods
            .insert(method_name, method);
    }

    pub fn load_into_table<'lua>(&self, lua: &'lua mlua::Lua) -> mlua::Result<Table<'lua>> {
        let mut source = Vec::new();

        for table_name in self.metatables.keys() {
            writeln!(&mut source, "local {table_name} = {{}}")?;
            writeln!(&mut source, "{table_name}.__index = {table_name}")?;
        }

        for (table_name, metatable) in &self.metatables {
            for (method_name, method) in &metatable.methods {
                if method.has_self {
                    write!(&mut source, "function {table_name}:{method_name}(")?;
                } else {
                    write!(&mut source, "function {table_name}.{method_name}(")?;
                }

                let mut arguments = method.arguments.join(", ");
                if method.variable_arguments {
                    if !arguments.is_empty() {
                        arguments += ", ";
                    }

                    arguments += "...";
                }

                writeln!(&mut source, "{arguments})")?;

                for statement in &method.body {
                    statement.write_to(&mut source)?;
                }

                writeln!(&mut source, "end")?;
            }
        }

        writeln!(&mut source, "return {{")?;
        for table_name in self.metatables.keys() {
            writeln!(&mut source, "{table_name} = {table_name},")?;
        }
        writeln!(&mut source, "}}")?;

        let chunk = lua.load(&source);
        chunk.eval()
    }
}

pub struct Metatable {
    methods: BTreeMap<&'static str, Method>,
}

impl Metatable {
    pub fn new() -> Self {
        Self {
            methods: BTreeMap::new(),
        }
    }
}

pub struct Method {
    has_self: bool,
    arguments: Vec<&'static str>,
    variable_arguments: bool,
    body: Vec<LuaStatement>,
}

impl Method {
    pub fn new(arguments: Vec<&'static str>) -> Self {
        Self {
            has_self: true,
            arguments,
            variable_arguments: false,
            body: Vec::new(),
        }
    }

    pub fn new_static(arguments: Vec<&'static str>) -> Self {
        Self {
            has_self: false,
            arguments,
            variable_arguments: false,
            body: Vec::new(),
        }
    }

    pub fn set_variable_arguments(&mut self) {
        self.variable_arguments = true;
    }

    pub fn add_statement(&mut self, statement: LuaStatement) {
        self.body.push(statement);
    }
}

pub enum LuaStatement {
    Assign {
        variable: &'static str,
        expression: LuaExpression,
    },
    Return {
        expression: LuaExpression,
    },
}
impl LuaStatement {
    fn write_to(&self, source: &mut Vec<u8>) -> std::io::Result<()> {
        match self {
            LuaStatement::Assign {
                variable,
                expression,
            } => {
                write!(source, "local {variable} = ")?;
                expression.write_to(source)?;
                writeln!(source)?;
            }
            LuaStatement::Return { expression } => {
                write!(source, "return ")?;
                expression.write_to(source)?;
                writeln!(source)?;
            }
        }

        Ok(())
    }
}

pub enum LuaExpression {
    Identifier {
        name: &'static str,
    },
    String {
        value: &'static str,
    },
    MakeTable {
        fields: Vec<(&'static str, Box<LuaExpression>)>,
    },
    SetMetatable {
        variable: &'static str,
        metatable: &'static str,
    },
    MakeArgumentsTable,
}
impl LuaExpression {
    fn write_to(&self, source: &mut Vec<u8>) -> std::io::Result<()> {
        match self {
            Self::Identifier { name } => write!(source, "{name}")?,
            Self::String { value } => write!(source, "{value:?}")?,
            Self::MakeTable { fields } => {
                write!(source, "{{")?;

                for (name, value) in fields {
                    write!(source, " {name} = ")?;
                    value.write_to(source)?;
                    write!(source, ",")?;
                }

                write!(source, " }}")?;
            }
            Self::SetMetatable {
                variable,
                metatable,
            } => write!(source, "setmetatable({variable}, {metatable})")?,
            Self::MakeArgumentsTable => write!(source, "{{...}}")?,
        }

        Ok(())
    }
}
