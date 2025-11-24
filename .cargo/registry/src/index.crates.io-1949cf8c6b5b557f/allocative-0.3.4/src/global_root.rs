/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::sync::Mutex;

use crate::allocative_trait::Allocative;

static ROOTS: Mutex<Vec<&'static (dyn Allocative + Sync + 'static)>> = Mutex::new(Vec::new());

/// Register global root which can be later traversed by profiler.
///
/// [`root`](crate::root) macro can be used to register global root.
pub fn register_root(root: &'static (dyn Allocative + Sync + 'static)) {
    ROOTS.lock().unwrap().push(root);
}

pub(crate) fn roots() -> Vec<&'static (dyn Allocative + Sync + 'static)> {
    ROOTS.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use crate as allocative;
    use crate::Allocative;
    use crate::FlameGraphBuilder;

    #[derive(Allocative)]
    struct TestGlobalRoot {
        x: u32,
    }

    #[allocative::root]
    static TEST_GLOBAL_ROOT: TestGlobalRoot = TestGlobalRoot { x: 17 };

    #[test]
    fn test_derive() {
        let mut fg = FlameGraphBuilder::default();
        fg.visit_global_roots();
        let fg = fg.finish_and_write_flame_graph();
        assert!(fg.contains("TestGlobalRoot;x;u32 4"), "{}", fg);
    }
}
