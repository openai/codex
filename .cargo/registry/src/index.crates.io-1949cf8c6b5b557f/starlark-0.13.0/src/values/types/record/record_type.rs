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

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use display_container::fmt_keyed_container;
use dupe::Dupe;
use either::Either;
use once_cell::unsync::OnceCell;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_map::small_map::SmallMap;
use starlark_map::sorted_map::SortedMap;
use starlark_map::StarlarkHasher;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::coerce;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::eval::ParametersSpec;
use crate::eval::ParametersSpecParam;
use crate::starlark_complex_values;
use crate::typing::callable::TyCallable;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::user::TyUser;
use crate::typing::user::TyUserFields;
use crate::typing::user::TyUserParams;
use crate::typing::ParamIsRequired;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::util::ArcStr;
use crate::values::function::FUNCTION_TYPE;
use crate::values::record::field::FieldGen;
use crate::values::record::matcher::RecordTypeMatcher;
use crate::values::record::ty_record_type::TyRecordData;
use crate::values::record::Record;
use crate::values::types::type_instance_id::TypeInstanceId;
use crate::values::typing::type_compiled::type_matcher_factory::TypeMatcherFactory;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;
use crate::values::ValueTypedComplex;

#[doc(hidden)]
pub trait RecordCell: ValueLifetimeless {
    type TyRecordDataOpt: Debug;

    fn get_or_init_ty(
        ty: &Self::TyRecordDataOpt,
        f: impl FnOnce() -> crate::Result<Arc<TyRecordData>>,
    ) -> crate::Result<()>;
    fn get_ty(ty: &Self::TyRecordDataOpt) -> Option<&Arc<TyRecordData>>;
}

impl<'v> RecordCell for Value<'v> {
    type TyRecordDataOpt = OnceCell<Arc<TyRecordData>>;

    fn get_or_init_ty(
        ty: &Self::TyRecordDataOpt,
        f: impl FnOnce() -> crate::Result<Arc<TyRecordData>>,
    ) -> crate::Result<()> {
        ty.get_or_try_init(f)?;
        Ok(())
    }

    fn get_ty(ty: &Self::TyRecordDataOpt) -> Option<&Arc<TyRecordData>> {
        ty.get()
    }
}
impl RecordCell for FrozenValue {
    type TyRecordDataOpt = Option<Arc<TyRecordData>>;

    fn get_or_init_ty(
        ty: &Self::TyRecordDataOpt,
        f: impl FnOnce() -> crate::Result<Arc<TyRecordData>>,
    ) -> crate::Result<()> {
        let _ignore = (ty, f);
        Ok(())
    }

    fn get_ty(ty: &Self::TyRecordDataOpt) -> Option<&Arc<TyRecordData>> {
        ty.as_ref()
    }
}

#[derive(Debug, thiserror::Error)]
enum RecordTypeError {
    #[error(
        "Record instance cannot be created if record type is not assigned to a global variable"
    )]
    RecordTypeNotAssigned,
}

/// The result of `record()`, being the type of records.
#[derive(Debug, Trace, NoSerialize, ProvidesStaticType, Allocative)]
pub struct RecordTypeGen<V: RecordCell> {
    pub(crate) id: TypeInstanceId,
    #[allocative(skip)] // TODO(nga): do not skip.
    // TODO(nga): teach derive to do something like `#[trace(static)]`.
    #[trace(unsafe_ignore)]
    pub(crate) ty_record_data: V::TyRecordDataOpt,
    /// The V is the type the field must satisfy (e.g. `"string"`)
    fields: SmallMap<String, FieldGen<V>>,
    /// Creating these on every invoke is pretty expensive (profiling shows)
    /// so compute them in advance and cache.
    parameter_spec: ParametersSpec<FrozenValue>,
}

impl<'v, V: ValueLike<'v> + RecordCell> Display for RecordTypeGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_keyed_container(f, "record(", ")", "=", &self.fields)
    }
}

/// Type of a record in a heap.
pub type RecordType<'v> = RecordTypeGen<Value<'v>>;
/// Type of a record in a frozen heap.
pub type FrozenRecordType = RecordTypeGen<FrozenValue>;

starlark_complex_values!(RecordType);

pub(crate) fn record_fields<'v>(
    x: Either<&'v RecordType<'v>, &'v FrozenRecordType>,
) -> &'v SmallMap<String, FieldGen<Value<'v>>> {
    x.either(|x| &x.fields, |x| coerce(&x.fields))
}

impl<'v> RecordType<'v> {
    pub(crate) fn new(fields: SmallMap<String, FieldGen<Value<'v>>>) -> Self {
        let parameter_spec = Self::make_parameter_spec(&fields);
        Self {
            id: TypeInstanceId::gen(),
            fields,
            parameter_spec,
            ty_record_data: OnceCell::new(),
        }
    }

    fn make_parameter_spec(
        fields: &SmallMap<String, FieldGen<Value<'v>>>,
    ) -> ParametersSpec<FrozenValue> {
        ParametersSpec::new_named_only(
            "record",
            fields.iter().map(|(name, field)| {
                (
                    name.as_str(),
                    match field.default {
                        None => ParametersSpecParam::Required,
                        Some(_default) => ParametersSpecParam::Optional,
                    },
                )
            }),
        )
    }
}

impl<'v> Freeze for RecordType<'v> {
    type Frozen = FrozenRecordType;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenRecordType {
            id: self.id,
            fields: self.fields.freeze(freezer)?,
            parameter_spec: self.parameter_spec,
            ty_record_data: self.ty_record_data.into_inner(),
        })
    }
}

impl<'v, V: ValueLike<'v> + RecordCell + 'v> RecordTypeGen<V>
where
    Self: ProvidesStaticType<'v>,
    FieldGen<V>: ProvidesStaticType<'v>,
{
    pub(crate) fn ty_record_data(&self) -> Option<&Arc<TyRecordData>> {
        V::get_ty(&self.ty_record_data)
    }

    pub(crate) fn instance_ty(&self) -> Ty {
        self.ty_record_data()
            .expect("Instances can only be created if named are assigned")
            .ty_record
            .dupe()
    }
}

#[starlark_value(type = FUNCTION_TYPE)]
impl<'v, V: ValueLike<'v> + RecordCell + 'v> StarlarkValue<'v> for RecordTypeGen<V>
where
    Self: ProvidesStaticType<'v>,
    FieldGen<V>: ProvidesStaticType<'v>,
{
    type Canonical = FrozenRecordType;

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        for (name, typ) in &self.fields {
            name.hash(hasher);
            // No need to hash typ.1, since it was computed from typ.0
            typ.write_hash(hasher)?;
        }
        Ok(())
    }

    fn invoke(
        &self,
        me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        if self.ty_record_data().is_none() {
            return Err(crate::Error::new_other(
                RecordTypeError::RecordTypeNotAssigned,
            ));
        }

        let this = me;

        self.parameter_spec
            .parser(args, eval, |param_parser, eval| {
                let fields = record_fields(RecordType::from_value(this).unwrap());
                let mut values = Vec::with_capacity(fields.len());
                for (name, field) in fields.iter() {
                    let value = match field.default {
                        None => {
                            let v: Value = param_parser.next()?;
                            field.typ.check_type(v, Some(name))?;
                            v
                        }
                        Some(default) => {
                            let v: Option<Value> = param_parser.next_opt()?;
                            match v {
                                None => default,
                                Some(v) => {
                                    field.typ.check_type(v, Some(name))?;
                                    v
                                }
                            }
                        }
                    };
                    values.push(value);
                }
                Ok(eval.heap().alloc_complex(Record {
                    typ: this,
                    values: values.into_boxed_slice(),
                }))
            })
            .map_err(Into::into)
    }

    fn get_methods() -> Option<&'static Methods>
    where
        Self: Sized,
    {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(record_type_methods)
    }

    fn eval_type(&self) -> Option<Ty> {
        self.ty_record_data().map(|t| t.ty_record.dupe())
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        self.ty_record_data().map(|t| t.ty_record_type.dupe())
    }

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<()> {
        V::get_or_init_ty(&self.ty_record_data, || {
            let fields: SortedMap<String, Ty> = self
                .fields
                .iter()
                .map(|(name, field)| (name.clone(), field.ty()))
                .collect();

            let ty_record = Ty::custom(TyUser::new(
                variable_name.to_owned(),
                TyStarlarkValue::new::<Record>(),
                self.id,
                TyUserParams {
                    matcher: Some(TypeMatcherFactory::new(RecordTypeMatcher { id: self.id })),
                    fields: TyUserFields {
                        known: fields,
                        unknown: false,
                    },
                    ..TyUserParams::default()
                },
            )?);

            let ty_record_type = Ty::custom(TyUser::new(
                format!("record[{}]", variable_name),
                TyStarlarkValue::new::<RecordType>(),
                TypeInstanceId::gen(),
                TyUserParams {
                    callable: Some(TyCallable::new(
                        ParamSpec::new_named_only(self.fields.iter().map(|(name, field)| {
                            (
                                ArcStr::from(name.as_str()),
                                if field.default.is_some() {
                                    ParamIsRequired::No
                                } else {
                                    ParamIsRequired::Yes
                                },
                                field.ty(),
                            )
                        }))?,
                        ty_record.dupe(),
                    )),
                    ..TyUserParams::default()
                },
            )?);

            Ok(Arc::new(TyRecordData {
                name: variable_name.to_owned(),
                id: self.id,
                ty_record,
                ty_record_type,
            }))
        })
    }
}

#[starlark_module]
fn record_type_methods(methods: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn r#type<'v>(this: ValueTypedComplex<'v, RecordType<'v>>) -> starlark::Result<&'v str> {
        let ty_record_type = match this.unpack() {
            Either::Left(x) => x.ty_record_data.get(),
            Either::Right(x) => x.ty_record_data.as_ref(),
        };
        Ok(ty_record_type.map_or(Record::TYPE, |s| s.name.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_record_type_as_type_pass() {
        assert::pass(
            r"
RecPass = record(a = field(int), b = field(int))

def f_pass(x: RecPass):
    return x.a

f_pass(RecPass(a = 1, b = 2))
",
        );
    }

    #[test]
    fn test_record_type_as_type_compile_time() {
        assert::fail_golden(
            "src/values/types/record/record_type/record_type_as_type_compile_time.golden",
            r"
RecFailCt1 = record(a = field(int), b = field(int))
RecFailCt2 = record(a = field(int), b = field(int))

def f_fail_ct(x: RecFailCt1):
    return x.a

def test():
    f_fail_ct(RecFailCt2(a = 1, b = 2))
",
        );
    }

    #[test]
    fn test_record_type_as_type_runtime() {
        assert::fail_golden(
            "src/values/types/record/record_type/record_type_as_type_runtime.golden",
            r"
RecFailRt1 = record(a = field(int), b = field(int))
RecFailRt2 = record(a = field(int), b = field(int))

def f_fail_rt(x: RecFailRt1):
    return x.a

noop(f_fail_rt)(RecFailRt2(a = 1, b = 2))
",
        );
    }

    #[test]
    fn test_anon_record() {
        assert::fail_golden(
            "src/values/types/record/record_type/anon_record.golden",
            "record(a = field(int))(a = 1)",
        );
    }
}
