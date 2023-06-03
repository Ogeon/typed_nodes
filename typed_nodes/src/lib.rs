use std::{any::TypeId, borrow::Borrow, collections::HashMap, hash::Hash, marker::PhantomData};

use bounds::{BoundedBy, Bounds};
use node_group::{BoxedNodeGroup, DynNodeGroup, GroupBounds, NodeGroup};
pub use node_group::{DynKey, Key, ReservedKey};
pub use parse::{Error, FromLua, FromLuaContext, TableId, TableIdSource, VisitLua, VisitTable};

pub mod bounds;
mod node_group;
mod parse;

type BoxedGroupOf<B> = <<B as Bounds>::GroupBounds as GroupBounds>::BoxedGroup<B>;

/// A set of nodes of different types.
///
/// The nodes can be inserted and found with an arbitrary ID.
pub struct Nodes<I = (), B: Bounds = bounds::AnyBounds> {
    node_groups: ahash::HashMap<TypeId, BoxedGroupOf<B>>,
    key_type: PhantomData<fn(I)>,
}

impl<I, B> Nodes<I, B>
where
    I: 'static,
    B: Bounds,
{
    #[inline]
    pub fn new() -> Self {
        Self {
            node_groups: HashMap::with_hasher(Default::default()),
            key_type: PhantomData,
        }
    }

    #[inline]
    pub fn insert<T>(&mut self, node: T) -> Key<T>
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .entry(TypeId::of::<T>())
            .or_insert_with(|| T::box_group(NodeGroup::<I, T>::default()))
            .downcast_mut::<I, T>()
            .expect("node group should be possible to downcast")
            .insert(node)
    }

    /// Insert a value in a reserved slot. Reservations can be made with [`Nodes::reserve_with_id`].
    #[inline]
    pub fn insert_reserved<T>(&mut self, key: ReservedKey<T>, node: T) -> Key<T>
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .entry(TypeId::of::<T>())
            .or_insert_with(|| T::box_group(NodeGroup::<I, T>::default()))
            .downcast_mut::<I, T>()
            .expect("node group should be possible to downcast")
            .insert_reserved(key, node)
    }

    #[inline]
    pub fn get<T>(&self, key: Key<T>) -> Option<&T>
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .get(&TypeId::of::<T>())?
            .downcast_ref::<I, T>()
            .expect("node group should be possible to downcast")
            .get(key)
    }

    #[inline]
    pub fn get_mut<T>(&mut self, key: Key<T>) -> Option<&mut T>
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .get_mut(&TypeId::of::<T>())?
            .downcast_mut::<I, T>()
            .expect("node group should be possible to downcast")
            .get_mut(key)
    }

    #[inline]
    pub fn remove<T>(&mut self, key: Key<T>) -> Option<T>
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .get_mut(&TypeId::of::<T>())?
            .downcast_mut::<I, T>()
            .expect("node group should be possible to downcast")
            .remove(key)
    }

    #[inline]
    pub fn get_dyn(&self, key: DynKey) -> Option<&B::DynSelf> {
        self.node_groups.get(&key.node_type)?.get_dyn(key)
    }

    #[inline]
    pub fn get_dyn_mut(&mut self, key: DynKey) -> Option<&mut B::DynSelf> {
        self.node_groups.get_mut(&key.node_type)?.get_dyn_mut(key)
    }

    #[inline]
    pub fn iter_dyn(&self) -> IterDyn<B> {
        IterDyn {
            inner: self
                .node_groups
                .values()
                .flat_map(DynNodeGroup::<B>::iter_dyn),
        }
    }

    #[inline]
    pub fn iter_dyn_mut(&mut self) -> IterDynMut<B> {
        IterDynMut {
            inner: self
                .node_groups
                .values_mut()
                .flat_map(DynNodeGroup::<B>::iter_dyn_mut),
        }
    }

    #[inline]
    pub fn nodes_dyn(&self) -> NodesDyn<B> {
        NodesDyn {
            inner: self
                .node_groups
                .values()
                .flat_map(DynNodeGroup::<B>::nodes_dyn),
        }
    }

    #[inline]
    pub fn nodes_dyn_mut(&mut self) -> NodesDynMut<B> {
        NodesDynMut {
            inner: self
                .node_groups
                .values_mut()
                .flat_map(DynNodeGroup::<B>::nodes_dyn_mut),
        }
    }
}

impl<I, B> Nodes<I, B>
where
    I: Hash + Eq + 'static,
    B: Bounds,
{
    /// Insert a node and assign an ID to it. The ID can be used later to find
    /// the node, but it's only unique for nodes of type `T`. Other node types
    /// can use the same ID.
    #[inline]
    pub fn insert_with_id<T>(&mut self, id: I, node: T) -> (Key<T>, Option<Key<T>>)
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .entry(TypeId::of::<T>())
            .or_insert_with(|| T::box_group(NodeGroup::<I, T>::default()))
            .downcast_mut::<I, T>()
            .expect("node group should be possible to downcast")
            .insert_with_id(id, node)
    }

    /// Reserves a node slot for `id` and node type `T` that can be filled
    /// later. The node will not be accessible but it's possible to request its
    /// key with [`Nodes::get_key`]. Reserving node slots is useful for handling
    /// circular references in the node graph.
    #[inline]
    pub fn reserve_with_id<T>(&mut self, id: I) -> (ReservedKey<T>, Option<Key<T>>)
    where
        T: BoundedBy<I, B>,
    {
        self.node_groups
            .entry(TypeId::of::<T>())
            .or_insert_with(|| T::box_group(NodeGroup::<I, T>::default()))
            .downcast_mut::<I, T>()
            .expect("node group should be possible to downcast")
            .reserve_with_id(id)
    }

    /// Find the key for `id` and node type `T`. The node may not have been
    /// inserted yet if it was reserved with [`Nodes::reserve_with_id`], so
    /// [`Nodes::get`] may still return `None`.
    #[inline]
    pub fn get_key<T, J>(&self, id: &J) -> Option<Key<T>>
    where
        T: BoundedBy<I, B>,
        J: ?Sized + Hash + Eq,
        I: Borrow<J>,
    {
        self.node_groups
            .get(&TypeId::of::<T>())?
            .downcast_ref::<I, T>()
            .expect("node group should be possible to downcast")
            .get_key(id)
    }
}

pub trait Context {
    type NodeId: PartialEq + Eq + Hash + 'static;
    type Bounds: Bounds;

    fn get_nodes(&self) -> &Nodes<Self::NodeId, Self::Bounds>;
    fn get_nodes_mut(&mut self) -> &mut Nodes<Self::NodeId, Self::Bounds>;
}

pub struct IterDyn<'a, B: Bounds> {
    inner: std::iter::FlatMap<
        std::collections::hash_map::Values<'a, TypeId, BoxedGroupOf<B>>,
        node_group::IterDyn<'a, B>,
        fn(&BoxedGroupOf<B>) -> node_group::IterDyn<B>,
    >,
}

impl<'a, B: Bounds> Iterator for IterDyn<'a, B> {
    type Item = (DynKey, &'a B::DynSelf);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct IterDynMut<'a, B: Bounds> {
    inner: std::iter::FlatMap<
        std::collections::hash_map::ValuesMut<'a, TypeId, BoxedGroupOf<B>>,
        node_group::IterDynMut<'a, B>,
        fn(&mut BoxedGroupOf<B>) -> node_group::IterDynMut<B>,
    >,
}

impl<'a, B: Bounds> Iterator for IterDynMut<'a, B> {
    type Item = (DynKey, &'a mut B::DynSelf);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct NodesDyn<'a, B: Bounds> {
    inner: std::iter::FlatMap<
        std::collections::hash_map::Values<'a, TypeId, BoxedGroupOf<B>>,
        node_group::NodesDyn<'a, B>,
        fn(&BoxedGroupOf<B>) -> node_group::NodesDyn<B>,
    >,
}

impl<'a, B: Bounds> Iterator for NodesDyn<'a, B> {
    type Item = &'a B::DynSelf;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct NodesDynMut<'a, B: Bounds> {
    inner: std::iter::FlatMap<
        std::collections::hash_map::ValuesMut<'a, TypeId, BoxedGroupOf<B>>,
        node_group::NodesDynMut<'a, B>,
        fn(&mut BoxedGroupOf<B>) -> node_group::NodesDynMut<B>,
    >,
}

impl<'a, B: Bounds> Iterator for NodesDynMut<'a, B> {
    type Item = &'a mut B::DynSelf;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[cfg(doctest)]
macro_rules! doctest {
    ($str: expr, $name: ident) => {
        #[doc = $str]
        mod $name {}
    };
}

// Makes doctest run tests on README.md.
#[cfg(doctest)]
doctest!(include_str!("../../README.md"), readme);
