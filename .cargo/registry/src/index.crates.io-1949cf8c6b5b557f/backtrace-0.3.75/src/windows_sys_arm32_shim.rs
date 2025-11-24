pub const ARM_MAX_BREAKPOINTS: usize = 8;
pub const ARM_MAX_WATCHPOINTS: usize = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NEON128 {
    pub Low: u64,
    pub High: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union CONTEXT_FloatRegs {
    pub Q: [NEON128; 16],
    pub D: [u64; 32],
    pub S: [u32; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CONTEXT {
    pub ContextFlags: u32,
    pub R0: u32,
    pub R1: u32,
    pub R2: u32,
    pub R3: u32,
    pub R4: u32,
    pub R5: u32,
    pub R6: u32,
    pub R7: u32,
    pub R8: u32,
    pub R9: u32,
    pub R10: u32,
    pub R11: u32,
    pub R12: u32,
    // Control registers
    pub Sp: u32,
    pub Lr: u32,
    pub Pc: u32,
    pub Cpsr: u32,
    // Floating-point registers
    pub Fpsrc: u32,
    pub Padding: u32,
    pub u: CONTEXT_FloatRegs,
    // Debug registers
    pub Bvr: [u32; ARM_MAX_BREAKPOINTS],
    pub Bcr: [u32; ARM_MAX_BREAKPOINTS],
    pub Wvr: [u32; ARM_MAX_WATCHPOINTS],
    pub Wcr: [u32; ARM_MAX_WATCHPOINTS],
    pub Padding2: [u32; 2],
}

pub const IMAGE_FILE_MACHINE_ARMNT: IMAGE_FILE_MACHINE = 0x01c4;
