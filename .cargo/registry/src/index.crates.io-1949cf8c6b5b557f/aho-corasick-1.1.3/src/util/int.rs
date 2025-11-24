/*!
This module provides several integer oriented traits for converting between
both fixed size integers and integers whose size varies based on the target
(like `usize`).

The main design principle for this module is to centralize all uses of `as`.
The thinking here is that `as` makes it very easy to perform accidental lossy
conversions, and if we centralize all its uses here under more descriptive
higher level operations, its use and correctness becomes easier to audit.

This was copied mostly wholesale from `regex-automata`.

NOTE: for simplicity, we don't take target pointer width into account here for
`usize` conversions. Since we currently only panic in debug mode, skipping the
check when it can be proven it isn't needed at compile time doesn't really
matter. Now, if we wind up wanting to do as many checks as possible in release
mode, then we would want to skip those when we know the conversions are always
non-lossy.
*/

// We define a little more than what we need, but I'd rather just have
// everything via a consistent and uniform API then have holes.
#![allow(dead_code)]

pub(crate) trait U8 {
    fn as_usize(self) -> usize;
}

impl U8 for u8 {
    fn as_usize(self) -> usize {
        usize::from(self)
    }
}

pub(crate) trait U16 {
    fn as_usize(self) -> usize;
    fn low_u8(self) -> u8;
    fn high_u8(self) -> u8;
}

impl U16 for u16 {
    fn as_usize(self) -> usize {
        usize::from(self)
    }

    fn low_u8(self) -> u8 {
        self as u8
    }

    fn high_u8(self) -> u8 {
        (self >> 8) as u8
    }
}

pub(crate) trait U32 {
    fn as_usize(self) -> usize;
    fn low_u8(self) -> u8;
    fn low_u16(self) -> u16;
    fn high_u16(self) -> u16;
}

impl U32 for u32 {
    #[inline]
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("u32 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }

    fn low_u8(self) -> u8 {
        self as u8
    }

    fn low_u16(self) -> u16 {
        self as u16
    }

    fn high_u16(self) -> u16 {
        (self >> 16) as u16
    }
}

pub(crate) trait U64 {
    fn as_usize(self) -> usize;
    fn low_u8(self) -> u8;
    fn low_u16(self) -> u16;
    fn low_u32(self) -> u32;
    fn high_u32(self) -> u32;
}

impl U64 for u64 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("u64 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }

    fn low_u8(self) -> u8 {
        self as u8
    }

    fn low_u16(self) -> u16 {
        self as u16
    }

    fn low_u32(self) -> u32 {
        self as u32
    }

    fn high_u32(self) -> u32 {
        (self >> 32) as u32
    }
}

pub(crate) trait I8 {
    fn as_usize(self) -> usize;
    fn to_bits(self) -> u8;
    fn from_bits(n: u8) -> i8;
}

impl I8 for i8 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("i8 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }

    fn to_bits(self) -> u8 {
        self as u8
    }

    fn from_bits(n: u8) -> i8 {
        n as i8
    }
}

pub(crate) trait I32 {
    fn as_usize(self) -> usize;
    fn to_bits(self) -> u32;
    fn from_bits(n: u32) -> i32;
}

impl I32 for i32 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("i32 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }

    fn to_bits(self) -> u32 {
        self as u32
    }

    fn from_bits(n: u32) -> i32 {
        n as i32
    }
}

pub(crate) trait I64 {
    fn as_usize(self) -> usize;
    fn to_bits(self) -> u64;
    fn from_bits(n: u64) -> i64;
}

impl I64 for i64 {
    fn as_usize(self) -> usize {
        #[cfg(debug_assertions)]
        {
            usize::try_from(self).expect("i64 overflowed usize")
        }
        #[cfg(not(debug_assertions))]
        {
            self as usize
        }
    }

    fn to_bits(self) -> u64 {
        self as u64
    }

    fn from_bits(n: u64) -> i64 {
        n as i64
    }
}

pub(crate) trait Usize {
    fn as_u8(self) -> u8;
    fn as_u16(self) -> u16;
    fn as_u32(self) -> u32;
    fn as_u64(self) -> u64;
}

impl Usize for usize {
    fn as_u8(self) -> u8 {
        #[cfg(debug_assertions)]
        {
            u8::try_from(self).expect("usize overflowed u8")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u8
        }
    }

    fn as_u16(self) -> u16 {
        #[cfg(debug_assertions)]
        {
            u16::try_from(self).expect("usize overflowed u16")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u16
        }
    }

    fn as_u32(self) -> u32 {
        #[cfg(debug_assertions)]
        {
            u32::try_from(self).expect("usize overflowed u32")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u32
        }
    }

    fn as_u64(self) -> u64 {
        #[cfg(debug_assertions)]
        {
            u64::try_from(self).expect("usize overflowed u64")
        }
        #[cfg(not(debug_assertions))]
        {
            self as u64
        }
    }
}

// Pointers aren't integers, but we convert pointers to integers to perform
// offset arithmetic in some places. (And no, we don't convert the integers
// back to pointers.) So add 'as_usize' conversions here too for completeness.
//
// These 'as' casts are actually okay because they're always non-lossy. But the
// idea here is to just try and remove as much 'as' as possible, particularly
// in this crate where we are being really paranoid about offsets and making
// sure we don't panic on inputs that might be untrusted. This way, the 'as'
// casts become easier to audit if they're all in one place, even when some of
// them are actually okay 100% of the time.

pub(crate) trait Pointer {
    fn as_usize(self) -> usize;
}

impl<T> Pointer for *const T {
    fn as_usize(self) -> usize {
        self as usize
    }
}
