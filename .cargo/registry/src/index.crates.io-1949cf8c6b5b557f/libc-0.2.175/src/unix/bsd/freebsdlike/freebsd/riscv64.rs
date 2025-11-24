use crate::prelude::*;

pub type clock_t = i32;
pub type wchar_t = c_int;
pub type time_t = i64;
pub type suseconds_t = c_long;
pub type register_t = i64;

s_no_extra_traits! {
    pub struct gpregs {
        pub gp_ra: crate::register_t,
        pub gp_sp: crate::register_t,
        pub gp_gp: crate::register_t,
        pub gp_tp: crate::register_t,
        pub gp_t: [crate::register_t; 7],
        pub gp_s: [crate::register_t; 12],
        pub gp_a: [crate::register_t; 8],
        pub gp_sepc: crate::register_t,
        pub gp_sstatus: crate::register_t,
    }

    pub struct fpregs {
        pub fp_x: [[u64; 2]; 32],
        pub fp_fcsr: u64,
        pub fp_flags: c_int,
        pub pad: c_int,
    }

    pub struct mcontext_t {
        pub mc_gpregs: gpregs,
        pub mc_fpregs: fpregs,
        pub mc_flags: c_int,
        pub mc_pad: c_int,
        pub mc_spare: [u64; 8],
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for gpregs {
            fn eq(&self, other: &gpregs) -> bool {
                self.gp_ra == other.gp_ra
                    && self.gp_sp == other.gp_sp
                    && self.gp_gp == other.gp_gp
                    && self.gp_tp == other.gp_tp
                    && self.gp_t.iter().zip(other.gp_t.iter()).all(|(a, b)| a == b)
                    && self.gp_s.iter().zip(other.gp_s.iter()).all(|(a, b)| a == b)
                    && self.gp_a.iter().zip(other.gp_a.iter()).all(|(a, b)| a == b)
                    && self.gp_sepc == other.gp_sepc
                    && self.gp_sstatus == other.gp_sstatus
            }
        }
        impl Eq for gpregs {}
        impl hash::Hash for gpregs {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.gp_ra.hash(state);
                self.gp_sp.hash(state);
                self.gp_gp.hash(state);
                self.gp_tp.hash(state);
                self.gp_t.hash(state);
                self.gp_s.hash(state);
                self.gp_a.hash(state);
                self.gp_sepc.hash(state);
                self.gp_sstatus.hash(state);
            }
        }
        impl PartialEq for fpregs {
            fn eq(&self, other: &fpregs) -> bool {
                self.fp_x == other.fp_x
                    && self.fp_fcsr == other.fp_fcsr
                    && self.fp_flags == other.fp_flags
                    && self.pad == other.pad
            }
        }
        impl Eq for fpregs {}
        impl hash::Hash for fpregs {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.fp_x.hash(state);
                self.fp_fcsr.hash(state);
                self.fp_flags.hash(state);
                self.pad.hash(state);
            }
        }
        impl PartialEq for mcontext_t {
            fn eq(&self, other: &mcontext_t) -> bool {
                self.mc_gpregs == other.mc_gpregs
                    && self.mc_fpregs == other.mc_fpregs
                    && self.mc_flags == other.mc_flags
                    && self.mc_pad == other.mc_pad
                    && self
                        .mc_spare
                        .iter()
                        .zip(other.mc_spare.iter())
                        .all(|(a, b)| a == b)
            }
        }
        impl Eq for mcontext_t {}
        impl hash::Hash for mcontext_t {
            fn hash<H: hash::Hasher>(&self, state: &mut H) {
                self.mc_gpregs.hash(state);
                self.mc_fpregs.hash(state);
                self.mc_flags.hash(state);
                self.mc_pad.hash(state);
                self.mc_spare.hash(state);
            }
        }
    }
}

pub(crate) const _ALIGNBYTES: usize = size_of::<c_longlong>() - 1;

pub const BIOCSRTIMEOUT: c_ulong = 0x8010426d;
pub const BIOCGRTIMEOUT: c_ulong = 0x4010426e;
pub const MAP_32BIT: c_int = 0x00080000;
pub const MINSIGSTKSZ: size_t = 4096; // 1024 * 4
pub const TIOCTIMESTAMP: c_ulong = 0x40107459;
