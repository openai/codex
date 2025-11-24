use crate::prelude::*;

pub type wchar_t = u32;
pub type time_t = i64;

s! {
    pub struct aarch64_qreg_t {
        pub qlo: u64,
        pub qhi: u64,
    }

    pub struct aarch64_fpu_registers {
        pub reg: [crate::aarch64_qreg_t; 32],
        pub fpsr: u32,
        pub fpcr: u32,
    }

    pub struct aarch64_cpu_registers {
        pub gpr: [u64; 32],
        pub elr: u64,
        pub pstate: u64,
    }

    #[repr(align(16))]
    pub struct mcontext_t {
        pub cpu: crate::aarch64_cpu_registers,
        pub fpu: crate::aarch64_fpu_registers,
    }

    pub struct stack_t {
        pub ss_sp: *mut c_void,
        pub ss_size: size_t,
        pub ss_flags: c_int,
    }
}
