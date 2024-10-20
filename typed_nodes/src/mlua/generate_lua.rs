use core::str;
use std::{any::TypeId, borrow::Cow, collections::BTreeMap, io::Write};

use ahash::AHashSet;
use mlua::Table;

pub use typed_nodes_macros::GenerateLua;

pub struct TypeSignature {
    pub name: &'static str,
    pub generics: &'static [&'static TypeSignature],
}

impl TypeSignature {
    fn get_generic_key(&self) -> Cow<'static, str> {
        let name = self.name;

        if self.generics.is_empty() {
            name.into()
        } else {
            let mut key = self
                .generics
                .iter()
                .map(|generic| generic.get_generic_key())
                .fold(format!("{name}("), |mut key, generic| {
                    if !key.ends_with('(') {
                        key.push(',');
                    }
                    key.push_str(&generic);
                    key
                });

            key.push(')');
            key.into()
        }
    }

    fn get_table_path(&self) -> Cow<'static, str> {
        let name = self.name;

        if self.generics.is_empty() {
            name.into()
        } else {
            let generic_key = self.get_generic_key();
            format!("{name}.__generic_variants[\"{generic_key}\"]").into()
        }
    }
}

pub trait GenerateLua {
    const TYPE_SIGNATURE: TypeSignature;

    fn generate_lua(module: &mut LuaModule);
}

pub struct LuaModule {
    metatables: BTreeMap<&'static str, Metatable>,
    visited_types: AHashSet<TypeId>,
}

impl LuaModule {
    pub fn new() -> Self {
        Self {
            metatables: BTreeMap::new(),
            visited_types: AHashSet::new(),
        }
    }

    pub fn visit_type<T: GenerateLua + 'static>(&mut self) -> bool {
        let unvisited = self.visited_types.insert(TypeId::of::<T>());

        if unvisited {
            self.metatables
                .entry(T::TYPE_SIGNATURE.name)
                .or_insert_with(Metatable::new);
        }

        unvisited
    }

    pub fn add_method(
        &mut self,
        metatable: &'static TypeSignature,
        method_name: &'static str,
        method: Method,
    ) {
        let table = self
            .metatables
            .entry(metatable.name)
            .or_insert_with(Metatable::new);

        if metatable.generics.is_empty() {
            table.methods.insert(method_name, method);
        } else {
            let generic_key = metatable.get_generic_key();
            table
                .generic_variants
                .entry(generic_key)
                .or_insert_with(BTreeMap::new)
                .insert(method_name, method);
        }
    }

    pub fn load_into_table<'lua>(&self, lua: &'lua mlua::Lua) -> mlua::Result<Table<'lua>> {
        let mut source = Vec::new();

        for (table_name, metatable) in &self.metatables {
            writeln!(
                &mut source,
                "local {table_name} = {{__generic_key = \"{table_name}\"}}"
            )?;
            writeln!(&mut source, "{table_name}.__index = {table_name}")?;

            if !metatable.generic_variants.is_empty() {
                writeln!(&mut source, "{table_name}.__generic_variants = {{}}")?;

                for generic_key in metatable.generic_variants.keys() {
                    let table_path = format!("{table_name}.__generic_variants[\"{generic_key}\"]");

                    writeln!(
                        &mut source,
                        "{table_path} = {{__generic_key = \"{generic_key}\"}}"
                    )?;
                    writeln!(&mut source, "{table_path}.__index = {table_path}")?;
                }
            }
        }

        for (table_name, metatable) in &self.metatables {
            populate_table(&mut source, table_name, &metatable.methods)?;

            for (generic_key, variant_methods) in &metatable.generic_variants {
                let table_path = format!("{table_name}.__generic_variants[\"{generic_key}\"]");

                populate_table(&mut source, &table_path, variant_methods)?;
            }

            let meta_table_name = format!("__{table_name}Meta");
            writeln!(&mut source, "local {meta_table_name} = {{}}")?;
            writeln!(&mut source, "{meta_table_name}.__index = {meta_table_name}")?;

            let key_start = format!("{table_name}(");
            let mut call_method_info = MethodInfo::new(vec![]);
            call_method_info.variable_arguments = true;
            call_method_info.write_to(&mut source, &meta_table_name, "__call", |source| {
                writeln!(
                    source,
                    r#"
local args = {{...}}
if #args == 0 then
    return {table_name}
end

local key = {key_start:?}
for i = 1, #args do
    if i > 1 then key = key .. "," end
    key = key .. args[i].__generic_key
end
key = key .. ")"

if {table_name}.__generic_variants[key] == nil then
    error(key .. " is not a possible instance of " .. {table_name:?})
end

return {table_name}.__generic_variants[key]
"#
                )?;
                Ok(())
            })?;
            writeln!(&mut source, "setmetatable({table_name}, {meta_table_name})")?;
        }

        writeln!(&mut source, "return {{")?;
        for table_name in self.metatables.keys() {
            writeln!(&mut source, "{table_name} = {table_name},")?;
        }
        writeln!(&mut source, "}}")?;

        // println!("{}", str::from_utf8(&source).unwrap());

        let chunk = lua.load(&source);
        chunk.eval()
    }
}

fn populate_table(
    source: &mut Vec<u8>,
    table_path: &str,
    methods: &BTreeMap<&'static str, Method>,
) -> mlua::Result<()> {
    for (method_name, method) in methods {
        writeln!(source, "local __table = {table_path}")?;

        method.write_to(source, "__table", method_name)?;
    }

    Ok(())
}

pub struct Metatable {
    methods: BTreeMap<&'static str, Method>,
    generic_variants: BTreeMap<Cow<'static, str>, BTreeMap<&'static str, Method>>,
}

impl Metatable {
    pub fn new() -> Self {
        Self {
            methods: BTreeMap::new(),
            generic_variants: BTreeMap::new(),
        }
    }
}

pub struct Method {
    info: MethodInfo,
    body: Vec<LuaStatement>,
}

impl Method {
    pub fn new(arguments: Vec<&'static str>) -> Self {
        Self {
            info: MethodInfo::new(arguments),
            body: Vec::new(),
        }
    }

    pub fn new_static(arguments: Vec<&'static str>) -> Self {
        Self {
            info: MethodInfo::new_static(arguments),
            body: Vec::new(),
        }
    }

    pub fn set_variable_arguments(&mut self) {
        self.info.variable_arguments = true;
    }

    pub fn add_statement(&mut self, statement: LuaStatement) {
        self.body.push(statement);
    }

    fn write_to(
        &self,
        source: &mut Vec<u8>,
        table_path: &str,
        method_name: &str,
    ) -> mlua::Result<()> {
        self.info
            .write_to(source, table_path, method_name, |source| {
                for statement in &self.body {
                    statement.write_to(source)?;
                }

                Ok(())
            })
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
        metatable: &'static TypeSignature,
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
            } => {
                let path = metatable.get_table_path();
                write!(source, "setmetatable({variable}, {path})")?;
            }
            Self::MakeArgumentsTable => write!(source, "{{...}}")?,
        }

        Ok(())
    }
}

struct MethodInfo {
    has_self: bool,
    arguments: Vec<&'static str>,
    variable_arguments: bool,
}

impl MethodInfo {
    fn new(arguments: Vec<&'static str>) -> Self {
        Self {
            has_self: true,
            arguments,
            variable_arguments: false,
        }
    }

    fn new_static(arguments: Vec<&'static str>) -> Self {
        Self {
            has_self: false,
            arguments,
            variable_arguments: false,
        }
    }

    fn write_to<F>(
        &self,
        source: &mut Vec<u8>,
        table_path: &str,
        method_name: &str,
        write_body: F,
    ) -> mlua::Result<()>
    where
        F: FnOnce(&mut Vec<u8>) -> mlua::Result<()>,
    {
        if self.has_self {
            write!(source, "function {table_path}:{method_name}(")?;
        } else {
            write!(source, "function {table_path}.{method_name}(")?;
        }

        let mut arguments = self.arguments.join(", ");
        if self.variable_arguments {
            if !arguments.is_empty() {
                arguments += ", ";
            }

            arguments += "...";
        }

        writeln!(source, "{arguments})")?;
        write_body(source)?;
        writeln!(source, "end")?;

        Ok(())
    }
}
