/*
 * Copyright 2019 The Starlark in Rust Authors.
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

/// Reduce boilerplate when making types instances of [`ComplexValue`](crate::values::ComplexValue)
/// - see the [`ComplexValue`](crate::values::ComplexValue) docs for an example.
#[macro_export]
macro_rules! starlark_complex_value {
    // Common part of macro variants.
    (impl $x:ident) => {
        $crate::__macro_refs::item! {
            impl<'v> $crate::values::AllocValue<'v> for $x<'v> {
                #[inline]
                fn alloc_value(self, heap: &'v $crate::values::Heap) -> $crate::values::Value<'v> {
                    heap.alloc_complex(self)
                }
            }

            impl $crate::values::AllocFrozenValue for [< Frozen $x >] {
                #[inline]
                fn alloc_frozen_value(self, heap: &$crate::values::FrozenHeap) -> $crate::values::FrozenValue {
                    heap.alloc_simple(self)
                }
            }

            impl<'v> $x<'v> {
                /// Downcast the value.
                #[inline]
                pub fn from_value(x: $crate::values::Value<'v>) -> Option<&'v Self> {
                    if let Some(x) = x.unpack_frozen() {
                        $crate::values::ValueLike::downcast_ref::< [< Frozen $x >] >(x).map($crate::__macro_refs::coerce)
                    } else {
                        $crate::values::ValueLike::downcast_ref::< $x<'v> >(x)
                    }
                }
            }

            impl<'v> $crate::values::type_repr::StarlarkTypeRepr for &'v $x<'v> {
                type Canonical = $x<'v>;

                #[inline]
                fn starlark_type_repr() -> $crate::typing::Ty {
                    <$x as $crate::values::StarlarkValue>::get_type_starlark_repr()
                }
            }

            impl<'v> $crate::values::UnpackValue<'v> for &'v $x<'v> {
                type Error = std::convert::Infallible;

                #[inline]
                fn unpack_value_impl(x: $crate::values::Value<'v>) -> Result<Option<&'v $x<'v>>, Self::Error> {
                    Ok($x::from_value(x))
                }
            }
        }
    };
    ($v:vis $x:ident) => {
        $crate::__macro_refs::item! {
            /// Type of value.
            $v type $x<'v> = [< $x Gen >]<$crate::values::Value<'v>>;
            /// Type of frozen value.
            $v type [< Frozen $x >] = [< $x Gen >]<$crate::values::FrozenValue>;

            starlark_complex_value!(impl $x);
        }
    };
    ($v:vis $x:ident <'v>) => {
        $crate::__macro_refs::item! {
            /// Type of unfrozen value.
            $v type $x<'v> = [< $x Gen >]<'v, $crate::values::Value<'v>>;
            /// Type of frozen value.
            $v type [< Frozen $x >] = [< $x Gen >]<'static, $crate::values::FrozenValue>;

            starlark_complex_value!(impl $x);
        }
    };
}

/// Reduce boilerplate when making types instances of [`ComplexValue`](crate::values::ComplexValue)
/// - see the [`ComplexValue`](crate::values::ComplexValue) docs for an example.
#[macro_export]
macro_rules! starlark_complex_values {
    ($x:ident) => {
        $crate::__macro_refs::item! {
            impl<'v> $crate::values::AllocValue<'v> for $x<'v> {
                #[inline]
                fn alloc_value(self, heap: &'v $crate::values::Heap) -> $crate::values::Value<'v> {
                    heap.alloc_complex(self)
                }
            }

            impl $crate::values::AllocFrozenValue for [< Frozen $x >] {
                #[inline]
                fn alloc_frozen_value(self, heap: &$crate::values::FrozenHeap) -> $crate::values::FrozenValue {
                    heap.alloc_simple(self)
                }
            }

            impl<'v> $x<'v> {
                #[allow(dead_code)]
                #[inline]
                pub(crate) fn from_value(
                    x: $crate::values::Value<'v>,
                ) -> Option<$crate::__macro_refs::Either<&'v Self, &'v [< Frozen $x >]>> {
                    if let Some(x) = x.unpack_frozen() {
                        $crate::values::ValueLike::downcast_ref(x).map($crate::__macro_refs::Either::Right)
                    } else {
                        $crate::values::ValueLike::downcast_ref(x).map($crate::__macro_refs::Either::Left)
                    }
                }
            }
        }
    };
}

/// A macro reducing boilerplace defining Starlark values which are simple - they
/// aren't mutable and can't contain references to other Starlark values.
///
/// Let's define a simple object, where `+x` makes the string uppercase:
///
/// ```
/// use allocative::Allocative;
/// use derive_more::Display;
/// use starlark::starlark_simple_value;
/// use starlark::values::Heap;
/// use starlark::values::NoSerialize;
/// use starlark::values::ProvidesStaticType;
/// use starlark::values::StarlarkValue;
/// use starlark::values::Value;
/// use starlark_derive::starlark_value;
///
/// #[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
/// struct MyObject(String);
/// starlark_simple_value!(MyObject);
///
/// #[starlark_value(type = "my_object")]
/// impl<'v> StarlarkValue<'v> for MyObject {
///     // We can choose to implement whichever methods we want.
///     // All other operations will result in runtime errors.
///     fn plus(&self, heap: &'v Heap) -> starlark::Result<Value<'v>> {
///         Ok(heap.alloc(MyObject(self.0.to_uppercase())))
///     }
/// }
/// ```
///
/// The `starlark_simple_value!` macro defines instances of
/// [`ProvidesStaticType`](crate::values::ProvidesStaticType),
/// [`AllocValue`](crate::values::AllocValue),
/// [`AllocFrozenValue`](crate::values::AllocFrozenValue) and
/// [`UnpackValue`](crate::values::UnpackValue). It also defines a method:
///
/// ```
/// # use crate::starlark::values::*;
/// # struct MyObject;
/// impl MyObject {
///     pub fn from_value<'v>(x: Value<'v>) -> Option<&'v MyObject> {
/// # unimplemented!(
/// # r#"
///         ...
/// # "#);
///     }
/// }
/// ```
#[macro_export]
macro_rules! starlark_simple_value {
    ($x:ident) => {
        $crate::__macro_refs::item! {
            impl<'v> $crate::values::AllocValue<'v> for $x {
                #[inline]
                fn alloc_value(self, heap: &'v $crate::values::Heap) -> $crate::values::Value<'v> {
                    heap.alloc_simple(self)
                }
            }

            impl $crate::values::AllocFrozenValue for $x {
                #[inline]
                fn alloc_frozen_value(self, heap: &$crate::values::FrozenHeap) -> $crate::values::FrozenValue {
                    heap.alloc_simple(self)
                }
            }

            impl $x {
                /// Downcast a value to self type.
                #[inline]
                pub fn from_value<'v>(x: $crate::values::Value<'v>) -> Option<&'v Self> {
                    $crate::values::ValueLike::downcast_ref::< $x >(x)
                }
            }

            impl<'v> $crate::values::type_repr::StarlarkTypeRepr for &'v $x {
                type Canonical = $x;

                fn starlark_type_repr() -> $crate::typing::Ty {
                    <$x as $crate::values::StarlarkValue>::get_type_starlark_repr()
                }
            }

            impl<'v> $crate::values::UnpackValue<'v> for &'v $x {
                type Error = std::convert::Infallible;

                #[inline]
                fn unpack_value_impl(x: $crate::values::Value<'v>) -> std::result::Result<Option<&'v $x>, Self::Error> {
                    std::result::Result::Ok($x::from_value(x))
                }
            }
        }
    };
}
