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

It's also possible to generate graphs from Lua values, currently using `mlua`. To make it even better, it's able to generate Lua code for building the expressions, using functions and operators.

```rust
use typed_nodes::{
    mlua::{Context, FromLua, GenerateLua, LuaModule},
    Key, Nodes,
};
use mlua::Lua;

#[derive(FromLua)]
enum UintExpression {
    #[typed_nodes(untagged(integer, number))] // Parse plain integer and number values
    #[typed_nodes(skip_method)]
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

#[derive(FromLua, GenerateLua)]
#[typed_nodes(is_node)] // Makes it usable as a node type when recursive.
#[typed_nodes(lua_metatable = "Uint")]
enum CompoundUintExpression {
    Add {
        #[typed_nodes(lua_self)] // Assigns `self` as `lhs` in Lua.
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

#[derive(FromLua, GenerateLua)]
#[typed_nodes(lua_metatable = "Bool")]
enum BoolExpression {
    // Puts the `equal` method in the `Uint` metatable.
    #[typed_nodes(lua_base_type(CompoundUintExpression))]
    Equal {
        #[typed_nodes(lua_self)]
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

let lua = Lua::new();

// This is an abstract representation of the Lua module we are about to generate.
let mut module = LuaModule::new();

// This adds both `Bool` and `Uint`, since the latter is a "base type" in the former.
BoolExpression::generate_lua(&mut module);

// Generate a global module `my_lib` that gives us a somewhat object oriented syntax.
let table = module.load_into_table(&lua).unwrap();
lua.globals().set("my_lib", table).unwrap();

let mut nodes = Nodes::new();

let value = lua.load(r#" my_lib.Uint.add(1, 2):equal(3) "#).eval().unwrap();

// Parses the expression and inserts it in `context.nodes`.
let check_equality = Key::<BoolExpression>::from_lua(value, &mut Context::new(&lua, &mut nodes)).unwrap();

assert!(nodes.get(check_equality).unwrap().evaluate(&nodes));
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
