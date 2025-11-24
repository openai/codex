use nix::errno::Errno;

#[test]
fn errno_set_and_read() {
    Errno::ENFILE.set();
    assert_eq!(Errno::last(), Errno::ENFILE);
}

#[test]
fn errno_set_and_clear() {
    Errno::ENFILE.set();
    assert_eq!(Errno::last(), Errno::ENFILE);

    Errno::clear();
    assert_eq!(Errno::last(), Errno::from_raw(0));
}
