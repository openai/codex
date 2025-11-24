#![allow(clippy::test_attr_in_doctest)]

//! Tests to ensure the rust and sys types implement the same traits.

/// A macro used to assert a type defined in both the rust and sys implementations of wayland-backend
/// implement the same traits.
///
/// There are four patterns which may be matched using this macro.
///
/// For example, assume you want to make sure both the rust and sys versions of `ObjectId` implement [`Debug`].
/// The following pattern would achieve that check.
///
/// ```no_run
/// #[test]
/// fn test() {
///     assert_impl!(server::ObjectId: std::fmt::Debug);
/// }
/// ```
///
/// Multiple traits may be tested by separating each trait with a comma.
///
/// ```no_run
/// #[test]
/// fn test() {
///     assert_impl!(server::ObjectId: std::fmt::Debug, Send, Sync);
/// }
/// ```
///
/// For the client side, simply change the path before the name of the type.
///
/// ```no_run
/// #[test]
/// fn test() {
///     assert_impl!(client::ObjectId: std::fmt::Debug)
/// }
/// ```
///
/// Traits may be tested through prefixing the contents of the macro with `dyn`.
///
/// ```ignore
/// #[test]
/// fn test() {
///     assert_impl!(dyn server::SomeTraitWithNoGeneric: std::fmt::Debug)
/// }
/// ```
///
/// Finally, generics may also be tested by simply adding the generics as expected in a normal type. Do note
/// you will need to monomorphize the type with something, such as, `()`, the unit type.
///
/// ```no_run
/// #[test]
/// fn test() {
///     assert_impl!(server::Backend<()>: Send, Sync); // No trait
///     assert_impl!(dyn server::ObjectData<()>: std::fmt::Debug); // Trait
/// }
/// ```
macro_rules! assert_impl {
    // No type parameters with dyn
    (
        dyn $side: ident::$ty: ident: $($trait_: path),+
    ) => {{
        #[allow(dead_code)]
        fn assert_impl<T: $($trait_ +)* ?Sized>() {}

        assert_impl::<dyn wayland_backend::rs::$side::$ty>();
        #[cfg(feature = "server_system")]
        assert_impl::<dyn wayland_backend::sys::$side::$ty>();
    }};

    // Type parameters with dyn
    (
        dyn $side: ident::$ty: ident<$($types: ty),*>: $($trait_: path),+
    ) => {{
        #[allow(dead_code)]
        fn assert_impl<T: $($trait_ +)* ?Sized>() {}

        assert_impl::<dyn wayland_backend::rs::$side::$ty <$($types),*>>();
        #[cfg(feature = "server_system")]
        assert_impl::<dyn wayland_backend::sys::$side::$ty <$($types),*>>();
    }};

    // No type parameters
    (
        $side: ident::$ty: ident: $($trait_: path),+
    ) => {{
        #[allow(dead_code)]
        fn assert_impl<T: $($trait_ +)* ?Sized>() {}

        assert_impl::<wayland_backend::rs::$side::$ty>();
        #[cfg(feature = "server_system")]
        assert_impl::<wayland_backend::sys::$side::$ty>();
    }};

    // Type parameters
    (
        $side: ident::$ty: ident<$($types: ty),*>: $($trait_: path),+
    ) => {{
        #[allow(dead_code)]
        fn assert_impl<T: $($trait_ +)* ?Sized>() {}

        assert_impl::<wayland_backend::rs::$side::$ty <$($types),*>>();
        #[cfg(feature = "server_system")]
        assert_impl::<wayland_backend::sys::$side::$ty <$($types),*>>();
    }};
}

mod server {
    #[test]
    fn test_impls() {
        // ObjectData
        assert_impl!(
            dyn server::ObjectData<()>: std::fmt::Debug, downcast_rs::DowncastSync
        );

        // GlobalHandler
        assert_impl!(
            dyn server::GlobalHandler<()>: std::fmt::Debug, downcast_rs::DowncastSync
        );

        // ClientData
        assert_impl!(
            dyn server::ClientData: std::fmt::Debug, downcast_rs::DowncastSync
        );

        // ObjectId
        assert_impl!(
            server::ObjectId: std::fmt::Debug,
            std::fmt::Display,
            Send,
            Sync,
            PartialEq,
            Eq,
            Clone
        );

        // ClientId
        assert_impl!(server::ClientId: std::fmt::Debug, Clone, Send, Sync, PartialEq, Eq);

        // GlobalId
        assert_impl!(server::GlobalId: std::fmt::Debug, Clone, Send, Sync, PartialEq, Eq);

        // Handle
        assert_impl!(server::Handle: std::fmt::Debug);

        // Backend
        assert_impl!(server::Backend<()>: Send, Sync);
    }
}

mod client {
    #[test]
    fn test_impls() {
        // ObjectData
        assert_impl!(
            dyn client::ObjectData: std::fmt::Debug, downcast_rs::DowncastSync
        );

        // ObjectId
        assert_impl!(
            client::ObjectId: std::fmt::Debug,
            std::fmt::Display,
            Clone,
            Send,
            Sync,
            PartialEq,
            Eq
        );

        // Backend
        assert_impl!(client::Backend: Send, Sync, std::fmt::Debug);
    }
}
