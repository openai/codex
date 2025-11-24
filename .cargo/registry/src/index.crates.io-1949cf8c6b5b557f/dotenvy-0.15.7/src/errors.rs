use std::env;
use std::error;
use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    LineParse(String, usize),
    Io(io::Error),
    EnvVar(env::VarError),
}

impl Error {
    pub fn not_found(&self) -> bool {
        if let Error::Io(ref io_error) = *self {
            return io_error.kind() == io::ErrorKind::NotFound;
        }
        false
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::EnvVar(err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(err) => write!(fmt, "{}", err),
            Error::EnvVar(err) => write!(fmt, "{}", err),
            Error::LineParse(line, error_index) => write!(
                fmt,
                "Error parsing line: '{}', error at line index: {}",
                line, error_index
            ),
        }
    }
}

#[cfg(test)]
mod test {
    use std::env;
    use std::error::Error as StdError;
    use std::io;

    use super::*;

    #[test]
    fn test_io_error_source() {
        let err = Error::Io(io::ErrorKind::PermissionDenied.into());
        let io_err = err.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(io::ErrorKind::PermissionDenied, io_err.kind());
    }

    #[test]
    fn test_envvar_error_source() {
        let err = Error::EnvVar(env::VarError::NotPresent);
        let var_err = err
            .source()
            .unwrap()
            .downcast_ref::<env::VarError>()
            .unwrap();
        assert_eq!(&env::VarError::NotPresent, var_err);
    }

    #[test]
    fn test_lineparse_error_source() {
        let err = Error::LineParse("test line".to_string(), 2);
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_not_found_true() {
        let err = Error::Io(io::ErrorKind::NotFound.into());
        assert!(err.not_found());
    }

    #[test]
    fn test_error_not_found_false() {
        let err = Error::Io(io::ErrorKind::PermissionDenied.into());
        assert!(!err.not_found());
    }

    #[test]
    fn test_io_error_display() {
        let err = Error::Io(io::ErrorKind::PermissionDenied.into());
        let io_err: io::Error = io::ErrorKind::PermissionDenied.into();

        let err_desc = format!("{}", err);
        let io_err_desc = format!("{}", io_err);
        assert_eq!(io_err_desc, err_desc);
    }

    #[test]
    fn test_envvar_error_display() {
        let err = Error::EnvVar(env::VarError::NotPresent);
        let var_err = env::VarError::NotPresent;

        let err_desc = format!("{}", err);
        let var_err_desc = format!("{}", var_err);
        assert_eq!(var_err_desc, err_desc);
    }

    #[test]
    fn test_lineparse_error_display() {
        let err = Error::LineParse("test line".to_string(), 2);
        let err_desc = format!("{}", err);
        assert_eq!(
            "Error parsing line: 'test line', error at line index: 2",
            err_desc
        );
    }
}
