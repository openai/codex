use crate::backend::c;

pub(crate) use c::{
    WEXITSTATUS, WIFCONTINUED, WIFEXITED, WIFSIGNALED, WIFSTOPPED, WNOHANG, WSTOPSIG, WTERMSIG,
};

#[cfg(not(target_os = "horizon"))]
pub(crate) use c::{WCONTINUED, WUNTRACED};

#[cfg(not(any(
    target_os = "cygwin",
    target_os = "horizon",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "wasi",
)))]
pub(crate) use c::{WEXITED, WNOWAIT, WSTOPPED};
