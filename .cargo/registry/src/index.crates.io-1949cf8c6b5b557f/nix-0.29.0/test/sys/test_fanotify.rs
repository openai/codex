use crate::*;
use nix::errno::Errno;
use nix::sys::fanotify::{
    EventFFlags, Fanotify, FanotifyResponse, InitFlags, MarkFlags, MaskFlags,
    Response,
};
use std::fs::{read_link, read_to_string, File, OpenOptions};
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::thread;

#[test]
/// Run fanotify tests sequentially to avoid tmp files races
pub fn test_fanotify() {
    require_capability!("test_fanotify", CAP_SYS_ADMIN);

    test_fanotify_notifications();
    test_fanotify_responses();
    test_fanotify_overflow();
}

fn test_fanotify_notifications() {
    let group =
        Fanotify::init(InitFlags::FAN_CLASS_NOTIF, EventFFlags::O_RDONLY)
            .unwrap();
    let tempdir = tempfile::tempdir().unwrap();
    let tempfile = tempdir.path().join("test");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tempfile)
        .unwrap();

    group
        .mark(
            MarkFlags::FAN_MARK_ADD,
            MaskFlags::FAN_OPEN | MaskFlags::FAN_MODIFY | MaskFlags::FAN_CLOSE,
            None,
            Some(&tempfile),
        )
        .unwrap();

    // modify test file
    {
        let mut f = OpenOptions::new().write(true).open(&tempfile).unwrap();
        f.write_all(b"hello").unwrap();
    }

    let mut events = group.read_events().unwrap();
    assert_eq!(events.len(), 1, "should have read exactly one event");
    let event = events.pop().unwrap();
    assert!(event.check_version());
    assert_eq!(
        event.mask(),
        MaskFlags::FAN_OPEN
            | MaskFlags::FAN_MODIFY
            | MaskFlags::FAN_CLOSE_WRITE
    );
    let fd_opt = event.fd();
    let fd = fd_opt.as_ref().unwrap();
    let path = read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).unwrap();
    assert_eq!(path, tempfile);

    // read test file
    {
        let mut f = File::open(&tempfile).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
    }

    let mut events = group.read_events().unwrap();
    assert_eq!(events.len(), 1, "should have read exactly one event");
    let event = events.pop().unwrap();
    assert!(event.check_version());
    assert_eq!(
        event.mask(),
        MaskFlags::FAN_OPEN | MaskFlags::FAN_CLOSE_NOWRITE
    );
    let fd_opt = event.fd();
    let fd = fd_opt.as_ref().unwrap();
    let path = read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).unwrap();
    assert_eq!(path, tempfile);
}

fn test_fanotify_responses() {
    let group =
        Fanotify::init(InitFlags::FAN_CLASS_CONTENT, EventFFlags::O_RDONLY)
            .unwrap();
    let tempdir = tempfile::tempdir().unwrap();
    let tempfile = tempdir.path().join("test");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tempfile)
        .unwrap();

    group
        .mark(
            MarkFlags::FAN_MARK_ADD,
            MaskFlags::FAN_OPEN_PERM,
            None,
            Some(&tempfile),
        )
        .unwrap();

    let file_thread = thread::spawn({
        let tempfile = tempfile.clone();

        move || {
            // first open, should fail
            let Err(e) = File::open(&tempfile) else {
                panic!("The first open should fail");
            };
            assert_eq!(e.kind(), ErrorKind::PermissionDenied);

            // second open, should succeed
            File::open(&tempfile).unwrap();
        }
    });

    // Deny the first open try
    let mut events = group.read_events().unwrap();
    assert_eq!(events.len(), 1, "should have read exactly one event");
    let event = events.pop().unwrap();
    assert!(event.check_version());
    assert_eq!(event.mask(), MaskFlags::FAN_OPEN_PERM);
    let fd_opt = event.fd();
    let fd = fd_opt.as_ref().unwrap();
    let path = read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).unwrap();
    assert_eq!(path, tempfile);
    group
        .write_response(FanotifyResponse::new(*fd, Response::FAN_DENY))
        .unwrap();

    // Allow the second open try
    let mut events = group.read_events().unwrap();
    assert_eq!(events.len(), 1, "should have read exactly one event");
    let event = events.pop().unwrap();
    assert!(event.check_version());
    assert_eq!(event.mask(), MaskFlags::FAN_OPEN_PERM);
    let fd_opt = event.fd();
    let fd = fd_opt.as_ref().unwrap();
    let path = read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).unwrap();
    assert_eq!(path, tempfile);
    group
        .write_response(FanotifyResponse::new(*fd, Response::FAN_ALLOW))
        .unwrap();

    file_thread.join().unwrap();
}

fn test_fanotify_overflow() {
    let max_events: usize =
        read_to_string("/proc/sys/fs/fanotify/max_queued_events")
            .unwrap()
            .trim()
            .parse()
            .unwrap();

    // make sure the kernel is configured with the default value,
    // just so this test doesn't run forever
    assert_eq!(max_events, 16384);

    let group = Fanotify::init(
        InitFlags::FAN_CLASS_NOTIF
            | InitFlags::FAN_REPORT_TID
            | InitFlags::FAN_NONBLOCK,
        EventFFlags::O_RDONLY,
    )
    .unwrap();
    let tempdir = tempfile::tempdir().unwrap();
    let tempfile = tempdir.path().join("test");

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tempfile)
        .unwrap();

    group
        .mark(
            MarkFlags::FAN_MARK_ADD,
            MaskFlags::FAN_OPEN,
            None,
            Some(&tempfile),
        )
        .unwrap();

    thread::scope(|s| {
        // perform 10 more events to demonstrate some will be dropped
        for _ in 0..(max_events + 10) {
            s.spawn(|| {
                File::open(&tempfile).unwrap();
            });
        }
    });

    // flush the queue until it's empty
    let mut n = 0;
    let mut last_event = None;
    loop {
        match group.read_events() {
            Ok(events) => {
                n += events.len();
                if let Some(event) = events.last() {
                    last_event = Some(event.mask());
                }
            }
            Err(e) if e == Errno::EWOULDBLOCK => break,
            Err(e) => panic!("{e:?}"),
        }
    }

    // make sure we read all we expected.
    // the +1 is for the overflow event.
    assert_eq!(n, max_events + 1);
    assert_eq!(last_event, Some(MaskFlags::FAN_Q_OVERFLOW));
}
