use crate::prelude::*;

/// L4Re specifics
/// This module contains definitions required by various L4Re libc backends.
/// Some of them are formally not part of the libc, but are a dependency of the
/// libc and hence we should provide them here.

pub type l4_umword_t = c_ulong; // Unsigned machine word.
pub type pthread_t = *mut c_void;

s! {
    /// CPU sets.
    pub struct l4_sched_cpu_set_t {
        // from the L4Re docs
        /// Combination of granularity and offset.
        ///
        /// The granularity defines how many CPUs each bit in map describes.
        /// The offset is the number of the first CPU described by the first
        /// bit in the bitmap.
        /// offset must be a multiple of 2^graularity.
        ///
        /// | MSB              |                 LSB |
        /// | ---------------- | ------------------- |
        /// | 8bit granularity | 24bit offset ..     |
        gran_offset: l4_umword_t,
        /// Bitmap of CPUs.
        map: l4_umword_t,
    }

    pub struct pthread_attr_t {
        pub __detachstate: c_int,
        pub __schedpolicy: c_int,
        pub __schedparam: super::__sched_param,
        pub __inheritsched: c_int,
        pub __scope: c_int,
        pub __guardsize: size_t,
        pub __stackaddr_set: c_int,
        pub __stackaddr: *mut c_void, // better don't use it
        pub __stacksize: size_t,
        // L4Re specifics
        pub affinity: l4_sched_cpu_set_t,
        pub create_flags: c_uint,
    }
}

// L4Re requires a min stack size of 64k; that isn't defined in uClibc, but
// somewhere in the core libraries. uClibc wants 16k, but that's not enough.
pub const PTHREAD_STACK_MIN: usize = 65536;

// Misc other constants required for building.
pub const SIGIO: c_int = 29;
pub const B19200: crate::speed_t = 0o000016;
pub const B38400: crate::speed_t = 0o000017;
