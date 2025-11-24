use crate::prelude::*;

pub type boolean_t = c_int;
pub type mcontext_t = *mut __darwin_mcontext64;

s! {
    pub struct malloc_zone_t {
        __private: [crate::uintptr_t; 18], // FIXME(macos): needs arm64 auth pointers support
    }

    pub struct ucontext_t {
        pub uc_onstack: c_int,
        pub uc_sigmask: crate::sigset_t,
        pub uc_stack: crate::stack_t,
        pub uc_link: *mut crate::ucontext_t,
        pub uc_mcsize: usize,
        pub uc_mcontext: mcontext_t,
    }

    pub struct __darwin_mcontext64 {
        pub __es: __darwin_arm_exception_state64,
        pub __ss: __darwin_arm_thread_state64,
        pub __ns: __darwin_arm_neon_state64,
    }

    pub struct __darwin_arm_exception_state64 {
        pub __far: u64,
        pub __esr: u32,
        pub __exception: u32,
    }

    pub struct __darwin_arm_thread_state64 {
        pub __x: [u64; 29],
        pub __fp: u64,
        pub __lr: u64,
        pub __sp: u64,
        pub __pc: u64,
        pub __cpsr: u32,
        pub __pad: u32,
    }

    pub struct __darwin_arm_neon_state64 {
        pub __v: [crate::__uint128_t; 32],
        pub __fpsr: u32,
        pub __fpcr: u32,
    }
}

s_no_extra_traits! {
    pub struct max_align_t {
        priv_: f64,
    }
}
