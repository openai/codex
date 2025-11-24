// FIXME: Link below is required to render in index
/// Asserts that the traits support dynamic dispatch
/// ([object-safety](https://doc.rust-lang.org/book/ch17-02-trait-objects.html#object-safety-is-required-for-trait-objects)).
///
/// This is useful for when changes are made to a trait that accidentally
/// prevent it from being used as an [object]. Such a case would be adding a
/// generic method and forgetting to add `where Self: Sized` after it. If left
/// unnoticed, that mistake will affect crate users and break both forward and
/// backward compatibility.
///
/// # Examples
///
/// When exposing a public API, it's important that traits that could previously
/// use dynamic dispatch can still do so in future compatible crate versions.
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// trait MySafeTrait {
///     fn foo(&self) -> u32;
/// }
///
/// assert_obj_safe!(std::fmt::Write, MySafeTrait);
/// ```
///
/// Works with traits that are not in the calling module:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// mod inner {
///     pub trait BasicTrait {
///         fn bar(&self);
///     }
///     assert_obj_safe!(BasicTrait);
/// }
///
/// assert_obj_safe!(inner::BasicTrait);
/// ```
///
/// The following example fails to compile because raw pointers cannot be sent
///  between threads safely:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_impl!(*const u8, Send);
/// ```
///
/// The following example fails to compile because generics without
/// `where Self: Sized` are not allowed in [object-safe][object] trait methods:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// trait MyUnsafeTrait {
///     fn baz<T>(&self) -> T;
/// }
///
/// assert_obj_safe!(MyUnsafeTrait);
/// ```
///
/// When we fix that, the previous code will compile:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// trait MyUnsafeTrait {
///     fn baz<T>(&self) -> T where Self: Sized;
/// }
///
/// assert_obj_safe!(MyUnsafeTrait);
/// ```
///
/// [object]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html#object-safety-is-required-for-trait-objects
#[macro_export]
macro_rules! assert_obj_safe {
    ($($xs:path),+ $(,)?) => {
        $(const _: Option<&$xs> = None;)+
    };
}
