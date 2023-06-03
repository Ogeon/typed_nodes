use std::any::Any;

use crate::node_group::{DynNodeGroup, GroupBounds, NodeGroup};

/// Makes a new type that represents a set of trait bounds.
///
/// This macro takes care of some repetitive trait implementations and makes
/// sure that `T` in [`BoundsFor<T>`] matches types that be cast to
/// [`Bounds::DynSelf`].
///
/// ```
/// use typed_nodes::{make_bounds, Nodes, DynKey};
///
/// trait MyTrait {
///     fn say_hello(&self) -> String;
/// }
///
/// struct MyNode {
///     name: String,
/// }
///
/// impl MyTrait for MyNode {
///     fn say_hello(&self) -> String {
///         format!("hello from {}", self.name)
///     }
/// }
///
/// make_bounds!(MyNodeBounds: MyTrait + 'static);
///
/// let mut nodes = Nodes::<(), MyNodeBounds>::new();
/// let alice_key: DynKey = nodes.insert(MyNode {name: "Alice".into()}).into();
/// let bob_key: DynKey = nodes.insert(MyNode {name: "Bob".into()}).into();
///
/// assert_eq!("hello from Alice", nodes.get_dyn(alice_key).unwrap().say_hello());
/// assert_eq!("hello from Bob", nodes.get_dyn(bob_key).unwrap().say_hello());
/// ```
///
/// It's also possible to put restrictions on the node groups:
///
/// ```
/// use typed_nodes::{make_bounds, Nodes, DynKey, bounds::SendSyncBounds};
///
/// trait MyTrait {
///     fn say_hello(&self) -> String;
/// }
///
/// struct MyNode {
///     name: String,
/// }
///
/// impl MyTrait for MyNode {
///     fn say_hello(&self) -> String {
///         format!("hello from {}", self.name)
///     }
/// }
///
/// make_bounds!(MyNodeBounds<GroupBounds = SendSyncBounds>: MyTrait + 'static);
///
/// let mut nodes = Nodes::<(), MyNodeBounds>::new();
/// let alice_key: DynKey = nodes.insert(MyNode {name: "Alice".into()}).into();
/// let bob_key: DynKey = nodes.insert(MyNode {name: "Bob".into()}).into();
///
/// std::thread::scope(|scope| {
///     scope.spawn(|| assert_eq!("hello from Alice", nodes.get_dyn(alice_key).unwrap().say_hello()));
///     scope.spawn(|| assert_eq!("hello from Bob", nodes.get_dyn(bob_key).unwrap().say_hello()));
/// });
/// ```
#[macro_export]
macro_rules! make_bounds {
    ($visibility:vis $name:ident : $($bounds:tt)+) => {
        make_bounds!($visibility $name<GroupBounds = $crate::bounds::AnyBounds> :  $($bounds)+);
    };

    ($visibility:vis $name:ident<GroupBounds = $group:path> : $($bounds:tt)+) => {
        $visibility enum $name {}

        impl $crate::bounds::Bounds for $name {
            type GroupBounds = $group;
            type DynSelf = dyn $($bounds)+;
        }

        impl<T> $crate::bounds::BoundsFor<T> for $name where T: $($bounds)+ {
            fn as_dyn_ref(value: &T) -> &<$name as $crate::bounds::Bounds>::DynSelf {
                value
            }

            fn as_dyn_mut(value: &mut T) -> &mut <$name as $crate::bounds::Bounds>::DynSelf {
                value
            }
        }
    };
}

pub trait Bounds: 'static {
    type GroupBounds: GroupBounds;
    type DynSelf: ?Sized;
}

pub trait BoundsFor<T>: Bounds {
    fn as_dyn_ref(value: &T) -> &Self::DynSelf;
    fn as_dyn_mut(value: &mut T) -> &mut Self::DynSelf;
}

pub trait BoundedBy<I, B: Bounds + ?Sized>:
    GroupBoundedBy<I, B::GroupBounds> + Sized + 'static
{
    fn as_dyn_ref(&self) -> &B::DynSelf;
    fn as_dyn_mut(&mut self) -> &mut B::DynSelf;
}

impl<I, B, T> BoundedBy<I, B> for T
where
    B: BoundsFor<T> + Bounds,
    T: GroupBoundedBy<I, B::GroupBounds> + 'static,
{
    fn as_dyn_ref(&self) -> &B::DynSelf {
        B::as_dyn_ref(self)
    }
    fn as_dyn_mut(&mut self) -> &mut B::DynSelf {
        B::as_dyn_mut(self)
    }
}

pub trait GroupBoundedBy<I, G: GroupBounds>: Sized {
    fn box_group<B>(group: NodeGroup<I, Self>) -> G::BoxedGroup<B>
    where
        Self: BoundedBy<I, B>,
        B: Bounds<GroupBounds = G>;
    fn downcast_group_ref<B>(group: &G::BoxedGroup<B>) -> Option<&NodeGroup<I, Self>>
    where
        Self: BoundedBy<I, B>,
        B: Bounds<GroupBounds = G>;
    fn downcast_group_mut<B>(group: &mut G::BoxedGroup<B>) -> Option<&mut NodeGroup<I, Self>>
    where
        Self: BoundedBy<I, B>,
        B: Bounds<GroupBounds = G>;
}

pub enum AnyBounds {}

impl Bounds for AnyBounds {
    type GroupBounds = Self;
    type DynSelf = dyn Any + 'static;
}

impl GroupBounds for AnyBounds {
    type BoxedGroup<B> = Box<dyn DynNodeGroup<B> + 'static> where B: Bounds<GroupBounds = Self> + 'static;
}

impl<T> BoundsFor<T> for AnyBounds
where
    T: 'static,
{
    fn as_dyn_ref(value: &T) -> &<AnyBounds as Bounds>::DynSelf {
        value
    }

    fn as_dyn_mut(value: &mut T) -> &mut <AnyBounds as Bounds>::DynSelf {
        value
    }
}

impl<I, T> GroupBoundedBy<I, AnyBounds> for T
where
    I: 'static,
    T: 'static,
{
    fn box_group<B>(group: NodeGroup<I, T>) -> <AnyBounds as GroupBounds>::BoxedGroup<B>
    where
        T: BoundedBy<I, B>,
        B: Bounds<GroupBounds = AnyBounds>,
    {
        Box::new(group)
    }

    fn downcast_group_ref<B>(
        group: &<AnyBounds as GroupBounds>::BoxedGroup<B>,
    ) -> Option<&NodeGroup<I, T>>
    where
        T: BoundedBy<I, B>,
        B: Bounds<GroupBounds = AnyBounds>,
    {
        group.downcast_ref()
    }

    fn downcast_group_mut<B>(
        group: &mut <AnyBounds as GroupBounds>::BoxedGroup<B>,
    ) -> Option<&mut NodeGroup<I, T>>
    where
        T: BoundedBy<I, B>,
        B: Bounds<GroupBounds = AnyBounds>,
    {
        group.downcast_mut()
    }
}

pub enum SendSyncBounds {}

impl Bounds for SendSyncBounds {
    type GroupBounds = Self;
    type DynSelf = dyn Any + Send + Sync + 'static;
}

impl GroupBounds for SendSyncBounds {
    type BoxedGroup<B> = Box<dyn DynNodeGroup<B> + Send + Sync + 'static> where B: Bounds<GroupBounds = Self>;
}

impl<T> BoundsFor<T> for SendSyncBounds
where
    T: Send + Sync + 'static,
{
    fn as_dyn_ref(value: &T) -> &<SendSyncBounds as Bounds>::DynSelf {
        value
    }

    fn as_dyn_mut(value: &mut T) -> &mut <SendSyncBounds as Bounds>::DynSelf {
        value
    }
}

impl<I, T> GroupBoundedBy<I, SendSyncBounds> for T
where
    I: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    fn box_group<B>(group: NodeGroup<I, T>) -> <SendSyncBounds as GroupBounds>::BoxedGroup<B>
    where
        T: BoundedBy<I, B>,
        B: Bounds<GroupBounds = SendSyncBounds>,
    {
        Box::new(group)
    }

    fn downcast_group_ref<B>(
        group: &<SendSyncBounds as GroupBounds>::BoxedGroup<B>,
    ) -> Option<&NodeGroup<I, T>>
    where
        T: BoundedBy<I, B>,
        B: Bounds<GroupBounds = SendSyncBounds>,
    {
        (&**group as &dyn DynNodeGroup<B>).downcast_ref()
    }

    fn downcast_group_mut<B>(
        group: &mut <SendSyncBounds as GroupBounds>::BoxedGroup<B>,
    ) -> Option<&mut NodeGroup<I, T>>
    where
        T: BoundedBy<I, B>,
        B: Bounds<GroupBounds = SendSyncBounds>,
    {
        (&mut **group as &mut dyn DynNodeGroup<B>).downcast_mut()
    }
}
