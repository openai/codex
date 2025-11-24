// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! 📚 *This module documents ICU4X constructor signatures.*
//!
//! One of the key differences between ICU4X and its parent projects, ICU4C and ICU4J, is in how
//! it deals with locale data.
//!
//! In ICU4X, data can always be explicitly passed to any function that requires data.
//! This enables ICU4X to achieve the following value propositions:
//!
//! 1. Configurable data sources (machine-readable data file, baked into code, JSON, etc).
//! 2. Dynamic data loading at runtime (load data on demand).
//! 3. Reduced overhead and code size (data is resolved locally at each call site).
//! 4. Explicit support for multiple ICU4X instances sharing data.
//!
//! However, as manual data management can be tedious, ICU4X also has a `compiled_data`
//! default Cargo feature that includes data and makes ICU4X work out-of-the box.
//!
//! Subsequently, there are 3 versions of all Rust ICU4X functions that use data:
//!
//! 1. `*`
//! 2. `*_unstable`
//! 3. `*_with_buffer_provider`
//!
//! # Which constructor should I use?
//!
//! ## When to use `*`
//!
//! If you don't want to customize data at runtime (i.e. if you don't care about code size,
//! updating your data, etc.) you can use the `compiled_data` Cargo feature and don't have to think
//! about where your data comes from.
//!
//! These constructors are sometimes `const` functions, this way Rust can most effectively optimize
//! your usage of ICU4X.
//!
//! ## When to use `*_unstable`
//!
//! Use this constructor if your data provider implements the [`DataProvider`] trait for all
//! data structs in *current and future* ICU4X versions. Examples:
//!
//! 1. `BakedDataProvider` generated for the specific ICU4X minor version
//! 2. Anything with a _blanket_ [`DataProvider`] impl
//!
//! Since the exact set of bounds may change at any time, including in minor SemVer releases,
//! it is the client's responsibility to guarantee that the requirement is upheld.
//!
//! ## When to use `*_with_buffer_provider`
//!
//! Use this constructor if your data originates as byte buffers that need to be deserialized.
//! All such providers should implement [`BufferProvider`]. Examples:
//!
//! 1. [`BlobDataProvider`]
//! 2. [`FsDataProvider`]
//! 3. [`ForkByMarkerProvider`] between two providers implementing [`BufferProvider`]
//!
//! Please note that you must enable the `serde` Cargo feature on each crate in which you use the
//! `*_with_buffer_provider` constructor.
//!
//! # Data Versioning Policy
//!
//! The `*_with_buffer_provider` functions will succeed to compile and
//! run if given a data provider supporting all of the markers required for the object being
//! constructed, either the current or any previous version within the same SemVer major release.
//! For example, if a data file is built to support FooFormatter version 1.1, then FooFormatter
//! version 1.2 will be able to read the same data file. Likewise, backwards-compatible markers can
//! always be included by `icu_provider_export` to support older library versions.
//!
//! The `*_unstable` functions are only guaranteed to work on data built for the exact same minor version
//! of ICU4X. The advantage of the `*_unstable` functions is that they result in the smallest code
//! size and allow for automatic data slicing when `BakedDataProvider` is used. However, the type
//! bounds of this function may change over time, breaking SemVer guarantees. These functions
//! should therefore only be used when you have full control over your data lifecycle at compile
//! time.
//!
//! # Data Providers Over FFI
//!
//! Over FFI, there is only one data provider type: [`ICU4XDataProvider`]. Internally, it is an
//! `enum` between`dyn `[`BufferProvider`] and a unit compiled data variant.
//!
//! To control for code size, there are two Cargo features, `compiled_data` and `buffer_provider`,
//! that enable the corresponding items in the enum.
//!
//! In Rust ICU4X, a similar enum approach was not taken because:
//!
//! 1. Feature-gating the enum branches gets complex across crates.
//! 2. Without feature gating, users need to carry Serde code even if they're not using it,
//!    violating one of the core value propositions of ICU4X.
//! 3. We could reduce the number of constructors from 4 to 2 but not to 1, so the educational
//!    benefit is limited.
//!
//! [`DataProvider`]: crate::DataProvider
//! [`BufferProvider`]: crate::buf::BufferProvider
//! [`FixedProvider`]: ../../icu_provider_adapters/fixed/struct.FixedProvider.html
//! [`ForkByMarkerProvider`]: ../../icu_provider_adapters/fork/struct.ForkByMarkerProvider.html
//! [`BlobDataProvider`]: ../../icu_provider_blob/struct.BlobDataProvider.html
//! [`StaticDataProvider`]: ../../icu_provider_blob/struct.StaticDataProvider.html
//! [`FsDataProvider`]: ../../icu_provider_blob/struct.FsDataProvider.html
//! [`ICU4XDataProvider`]: ../../icu_capi/provider/ffi/struct.ICU4XDataProvider.html

#[doc(hidden)] // macro
#[macro_export]
macro_rules! gen_buffer_unstable_docs {
    (BUFFER, $data:path) => {
        concat!(
            "A version of [`", stringify!($data), "`] that uses custom data ",
            "provided by a [`BufferProvider`].\n\n",
            "✨ *Enabled with the `serde` feature.*\n\n",
            "[📚 Help choosing a constructor](icu_provider::constructors)",
        )
    };
    (UNSTABLE, $data:path) => {
        concat!(
            "A version of [`", stringify!($data), "`] that uses custom data ",
            "provided by a [`DataProvider`].\n\n",
            "[📚 Help choosing a constructor](icu_provider::constructors)\n\n",
            "<div class=\"stab unstable\">⚠️ The bounds on <tt>provider</tt> may change over time, including in SemVer minor releases.</div>"
        )
    };
}

/// Usage:
///
/// ```rust,ignore
/// gen_buffer_data_constructors!((locale, options: FooOptions) -> error: DataError,
///    /// Some docs
///   functions: [try_new, try_new_with_buffer_provider, try_new_unstable]
/// );
/// ```
///
/// `functions` can be omitted if using standard names. If `locale` is omitted, the method will not take a locale. You can specify any number
/// of options arguments, including zero.
///
/// By default the macro will generate a `try_new`. If you wish to skip it, write `try_new: skip`
///
/// Errors can be specified as `error: SomeError` or `result: SomeType`, where `error` will get it wrapped in `Result<Self, SomeError>`.
#[expect(clippy::crate_in_macro_def)] // by convention each crate's data provider is `crate::provider::Baked`
#[doc(hidden)] // macro
#[macro_export]
macro_rules! gen_buffer_data_constructors {
    // Allow people to omit the functions
    (($($args:tt)*) -> $error_kind:ident: $error_ty:ty, $(#[$doc:meta])*) => {
        $crate::gen_buffer_data_constructors!(
            ($($args)*) -> $error_kind: $error_ty,
            $(#[$doc])*
            functions: [
                try_new,
                try_new_with_buffer_provider,
                try_new_unstable,
                Self,
            ]
        );
    };
    // Allow people to specify errors instead of results
    (($($args:tt)*) -> error: $error_ty:path, $(#[$doc:meta])* functions: [$baked:ident$(: $baked_cmd:ident)?, $buffer:ident, $unstable:ident $(, $struct:ident)? $(,)?]) => {
        $crate::gen_buffer_data_constructors!(
            ($($args)*) -> result: Result<Self, $error_ty>,
            $(#[$doc])*
            functions: [
                $baked$(: $baked_cmd)?,
                $buffer,
                $unstable
                $(, $struct)?
            ]
        );
    };

    // locale shorthand
    ((locale, $($args:tt)*) -> result: $result_ty:ty, $(#[$doc:meta])* functions: [$baked:ident$(: $baked_cmd:ident)?, $buffer:ident, $unstable:ident $(, $struct:ident)? $(,)?]) => {
        $crate::gen_buffer_data_constructors!(
            (locale: &$crate::DataLocale, $($args)*) -> result: $result_ty,
            $(#[$doc])*
            functions: [
                $baked$(: $baked_cmd)?,
                $buffer,
                $unstable
                $(, $struct)?
            ]
        );
    };
    ((locale) -> result: $result_ty:ty, $(#[$doc:meta])* functions: [$baked:ident$(: $baked_cmd:ident)?, $buffer:ident, $unstable:ident $(, $struct:ident)? $(,)?]) => {
        $crate::gen_buffer_data_constructors!(
            (locale: &$crate::DataLocale) -> result: $result_ty,
            $(#[$doc])*
            functions: [
                $baked$(: $baked_cmd)?,
                $buffer,
                $unstable
                $(, $struct)?
            ]
        );
    };


    (($($options_arg:ident: $options_ty:ty),*) -> result: $result_ty:ty, $(#[$doc:meta])* functions: [$baked:ident, $buffer:ident, $unstable:ident $(, $struct:ident)? $(,)?]) => {
        #[cfg(feature = "compiled_data")]
        $(#[$doc])*
        ///
        /// ✨ *Enabled with the `compiled_data` Cargo feature.*
        ///
        /// [📚 Help choosing a constructor](icu_provider::constructors)
        pub fn $baked($($options_arg: $options_ty),* ) -> $result_ty {
            $($struct :: )? $unstable(&crate::provider::Baked $(, $options_arg)* )
        }

        $crate::gen_buffer_data_constructors!(
            ($($options_arg: $options_ty),*) -> result: $result_ty,
            $(#[$doc])*
            functions: [
                $baked: skip,
                $buffer,
                $unstable
                $(, $struct)?
            ]
        );
    };
    (($($options_arg:ident: $options_ty:ty),*) -> result: $result_ty:ty, $(#[$doc:meta])* functions: [$baked:ident: skip, $buffer:ident, $unstable:ident $(, $struct:ident)? $(,)?]) => {
        #[cfg(feature = "serde")]
        #[doc = $crate::gen_buffer_unstable_docs!(BUFFER, $($struct ::)? $baked)]
        pub fn $buffer(provider: &(impl $crate::buf::BufferProvider + ?Sized) $(, $options_arg: $options_ty)* ) -> $result_ty {
            use $crate::buf::AsDeserializingBufferProvider;
            $($struct :: )? $unstable(&provider.as_deserializing()  $(, $options_arg)* )
        }
    };
}
