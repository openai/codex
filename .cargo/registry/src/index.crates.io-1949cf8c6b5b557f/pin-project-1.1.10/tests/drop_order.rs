// SPDX-License-Identifier: Apache-2.0 OR MIT

// Refs: https://doc.rust-lang.org/reference/destructors.html

use std::{cell::Cell, pin::Pin, thread};

use pin_project::pin_project;

struct D<'a>(&'a Cell<usize>, usize);

impl Drop for D<'_> {
    fn drop(&mut self) {
        if !thread::panicking() {
            let old = self.0.replace(self.1);
            assert_eq!(old, self.1 - 1);
        }
    }
}

#[pin_project(project_replace)]
struct StructPinned<'a> {
    #[pin]
    f1: D<'a>,
    #[pin]
    f2: D<'a>,
}

#[pin_project(project_replace)]
struct StructUnpinned<'a> {
    f1: D<'a>,
    f2: D<'a>,
}

#[pin_project(project_replace)]
struct TuplePinned<'a>(#[pin] D<'a>, #[pin] D<'a>);

#[pin_project(project_replace)]
struct TupleUnpinned<'a>(D<'a>, D<'a>);

#[pin_project(project_replace = EnumProj)]
enum Enum<'a> {
    StructPinned {
        #[pin]
        f1: D<'a>,
        #[pin]
        f2: D<'a>,
    },
    StructUnpinned {
        f1: D<'a>,
        f2: D<'a>,
    },
    TuplePinned(#[pin] D<'a>, #[pin] D<'a>),
    TupleUnpinned(D<'a>, D<'a>),
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
fn tuple_pinned() {
    {
        let c = Cell::new(0);
        let _x = TuplePinned(D(&c, 1), D(&c, 2));
    }
    {
        let c = Cell::new(0);
        let mut x = TuplePinned(D(&c, 1), D(&c, 2));
        let y = Pin::new(&mut x);
        let _z = y.project_replace(TuplePinned(D(&c, 3), D(&c, 4)));
    }
}

#[test]
fn tuple_unpinned() {
    {
        let c = Cell::new(0);
        let _x = TupleUnpinned(D(&c, 1), D(&c, 2));
    }
    {
        let c = Cell::new(0);
        let mut x = TupleUnpinned(D(&c, 1), D(&c, 2));
        let y = Pin::new(&mut x);
        let _z = y.project_replace(TupleUnpinned(D(&c, 3), D(&c, 4)));
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

#[test]
fn enum_tuple() {
    {
        let c = Cell::new(0);
        let _x = Enum::TuplePinned(D(&c, 1), D(&c, 2));
    }
    {
        let c = Cell::new(0);
        let mut x = Enum::TuplePinned(D(&c, 1), D(&c, 2));
        let y = Pin::new(&mut x);
        let _z = y.project_replace(Enum::TuplePinned(D(&c, 3), D(&c, 4)));
    }

    {
        let c = Cell::new(0);
        let _x = Enum::TupleUnpinned(D(&c, 1), D(&c, 2));
    }
    {
        let c = Cell::new(0);
        let mut x = Enum::TupleUnpinned(D(&c, 1), D(&c, 2));
        let y = Pin::new(&mut x);
        let _z = y.project_replace(Enum::TupleUnpinned(D(&c, 3), D(&c, 4)));
    }
}
