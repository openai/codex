/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::mem;

use crate::allocative_trait::Allocative;
use crate::impls::common::CAPACITY_NAME;
use crate::impls::common::DATA_NAME;
use crate::impls::common::KEY_NAME;
use crate::impls::common::UNUSED_CAPACITY_NAME;
use crate::impls::common::VALUE_NAME;
use crate::key::Key;

/// Actual implementation of the visitor.
///
/// At the moment there's only one implementation, the one which generates flame graph,
/// and this trait is crate-private. This may change in the future.
pub(crate) trait VisitorImpl {
    /// Enter simple field like `u32`.
    /// All sizes are in bytes.
    fn enter_inline_impl(&mut self, name: Key, size: usize, parent: NodeKind);
    /// Enter field which points to heap-allocated unique memory (e.g. `Box<T>`).
    fn enter_unique_impl(&mut self, name: Key, size: usize, parent: NodeKind);
    /// Enter field which points to heap-allocated shared memory (e.g. `Arc<T>`).
    /// This function returns `false` if pointee already visited.
    #[must_use]
    fn enter_shared_impl(
        &mut self,
        name: Key,
        size: usize,
        ptr: *const (),
        parent: NodeKind,
    ) -> bool;

    /// Exit the field. Each `enter_` must be matched by `exit_`.
    /// `Visitor` wrapper guarantees that.
    fn exit_inline_impl(&mut self);
    fn exit_unique_impl(&mut self);
    fn exit_shared_impl(&mut self);
    // Exit "root" visitor.
    fn exit_root_impl(&mut self);
}

#[derive(Copy, Clone)]
pub(crate) enum NodeKind {
    Inline,
    Unique,
    Shared,
    Root,
}

#[must_use] // Must call `.exit()`.
pub struct Visitor<'a> {
    pub(crate) visitor: &'a mut dyn VisitorImpl,
    pub(crate) node_kind: NodeKind,
}

impl<'a> Drop for Visitor<'a> {
    fn drop(&mut self) {
        self.exit_impl();
    }
}

impl<'a> Visitor<'a> {
    pub fn enter<'b>(&'b mut self, name: Key, size: usize) -> Visitor<'b>
    where
        'a: 'b,
    {
        self.visitor.enter_inline_impl(name, size, self.node_kind);
        Visitor {
            visitor: self.visitor,
            node_kind: NodeKind::Inline,
        }
    }

    pub fn enter_unique<'b>(&'b mut self, name: Key, size: usize) -> Visitor<'b>
    where
        'a: 'b,
    {
        self.visitor.enter_unique_impl(name, size, self.node_kind);
        Visitor {
            visitor: self.visitor,
            node_kind: NodeKind::Unique,
        }
    }

    /// Enter a field containing a shared pointer.
    ///
    /// This functions does nothing and returns `None`
    /// if pointee (`ptr` argument) was previously visited.
    pub fn enter_shared<'b>(
        &'b mut self,
        name: Key,
        size: usize,
        ptr: *const (),
    ) -> Option<Visitor<'b>>
    where
        'a: 'b,
    {
        if self
            .visitor
            .enter_shared_impl(name, size, ptr, self.node_kind)
        {
            Some(Visitor {
                visitor: self.visitor,
                node_kind: NodeKind::Shared,
            })
        } else {
            None
        }
    }

    /// This function is typically called as the first function of an `Allocative`
    /// implementation to record self.
    pub fn enter_self_sized<'b, T>(&'b mut self) -> Visitor<'b>
    where
        'a: 'b,
    {
        self.enter(Key::for_type_name::<T>(), mem::size_of::<T>())
    }

    /// This function is typically called as first function of an `Allocative`
    /// implementation to record self.
    pub fn enter_self<'b, T: ?Sized>(&'b mut self, this: &T) -> Visitor<'b>
    where
        'a: 'b,
    {
        self.enter(Key::for_type_name::<T>(), mem::size_of_val(this))
    }

    /// Visit simple sized field (e.g. `u32`) without descending into children.
    pub fn visit_simple<'b>(&'b mut self, name: Key, size: usize)
    where
        'a: 'b,
    {
        self.enter(name, size).exit();
    }

    /// Visit simple sized field (e.g. `u32`) without descending into children.
    pub fn visit_simple_sized<'b, T>(&'b mut self)
    where
        'a: 'b,
    {
        self.enter_self_sized::<T>().exit();
    }

    pub fn visit_field<'b, T: Allocative + ?Sized>(&'b mut self, name: Key, field: &T)
    where
        'a: 'b,
    {
        self.visit_field_with(name, mem::size_of_val::<T>(field), |visitor| {
            field.visit(visitor);
        })
    }

    /// Similar to `visit_field` but instead of calling [`Allocative::visit`] for
    /// whichever is the field type, you can provide a custom closure to call
    /// instead.
    ///
    /// Useful if the field type does not implement [`Allocative`].
    pub fn visit_field_with<'b, 'f, F: for<'c, 'd> FnOnce(&'d mut Visitor<'c>)>(
        &'b mut self,
        name: Key,
        field_size: usize,
        visit: F,
    ) {
        let mut visitor = self.enter(name, field_size);
        visit(&mut visitor);
        visitor.exit();
    }

    pub fn visit_slice<'b, T: Allocative>(&'b mut self, slice: &[T])
    where
        'a: 'b,
    {
        self.visit_iter(slice);
    }

    pub fn visit_iter<'b, 'i, T: Allocative + 'i, I: IntoIterator<Item = &'i T>>(
        &'b mut self,
        iter: I,
    ) where
        'a: 'b,
    {
        if !mem::needs_drop::<T>() || mem::size_of::<T>() == 0 {
            // `T` has no pointers it owns.
            self.visit_simple(
                Key::for_type_name::<T>(),
                mem::size_of::<T>() * iter.into_iter().count(),
            );
        } else {
            for item in iter {
                item.visit(self);
            }
        }
    }

    pub fn visit_vec_like_body<'b, T>(&'b mut self, data: &[T], capacity: usize)
    where
        'a: 'b,
        T: Allocative,
    {
        self.visit_field_with(CAPACITY_NAME, mem::size_of::<T>() * capacity, |visitor| {
            visitor.visit_slice(data);
            visitor.visit_simple(
                UNUSED_CAPACITY_NAME,
                mem::size_of::<T>() * capacity.wrapping_sub(data.len()),
            );
        })
    }

    pub fn visit_generic_map_fields<'b, 'x, K: Allocative + 'x, V: Allocative + 'x>(
        &'b mut self,
        entries: impl IntoIterator<Item = (&'x K, &'x V)>,
    ) {
        self.visit_field_with(DATA_NAME, mem::size_of::<*const ()>(), move |visitor| {
            for (k, v) in entries {
                visitor.visit_field(KEY_NAME, k);
                visitor.visit_field(VALUE_NAME, v);
            }
        })
    }

    pub fn visit_generic_set_fields<'b, 'x, K: Allocative + 'x>(
        &'b mut self,
        entries: impl IntoIterator<Item = &'x K>,
    ) where
        'a: 'b,
    {
        self.visit_field_with(DATA_NAME, mem::size_of::<*const ()>(), |visitor| {
            for k in entries {
                visitor.visit_field(KEY_NAME, k);
            }
        })
    }

    fn exit_impl(&mut self) {
        match self.node_kind {
            NodeKind::Inline => self.visitor.exit_inline_impl(),
            NodeKind::Unique => self.visitor.exit_unique_impl(),
            NodeKind::Shared => self.visitor.exit_shared_impl(),
            NodeKind::Root => self.visitor.exit_root_impl(),
        }
    }

    pub fn exit(mut self) {
        self.exit_impl();
        // Prevent `drop`.
        mem::forget(self);
    }
}
