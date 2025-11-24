use std::{fmt, time::Duration};

use super::BuildError;

/// [`Pool`] configuration.
///
/// [`Pool`]: super::Pool
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PoolConfig {
    /// Maximum size of the [`Pool`].
    ///
    /// Default: `cpu_count * 4`
    ///
    /// [`Pool`]: super::Pool
    pub max_size: usize,

    /// Timeouts of the [`Pool`].
    ///
    /// Default: No timeouts
    ///
    /// [`Pool`]: super::Pool
    #[cfg_attr(feature = "serde", serde(default))]
    pub timeouts: Timeouts,

    /// Queue mode of the [`Pool`].
    ///
    /// Determines the order of objects being queued and dequeued.
    ///
    /// Default: `Fifo`
    ///
    /// [`Pool`]: super::Pool
    #[cfg_attr(feature = "serde", serde(default))]
    pub queue_mode: QueueMode,
}

impl PoolConfig {
    /// Creates a new [`PoolConfig`] without any timeouts and with the provided
    /// `max_size`.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            timeouts: Timeouts::default(),
            queue_mode: QueueMode::default(),
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

/// Timeouts when getting [`Object`]s from a [`Pool`].
///
/// [`Object`]: super::Object
/// [`Pool`]: super::Pool
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Timeouts {
    /// Timeout when waiting for a slot to become available.
    pub wait: Option<Duration>,

    /// Timeout when creating a new object.
    pub create: Option<Duration>,

    /// Timeout when recycling an object.
    pub recycle: Option<Duration>,
}

impl Timeouts {
    /// Create an empty [`Timeouts`] config (no timeouts set).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            create: None,
            wait: None,
            recycle: None,
        }
    }

    /// Creates a new [`Timeouts`] config with only the `wait` timeout being
    /// set.
    #[must_use]
    pub const fn wait_millis(wait: u64) -> Self {
        Self {
            create: None,
            wait: Some(Duration::from_millis(wait)),
            recycle: None,
        }
    }
}

// Implemented manually to provide a custom documentation.
impl Default for Timeouts {
    /// Creates an empty [`Timeouts`] config (no timeouts set).
    fn default() -> Self {
        Self::new()
    }
}

/// Mode for dequeuing [`Object`]s from a [`Pool`].
///
/// [`Object`]: super::Object
/// [`Pool`]: super::Pool
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum QueueMode {
    /// Dequeue the object that was least recently added (first in first out).
    #[default]
    Fifo,
    /// Dequeue the object that was most recently added (last in first out).
    Lifo,
}

/// This error is used when building pools via the config `create_pool`
/// methods.
#[derive(Debug)]
pub enum CreatePoolError<C> {
    /// This variant is used for configuration errors
    Config(C),
    /// This variant is used for errors while building the pool
    Build(BuildError),
}

impl<C> fmt::Display for CreatePoolError<C>
where
    C: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(e) => write!(f, "Config: {}", e),
            Self::Build(e) => write!(f, "Build: {}", e),
        }
    }
}

impl<C> std::error::Error for CreatePoolError<C> where C: std::error::Error {}
