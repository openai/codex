#![allow(dead_code)]

// Simple tests to ensure macro generated fns compile
ioctl_none_bad!(do_bad, 0x1234);
ioctl_read_bad!(do_bad_read, 0x1234, u16);
ioctl_write_int_bad!(do_bad_write_int, 0x1234);
ioctl_write_ptr_bad!(do_bad_write_ptr, 0x1234, u8);
ioctl_readwrite_bad!(do_bad_readwrite, 0x1234, u32);
ioctl_none!(do_none, 0, 0);
ioctl_read!(read_test, 0, 0, u32);
ioctl_write_int!(write_ptr_int, 0, 0);
ioctl_write_ptr!(write_ptr_u8, 0, 0, u8);
ioctl_write_ptr!(write_ptr_u32, 0, 0, u32);
ioctl_write_ptr!(write_ptr_u64, 0, 0, u64);
ioctl_readwrite!(readwrite_test, 0, 0, u64);
ioctl_read_buf!(readbuf_test, 0, 0, u32);
const SPI_IOC_MAGIC: u8 = b'k';
const SPI_IOC_MESSAGE: u8 = 0;
ioctl_write_buf!(writebuf_test_consts, SPI_IOC_MAGIC, SPI_IOC_MESSAGE, u8);
ioctl_write_buf!(writebuf_test_u8, 0, 0, u8);
ioctl_write_buf!(writebuf_test_u32, 0, 0, u32);
ioctl_write_buf!(writebuf_test_u64, 0, 0, u64);
ioctl_readwrite_buf!(readwritebuf_test, 0, 0, u32);

// See C code for source of values for op calculations (does NOT work for mips/powerpc):
// https://gist.github.com/posborne/83ea6880770a1aef332e
//
// TODO:  Need a way to compute these constants at test time.  Using precomputed
// values is fragile and needs to be maintained.

#[cfg(linux_android)]
mod linux {
    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    #[test]
    fn test_op_none() {
        if cfg!(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )) {
            assert_eq!(request_code_none!(b'q', 10) as u32, 0x2000_710A);
            assert_eq!(request_code_none!(b'a', 255) as u32, 0x2000_61FF);
        } else {
            assert_eq!(request_code_none!(b'q', 10) as u32, 0x0000_710A);
            assert_eq!(request_code_none!(b'a', 255) as u32, 0x0000_61FF);
        }
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    #[test]
    fn test_op_write() {
        if cfg!(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )) {
            assert_eq!(request_code_write!(b'z', 10, 1) as u32, 0x8001_7A0A);
            assert_eq!(request_code_write!(b'z', 10, 512) as u32, 0x8200_7A0A);
        } else {
            assert_eq!(request_code_write!(b'z', 10, 1) as u32, 0x4001_7A0A);
            assert_eq!(request_code_write!(b'z', 10, 512) as u32, 0x4200_7A0A);
        }
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_op_write_64() {
        if cfg!(any(
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64"
        )) {
            assert_eq!(
                request_code_write!(b'z', 10, 1u64 << 32) as u32,
                0x8000_7A0A
            );
        } else {
            assert_eq!(
                request_code_write!(b'z', 10, 1u64 << 32) as u32,
                0x4000_7A0A
            );
        }
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    #[test]
    fn test_op_read() {
        if cfg!(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )) {
            assert_eq!(request_code_read!(b'z', 10, 1) as u32, 0x4001_7A0A);
            assert_eq!(request_code_read!(b'z', 10, 512) as u32, 0x4200_7A0A);
        } else {
            assert_eq!(request_code_read!(b'z', 10, 1) as u32, 0x8001_7A0A);
            assert_eq!(request_code_read!(b'z', 10, 512) as u32, 0x8200_7A0A);
        }
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_op_read_64() {
        if cfg!(any(
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "powerpc64"
        )) {
            assert_eq!(
                request_code_read!(b'z', 10, 1u64 << 32) as u32,
                0x4000_7A0A
            );
        } else {
            assert_eq!(
                request_code_read!(b'z', 10, 1u64 << 32) as u32,
                0x8000_7A0A
            );
        }
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    #[test]
    fn test_op_read_write() {
        assert_eq!(request_code_readwrite!(b'z', 10, 1) as u32, 0xC001_7A0A);
        assert_eq!(request_code_readwrite!(b'z', 10, 512) as u32, 0xC200_7A0A);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_op_read_write_64() {
        assert_eq!(
            request_code_readwrite!(b'z', 10, 1u64 << 32) as u32,
            0xC000_7A0A
        );
    }
}

#[cfg(bsd)]
mod bsd {
    #[test]
    fn test_op_none() {
        assert_eq!(request_code_none!(b'q', 10), 0x2000_710A);
        assert_eq!(request_code_none!(b'a', 255), 0x2000_61FF);
    }

    #[cfg(freebsdlike)]
    #[test]
    fn test_op_write_int() {
        assert_eq!(request_code_write_int!(b'v', 4), 0x2004_7604);
        assert_eq!(request_code_write_int!(b'p', 2), 0x2004_7002);
    }

    #[test]
    fn test_op_write() {
        assert_eq!(request_code_write!(b'z', 10, 1), 0x8001_7A0A);
        assert_eq!(request_code_write!(b'z', 10, 512), 0x8200_7A0A);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_op_write_64() {
        assert_eq!(request_code_write!(b'z', 10, 1u64 << 32), 0x8000_7A0A);
    }

    #[test]
    fn test_op_read() {
        assert_eq!(request_code_read!(b'z', 10, 1), 0x4001_7A0A);
        assert_eq!(request_code_read!(b'z', 10, 512), 0x4200_7A0A);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_op_read_64() {
        assert_eq!(request_code_read!(b'z', 10, 1u64 << 32), 0x4000_7A0A);
    }

    #[test]
    fn test_op_read_write() {
        assert_eq!(request_code_readwrite!(b'z', 10, 1), 0xC001_7A0A);
        assert_eq!(request_code_readwrite!(b'z', 10, 512), 0xC200_7A0A);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_op_read_write_64() {
        assert_eq!(request_code_readwrite!(b'z', 10, 1u64 << 32), 0xC000_7A0A);
    }
}

#[cfg(linux_android)]
mod linux_ioctls {
    use std::mem;
    use std::os::unix::io::AsRawFd;

    use libc::{termios, TCGETS, TCSBRK, TCSETS, TIOCNXCL};
    use tempfile::tempfile;

    use nix::errno::Errno;

    ioctl_none_bad!(tiocnxcl, TIOCNXCL);
    #[test]
    fn test_ioctl_none_bad() {
        let file = tempfile().unwrap();
        let res = unsafe { tiocnxcl(file.as_raw_fd()) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }

    ioctl_read_bad!(tcgets, TCGETS, termios);
    #[test]
    fn test_ioctl_read_bad() {
        let file = tempfile().unwrap();
        let mut termios = unsafe { mem::zeroed() };
        let res = unsafe { tcgets(file.as_raw_fd(), &mut termios) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }

    ioctl_write_int_bad!(tcsbrk, TCSBRK);
    #[test]
    fn test_ioctl_write_int_bad() {
        let file = tempfile().unwrap();
        let res = unsafe { tcsbrk(file.as_raw_fd(), 0) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }

    ioctl_write_ptr_bad!(tcsets, TCSETS, termios);
    #[test]
    fn test_ioctl_write_ptr_bad() {
        let file = tempfile().unwrap();
        let termios: termios = unsafe { mem::zeroed() };
        let res = unsafe { tcsets(file.as_raw_fd(), &termios) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }

    // FIXME: Find a suitable example for `ioctl_readwrite_bad`

    // From linux/videodev2.h
    ioctl_none!(log_status, b'V', 70);
    #[test]
    fn test_ioctl_none() {
        let file = tempfile().unwrap();
        let res = unsafe { log_status(file.as_raw_fd()) };
        assert!(res == Err(Errno::ENOTTY) || res == Err(Errno::ENOSYS));
    }

    #[repr(C)]
    pub struct v4l2_audio {
        index: u32,
        name: [u8; 32],
        capability: u32,
        mode: u32,
        reserved: [u32; 2],
    }

    // From linux/videodev2.h
    ioctl_write_ptr!(s_audio, b'V', 34, v4l2_audio);
    #[test]
    fn test_ioctl_write_ptr() {
        let file = tempfile().unwrap();
        let data: v4l2_audio = unsafe { mem::zeroed() };
        let res = unsafe { s_audio(file.as_raw_fd(), &data) };
        assert!(res == Err(Errno::ENOTTY) || res == Err(Errno::ENOSYS));
    }

    // From linux/net/bluetooth/hci_sock.h
    const HCI_IOC_MAGIC: u8 = b'H';
    const HCI_IOC_HCIDEVUP: u8 = 201;
    ioctl_write_int!(hcidevup, HCI_IOC_MAGIC, HCI_IOC_HCIDEVUP);
    #[test]
    fn test_ioctl_write_int() {
        let file = tempfile().unwrap();
        let res = unsafe { hcidevup(file.as_raw_fd(), 0) };
        assert!(res == Err(Errno::ENOTTY) || res == Err(Errno::ENOSYS));
    }

    // From linux/videodev2.h
    ioctl_read!(g_audio, b'V', 33, v4l2_audio);
    #[test]
    fn test_ioctl_read() {
        let file = tempfile().unwrap();
        let mut data: v4l2_audio = unsafe { mem::zeroed() };
        let res = unsafe { g_audio(file.as_raw_fd(), &mut data) };
        assert!(res == Err(Errno::ENOTTY) || res == Err(Errno::ENOSYS));
    }

    // From linux/videodev2.h
    ioctl_readwrite!(enum_audio, b'V', 65, v4l2_audio);
    #[test]
    fn test_ioctl_readwrite() {
        let file = tempfile().unwrap();
        let mut data: v4l2_audio = unsafe { mem::zeroed() };
        let res = unsafe { enum_audio(file.as_raw_fd(), &mut data) };
        assert!(res == Err(Errno::ENOTTY) || res == Err(Errno::ENOSYS));
    }

    // FIXME: Find a suitable example for `ioctl_read_buf`.

    #[repr(C)]
    pub struct spi_ioc_transfer {
        tx_buf: u64,
        rx_buf: u64,
        len: u32,
        speed_hz: u32,
        delay_usecs: u16,
        bits_per_word: u8,
        cs_change: u8,
        tx_nbits: u8,
        rx_nbits: u8,
        pad: u16,
    }

    // From linux/spi/spidev.h
    ioctl_write_buf!(
        spi_ioc_message,
        super::SPI_IOC_MAGIC,
        super::SPI_IOC_MESSAGE,
        spi_ioc_transfer
    );
    #[test]
    fn test_ioctl_write_buf() {
        let file = tempfile().unwrap();
        let data: [spi_ioc_transfer; 4] = unsafe { mem::zeroed() };
        let res = unsafe { spi_ioc_message(file.as_raw_fd(), &data[..]) };
        assert!(res == Err(Errno::ENOTTY) || res == Err(Errno::ENOSYS));
    }

    // FIXME: Find a suitable example for `ioctl_readwrite_buf`.
}

#[cfg(target_os = "freebsd")]
mod freebsd_ioctls {
    use std::mem;
    use std::os::unix::io::AsRawFd;

    use libc::termios;
    use tempfile::tempfile;

    use nix::errno::Errno;

    // From sys/sys/ttycom.h
    const TTY_IOC_MAGIC: u8 = b't';
    const TTY_IOC_TYPE_NXCL: u8 = 14;
    const TTY_IOC_TYPE_GETA: u8 = 19;
    const TTY_IOC_TYPE_SETA: u8 = 20;

    ioctl_none!(tiocnxcl, TTY_IOC_MAGIC, TTY_IOC_TYPE_NXCL);
    #[test]
    fn test_ioctl_none() {
        let file = tempfile().unwrap();
        let res = unsafe { tiocnxcl(file.as_raw_fd()) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }

    ioctl_read!(tiocgeta, TTY_IOC_MAGIC, TTY_IOC_TYPE_GETA, termios);
    #[test]
    fn test_ioctl_read() {
        let file = tempfile().unwrap();
        let mut termios = unsafe { mem::zeroed() };
        let res = unsafe { tiocgeta(file.as_raw_fd(), &mut termios) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }

    ioctl_write_ptr!(tiocseta, TTY_IOC_MAGIC, TTY_IOC_TYPE_SETA, termios);
    #[test]
    fn test_ioctl_write_ptr() {
        let file = tempfile().unwrap();
        let termios: termios = unsafe { mem::zeroed() };
        let res = unsafe { tiocseta(file.as_raw_fd(), &termios) };
        assert_eq!(res, Err(Errno::ENOTTY));
    }
}
