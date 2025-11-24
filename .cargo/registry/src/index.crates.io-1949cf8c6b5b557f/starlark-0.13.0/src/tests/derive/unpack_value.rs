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

use either::Either;

use crate as starlark;
use crate::const_frozen_string;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::typing::StarlarkNever;
use crate::values::UnpackValue;
use crate::values::Value;

#[derive(StarlarkTypeRepr, UnpackValue, Eq, PartialEq, Debug)]
enum EmptyEnum {}

#[derive(StarlarkTypeRepr, UnpackValue, Eq, PartialEq, Debug)]
enum JustInt {
    Int(i32),
}

#[derive(StarlarkTypeRepr, UnpackValue, Eq, PartialEq, Debug)]
enum IntOrStr {
    Int(i32),
    Str(String),
}

#[derive(StarlarkTypeRepr, UnpackValue, Eq, PartialEq, Debug)]
enum WithLifetime<'v> {
    Int(i32),
    Str(&'v str),
}

#[test]
fn test_starlark_type_repr() {
    assert_eq!(
        StarlarkNever::starlark_type_repr(),
        EmptyEnum::starlark_type_repr()
    );

    assert_eq!(i32::starlark_type_repr(), JustInt::starlark_type_repr());

    assert_eq!(
        Either::<i32, String>::starlark_type_repr(),
        IntOrStr::starlark_type_repr()
    );

    assert_eq!(
        Either::<i32, String>::starlark_type_repr(),
        WithLifetime::starlark_type_repr()
    );
}

#[test]
fn test_unpack_value() {
    assert_eq!(
        Some(JustInt::Int(17)),
        JustInt::unpack_value(Value::testing_new_int(17)).unwrap(),
    );

    assert_eq!(
        Some(IntOrStr::Int(19)),
        IntOrStr::unpack_value(Value::testing_new_int(19)).unwrap(),
    );
    assert_eq!(
        Some(IntOrStr::Str("abc".to_owned())),
        IntOrStr::unpack_value(const_frozen_string!("abc").to_value()).unwrap(),
    );

    assert_eq!(
        Some(WithLifetime::Int(23)),
        WithLifetime::unpack_value(Value::testing_new_int(23)).unwrap(),
    );

    assert_eq!(
        Some(WithLifetime::Str("def")),
        WithLifetime::unpack_value(const_frozen_string!("def").to_value()).unwrap(),
    );
}
