use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use static_assertions::assert_impl_all;
use std::os::fd::{self, AsFd, AsRawFd, BorrowedFd, RawFd};

use crate::{serialized::Format, Basic, Signature, Type};

/// A file-descriptor type wrapper.
///
/// Since [`std::os::fd::BorrowedFd`] and [`std::os::fd::OwnedFd`] types
/// do not implement  [`Serialize`] and [`Deserialize`]. So we provide a
/// wrapper for both that implements these traits.
///
/// [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
/// [`Deserialize`]: https://docs.serde.rs/serde/de/trait.Deserialize.html
#[derive(Debug)]
pub enum Fd<'f> {
    Borrowed(BorrowedFd<'f>),
    Owned(fd::OwnedFd),
}

impl<'f> Fd<'f> {
    /// Try to create an owned version of `self`.
    pub fn try_to_owned(&self) -> crate::Result<Fd<'static>> {
        self.as_fd()
            .try_clone_to_owned()
            .map(Fd::Owned)
            .map_err(Into::into)
    }

    /// Try to clone `self`.
    pub fn try_clone(&self) -> crate::Result<Self> {
        Ok(match self {
            Self::Borrowed(fd) => Self::Borrowed(*fd),
            Self::Owned(fd) => Self::Owned(fd.try_clone()?),
        })
    }
}

impl<'f> From<BorrowedFd<'f>> for Fd<'f> {
    fn from(fd: BorrowedFd<'f>) -> Self {
        Self::Borrowed(fd)
    }
}

impl From<fd::OwnedFd> for Fd<'_> {
    fn from(fd: fd::OwnedFd) -> Self {
        Self::Owned(fd)
    }
}

impl From<OwnedFd> for Fd<'_> {
    fn from(owned: OwnedFd) -> Self {
        owned.inner
    }
}

impl TryFrom<Fd<'_>> for fd::OwnedFd {
    type Error = crate::Error;

    fn try_from(fd: Fd<'_>) -> crate::Result<Self> {
        match fd {
            Fd::Borrowed(fd) => fd.try_clone_to_owned().map_err(Into::into),
            Fd::Owned(fd) => Ok(fd),
        }
    }
}

impl AsRawFd for Fd<'_> {
    fn as_raw_fd(&self) -> RawFd {
        self.as_fd().as_raw_fd()
    }
}

impl AsFd for Fd<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        match self {
            Self::Borrowed(fd) => fd.as_fd(),
            Self::Owned(fd) => fd.as_fd(),
        }
    }
}

impl<'fd, T> From<&'fd T> for Fd<'fd>
where
    T: AsFd,
{
    fn from(t: &'fd T) -> Self {
        Self::Borrowed(t.as_fd())
    }
}

impl std::fmt::Display for Fd<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_raw_fd().fmt(f)
    }
}

macro_rules! fd_impl {
    ($i:ty) => {
        assert_impl_all!($i: Send, Sync, Unpin);

        impl Basic for $i {
            const SIGNATURE_CHAR: char = 'h';
            const SIGNATURE_STR: &'static str = "h";

            fn alignment(format: Format) -> usize {
                u32::alignment(format)
            }
        }

        impl Type for $i {
            fn signature() -> Signature<'static> {
                Signature::from_static_str_unchecked(Self::SIGNATURE_STR)
            }
        }
    };
}

fd_impl!(Fd<'_>);

impl Serialize for Fd<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.as_raw_fd())
    }
}

impl<'de> Deserialize<'de> for Fd<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = i32::deserialize(deserializer)?;
        // SAFETY: The `'de` lifetimes will ensure the borrow won't outlive the raw FD.
        let fd = unsafe { BorrowedFd::borrow_raw(raw) };

        Ok(Fd::Borrowed(fd))
    }
}

impl PartialEq for Fd<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_raw_fd().eq(&other.as_raw_fd())
    }
}
impl Eq for Fd<'_> {}

impl PartialOrd for Fd<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fd<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_raw_fd().cmp(&other.as_raw_fd())
    }
}

impl std::hash::Hash for Fd<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_raw_fd().hash(state)
    }
}

/// A file-descriptor type wrapper.
///
/// This is the same as [`Fd`] type, except it only keeps an owned file descriptor.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct OwnedFd {
    inner: Fd<'static>,
}

fd_impl!(OwnedFd);

impl Serialize for OwnedFd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OwnedFd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fd = Fd::deserialize(deserializer)?;
        Ok(OwnedFd {
            inner: fd
                .as_fd()
                .try_clone_to_owned()
                .map(Fd::Owned)
                .map_err(D::Error::custom)?,
        })
    }
}

impl AsFd for OwnedFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl From<fd::OwnedFd> for OwnedFd {
    fn from(value: fd::OwnedFd) -> Self {
        Self {
            inner: Fd::Owned(value),
        }
    }
}

impl From<OwnedFd> for fd::OwnedFd {
    fn from(value: OwnedFd) -> fd::OwnedFd {
        match value.inner {
            Fd::Owned(fd) => fd,
            Fd::Borrowed(_) => unreachable!(),
        }
    }
}

impl From<Fd<'static>> for OwnedFd {
    fn from(value: Fd<'static>) -> Self {
        Self { inner: value }
    }
}

impl std::fmt::Display for OwnedFd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
