#![allow(unreachable_pub)]
//! Mock version of `std::fs::OpenOptions`;
use mockall::mock;

use crate::fs::mocks::MockFile;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::{io, path::Path};

mock! {
    #[derive(Debug)]
    pub OpenOptions {
        pub fn append(&mut self, append: bool) -> &mut Self;
        pub fn create(&mut self, create: bool) -> &mut Self;
        pub fn create_new(&mut self, create_new: bool) -> &mut Self;
        pub fn open<P: AsRef<Path> + 'static>(&self, path: P) -> io::Result<MockFile>;
        pub fn read(&mut self, read: bool) -> &mut Self;
        pub fn truncate(&mut self, truncate: bool) -> &mut Self;
        pub fn write(&mut self, write: bool) -> &mut Self;
    }
    impl Clone for OpenOptions {
        fn clone(&self) -> Self;
    }
    #[cfg(unix)]
    impl OpenOptionsExt for OpenOptions {
        fn custom_flags(&mut self, flags: i32) -> &mut Self;
        fn mode(&mut self, mode: u32) -> &mut Self;
    }
    #[cfg(windows)]
    impl OpenOptionsExt for OpenOptions {
        fn access_mode(&mut self, access: u32) -> &mut Self;
        fn share_mode(&mut self, val: u32) -> &mut Self;
        fn custom_flags(&mut self, flags: u32) -> &mut Self;
        fn attributes(&mut self, val: u32) -> &mut Self;
        fn security_qos_flags(&mut self, flags: u32) -> &mut Self;
    }
}
