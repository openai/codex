/// Creates a [`Vec`] containing the arguments.
///
/// `vec!` allows `Vec`s to be defined with the same syntax as array expressions.
/// There are two forms of this macro:
///
/// - Create a [`Vec`] containing a given list of elements:
///
/// ```
/// use allocator_api2::vec;
/// let v = vec![1, 2, 3];
/// assert_eq!(v[0], 1);
/// assert_eq!(v[1], 2);
/// assert_eq!(v[2], 3);
/// ```
///
///
/// ```
/// use allocator_api2::{vec, alloc::Global};
/// let v = vec![in Global; 1, 2, 3];
/// assert_eq!(v[0], 1);
/// assert_eq!(v[1], 2);
/// assert_eq!(v[2], 3);
/// ```
///
/// - Create a [`Vec`] from a given element and size:
///
/// ```
/// use allocator_api2::vec;
/// let v = vec![1; 3];
/// assert_eq!(v, [1, 1, 1]);
/// ```
///
/// ```
/// use allocator_api2::{vec, alloc::Global};
/// let v = vec![in Global; 1; 3];
/// assert_eq!(v, [1, 1, 1]);
/// ```
///
/// Note that unlike array expressions this syntax supports all elements
/// which implement [`Clone`] and the number of elements doesn't have to be
/// a constant.
///
/// This will use `clone` to duplicate an expression, so one should be careful
/// using this with types having a nonstandard `Clone` implementation. For
/// example, `vec![Rc::new(1); 5]` will create a vector of five references
/// to the same boxed integer value, not five references pointing to independently
/// boxed integers.
///
/// Also, note that `vec![expr; 0]` is allowed, and produces an empty vector.
/// This will still evaluate `expr`, however, and immediately drop the resulting value, so
/// be mindful of side effects.
///
/// [`Vec`]: crate::vec::Vec
#[cfg(not(no_global_oom_handling))]
#[macro_export]
macro_rules! vec {
    (in $alloc:expr $(;)?) => (
        $crate::vec::Vec::new_in($alloc)
    );
    (in $alloc:expr; $elem:expr; $n:expr) => (
        $crate::vec::from_elem_in($elem, $n, $alloc)
    );
    (in $alloc:expr; $($x:expr),+ $(,)?) => (
        $crate::boxed::Box::<[_]>::into_vec(
            $crate::boxed::Box::slice(
                $crate::boxed::Box::new_in([$($x),+], $alloc)
            )
        )
    );
    () => (
        $crate::vec::Vec::new()
    );
    ($elem:expr; $n:expr) => (
        $crate::vec::from_elem($elem, $n)
    );
    ($($x:expr),+ $(,)?) => (
        $crate::boxed::Box::<[_]>::into_vec(
            $crate::boxed::Box::slice(
                $crate::boxed::Box::new([$($x),+])
            )
        )
    );
}
