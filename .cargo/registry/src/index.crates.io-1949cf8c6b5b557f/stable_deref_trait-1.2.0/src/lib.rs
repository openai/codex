// Copyright 2017 Robert Grosse

// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
This module defines an unsafe marker trait, StableDeref, for container types that deref to a fixed address which is valid even when the containing type is moved. For example, Box, Vec, Rc, Arc and String implement this trait. Additionally, it defines CloneStableDeref for types like Rc where clones deref to the same address.

It is intended to be used by crates such as [owning_ref](https://crates.io/crates/owning_ref) and [rental](https://crates.io/crates/rental), as well as library authors who wish to make their code interoperable with such crates. For example, if you write a custom Vec type, you can implement StableDeref, and then users will be able to use your custom type together with owning_ref and rental.

no_std support can be enabled by disabling default features (specifically "std"). In this case, the trait will not be implemented for the std types mentioned above, but you can still use it for your own types.
*/

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

use core::ops::Deref;


/**
An unsafe marker trait for types that deref to a stable address, even when moved. For example, this is implemented by Box, Vec, Rc, Arc and String, among others. Even when a Box is moved, the underlying storage remains at a fixed location.

More specifically, implementors must ensure that the result of calling deref() is valid for the lifetime of the object, not just the lifetime of the borrow, and that the deref is valid even if the object is moved. Also, it must be valid even after invoking arbitrary &self methods or doing anything transitively accessible from &Self. If Self also implements DerefMut, the same restrictions apply to deref_mut() and it must remain valid if anything transitively accessible from the result of deref_mut() is mutated/called. Additionally, multiple calls to deref, (and deref_mut if implemented) must return the same address. No requirements are placed on &mut self methods other than deref_mut() and drop(), if applicable.

Basically, it must be valid to convert the result of deref() to a pointer, and later dereference that pointer, as long as the original object is still live, even if it has been moved or &self methods have been called on it. If DerefMut is also implemented, it must be valid to get pointers from deref() and deref_mut() and dereference them while the object is live, as long as you don't simultaneously dereference both of them.

Additionally, Deref and DerefMut implementations must not panic, but users of the trait are not allowed to rely on this fact (so that this restriction can be removed later without breaking backwards compatibility, should the need arise).

Here are some examples to help illustrate the requirements for implementing this trait:

```
# use std::ops::Deref;
struct Foo(u8);
impl Deref for Foo {
    type Target = u8;
    fn deref(&self) -> &Self::Target { &self.0 }
}
```

Foo cannot implement StableDeref because the int will move when Foo is moved, invalidating the result of deref().

```
# use std::ops::Deref;
struct Foo(Box<u8>);
impl Deref for Foo {
    type Target = u8;
    fn deref(&self) -> &Self::Target { &*self.0 }
}
```

Foo can safely implement StableDeref, due to the use of Box.


```
# use std::ops::Deref;
# use std::ops::DerefMut;
# use std::rc::Rc;
#[derive(Clone)]
struct Foo(Rc<u8>);
impl Deref for Foo {
    type Target = u8;
    fn deref(&self) -> &Self::Target { &*self.0 }
}
impl DerefMut for Foo {
    fn deref_mut(&mut self) -> &mut Self::Target { Rc::make_mut(&mut self.0) }
}
```

This is a simple implementation of copy-on-write: Foo's deref_mut will copy the underlying int if it is not uniquely owned, ensuring unique access at the point where deref_mut() returns. However, Foo cannot implement StableDeref because calling deref_mut(), followed by clone().deref() will result in mutable and immutable references to the same location. Note that if the DerefMut implementation were removed, Foo could safely implement StableDeref. Likewise, if the Clone implementation were removed, it would be safe to implement StableDeref, although Foo would not be very useful in that case, (without clones, the rc will always be uniquely owned).


```
# use std::ops::Deref;
struct Foo;
impl Deref for Foo {
    type Target = str;
    fn deref(&self) -> &Self::Target { &"Hello" }
}
```
Foo can safely implement StableDeref. It doesn't own the data being derefed, but the data is gaurenteed to live long enough, due to it being 'static.

```
# use std::ops::Deref;
# use std::cell::Cell;
struct Foo(Cell<bool>);
impl Deref for Foo {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        let b = self.0.get();
        self.0.set(!b);
        if b { &"Hello" } else { &"World" }
    }
}
```
Foo cannot safely implement StableDeref, even though every possible result of deref lives long enough. In order to safely implement StableAddress, multiple calls to deref must return the same result.

```
# use std::ops::Deref;
# use std::ops::DerefMut;
struct Foo(Box<(u8, u8)>);
impl Deref for Foo {
    type Target = u8;
    fn deref(&self) -> &Self::Target { &self.0.deref().0 }
}
impl DerefMut for Foo {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0.deref_mut().1 }
}
```

Foo cannot implement StableDeref because deref and deref_mut return different addresses.


*/
pub unsafe trait StableDeref: Deref {}

/**
An unsafe marker trait for types where clones deref to the same address. This has all the requirements of StableDeref, and additionally requires that after calling clone(), both the old and new value deref to the same address. For example, Rc and Arc implement CloneStableDeref, but Box and Vec do not.

Note that a single type should never implement both DerefMut and CloneStableDeref. If it did, this would let you get two mutable references to the same location, by cloning and then calling deref_mut() on both values.
*/
pub unsafe trait CloneStableDeref: StableDeref + Clone {}

/////////////////////////////////////////////////////////////////////////////
// std types integration
/////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::rc::Rc;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(feature = "alloc")]
use alloc::string::String;

#[cfg(feature = "std")]
use std::ffi::{CString, OsString};
#[cfg(feature = "std")]
use std::path::PathBuf;
#[cfg(feature = "std")]
use std::sync::{MutexGuard, RwLockReadGuard, RwLockWriteGuard};

use core::cell::{Ref, RefMut};


#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized> StableDeref for Box<T> {}
#[cfg(feature = "alloc")]
unsafe impl<T> StableDeref for Vec<T> {}
#[cfg(feature = "alloc")]
unsafe impl StableDeref for String {}
#[cfg(feature = "std")]
unsafe impl StableDeref for CString {}
#[cfg(feature = "std")]
unsafe impl StableDeref for OsString {}
#[cfg(feature = "std")]
unsafe impl StableDeref for PathBuf {}

#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized> StableDeref for Rc<T> {}
#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized> CloneStableDeref for Rc<T> {}
#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized> StableDeref for Arc<T> {}
#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized> CloneStableDeref for Arc<T> {}

unsafe impl<'a, T: ?Sized> StableDeref for Ref<'a, T> {}
unsafe impl<'a, T: ?Sized> StableDeref for RefMut<'a, T> {}
#[cfg(feature = "std")]
unsafe impl<'a, T: ?Sized> StableDeref for MutexGuard<'a, T> {}
#[cfg(feature = "std")]
unsafe impl<'a, T: ?Sized> StableDeref for RwLockReadGuard<'a, T> {}
#[cfg(feature = "std")]
unsafe impl<'a, T: ?Sized> StableDeref for RwLockWriteGuard<'a, T> {}

unsafe impl<'a, T: ?Sized> StableDeref for &'a T {}
unsafe impl<'a, T: ?Sized> CloneStableDeref for &'a T {}
unsafe impl<'a, T: ?Sized> StableDeref for &'a mut T {}
