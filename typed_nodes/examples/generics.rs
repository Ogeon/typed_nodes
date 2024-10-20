use mlua::Lua;
use typed_nodes::{
    mlua::{Context, FromLua, GenerateLua, LuaModule, TableId},
    Key, Nodes,
};

trait Evaluate: 'static {
    type Output;

    fn evaluate(&self, nodes: &Nodes<TableId>) -> Self::Output;
}

#[derive(FromLua, GenerateLua)]
enum Uint {
    #[typed_nodes(untagged(integer, number))] // Parse plain integer and number values
    #[typed_nodes(skip_method)]
    Literal(u32),
}

impl Evaluate for Uint {
    type Output = u32;

    fn evaluate(&self, _nodes: &Nodes<TableId>) -> Self::Output {
        match self {
            Self::Literal(value) => *value,
        }
    }
}

#[derive(FromLua, GenerateLua)]
enum List<T> {
    New {},
    With {
        #[typed_nodes(lua_self)]
        list: Key<Self>,
        value: T,
    },
}

impl<T> Evaluate for List<T>
where
    T: Evaluate,
{
    type Output = Vec<T::Output>;

    fn evaluate(&self, nodes: &Nodes<TableId>) -> Self::Output {
        match self {
            Self::New {} => Vec::new(),
            Self::With { list, value } => {
                let mut list = nodes.get(*list).unwrap().evaluate(nodes);
                list.push(value.evaluate(nodes));
                list
            }
        }
    }
}

#[derive(FromLua, GenerateLua)]
enum Optional<T> {
    Some {
        value: T,
    },
    None {},
    #[typed_nodes(lua_base_type(List::<T>))]
    #[typed_nodes(lua_method = "get")]
    ListGet {
        #[typed_nodes(lua_self)]
        list: Key<List<T>>,
        index: usize,
    },
}

impl<T> Evaluate for Optional<T>
where
    T: Evaluate<Output: Clone>,
{
    type Output = Option<T::Output>;

    fn evaluate(&self, nodes: &Nodes<TableId>) -> Self::Output {
        match self {
            Self::Some { value } => Some(value.evaluate(nodes)),
            Self::None {} => None,
            Self::ListGet { list, index } => {
                let list = nodes.get(*list).unwrap().evaluate(nodes);
                list.get(*index).cloned()
            }
        }
    }
}

#[derive(FromLua, GenerateLua)]
#[typed_nodes(lua_base_type(T))]
enum Expr<T> {
    #[typed_nodes(lua_base_type(Optional::<T>))]
    UnwrapOr {
        #[typed_nodes(lua_self)]
        optional: Key<Optional<T>>,
        default: T,
    },
}

impl<T> Evaluate for Expr<T>
where
    T: Evaluate<Output: Clone>,
{
    type Output = T::Output;

    fn evaluate(&self, nodes: &Nodes<TableId>) -> Self::Output {
        match self {
            Self::UnwrapOr { optional, default } => {
                let optional = nodes.get(*optional).unwrap();
                optional
                    .evaluate(nodes)
                    .unwrap_or_else(|| default.evaluate(nodes))
            }
        }
    }
}

fn main() -> mlua::Result<()> {
    let lua = Lua::new();

    let mut module = LuaModule::new();
    Expr::<Uint>::generate_lua(&mut module);

    let table = module.load_into_table(&lua)?;
    lua.globals().set("my_lib", table)?;

    let mut nodes = Nodes::new();

    let lua_value = lua
        .load(r#"my_lib.Optional(my_lib.Uint).some(5)"#)
        .eval()
        .unwrap();
    let expr = Optional::<Uint>::from_lua(lua_value, &mut Context::new(&lua, &mut nodes))?;
    println!("{:?}", expr.evaluate(&nodes));

    let lua_value = lua
        .load(r#"my_lib.Optional(my_lib.Uint).none()"#)
        .eval()
        .unwrap();
    let expr = Optional::<Uint>::from_lua(lua_value, &mut Context::new(&lua, &mut nodes))?;
    println!("{:?}", expr.evaluate(&nodes));

    let lua_value = lua
        .load(
            r#"my_lib.List(my_lib.Uint).new()
                :with(48)
                :with(1337)
                :get(1)
                :unwrap_or(0)"#,
        )
        .eval()
        .unwrap();
    let expr = Expr::<Uint>::from_lua(lua_value, &mut Context::new(&lua, &mut nodes))?;
    println!("{:?}", expr.evaluate(&nodes));

    Ok(())
}
