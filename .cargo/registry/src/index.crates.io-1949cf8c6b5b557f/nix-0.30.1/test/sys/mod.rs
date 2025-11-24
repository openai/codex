mod test_signal;

// NOTE: DragonFly lacks a kernel-level implementation of Posix AIO as of
// this writing. There is an user-level implementation, but whether aio
// works or not heavily depends on which pthread implementation is chosen
// by the user at link time. For this reason we do not want to run aio test
// cases on DragonFly.
#[cfg(any(
    target_os = "freebsd",
    apple_targets,
    all(
        target_os = "linux",
        not(any(target_env = "uclibc", target_env = "ohos"))
    ),
    target_os = "netbsd"
))]
mod test_aio;
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "hurd",
    target_os = "cygwin"
)))]
mod test_ioctl;
#[cfg(not(target_os = "redox"))]
mod test_mman;
#[cfg(not(target_os = "redox"))]
mod test_select;
#[cfg(target_os = "linux")]
mod test_signalfd;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
mod test_socket;
#[cfg(not(any(target_os = "redox")))]
mod test_sockopt;
mod test_stat;
#[cfg(linux_android)]
mod test_sysinfo;
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
mod test_termios;
mod test_uio;
mod test_wait;

#[cfg(linux_android)]
mod test_epoll;
#[cfg(target_os = "linux")]
mod test_fanotify;
#[cfg(target_os = "linux")]
mod test_inotify;
mod test_pthread;

#[cfg(any(linux_android, freebsdlike, netbsdlike, apple_targets))]
mod test_ptrace;
#[cfg(linux_android)]
mod test_timerfd;

#[cfg(all(
    any(
        target_os = "freebsd",
        solarish,
        target_os = "linux",
        target_os = "netbsd"
    ),
    feature = "time",
    feature = "signal"
))]
mod test_timer;

#[cfg(bsd)]
mod test_event;
mod test_statvfs;
mod test_time;
mod test_utsname;

#[cfg(any(linux_android, freebsdlike, apple_targets, target_os = "openbsd"))]
mod test_statfs;

#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    solarish,
    target_os = "haiku"
)))]
mod test_resource;

// This test module should be enabled for both linux_android and freebsd, but
// the `memfd_create(2)` symbol is not available under Linux QEMU,
//
// https://github.com/nix-rust/nix/actions/runs/9427112650/job/25970870477
//
// and I haven't found a way to stop the linker from linking that symbol, so
// only enable this for FreeBSD for now.
#[cfg(target_os = "freebsd")]
mod test_memfd;
