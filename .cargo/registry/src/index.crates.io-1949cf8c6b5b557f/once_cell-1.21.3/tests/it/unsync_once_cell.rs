use core::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

use once_cell::unsync::OnceCell;

#[test]
fn once_cell() {
    let c = OnceCell::new();
    assert!(c.get().is_none());
    c.get_or_init(|| 92);
    assert_eq!(c.get(), Some(&92));

    c.get_or_init(|| panic!("Kabom!"));
    assert_eq!(c.get(), Some(&92));
}

#[test]
fn once_cell_with_value() {
    const CELL: OnceCell<i32> = OnceCell::with_value(12);
    let cell = CELL;
    assert_eq!(cell.get(), Some(&12));
}

#[test]
fn once_cell_get_mut() {
    let mut c = OnceCell::new();
    assert!(c.get_mut().is_none());
    c.set(90).unwrap();
    *c.get_mut().unwrap() += 2;
    assert_eq!(c.get_mut(), Some(&mut 92));
}

#[test]
fn once_cell_drop() {
    static DROP_CNT: AtomicUsize = AtomicUsize::new(0);
    struct Dropper;
    impl Drop for Dropper {
        fn drop(&mut self) {
            DROP_CNT.fetch_add(1, SeqCst);
        }
    }

    let x = OnceCell::new();
    x.get_or_init(|| Dropper);
    assert_eq!(DROP_CNT.load(SeqCst), 0);
    drop(x);
    assert_eq!(DROP_CNT.load(SeqCst), 1);
}

#[test]
fn once_cell_drop_empty() {
    let x = OnceCell::<String>::new();
    drop(x);
}

#[test]
fn clone() {
    let s = OnceCell::new();
    let c = s.clone();
    assert!(c.get().is_none());

    s.set("hello".to_string()).unwrap();
    let c = s.clone();
    assert_eq!(c.get().map(String::as_str), Some("hello"));
}

#[test]
fn get_or_try_init() {
    let cell: OnceCell<String> = OnceCell::new();
    assert!(cell.get().is_none());

    let res = std::panic::catch_unwind(|| cell.get_or_try_init(|| -> Result<_, ()> { panic!() }));
    assert!(res.is_err());
    assert!(cell.get().is_none());

    assert_eq!(cell.get_or_try_init(|| Err(())), Err(()));

    assert_eq!(cell.get_or_try_init(|| Ok::<_, ()>("hello".to_string())), Ok(&"hello".to_string()));
    assert_eq!(cell.get(), Some(&"hello".to_string()));
}

#[test]
fn from_impl() {
    assert_eq!(OnceCell::from("value").get(), Some(&"value"));
    assert_ne!(OnceCell::from("foo").get(), Some(&"bar"));
}

#[test]
fn partialeq_impl() {
    assert!(OnceCell::from("value") == OnceCell::from("value"));
    assert!(OnceCell::from("foo") != OnceCell::from("bar"));

    assert!(OnceCell::<String>::new() == OnceCell::new());
    assert!(OnceCell::<String>::new() != OnceCell::from("value".to_owned()));
}

#[test]
fn into_inner() {
    let cell: OnceCell<String> = OnceCell::new();
    assert_eq!(cell.into_inner(), None);
    let cell = OnceCell::new();
    cell.set("hello".to_string()).unwrap();
    assert_eq!(cell.into_inner(), Some("hello".to_string()));
}

#[test]
fn debug_impl() {
    let cell = OnceCell::new();
    assert_eq!(format!("{:#?}", cell), "OnceCell(Uninit)");
    cell.set(vec!["hello", "world"]).unwrap();
    assert_eq!(
        format!("{:#?}", cell),
        r#"OnceCell(
    [
        "hello",
        "world",
    ],
)"#
    );
}

#[test]
#[should_panic(expected = "reentrant init")]
fn reentrant_init() {
    let x: OnceCell<Box<i32>> = OnceCell::new();
    let dangling_ref: Cell<Option<&i32>> = Cell::new(None);
    x.get_or_init(|| {
        let r = x.get_or_init(|| Box::new(92));
        dangling_ref.set(Some(r));
        Box::new(62)
    });
    eprintln!("use after free: {:?}", dangling_ref.get().unwrap());
}

#[test]
fn aliasing_in_get() {
    let x = OnceCell::new();
    x.set(42).unwrap();
    let at_x = x.get().unwrap(); // --- (shared) borrow of inner `Option<T>` --+
    let _ = x.set(27); // <-- temporary (unique) borrow of inner `Option<T>`   |
    println!("{}", at_x); // <------- up until here ---------------------------+
}

#[test]
// https://github.com/rust-lang/rust/issues/34761#issuecomment-256320669
fn arrrrrrrrrrrrrrrrrrrrrr() {
    let cell = OnceCell::new();
    {
        let s = String::new();
        cell.set(&s).unwrap();
    }
}
