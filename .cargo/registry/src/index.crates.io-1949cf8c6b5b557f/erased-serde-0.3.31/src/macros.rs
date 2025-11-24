/// Implement `serde::Serialize` for a trait object that has
/// `erased_serde::Serialize` as a supertrait.
///
/// ```
/// use erased_serde::serialize_trait_object;
///
/// trait Event: erased_serde::Serialize {
///     /* ... */
/// }
///
/// erased_serde::serialize_trait_object!(Event);
/// ```
///
/// The macro supports traits that have type parameters and/or `where` clauses.
///
/// ```
/// # use erased_serde::serialize_trait_object;
/// #
/// trait Difficult<T>: erased_serde::Serialize where T: Copy {
///     /* ... */
/// }
///
/// serialize_trait_object!(<T> Difficult<T> where T: Copy);
/// ```
#[macro_export]
macro_rules! serialize_trait_object {
    ($($path:tt)+) => {
        $crate::__internal_serialize_trait_object!(begin $($path)+);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __internal_serialize_trait_object {
    // Invocation started with `<`, parse generics.
    (begin < $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(generics () () $($rest)*);
    };

    // Invocation did not start with `<`.
    (begin $first:tt $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(path () ($first) $($rest)*);
    };

    // End of generics with trailing comma.
    (generics ($($generics:tt)*) () , > $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(path ($($generics)* ,) () $($rest)*);
    };

    // End of generics without trailing comma.
    (generics ($($generics:tt)*) () > $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(path ($($generics)* ,) () $($rest)*);
    };

    // Generics open bracket.
    (generics ($($generics:tt)*) ($($brackets:tt)*) < $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(generics ($($generics)* <) ($($brackets)* <) $($rest)*);
    };

    // Generics close bracket.
    (generics ($($generics:tt)*) (< $($brackets:tt)*) > $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(generics ($($generics)* >) ($($brackets)*) $($rest)*);
    };

    // Token inside of generics.
    (generics ($($generics:tt)*) ($($brackets:tt)*) $first:tt $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(generics ($($generics)* $first) ($($brackets)*) $($rest)*);
    };

    // End with `where` clause.
    (path ($($generics:tt)*) ($($path:tt)*) where $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(sendsync ($($generics)*) ($($path)*) ($($rest)*));
    };

    // End without `where` clause.
    (path ($($generics:tt)*) ($($path:tt)*)) => {
        $crate::__internal_serialize_trait_object!(sendsync ($($generics)*) ($($path)*) ());
    };

    // Token inside of path.
    (path ($($generics:tt)*) ($($path:tt)*) $first:tt $($rest:tt)*) => {
        $crate::__internal_serialize_trait_object!(path ($($generics)*) ($($path)* $first) $($rest)*);
    };

    // Expand into four impls.
    (sendsync ($($generics:tt)*) ($($path:tt)*) ($($bound:tt)*)) => {
        $crate::__internal_serialize_trait_object!(impl ($($generics)*) ($($path)*) ($($bound)*) {
            fn __check_erased_serialize_supertrait<$($generics)* __T>()
            where
                __T: ?$crate::__private::Sized + $($path)*,
                $($bound)*
            {
                $crate::__private::require_erased_serialize_impl::<__T>();
            }
        });
        $crate::__internal_serialize_trait_object!(impl ($($generics)*) ($($path)* + $crate::__private::Send) ($($bound)*));
        $crate::__internal_serialize_trait_object!(impl ($($generics)*) ($($path)* + $crate::__private::Sync) ($($bound)*));
        $crate::__internal_serialize_trait_object!(impl ($($generics)*) ($($path)* + $crate::__private::Send + $crate::__private::Sync) ($($bound)*));
    };

    // The impl.
    (impl ($($generics:tt)*) ($($path:tt)*) ($($bound:tt)*) $({$($body:tt)*})*) => {
        impl<'erased, $($generics)*> $crate::__private::serde::Serialize for dyn $($path)* + 'erased
        where
            $($bound)*
        {
            fn serialize<S>(&self, serializer: S) -> $crate::__private::Result<S::Ok, S::Error>
            where
                S: $crate::__private::serde::Serializer,
            {
                $($($body)*)*
                $crate::serialize(self, serializer)
            }
        }
    };
}

// TEST ////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::Serialize;

    fn assert_serialize<T: ?Sized + serde::Serialize>() {}

    #[test]
    fn test_plain() {
        trait Trait: Serialize {}

        serialize_trait_object!(Trait);
        assert_serialize::<dyn Trait>();
        assert_serialize::<dyn Trait + Send>();
    }

    #[test]
    fn test_type_parameter() {
        trait Trait<T>: Serialize {}

        serialize_trait_object!(<T> Trait<T>);
        assert_serialize::<dyn Trait<u32>>();
        assert_serialize::<dyn Trait<u32> + Send>();
    }

    #[test]
    fn test_generic_bound() {
        trait Trait<T: PartialEq<T>, U>: Serialize {}

        serialize_trait_object!(<T: PartialEq<T>, U> Trait<T, U>);
        assert_serialize::<dyn Trait<u32, ()>>();
        assert_serialize::<dyn Trait<u32, ()> + Send>();
    }

    #[test]
    fn test_where_clause() {
        trait Trait<T>: Serialize
        where
            T: Clone,
        {
        }

        serialize_trait_object!(<T> Trait<T> where T: Clone);
        assert_serialize::<dyn Trait<u32>>();
        assert_serialize::<dyn Trait<u32> + Send>();
    }
}
