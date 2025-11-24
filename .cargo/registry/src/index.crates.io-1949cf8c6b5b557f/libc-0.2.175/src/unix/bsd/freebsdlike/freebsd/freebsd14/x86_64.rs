use crate::prelude::*;

pub const PROC_KPTI_CTL: c_int = crate::PROC_PROCCTL_MD_MIN;
pub const PROC_KPTI_CTL_ENABLE_ON_EXEC: c_int = 1;
pub const PROC_KPTI_CTL_DISABLE_ON_EXEC: c_int = 2;
pub const PROC_KPTI_STATUS: c_int = crate::PROC_PROCCTL_MD_MIN + 1;
pub const PROC_KPTI_STATUS_ACTIVE: c_int = 0x80000000;
pub const PROC_LA_CTL: c_int = crate::PROC_PROCCTL_MD_MIN + 2;
pub const PROC_LA_STATUS: c_int = crate::PROC_PROCCTL_MD_MIN + 3;
pub const PROC_LA_CTL_LA48_ON_EXEC: c_int = 1;
pub const PROC_LA_CTL_LA57_ON_EXEC: c_int = 2;
pub const PROC_LA_CTL_DEFAULT_ON_EXEC: c_int = 3;
pub const PROC_LA_STATUS_LA48: c_int = 0x01000000;
pub const PROC_LA_STATUS_LA57: c_int = 0x02000000;
