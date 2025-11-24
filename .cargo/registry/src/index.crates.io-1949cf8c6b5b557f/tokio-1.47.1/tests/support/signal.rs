pub fn send_signal(signal: libc::c_int) {
    use libc::{getpid, kill};

    unsafe {
        let pid = getpid();
        assert_eq!(
            kill(pid, signal),
            0,
            "kill(pid = {}, {}) failed with error: {}",
            pid,
            signal,
            std::io::Error::last_os_error(),
        );
    }
}
