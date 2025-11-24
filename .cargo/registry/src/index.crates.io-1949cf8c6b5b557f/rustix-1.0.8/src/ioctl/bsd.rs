//! `ioctl` opcode behavior for BSD platforms.

use super::{Direction, Opcode};

pub(super) const fn compose_opcode(
    dir: Direction,
    group: Opcode,
    num: Opcode,
    size: Opcode,
) -> Opcode {
    let dir = match dir {
        Direction::None => NONE,
        Direction::Read => READ,
        Direction::Write => WRITE,
        Direction::ReadWrite => READ | WRITE,
    };

    dir | num | (group << 8) | ((size & IOCPARAM_MASK) << 16)
}

// `IOC_VOID`
pub const NONE: Opcode = 0x2000_0000;
// `IOC_OUT` (“out” is from the perspective of the kernel)
pub const READ: Opcode = 0x4000_0000;
// `IOC_IN` (“in” is from the perspective of the kernel)
pub const WRITE: Opcode = 0x8000_0000;
pub const IOCPARAM_MASK: Opcode = 0x1FFF;
