use crate::prelude::*;

pub type clock_t = u32;
pub type wchar_t = i32;
pub type time_t = i64;
pub type suseconds_t = i32;
pub type register_t = i32;

s_no_extra_traits! {
    #[repr(align(16))]
    pub struct mcontext_t {
        pub mc_vers: c_int,
        pub mc_flags: c_int,
        pub mc_onstack: c_int,
        pub mc_len: c_int,
        pub mc_avec: [u64; 64],
        pub mc_av: [u32; 2],
        pub mc_frame: [crate::register_t; 42],
        pub mc_fpreg: [u64; 33],
        pub mc_vsxfpreg: [u64; 32],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for mcontext_t {
            fn eq(&self, other: &mcontext_t) -> bool {
                self.mc_vers == other.mc_vers
                    && self.mc_flags == other.mc_flags
                    && self.mc_onstack == other.mc_onstack
                    && self.mc_len == other.mc_len
                    && self.mc_avec == other.mc_avec
                    && self.mc_av == other.mc_av
                    && self.mc_frame == other.mc_frame
                    && self.mc_fpreg == other.mc_fpreg
                    && self.mc_vsxfpreg == other.mc_vsxfpreg
            }
        }
        impl Eq for mcontext_t {}
        impl hash::Hash for mcontext_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.mc_vers.hash(state);
                self.mc_flags.hash(state);
                self.mc_onstack.hash(state);
                self.mc_len.hash(state);
                self.mc_avec.hash(state);
                self.mc_av.hash(state);
                self.mc_frame.hash(state);
                self.mc_fpreg.hash(state);
                self.mc_vsxfpreg.hash(state);
            }
        }
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_int>() - 1;

pub const BIOCSRTIMEOUT: c_ulong = 0x8010426d;
pub const BIOCGRTIMEOUT: c_ulong = 0x4010426e;
pub const MAP_32BIT: c_int = 0x00080000;
pub const MINSIGSTKSZ: size_t = 2048; // 512 * 4
pub const TIOCTIMESTAMP: c_ulong = 0x40107459;
