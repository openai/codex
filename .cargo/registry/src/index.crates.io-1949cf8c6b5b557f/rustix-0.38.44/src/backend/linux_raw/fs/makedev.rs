use crate::fs::Dev;

#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    ((u64::from(maj) & 0xffff_f000_u64) << 32)
        | ((u64::from(maj) & 0x0000_0fff_u64) << 8)
        | ((u64::from(min) & 0xffff_ff00_u64) << 12)
        | (u64::from(min) & 0x0000_00ff_u64)
}

#[inline]
pub(crate) fn major(dev: Dev) -> u32 {
    (((dev >> 31 >> 1) & 0xffff_f000) | ((dev >> 8) & 0x0000_0fff)) as u32
}

#[inline]
pub(crate) fn minor(dev: Dev) -> u32 {
    (((dev >> 12) & 0xffff_ff00) | (dev & 0x0000_00ff)) as u32
}
