// SPDX-License-Identifier: Apache-2.0 OR MIT

// default pin_project! is completely safe.

::pin_project_lite::pin_project! {
    /// Testing default struct.
    #[derive(Debug)]
    #[allow(clippy::exhaustive_structs)] // for the type itself
    pub struct DefaultStruct<T, U> {
        #[pin]
        pub pinned: T,
        pub unpinned: U,
    }
}

::pin_project_lite::pin_project! {
    /// Testing named struct.
    #[allow(clippy::exhaustive_structs)] // for the type itself
    #[project = DefaultStructProj]
    #[project_ref = DefaultStructProjRef]
    #[derive(Debug)]
    pub struct DefaultStructNamed<T, U> {
        #[pin]
        pub pinned: T,
        pub unpinned: U,
    }
}

::pin_project_lite::pin_project! {
    /// Testing enum.
    #[allow(clippy::exhaustive_enums)] // for the type itself
    #[project = DefaultEnumProj]
    #[project_ref = DefaultEnumProjRef]
    #[derive(Debug)]
    pub enum DefaultEnum<T, U> {
        /// Struct variant.
        Struct {
            #[pin]
            pinned: T,
            unpinned: U,
        },
        /// Unit variant.
        Unit,
    }
}

::pin_project_lite::pin_project! {
    /// Testing pinned drop struct.
    #[allow(clippy::exhaustive_structs)] // for the type itself
    #[derive(Debug)]
    pub struct PinnedDropStruct<T, U> {
        #[pin]
        pub pinned: T,
        pub unpinned: U,
    }
    impl<T, U> PinnedDrop for PinnedDropStruct<T, U> {
        fn drop(_this: Pin<&mut Self>) {}
    }
}

::pin_project_lite::pin_project! {
    /// Testing pinned drop enum.
    #[allow(clippy::absolute_paths, clippy::exhaustive_enums)] // for the type itself
    #[project = PinnedDropEnumProj]
    #[project_ref = PinnedDropEnumProjRef]
    #[derive(Debug)]
    pub enum PinnedDropEnum<T: ::pin_project_lite::__private::Unpin, U> {
        /// Struct variant.
        Struct {
            #[pin]
            pinned: T,
            unpinned: U,
        },
        /// Unit variant.
        Unit,
    }
    #[allow(clippy::absolute_paths)]
    impl<T: ::pin_project_lite::__private::Unpin, U> PinnedDrop for PinnedDropEnum<T, U> {
        fn drop(_this: Pin<&mut Self>) {}
    }
}
