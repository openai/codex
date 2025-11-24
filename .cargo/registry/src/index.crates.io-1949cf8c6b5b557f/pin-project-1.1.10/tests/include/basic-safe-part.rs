// SPDX-License-Identifier: Apache-2.0 OR MIT

// default #[pin_project], PinnedDrop, project_replace, !Unpin, and UnsafeUnpin without UnsafeUnpin impl are completely safe.

/// Testing default struct.
#[allow(clippy::exhaustive_structs)] // for the type itself
#[::pin_project::pin_project]
#[derive(Debug)]
pub struct DefaultStruct<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

/// Testing named struct.
#[::pin_project::pin_project(
    project = DefaultStructNamedProj,
    project_ref = DefaultStructNamedProjRef,
)]
#[derive(Debug)]
#[allow(clippy::exhaustive_structs)] // for the type itself
pub struct DefaultStructNamed<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

/// Testing default tuple struct.
#[::pin_project::pin_project]
#[allow(clippy::exhaustive_structs)] // for the type itself
#[derive(Debug)]
pub struct DefaultTupleStruct<T, U>(#[pin] pub T, pub U);

/// Testing named tuple struct.
#[::pin_project::pin_project(
    project = DefaultTupleStructNamedProj,
    project_ref = DefaultTupleStructNamedProjRef,
)]
#[allow(clippy::exhaustive_structs)] // for the type itself
#[derive(Debug)]
pub struct DefaultTupleStructNamed<T, U>(#[pin] pub T, pub U);

/// Testing enum.
#[allow(clippy::exhaustive_enums)] // for the type itself
#[::pin_project::pin_project(
    project = DefaultEnumProj,
    project_ref = DefaultEnumProjRef,
)]
#[derive(Debug)]
pub enum DefaultEnum<T, U> {
    /// Struct variant.
    Struct {
        /// Pinned field.
        #[pin]
        pinned: T,
        /// Unpinned field.
        unpinned: U,
    },
    /// Tuple variant.
    Tuple(#[pin] T, U),
    /// Unit variant.
    Unit,
}

/// Testing pinned drop struct.
#[allow(clippy::exhaustive_structs)] // for the type itself
#[::pin_project::pin_project(PinnedDrop)]
#[derive(Debug)]
pub struct PinnedDropStruct<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

#[::pin_project::pinned_drop]
impl<T, U> PinnedDrop for PinnedDropStruct<T, U> {
    #[allow(clippy::absolute_paths)]
    fn drop(self: ::pin_project::__private::Pin<&mut Self>) {}
}

/// Testing pinned drop tuple struct.
#[allow(clippy::exhaustive_structs)] // for the type itself
#[::pin_project::pin_project(PinnedDrop)]
#[derive(Debug)]
pub struct PinnedDropTupleStruct<T, U>(#[pin] pub T, pub U);

#[::pin_project::pinned_drop]
impl<T, U> PinnedDrop for PinnedDropTupleStruct<T, U> {
    #[allow(clippy::absolute_paths)]
    fn drop(self: ::pin_project::__private::Pin<&mut Self>) {}
}

/// Testing pinned drop enum.
#[allow(clippy::absolute_paths, clippy::exhaustive_enums)] // for the type itself
#[::pin_project::pin_project(
    PinnedDrop,
    project = PinnedDropEnumProj,
    project_ref = PinnedDropEnumProjRef,
)]
#[derive(Debug)]
pub enum PinnedDropEnum<T: ::pin_project::__private::Unpin, U> {
    /// Struct variant.
    Struct {
        /// Pinned field.
        #[pin]
        pinned: T,
        /// Unpinned field.
        unpinned: U,
    },
    /// Tuple variant.
    Tuple(#[pin] T, U),
    /// Unit variant.
    Unit,
}

#[::pin_project::pinned_drop]
#[allow(clippy::absolute_paths)]
impl<T: ::pin_project::__private::Unpin, U> PinnedDrop for PinnedDropEnum<T, U> {
    fn drop(self: ::pin_project::__private::Pin<&mut Self>) {}
}

/// Testing default struct with replace.
#[allow(clippy::exhaustive_structs)] // for the type itself
#[::pin_project::pin_project(project_replace)]
#[derive(Debug)]
pub struct ReplaceStruct<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

/// Testing named struct with replace.
#[::pin_project::pin_project(
    project = ReplaceStructNamedProj,
    project_ref = ReplaceStructNamedProjRef,
    project_replace = ReplaceStructNamedProjOwn,
)]
#[allow(clippy::exhaustive_structs)] // for the type itself
#[derive(Debug)]
pub struct ReplaceStructNamed<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

/// Testing default struct with replace.
#[::pin_project::pin_project(project_replace)]
#[derive(Debug)]
#[allow(clippy::exhaustive_structs)] // for the type itself
pub struct ReplaceTupleStruct<T, U>(#[pin] pub T, pub U);

/// Testing named struct with replace.
#[::pin_project::pin_project(
    project = ReplaceTupleStructNamedProj,
    project_ref = ReplaceTupleStructNamedProjRef,
    project_replace = ReplaceTupleStructNamedProjOwn,
)]
#[derive(Debug)]
#[allow(clippy::exhaustive_structs)] // for the type itself
pub struct ReplaceTupleStructNamed<T, U>(#[pin] pub T, pub U);

/// Testing enum with replace.
#[allow(clippy::exhaustive_enums)] // for the type itself
#[::pin_project::pin_project(
    project = ReplaceEnumProj,
    project_ref = ReplaceEnumProjRef,
    project_replace = ReplaceEnumProjOwn,
)]
#[derive(Debug)]
pub enum ReplaceEnum<T, U> {
    /// Struct variant.
    Struct {
        /// Pinned field.
        #[pin]
        pinned: T,
        /// Unpinned field.
        unpinned: U,
    },
    /// Tuple variant.
    Tuple(#[pin] T, U),
    /// Unit variant.
    Unit,
}

/// Testing struct with unsafe `Unpin`.
#[::pin_project::pin_project(UnsafeUnpin)]
#[allow(clippy::exhaustive_structs)] // for the type itself
#[derive(Debug)]
pub struct UnsafeUnpinStruct<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

/// Testing tuple struct with unsafe `Unpin`.
#[::pin_project::pin_project(UnsafeUnpin)]
#[allow(clippy::exhaustive_structs)] // for the type itself
#[derive(Debug)]
pub struct UnsafeUnpinTupleStruct<T, U>(#[pin] pub T, pub U);

/// Testing enum unsafe `Unpin`.
#[allow(clippy::exhaustive_enums)] // for the type itself
#[::pin_project::pin_project(
    UnsafeUnpin,
    project = UnsafeUnpinEnumProj,
    project_ref = UnsafeUnpinEnumProjRef,
)]
#[derive(Debug)]
pub enum UnsafeUnpinEnum<T, U> {
    /// Struct variant.
    Struct {
        /// Pinned field.
        #[pin]
        pinned: T,
        /// Unpinned field.
        unpinned: U,
    },
    /// Tuple variant.
    Tuple(#[pin] T, U),
    /// Unit variant.
    Unit,
}

/// Testing struct with `!Unpin`.
#[::pin_project::pin_project(!Unpin)]
#[derive(Debug)]
#[allow(clippy::exhaustive_structs)] // for the type itself
pub struct NotUnpinStruct<T, U> {
    /// Pinned field.
    #[pin]
    pub pinned: T,
    /// Unpinned field.
    pub unpinned: U,
}

/// Testing tuple struct with `!Unpin`.
#[allow(clippy::exhaustive_structs)] // for the type itself
#[::pin_project::pin_project(!Unpin)]
#[derive(Debug)]
pub struct NotUnpinTupleStruct<T, U>(#[pin] pub T, pub U);

/// Testing enum with `!Unpin`.
#[allow(clippy::exhaustive_enums)] // for the type itself
#[::pin_project::pin_project(
    !Unpin,
    project = NotUnpinEnumProj,
    project_ref = NotUnpinEnumProjRef,
)]
#[derive(Debug)]
pub enum NotUnpinEnum<T, U> {
    /// Struct variant.
    Struct {
        /// Pinned field.
        #[pin]
        pinned: T,
        /// Unpinned field.
        unpinned: U,
    },
    /// Tuple variant.
    Tuple(#[pin] T, U),
    /// Unit variant.
    Unit,
}
