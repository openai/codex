use std::time::Duration;

/// PollTimeout argument for polling.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct PollTimeout(i32);

impl PollTimeout {
    /// Blocks indefinitely.
    ///
    /// > Specifying a negative value in timeout means an infinite timeout.
    pub const NONE: Self = Self(-1);
    /// Returns immediately.
    ///
    /// > Specifying a timeout of zero causes poll() to return immediately, even if no file
    /// > descriptors are ready.
    pub const ZERO: Self = Self(0);
    /// Blocks for at most [`i32::MAX`] milliseconds.
    pub const MAX: Self = Self(i32::MAX);
    /// Returns if `self` equals [`PollTimeout::NONE`].
    pub fn is_none(&self) -> bool {
        // > Specifying a negative value in timeout means an infinite timeout.
        *self <= Self::NONE
    }
    /// Returns if `self` does not equal [`PollTimeout::NONE`].
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
    /// Returns the timeout in milliseconds if there is some, otherwise returns `None`.
    pub fn as_millis(&self) -> Option<u32> {
        self.is_some().then_some(u32::try_from(self.0).unwrap())
    }
    /// Returns the timeout as a `Duration` if there is some, otherwise returns `None`.
    pub fn duration(&self) -> Option<Duration> {
        self.as_millis()
            .map(|x| Duration::from_millis(u64::from(x)))
    }
}

/// Error type for integer conversions into `PollTimeout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollTimeoutTryFromError {
    /// Passing a value less than -1 is invalid on some systems, see
    /// <https://man.freebsd.org/cgi/man.cgi?poll#end>.
    TooNegative,
    /// Passing a value greater than `i32::MAX` is invalid.
    TooPositive,
}

impl std::fmt::Display for PollTimeoutTryFromError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooNegative => write!(f, "Passed a negative timeout less than -1."),
            Self::TooPositive => write!(f, "Passed a positive timeout greater than `i32::MAX` milliseconds.")
        }
    }
}

impl std::error::Error for PollTimeoutTryFromError {}

impl<T: Into<PollTimeout>> From<Option<T>> for PollTimeout {
    fn from(x: Option<T>) -> Self {
        x.map_or(Self::NONE, |x| x.into())
    }
}
impl TryFrom<Duration> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: Duration) -> std::result::Result<Self, Self::Error> {
        Ok(Self(
            i32::try_from(x.as_millis())
                .map_err(|_| PollTimeoutTryFromError::TooPositive)?,
        ))
    }
}
impl TryFrom<u128> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: u128) -> std::result::Result<Self, Self::Error> {
        Ok(Self(
            i32::try_from(x)
                .map_err(|_| PollTimeoutTryFromError::TooPositive)?,
        ))
    }
}
impl TryFrom<u64> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: u64) -> std::result::Result<Self, Self::Error> {
        Ok(Self(
            i32::try_from(x)
                .map_err(|_| PollTimeoutTryFromError::TooPositive)?,
        ))
    }
}
impl TryFrom<u32> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: u32) -> std::result::Result<Self, Self::Error> {
        Ok(Self(
            i32::try_from(x)
                .map_err(|_| PollTimeoutTryFromError::TooPositive)?,
        ))
    }
}
impl From<u16> for PollTimeout {
    fn from(x: u16) -> Self {
        Self(i32::from(x))
    }
}
impl From<u8> for PollTimeout {
    fn from(x: u8) -> Self {
        Self(i32::from(x))
    }
}
impl TryFrom<i128> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: i128) -> std::result::Result<Self, Self::Error> {
        match x {
            ..=-2 => Err(PollTimeoutTryFromError::TooNegative),
            -1.. => Ok(Self(
                i32::try_from(x)
                    .map_err(|_| PollTimeoutTryFromError::TooPositive)?,
            )),
        }
    }
}
impl TryFrom<i64> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: i64) -> std::result::Result<Self, Self::Error> {
        match x {
            ..=-2 => Err(PollTimeoutTryFromError::TooNegative),
            -1.. => Ok(Self(
                i32::try_from(x)
                    .map_err(|_| PollTimeoutTryFromError::TooPositive)?,
            )),
        }
    }
}
impl TryFrom<i32> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: i32) -> std::result::Result<Self, Self::Error> {
        match x {
            ..=-2 => Err(PollTimeoutTryFromError::TooNegative),
            -1.. => Ok(Self(x)),
        }
    }
}
impl TryFrom<i16> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: i16) -> std::result::Result<Self, Self::Error> {
        match x {
            ..=-2 => Err(PollTimeoutTryFromError::TooNegative),
            -1.. => Ok(Self(i32::from(x))),
        }
    }
}
impl TryFrom<i8> for PollTimeout {
    type Error = PollTimeoutTryFromError;
    fn try_from(x: i8) -> std::result::Result<Self, Self::Error> {
        match x {
            ..=-2 => Err(PollTimeoutTryFromError::TooNegative),
            -1.. => Ok(Self(i32::from(x))),
        }
    }
}
impl TryFrom<PollTimeout> for Duration {
    type Error = ();
    fn try_from(x: PollTimeout) -> std::result::Result<Self, ()> {
        x.duration().ok_or(())
    }
}
impl TryFrom<PollTimeout> for u128 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
impl TryFrom<PollTimeout> for u64 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
impl TryFrom<PollTimeout> for u32 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
impl TryFrom<PollTimeout> for u16 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
impl TryFrom<PollTimeout> for u8 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
impl From<PollTimeout> for i128 {
    fn from(x: PollTimeout) -> Self {
        Self::from(x.0)
    }
}
impl From<PollTimeout> for i64 {
    fn from(x: PollTimeout) -> Self {
        Self::from(x.0)
    }
}
impl From<PollTimeout> for i32 {
    fn from(x: PollTimeout) -> Self {
        x.0
    }
}
impl TryFrom<PollTimeout> for i16 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
impl TryFrom<PollTimeout> for i8 {
    type Error = <Self as TryFrom<i32>>::Error;
    fn try_from(x: PollTimeout) -> std::result::Result<Self, Self::Error> {
        Self::try_from(x.0)
    }
}
