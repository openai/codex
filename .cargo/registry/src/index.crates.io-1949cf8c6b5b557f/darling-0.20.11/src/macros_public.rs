//! Macros that should be exported from both `darling_core` and `darling`.
//! Note that these are **sym-linked** into the main code, and so cannot declare on items that are exported differently
//! in `darling_core` vs. `darling`.

/// Generator for `UsesTypeParam` impls that unions the used type parameters of the selected fields.
///
/// # Usage
/// The macro takes the type implementing the trait as the first argument, then a comma-separated list of
/// fields for the rest of its arguments.
///
/// The type of each passed-in field must implement `UsesTypeParams`, or the resulting code won't compile.
///
/// ```rust
/// # extern crate syn;
/// # use darling_core::uses_type_params;
/// #
/// struct MyField {
///     ty: syn::Type,
/// }
///
/// uses_type_params!(MyField, ty);
///
/// fn main() {
///     // no test run
/// }
/// ```
///
/// `darling` cannot derive this trait automatically, as it doesn't know which information extracted from
/// proc-macro input is meant to constitute "using" the type parameter, but crate consumers should
/// implement it by hand or using the macro.
#[macro_export]
macro_rules! uses_type_params {
    ($impl_type:ty, $accessor:ident) => {
        impl $crate::usage::UsesTypeParams for $impl_type {
            fn uses_type_params<'gen>(
                &self,
                options: &$crate::usage::Options,
                type_set: &'gen $crate::usage::IdentSet
            ) -> $crate::usage::IdentRefSet<'gen> {
                self.$accessor.uses_type_params(options, type_set)
            }
        }
    };
    ($impl_type:ty, $first:ident, $($field:ident),+) => {
        impl $crate::usage::UsesTypeParams for $impl_type {
            fn uses_type_params<'gen>(
                &self,
                options: &$crate::usage::Options,
                type_set: &'gen $crate::usage::IdentSet
            ) -> $crate::usage::IdentRefSet<'gen> {
                let mut hits = self.$first.uses_type_params(options, type_set);
                $(
                    hits.extend(self.$field.uses_type_params(options, type_set));
                )*
                hits
            }
        }
    };
}

/// Generator for `UsesLifetimes` impls that unions the used lifetimes of the selected fields.
///
/// # Usage
/// The macro takes the type implementing the trait as the first argument, then a comma-separated list of
/// fields for the rest of its arguments.
///
/// The type of each passed-in field must implement `UsesLifetimes`, or the resulting code won't compile.
#[macro_export]
macro_rules! uses_lifetimes {
    ($impl_type:ty, $accessor:ident) => {
        impl $crate::usage::UsesLifetimes for $impl_type {
            fn uses_lifetimes<'gen>(
                &self,
                options: &$crate::usage::Options,
                type_set: &'gen $crate::usage::LifetimeSet
            ) -> $crate::usage::LifetimeRefSet<'gen> {
                self.$accessor.uses_lifetimes(options, type_set)
            }
        }
    };
    ($impl_type:ty, $first:ident, $($field:ident),+) => {
        impl $crate::usage::UsesLifetimes for $impl_type {
            fn uses_lifetimes<'gen>(
                &self,
                options: &$crate::usage::Options,
                type_set: &'gen $crate::usage::LifetimeSet
            ) -> $crate::usage::LifetimeRefSet<'gen> {
                let mut hits = self.$first.uses_lifetimes(options, type_set);
                $(
                    hits.extend(self.$field.uses_lifetimes(options, type_set));
                )*
                hits
            }
        }
    };
}
