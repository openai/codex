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

use allocative::Allocative;
use display_container::fmt_keyed_container;
use either::Either;
use serde::Serialize;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::Coerce;
use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::collections::StarlarkHasher;
use crate::starlark_complex_value;
use crate::typing::Ty;
use crate::values::comparison::equals_slice;
use crate::values::record::field::FieldGen;
use crate::values::record::record_type::record_fields;
use crate::values::record::record_type::FrozenRecordType;
use crate::values::record::record_type::RecordType;
use crate::values::types::type_instance_id::TypeInstanceId;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

/// An actual record.
#[derive(Clone, Debug, Trace, Coerce, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct RecordGen<V: ValueLifetimeless> {
    pub(crate) typ: V, // Must be RecordType
    pub(crate) values: Box<[V]>,
}

impl<'v, V: ValueLike<'v>> Display for RecordGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.record_type_name().unwrap_or("anon");
        fmt_keyed_container(f, &format!("record[{}](", name), ")", "=", self.iter())
    }
}

starlark_complex_value!(pub Record);

impl<'v, V: ValueLike<'v>> RecordGen<V> {
    /// `type(x)` for records.
    pub const TYPE: &'static str = "record";

    fn get_record_type(&self) -> Either<&'v RecordType<'v>, &'v FrozenRecordType> {
        // Safe to unwrap because we always ensure typ is RecordType
        RecordType::from_value(self.typ.to_value()).unwrap()
    }

    fn record_type_name(&self) -> Option<&'v str> {
        match self.get_record_type() {
            Either::Left(x) => Some(&x.ty_record_data.get()?.name),
            Either::Right(x) => Some(&x.ty_record_data.as_ref()?.name),
        }
    }

    pub(crate) fn record_type_id(&self) -> TypeInstanceId {
        match self.get_record_type() {
            Either::Left(x) => x.id,
            Either::Right(x) => x.id,
        }
    }

    fn get_record_fields(&self) -> &'v SmallMap<String, FieldGen<Value<'v>>> {
        record_fields(self.get_record_type())
    }

    /// Iterate over the elements in the record.
    pub fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = (&'v str, V)> + 'a
    where
        'v: 'a,
    {
        self.get_record_fields()
            .keys()
            .map(String::as_str)
            .zip(self.values.iter().copied())
    }
}

#[starlark_value(type = Record::TYPE)]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for RecordGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        match Record::from_value(other) {
            Some(other) if self.typ.equals(other.typ)? => {
                equals_slice(&self.values, &other.values, |x, y| x.equals(*y))
            }
            _ => Ok(false),
        }
    }

    fn get_attr(&self, attribute: &str, heap: &'v Heap) -> Option<Value<'v>> {
        self.get_attr_hashed(Hashed::new(attribute), heap)
    }

    fn get_attr_hashed(&self, attribute: Hashed<&str>, _heap: &'v Heap) -> Option<Value<'v>> {
        let i = self.get_record_fields().get_index_of_hashed(attribute)?;
        Some(self.values[i].to_value())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        self.typ.write_hash(hasher)?;
        for v in &*self.values {
            v.write_hash(hasher)?;
        }
        Ok(())
    }

    fn dir_attr(&self) -> Vec<String> {
        self.get_record_fields().keys().cloned().collect()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(
            self.get_record_type()
                .either(|r| r.instance_ty(), |r| r.instance_ty()),
        )
    }
}

impl<'v, V: ValueLike<'v>> Serialize for RecordGen<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.iter())
    }
}
