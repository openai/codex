use crate::prelude::*;

pub const PROC_KPTI_CTL: c_int = crate::PROC_PROCCTL_MD_MIN;
pub const PROC_KPTI_CTL_ENABLE_ON_EXEC: c_int = 1;
pub const PROC_KPTI_CTL_DISABLE_ON_EXEC: c_int = 2;
pub const PROC_KPTI_STATUS: c_int = crate::PROC_PROCCTL_MD_MIN + 1;
pub const PROC_KPTI_STATUS_ACTIVE: c_int = 0x80000000;
