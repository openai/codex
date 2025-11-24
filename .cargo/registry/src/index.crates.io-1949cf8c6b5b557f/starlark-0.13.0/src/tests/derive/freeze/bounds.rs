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

use crate as starlark;
use crate::values::Freeze;
use crate::values::FreezeError;
use crate::values::FreezeResult;

trait Bound<'x> {}

#[derive(Freeze)]
#[freeze(validator = check_type, bounds = "<V as Freeze>::Frozen: Bound<'freeze>")]
struct Test<V> {
    field: V,
}

fn check_type<'x, V>(_: &Test<V>) -> anyhow::Result<()>
where
    V: Bound<'x>,
{
    Ok(())
}

#[test]
fn assert_impl() {
    #[derive(Freeze)]
    struct Impl {}
    impl<'x> Bound<'x> for Impl {}
    fn check(_: impl Freeze) {}
    check(Test { field: Impl {} });
}
