//! 32-bit specific Apple (ios/darwin) definitions

use crate::prelude::*;

pub type boolean_t = c_int;

s! {
    pub struct if_data {
        pub ifi_type: c_uchar,
        pub ifi_typelen: c_uchar,
        pub ifi_physical: c_uchar,
        pub ifi_addrlen: c_uchar,
        pub ifi_hdrlen: c_uchar,
        pub ifi_recvquota: c_uchar,
        pub ifi_xmitquota: c_uchar,
        pub ifi_unused1: c_uchar,
        pub ifi_mtu: u32,
        pub ifi_metric: u32,
        pub ifi_baudrate: u32,
        pub ifi_ipackets: u32,
        pub ifi_ierrors: u32,
        pub ifi_opackets: u32,
        pub ifi_oerrors: u32,
        pub ifi_collisions: u32,
        pub ifi_ibytes: u32,
        pub ifi_obytes: u32,
        pub ifi_imcasts: u32,
        pub ifi_omcasts: u32,
        pub ifi_iqdrops: u32,
        pub ifi_noproto: u32,
        pub ifi_recvtiming: u32,
        pub ifi_xmittiming: u32,
        pub ifi_lastchange: crate::timeval,
        pub ifi_unused2: u32,
        pub ifi_hwassist: u32,
        pub ifi_reserved1: u32,
        pub ifi_reserved2: u32,
    }

    pub struct bpf_hdr {
        pub bh_tstamp: crate::timeval,
        pub bh_caplen: u32,
        pub bh_datalen: u32,
        pub bh_hdrlen: c_ushort,
    }

    pub struct malloc_zone_t {
        __private: [crate::uintptr_t; 18], // FIXME(macos): keeping private for now
    }
}

s_no_extra_traits! {
    pub struct pthread_attr_t {
        __sig: c_long,
        __opaque: [c_char; 36],
    }

    pub struct pthread_once_t {
        __sig: c_long,
        __opaque: [c_char; crate::__PTHREAD_ONCE_SIZE__],
    }

    #[repr(align(16))]
    pub struct max_align_t {
        priv_: [f64; 2],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for pthread_attr_t {
            fn eq(&self, other: &pthread_attr_t) -> bool {
                self.__sig == other.__sig
                    && self
                        .__opaque
                        .iter()
                        .zip(other.__opaque.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for pthread_attr_t {}
        impl hash::Hash for pthread_attr_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.__sig.hash(state);
                self.__opaque.hash(state);
            }
        }
        impl PartialEq for pthread_once_t {
            fn eq(&self, other: &pthread_once_t) -> bool {
                self.__sig == other.__sig
                    && self
                        .__opaque
                        .iter()
                        .zip(other.__opaque.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for pthread_once_t {}
        impl hash::Hash for pthread_once_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.__sig.hash(state);
                self.__opaque.hash(state);
            }
        }
    }
}

#[doc(hidden)]
#[deprecated(since = "0.2.55")]
pub const NET_RT_MAXID: c_int = 10;

pub const __PTHREAD_MUTEX_SIZE__: usize = 40;
pub const __PTHREAD_COND_SIZE__: usize = 24;
pub const __PTHREAD_CONDATTR_SIZE__: usize = 4;
pub const __PTHREAD_ONCE_SIZE__: usize = 4;
pub const __PTHREAD_RWLOCK_SIZE__: usize = 124;
pub const __PTHREAD_RWLOCKATTR_SIZE__: usize = 12;

pub const TIOCTIMESTAMP: c_ulong = 0x40087459;
pub const TIOCDCDTIMESTAMP: c_ulong = 0x40087458;

pub const BIOCSETF: c_ulong = 0x80084267;
pub const BIOCSRTIMEOUT: c_ulong = 0x8008426d;
pub const BIOCGRTIMEOUT: c_ulong = 0x4008426e;
pub const BIOCSETFNR: c_ulong = 0x8008427e;

const _PTHREAD_ONCE_SIG_INIT: c_long = 0x30B1BCBA;
pub const PTHREAD_ONCE_INIT: crate::pthread_once_t = crate::pthread_once_t {
    __sig: _PTHREAD_ONCE_SIG_INIT,
    __opaque: [0; 4],
};

extern "C" {
    pub fn exchangedata(path1: *const c_char, path2: *const c_char, options: c_ulong) -> c_int;
}
