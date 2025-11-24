use crate::prelude::*;
use crate::pthread_mutex_t;

pub const __SIZEOF_PTHREAD_CONDATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_MUTEX_T: usize = 32;
pub const __SIZEOF_PTHREAD_MUTEXATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_RWLOCK_T: usize = 48;
pub const __SIZEOF_PTHREAD_BARRIERATTR_T: usize = 4;
pub const __SIZEOF_PTHREAD_BARRIER_T: usize = 20;

#[cfg(target_endian = "little")]
pub const PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
};
#[cfg(target_endian = "little")]
pub const PTHREAD_ERRORCHECK_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
};
#[cfg(target_endian = "little")]
pub const PTHREAD_ADAPTIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
};
#[cfg(target_endian = "big")]
pub const PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
};
#[cfg(target_endian = "big")]
pub const PTHREAD_ERRORCHECK_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
};
#[cfg(target_endian = "big")]
pub const PTHREAD_ADAPTIVE_MUTEX_INITIALIZER_NP: crate::pthread_mutex_t = pthread_mutex_t {
    size: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ],
};

pub const SYS_sync_file_range2: c_long = 84;
