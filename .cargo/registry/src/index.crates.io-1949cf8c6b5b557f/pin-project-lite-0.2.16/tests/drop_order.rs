// SPDX-License-Identifier: Apache-2.0 OR MIT

// Refs: https://doc.rust-lang.org/reference/destructors.html

use std::{cell::Cell, panic, pin::Pin, thread};

use pin_project_lite::pin_project;

struct D<'a>(&'a Cell<usize>, usize);

impl Drop for D<'_> {
    fn drop(&mut self) {
        if !thread::panicking() {
            let old = self.0.replace(self.1);
            assert_eq!(old, self.1 - 1);
        }
    }
}

pin_project! {
#[project = StructPinnedProj]
#[project_ref = StructPinnedProjRef]
#[project_replace = StructPinnedProjReplace]
struct StructPinned<'a> {
    #[pin]
    f1: D<'a>,
    #[pin]
    f2: D<'a>,
}
}

pin_project! {
#[project = StructUnpinnedProj]
#[project_ref = StructUnpinnedProjRef]
#[project_replace = StructUnpinnedProjReplace]
struct StructUnpinned<'a> {
    f1: D<'a>,
    f2: D<'a>,
}
}

pin_project! {
#[project_replace = EnumProjReplace]
enum Enum<'a> {
    #[allow(dead_code)] // false positive that fixed in Rust 1.38
    StructPinned {
        #[pin]
        f1: D<'a>,
        #[pin]
        f2: D<'a>,
    },
    #[allow(dead_code)] // false positive that fixed in Rust 1.38
    StructUnpinned {
        f1: D<'a>,
        f2: D<'a>,
    },
}
}

#[test]
fn struct_pinned() {
    {
        let c = Cell::new(0);
        let _x = StructPinned { f1: D(&c, 1), f2: D(&c, 2) };
    }
    {
        let c = Cell::new(0);
        let mut x = StructPinned { f1: D(&c, 1), f2: D(&c, 2) };
        let y = Pin::new(&mut x);
        let _z = y.project_replace(StructPinned { f1: D(&c, 3), f2: D(&c, 4) });
    }
}

#[test]
fn struct_unpinned() {
    {
        let c = Cell::new(0);
        let _x = StructUnpinned { f1: D(&c, 1), f2: D(&c, 2) };
    }
    {
        let c = Cell::new(0);
        let mut x = StructUnpinned { f1: D(&c, 1), f2: D(&c, 2) };
        let y = Pin::new(&mut x);
        let _z = y.project_replace(StructUnpinned { f1: D(&c, 3), f2: D(&c, 4) });
    }
}

#[test]
fn enum_struct() {
    {
        let c = Cell::new(0);
        let _x = Enum::StructPinned { f1: D(&c, 1), f2: D(&c, 2) };
    }
    {
        let c = Cell::new(0);
        let mut x = Enum::StructPinned { f1: D(&c, 1), f2: D(&c, 2) };
        let y = Pin::new(&mut x);
        let _z = y.project_replace(Enum::StructPinned { f1: D(&c, 3), f2: D(&c, 4) });
    }

    {
        let c = Cell::new(0);
        let _x = Enum::StructUnpinned { f1: D(&c, 1), f2: D(&c, 2) };
    }
    {
        let c = Cell::new(0);
        let mut x = Enum::StructUnpinned { f1: D(&c, 1), f2: D(&c, 2) };
        let y = Pin::new(&mut x);
        let _z = y.project_replace(Enum::StructUnpinned { f1: D(&c, 3), f2: D(&c, 4) });
    }
}

// https://github.com/rust-lang/rust/issues/47949
// https://github.com/taiki-e/pin-project/pull/194#discussion_r419098111
#[allow(clippy::many_single_char_names)]
#[test]
fn project_replace_panic() {
    pin_project! {
    #[project_replace = SProjReplace]
    struct S<T, U> {
        #[pin]
        pinned: T,
        unpinned: U,
    }
    }

    struct D<'a>(&'a mut bool, bool);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            *self.0 = true;
            if self.1 {
                panic!();
            }
        }
    }

    let (mut a, mut b, mut c, mut d) = (false, false, false, false);
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let mut x = S { pinned: D(&mut a, true), unpinned: D(&mut b, false) };
        let _y = Pin::new(&mut x)
            .project_replace(S { pinned: D(&mut c, false), unpinned: D(&mut d, false) });
        // Previous `x.pinned` was dropped and panicked when `project_replace` is
        // called, so this is unreachable.
        unreachable!();
    }));
    assert!(res.is_err());
    assert!(a);
    assert!(b);
    assert!(c);
    assert!(d);

    let (mut a, mut b, mut c, mut d) = (false, false, false, false);
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let mut x = S { pinned: D(&mut a, false), unpinned: D(&mut b, true) };
        {
            let _y = Pin::new(&mut x)
                .project_replace(S { pinned: D(&mut c, false), unpinned: D(&mut d, false) });
            // `_y` (previous `x.unpinned`) live to the end of this scope, so
            // this is not unreachable.
            // unreachable!();
        }
        unreachable!();
    }));
    assert!(res.is_err());
    assert!(a);
    assert!(b);
    assert!(c);
    assert!(d);
}
