use std::ffi::CStr;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::raw::c_char;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::bazel_bwrap;
use crate::exec_util::argv_to_cstrings;
use crate::exec_util::make_files_inheritable;
use codex_utils_absolute_path::AbsolutePathBuf;
use sha2::Digest as _;
use sha2::Sha256;

const SHA256_HEX_LEN: usize = 64;
const NULL_SHA256_DIGEST: [u8; 32] = [0; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundledBwrapLauncher {
    program: AbsolutePathBuf,
}

pub(crate) fn launcher() -> Option<BundledBwrapLauncher> {
    let current_exe = std::env::current_exe().ok()?;
    find_for_exe(&current_exe).map(|program| BundledBwrapLauncher { program })
}

impl BundledBwrapLauncher {
    pub(crate) fn exec(&self, argv: Vec<String>, preserved_files: Vec<File>) -> ! {
        let bwrap_file = File::open(self.program.as_path()).unwrap_or_else(|err| {
            panic!(
                "failed to open bundled bubblewrap {}: {err}",
                self.program.as_path().display()
            )
        });
        verify_digest(&bwrap_file, expected_sha256(), self.program.as_path())
            .unwrap_or_else(|err| panic!("{err}"));

        make_files_inheritable(&preserved_files);

        let fd_path = format!("/proc/self/fd/{}", bwrap_file.as_raw_fd());
        let program_cstring = CString::new(fd_path.as_str())
            .unwrap_or_else(|err| panic!("invalid bundled bubblewrap fd path: {err}"));
        let cstrings = argv_to_cstrings(&argv);
        let mut argv_ptrs: Vec<*const c_char> = cstrings
            .iter()
            .map(CString::as_c_str)
            .map(CStr::as_ptr)
            .collect();
        argv_ptrs.push(std::ptr::null());

        // SAFETY: `program_cstring` and every entry in `argv_ptrs` are valid C
        // strings for the duration of the call. On success `execv` does not return.
        unsafe {
            libc::execv(program_cstring.as_ptr(), argv_ptrs.as_ptr());
        }
        let err = std::io::Error::last_os_error();
        panic!(
            "failed to exec bundled bubblewrap {} via {fd_path}: {err}",
            self.program.as_path().display()
        );
    }
}

fn find_for_exe(exe: &Path) -> Option<AbsolutePathBuf> {
    candidates_for_exe(exe)
        .into_iter()
        .find(|candidate| is_executable_file(candidate))
        .map(|path| {
            AbsolutePathBuf::from_absolute_path(&path).unwrap_or_else(|err| {
                panic!(
                    "failed to normalize bundled bubblewrap path {}: {err}",
                    path.display()
                )
            })
        })
}

fn candidates_for_exe(exe: &Path) -> Vec<PathBuf> {
    let Some(exe_dir) = exe.parent() else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    candidates.push(exe_dir.join("codex-resources").join("bwrap"));
    if let Some(package_target_dir) = exe_dir.parent() {
        candidates.push(package_target_dir.join("codex-resources").join("bwrap"));
    }
    candidates.push(exe_dir.join("bwrap"));
    if let Some(path) = bazel_bwrap::candidate() {
        candidates.push(path);
    }
    candidates
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

fn expected_sha256() -> Option<[u8; 32]> {
    static EXPECTED: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    *EXPECTED.get_or_init(|| {
        let raw_digest = option_env!("CODEX_BWRAP_SHA256")?;
        let digest = parse_sha256_hex(raw_digest)
            .unwrap_or_else(|err| panic!("invalid CODEX_BWRAP_SHA256 value: {err}"));
        (digest != NULL_SHA256_DIGEST).then_some(digest)
    })
}

fn verify_digest(file: &File, expected: Option<[u8; 32]>, path: &Path) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };

    let mut file = file
        .try_clone()
        .map_err(|err| format!("failed to clone bundled bubblewrap fd: {err}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|err| {
            format!(
                "failed to read bundled bubblewrap {} for digest verification: {err}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let actual: [u8; 32] = hasher.finalize().into();
    if actual == expected {
        return Ok(());
    }

    Err(format!(
        "bundled bubblewrap digest mismatch for {}: expected sha256:{}, got sha256:{}",
        path.display(),
        bytes_to_hex(&expected),
        bytes_to_hex(&actual),
    ))
}

fn parse_sha256_hex(raw: &str) -> Result<[u8; 32], String> {
    if raw.len() != SHA256_HEX_LEN {
        return Err(format!(
            "expected {SHA256_HEX_LEN} hex characters, got {}",
            raw.len()
        ));
    }

    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&raw[start..start + 2], 16)
            .map_err(|err| format!("invalid hex byte at offset {start}: {err}"))?;
    }
    Ok(digest)
}

fn bytes_to_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(SHA256_HEX_LEN);
    for byte in bytes {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0x0f) as usize] as char);
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::NamedTempFile;
    use tempfile::tempdir;

    #[test]
    fn finds_standalone_bundled_bwrap_next_to_exe_resources() {
        let temp_dir = tempdir().expect("temp dir");
        let exe = temp_dir.path().join("codex");
        let expected_bwrap = temp_dir.path().join("codex-resources").join("bwrap");
        write_executable(&exe);
        write_executable(&expected_bwrap);

        assert_eq!(
            find_for_exe(&exe),
            Some(AbsolutePathBuf::from_absolute_path(&expected_bwrap).expect("absolute"))
        );
    }

    #[test]
    fn finds_npm_bundled_bwrap_next_to_target_vendor_dir() {
        let temp_dir = tempdir().expect("temp dir");
        let target_dir = temp_dir.path().join("vendor/x86_64-unknown-linux-musl");
        let exe = target_dir.join("codex").join("codex");
        let expected_bwrap = target_dir.join("codex-resources").join("bwrap");
        write_executable(&exe);
        write_executable(&expected_bwrap);

        assert_eq!(
            find_for_exe(&exe),
            Some(AbsolutePathBuf::from_absolute_path(&expected_bwrap).expect("absolute"))
        );
    }

    #[test]
    fn finds_adjacent_dev_bwrap() {
        let temp_dir = tempdir().expect("temp dir");
        let exe = temp_dir.path().join("codex");
        let expected_bwrap = temp_dir.path().join("bwrap");
        write_executable(&exe);
        write_executable(&expected_bwrap);

        assert_eq!(
            find_for_exe(&exe),
            Some(AbsolutePathBuf::from_absolute_path(&expected_bwrap).expect("absolute"))
        );
    }

    #[test]
    fn digest_verification_skips_missing_expected_digest() {
        let file = NamedTempFile::new().expect("temp file");
        fs::write(file.path(), b"contents").expect("write file");

        verify_digest(file.as_file(), /*expected*/ None, file.path())
            .expect("missing digest should skip verification");
    }

    #[test]
    fn digest_verification_accepts_matching_digest() {
        let file = NamedTempFile::new().expect("temp file");
        fs::write(file.path(), b"contents").expect("write file");
        let expected: [u8; 32] = Sha256::digest(b"contents").into();

        verify_digest(file.as_file(), Some(expected), file.path())
            .expect("matching digest should verify");
    }

    #[test]
    fn digest_verification_rejects_mismatched_digest() {
        let file = NamedTempFile::new().expect("temp file");
        fs::write(file.path(), b"contents").expect("write file");

        let err = verify_digest(file.as_file(), Some([0xab; 32]), file.path())
            .expect_err("mismatched digest should fail");
        assert!(err.contains("bundled bubblewrap digest mismatch"));
    }

    #[test]
    fn parses_sha256_hex_digest() {
        assert_eq!(parse_sha256_hex(&"ab".repeat(32)), Ok([0xab; 32]));
        assert_eq!(parse_sha256_hex(&"00".repeat(32)), Ok(NULL_SHA256_DIGEST));
        assert!(parse_sha256_hex("ab").is_err());
        assert!(parse_sha256_hex(&format!("{}xx", "00".repeat(31))).is_err());
    }

    fn write_executable(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, b"").expect("write executable");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .expect("set executable permissions");
    }
}
