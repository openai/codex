use nix::sys::select::*;
use nix::sys::signal::SigSet;
use nix::sys::time::{TimeSpec, TimeVal, TimeValLike};
use nix::unistd::{pipe, write};
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};

#[test]
pub fn test_pselect() {
    let _mtx = crate::SIGNAL_MTX.lock();

    let (r1, w1) = pipe().unwrap();
    write(&w1, b"hi!").unwrap();
    let (r2, _w2) = pipe().unwrap();

    let mut fd_set = FdSet::new();
    fd_set.insert(r1.as_fd());
    fd_set.insert(r2.as_fd());

    let timeout = TimeSpec::seconds(10);
    let sigmask = SigSet::empty();
    assert_eq!(
        1,
        pselect(None, &mut fd_set, None, None, &timeout, &sigmask).unwrap()
    );
    assert!(fd_set.contains(r1.as_fd()));
    assert!(!fd_set.contains(r2.as_fd()));
}

#[test]
pub fn test_pselect_nfds2() {
    let (r1, w1) = pipe().unwrap();
    write(&w1, b"hi!").unwrap();
    let (r2, _w2) = pipe().unwrap();

    let mut fd_set = FdSet::new();
    fd_set.insert(r1.as_fd());
    fd_set.insert(r2.as_fd());

    let timeout = TimeSpec::seconds(10);
    assert_eq!(
        1,
        pselect(
            std::cmp::max(r1.as_raw_fd(), r2.as_raw_fd()) + 1,
            &mut fd_set,
            None,
            None,
            &timeout,
            None
        )
        .unwrap()
    );
    assert!(fd_set.contains(r1.as_fd()));
    assert!(!fd_set.contains(r2.as_fd()));
}

macro_rules! generate_fdset_bad_fd_tests {
    ($fd:expr, $($method:ident),* $(,)?) => {
        $(
            #[test]
            #[should_panic]
            fn $method() {
                let bad_fd = unsafe{BorrowedFd::borrow_raw($fd)};
                FdSet::new().$method(bad_fd);
            }
        )*
    }
}

mod test_fdset_too_large_fd {
    use super::*;
    generate_fdset_bad_fd_tests!(
        FD_SETSIZE.try_into().unwrap(),
        insert,
        remove,
        contains,
    );
}

#[test]
fn fdset_insert() {
    let mut fd_set = FdSet::new();

    for i in 0..FD_SETSIZE {
        let borrowed_i = unsafe { BorrowedFd::borrow_raw(i as RawFd) };
        assert!(!fd_set.contains(borrowed_i));
    }

    let fd_seven = unsafe { BorrowedFd::borrow_raw(7) };
    fd_set.insert(fd_seven);

    assert!(fd_set.contains(fd_seven));
}

#[test]
fn fdset_remove() {
    let mut fd_set = FdSet::new();

    for i in 0..FD_SETSIZE {
        let borrowed_i = unsafe { BorrowedFd::borrow_raw(i as RawFd) };
        assert!(!fd_set.contains(borrowed_i));
    }

    let fd_seven = unsafe { BorrowedFd::borrow_raw(7) };
    fd_set.insert(fd_seven);
    fd_set.remove(fd_seven);

    for i in 0..FD_SETSIZE {
        let borrowed_i = unsafe { BorrowedFd::borrow_raw(i as RawFd) };
        assert!(!fd_set.contains(borrowed_i));
    }
}

#[test]
#[allow(non_snake_case)]
fn fdset_clear() {
    let mut fd_set = FdSet::new();
    let fd_one = unsafe { BorrowedFd::borrow_raw(1) };
    let fd_FD_SETSIZE_divided_by_two =
        unsafe { BorrowedFd::borrow_raw((FD_SETSIZE / 2) as RawFd) };
    let fd_FD_SETSIZE_minus_one =
        unsafe { BorrowedFd::borrow_raw((FD_SETSIZE - 1) as RawFd) };
    fd_set.insert(fd_one);
    fd_set.insert(fd_FD_SETSIZE_divided_by_two);
    fd_set.insert(fd_FD_SETSIZE_minus_one);

    fd_set.clear();

    for i in 0..FD_SETSIZE {
        let borrowed_i = unsafe { BorrowedFd::borrow_raw(i as RawFd) };
        assert!(!fd_set.contains(borrowed_i));
    }
}

#[test]
fn fdset_highest() {
    let mut set = FdSet::new();
    assert_eq!(
        set.highest().map(|borrowed_fd| borrowed_fd.as_raw_fd()),
        None
    );
    let fd_zero = unsafe { BorrowedFd::borrow_raw(0) };
    let fd_ninety = unsafe { BorrowedFd::borrow_raw(90) };
    set.insert(fd_zero);
    assert_eq!(
        set.highest().map(|borrowed_fd| borrowed_fd.as_raw_fd()),
        Some(0)
    );
    set.insert(fd_ninety);
    assert_eq!(
        set.highest().map(|borrowed_fd| borrowed_fd.as_raw_fd()),
        Some(90)
    );
    set.remove(fd_zero);
    assert_eq!(
        set.highest().map(|borrowed_fd| borrowed_fd.as_raw_fd()),
        Some(90)
    );
    set.remove(fd_ninety);
    assert_eq!(
        set.highest().map(|borrowed_fd| borrowed_fd.as_raw_fd()),
        None
    );

    let fd_four = unsafe { BorrowedFd::borrow_raw(4) };
    let fd_five = unsafe { BorrowedFd::borrow_raw(5) };
    let fd_seven = unsafe { BorrowedFd::borrow_raw(7) };
    set.insert(fd_four);
    set.insert(fd_five);
    set.insert(fd_seven);
    assert_eq!(
        set.highest().map(|borrowed_fd| borrowed_fd.as_raw_fd()),
        Some(7)
    );
}

#[test]
fn fdset_fds() {
    let mut set = FdSet::new();
    let fd_zero = unsafe { BorrowedFd::borrow_raw(0) };
    let fd_ninety = unsafe { BorrowedFd::borrow_raw(90) };
    assert_eq!(
        set.fds(None)
            .map(|borrowed_fd| borrowed_fd.as_raw_fd())
            .collect::<Vec<_>>(),
        vec![]
    );
    set.insert(fd_zero);
    assert_eq!(
        set.fds(None)
            .map(|borrowed_fd| borrowed_fd.as_raw_fd())
            .collect::<Vec<_>>(),
        vec![0]
    );
    set.insert(fd_ninety);
    assert_eq!(
        set.fds(None)
            .map(|borrowed_fd| borrowed_fd.as_raw_fd())
            .collect::<Vec<_>>(),
        vec![0, 90]
    );

    // highest limit
    assert_eq!(
        set.fds(Some(89))
            .map(|borrowed_fd| borrowed_fd.as_raw_fd())
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        set.fds(Some(90))
            .map(|borrowed_fd| borrowed_fd.as_raw_fd())
            .collect::<Vec<_>>(),
        vec![0, 90]
    );
}

#[test]
fn test_select() {
    let (r1, w1) = pipe().unwrap();
    let (r2, _w2) = pipe().unwrap();

    write(&w1, b"hi!").unwrap();
    let mut fd_set = FdSet::new();
    fd_set.insert(r1.as_fd());
    fd_set.insert(r2.as_fd());

    let mut timeout = TimeVal::seconds(10);
    assert_eq!(
        1,
        select(None, &mut fd_set, None, None, &mut timeout).unwrap()
    );
    assert!(fd_set.contains(r1.as_fd()));
    assert!(!fd_set.contains(r2.as_fd()));
}

#[test]
fn test_select_nfds() {
    let (r1, w1) = pipe().unwrap();
    let (r2, _w2) = pipe().unwrap();

    write(&w1, b"hi!").unwrap();
    let mut fd_set = FdSet::new();
    fd_set.insert(r1.as_fd());
    fd_set.insert(r2.as_fd());

    let mut timeout = TimeVal::seconds(10);
    {
        assert_eq!(
            1,
            select(
                Some(
                    fd_set
                        .highest()
                        .map(|borrowed_fd| borrowed_fd.as_raw_fd())
                        .unwrap()
                        + 1
                ),
                &mut fd_set,
                None,
                None,
                &mut timeout
            )
            .unwrap()
        );
    }
    assert!(fd_set.contains(r1.as_fd()));
    assert!(!fd_set.contains(r2.as_fd()));
}

#[test]
fn test_select_nfds2() {
    let (r1, w1) = pipe().unwrap();
    write(&w1, b"hi!").unwrap();
    let (r2, _w2) = pipe().unwrap();
    let mut fd_set = FdSet::new();
    fd_set.insert(r1.as_fd());
    fd_set.insert(r2.as_fd());

    let mut timeout = TimeVal::seconds(10);
    assert_eq!(
        1,
        select(
            std::cmp::max(r1.as_raw_fd(), r2.as_raw_fd()) + 1,
            &mut fd_set,
            None,
            None,
            &mut timeout
        )
        .unwrap()
    );
    assert!(fd_set.contains(r1.as_fd()));
    assert!(!fd_set.contains(r2.as_fd()));
}
