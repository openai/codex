/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::marker;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI16;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicI8;
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::Mutex;

use either::Either;
use hashbrown::raw::RawTable;
use hashbrown::HashTable;
use starlark_map::small_set::SmallSet;
use starlark_map::Hashed;

use crate::collections::SmallMap;
use crate::values::FrozenValue;
use crate::values::Tracer;
use crate::values::Value;

/// Called by the garbage collection, and must walk over every contained `Value` in the type.
/// Marked `unsafe` because if you miss a nested `Value`, it will probably segfault.
///
/// For the most cases `#[derive(Trace)]` is enough to implement this trait:
///
/// ```
/// # use starlark::values::Value;
/// # use starlark::values::Trace;
///
/// #[derive(Trace)]
/// struct MySet<'v> {
///     keys: Vec<Value<'v>>,
/// }
/// ```
pub unsafe trait Trace<'v> {
    /// Recursively "trace" the value.
    ///
    /// Note during trace, `Value` objects in `Self` might be already special forward-objects,
    /// trying to unpack these values may crash the process.
    ///
    /// Generally this function should not do anything except calling `trace` on the fields.
    fn trace(&mut self, tracer: &Tracer<'v>);
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for Vec<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.iter_mut().for_each(|x| x.trace(tracer));
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for [T] {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.iter_mut().for_each(|x| x.trace(tracer));
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for RawTable<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        unsafe {
            self.iter().for_each(|e| e.as_mut().trace(tracer));
        }
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for HashTable<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.iter_mut().for_each(|e| e.trace(tracer));
    }
}

unsafe impl<'v, K: Trace<'v>, V: Trace<'v>> Trace<'v> for SmallMap<K, V> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        for (k, v) in self.iter_mut_unchecked() {
            k.trace(tracer);
            v.trace(tracer);
        }
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for SmallSet<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        for v in self.iter_mut_unchecked() {
            v.trace(tracer);
        }
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for Hashed<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.key_mut().trace(tracer);
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for Option<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        if let Some(x) = self {
            x.trace(tracer)
        }
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for RefCell<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.get_mut().trace(tracer)
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for Cell<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.get_mut().trace(tracer);
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for OnceCell<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        if let Some(x) = self.get_mut() {
            x.trace(tracer)
        }
    }
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for UnsafeCell<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.get_mut().trace(tracer);
    }
}

unsafe impl<'v, T: Trace<'v> + ?Sized> Trace<'v> for Box<T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        Box::as_mut(self).trace(tracer)
    }
}

unsafe impl<'v> Trace<'v> for () {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v, T1: Trace<'v>> Trace<'v> for (T1,) {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.0.trace(tracer);
    }
}

unsafe impl<'v, T1: Trace<'v>, T2: Trace<'v>> Trace<'v> for (T1, T2) {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.0.trace(tracer);
        self.1.trace(tracer);
    }
}

unsafe impl<'v, T1: Trace<'v>, T2: Trace<'v>, T3: Trace<'v>> Trace<'v> for (T1, T2, T3) {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.0.trace(tracer);
        self.1.trace(tracer);
        self.2.trace(tracer);
    }
}

unsafe impl<'v, T1: Trace<'v>, T2: Trace<'v>, T3: Trace<'v>, T4: Trace<'v>> Trace<'v>
    for (T1, T2, T3, T4)
{
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.0.trace(tracer);
        self.1.trace(tracer);
        self.2.trace(tracer);
        self.3.trace(tracer);
    }
}

unsafe impl<'v, T1: Trace<'v>, T2: Trace<'v>> Trace<'v> for Either<T1, T2> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        match self {
            Either::Left(x) => x.trace(tracer),
            Either::Right(x) => x.trace(tracer),
        }
    }
}

unsafe impl<'v> Trace<'v> for Value<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        tracer.trace(self)
    }
}

unsafe impl<'v> Trace<'v> for FrozenValue {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for String {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for usize {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for i32 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for u32 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for u64 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for bool {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicBool {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicI8 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicU8 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicI16 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicU16 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicI32 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicU32 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicI64 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicU64 {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicUsize {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for AtomicIsize {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v> Trace<'v> for std::time::Instant {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v, T> Trace<'v> for marker::PhantomData<T> {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v, T: Trace<'v>> Trace<'v> for Arc<Mutex<T>> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.lock().unwrap().trace(tracer);
    }
}

unsafe impl<'v, A, R> Trace<'v> for fn(A) -> R {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v, A, B, R> Trace<'v> for fn(A, B) -> R {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

unsafe impl<'v, A, B, C, R> Trace<'v> for fn(A, B, C) -> R {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}
