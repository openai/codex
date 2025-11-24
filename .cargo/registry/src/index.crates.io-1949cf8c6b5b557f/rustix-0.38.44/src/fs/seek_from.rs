//! The following is derived from Rust's
//! library/std/src/io/mod.rs at revision
//! dca3f1b786efd27be3b325ed1e01e247aa589c3b.

/// Enumeration of possible methods to seek within an I/O object.
///
/// It is used by the [`seek`] function.
///
/// This is similar to [`std::io::SeekFrom`], however it adds platform-specific
/// seek options.
///
/// [`seek`]: crate::fs::seek
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes.
    #[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
    Start(#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))] u64),

    /// Sets the offset to the size of this object plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error
    /// to seek before byte 0.
    #[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
    End(#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))] i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error
    /// to seek before byte 0.
    #[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))]
    Current(#[cfg_attr(staged_api, stable(feature = "rust1", since = "1.0.0"))] i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes, plus the distance to the next byte which is not in a hole.
    ///
    /// If the offset is in a hole at the end of the file, the seek will fail
    /// with [`Errno::NXIO`].
    ///
    /// [`Errno::NXIO`]: crate::io::Errno::NXIO
    #[cfg(any(apple, freebsdlike, linux_kernel, solarish))]
    Data(i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes, plus the distance to the next byte which is in a hole.
    ///
    /// If there is no hole past the offset, it will be set to the end of the
    /// file i.e. there is an implicit hole at the end of any file.
    #[cfg(any(apple, freebsdlike, linux_kernel, solarish))]
    Hole(i64),
}
