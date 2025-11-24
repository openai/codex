use std::time::Duration;

use crate::Runtime;

/// Pool configuration.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PoolConfig {
    /// Maximum size of the pool.
    pub max_size: usize,

    /// Timeout for [`Pool::get()`] operation.
    ///
    /// [`Pool::get()`]: super::Pool::get
    pub timeout: Option<Duration>,

    /// [`Runtime`] to be used.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub runtime: Option<Runtime>,
}

impl PoolConfig {
    /// Create a new [`PoolConfig`] without any timeouts.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            timeout: None,
            runtime: None,
        }
    }
}

impl Default for PoolConfig {
    /// Create a [`PoolConfig`] where [`PoolConfig::max_size`] is set to
    /// `cpu_core_count * 2` including logical cores (Hyper-Threading).
    fn default() -> Self {
        Self::new(crate::util::get_default_pool_max_size())
    }
}
