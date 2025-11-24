use nix::{
    errno::Errno,
    poll::{poll, PollFd, PollFlags, PollTimeout},
    unistd::{pipe, write},
};
use std::os::unix::io::{AsFd, BorrowedFd};

macro_rules! loop_while_eintr {
    ($poll_expr: expr) => {
        loop {
            match $poll_expr {
                Ok(nfds) => break nfds,
                Err(Errno::EINTR) => (),
                Err(e) => panic!("{}", e),
            }
        }
    };
}

#[test]
fn test_poll() {
    let (r, w) = pipe().unwrap();
    let mut fds = [PollFd::new(r.as_fd(), PollFlags::POLLIN)];

    // Poll an idle pipe.  Should timeout
    let nfds = loop_while_eintr!(poll(&mut fds, PollTimeout::from(100u8)));
    assert_eq!(nfds, 0);
    assert!(!fds[0].revents().unwrap().contains(PollFlags::POLLIN));

    write(&w, b".").unwrap();

    // Poll a readable pipe.  Should return an event.
    let nfds = poll(&mut fds, PollTimeout::from(100u8)).unwrap();
    assert_eq!(nfds, 1);
    assert!(fds[0].revents().unwrap().contains(PollFlags::POLLIN));
}

// ppoll(2) is the same as poll except for how it handles timeouts and signals.
// Repeating the test for poll(2) should be sufficient to check that our
// bindings are correct.
#[cfg(any(linux_android, freebsdlike))]
#[test]
fn test_ppoll() {
    use nix::poll::ppoll;
    use nix::sys::signal::SigSet;
    use nix::sys::time::{TimeSpec, TimeValLike};

    let timeout = TimeSpec::milliseconds(1);
    let (r, w) = pipe().unwrap();
    let mut fds = [PollFd::new(r.as_fd(), PollFlags::POLLIN)];

    // Poll an idle pipe.  Should timeout
    let sigset = SigSet::empty();
    let nfds = loop_while_eintr!(ppoll(&mut fds, Some(timeout), Some(sigset)));
    assert_eq!(nfds, 0);
    assert!(!fds[0].revents().unwrap().contains(PollFlags::POLLIN));

    write(&w, b".").unwrap();

    // Poll a readable pipe.  Should return an event.
    let nfds = ppoll(&mut fds, Some(timeout), None).unwrap();
    assert_eq!(nfds, 1);
    assert!(fds[0].revents().unwrap().contains(PollFlags::POLLIN));
}

#[test]
fn test_pollfd_events() {
    let fd_zero = unsafe { BorrowedFd::borrow_raw(0) };
    let mut pfd = PollFd::new(fd_zero.as_fd(), PollFlags::POLLIN);
    assert_eq!(pfd.events(), PollFlags::POLLIN);
    pfd.set_events(PollFlags::POLLOUT);
    assert_eq!(pfd.events(), PollFlags::POLLOUT);
}
