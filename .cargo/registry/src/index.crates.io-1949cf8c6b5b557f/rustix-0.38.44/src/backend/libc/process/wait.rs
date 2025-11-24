use crate::backend::c;

pub(crate) use c::{
    WCONTINUED, WEXITSTATUS, WIFCONTINUED, WIFEXITED, WIFSIGNALED, WIFSTOPPED, WNOHANG, WSTOPSIG,
    WTERMSIG, WUNTRACED,
};

#[cfg(not(any(target_os = "openbsd", target_os = "redox", target_os = "wasi")))]
pub(crate) use c::{WEXITED, WNOWAIT, WSTOPPED};
