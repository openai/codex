use std::error;
use std::fmt;
use std::os::raw::c_int;

/// Error Codes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
    /// Internal logic error in SQLite
    InternalMalfunction,
    /// Access permission denied
    PermissionDenied,
    /// Callback routine requested an abort
    OperationAborted,
    /// The database file is locked
    DatabaseBusy,
    /// A table in the database is locked
    DatabaseLocked,
    /// A malloc() failed
    OutOfMemory,
    /// Attempt to write a readonly database
    ReadOnly,
    /// Operation terminated by sqlite3_interrupt()
    OperationInterrupted,
    /// Some kind of disk I/O error occurred
    SystemIoFailure,
    /// The database disk image is malformed
    DatabaseCorrupt,
    /// Unknown opcode in sqlite3_file_control()
    NotFound,
    /// Insertion failed because database is full
    DiskFull,
    /// Unable to open the database file
    CannotOpen,
    /// Database lock protocol error
    FileLockingProtocolFailed,
    /// The database schema changed
    SchemaChanged,
    /// String or BLOB exceeds size limit
    TooBig,
    /// Abort due to constraint violation
    ConstraintViolation,
    /// Data type mismatch
    TypeMismatch,
    /// Library used incorrectly
    ApiMisuse,
    /// Uses OS features not supported on host
    NoLargeFileSupport,
    /// Authorization denied
    AuthorizationForStatementDenied,
    /// 2nd parameter to sqlite3_bind out of range
    ParameterOutOfRange,
    /// File opened that is not a database file
    NotADatabase,
    /// SQL error or missing database
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub extended_code: c_int,
}

impl Error {
    #[must_use]
    pub fn new(result_code: c_int) -> Error {
        let code = match result_code & 0xff {
            super::SQLITE_INTERNAL => ErrorCode::InternalMalfunction,
            super::SQLITE_PERM => ErrorCode::PermissionDenied,
            super::SQLITE_ABORT => ErrorCode::OperationAborted,
            super::SQLITE_BUSY => ErrorCode::DatabaseBusy,
            super::SQLITE_LOCKED => ErrorCode::DatabaseLocked,
            super::SQLITE_NOMEM => ErrorCode::OutOfMemory,
            super::SQLITE_READONLY => ErrorCode::ReadOnly,
            super::SQLITE_INTERRUPT => ErrorCode::OperationInterrupted,
            super::SQLITE_IOERR => ErrorCode::SystemIoFailure,
            super::SQLITE_CORRUPT => ErrorCode::DatabaseCorrupt,
            super::SQLITE_NOTFOUND => ErrorCode::NotFound,
            super::SQLITE_FULL => ErrorCode::DiskFull,
            super::SQLITE_CANTOPEN => ErrorCode::CannotOpen,
            super::SQLITE_PROTOCOL => ErrorCode::FileLockingProtocolFailed,
            super::SQLITE_SCHEMA => ErrorCode::SchemaChanged,
            super::SQLITE_TOOBIG => ErrorCode::TooBig,
            super::SQLITE_CONSTRAINT => ErrorCode::ConstraintViolation,
            super::SQLITE_MISMATCH => ErrorCode::TypeMismatch,
            super::SQLITE_MISUSE => ErrorCode::ApiMisuse,
            super::SQLITE_NOLFS => ErrorCode::NoLargeFileSupport,
            super::SQLITE_AUTH => ErrorCode::AuthorizationForStatementDenied,
            super::SQLITE_RANGE => ErrorCode::ParameterOutOfRange,
            super::SQLITE_NOTADB => ErrorCode::NotADatabase,
            _ => ErrorCode::Unknown,
        };

        Error {
            code,
            extended_code: result_code,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Error code {}: {}",
            self.extended_code,
            code_to_str(self.extended_code)
        )
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        code_to_str(self.extended_code)
    }
}

// Result codes.
// Note: These are not public because our bindgen bindings export whichever
// constants are present in the current version of SQLite. We repeat them here,
// so we don't have to worry about which version of SQLite added which
// constants, and we only use them to implement code_to_str below.

// Extended result codes.

const SQLITE_ERROR_MISSING_COLLSEQ: c_int = super::SQLITE_ERROR | (1 << 8);
const SQLITE_ERROR_RETRY: c_int = super::SQLITE_ERROR | (2 << 8);
const SQLITE_ERROR_SNAPSHOT: c_int = super::SQLITE_ERROR | (3 << 8);

const SQLITE_IOERR_BEGIN_ATOMIC: c_int = super::SQLITE_IOERR | (29 << 8);
const SQLITE_IOERR_COMMIT_ATOMIC: c_int = super::SQLITE_IOERR | (30 << 8);
const SQLITE_IOERR_ROLLBACK_ATOMIC: c_int = super::SQLITE_IOERR | (31 << 8);
const SQLITE_IOERR_DATA: c_int = super::SQLITE_IOERR | (32 << 8);
const SQLITE_IOERR_CORRUPTFS: c_int = super::SQLITE_IOERR | (33 << 8);
const SQLITE_IOERR_IN_PAGE: c_int = super::SQLITE_IOERR | (34 << 8);

const SQLITE_LOCKED_VTAB: c_int = super::SQLITE_LOCKED | (2 << 8);

const SQLITE_BUSY_TIMEOUT: c_int = super::SQLITE_BUSY | (3 << 8);

const SQLITE_CANTOPEN_SYMLINK: c_int = super::SQLITE_CANTOPEN | (6 << 8);

const SQLITE_CORRUPT_SEQUENCE: c_int = super::SQLITE_CORRUPT | (2 << 8);
const SQLITE_CORRUPT_INDEX: c_int = super::SQLITE_CORRUPT | (3 << 8);

const SQLITE_READONLY_CANTINIT: c_int = super::SQLITE_READONLY | (5 << 8);
const SQLITE_READONLY_DIRECTORY: c_int = super::SQLITE_READONLY | (6 << 8);

const SQLITE_CONSTRAINT_PINNED: c_int = super::SQLITE_CONSTRAINT | (11 << 8);
const SQLITE_CONSTRAINT_DATATYPE: c_int = super::SQLITE_CONSTRAINT | (12 << 8);

#[must_use]
pub fn code_to_str(code: c_int) -> &'static str {
    match code {
        super::SQLITE_OK        => "Successful result",
        super::SQLITE_ERROR     => "SQL error or missing database",
        super::SQLITE_INTERNAL  => "Internal logic error in SQLite",
        super::SQLITE_PERM      => "Access permission denied",
        super::SQLITE_ABORT     => "Callback routine requested an abort",
        super::SQLITE_BUSY      => "The database file is locked",
        super::SQLITE_LOCKED    => "A table in the database is locked",
        super::SQLITE_NOMEM     => "A malloc() failed",
        super::SQLITE_READONLY  => "Attempt to write a readonly database",
        super::SQLITE_INTERRUPT => "Operation terminated by sqlite3_interrupt()",
        super::SQLITE_IOERR     => "Some kind of disk I/O error occurred",
        super::SQLITE_CORRUPT   => "The database disk image is malformed",
        super::SQLITE_NOTFOUND  => "Unknown opcode in sqlite3_file_control()",
        super::SQLITE_FULL      => "Insertion failed because database is full",
        super::SQLITE_CANTOPEN  => "Unable to open the database file",
        super::SQLITE_PROTOCOL  => "Database lock protocol error",
        super::SQLITE_EMPTY     => "Database is empty",
        super::SQLITE_SCHEMA    => "The database schema changed",
        super::SQLITE_TOOBIG    => "String or BLOB exceeds size limit",
        super::SQLITE_CONSTRAINT=> "Abort due to constraint violation",
        super::SQLITE_MISMATCH  => "Data type mismatch",
        super::SQLITE_MISUSE    => "Library used incorrectly",
        super::SQLITE_NOLFS     => "Uses OS features not supported on host",
        super::SQLITE_AUTH      => "Authorization denied",
        super::SQLITE_FORMAT    => "Auxiliary database format error",
        super::SQLITE_RANGE     => "2nd parameter to sqlite3_bind out of range",
        super::SQLITE_NOTADB    => "File opened that is not a database file",
        super::SQLITE_NOTICE    => "Notifications from sqlite3_log()",
        super::SQLITE_WARNING   => "Warnings from sqlite3_log()",
        super::SQLITE_ROW       => "sqlite3_step() has another row ready",
        super::SQLITE_DONE      => "sqlite3_step() has finished executing",

        SQLITE_ERROR_MISSING_COLLSEQ   => "SQLITE_ERROR_MISSING_COLLSEQ",
        SQLITE_ERROR_RETRY   => "SQLITE_ERROR_RETRY",
        SQLITE_ERROR_SNAPSHOT   => "SQLITE_ERROR_SNAPSHOT",

        super::SQLITE_IOERR_READ              => "Error reading from disk",
        super::SQLITE_IOERR_SHORT_READ        => "Unable to obtain number of requested bytes (file truncated?)",
        super::SQLITE_IOERR_WRITE             => "Error writing to disk",
        super::SQLITE_IOERR_FSYNC             => "Error flushing data to persistent storage (fsync)",
        super::SQLITE_IOERR_DIR_FSYNC         => "Error calling fsync on a directory",
        super::SQLITE_IOERR_TRUNCATE          => "Error attempting to truncate file",
        super::SQLITE_IOERR_FSTAT             => "Error invoking fstat to get file metadata",
        super::SQLITE_IOERR_UNLOCK            => "I/O error within xUnlock of a VFS object",
        super::SQLITE_IOERR_RDLOCK            => "I/O error within xLock of a VFS object (trying to obtain a read lock)",
        super::SQLITE_IOERR_DELETE            => "I/O error within xDelete of a VFS object",
        super::SQLITE_IOERR_BLOCKED           => "SQLITE_IOERR_BLOCKED", // no longer used
        super::SQLITE_IOERR_NOMEM             => "Out of memory in I/O layer",
        super::SQLITE_IOERR_ACCESS            => "I/O error within xAccess of a VFS object",
        super::SQLITE_IOERR_CHECKRESERVEDLOCK => "I/O error within then xCheckReservedLock method",
        super::SQLITE_IOERR_LOCK              => "I/O error in the advisory file locking layer",
        super::SQLITE_IOERR_CLOSE             => "I/O error within the xClose method",
        super::SQLITE_IOERR_DIR_CLOSE         => "SQLITE_IOERR_DIR_CLOSE", // no longer used
        super::SQLITE_IOERR_SHMOPEN           => "I/O error within the xShmMap method (trying to open a new shared-memory segment)",
        super::SQLITE_IOERR_SHMSIZE           => "I/O error within the xShmMap method (trying to resize an existing shared-memory segment)",
        super::SQLITE_IOERR_SHMLOCK           => "SQLITE_IOERR_SHMLOCK", // no longer used
        super::SQLITE_IOERR_SHMMAP            => "I/O error within the xShmMap method (trying to map a shared-memory segment into process address space)",
        super::SQLITE_IOERR_SEEK              => "I/O error within the xRead or xWrite (trying to seek within a file)",
        super::SQLITE_IOERR_DELETE_NOENT      => "File being deleted does not exist",
        super::SQLITE_IOERR_MMAP              => "I/O error while trying to map or unmap part of the database file into process address space",
        super::SQLITE_IOERR_GETTEMPPATH       => "VFS is unable to determine a suitable directory for temporary files",
        super::SQLITE_IOERR_CONVPATH          => "cygwin_conv_path() system call failed",
        super::SQLITE_IOERR_VNODE             => "SQLITE_IOERR_VNODE", // not documented?
        super::SQLITE_IOERR_AUTH              => "SQLITE_IOERR_AUTH",
        SQLITE_IOERR_BEGIN_ATOMIC      => "SQLITE_IOERR_BEGIN_ATOMIC",
        SQLITE_IOERR_COMMIT_ATOMIC     => "SQLITE_IOERR_COMMIT_ATOMIC",
        SQLITE_IOERR_ROLLBACK_ATOMIC   => "SQLITE_IOERR_ROLLBACK_ATOMIC",
        SQLITE_IOERR_DATA   => "SQLITE_IOERR_DATA",
        SQLITE_IOERR_CORRUPTFS   => "SQLITE_IOERR_CORRUPTFS",
        SQLITE_IOERR_IN_PAGE   => "SQLITE_IOERR_IN_PAGE",

        super::SQLITE_LOCKED_SHAREDCACHE      => "Locking conflict due to another connection with a shared cache",
        SQLITE_LOCKED_VTAB             => "SQLITE_LOCKED_VTAB",

        super::SQLITE_BUSY_RECOVERY           => "Another process is recovering a WAL mode database file",
        super::SQLITE_BUSY_SNAPSHOT           => "Cannot promote read transaction to write transaction because of writes by another connection",
        SQLITE_BUSY_TIMEOUT           => "SQLITE_BUSY_TIMEOUT",

        super::SQLITE_CANTOPEN_NOTEMPDIR      => "SQLITE_CANTOPEN_NOTEMPDIR", // no longer used
        super::SQLITE_CANTOPEN_ISDIR          => "Attempted to open directory as file",
        super::SQLITE_CANTOPEN_FULLPATH       => "Unable to convert filename into full pathname",
        super::SQLITE_CANTOPEN_CONVPATH       => "cygwin_conv_path() system call failed",
        SQLITE_CANTOPEN_SYMLINK       => "SQLITE_CANTOPEN_SYMLINK",

        super::SQLITE_CORRUPT_VTAB            => "Content in the virtual table is corrupt",
        SQLITE_CORRUPT_SEQUENCE        => "SQLITE_CORRUPT_SEQUENCE",
        SQLITE_CORRUPT_INDEX        => "SQLITE_CORRUPT_INDEX",

        super::SQLITE_READONLY_RECOVERY       => "WAL mode database file needs recovery (requires write access)",
        super::SQLITE_READONLY_CANTLOCK       => "Shared-memory file associated with WAL mode database is read-only",
        super::SQLITE_READONLY_ROLLBACK       => "Database has hot journal that must be rolled back (requires write access)",
        super::SQLITE_READONLY_DBMOVED        => "Database cannot be modified because database file has moved",
        SQLITE_READONLY_CANTINIT       => "SQLITE_READONLY_CANTINIT",
        SQLITE_READONLY_DIRECTORY      => "SQLITE_READONLY_DIRECTORY",

        super::SQLITE_ABORT_ROLLBACK          => "Transaction was rolled back",

        super::SQLITE_CONSTRAINT_CHECK        => "A CHECK constraint failed",
        super::SQLITE_CONSTRAINT_COMMITHOOK   => "Commit hook caused rollback",
        super::SQLITE_CONSTRAINT_FOREIGNKEY   => "Foreign key constraint failed",
        super::SQLITE_CONSTRAINT_FUNCTION     => "Error returned from extension function",
        super::SQLITE_CONSTRAINT_NOTNULL      => "A NOT NULL constraint failed",
        super::SQLITE_CONSTRAINT_PRIMARYKEY   => "A PRIMARY KEY constraint failed",
        super::SQLITE_CONSTRAINT_TRIGGER      => "A RAISE function within a trigger fired",
        super::SQLITE_CONSTRAINT_UNIQUE       => "A UNIQUE constraint failed",
        super::SQLITE_CONSTRAINT_VTAB         => "An application-defined virtual table error occurred",
        super::SQLITE_CONSTRAINT_ROWID        => "A non-unique rowid occurred",
        SQLITE_CONSTRAINT_PINNED        => "SQLITE_CONSTRAINT_PINNED",
        SQLITE_CONSTRAINT_DATATYPE        => "SQLITE_CONSTRAINT_DATATYPE",

        super::SQLITE_NOTICE_RECOVER_WAL      => "A WAL mode database file was recovered",
        super::SQLITE_NOTICE_RECOVER_ROLLBACK => "Hot journal was rolled back",

        super::SQLITE_WARNING_AUTOINDEX       => "Automatic indexing used - database might benefit from additional indexes",

        super::SQLITE_AUTH_USER               => "SQLITE_AUTH_USER", // not documented?

        _ => "Unknown error code",
    }
}

/// Loadable extension initialization error
#[cfg(feature = "loadable_extension")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InitError {
    /// Version mismatch between the extension and the SQLite3 library
    VersionMismatch { compile_time: i32, runtime: i32 },
    /// Invalid function pointer in one of sqlite3_api_routines fields
    NullFunctionPointer,
}
#[cfg(feature = "loadable_extension")]
impl ::std::fmt::Display for InitError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            InitError::VersionMismatch {
                compile_time,
                runtime,
            } => {
                write!(f, "SQLite version mismatch: {runtime} < {compile_time}")
            }
            InitError::NullFunctionPointer => {
                write!(f, "Some sqlite3_api_routines fields are null")
            }
        }
    }
}
#[cfg(feature = "loadable_extension")]
impl error::Error for InitError {}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    pub fn error_new() {
        let assoc = vec![
            (SQLITE_INTERNAL, ErrorCode::InternalMalfunction),
            (SQLITE_PERM, ErrorCode::PermissionDenied),
            (SQLITE_ABORT_ROLLBACK, ErrorCode::OperationAborted),
            (SQLITE_BUSY_RECOVERY, ErrorCode::DatabaseBusy),
            (SQLITE_LOCKED_SHAREDCACHE, ErrorCode::DatabaseLocked),
            (SQLITE_NOMEM, ErrorCode::OutOfMemory),
            (SQLITE_IOERR_READ, ErrorCode::SystemIoFailure),
            (SQLITE_NOTFOUND, ErrorCode::NotFound),
            (SQLITE_FULL, ErrorCode::DiskFull),
            (SQLITE_PROTOCOL, ErrorCode::FileLockingProtocolFailed),
            (SQLITE_SCHEMA, ErrorCode::SchemaChanged),
            (SQLITE_TOOBIG, ErrorCode::TooBig),
            (SQLITE_MISMATCH, ErrorCode::TypeMismatch),
            (SQLITE_NOLFS, ErrorCode::NoLargeFileSupport),
            (SQLITE_RANGE, ErrorCode::ParameterOutOfRange),
            (SQLITE_NOTADB, ErrorCode::NotADatabase),
        ];
        for (sqlite_code, rust_code) in assoc {
            let err = Error::new(sqlite_code);
            assert_eq!(
                err,
                Error {
                    code: rust_code,
                    extended_code: sqlite_code
                }
            );
            let s = format!("{}", err);
            assert!(!s.is_empty());
        }
    }
}
