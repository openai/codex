use anyhow::Context;
use anyhow::Result;
use libsqlite3_sys as ffi;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_void;
use std::path::Path;
use std::ptr;

const SQLITE_RECOVER_LOST_AND_FOUND: c_int = 1;

#[repr(C)]
struct SqliteRecover {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn sqlite3_recover_init(
        db: *mut ffi::sqlite3,
        z_db: *const c_char,
        z_uri: *const c_char,
    ) -> *mut SqliteRecover;
    fn sqlite3_recover_config(recover: *mut SqliteRecover, op: c_int, arg: *mut c_void) -> c_int;
    fn sqlite3_recover_run(recover: *mut SqliteRecover) -> c_int;
    fn sqlite3_recover_errmsg(recover: *mut SqliteRecover) -> *const c_char;
    fn sqlite3_recover_errcode(recover: *mut SqliteRecover) -> c_int;
    fn sqlite3_recover_finish(recover: *mut SqliteRecover) -> c_int;
}

pub(super) fn recover(path: &Path, recovered_path: &Path) -> Result<()> {
    let db = SqliteHandle::open(path)?;
    let recovered_path = path_to_cstring(recovered_path)?;
    let mut recovery = RecoveryHandle::new(db.as_ptr(), recovered_path.as_c_str())?;
    recovery.configure_lost_and_found()?;
    recovery.run()?;
    recovery.finish()
}

struct SqliteHandle {
    db: *mut ffi::sqlite3,
}

impl SqliteHandle {
    fn open(path: &Path) -> Result<Self> {
        let path = path_to_cstring(path)?;
        let mut db = ptr::null_mut();
        let flags = ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_URI;
        // The recovery API reads pages through sqlite_dbpage on this handle.
        // It does not depend on SQLx because the database may be malformed.
        let rc = unsafe { ffi::sqlite3_open_v2(path.as_ptr(), &mut db, flags, ptr::null()) };
        if rc != ffi::SQLITE_OK {
            let message = sqlite_error_message(db);
            if !db.is_null() {
                let _ = unsafe { ffi::sqlite3_close(db) };
            }
            anyhow::bail!("failed to open malformed database for recovery ({rc}): {message}");
        }
        Ok(Self { db })
    }

    fn as_ptr(&self) -> *mut ffi::sqlite3 {
        self.db
    }
}

impl Drop for SqliteHandle {
    fn drop(&mut self) {
        if !self.db.is_null() {
            let _ = unsafe { ffi::sqlite3_close(self.db) };
        }
    }
}

struct RecoveryHandle {
    recover: *mut SqliteRecover,
}

impl RecoveryHandle {
    fn new(db: *mut ffi::sqlite3, recovered_path: &CStr) -> Result<Self> {
        let recover =
            unsafe { sqlite3_recover_init(db, c"main".as_ptr(), recovered_path.as_ptr()) };
        if recover.is_null() {
            anyhow::bail!("failed to initialize SQLite recovery: out of memory");
        }
        Ok(Self { recover })
    }

    fn configure_lost_and_found(&mut self) -> Result<()> {
        // Match sqlite3 shell recovery behavior by keeping orphaned rows in a
        // table instead of discarding pages not reachable from recovered schema.
        let table_name = c"lost_and_found";
        self.configure(
            SQLITE_RECOVER_LOST_AND_FOUND,
            table_name.as_ptr().cast_mut().cast(),
        )
    }

    fn configure(&mut self, op: c_int, arg: *mut c_void) -> Result<()> {
        let rc = unsafe { sqlite3_recover_config(self.recover, op, arg) };
        if rc != ffi::SQLITE_OK {
            anyhow::bail!(
                "failed to configure SQLite recovery ({rc}): {}",
                self.error_message()
            );
        }
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let rc = unsafe { sqlite3_recover_run(self.recover) };
        if rc != ffi::SQLITE_OK {
            anyhow::bail!("SQLite recovery failed ({rc}): {}", self.error_message());
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        let rc = unsafe { sqlite3_recover_finish(self.recover) };
        self.recover = ptr::null_mut();
        if rc != ffi::SQLITE_OK {
            anyhow::bail!("SQLite recovery cleanup failed ({rc})");
        }
        Ok(())
    }

    fn error_message(&self) -> String {
        let errcode = unsafe { sqlite3_recover_errcode(self.recover) };
        let message = unsafe { sqlite3_recover_errmsg(self.recover) };
        format!("{errcode}: {}", c_string_lossy(message))
    }
}

impl Drop for RecoveryHandle {
    fn drop(&mut self) {
        if !self.recover.is_null() {
            let _ = unsafe { sqlite3_recover_finish(self.recover) };
        }
    }
}

fn sqlite_error_message(db: *mut ffi::sqlite3) -> String {
    if db.is_null() {
        return "out of memory".to_string();
    }
    c_string_lossy(unsafe { ffi::sqlite3_errmsg(db) })
}

fn c_string_lossy(message: *const c_char) -> String {
    if message.is_null() {
        return "unknown error".to_string();
    }
    unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(unix)]
fn path_to_cstring(path: &Path) -> Result<CString> {
    use std::os::unix::ffi::OsStrExt;

    CString::new(path.as_os_str().as_bytes())
        .with_context(|| format!("path contains a NUL byte: {}", path.display()))
}

#[cfg(not(unix))]
fn path_to_cstring(path: &Path) -> Result<CString> {
    let path_str = path
        .to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))?;
    CString::new(path_str).with_context(|| format!("path contains a NUL byte: {}", path.display()))
}
