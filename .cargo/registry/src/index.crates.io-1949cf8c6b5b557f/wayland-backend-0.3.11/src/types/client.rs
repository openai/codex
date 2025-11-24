/// An error type representing the failure to load libwayland
#[derive(Debug)]
pub struct NoWaylandLib;

impl std::error::Error for NoWaylandLib {}

impl std::fmt::Display for NoWaylandLib {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        f.write_str("could not load libwayland-client.so")
    }
}

/// An error that can occur when using a Wayland connection
#[derive(Debug)]
pub enum WaylandError {
    /// The connection encountered an IO error
    Io(std::io::Error),
    /// The connection encountered a protocol error
    Protocol(crate::protocol::ProtocolError),
}

impl std::error::Error for WaylandError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self {
            Self::Io(e) => Some(e),
            Self::Protocol(e) => Some(e),
        }
    }
}

impl std::fmt::Display for WaylandError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match self {
            Self::Io(e) => write!(f, "Io error: {e}"),
            Self::Protocol(e) => std::fmt::Display::fmt(e, f),
        }
    }
}

impl Clone for WaylandError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn clone(&self) -> Self {
        match self {
            Self::Protocol(e) => Self::Protocol(e.clone()),
            Self::Io(e) => {
                if let Some(code) = e.raw_os_error() {
                    Self::Io(std::io::Error::from_raw_os_error(code))
                } else {
                    Self::Io(std::io::Error::new(e.kind(), ""))
                }
            }
        }
    }
}

impl From<crate::protocol::ProtocolError> for WaylandError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn from(err: crate::protocol::ProtocolError) -> Self {
        Self::Protocol(err)
    }
}

impl From<std::io::Error> for WaylandError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// An error generated when trying to act on an invalid `ObjectId`.
#[derive(Clone, Debug)]
pub struct InvalidId;

impl std::error::Error for InvalidId {}

impl std::fmt::Display for InvalidId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(f, "Invalid ObjectId")
    }
}
