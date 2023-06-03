# `typed_nodes`

This is an experimental crate for working with a node graph, where nodes can have different data types. The intended use case is something like an expression graph. It's in a very early stage, the documentation is barely there, and the API is either obtuse or lacking. Don't expect much.

**Q:** Why make this instead of using [insert library X]?
**A:** I want to experiment with the idea and see where it goes. Maybe it will end up identical to [insert library X], maybe not. Besides, most graph libraries are focused on mathematical graphs with edge weights and all, while I'm looking for more of a heterogenous linked data structure.

**Q:** Why isn't this on crates.io?
**A:** Because I'm not even sure if it's a good idea.

```rust
use typed_nodes::{Key, Nodes};

enum UintExpression {
    Constant(u32),
    Add {
        lhs: Key<UintExpression>,
        rhs: Key<UintExpression>,
    }
}

impl UintExpression {
    fn evaluate(&self, nodes: &Nodes) -> u32 {
        match *self {
            Self::Constant(value) => value,
            Self::Add{lhs, rhs} => {
                let lhs = nodes.get(lhs).unwrap().evaluate(nodes);
                let rhs = nodes.get(rhs).unwrap().evaluate(nodes);
                lhs + rhs
            }
        }
    }
}

enum BoolExpression {
    Equal {
        lhs: Key<UintExpression>,
        rhs: Key<UintExpression>,
    }
}

impl BoolExpression {
    fn evaluate(&self, nodes: &Nodes) -> bool {
        match *self {
            Self::Equal{lhs, rhs} => {
                let lhs = nodes.get(lhs).unwrap().evaluate(nodes);
                let rhs = nodes.get(rhs).unwrap().evaluate(nodes);
                lhs == rhs
            }
        }
    }
}

let mut nodes = Nodes::new();

let lhs = nodes.insert(UintExpression::Constant(1));
let rhs = nodes.insert(UintExpression::Constant(2));
let sum = nodes.insert(UintExpression::Add {lhs, rhs});
let result = nodes.insert(UintExpression::Constant(3));
let check_equality = nodes.insert(BoolExpression::Equal{lhs: sum, rhs: result});

assert!(nodes.get(check_equality).unwrap().evaluate(&nodes));
```

## How it works

Each node type is stored in a separate collection (`NodeGroup`) and each `Key<T>` is an index into each of those collections. A node can also be associated with an ID, which helps when parsing serialized graphs. The ID can be any value that implements `Hash`, and will refer to the key of a node, allowing them to be reused when the same ID is encountered multiple times.

## Lua integration

It's also possible to generate graphs from Lua values, currently using `mlua`. A future plan is that it should also generate Lua code for building the expressions, using functions and operators.

```rust
use typed_nodes::{Key, Nodes, FromLua, TableId, TableIdSource, FromLuaContext};
use mlua::Lua;

#[derive(FromLua)]
enum UintExpression {
    #[typed_nodes(untagged(integer, number))] // Parse plain integer and number values
    Constant(u32),
    #[typed_nodes(untagged(table))]
    Compound(Key<CompoundUintExpression>)
}

impl UintExpression {
    fn evaluate<I: 'static>(&self, nodes: &Nodes<I>) -> u32 {
        match *self {
            Self::Constant(value) => value,
            Self::Compound(expression) => nodes.get(expression).unwrap().evaluate(nodes),
        }
    }
}

#[derive(FromLua)]
#[typed_nodes(is_node)] // Makes it usable as a node type when recursive.
enum CompoundUintExpression {
    Add {
        #[typed_nodes(recursive)] // Breaks infinite loops in trait resolver.
        lhs: UintExpression,
        rhs: UintExpression,
    }
}

impl CompoundUintExpression {
    fn evaluate<I: 'static>(&self, nodes: &Nodes<I>) -> u32 {
        match self {
            Self::Add{lhs, rhs} => {
                let lhs = lhs.evaluate(nodes);
                let rhs = rhs.evaluate(nodes);
                lhs + rhs
            }
        }
    }
}

#[derive(FromLua)]
enum BoolExpression {
    Equal {
        lhs: UintExpression,
        rhs: UintExpression,
    }
}

impl BoolExpression {
    fn evaluate<I: 'static>(&self, nodes: &Nodes<I>) -> bool {
        match self {
            Self::Equal{lhs, rhs} => {
                let lhs = lhs.evaluate(nodes);
                let rhs = rhs.evaluate(nodes);
                lhs == rhs
            }
        }
    }
}

// Some resources for the Lua parsing.
struct Context<'lua> {
    nodes: Nodes<TableId>, // Use an ID to be able to identify Lua tables.
    lua: &'lua Lua,
    table_id_source: TableIdSource // Produces table IDs while parsing.
}

impl<'lua> typed_nodes::Context for Context<'lua> {
    type NodeId = TableId;
    type Bounds = typed_nodes::bounds::AnyBounds;

    fn get_nodes(&self) -> &Nodes<Self::NodeId, Self::Bounds> {
        &self.nodes
    }

    fn get_nodes_mut(&mut self) -> &mut Nodes<Self::NodeId, Self::Bounds> {
        &mut self.nodes
    }
}

impl<'lua> FromLuaContext<'lua> for Context<'lua> {
    type Error = mlua::Error;

    fn get_lua(&self) -> &'lua mlua::Lua {
        &self.lua
    }

    fn table_id_to_node_id(&self, id: TableId) -> Self::NodeId {
        id
    }

    fn next_table_id(&mut self) -> TableId {
        self.table_id_source.next_table_id()
    }
}

let lua = Lua::new();

let mut context = Context {
    nodes: Nodes::new(),
    lua: &lua,
    table_id_source: TableIdSource::new(),
};

let value = lua.load(r#"
{
    type = "equal",
    lhs = {
        type = "add",
        lhs = 1,
        rhs = 2
    },
    rhs = 3
}
"#).eval().unwrap();

// Parses the expression and inserts it in `context.nodes`.
let check_equality = Key::<BoolExpression>::from_lua(value, &mut context).unwrap();

assert!(context.nodes.get(check_equality).unwrap().evaluate(&context.nodes));
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
