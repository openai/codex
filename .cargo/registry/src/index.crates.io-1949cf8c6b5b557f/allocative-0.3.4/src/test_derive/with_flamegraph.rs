/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::mem;

use allocative::Allocative;

use crate as allocative;
use crate::FlameGraphBuilder;

#[derive(Allocative)]
struct TestData {
    data: Box<[u8]>,
    b: u8,
}

#[test]
fn test_flamegraph() {
    if mem::size_of::<usize>() != 8 {
        return;
    }

    let mut fg = FlameGraphBuilder::default();
    fg.visit_root(&TestData {
        data: vec![0; 100].into_boxed_slice(),
        b: 1,
    });
    assert_eq!(
        "\
        allocative::test_derive::with_flamegraph::TestData 7\n\
        allocative::test_derive::with_flamegraph::TestData;b;u8 1\n\
        allocative::test_derive::with_flamegraph::TestData;data;alloc::boxed::Box<[u8]>;ptr 16\n\
        allocative::test_derive::with_flamegraph::TestData;data;alloc::boxed::Box<[u8]>;ptr;u8;data;u8 100\n\
        ",
        // When running test with buck, crate name is `allocative_unittest`.
        fg.finish_and_write_flame_graph()
            .replace("allocative_unittest::", "allocative::")
    );
}
