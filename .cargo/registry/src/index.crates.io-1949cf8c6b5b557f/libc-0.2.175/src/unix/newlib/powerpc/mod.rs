use crate::prelude::*;

pub type clock_t = c_ulong;
pub type wchar_t = c_int;

pub use crate::unix::newlib::generic::{dirent, sigset_t, stat};

// the newlib shipped with devkitPPC does not support the following components:
// - sockaddr
// - AF_INET6
// - FIONBIO
// - POLL*
// - SOL_SOCKET
// - MSG_*
