#[cfg(unix)]
use crate::OwnedFd;
use std::ops::Deref;

use crate::serialized::Context;

/// Represents the return value of [`crate::to_writer`] function.
///
/// It mainly contains the size of serialized bytes in a specific format.
///
/// On Unix platforms, it also contains a list of file descriptors, whose indexes are included in
/// the serialized bytes.
#[derive(Debug)]
pub struct Written {
    size: usize,
    context: Context,
    #[cfg(unix)]
    fds: Vec<OwnedFd>,
}

impl Written {
    /// Create a new `Written` instance.
    pub fn new(size: usize, context: Context) -> Self {
        Self {
            size,
            context,
            #[cfg(unix)]
            fds: vec![],
        }
    }

    /// Set the file descriptors.
    #[cfg(unix)]
    pub fn set_fds(mut self, fds: impl IntoIterator<Item = impl Into<OwnedFd>>) -> Self {
        self.fds = fds.into_iter().map(Into::into).collect();
        self
    }

    /// The size of the serialized bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The encoding context.
    pub fn context(&self) -> Context {
        self.context
    }

    /// Consume `self` and return the file descriptors.
    ///
    /// This method is only available on Unix platforms.
    #[cfg(unix)]
    pub fn into_fds(self) -> Vec<OwnedFd> {
        self.fds
    }

    /// The file descriptors that are references by the serialized bytes.
    ///
    /// This method is only available on Unix platforms.
    #[cfg(unix)]
    pub fn fds(&self) -> &[OwnedFd] {
        &self.fds
    }
}

impl Deref for Written {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.size
    }
}
