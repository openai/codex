use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::str;

use option_ext::OptionExt;

/// Returns all XDG user directories obtained from $(XDG_CONFIG_HOME)/user-dirs.dirs.
pub fn all(home_dir_path: &Path, user_dir_file_path: &Path) -> HashMap<String, PathBuf> {
    let bytes = read_all(user_dir_file_path).unwrap_or(Vec::new());
    parse_user_dirs(home_dir_path, None, &bytes)
}

/// Returns a single XDG user directory obtained from $(XDG_CONFIG_HOME)/user-dirs.dirs.
pub fn single(home_dir_path: &Path, user_dir_file_path: &Path, user_dir_name: &str) -> HashMap<String, PathBuf> {
    let bytes = read_all(user_dir_file_path).unwrap_or(Vec::new());
    parse_user_dirs(home_dir_path, Some(user_dir_name), &bytes)
}

fn parse_user_dirs(home_dir: &Path, user_dir: Option<&str>, bytes: &[u8]) -> HashMap<String, PathBuf> {
    let mut user_dirs = HashMap::new();

    for line in bytes.split(|b| *b == b'\n') {
        let mut single_dir_found = false;
        let (key, value) = match split_once(line, b'=') {
            Some(kv) => kv,
            None => continue,
        };

        let key = trim_blank(key);
        let key = if key.starts_with(b"XDG_") && key.ends_with(b"_DIR") {
            match str::from_utf8(&key[4..key.len()-4]) {
                Ok(key) =>
                    if user_dir.contains(&key) {
                        single_dir_found = true;
                        key
                    } else if user_dir.is_none() {
                        key
                    } else {
                        continue
                    },
                Err(_)  => continue,
            }
        } else {
            continue
        };

        // xdg-user-dirs-update uses double quotes and we don't support anything else.
        let value = trim_blank(value);
        let mut value = if value.starts_with(b"\"") && value.ends_with(b"\"") {
            &value[1..value.len()-1]
        } else {
            continue
        };

        // Path should be either relative to the home directory or absolute.
        let is_relative = if value == b"$HOME/" {
            // "Note: To disable a directory, point it to the homedir."
            // Source: https://www.freedesktop.org/wiki/Software/xdg-user-dirs/
            // Additionally directory is reassigned to homedir when removed.
            continue
        } else if value.starts_with(b"$HOME/") {
            value = &value[b"$HOME/".len()..];
            true
        } else if value.starts_with(b"/") {
            false
        } else {
            continue
        };

        let value = OsString::from_vec(shell_unescape(value));

        let path = if is_relative {
            let mut path = PathBuf::from(&home_dir);
            path.push(value);
            path
        } else {
            PathBuf::from(value)
        };

        user_dirs.insert(key.to_owned(), path);
        if single_dir_found {
            break;
        }
    }

    user_dirs
}

/// Reads the entire contents of a file into a byte vector.
fn read_all(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::with_capacity(1024);
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Returns bytes before and after first occurrence of separator.
fn split_once(bytes: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    bytes.iter().position(|b| *b == separator).map(|i| {
        (&bytes[..i], &bytes[i+1..])
    })
}

/// Returns a slice with leading and trailing <blank> characters removed.
fn trim_blank(bytes: &[u8]) -> &[u8] {
    // Trim leading <blank> characters.
    let i = bytes.iter().cloned().take_while(|b| *b == b' ' || *b == b'\t').count();
    let bytes = &bytes[i..];

    // Trim trailing <blank> characters.
    let i = bytes.iter().cloned().rev().take_while(|b| *b == b' ' || *b == b'\t').count();
    &bytes[..bytes.len()-i]
}

/// Unescape bytes escaped with POSIX shell double-quotes rules (as used by xdg-user-dirs-update).
fn shell_unescape(escaped: &[u8]) -> Vec<u8> {
    // We assume that byte string was created by xdg-user-dirs-update which
    // escapes all characters that might potentially have special meaning,
    // so there is no need to check if backslash is actually followed by
    // $ ` " \ or a <newline>.

    let mut unescaped: Vec<u8> = Vec::with_capacity(escaped.len());
    let mut i = escaped.iter().cloned();

    while let Some(b) = i.next() {
        if b == b'\\' {
            if let Some(b) = i.next() {
                unescaped.push(b);
            }
        } else {
            unescaped.push(b);
        }
    }

    unescaped
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use super::{parse_user_dirs, shell_unescape, split_once, trim_blank};

    #[test]
    fn test_trim_blank() {
        assert_eq!(b"x", trim_blank(b"x"));
        assert_eq!(b"", trim_blank(b" \t  "));
        assert_eq!(b"hello there", trim_blank(b" \t hello there \t "));
        assert_eq!(b"\r\n", trim_blank(b"\r\n"));
    }

    #[test]
    fn test_split_once() {
        assert_eq!(None, split_once(b"a b c", b'='));
        assert_eq!(Some((b"before".as_ref(), b"after".as_ref())), split_once(b"before=after", b'='));
    }

    #[test]
    fn test_shell_unescape() {
        assert_eq!(b"abc", shell_unescape(b"abc").as_slice());
        assert_eq!(b"x\\y$z`", shell_unescape(b"x\\\\y\\$z\\`").as_slice());
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(HashMap::new(), parse_user_dirs(Path::new("/root/"), None, b""));
        assert_eq!(HashMap::new(), parse_user_dirs(Path::new("/root/"), Some("MUSIC"), b""));
    }

    #[test]
    fn test_absolute_path_is_accepted() {
        let mut dirs = HashMap::new();
        dirs.insert("MUSIC".to_owned(), PathBuf::from("/media/music"));
        let bytes = br#"XDG_MUSIC_DIR="/media/music""#;
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), None, bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), Some("MUSIC"), bytes));
    }

    #[test]
    fn test_relative_path_is_rejected() {
        let dirs = HashMap::new();
        let bytes = br#"XDG_MUSIC_DIR="music""#;
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), None, bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), Some("MUSIC"), bytes));
    }

    #[test]
    fn test_relative_to_home() {
        let mut dirs = HashMap::new();
        dirs.insert("MUSIC".to_owned(), PathBuf::from("/home/john/Music"));
        let bytes = br#"XDG_MUSIC_DIR="$HOME/Music""#;
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), None, bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), Some("MUSIC"), bytes));
    }

    #[test]
    fn test_disabled_directory() {
        let dirs = HashMap::new();
        let bytes = br#"XDG_MUSIC_DIR="$HOME/""#;
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), None, bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/john"), Some("MUSIC"), bytes));
    }

    #[test]
    fn test_parse_user_dirs() {
        let mut dirs: HashMap<String, PathBuf> = HashMap::new();
        dirs.insert("DESKTOP".to_string(), PathBuf::from("/home/bob/Desktop"));
        dirs.insert("DOWNLOAD".to_string(), PathBuf::from("/home/bob/Downloads"));
        dirs.insert("PICTURES".to_string(), PathBuf::from("/home/eve/pics"));

        let bytes = br#"
# This file is written by xdg-user-dirs-update
# If you want to change or add directories, just edit the line you're
# interested in. All local changes will be retained on the next run.
# Format is XDG_xxx_DIR="$HOME/yyy", where yyy is a shell-escaped
# homedir-relative path, or XDG_xxx_DIR="/yyy", where /yyy is an
# absolute path. No other format is supported.
XDG_DESKTOP_DIR="$HOME/Desktop"
XDG_DOWNLOAD_DIR="$HOME/Downloads"
XDG_TEMPLATES_DIR=""
XDG_PUBLICSHARE_DIR="$HOME"
XDG_DOCUMENTS_DIR="$HOME/"
XDG_PICTURES_DIR="/home/eve/pics"
XDG_VIDEOS_DIR="$HOxyzME/Videos"
"#;

        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), None, bytes));

        let mut dirs: HashMap<String, PathBuf> = HashMap::new();
        dirs.insert("DESKTOP".to_string(), PathBuf::from("/home/bob/Desktop"));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), Some("DESKTOP"), bytes));

        let mut dirs: HashMap<String, PathBuf> = HashMap::new();
        dirs.insert("PICTURES".to_string(), PathBuf::from("/home/eve/pics"));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), Some("PICTURES"), bytes));

        let dirs: HashMap<String, PathBuf> = HashMap::new();
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), Some("TEMPLATES"), bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), Some("PUBLICSHARE"), bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), Some("DOCUMENTS"), bytes));
        assert_eq!(dirs, parse_user_dirs(Path::new("/home/bob"), Some("VIDEOS"), bytes));
    }
}
