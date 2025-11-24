/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use crate as allocative;
use crate::Allocative;

struct Unsupported;

#[derive(Allocative)]
struct TestIgnoreField {
    #[allocative(skip)]
    _unsupported: Unsupported,
}

#[derive(Allocative)]
#[allocative(skip)]
struct TestSkipOnStruct {
    _unsupported: Unsupported,
}

#[derive(Allocative)]
#[allocative(skip)]
enum TestSkipOnEnum {
    Unsupported(Unsupported),
}

#[derive(Allocative)]
enum TestEmptyNoVariants {}

#[derive(Allocative)]
struct TestIgnoreInTupleStruct(#[allocative(skip)] Unsupported);

#[derive(Allocative)]
enum TestAllocativeIgnoreEnumVariant {
    String(String),
    #[allocative(skip)]
    Unsupported(Unsupported),
    UnsupportedInside(#[allocative(skip)] Unsupported),
}

#[derive(Allocative)]
#[allocative(bound = "")]
struct IgnoreBound<T> {
    #[allocative(skip)]
    t: T,
}

#[derive(Allocative)]
struct TestBoundIgnored {
    ignore_bound: IgnoreBound<Unsupported>,
}

#[derive(Allocative)]
#[allocative(skip)]
struct TypeLevelSkipImpliesNoBounds<T> {
    t: T,
}

#[derive(Allocative)]
struct TestTypeLevelSkipImpliesNoBounds {
    type_level_skip: TypeLevelSkipImpliesNoBounds<Unsupported>,
}
