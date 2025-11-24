//! Filesystem paths in Windows are a total mess. This crate normalizes paths to the most
//! compatible (but still correct) format, so that you don't have to worry about the mess.
//!
//! In Windows the regular/legacy paths (`C:\foo`) are supported by all programs, but have
//! lots of bizarre restrictions for backwards compatibility with MS-DOS.
//!
//! And there are Windows NT UNC paths (`\\?\C:\foo`), which are more robust and with fewer
//! gotchas, but are rarely supported by Windows programs. Even Microsoft's own!
//!
//! This crate converts paths to legacy format whenever possible, but leaves UNC paths as-is
//! when they can't be unambiguously expressed in a simpler way. This allows legacy programs
//! to access all paths they can possibly access, and UNC-aware programs to access all paths.
//!
//! On non-Windows platforms these functions leave paths unmodified, so it's safe to use them
//! unconditionally for all platforms.
//!
//! Parsing is based on <https://msdn.microsoft.com/en-us/library/windows/desktop/aa365247(v=vs.85).aspx>
//!
//! [Project homepage](https://lib.rs/crates/dunce).
#![doc(html_logo_url = "https://assets.gitlab-static.net/uploads/-/system/project/avatar/4717715/dyc.png")]

#[cfg(any(windows, test))]
use std::ffi::OsStr;
use std::fs;
use std::io;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};

/// Takes any path, and when possible, converts Windows UNC paths to regular paths.
/// If the path can't be converted, it's returned unmodified.
///
/// On non-Windows this is no-op.
///
/// `\\?\C:\Windows` will be converted to `C:\Windows`,
/// but `\\?\C:\COM` will be left as-is (due to a reserved filename).
///
/// Use this to pass arbitrary paths to programs that may not be UNC-aware.
///
/// It's generally safe to pass UNC paths to legacy programs, because
/// these paths contain a reserved prefix, so will gracefully fail
/// if used with legacy APIs that don't support UNC.
///
/// This function does not perform any I/O.
///
/// Currently paths with unpaired surrogates aren't converted even if they
/// could be, due to limitations of Rust's `OsStr` API.
///
/// To check if a path remained as UNC, use `path.as_os_str().as_encoded_bytes().starts_with(b"\\\\")`.
#[inline]
pub fn simplified(path: &Path) -> &Path {
    if is_safe_to_strip_unc(path) {
        // unfortunately we can't safely strip prefix from a non-Unicode path
        path.to_str().and_then(|s| s.get(4..)).map_or(path, Path::new)
    } else {
        path
    }
}

/// Like `std::fs::canonicalize()`, but on Windows it outputs the most
/// compatible form of a path instead of UNC.
#[inline(always)]
pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref();

    #[cfg(not(windows))]
    {
        fs::canonicalize(path)
    }
    #[cfg(windows)]
    {
        canonicalize_win(path)
    }
}

#[cfg(windows)]
fn canonicalize_win(path: &Path) -> io::Result<PathBuf> {
    let real_path = fs::canonicalize(path)?;
    Ok(if is_safe_to_strip_unc(&real_path) {
        real_path.to_str().and_then(|s| s.get(4..)).map(PathBuf::from).unwrap_or(real_path)
    } else {
        real_path
    })
}

pub use self::canonicalize as realpath;

#[cfg(any(windows,test))]
fn windows_char_len(s: &OsStr) -> usize {
    #[cfg(not(windows))]
    let len = s.to_string_lossy().chars().map(|c| if c as u32 <= 0xFFFF {1} else {2}).sum();
    #[cfg(windows)]
    let len = s.encode_wide().count();
    len
}

#[cfg(any(windows,test))]
fn is_valid_filename(file_name: &OsStr) -> bool {
    if file_name.len() > 255 && windows_char_len(file_name) > 255 {
        return false;
    }

    // Non-unicode is safe, but Rust can't reasonably losslessly operate on such strings
    let byte_str = if let Some(s) = file_name.to_str() {
        s.as_bytes()
    } else {
        return false;
    };
    if byte_str.is_empty() {
        return false;
    }
    // Only ASCII subset is checked, and WTF-8/UTF-8 is safe for that
    if byte_str.iter().any(|&c| matches!(c, 0..=31 | b'<' | b'>' | b':' | b'"' | b'/' | b'\\' | b'|' | b'?' | b'*')) {
        return false
    }
    // Filename can't end with . or space (except before extension, but this checks the whole name)
    if matches!(byte_str.last(), Some(b' ' | b'.')) {
        return false;
    }
    true
}

#[cfg(any(windows, test))]
const RESERVED_NAMES: [&str; 22] = [
    "AUX", "NUL", "PRN", "CON", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[cfg(any(windows, test))]
fn is_reserved<P: AsRef<OsStr>>(file_name: P) -> bool {
    // con.txt is reserved too
    // all reserved DOS names have ASCII-compatible stem
    if let Some(name) = Path::new(&file_name).file_stem().and_then(|s| s.to_str()) {
        // "con.. .txt" is "CON" for DOS
        let trimmed = right_trim(name);
        if trimmed.len() <= 4 && RESERVED_NAMES.into_iter().any(|name| trimmed.eq_ignore_ascii_case(name)) {
            return true;
        }
    }
    false
}

#[cfg(not(windows))]
#[inline]
const fn is_safe_to_strip_unc(_path: &Path) -> bool {
    false
}

#[cfg(windows)]
fn is_safe_to_strip_unc(path: &Path) -> bool {
    let mut components = path.components();
    match components.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::VerbatimDisk(..) => {},
            _ => return false, // Other kinds of UNC paths
        },
        _ => return false, // relative or empty
    }

    for component in components {
        match component {
            Component::RootDir => {},
            Component::Normal(file_name) => {
                // it doesn't allocate in most cases,
                // and checks are interested only in the ASCII subset, so lossy is fine
                if !is_valid_filename(file_name) || is_reserved(file_name) {
                    return false;
                }
            }
            _ => return false, // UNC paths take things like ".." literally
        };
    }

    let path_os_str = path.as_os_str();
    // However, if the path is going to be used as a directory it's 248
    if path_os_str.len() > 260 && windows_char_len(path_os_str) > 260 {
        return false;
    }
    true
}

/// Trim '.' and ' '
#[cfg(any(windows, test))]
fn right_trim(s: &str) -> &str {
    s.trim_end_matches([' ','.'])
}

#[test]
fn trim_test() {
    assert_eq!("a", right_trim("a."));
    assert_eq!("ą", right_trim("ą."));
    assert_eq!("a", right_trim("a "));
    assert_eq!("ąą", right_trim("ąą "));
    assert_eq!("a", right_trim("a. . . ....   "));
    assert_eq!("a. . . ..ź", right_trim("a. . . ..ź..   "));
    assert_eq!(" b", right_trim(" b"));
    assert_eq!(" べ", right_trim(" べ"));
    assert_eq!("c. c", right_trim("c. c."));
    assert_eq!("。", right_trim("。"));
    assert_eq!("", right_trim(""));
}

#[test]
fn reserved() {
    assert!(is_reserved("CON"));
    assert!(is_reserved("con"));
    assert!(is_reserved("con.con"));
    assert!(is_reserved("COM4"));
    assert!(is_reserved("COM4.txt"));
    assert!(is_reserved("COM4 .txt"));
    assert!(is_reserved("con."));
    assert!(is_reserved("con ."));
    assert!(is_reserved("con  "));
    assert!(is_reserved("con . "));
    assert!(is_reserved("con . .txt"));
    assert!(is_reserved("con.....txt"));
    assert!(is_reserved("PrN....."));

    assert!(!is_reserved(" PrN....."));
    assert!(!is_reserved(" CON"));
    assert!(!is_reserved("COM0"));
    assert!(!is_reserved("COM77"));
    assert!(!is_reserved(" CON "));
    assert!(!is_reserved(".CON"));
    assert!(!is_reserved("@CON"));
    assert!(!is_reserved("not.CON"));
    assert!(!is_reserved("CON。"));
}

#[test]
fn len() {
    assert_eq!(1, windows_char_len(OsStr::new("a")));
    assert_eq!(1, windows_char_len(OsStr::new("€")));
    assert_eq!(1, windows_char_len(OsStr::new("本")));
    assert_eq!(2, windows_char_len(OsStr::new("🧐")));
    assert_eq!(2, windows_char_len(OsStr::new("®®")));
}

#[test]
fn valid() {
    assert!(!is_valid_filename("..".as_ref()));
    assert!(!is_valid_filename(".".as_ref()));
    assert!(!is_valid_filename("aaaaaaaaaa:".as_ref()));
    assert!(!is_valid_filename("ą:ą".as_ref()));
    assert!(!is_valid_filename("".as_ref()));
    assert!(!is_valid_filename("a ".as_ref()));
    assert!(!is_valid_filename(" a. ".as_ref()));
    assert!(!is_valid_filename("a/".as_ref()));
    assert!(!is_valid_filename("/a".as_ref()));
    assert!(!is_valid_filename("/".as_ref()));
    assert!(!is_valid_filename("\\".as_ref()));
    assert!(!is_valid_filename("\\a".as_ref()));
    assert!(!is_valid_filename("<x>".as_ref()));
    assert!(!is_valid_filename("a*".as_ref()));
    assert!(!is_valid_filename("?x".as_ref()));
    assert!(!is_valid_filename("a\0a".as_ref()));
    assert!(!is_valid_filename("\x1f".as_ref()));
    assert!(!is_valid_filename(::std::iter::repeat("a").take(257).collect::<String>().as_ref()));

    assert!(is_valid_filename(::std::iter::repeat("®").take(254).collect::<String>().as_ref()));
    assert!(is_valid_filename("ファイル".as_ref()));
    assert!(is_valid_filename("a".as_ref()));
    assert!(is_valid_filename("a.aaaaaaaa".as_ref()));
    assert!(is_valid_filename("a........a".as_ref()));
    assert!(is_valid_filename("       b".as_ref()));
}

#[test]
#[cfg(windows)]
fn realpath_test() {
    assert_eq!(r"C:\WINDOWS", canonicalize(r"C:\Windows").unwrap().to_str().unwrap().to_uppercase());
    assert_ne!(r".", canonicalize(r".").unwrap().to_str().unwrap());
}

#[test]
#[cfg(windows)]
fn strip() {
    assert_eq!(Path::new(r"C:\foo\😀"), simplified(Path::new(r"\\?\C:\foo\😀")));
    assert_eq!(Path::new(r"\\?\serv\"), simplified(Path::new(r"\\?\serv\")));
    assert_eq!(Path::new(r"\\.\C:\notdisk"), simplified(Path::new(r"\\.\C:\notdisk")));
    assert_eq!(Path::new(r"\\?\GLOBALROOT\Device\ImDisk0\path\to\file.txt"), simplified(Path::new(r"\\?\GLOBALROOT\Device\ImDisk0\path\to\file.txt")));
}

#[test]
#[cfg(windows)]
fn safe() {
    assert!(is_safe_to_strip_unc(Path::new(r"\\?\C:\foo\bar")));
    assert!(is_safe_to_strip_unc(Path::new(r"\\?\Z:\foo\bar\")));
    assert!(is_safe_to_strip_unc(Path::new(r"\\?\Z:\😀\🎃\")));
    assert!(is_safe_to_strip_unc(Path::new(r"\\?\c:\foo")));

    let long = ::std::iter::repeat("®").take(160).collect::<String>();
    assert!(is_safe_to_strip_unc(Path::new(&format!(r"\\?\c:\{}", long))));
    assert!(!is_safe_to_strip_unc(Path::new(&format!(r"\\?\c:\{}\{}", long, long))));

    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\C:\foo\.\bar")));
    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\C:\foo\..\bar")));
    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\c\foo")));
    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\c\foo/bar")));
    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\c:foo")));
    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\cc:foo")));
    assert!(!is_safe_to_strip_unc(Path::new(r"\\?\c:foo\bar")));
}
