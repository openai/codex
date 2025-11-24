use std::fmt;
use std::ops::{Deref, DerefMut};

use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

pub struct IoStatusBlock(IO_STATUS_BLOCK);

cfg_io_source! {
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK_0;

    impl IoStatusBlock {
        pub fn zeroed() -> Self {
            Self(IO_STATUS_BLOCK {
                Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
                Information: 0,
            })
        }
    }
}

unsafe impl Send for IoStatusBlock {}

impl Deref for IoStatusBlock {
    type Target = IO_STATUS_BLOCK;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for IoStatusBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Debug for IoStatusBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IoStatusBlock").finish()
    }
}
