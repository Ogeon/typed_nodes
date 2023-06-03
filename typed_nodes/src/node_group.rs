use std::{
    any::TypeId,
    borrow::Borrow,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use smallbox::{smallbox, SmallBox};

use downcast_rs::{impl_downcast, Downcast};
use slotmap::{DefaultKey, SlotMap};

use crate::{BoundedBy, Bounds};

pub struct NodeGroup<I, T> {
    nodes: SlotMap<DefaultKey, Slot<T>>,
    id_map: ahash::HashMap<I, DefaultKey>,
}

impl<I, T> NodeGroup<I, T> {
    #[inline]
    #[must_use]
    pub(crate) fn insert(&mut self, node: T) -> Key<T> {
        Key::new(self.nodes.insert(Slot::Filled(node)))
    }

    #[inline]
    pub(crate) fn insert_reserved(&mut self, key: ReservedKey<T>, node: T) -> Key<T> {
        let slot = self
            .nodes
            .get_mut(key.slot)
            .expect("reserved slot was removed");
        *slot = Slot::Filled(node);

        Key::new(key.slot)
    }

    #[inline]
    pub(crate) fn get(&self, key: Key<T>) -> Option<&T> {
        self.nodes.get(key.slot)?.as_filled()
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, key: Key<T>) -> Option<&mut T> {
        self.nodes.get_mut(key.slot)?.as_filled_mut()
    }

    #[inline]
    pub(crate) fn remove(&mut self, key: Key<T>) -> Option<T> {
        if matches!(self.nodes.get(key.slot), Some(&Slot::Reserved) | None) {
            return None;
        }

        self.id_map.retain(|_, &mut slot| slot != key.slot);
        self.nodes.remove(key.slot)?.into_filled()
    }
}

impl<I, T> NodeGroup<I, T>
where
    I: Eq + Hash,
{
    #[inline]
    pub(crate) fn insert_with_id(&mut self, id: I, node: T) -> (Key<T>, Option<Key<T>>) {
        let slot = self.nodes.insert(Slot::Filled(node));
        let old_slot = self.id_map.insert(id, slot);

        (Key::new(slot), old_slot.map(Key::new))
    }

    #[inline]
    #[must_use]
    pub(crate) fn reserve_with_id(&mut self, id: I) -> (ReservedKey<T>, Option<Key<T>>) {
        let slot = self.nodes.insert(Slot::Reserved);
        let old_slot = self.id_map.insert(id, slot);

        (ReservedKey::new(slot), old_slot.map(Key::new))
    }

    #[inline]
    pub(crate) fn get_key<J>(&self, id: &J) -> Option<Key<T>>
    where
        J: ?Sized + Hash + Eq,
        I: Borrow<J>,
    {
        self.id_map.get(id).copied().map(Key::new)
    }
}

impl<K, T> Default for NodeGroup<K, T> {
    #[inline]
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            id_map: Default::default(),
        }
    }
}

/// A unique key for accessing a node of type `T`.
pub struct Key<T> {
    slot: DefaultKey,
    node_type: PhantomData<fn(DefaultKey) -> T>,
}

impl<T> Key<T> {
    #[inline]
    fn new(slot: DefaultKey) -> Self {
        Self {
            slot,
            node_type: PhantomData,
        }
    }
}

impl<T> Hash for Key<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.slot.hash(state);
    }
}

impl<T> Ord for Key<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.slot.cmp(&other.slot)
    }
}

impl<T> PartialOrd for Key<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.slot.partial_cmp(&other.slot)
    }
}

impl<T> Eq for Key<T> {}

impl<T> PartialEq for Key<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
    }
}

impl<T> Copy for Key<T> {}

impl<T> Clone for Key<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
            node_type: PhantomData,
        }
    }
}

/// A unique key for accessing a node with a dynamic type.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynKey {
    slot: DefaultKey,
    pub(crate) node_type: TypeId,
}

impl DynKey {
    #[inline]
    pub fn new<T: 'static>(key: Key<T>) -> Self {
        Self {
            slot: key.slot,
            node_type: TypeId::of::<T>(),
        }
    }

    #[inline]
    pub fn into_static<T: 'static>(self) -> Option<Key<T>> {
        if TypeId::of::<T>() == self.node_type {
            Some(Key::new(self.slot))
        } else {
            None
        }
    }
}

impl<T: 'static> From<Key<T>> for DynKey {
    fn from(key: Key<T>) -> Self {
        Self::new(key)
    }
}

/// A unique key for accessing a reserved node slot of type `T`.
pub struct ReservedKey<T> {
    slot: DefaultKey,
    node_type: PhantomData<fn(DefaultKey) -> T>,
}

impl<T> ReservedKey<T> {
    #[inline]
    fn new(slot: DefaultKey) -> Self {
        Self {
            slot,
            node_type: PhantomData,
        }
    }
}

impl<T> Hash for ReservedKey<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.slot.hash(state);
    }
}

impl<T> Ord for ReservedKey<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.slot.cmp(&other.slot)
    }
}

impl<T> PartialOrd for ReservedKey<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.slot.partial_cmp(&other.slot)
    }
}

impl<T> Eq for ReservedKey<T> {}

impl<T> PartialEq for ReservedKey<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
    }
}

enum Slot<T> {
    Reserved,
    Filled(T),
}

impl<T> Slot<T> {
    #[inline]
    fn as_filled(&self) -> Option<&T> {
        if let Slot::Filled(value) = self {
            Some(value)
        } else {
            None
        }
    }

    #[inline]
    fn as_filled_mut(&mut self) -> Option<&mut T> {
        if let Slot::Filled(value) = self {
            Some(value)
        } else {
            None
        }
    }

    #[inline]
    fn into_filled(self) -> Option<T> {
        if let Slot::Filled(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

impl_downcast!(DynNodeGroup<B> where B: Bounds);
pub trait DynNodeGroup<B: Bounds>: Downcast {
    fn get_dyn(&self, key: DynKey) -> Option<&B::DynSelf>;
    fn get_dyn_mut(&mut self, key: DynKey) -> Option<&mut B::DynSelf>;
    fn iter_dyn(&self) -> IterDyn<B>;
    fn iter_dyn_mut(&mut self) -> IterDynMut<B>;
    fn nodes_dyn(&self) -> NodesDyn<B> {
        NodesDyn {
            inner: self.iter_dyn(),
        }
    }
    fn nodes_dyn_mut(&mut self) -> NodesDynMut<B> {
        NodesDynMut {
            inner: self.iter_dyn_mut(),
        }
    }
}

impl<B: Bounds> DynNodeGroup<B> for Box<dyn DynNodeGroup<B> + 'static> {
    fn get_dyn(&self, key: DynKey) -> Option<&B::DynSelf> {
        (**self).get_dyn(key)
    }

    fn get_dyn_mut(&mut self, key: DynKey) -> Option<&mut <B as Bounds>::DynSelf> {
        (**self).get_dyn_mut(key)
    }

    fn iter_dyn(&self) -> IterDyn<B> {
        (**self).iter_dyn()
    }

    fn iter_dyn_mut(&mut self) -> IterDynMut<B> {
        (**self).iter_dyn_mut()
    }
}

impl<B: Bounds> DynNodeGroup<B> for Box<dyn DynNodeGroup<B> + Send + Sync + 'static> {
    fn get_dyn(&self, key: DynKey) -> Option<&B::DynSelf> {
        (**self).get_dyn(key)
    }

    fn get_dyn_mut(&mut self, key: DynKey) -> Option<&mut <B as Bounds>::DynSelf> {
        (**self).get_dyn_mut(key)
    }

    fn iter_dyn(&self) -> IterDyn<B> {
        (**self).iter_dyn()
    }

    fn iter_dyn_mut(&mut self) -> IterDynMut<B> {
        (**self).iter_dyn_mut()
    }
}

impl<I, T, B> DynNodeGroup<B> for NodeGroup<I, T>
where
    I: 'static,
    T: BoundedBy<I, B> + 'static,
    B: Bounds,
{
    fn get_dyn(&self, key: DynKey) -> Option<&<B as Bounds>::DynSelf> {
        self.get(key.into_static()?).map(T::as_dyn_ref)
    }

    fn get_dyn_mut(&mut self, key: DynKey) -> Option<&mut <B as Bounds>::DynSelf> {
        self.get_mut(key.into_static()?).map(T::as_dyn_mut)
    }

    fn iter_dyn(&self) -> IterDyn<B> {
        IterDyn {
            inner: smallbox!(self.nodes.iter().filter_map(|(key, slot)| {
                Some((
                    DynKey {
                        slot: key,
                        node_type: TypeId::of::<T>(),
                    },
                    slot.as_filled().map(T::as_dyn_ref)?,
                ))
            })),
        }
    }

    fn iter_dyn_mut(&mut self) -> IterDynMut<B> {
        IterDynMut {
            inner: smallbox!(self.nodes.iter_mut().filter_map(|(key, slot)| {
                Some((
                    DynKey {
                        slot: key,
                        node_type: TypeId::of::<T>(),
                    },
                    slot.as_filled_mut().map(T::as_dyn_mut)?,
                ))
            })),
        }
    }
}

pub trait BoxedNodeGroup {
    fn downcast_ref<I: 'static, T: 'static>(&self) -> Option<&NodeGroup<I, T>>;
    fn downcast_mut<I: 'static, T: 'static>(&mut self) -> Option<&mut NodeGroup<I, T>>;
}

impl<B: Bounds> BoxedNodeGroup for Box<dyn DynNodeGroup<B> + 'static> {
    fn downcast_ref<I: 'static, T: 'static>(&self) -> Option<&NodeGroup<I, T>> {
        (**self).as_any().downcast_ref()
    }

    fn downcast_mut<I: 'static, T: 'static>(&mut self) -> Option<&mut NodeGroup<I, T>> {
        (**self).as_any_mut().downcast_mut()
    }
}

impl<B: Bounds> BoxedNodeGroup for Box<dyn DynNodeGroup<B> + Send + Sync + 'static> {
    fn downcast_ref<I: 'static, T: 'static>(&self) -> Option<&NodeGroup<I, T>> {
        (**self).as_any().downcast_ref()
    }

    fn downcast_mut<I: 'static, T: 'static>(&mut self) -> Option<&mut NodeGroup<I, T>> {
        (**self).as_any_mut().downcast_mut()
    }
}

pub trait GroupBounds {
    type BoxedGroup<B>: DynNodeGroup<B> + BoxedNodeGroup
    where
        B: Bounds<GroupBounds = Self>;
}

pub struct IterDyn<'a, B: Bounds> {
    inner: SmallBox<dyn Iterator<Item = (DynKey, &'a B::DynSelf)> + 'a, smallbox::space::S4>,
}

impl<'a, B: Bounds> Iterator for IterDyn<'a, B> {
    type Item = (DynKey, &'a B::DynSelf);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct IterDynMut<'a, B: Bounds> {
    inner: SmallBox<dyn Iterator<Item = (DynKey, &'a mut B::DynSelf)> + 'a, smallbox::space::S4>,
}

impl<'a, B: Bounds> Iterator for IterDynMut<'a, B> {
    type Item = (DynKey, &'a mut B::DynSelf);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct NodesDyn<'a, B: Bounds> {
    inner: IterDyn<'a, B>,
}

impl<'a, B: Bounds> Iterator for NodesDyn<'a, B> {
    type Item = &'a B::DynSelf;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, node) = self.inner.next()?;
        Some(node)
    }
}

pub struct NodesDynMut<'a, B: Bounds> {
    inner: IterDynMut<'a, B>,
}

impl<'a, B: Bounds> Iterator for NodesDynMut<'a, B> {
    type Item = &'a mut B::DynSelf;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, node) = self.inner.next()?;
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use crate::bounds::{AnyBounds, GroupBoundedBy};

    use super::NodeGroup;

    #[test]
    fn iterators_are_on_stack() {
        let mut group = String::box_group::<AnyBounds>(NodeGroup::<String, String>::default());
        assert!(!group.iter_dyn().inner.is_heap());
        assert!(!group.iter_dyn_mut().inner.is_heap());
    }
}
