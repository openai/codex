// @generated
include!("normalizer_nfd_tables_v1.rs.data");
include!("normalizer_nfd_supplement_v1.rs.data");
include!("normalizer_nfkd_data_v1.rs.data");
include!("normalizer_nfkd_tables_v1.rs.data");
include!("normalizer_nfc_v1.rs.data");
include!("normalizer_nfd_data_v1.rs.data");
include!("normalizer_uts46_data_v1.rs.data");
/// Marks a type as a data provider. You can then use macros like
/// `impl_core_helloworld_v1` to add implementations.
///
/// ```ignore
/// struct MyProvider;
/// const _: () = {
///     include!("path/to/generated/macros.rs");
///     make_provider!(MyProvider);
///     impl_core_helloworld_v1!(MyProvider);
/// }
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __make_provider {
    ($ name : ty) => {
        #[clippy::msrv = "1.83"]
        impl $name {
            #[allow(dead_code)]
            pub(crate) const MUST_USE_MAKE_PROVIDER_MACRO: () = ();
        }
        icu_provider::marker::impl_data_provider_never_marker!($name);
    };
}
#[doc(inline)]
pub use __make_provider as make_provider;
/// This macro requires the following crates:
/// * `icu`
/// * `icu_provider`
/// * `zerovec`
#[allow(unused_macros)]
macro_rules! impl_data_provider {
    ($ provider : ty) => {
        make_provider!($provider);
        impl_normalizer_nfd_tables_v1!($provider);
        impl_normalizer_nfd_supplement_v1!($provider);
        impl_normalizer_nfkd_data_v1!($provider);
        impl_normalizer_nfkd_tables_v1!($provider);
        impl_normalizer_nfc_v1!($provider);
        impl_normalizer_nfd_data_v1!($provider);
        impl_normalizer_uts46_data_v1!($provider);
    };
}
