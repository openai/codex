use anyhow::Context;
use anyhow::Result;
use libsqlite3_sys as ffi;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_void;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const SQLITE_RECOVER_LOST_AND_FOUND: c_int = 1;
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
const SQLITE_UTF8_ENCODING: u32 = 1;
const SQLITE_SCHEMA_FORMAT_4: u32 = 4;
const SQLITE_MAX_U16_PAGE_SIZE_SENTINEL: u32 = 1;
const SQLITE_MAX_PAGE_SIZE: usize = 65_536;
const SQLITE_MIN_PAGE_SIZE: usize = 512;
const SQLITE_PAGE1_BTREE_HEADER_OFFSET: usize = 100;
const SQLITE_LEAF_TABLE_PAGE: u8 = 0x0d;
const HEADER_REPAIR_SCAN_PAGES: usize = 64;

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
    match recover_inner(path, recovered_path) {
        Ok(()) => Ok(()),
        Err(err) if is_notadb_error(&err) => {
            let repaired = HeaderRepairedInput::create(path).with_context(|| {
                format!(
                    "failed to prepare header-repaired recovery input for {}",
                    path.display()
                )
            })?;
            recover_inner(repaired.path(), recovered_path).with_context(|| {
                format!(
                    "SQLite recovery with a synthesized header failed after direct recovery failed: {err}"
                )
            })
        }
        Err(err) => Err(err),
    }
}

fn recover_inner(path: &Path, recovered_path: &Path) -> Result<()> {
    let db = SqliteHandle::open(path)?;
    let recovered_path = path_to_cstring(recovered_path)?;
    let mut recovery = RecoveryHandle::new(db.as_ptr(), recovered_path.as_c_str())?;
    recovery.configure_lost_and_found()?;
    recovery.run()?;
    recovery.finish()
}

fn is_notadb_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("(26)") || message.contains("file is not a database")
    })
}

struct HeaderRepairedInput {
    path: PathBuf,
}

impl HeaderRepairedInput {
    fn create(path: &Path) -> Result<Self> {
        let mut input =
            File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        let file_len = input.metadata()?.len();
        let page_size = detect_page_size(&mut input, file_len)?;
        let page_count = file_len
            .checked_div(page_size as u64)
            .context("database page count overflowed")?;
        let page_count = u32::try_from(page_count)
            .context("database is too large to synthesize a SQLite header")?;

        let mut output = None;
        for sequence in 0..1000 {
            let repaired_path = header_repair_path(path, sequence)?;
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(repaired_path.as_path())
            {
                Ok(file) => {
                    output = Some((repaired_path, file));
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err.into()),
            }
        }
        let (repaired_path, mut output) =
            output.context("failed to allocate a unique header-repaired recovery path")?;

        let header = synthesized_header_page(page_size, page_count)?;
        output.write_all(header.as_slice())?;
        input.seek(SeekFrom::Start(page_size as u64))?;
        std::io::copy(&mut input, &mut output)?;

        Ok(Self {
            path: repaired_path,
        })
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for HeaderRepairedInput {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path.as_path());
    }
}

fn header_repair_path(path: &Path, sequence: u32) -> Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        anyhow::anyhow!(
            "cannot create a header-repaired recovery file name for {}",
            path.display()
        )
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut candidate = file_name.to_os_string();
    candidate.push(format!(
        ".codex-recovery-header-repair.{}.{timestamp}.{sequence}.sqlite",
        std::process::id()
    ));
    Ok(path.with_file_name(candidate))
}

fn detect_page_size(input: &mut File, file_len: u64) -> Result<usize> {
    if file_len < SQLITE_MIN_PAGE_SIZE as u64 {
        anyhow::bail!("database file is too small to repair its SQLite header");
    }

    input.seek(SeekFrom::Start(0))?;
    let scan_len = file_len.min((SQLITE_MAX_PAGE_SIZE * 4) as u64) as usize;
    let mut prefix = vec![0; scan_len];
    input.read_exact(prefix.as_mut_slice())?;

    let mut best_page_size = None;
    let mut best_valid_pages = 0;
    let mut page_size = SQLITE_MIN_PAGE_SIZE;
    while page_size <= SQLITE_MAX_PAGE_SIZE {
        if file_len.is_multiple_of(page_size as u64) {
            let valid_pages = count_valid_btree_pages(prefix.as_slice(), page_size);
            if valid_pages > best_valid_pages {
                best_valid_pages = valid_pages;
                best_page_size = Some(page_size);
            }
        }
        page_size *= 2;
    }

    best_page_size.context("failed to infer SQLite page size for header repair")
}

fn count_valid_btree_pages(prefix: &[u8], page_size: usize) -> usize {
    prefix
        .chunks_exact(page_size)
        .take(HEADER_REPAIR_SCAN_PAGES)
        .filter(|page| is_plausible_btree_page(page))
        .count()
}

fn is_plausible_btree_page(page: &[u8]) -> bool {
    let page_type = page[0];
    let header_size = match page_type {
        0x02 | 0x05 => 12,
        0x0a | 0x0d => 8,
        _ => return false,
    };
    let first_freeblock = read_u16(&page[1..3]) as usize;
    let cell_count = read_u16(&page[3..5]) as usize;
    let cell_content_start = read_u16(&page[5..7]) as usize;
    let cell_content_start = if cell_content_start == 0 && page.len() == SQLITE_MAX_PAGE_SIZE {
        SQLITE_MAX_PAGE_SIZE
    } else {
        cell_content_start
    };
    let pointer_array_end = header_size + cell_count.saturating_mul(2);

    if first_freeblock >= page.len()
        || cell_content_start > page.len()
        || pointer_array_end > page.len()
        || cell_count > (page.len() - header_size) / 2
    {
        return false;
    }

    for index in 0..cell_count {
        let offset = header_size + index * 2;
        let cell_offset = read_u16(&page[offset..offset + 2]) as usize;
        if cell_offset < pointer_array_end || cell_offset >= page.len() {
            return false;
        }
    }

    true
}

fn synthesized_header_page(page_size: usize, page_count: u32) -> Result<Vec<u8>> {
    if !(SQLITE_MIN_PAGE_SIZE..=SQLITE_MAX_PAGE_SIZE).contains(&page_size)
        || !page_size.is_power_of_two()
    {
        anyhow::bail!("invalid SQLite page size inferred for header repair: {page_size}");
    }

    let mut page = vec![0; page_size];
    page[0..16].copy_from_slice(SQLITE_HEADER);
    let header_page_size = if page_size == SQLITE_MAX_PAGE_SIZE {
        SQLITE_MAX_U16_PAGE_SIZE_SENTINEL
    } else {
        page_size as u32
    };
    write_u16(&mut page[16..18], header_page_size);
    page[18] = 1;
    page[19] = 1;
    page[20] = 0;
    page[21] = 64;
    page[22] = 32;
    page[23] = 32;
    write_u32(&mut page[28..32], page_count);
    write_u32(&mut page[44..48], SQLITE_SCHEMA_FORMAT_4);
    write_u32(&mut page[56..60], SQLITE_UTF8_ENCODING);

    // Use an empty sqlite_schema b-tree. The damaged page 1 cannot be trusted,
    // and orphaned schema rows will be recovered through lost_and_found.
    page[SQLITE_PAGE1_BTREE_HEADER_OFFSET] = SQLITE_LEAF_TABLE_PAGE;
    let btree_content_start = if page_size == SQLITE_MAX_PAGE_SIZE {
        0
    } else {
        page_size as u32
    };
    write_u16(
        &mut page[SQLITE_PAGE1_BTREE_HEADER_OFFSET + 5..SQLITE_PAGE1_BTREE_HEADER_OFFSET + 7],
        btree_content_start,
    );
    Ok(page)
}

fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn write_u16(bytes: &mut [u8], value: u32) {
    bytes.copy_from_slice(&(value as u16).to_be_bytes());
}

fn write_u32(bytes: &mut [u8], value: u32) {
    bytes.copy_from_slice(&value.to_be_bytes());
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
