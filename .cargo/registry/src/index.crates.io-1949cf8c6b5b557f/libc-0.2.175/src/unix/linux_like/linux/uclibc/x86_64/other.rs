use crate::prelude::*;

// Thestyle checker discourages the use of #[cfg], so this has to go into a
// separate module
pub type pthread_t = c_ulong;

pub const PTHREAD_STACK_MIN: usize = 16384;
