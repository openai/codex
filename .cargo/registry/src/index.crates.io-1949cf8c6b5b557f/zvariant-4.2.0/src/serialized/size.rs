use std::ops::Deref;

use crate::serialized::Context;

/// Represents the return value of [`crate::serialized_size`] function.
///
/// It mainly contains the size of serialized bytes in a specific format.
///
/// On Unix platforms, it also contains the number of file descriptors, whose indexes are included
/// in the serialized bytes.
#[derive(Debug)]
pub struct Size {
    size: usize,
    context: Context,
    #[cfg(unix)]
    num_fds: u32,
}

impl Size {
    /// Create a new `Size` instance.
    pub fn new(size: usize, context: Context) -> Self {
        Self {
            size,
            context,
            #[cfg(unix)]
            num_fds: 0,
        }
    }

    /// Set the number of file descriptors.
    #[cfg(unix)]
    pub fn set_num_fds(mut self, num_fds: u32) -> Self {
        self.num_fds = num_fds;
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

    /// The number file descriptors that are references by the serialized bytes.
    ///
    /// This method is only available on Unix platforms.
    #[cfg(unix)]
    pub fn num_fds(&self) -> u32 {
        self.num_fds
    }
}

impl Deref for Size {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.size
    }
}
