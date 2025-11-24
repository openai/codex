#![deny(warnings)]

#[macro_use]
extern crate lazy_static;
use std::collections::HashMap;

lazy_static! {
    /// Documentation!
    pub static ref NUMBER: u32 = times_two(3);

    static ref ARRAY_BOXES: [Box<u32>; 3] = [Box::new(1), Box::new(2), Box::new(3)];

    /// More documentation!
    #[allow(unused_variables)]
    #[derive(Copy, Clone, Debug)]
    pub static ref STRING: String = "hello".to_string();

    static ref HASHMAP: HashMap<u32, &'static str> = {
        let mut m = HashMap::new();
        m.insert(0, "abc");
        m.insert(1, "def");
        m.insert(2, "ghi");
        m
    };

    // This should not compile if the unsafe is removed.
    static ref UNSAFE: u32 = unsafe {
        std::mem::transmute::<i32, u32>(-1)
    };
}

lazy_static! {
    static ref S1: &'static str = "a";
    static ref S2: &'static str = "b";
}
lazy_static! {
    static ref S3: String = [*S1, *S2].join("");
}

lazy_static! {
    #[allow(non_upper_case_globals)]
    pub static ref string: String = "hello".to_string();
}

#[test]
fn s3() {
    assert_eq!(&*S3, "ab");
}

fn times_two(n: u32) -> u32 {
    n * 2
}

#[test]
fn test_basic() {
    assert_eq!(&**STRING, "hello");
    assert_eq!(*NUMBER, 6);
    assert!(HASHMAP.get(&1).is_some());
    assert!(HASHMAP.get(&3).is_none());
    assert_eq!(&*ARRAY_BOXES, &[Box::new(1), Box::new(2), Box::new(3)]);
    assert_eq!(*UNSAFE, std::u32::MAX);
}

#[test]
fn test_repeat() {
    assert_eq!(*NUMBER, 6);
    assert_eq!(*NUMBER, 6);
    assert_eq!(*NUMBER, 6);
}

#[test]
fn test_meta() {
    // this would not compile if STRING were not marked #[derive(Copy, Clone)]
    let copy_of_string = STRING;
    // just to make sure it was copied
    assert!(&STRING as *const _ != &copy_of_string as *const _);

    // this would not compile if STRING were not marked #[derive(Debug)]
    assert_eq!(
        format!("{:?}", STRING),
        "STRING { __private_field: () }".to_string()
    );
}

mod visibility {
    lazy_static! {
        pub static ref FOO: Box<u32> = Box::new(0);
        static ref BAR: Box<u32> = Box::new(98);
    }

    pub mod inner {
        lazy_static! {
            pub(in visibility) static ref BAZ: Box<u32> = Box::new(42);
            pub(crate) static ref BAG: Box<u32> = Box::new(37);
        }
    }

    #[test]
    fn sub_test() {
        assert_eq!(**FOO, 0);
        assert_eq!(**BAR, 98);
        assert_eq!(**inner::BAZ, 42);
        assert_eq!(**inner::BAG, 37);
    }
}

#[test]
fn test_visibility() {
    assert_eq!(*visibility::FOO, Box::new(0));
    assert_eq!(*visibility::inner::BAG, Box::new(37));
}

// This should not cause a warning about a missing Copy implementation
lazy_static! {
    pub static ref VAR: i32 = 0;
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct X;
struct Once(X);
const ONCE_INIT: Once = Once(X);
static DATA: X = X;
static ONCE: X = X;
fn require_sync() -> X {
    X
}
fn transmute() -> X {
    X
}
fn __static_ref_initialize() -> X {
    X
}
fn test(_: Vec<X>) -> X {
    X
}

// All these names should not be shadowed
lazy_static! {
    static ref ITEM_NAME_TEST: X = {
        test(vec![X, Once(X).0, ONCE_INIT.0, DATA, ONCE,
                  require_sync(), transmute(),
                  // Except this, which will sadly be shadowed by internals:
                  // __static_ref_initialize()
                  ])
    };
}

#[test]
fn item_name_shadowing() {
    assert_eq!(*ITEM_NAME_TEST, X);
}

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
#[allow(deprecated)]
use std::sync::atomic::ATOMIC_BOOL_INIT;

#[allow(deprecated)]
static PRE_INIT_FLAG: AtomicBool = ATOMIC_BOOL_INIT;

lazy_static! {
    static ref PRE_INIT: () = {
        PRE_INIT_FLAG.store(true, SeqCst);
        ()
    };
}

#[test]
fn pre_init() {
    assert_eq!(PRE_INIT_FLAG.load(SeqCst), false);
    lazy_static::initialize(&PRE_INIT);
    assert_eq!(PRE_INIT_FLAG.load(SeqCst), true);
}

lazy_static! {
    static ref LIFETIME_NAME: for<'a> fn(&'a u8) = {
        fn f(_: &u8) {}
        f
    };
}

#[test]
fn lifetime_name() {
    let _ = LIFETIME_NAME;
}
