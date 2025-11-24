pub(crate) mod addr;
pub(crate) mod ext;
#[cfg(not(any(
    windows,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) mod msghdr;
#[cfg(linux_kernel)]
pub(crate) mod netdevice;
pub(crate) mod read_sockaddr;
pub(crate) mod send_recv;
#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) mod sockopt;
pub(crate) mod syscalls;
pub(crate) mod write_sockaddr;
