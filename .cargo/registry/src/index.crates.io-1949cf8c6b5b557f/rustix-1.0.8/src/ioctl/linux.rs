//! `ioctl` opcode behavior for Linux platforms.

use super::{Direction, Opcode};
use consts::*;

/// Compose an opcode from its component parts.
pub(super) const fn compose_opcode(
    dir: Direction,
    group: Opcode,
    num: Opcode,
    size: Opcode,
) -> Opcode {
    macro_rules! mask_and_shift {
        ($val:expr, $shift:expr, $mask:expr) => {{
            ($val & $mask) << $shift
        }};
    }

    let dir = match dir {
        Direction::None => NONE,
        Direction::Read => READ,
        Direction::Write => WRITE,
        Direction::ReadWrite => READ | WRITE,
    };

    mask_and_shift!(group, GROUP_SHIFT, GROUP_MASK)
        | mask_and_shift!(num, NUM_SHIFT, NUM_MASK)
        | mask_and_shift!(size, SIZE_SHIFT, SIZE_MASK)
        | mask_and_shift!(dir, DIR_SHIFT, DIR_MASK)
}

const NUM_BITS: Opcode = 8;
const GROUP_BITS: Opcode = 8;

const NUM_SHIFT: Opcode = 0;
const GROUP_SHIFT: Opcode = NUM_SHIFT + NUM_BITS;
const SIZE_SHIFT: Opcode = GROUP_SHIFT + GROUP_BITS;
const DIR_SHIFT: Opcode = SIZE_SHIFT + SIZE_BITS;

const NUM_MASK: Opcode = (1 << NUM_BITS) - 1;
const GROUP_MASK: Opcode = (1 << GROUP_BITS) - 1;
const SIZE_MASK: Opcode = (1 << SIZE_BITS) - 1;
const DIR_MASK: Opcode = (1 << DIR_BITS) - 1;

#[cfg(any(
    target_arch = "x86",
    target_arch = "arm",
    target_arch = "s390x",
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "loongarch64",
    target_arch = "csky"
))]
mod consts {
    use super::Opcode;

    pub(super) const NONE: Opcode = 0;
    pub(super) const READ: Opcode = 2;
    pub(super) const WRITE: Opcode = 1;
    pub(super) const SIZE_BITS: Opcode = 14;
    pub(super) const DIR_BITS: Opcode = 2;
}

#[cfg(any(
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "sparc",
    target_arch = "sparc64"
))]
mod consts {
    use super::Opcode;

    pub(super) const NONE: Opcode = 1;
    pub(super) const READ: Opcode = 2;
    pub(super) const WRITE: Opcode = 4;
    pub(super) const SIZE_BITS: Opcode = 13;
    pub(super) const DIR_BITS: Opcode = 3;
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(not(any(
    // These have no ioctl opcodes defined in linux_raw_sys so we can't use
    // that as a known-good value for this test.
    target_arch = "sparc",
    target_arch = "sparc64"
)))]
    #[test]
    fn check_known_opcodes() {
        use crate::backend::c::{c_long, c_uint};
        use core::mem::size_of;

        // _IOR('U', 15, unsigned int)
        assert_eq!(
            compose_opcode(
                Direction::Read,
                b'U' as Opcode,
                15,
                size_of::<c_uint>() as Opcode
            ),
            linux_raw_sys::ioctl::USBDEVFS_CLAIMINTERFACE as Opcode
        );

        // _IOW('v', 2, long)
        assert_eq!(
            compose_opcode(
                Direction::Write,
                b'v' as Opcode,
                2,
                size_of::<c_long>() as Opcode
            ),
            linux_raw_sys::ioctl::FS_IOC_SETVERSION as Opcode
        );
    }
}
