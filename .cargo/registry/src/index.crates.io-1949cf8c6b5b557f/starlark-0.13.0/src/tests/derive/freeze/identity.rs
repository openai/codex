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

#![allow(dead_code)]

use starlark::values::Freeze;

use crate as starlark;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;

struct NonFreeze(u32);

#[derive(Freeze)]
struct TestStruct {
    s: String,
    #[freeze(identity)]
    s2: NonFreeze,
}

#[derive(Freeze)]
struct TestUnitStruct(String, #[freeze(identity)] NonFreeze);

#[derive(Freeze)]
enum TestEnum {
    A(String),
    B(#[freeze(identity)] NonFreeze),
}

#[test]
fn test_struct() -> anyhow::Result<()> {
    let t = TestStruct {
        s: "test".to_owned(),
        s2: NonFreeze(55),
    };
    let freezer = Freezer::new(FrozenHeap::new());
    t.freeze(&freezer)?;
    Ok(())
}

#[test]
fn test_anon_struct() -> anyhow::Result<()> {
    let t = TestUnitStruct("test".to_owned(), NonFreeze(56));
    let freezer = Freezer::new(FrozenHeap::new());
    t.freeze(&freezer)?;
    Ok(())
}
