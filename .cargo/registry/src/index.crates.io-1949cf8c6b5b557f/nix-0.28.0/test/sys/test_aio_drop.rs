// Test dropping an AioCb that hasn't yet finished.
// This must happen in its own process, because on OSX this test seems to hose
// the AIO subsystem and causes subsequent tests to fail
#[test]
#[should_panic(expected = "Dropped an in-progress AioCb")]
#[cfg(all(
    not(target_env = "musl"),
    not(target_env = "uclibc"),
    any(
        target_os = "linux",
        apple_targets,
        target_os = "freebsd",
        target_os = "netbsd"
    )
))]
fn test_drop() {
    use nix::sys::aio::*;
    use nix::sys::signal::*;
    use std::os::unix::io::AsRawFd;
    use tempfile::tempfile;

    const WBUF: &[u8] = b"CDEF";

    let f = tempfile().unwrap();
    f.set_len(6).unwrap();
    let mut aiocb = Box::pin(AioWrite::new(
        f.as_raw_fd(),
        2, //offset
        WBUF,
        0, //priority
        SigevNotify::SigevNone,
    ));
    aiocb.as_mut().submit().unwrap();
}
