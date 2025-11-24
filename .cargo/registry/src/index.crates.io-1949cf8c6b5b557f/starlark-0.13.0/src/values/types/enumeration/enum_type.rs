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

use std::cell::UnsafeCell;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::sync::Arc;

use allocative::Allocative;
use display_container::fmt_container;
use dupe::Dupe;
use either::Either;
use once_cell::unsync::OnceCell;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;
use starlark_derive::Coerce;
use starlark_derive::NoSerialize;
use starlark_derive::Trace;
use starlark_map::small_map::SmallMap;
use starlark_map::Equivalent;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::typing::callable::TyCallable;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::user::TyUser;
use crate::typing::user::TyUserIndex;
use crate::typing::user::TyUserParams;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::values::enumeration::matcher::EnumTypeMatcher;
use crate::values::enumeration::ty_enum_type::TyEnumData;
use crate::values::enumeration::value::EnumValueGen;
use crate::values::enumeration::EnumValue;
use crate::values::function::FUNCTION_TYPE;
use crate::values::index::convert_index;
use crate::values::list::AllocList;
use crate::values::types::type_instance_id::TypeInstanceId;
use crate::values::typing::type_compiled::type_matcher_factory::TypeMatcherFactory;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(thiserror::Error, Debug)]
enum EnumError {
    #[error("enum values must all be distinct, but repeated `{0}`")]
    DuplicateEnumValue(String),
    #[error("Unknown enum element `{0}`, given to `{1}`")]
    InvalidElement(String, String),
}

#[doc(hidden)]
pub trait EnumCell: Freeze {
    type TyEnumDataOpt: Debug;

    fn get_or_init_ty(
        ty: &Self::TyEnumDataOpt,
        f: impl FnOnce() -> crate::Result<Arc<TyEnumData>>,
    ) -> crate::Result<()>;
    fn get_ty(ty: &Self::TyEnumDataOpt) -> Option<&Arc<TyEnumData>>;
}

impl<'v> EnumCell for Value<'v> {
    type TyEnumDataOpt = OnceCell<Arc<TyEnumData>>;

    fn get_or_init_ty(
        ty: &Self::TyEnumDataOpt,
        f: impl FnOnce() -> crate::Result<Arc<TyEnumData>>,
    ) -> crate::Result<()> {
        ty.get_or_try_init(f)?;
        Ok(())
    }

    fn get_ty(ty: &Self::TyEnumDataOpt) -> Option<&Arc<TyEnumData>> {
        ty.get()
    }
}

impl EnumCell for FrozenValue {
    type TyEnumDataOpt = Option<Arc<TyEnumData>>;

    fn get_or_init_ty(
        ty: &Self::TyEnumDataOpt,
        f: impl FnOnce() -> crate::Result<Arc<TyEnumData>>,
    ) -> crate::Result<()> {
        let _ignore = (ty, f);
        Ok(())
    }

    fn get_ty(ty: &Self::TyEnumDataOpt) -> Option<&Arc<TyEnumData>> {
        ty.as_ref()
    }
}

/// The type of an enumeration, created by `enum()`.
#[derive(Debug, Trace, Coerce, NoSerialize, ProvidesStaticType, Allocative)]
#[repr(C)]
// Deliberately store fully populated values
// for each entry, so we can produce enum values with zero allocation.
pub struct EnumTypeGen<V: EnumCell> {
    pub(crate) id: TypeInstanceId,
    #[allocative(skip)] // TODO(nga): do not skip.
    // TODO(nga): teach derive to do something like `#[trace(static)]`.
    #[trace(unsafe_ignore)]
    pub(crate) ty_enum_data: V::TyEnumDataOpt,
    // The key is the value of the enumeration
    // The value is a value of type EnumValue
    #[allocative(skip)] // TODO(nga): do not skip.
    elements: UnsafeCell<SmallMap<V, V>>,
}

impl<'v> Freeze for EnumTypeGen<Value<'v>> {
    type Frozen = EnumTypeGen<FrozenValue>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let EnumTypeGen {
            id,
            ty_enum_data: ty_enum_type,
            elements,
        } = self;
        let ty_enum_type = ty_enum_type.into_inner();
        let elements = elements.freeze(freezer)?;
        Ok(EnumTypeGen {
            id,
            ty_enum_data: ty_enum_type,
            elements,
        })
    }
}

unsafe impl<V: EnumCell + Freeze> Send for EnumTypeGen<V> {}
unsafe impl<V: EnumCell + Freeze> Sync for EnumTypeGen<V> {}

impl<'v, V: EnumCell + ValueLike<'v>> Display for EnumTypeGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_container(f, "enum(", ")", self.elements().iter().map(|(k, _v)| k))
    }
}

/// Unfrozen enum type.
pub type EnumType<'v> = EnumTypeGen<Value<'v>>;
/// Frozen enum type.
pub type FrozenEnumType = EnumTypeGen<FrozenValue>;

impl<'v> EnumType<'v> {
    pub(crate) fn new(elements: Vec<StringValue<'v>>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        // We are constructing the enum and all elements in one go.
        // They both point at each other, which adds to the complexity.
        let id = TypeInstanceId::gen();
        let typ = heap.alloc(EnumType {
            id,
            ty_enum_data: OnceCell::new(),
            elements: UnsafeCell::new(SmallMap::new()),
        });

        let mut res = SmallMap::with_capacity(elements.len());
        for (i, x) in elements.iter().enumerate() {
            let v = heap.alloc(EnumValue {
                id,
                typ,
                index: i as i32,
                value: x.to_value(),
            });
            if res.insert_hashed(x.to_value().get_hashed()?, v).is_some() {
                return Err(crate::Error::new_other(EnumError::DuplicateEnumValue(
                    x.to_string(),
                )));
            }
        }

        // Here we tie the cycle
        let t = typ.downcast_ref::<EnumType>().unwrap();
        unsafe {
            // SAFETY: we own unique reference to `t`.
            *t.elements.get() = res;
        }
        Ok(typ)
    }
}

impl<V: EnumCell + Freeze> EnumTypeGen<V> {
    pub(crate) fn elements(&self) -> &SmallMap<V, V> {
        // Safe because we never mutate the elements after construction.
        unsafe { &*self.elements.get() }
    }
}

impl<'v, V> EnumTypeGen<V>
where
    Value<'v>: Equivalent<V>,
    V: ValueLike<'v> + EnumCell,
{
    pub(crate) fn ty_enum_data(&self) -> Option<&Arc<TyEnumData>> {
        V::get_ty(&self.ty_enum_data)
    }

    pub(crate) fn construct(&self, val: Value<'v>) -> crate::Result<V> {
        match self.elements().get_hashed_by_value(val.get_hashed()?) {
            Some(v) => Ok(*v),
            None => Err(crate::Error::new_other(EnumError::InvalidElement(
                val.to_str(),
                self.to_string(),
            ))),
        }
    }
}

#[starlark_value(type = FUNCTION_TYPE)]
impl<'v, V> StarlarkValue<'v> for EnumTypeGen<V>
where
    Self: ProvidesStaticType<'v>,
    Value<'v>: Equivalent<V>,
    V: ValueLike<'v> + EnumCell,
{
    type Canonical = FrozenEnumType;

    // TODO(nga): replace `Color("RED")` with `Color.RED`.
    //   https://www.internalfb.com/tasks/?t=183515013
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        args.no_named_args()?;
        let val = args.positional1(eval.heap())?;
        Ok(self.construct(val)?.to_value())
    }

    fn length(&self) -> crate::Result<i32> {
        Ok(self.elements().len() as i32)
    }

    fn at(&self, index: Value, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        let i = convert_index(index, self.elements().len() as i32)? as usize;
        // Must be in the valid range since convert_index checks that, so just unwrap
        Ok(self
            .elements()
            .get_index(i)
            .map(|x| *x.1)
            .unwrap()
            .to_value())
    }

    unsafe fn iterate(&self, me: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(me)
    }

    unsafe fn iter_size_hint(&self, index: usize) -> (usize, Option<usize>) {
        debug_assert!(index <= self.elements().len());
        let rem = self.elements().len() - index;
        (rem, Some(rem))
    }

    unsafe fn iter_next(&self, index: usize, _heap: &'v Heap) -> Option<Value<'v>> {
        self.elements().values().nth(index).map(|v| v.to_value())
    }

    unsafe fn iter_stop(&self) {}

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(enum_type_methods)
    }

    fn eval_type(&self) -> Option<Ty> {
        self.ty_enum_data().map(|t| t.ty_enum_value.dupe())
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        self.ty_enum_data().map(|t| t.ty_enum_type.dupe())
    }

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<()> {
        V::get_or_init_ty(&self.ty_enum_data, || {
            let ty_enum_value = Ty::custom(TyUser::new(
                variable_name.to_owned(),
                TyStarlarkValue::new::<EnumValue>(),
                self.id,
                TyUserParams {
                    matcher: Some(TypeMatcherFactory::new(EnumTypeMatcher { id: self.id })),
                    ..TyUserParams::default()
                },
            )?);
            let ty_enum_type = Ty::custom(TyUser::new(
                format!("enum[{}]", variable_name),
                TyStarlarkValue::new::<EnumType>(),
                TypeInstanceId::gen(),
                TyUserParams {
                    index: Some(TyUserIndex {
                        index: Ty::int(),
                        result: ty_enum_value.dupe(),
                    }),
                    iter_item: Some(ty_enum_value.dupe()),
                    callable: Some(TyCallable::new(
                        ParamSpec::pos_only(
                            [
                                // TODO(nga): we can do better parameter type.
                                Ty::any(),
                            ],
                            [],
                        ),
                        ty_enum_value.dupe(),
                    )),
                    ..TyUserParams::default()
                },
            )?);
            Ok(Arc::new(TyEnumData {
                name: variable_name.to_owned(),
                variants: self
                    .elements()
                    .iter()
                    .map(|(_, enum_value)| {
                        let enum_value: &EnumValueGen<_> =
                            EnumValue::from_value(enum_value.to_value())
                                .expect("known to be enum value");
                        Ty::of_value(enum_value.value)
                    })
                    .collect(),
                id: self.id,
                ty_enum_value,
                ty_enum_type,
            }))
        })
    }
}

#[starlark_module]
fn enum_type_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn r#type<'v>(this: Value, heap: &Heap) -> starlark::Result<Value<'v>> {
        let this = EnumType::from_value(this).unwrap();
        let ty_enum_type = match this {
            Either::Left(x) => x.ty_enum_data(),
            Either::Right(x) => x.ty_enum_data(),
        };
        match ty_enum_type {
            Some(ty_enum_type) => Ok(heap.alloc(ty_enum_type.name.as_str())),
            None => Ok(heap.alloc(EnumValue::TYPE)),
        }
    }

    fn values<'v>(this: Value<'v>) -> anyhow::Result<AllocList<impl Iterator<Item = Value<'v>>>> {
        let this = EnumType::from_value(this).unwrap();
        match this {
            Either::Left(x) => Ok(AllocList(Either::Left(x.elements().keys().copied()))),
            Either::Right(x) => Ok(AllocList(Either::Right(
                x.elements().keys().map(|x| x.to_value()),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_enum_type_as_type_pass() {
        assert::pass(
            r#"
Color = enum("RED", "GREEN", "BLUE")

def f_pass(x: Color):
    pass

def g_pass(x: Color):
    f_pass(x)
"#,
        );
    }

    #[test]
    fn test_enum_type_fail_runtime() {
        assert::fail(
            r#"
Color = enum("RED", "GREEN", "BLUE")
Season = enum("SPRING", "SUMMER", "AUTUMN", "WINTER")

def f(x: Color):
    pass

def g(x):
    f(x)

g(Season[0])
"#,
            r#"Value `Season("SPRING")` of type `enum` does not match the type annotation `Color` for argument `x`"#,
        );
    }

    #[test]
    fn test_enum_type_fail_compile_time() {
        assert::fail(
            r#"
Color = enum("RED", "GREEN", "BLUE")
Season = enum("SPRING", "SUMMER", "AUTUMN", "WINTER")

def f(x: Color):
    pass

def g(x: Season):
    f(x)
"#,
            r#"Expected type `Color` but got `Season`"#,
        );
    }

    #[test]
    fn test_enum_is_callable() {
        assert::pass(
            r#"
Color = enum("RED", "GREEN", "BLUE")

def foo(x: typing.Callable):
    pass

def bar():
    foo(Color)
"#,
        );
    }

    #[test]
    fn test_enum_value_index() {
        // Test `.index` is available at both compile and runtime.
        assert::pass(
            r#"
Color = enum("RED", "GREEN", "BLUE")

def test():
    for c in Color:
        if c.index == 1:
            pass

test()
"#,
        );
    }

    #[test]
    fn test_enum_value_index_correct_type() {
        assert::fail(
            r#"
Fruit = enum("APPLE", "BANANA", "ORANGE")

def expect_str(s: str):
    pass

def test():
    for f in Fruit:
        expect_str(f.index)
"#,
            "Expected type `str` but got `int`",
        );
    }

    #[test]
    fn test_enum_index() {
        assert::pass(
            r#"
Mood = enum("HAPPY", "SAD")

def test() -> Mood:
    return Mood[0]

test()
"#,
        );
    }

    #[test]
    fn test_enum_index_fail() {
        assert::fail(
            r#"
Shape = enum("SQUARE", "CIRCLE")

def accept_str(s: str):
    pass

def test():
    accept_str(Shape[0])
"#,
            "Expected type `str` but got `Shape`",
        );
    }

    #[test]
    fn test_enum_call() {
        assert::fail(
            r#"
Currency = enum("GBP", "USD", "EUR")

def accept_str(s: str):
    pass

def test():
    accept_str(Currency("GBP"))
"#,
            "Expected type `str` but got `Currency`",
        );
    }
}
