#![allow(renamed_and_removed_lints)] // clippy::cyclomatic_complexity → clippy::cognitive_complexity
#![allow(clippy::cyclomatic_complexity)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::unknown_clippy_lints)]

#[cfg(feature = "use_core")]
extern crate core;

use std::marker::PhantomData;

#[macro_use]
extern crate derivative;

#[derive(PartialEq, Eq, Derivative)]
#[derivative(PartialOrd, Ord)]
struct Foo {
    foo: u8,
}

#[derive(PartialEq, Eq, Derivative)]
#[derivative(
    PartialOrd = "feature_allow_slow_enum",
    Ord = "feature_allow_slow_enum"
)]
enum Option<T> {
    None,
    Some(T),
}

#[derive(Derivative)]
#[derivative(PartialEq, PartialOrd, Ord, Eq)]
struct WithPtr<T: ?Sized> {
    #[derivative(PartialEq(bound = ""))]
    #[derivative(PartialOrd(bound = ""))]
    #[derivative(Ord(bound = ""))]
    #[derivative(Eq(bound = ""))]
    foo: *const T,
}

#[derive(PartialEq, Eq, Derivative)]
#[derivative(PartialOrd, Ord)]
struct Empty;

#[derive(PartialEq, Eq, Derivative)]
#[derivative(PartialOrd, Ord)]
struct AllIgnored {
    #[derivative(PartialOrd = "ignore")]
    #[derivative(Ord = "ignore")]
    foo: u8,
}

#[derive(PartialEq, Eq, Derivative)]
#[derivative(PartialOrd, Ord)]
struct OneIgnored {
    #[derivative(PartialOrd = "ignore")]
    #[derivative(Ord = "ignore")]
    foo: u8,
    bar: u8,
}

#[derive(PartialEq, Eq, Derivative)]
#[derivative(PartialOrd, Ord)]
struct Tenth(
    #[derivative(
        PartialOrd(compare_with = "partial_cmp_tenth"),
        Ord(compare_with = "cmp_tenth")
    )]
    u8,
);

fn partial_cmp_tenth(lhs: &u8, rhs: &u8) -> std::option::Option<std::cmp::Ordering> {
    if *lhs == 0 {
        None
    } else {
        Some((lhs / 10).cmp(&(rhs / 10)))
    }
}
fn cmp_tenth(lhs: &u8, rhs: &u8) -> std::cmp::Ordering {
    (lhs / 10).cmp(&(rhs / 10))
}

#[derive(Derivative)]
#[derivative(PartialOrd, Ord, PartialEq, Eq)]
struct Generic<T>(
    #[derivative(
        PartialEq = "ignore",
        PartialOrd(compare_with = "dummy_partial_cmp", bound = ""),
        Ord(compare_with = "dummy_cmp", bound = "")
    )]
    T,
);

fn dummy_partial_cmp<T>(_: &T, _: &T) -> std::option::Option<std::cmp::Ordering> {
    Some(std::cmp::Ordering::Less)
}
fn dummy_cmp<T>(_: &T, _: &T) -> std::cmp::Ordering {
    std::cmp::Ordering::Less
}

struct NonPartialOrd;

#[derive(Derivative)]
#[derivative(PartialEq, PartialOrd, Ord, Eq)]
struct GenericIgnore<T> {
    f: u32,
    #[derivative(PartialEq = "ignore")]
    #[derivative(PartialOrd = "ignore")]
    #[derivative(Ord = "ignore")]
    t: PhantomData<T>,
}

trait SomeTrait {}
struct SomeType {
    #[allow(dead_code)]
    foo: u8,
}
impl SomeTrait for SomeType {}

#[test]
fn main() {
    use std::cmp::Ordering;

    assert_eq!(
        Foo { foo: 7 }.partial_cmp(&Foo { foo: 42 }),
        Some(Ordering::Less)
    );
    assert_eq!(
        Foo { foo: 42 }.partial_cmp(&Foo { foo: 42 }),
        Some(Ordering::Equal)
    );
    assert_eq!(
        Foo { foo: 42 }.partial_cmp(&Foo { foo: 7 }),
        Some(Ordering::Greater)
    );
    assert_eq!(Foo { foo: 7 }.cmp(&Foo { foo: 42 }), Ordering::Less);
    assert_eq!(Foo { foo: 42 }.cmp(&Foo { foo: 42 }), Ordering::Equal);
    assert_eq!(Foo { foo: 42 }.cmp(&Foo { foo: 7 }), Ordering::Greater);

    let pointers: [*const dyn SomeTrait; 2] = [&SomeType { foo: 1 }, &SomeType { foo: 0 }];
    let ptr1: *const dyn SomeTrait = pointers[0];
    let ptr2: *const dyn SomeTrait = pointers[1];
    let (ptr1, ptr2) = (std::cmp::min(ptr1, ptr2), std::cmp::max(ptr1, ptr2));
    assert_eq!(
        WithPtr { foo: ptr1 }.partial_cmp(&WithPtr { foo: ptr1 }),
        Some(Ordering::Equal)
    );
    assert_eq!(
        WithPtr { foo: ptr1 }.cmp(&WithPtr { foo: ptr1 }),
        Ordering::Equal
    );
    assert_eq!(
        WithPtr { foo: ptr1 }.partial_cmp(&WithPtr { foo: ptr2 }),
        Some(Ordering::Less)
    );
    assert_eq!(
        WithPtr { foo: ptr1 }.cmp(&WithPtr { foo: ptr2 }),
        Ordering::Less
    );

    assert_eq!(Empty.partial_cmp(&Empty), Some(Ordering::Equal));
    assert_eq!(
        AllIgnored { foo: 0 }.partial_cmp(&AllIgnored { foo: 42 }),
        Some(Ordering::Equal)
    );
    assert_eq!(
        OneIgnored { foo: 0, bar: 6 }.partial_cmp(&OneIgnored { foo: 42, bar: 7 }),
        Some(Ordering::Less)
    );
    assert_eq!(
        OneIgnored { foo: 0, bar: 6 }.partial_cmp(&OneIgnored { foo: 42, bar: 6 }),
        Some(Ordering::Equal)
    );
    assert_eq!(
        OneIgnored { foo: 0, bar: 7 }.partial_cmp(&OneIgnored { foo: 42, bar: 6 }),
        Some(Ordering::Greater)
    );
    assert_eq!(Empty.cmp(&Empty), Ordering::Equal);
    assert_eq!(
        AllIgnored { foo: 0 }.cmp(&AllIgnored { foo: 42 }),
        Ordering::Equal
    );
    assert_eq!(
        OneIgnored { foo: 0, bar: 6 }.cmp(&OneIgnored { foo: 42, bar: 7 }),
        Ordering::Less
    );
    assert_eq!(
        OneIgnored { foo: 0, bar: 6 }.cmp(&OneIgnored { foo: 42, bar: 6 }),
        Ordering::Equal
    );
    assert_eq!(
        OneIgnored { foo: 0, bar: 7 }.cmp(&OneIgnored { foo: 42, bar: 6 }),
        Ordering::Greater
    );

    assert_eq!(
        Option::None::<u8>.partial_cmp(&Option::Some(7)),
        Some(Ordering::Less)
    );
    assert_eq!(
        Option::Some(6).partial_cmp(&Option::Some(7)),
        Some(Ordering::Less)
    );
    assert_eq!(
        Option::Some(42).partial_cmp(&Option::Some(42)),
        Some(Ordering::Equal)
    );
    assert_eq!(
        Option::None::<u8>.partial_cmp(&Option::None::<u8>),
        Some(Ordering::Equal)
    );
    assert_eq!(
        Option::Some(7).partial_cmp(&Option::Some(6)),
        Some(Ordering::Greater)
    );
    assert_eq!(
        Option::Some(7).partial_cmp(&Option::None::<u8>),
        Some(Ordering::Greater)
    );
    assert_eq!(Option::None::<u8>.cmp(&Option::Some(7)), Ordering::Less);
    assert_eq!(Option::Some(6).cmp(&Option::Some(7)), Ordering::Less);
    assert_eq!(Option::Some(42).cmp(&Option::Some(42)), Ordering::Equal);
    assert_eq!(Option::None::<u8>.cmp(&Option::None::<u8>), Ordering::Equal);
    assert_eq!(Option::Some(7).cmp(&Option::Some(6)), Ordering::Greater);
    assert_eq!(Option::Some(7).cmp(&Option::None::<u8>), Ordering::Greater);

    assert_eq!(Tenth(0).partial_cmp(&Tenth(67)), None);
    assert_eq!(Tenth(42).partial_cmp(&Tenth(67)), Some(Ordering::Less));
    assert_eq!(Tenth(60).partial_cmp(&Tenth(67)), Some(Ordering::Equal));
    assert_eq!(Tenth(100).partial_cmp(&Tenth(67)), Some(Ordering::Greater));
    assert_eq!(Tenth(42).cmp(&Tenth(67)), Ordering::Less);
    assert_eq!(Tenth(60).cmp(&Tenth(67)), Ordering::Equal);
    assert_eq!(Tenth(100).cmp(&Tenth(67)), Ordering::Greater);

    assert_eq!(
        Generic(SomeType { foo: 0 }).partial_cmp(&Generic(SomeType { foo: 0 })),
        Some(Ordering::Less)
    );
    assert_eq!(
        Generic(SomeType { foo: 0 }).cmp(&Generic(SomeType { foo: 0 })),
        Ordering::Less
    );

    assert_eq!(
        GenericIgnore {
            f: 123,
            t: PhantomData::<NonPartialOrd>::default()
        }
        .cmp(&GenericIgnore {
            f: 123,
            t: PhantomData::<NonPartialOrd>::default()
        }),
        Ordering::Equal
    );
    assert_eq!(
        GenericIgnore {
            f: 123,
            t: PhantomData::<NonPartialOrd>::default()
        }
        .partial_cmp(&GenericIgnore {
            f: 123,
            t: PhantomData::<NonPartialOrd>::default()
        }),
        Some(Ordering::Equal)
    );
}
