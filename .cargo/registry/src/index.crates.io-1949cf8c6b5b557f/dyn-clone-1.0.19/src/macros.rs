use crate::DynClone;

/// Implement the standard library `Clone` for a trait object that has
/// `DynClone` as a supertrait.
///
/// ```
/// use dyn_clone::DynClone;
///
/// trait MyTrait: DynClone {
///     /* ... */
/// }
///
/// dyn_clone::clone_trait_object!(MyTrait);
///
/// // Now data structures containing Box<dyn MyTrait> can derive Clone.
/// #[derive(Clone)]
/// struct Container {
///     trait_object: Box<dyn MyTrait>,
/// }
/// ```
///
/// The macro supports traits that have type parameters and/or `where` clauses.
///
/// ```
/// use dyn_clone::DynClone;
/// use std::io::Read;
///
/// trait Difficult<R>: DynClone where R: Read {
///     /* ... */
/// }
///
/// dyn_clone::clone_trait_object!(<R> Difficult<R> where R: Read);
/// ```
#[macro_export]
macro_rules! clone_trait_object {
    ($($path:tt)+) => {
        $crate::__internal_clone_trait_object!(begin $($path)+);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __internal_clone_trait_object {
    // Invocation started with `<`, parse generics.
    (begin < $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(generics () () $($rest)*);
    };

    // Invocation did not start with `<`.
    (begin $first:tt $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(path () ($first) $($rest)*);
    };

    // End of generics.
    (generics ($($generics:tt)*) () > $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(path ($($generics)*) () $($rest)*);
    };

    // Generics open bracket.
    (generics ($($generics:tt)*) ($($brackets:tt)*) < $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(generics ($($generics)* <) ($($brackets)* <) $($rest)*);
    };

    // Generics close bracket.
    (generics ($($generics:tt)*) (< $($brackets:tt)*) > $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(generics ($($generics)* >) ($($brackets)*) $($rest)*);
    };

    // Token inside of generics.
    (generics ($($generics:tt)*) ($($brackets:tt)*) $first:tt $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(generics ($($generics)* $first) ($($brackets)*) $($rest)*);
    };

    // End with `where` clause.
    (path ($($generics:tt)*) ($($path:tt)*) where $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(impl ($($generics)*) ($($path)*) ($($rest)*));
    };

    // End without `where` clause.
    (path ($($generics:tt)*) ($($path:tt)*)) => {
        $crate::__internal_clone_trait_object!(impl ($($generics)*) ($($path)*) ());
    };

    // Token inside of path.
    (path ($($generics:tt)*) ($($path:tt)*) $first:tt $($rest:tt)*) => {
        $crate::__internal_clone_trait_object!(path ($($generics)*) ($($path)* $first) $($rest)*);
    };

    // The impl.
    (impl ($($generics:tt)*) ($($path:tt)*) ($($bound:tt)*)) => {
        #[allow(unknown_lints, non_local_definitions)] // false positive: https://github.com/rust-lang/rust/issues/121621
        impl<'clone, $($generics)*> $crate::__private::Clone for $crate::__private::Box<dyn $($path)* + 'clone> where $($bound)* {
            fn clone(&self) -> Self {
                $crate::clone_box(&**self)
            }
        }

        #[allow(unknown_lints, non_local_definitions)]
        impl<'clone, $($generics)*> $crate::__private::Clone for $crate::__private::Box<dyn $($path)* + $crate::__private::Send + 'clone> where $($bound)* {
            fn clone(&self) -> Self {
                $crate::clone_box(&**self)
            }
        }

        #[allow(unknown_lints, non_local_definitions)]
        impl<'clone, $($generics)*> $crate::__private::Clone for $crate::__private::Box<dyn $($path)* + $crate::__private::Sync + 'clone> where $($bound)* {
            fn clone(&self) -> Self {
                $crate::clone_box(&**self)
            }
        }

        #[allow(unknown_lints, non_local_definitions)]
        impl<'clone, $($generics)*> $crate::__private::Clone for $crate::__private::Box<dyn $($path)* + $crate::__private::Send + $crate::__private::Sync + 'clone> where $($bound)* {
            fn clone(&self) -> Self {
                $crate::clone_box(&**self)
            }
        }
    };
}

clone_trait_object!(DynClone);
