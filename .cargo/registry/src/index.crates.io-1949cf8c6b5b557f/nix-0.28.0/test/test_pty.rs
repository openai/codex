use std::fs::File;
use std::io::{stdout, Read, Write};
use std::os::unix::prelude::*;
use std::path::Path;

use libc::_exit;
use nix::fcntl::{open, OFlag};
use nix::pty::*;
use nix::sys::stat;
use nix::sys::termios::*;
use nix::unistd::{pause, write};

/// Test equivalence of `ptsname` and `ptsname_r`
#[test]
#[cfg(linux_android)]
fn test_ptsname_equivalence() {
    let _m = crate::PTSNAME_MTX.lock();

    // Open a new PTTY master
    let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();
    assert!(master_fd.as_raw_fd() > 0);

    // Get the name of the slave
    let slave_name = unsafe { ptsname(&master_fd) }.unwrap();
    let slave_name_r = ptsname_r(&master_fd).unwrap();
    assert_eq!(slave_name, slave_name_r);
}

/// Test data copying of `ptsname`
// TODO need to run in a subprocess, since ptsname is non-reentrant
#[test]
#[cfg(linux_android)]
fn test_ptsname_copy() {
    let _m = crate::PTSNAME_MTX.lock();

    // Open a new PTTY master
    let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();

    // Get the name of the slave
    let slave_name1 = unsafe { ptsname(&master_fd) }.unwrap();
    let slave_name2 = unsafe { ptsname(&master_fd) }.unwrap();
    assert_eq!(slave_name1, slave_name2);
    // Also make sure that the string was actually copied and they point to different parts of
    // memory.
    assert_ne!(slave_name1.as_ptr(), slave_name2.as_ptr());
}

/// Test data copying of `ptsname_r`
#[test]
#[cfg(linux_android)]
fn test_ptsname_r_copy() {
    // Open a new PTTY master
    let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();

    // Get the name of the slave
    let slave_name1 = ptsname_r(&master_fd).unwrap();
    let slave_name2 = ptsname_r(&master_fd).unwrap();
    assert_eq!(slave_name1, slave_name2);
    assert_ne!(slave_name1.as_ptr(), slave_name2.as_ptr());
}

/// Test that `ptsname` returns different names for different devices
#[test]
#[cfg(linux_android)]
fn test_ptsname_unique() {
    let _m = crate::PTSNAME_MTX.lock();

    // Open a new PTTY master
    let master1_fd = posix_openpt(OFlag::O_RDWR).unwrap();

    // Open a second PTTY master
    let master2_fd = posix_openpt(OFlag::O_RDWR).unwrap();

    // Get the name of the slave
    let slave_name1 = unsafe { ptsname(&master1_fd) }.unwrap();
    let slave_name2 = unsafe { ptsname(&master2_fd) }.unwrap();
    assert_ne!(slave_name1, slave_name2);
}

/// Common setup for testing PTTY pairs
fn open_ptty_pair() -> (PtyMaster, File) {
    let _m = crate::PTSNAME_MTX.lock();

    // Open a new PTTY master
    let master = posix_openpt(OFlag::O_RDWR).expect("posix_openpt failed");

    // Allow a slave to be generated for it
    grantpt(&master).expect("grantpt failed");
    unlockpt(&master).expect("unlockpt failed");

    // Get the name of the slave
    let slave_name = unsafe { ptsname(&master) }.expect("ptsname failed");

    // Open the slave device
    let slave_fd =
        open(Path::new(&slave_name), OFlag::O_RDWR, stat::Mode::empty())
            .unwrap();

    #[cfg(solarish)]
    // TODO: rewrite using ioctl!
    #[allow(clippy::comparison_chain)]
    {
        use libc::{ioctl, I_FIND, I_PUSH};

        // On illumos systems, as per pts(7D), one must push STREAMS modules
        // after opening a device path returned from ptsname().
        let ptem = b"ptem\0";
        let ldterm = b"ldterm\0";
        let r = unsafe { ioctl(slave_fd, I_FIND, ldterm.as_ptr()) };
        if r < 0 {
            panic!("I_FIND failure");
        } else if r == 0 {
            if unsafe { ioctl(slave_fd, I_PUSH, ptem.as_ptr()) } < 0 {
                panic!("I_PUSH ptem failure");
            }
            if unsafe { ioctl(slave_fd, I_PUSH, ldterm.as_ptr()) } < 0 {
                panic!("I_PUSH ldterm failure");
            }
        }
    }

    let slave = unsafe { File::from_raw_fd(slave_fd) };

    (master, slave)
}

/// Test opening a master/slave PTTY pair
///
/// This uses a common `open_ptty_pair` because much of these functions aren't useful by
/// themselves. So for this test we perform the basic act of getting a file handle for a
/// master/slave PTTY pair.
#[test]
fn test_open_ptty_pair() {
    let (_, _) = open_ptty_pair();
}

/// Put the terminal in raw mode.
fn make_raw<Fd: AsFd>(fd: Fd) {
    let mut termios = tcgetattr(&fd).unwrap();
    cfmakeraw(&mut termios);
    tcsetattr(&fd, SetArg::TCSANOW, &termios).unwrap();
}

/// Test `io::Read` on the PTTY master
#[test]
fn test_read_ptty_pair() {
    let (mut master, mut slave) = open_ptty_pair();
    make_raw(&slave);

    let mut buf = [0u8; 5];
    slave.write_all(b"hello").unwrap();
    master.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");

    let mut master = &master;
    slave.write_all(b"hello").unwrap();
    master.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
}

/// Test `io::Write` on the PTTY master
#[test]
fn test_write_ptty_pair() {
    let (mut master, mut slave) = open_ptty_pair();
    make_raw(&slave);

    let mut buf = [0u8; 5];
    master.write_all(b"adios").unwrap();
    slave.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"adios");

    let mut master = &master;
    master.write_all(b"adios").unwrap();
    slave.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"adios");
}

#[test]
fn test_openpty() {
    // openpty uses ptname(3) internally
    let _m = crate::PTSNAME_MTX.lock();

    let pty = openpty(None, None).unwrap();

    // Writing to one should be readable on the other one
    let string = "foofoofoo\n";
    let mut buf = [0u8; 10];
    write(&pty.master, string.as_bytes()).unwrap();
    crate::read_exact(&pty.slave, &mut buf);

    assert_eq!(&buf, string.as_bytes());

    // Read the echo as well
    let echoed_string = "foofoofoo\r\n";
    let mut buf = [0u8; 11];
    crate::read_exact(&pty.master, &mut buf);
    assert_eq!(&buf, echoed_string.as_bytes());

    let string2 = "barbarbarbar\n";
    let echoed_string2 = "barbarbarbar\r\n";
    let mut buf = [0u8; 14];
    write(&pty.slave, string2.as_bytes()).unwrap();
    crate::read_exact(&pty.master, &mut buf);

    assert_eq!(&buf, echoed_string2.as_bytes());
}

#[test]
fn test_openpty_with_termios() {
    // openpty uses ptname(3) internally
    let _m = crate::PTSNAME_MTX.lock();

    // Open one pty to get attributes for the second one
    let mut termios = {
        let pty = openpty(None, None).unwrap();
        tcgetattr(&pty.slave).unwrap()
    };
    // Make sure newlines are not transformed so the data is preserved when sent.
    termios.output_flags.remove(OutputFlags::ONLCR);

    let pty = openpty(None, &termios).unwrap();
    // Must be valid file descriptors

    // Writing to one should be readable on the other one
    let string = "foofoofoo\n";
    let mut buf = [0u8; 10];
    write(&pty.master, string.as_bytes()).unwrap();
    crate::read_exact(&pty.slave, &mut buf);

    assert_eq!(&buf, string.as_bytes());

    // read the echo as well
    let echoed_string = "foofoofoo\n";
    crate::read_exact(&pty.master, &mut buf);
    assert_eq!(&buf, echoed_string.as_bytes());

    let string2 = "barbarbarbar\n";
    let echoed_string2 = "barbarbarbar\n";
    let mut buf = [0u8; 13];
    write(&pty.slave, string2.as_bytes()).unwrap();
    crate::read_exact(&pty.master, &mut buf);

    assert_eq!(&buf, echoed_string2.as_bytes());
}

#[test]
fn test_forkpty() {
    use nix::sys::signal::*;
    use nix::sys::wait::wait;
    use nix::unistd::ForkResult::*;
    // forkpty calls openpty which uses ptname(3) internally.
    let _m0 = crate::PTSNAME_MTX.lock();
    // forkpty spawns a child process
    let _m1 = crate::FORK_MTX.lock();

    let string = "naninani\n";
    let echoed_string = "naninani\r\n";
    let pty = unsafe { forkpty(None, None).unwrap() };
    match pty.fork_result {
        Child => {
            write(stdout(), string.as_bytes()).unwrap();
            pause(); // we need the child to stay alive until the parent calls read
            unsafe {
                _exit(0);
            }
        }
        Parent { child } => {
            let mut buf = [0u8; 10];
            assert!(child.as_raw() > 0);
            crate::read_exact(&pty.master, &mut buf);
            kill(child, SIGTERM).unwrap();
            wait().unwrap(); // keep other tests using generic wait from getting our child
            assert_eq!(&buf, echoed_string.as_bytes());
        }
    }
}
