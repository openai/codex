use cfg_if::cfg_if;
use std::str;

use nix::errno::Errno;
use nix::mqueue::{
    mq_attr_member_t, mq_close, mq_open, mq_receive, mq_send, mq_timedreceive,
};
use nix::mqueue::{MQ_OFlag, MqAttr};
use nix::sys::stat::Mode;
use nix::sys::time::{TimeSpec, TimeValLike};
use nix::time::{clock_gettime, ClockId};

// Defined as a macro such that the error source is reported as the caller's location.
macro_rules! assert_attr_eq {
    ($read_attr:ident, $initial_attr:ident) => {
        cfg_if! {
            if #[cfg(any(target_os = "dragonfly", target_os = "netbsd"))] {
                // NetBSD (and others which inherit its implementation) include other flags
                // in read_attr, such as those specified by oflag. Just make sure at least
                // the correct bits are set.
                assert_eq!($read_attr.flags() & $initial_attr.flags(), $initial_attr.flags());
                assert_eq!($read_attr.maxmsg(), $initial_attr.maxmsg());
                assert_eq!($read_attr.msgsize(), $initial_attr.msgsize());
                assert_eq!($read_attr.curmsgs(), $initial_attr.curmsgs());
            } else {
                assert_eq!($read_attr, $initial_attr);
            }
        }
    }
}

#[test]
fn test_mq_send_and_receive() {
    const MSG_SIZE: mq_attr_member_t = 32;
    let attr = MqAttr::new(0, 10, MSG_SIZE, 0);
    let mq_name = "/a_nix_test_queue";

    let oflag0 = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
    let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let r0 = mq_open(mq_name, oflag0, mode, Some(&attr));
    if let Err(Errno::ENOSYS) = r0 {
        println!("message queues not supported or module not loaded?");
        return;
    };
    let mqd0 = r0.unwrap();
    let msg_to_send = "msg_1";
    mq_send(&mqd0, msg_to_send.as_bytes(), 1).unwrap();

    let oflag1 = MQ_OFlag::O_CREAT | MQ_OFlag::O_RDONLY;
    let mqd1 = mq_open(mq_name, oflag1, mode, Some(&attr)).unwrap();
    let mut buf = [0u8; 32];
    let mut prio = 0u32;
    let len = mq_receive(&mqd1, &mut buf, &mut prio).unwrap();
    assert_eq!(prio, 1);

    mq_close(mqd1).unwrap();
    mq_close(mqd0).unwrap();
    assert_eq!(msg_to_send, str::from_utf8(&buf[0..len]).unwrap());
}

#[test]
fn test_mq_timedreceive() {
    const MSG_SIZE: mq_attr_member_t = 32;
    let attr = MqAttr::new(0, 10, MSG_SIZE, 0);
    let mq_name = "/a_nix_test_queue";

    let oflag0 = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
    let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let r0 = mq_open(mq_name, oflag0, mode, Some(&attr));
    if let Err(Errno::ENOSYS) = r0 {
        println!("message queues not supported or module not loaded?");
        return;
    };
    let mqd0 = r0.unwrap();
    let msg_to_send = "msg_1";
    mq_send(&mqd0, msg_to_send.as_bytes(), 1).unwrap();

    let oflag1 = MQ_OFlag::O_CREAT | MQ_OFlag::O_RDONLY;
    let mqd1 = mq_open(mq_name, oflag1, mode, Some(&attr)).unwrap();
    let mut buf = [0u8; 32];
    let mut prio = 0u32;
    let abstime =
        clock_gettime(ClockId::CLOCK_REALTIME).unwrap() + TimeSpec::seconds(1);
    let len = mq_timedreceive(&mqd1, &mut buf, &mut prio, &abstime).unwrap();
    assert_eq!(prio, 1);

    mq_close(mqd1).unwrap();
    mq_close(mqd0).unwrap();
    assert_eq!(msg_to_send, str::from_utf8(&buf[0..len]).unwrap());
}

#[test]
fn test_mq_getattr() {
    use nix::mqueue::mq_getattr;
    const MSG_SIZE: mq_attr_member_t = 32;
    let initial_attr = MqAttr::new(0, 10, MSG_SIZE, 0);
    let mq_name = "/attr_test_get_attr";
    let oflag = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
    let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let r = mq_open(mq_name, oflag, mode, Some(&initial_attr));
    if let Err(Errno::ENOSYS) = r {
        println!("message queues not supported or module not loaded?");
        return;
    };
    let mqd = r.unwrap();

    let read_attr = mq_getattr(&mqd).unwrap();
    assert_attr_eq!(read_attr, initial_attr);
    mq_close(mqd).unwrap();
}

// FIXME: Fix failures for mips in QEMU
#[test]
#[cfg_attr(
    all(
        qemu,
        any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    ),
    ignore
)]
fn test_mq_setattr() {
    use nix::mqueue::{mq_getattr, mq_setattr};
    const MSG_SIZE: mq_attr_member_t = 32;
    let initial_attr = MqAttr::new(0, 10, MSG_SIZE, 0);
    let mq_name = "/attr_test_get_attr";
    let oflag = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
    let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let r = mq_open(mq_name, oflag, mode, Some(&initial_attr));
    if let Err(Errno::ENOSYS) = r {
        println!("message queues not supported or module not loaded?");
        return;
    };
    let mqd = r.unwrap();

    let new_attr = MqAttr::new(0, 20, MSG_SIZE * 2, 100);
    let old_attr = mq_setattr(&mqd, &new_attr).unwrap();
    assert_attr_eq!(old_attr, initial_attr);

    // No changes here because according to the Linux man page only
    // O_NONBLOCK can be set (see tests below)
    #[cfg(not(any(target_os = "dragonfly", target_os = "netbsd")))]
    {
        let new_attr_get = mq_getattr(&mqd).unwrap();
        assert_ne!(new_attr_get, new_attr);
    }

    let new_attr_non_blocking = MqAttr::new(
        MQ_OFlag::O_NONBLOCK.bits() as mq_attr_member_t,
        10,
        MSG_SIZE,
        0,
    );
    mq_setattr(&mqd, &new_attr_non_blocking).unwrap();
    let new_attr_get = mq_getattr(&mqd).unwrap();

    // now the O_NONBLOCK flag has been set
    #[cfg(not(any(target_os = "dragonfly", target_os = "netbsd")))]
    {
        assert_ne!(new_attr_get, initial_attr);
    }
    assert_attr_eq!(new_attr_get, new_attr_non_blocking);
    mq_close(mqd).unwrap();
}

// FIXME: Fix failures for mips in QEMU
#[test]
#[cfg_attr(
    all(
        qemu,
        any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    ),
    ignore
)]
fn test_mq_set_nonblocking() {
    use nix::mqueue::{mq_getattr, mq_remove_nonblock, mq_set_nonblock};
    const MSG_SIZE: mq_attr_member_t = 32;
    let initial_attr = MqAttr::new(0, 10, MSG_SIZE, 0);
    let mq_name = "/attr_test_get_attr";
    let oflag = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
    let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let r = mq_open(mq_name, oflag, mode, Some(&initial_attr));
    if let Err(Errno::ENOSYS) = r {
        println!("message queues not supported or module not loaded?");
        return;
    };
    let mqd = r.unwrap();
    mq_set_nonblock(&mqd).unwrap();
    let new_attr = mq_getattr(&mqd);
    let o_nonblock_bits = MQ_OFlag::O_NONBLOCK.bits() as mq_attr_member_t;
    assert_eq!(new_attr.unwrap().flags() & o_nonblock_bits, o_nonblock_bits);
    mq_remove_nonblock(&mqd).unwrap();
    let new_attr = mq_getattr(&mqd);
    assert_eq!(new_attr.unwrap().flags() & o_nonblock_bits, 0);
    mq_close(mqd).unwrap();
}

#[test]
fn test_mq_unlink() {
    use nix::mqueue::mq_unlink;
    const MSG_SIZE: mq_attr_member_t = 32;
    let initial_attr = MqAttr::new(0, 10, MSG_SIZE, 0);
    let mq_name_opened = "/mq_unlink_test";
    #[cfg(not(any(target_os = "dragonfly", target_os = "netbsd")))]
    let mq_name_not_opened = "/mq_unlink_test";
    let oflag = MQ_OFlag::O_CREAT | MQ_OFlag::O_WRONLY;
    let mode = Mode::S_IWUSR | Mode::S_IRUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let r = mq_open(mq_name_opened, oflag, mode, Some(&initial_attr));
    if let Err(Errno::ENOSYS) = r {
        println!("message queues not supported or module not loaded?");
        return;
    };
    let mqd = r.unwrap();

    let res_unlink = mq_unlink(mq_name_opened);
    assert_eq!(res_unlink, Ok(()));

    // NetBSD (and others which inherit its implementation) defer removing the message
    // queue name until all references are closed, whereas Linux and others remove the
    // message queue name immediately.
    #[cfg(not(any(target_os = "dragonfly", target_os = "netbsd")))]
    {
        let res_unlink_not_opened = mq_unlink(mq_name_not_opened);
        assert_eq!(res_unlink_not_opened, Err(Errno::ENOENT));
    }

    mq_close(mqd).unwrap();
    let res_unlink_after_close = mq_unlink(mq_name_opened);
    assert_eq!(res_unlink_after_close, Err(Errno::ENOENT));
}
