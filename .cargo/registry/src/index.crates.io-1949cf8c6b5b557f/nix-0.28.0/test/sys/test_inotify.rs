use nix::errno::Errno;
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use std::ffi::OsString;
use std::fs::{rename, File};

#[test]
pub fn test_inotify() {
    let instance = Inotify::init(InitFlags::IN_NONBLOCK).unwrap();
    let tempdir = tempfile::tempdir().unwrap();

    instance
        .add_watch(tempdir.path(), AddWatchFlags::IN_ALL_EVENTS)
        .unwrap();

    let events = instance.read_events();
    assert_eq!(events.unwrap_err(), Errno::EAGAIN);

    File::create(tempdir.path().join("test")).unwrap();

    let events = instance.read_events().unwrap();
    assert_eq!(events[0].name, Some(OsString::from("test")));
}

#[test]
pub fn test_inotify_multi_events() {
    let instance = Inotify::init(InitFlags::IN_NONBLOCK).unwrap();
    let tempdir = tempfile::tempdir().unwrap();

    instance
        .add_watch(tempdir.path(), AddWatchFlags::IN_ALL_EVENTS)
        .unwrap();

    let events = instance.read_events();
    assert_eq!(events.unwrap_err(), Errno::EAGAIN);

    File::create(tempdir.path().join("test")).unwrap();
    rename(tempdir.path().join("test"), tempdir.path().join("test2")).unwrap();

    // Now there should be 5 events in queue:
    //   - IN_CREATE on test
    //   - IN_OPEN on test
    //   - IN_CLOSE_WRITE on test
    //   - IN_MOVED_FROM on test with a cookie
    //   - IN_MOVED_TO on test2 with the same cookie

    let events = instance.read_events().unwrap();
    assert_eq!(events.len(), 5);

    assert_eq!(events[0].mask, AddWatchFlags::IN_CREATE);
    assert_eq!(events[0].name, Some(OsString::from("test")));

    assert_eq!(events[1].mask, AddWatchFlags::IN_OPEN);
    assert_eq!(events[1].name, Some(OsString::from("test")));

    assert_eq!(events[2].mask, AddWatchFlags::IN_CLOSE_WRITE);
    assert_eq!(events[2].name, Some(OsString::from("test")));

    assert_eq!(events[3].mask, AddWatchFlags::IN_MOVED_FROM);
    assert_eq!(events[3].name, Some(OsString::from("test")));

    assert_eq!(events[4].mask, AddWatchFlags::IN_MOVED_TO);
    assert_eq!(events[4].name, Some(OsString::from("test2")));

    assert_eq!(events[3].cookie, events[4].cookie);
}
