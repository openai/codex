#![warn(missing_docs)]
#![warn(unused_results)]
#![doc(html_root_url="https://docs.rs/maplit/1/")]

//! Macros for container literals with specific type.
//!
//! ```
//! #[macro_use] extern crate maplit;
//!
//! # fn main() {
//! let map = hashmap!{
//!     "a" => 1,
//!     "b" => 2,
//! };
//! # }
//! ```
//!
//! The **maplit** crate uses `=>` syntax to separate the key and value for the
//! mapping macros. (It was not possible to use `:` as separator due to syntactic
//! restrictions in regular `macro_rules!` macros.)
//!
//! Note that rust macros are flexible in which brackets you use for the invocation.
//! You can use them as `hashmap!{}` or `hashmap![]` or `hashmap!()`.
//!
//! Generic container macros already exist elsewhere, so those are not provided
//! here at the moment.

#[macro_export(local_inner_macros)]
/// Create a **HashMap** from a list of key-value pairs
///
/// ## Example
///
/// ```
/// #[macro_use] extern crate maplit;
/// # fn main() {
///
/// let map = hashmap!{
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// assert_eq!(map.get("c"), None);
/// # }
/// ```
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = hashmap!(@count $($key),*);
            let mut _map = ::std::collections::HashMap::with_capacity(_cap);
            $(
                let _ = _map.insert($key, $value);
            )*
            _map
        }
    };
}

/// Create a **HashSet** from a list of elements.
///
/// ## Example
///
/// ```
/// #[macro_use] extern crate maplit;
/// # fn main() {
///
/// let set = hashset!{"a", "b"};
/// assert!(set.contains("a"));
/// assert!(set.contains("b"));
/// assert!(!set.contains("c"));
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! hashset {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashset!(@single $rest)),*]));

    ($($key:expr,)+) => { hashset!($($key),+) };
    ($($key:expr),*) => {
        {
            let _cap = hashset!(@count $($key),*);
            let mut _set = ::std::collections::HashSet::with_capacity(_cap);
            $(
                let _ = _set.insert($key);
            )*
            _set
        }
    };
}

#[macro_export(local_inner_macros)]
/// Create a **BTreeMap** from a list of key-value pairs
///
/// ## Example
///
/// ```
/// #[macro_use] extern crate maplit;
/// # fn main() {
///
/// let map = btreemap!{
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// assert_eq!(map.get("c"), None);
/// # }
/// ```
macro_rules! btreemap {
    // trailing comma case
    ($($key:expr => $value:expr,)+) => (btreemap!($($key => $value),+));

    ( $($key:expr => $value:expr),* ) => {
        {
            let mut _map = ::std::collections::BTreeMap::new();
            $(
                let _ = _map.insert($key, $value);
            )*
            _map
        }
    };
}

#[macro_export(local_inner_macros)]
/// Create a **BTreeSet** from a list of elements.
///
/// ## Example
///
/// ```
/// #[macro_use] extern crate maplit;
/// # fn main() {
///
/// let set = btreeset!{"a", "b"};
/// assert!(set.contains("a"));
/// assert!(set.contains("b"));
/// assert!(!set.contains("c"));
/// # }
/// ```
macro_rules! btreeset {
    ($($key:expr,)+) => (btreeset!($($key),+));

    ( $($key:expr),* ) => {
        {
            let mut _set = ::std::collections::BTreeSet::new();
            $(
                _set.insert($key);
            )*
            _set
        }
    };
}

/// Identity function. Used as the fallback for conversion.
#[doc(hidden)]
pub fn __id<T>(t: T) -> T { t }

/// Macro that converts the keys or key-value pairs passed to another maplit
/// macro. The default conversion is to use the [`Into`] trait, if no
/// custom conversion is passed.
///
/// The syntax is:
///
/// `convert_args!(` `keys=` *function* `,` `values=` *function* `,`
///     *macro_name* `!(` [ *key* => *value* [, *key* => *value* ... ] ] `))`
///
/// Here *macro_name* is any other maplit macro and either or both of the
/// explicit `keys=` and `values=` parameters can be omitted.
///
/// [`Into`]: https://doc.rust-lang.org/std/convert/trait.Into.html
///
/// **Note** To use `convert_args`, the macro that is being wrapped
/// must itself be brought into the current scope with `#[macro_use]` or `use`.
///
/// # Examples
///
/// ```
/// #[macro_use] extern crate maplit;
/// # fn main() {
///
/// use std::collections::HashMap;
/// use std::collections::BTreeSet;
///
/// // a. Use the default conversion with the Into trait.
/// // Here this converts both the key and value string literals to `String`,
/// // but we need to specify the map type exactly!
///
/// let map1: HashMap<String, String> = convert_args!(hashmap!(
///     "a" => "b",
///     "c" => "d",
/// ));
///
/// // b. Specify an explicit custom conversion for the keys. If we don't specify
/// // a conversion for the values, they are not converted at all.
///
/// let map2 = convert_args!(keys=String::from, hashmap!(
///     "a" => 1,
///     "c" => 2,
/// ));
///
/// // Note: map2 is a HashMap<String, i32>, but we didn't need to specify the type
/// let _: HashMap<String, i32> = map2;
///
/// // c. convert_args! works with all the maplit macros -- and macros from other
/// // crates that have the same "signature".
/// // For example, btreeset and conversion from &str to Vec<u8>.
///
/// let set: BTreeSet<Vec<u8>> = convert_args!(btreeset!(
///     "a", "b", "c", "d", "a", "e", "f",
/// ));
/// assert_eq!(set.len(), 6);
///
///
/// # }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! convert_args {
    (keys=$kf:expr, $macro_name:ident !($($k:expr),* $(,)*)) => {
        $macro_name! { $(($kf)($k)),* }
    };
    (keys=$kf:expr, values=$vf:expr, $macro_name:ident !($($k:expr),* $(,)*)) => {
        $macro_name! { $(($kf)($k)),* }
    };
    (keys=$kf:expr, values=$vf:expr, $macro_name:ident !( $($k:expr => $v:expr),* $(,)*)) => {
        $macro_name! { $(($kf)($k) => ($vf)($v)),* }
    };
    (keys=$kf:expr, $macro_name:ident !($($rest:tt)*)) => {
        convert_args! {
            keys=$kf, values=$crate::__id,
            $macro_name !(
                $($rest)*
            )
        }
    };
    (values=$vf:expr, $macro_name:ident !($($rest:tt)*)) => {
        convert_args! {
            keys=$crate::__id, values=$vf,
            $macro_name !(
                $($rest)*
            )
        }
    };
    ($macro_name:ident ! $($rest:tt)*) => {
        convert_args! {
            keys=::std::convert::Into::into, values=::std::convert::Into::into,
            $macro_name !
            $($rest)*
        }
    };
}

#[test]
fn test_hashmap() {
    use std::collections::HashMap;
    use std::collections::HashSet;
    let names = hashmap!{
        1 => "one",
        2 => "two",
    };
    assert_eq!(names.len(), 2);
    assert_eq!(names[&1], "one");
    assert_eq!(names[&2], "two");
    assert_eq!(names.get(&3), None);

    let empty: HashMap<i32, i32> = hashmap!{};
    assert_eq!(empty.len(), 0);

    let _nested_compiles = hashmap!{
        1 => hashmap!{0 => 1 + 2,},
        2 => hashmap!{1 => 1,},
    };

    let _: HashMap<String, i32> = convert_args!(keys=String::from, hashmap!(
        "one" => 1,
        "two" => 2,
    ));

    let _: HashMap<String, i32> = convert_args!(keys=String::from, values=__id, hashmap!(
        "one" => 1,
        "two" => 2,
    ));

    let names: HashSet<String> = convert_args!(hashset!(
        "one",
        "two",
    ));
    assert!(names.contains("one"));
    assert!(names.contains("two"));

    let lengths: HashSet<usize> = convert_args!(keys=str::len, hashset!(
        "one",
        "two",
    ));
    assert_eq!(lengths.len(), 1);

    let _no_trailing: HashSet<usize> = convert_args!(keys=str::len, hashset!(
        "one",
        "two"
    ));
}

#[test]
fn test_btreemap() {
    use std::collections::BTreeMap;
    let names = btreemap!{
        1 => "one",
        2 => "two",
    };
    assert_eq!(names.len(), 2);
    assert_eq!(names[&1], "one");
    assert_eq!(names[&2], "two");
    assert_eq!(names.get(&3), None);

    let empty: BTreeMap<i32, i32> = btreemap!{};
    assert_eq!(empty.len(), 0);

    let _nested_compiles = btreemap!{
        1 => btreemap!{0 => 1 + 2,},
        2 => btreemap!{1 => 1,},
    };
}
