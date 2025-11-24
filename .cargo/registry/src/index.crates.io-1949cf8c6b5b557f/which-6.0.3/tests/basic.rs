extern crate which;

#[cfg(all(unix, feature = "regex"))]
use regex::Regex;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::{env, vec};
use tempfile::TempDir;

#[derive(Debug)]
struct TestFixture {
    /// Temp directory.
    pub tempdir: TempDir,
    /// $PATH
    pub paths: OsString,
    /// Binaries created in $PATH
    pub bins: Vec<PathBuf>,
}

const SUBDIRS: &[&str] = &["a", "b", "c"];
const BIN_NAME: &str = "bin";

#[allow(clippy::unnecessary_cast)]
#[cfg(unix)]
fn mk_bin(dir: &Path, path: &str, extension: &str) -> io::Result<PathBuf> {
    use std::os::unix::fs::OpenOptionsExt;
    let bin = dir.join(path).with_extension(extension);

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let mode = rustix::fs::Mode::XUSR.bits() as u32;
    let mode = 0o666 | mode;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .mode(mode)
        .open(&bin)
        .and_then(|_f| bin.canonicalize())
}

fn touch(dir: &Path, path: &str, extension: &str) -> io::Result<PathBuf> {
    let b = dir.join(path).with_extension(extension);
    fs::File::create(&b).and_then(|_f| b.canonicalize())
}

#[cfg(windows)]
fn mk_bin(dir: &Path, path: &str, extension: &str) -> io::Result<PathBuf> {
    touch(dir, path, extension)
}

impl TestFixture {
    // tmp/a/bin
    // tmp/a/bin.exe
    // tmp/a/bin.cmd
    // tmp/b/bin
    // tmp/b/bin.exe
    // tmp/b/bin.cmd
    // tmp/c/bin
    // tmp/c/bin.exe
    // tmp/c/bin.cmd
    pub fn new() -> TestFixture {
        let tempdir = tempfile::tempdir().unwrap();
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        let mut paths = vec![];
        let mut bins = vec![];
        for d in SUBDIRS.iter() {
            let p = tempdir.path().join(d);
            builder.create(&p).unwrap();
            bins.push(mk_bin(&p, BIN_NAME, "").unwrap());
            bins.push(mk_bin(&p, BIN_NAME, "exe").unwrap());
            bins.push(mk_bin(&p, BIN_NAME, "cmd").unwrap());
            paths.push(p);
        }
        let p = tempdir.path().join("win-bin");
        builder.create(&p).unwrap();
        bins.push(mk_bin(&p, "win-bin", "exe").unwrap());
        paths.push(p);
        TestFixture {
            tempdir,
            paths: env::join_paths(paths).unwrap(),
            bins,
        }
    }

    #[cfg(unix)]
    pub fn new_with_tilde_path() -> TestFixture {
        let tempdir = tempfile::tempdir().unwrap();
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        let mut paths = vec![];
        let mut bins = vec![];
        for d in SUBDIRS.iter() {
            let p = PathBuf::from("~").join(d);
            let p_bin = tempdir.path().join(d);
            builder.create(&p_bin).unwrap();
            bins.push(mk_bin(&p_bin, BIN_NAME, "").unwrap());
            bins.push(mk_bin(&p_bin, BIN_NAME, "exe").unwrap());
            bins.push(mk_bin(&p_bin, BIN_NAME, "cmd").unwrap());
            paths.push(p);
        }
        let p = tempdir.path().join("win-bin");
        builder.create(&p).unwrap();
        bins.push(mk_bin(&p, "win-bin", "exe").unwrap());
        paths.push(p);
        TestFixture {
            tempdir,
            paths: env::join_paths(paths).unwrap(),
            bins,
        }
    }

    #[allow(dead_code)]
    pub fn touch(&self, path: &str, extension: &str) -> io::Result<PathBuf> {
        touch(self.tempdir.path(), path, extension)
    }

    pub fn mk_bin(&self, path: &str, extension: &str) -> io::Result<PathBuf> {
        mk_bin(self.tempdir.path(), path, extension)
    }
}

fn _which<T: AsRef<OsStr>>(f: &TestFixture, path: T) -> which::Result<which::CanonicalPath> {
    which::CanonicalPath::new_in(path, Some(f.paths.clone()), f.tempdir.path())
}

fn _which_all<'a, T: AsRef<OsStr> + 'a>(
    f: &'a TestFixture,
    path: T,
) -> which::Result<impl Iterator<Item = which::Result<which::CanonicalPath>> + '_> {
    which::CanonicalPath::all_in(path, Some(f.paths.clone()), f.tempdir.path())
}

#[test]
#[cfg(unix)]
fn it_works() {
    use std::process::Command;
    let result = which::Path::new("rustc");
    assert!(result.is_ok());

    let which_result = Command::new("which").arg("rustc").output();

    assert_eq!(
        String::from(result.unwrap().to_str().unwrap()),
        String::from_utf8(which_result.unwrap().stdout)
            .unwrap()
            .trim()
    );
}

#[test]
#[cfg(unix)]
fn test_which() {
    let f = TestFixture::new();
    assert_eq!(_which(&f, BIN_NAME).unwrap(), f.bins[0])
}

#[test]
#[cfg(windows)]
fn test_which() {
    let f = TestFixture::new();
    assert_eq!(_which(&f, BIN_NAME).unwrap(), f.bins[1])
}

#[test]
#[cfg(unix)]
fn test_which_tilde() {
    let old_home = env::var_os("HOME");
    let f = TestFixture::new_with_tilde_path();
    env::set_var("HOME", f.tempdir.path().as_os_str());
    assert_eq!(_which(&f, BIN_NAME).unwrap(), f.bins[0]);
    if let Some(old_home) = old_home {
        env::set_var("HOME", old_home);
    } else {
        env::remove_var("HOME");
    }
}

// Windows test_which_tilde intentionally omitted because
// we don't want to pollute the home directory.
// It's non-trivial to adjust which directory Windows thinks
// is the home directory. At this time, tilde expansion has
// no Windows specific behavior. It works as normal on Windows.

#[test]
#[cfg(all(unix, feature = "regex"))]
fn test_which_re_in_with_matches() {
    let f = TestFixture::new();
    f.mk_bin("a/bin_0", "").unwrap();
    f.mk_bin("b/bin_1", "").unwrap();
    let re = Regex::new(r"bin_\d").unwrap();

    let result: Vec<PathBuf> = which::which_re_in(re, Some(f.paths)).unwrap().collect();

    let temp = f.tempdir;

    assert_eq!(
        result,
        vec![temp.path().join("a/bin_0"), temp.path().join("b/bin_1")]
    )
}

#[test]
#[cfg(all(unix, feature = "regex"))]
fn test_which_re_in_without_matches() {
    let f = TestFixture::new();
    let re = Regex::new(r"bi[^n]").unwrap();

    let result: Vec<PathBuf> = which::which_re_in(re, Some(f.paths)).unwrap().collect();

    assert_eq!(result, Vec::<PathBuf>::new())
}

#[test]
#[cfg(all(unix, feature = "regex"))]
fn test_which_re_accepts_owned_and_borrow() {
    which::which_re(Regex::new(r".").unwrap())
        .unwrap()
        .for_each(drop);
    which::which_re(&Regex::new(r".").unwrap())
        .unwrap()
        .for_each(drop);
    which::which_re_in(Regex::new(r".").unwrap(), Some("pth"))
        .unwrap()
        .for_each(drop);
    which::which_re_in(&Regex::new(r".").unwrap(), Some("pth"))
        .unwrap()
        .for_each(drop);
}

#[test]
#[cfg(unix)]
fn test_which_extension() {
    let f = TestFixture::new();
    let b = Path::new(&BIN_NAME).with_extension("");
    assert_eq!(_which(&f, b).unwrap(), f.bins[0])
}

#[test]
#[cfg(windows)]
fn test_which_extension() {
    let f = TestFixture::new();
    let b = Path::new(&BIN_NAME).with_extension("cmd");
    assert_eq!(_which(&f, b).unwrap(), f.bins[2])
}

#[test]
#[cfg(windows)]
fn test_which_no_extension() {
    let f = TestFixture::new();
    let b = Path::new("win-bin");
    let which_result = which::which_in(b, Some(&f.paths), ".").unwrap();
    // Make sure the extension is the correct case.
    assert_eq!(which_result.extension(), f.bins[9].extension());
    assert_eq!(fs::canonicalize(&which_result).unwrap(), f.bins[9])
}

#[test]
fn test_which_not_found() {
    let f = TestFixture::new();
    assert!(_which(&f, "a").is_err());
}

#[test]
fn test_which_second() {
    let f = TestFixture::new();
    let b = f.mk_bin("b/another", env::consts::EXE_EXTENSION).unwrap();
    assert_eq!(_which(&f, "another").unwrap(), b);
}

#[test]
fn test_which_all() {
    let f = TestFixture::new();
    let actual = _which_all(&f, BIN_NAME)
        .unwrap()
        .map(|c| c.unwrap())
        .collect::<Vec<_>>();
    let mut expected = f
        .bins
        .iter()
        .map(|p| p.canonicalize().unwrap())
        .collect::<Vec<_>>();
    #[cfg(windows)]
    {
        expected.retain(|p| p.file_stem().unwrap() == BIN_NAME);
        expected.retain(|p| p.extension().map(|ext| ext == "exe" || ext == "cmd") == Some(true));
    }
    #[cfg(not(windows))]
    {
        expected.retain(|p| p.file_name().unwrap() == BIN_NAME);
    }
    assert_eq!(actual, expected);
}

#[test]
#[cfg(unix)]
fn test_which_absolute() {
    let f = TestFixture::new();
    assert_eq!(
        _which(&f, &f.bins[3]).unwrap(),
        f.bins[3].canonicalize().unwrap()
    );
}

#[test]
#[cfg(windows)]
fn test_which_absolute() {
    let f = TestFixture::new();
    assert_eq!(
        _which(&f, &f.bins[4]).unwrap(),
        f.bins[4].canonicalize().unwrap()
    );
}

#[test]
#[cfg(windows)]
fn test_which_absolute_path_case() {
    // Test that an absolute path with an uppercase extension
    // is accepted.
    let f = TestFixture::new();
    let p = &f.bins[4];
    assert_eq!(_which(&f, p).unwrap(), f.bins[4].canonicalize().unwrap());
}

#[test]
#[cfg(unix)]
fn test_which_absolute_extension() {
    let f = TestFixture::new();
    // Don't append EXE_EXTENSION here.
    let b = f.bins[3].parent().unwrap().join(BIN_NAME);
    assert_eq!(_which(&f, b).unwrap(), f.bins[3].canonicalize().unwrap());
}

#[test]
#[cfg(windows)]
fn test_which_absolute_extension() {
    let f = TestFixture::new();
    // Don't append EXE_EXTENSION here.
    let b = f.bins[4].parent().unwrap().join(BIN_NAME);
    assert_eq!(_which(&f, b).unwrap(), f.bins[4].canonicalize().unwrap());
}

#[test]
#[cfg(unix)]
fn test_which_relative() {
    let f = TestFixture::new();
    assert_eq!(
        _which(&f, "b/bin").unwrap(),
        f.bins[3].canonicalize().unwrap()
    );
}

#[test]
#[cfg(windows)]
fn test_which_relative() {
    let f = TestFixture::new();
    assert_eq!(
        _which(&f, "b/bin").unwrap(),
        f.bins[4].canonicalize().unwrap()
    );
}

#[test]
#[cfg(unix)]
fn test_which_relative_extension() {
    // test_which_relative tests a relative path without an extension,
    // so test a relative path with an extension here.
    let f = TestFixture::new();
    let b = Path::new("b/bin").with_extension(env::consts::EXE_EXTENSION);
    assert_eq!(_which(&f, b).unwrap(), f.bins[3].canonicalize().unwrap());
}

#[test]
#[cfg(windows)]
fn test_which_relative_extension() {
    // test_which_relative tests a relative path without an extension,
    // so test a relative path with an extension here.
    let f = TestFixture::new();
    let b = Path::new("b/bin").with_extension("cmd");
    assert_eq!(_which(&f, b).unwrap(), f.bins[5].canonicalize().unwrap());
}

#[test]
#[cfg(windows)]
fn test_which_relative_extension_case() {
    // Test that a relative path with an uppercase extension
    // is accepted.
    let f = TestFixture::new();
    let b = Path::new("b/bin").with_extension("EXE");
    assert_eq!(_which(&f, b).unwrap(), f.bins[4].canonicalize().unwrap());
}

#[test]
#[cfg(unix)]
fn test_which_relative_leading_dot() {
    let f = TestFixture::new();
    assert_eq!(
        _which(&f, "./b/bin").unwrap(),
        f.bins[3].canonicalize().unwrap()
    );
}

#[test]
#[cfg(windows)]
fn test_which_relative_leading_dot() {
    let f = TestFixture::new();
    assert_eq!(
        _which(&f, "./b/bin").unwrap(),
        f.bins[4].canonicalize().unwrap()
    );
}

#[test]
#[cfg(unix)]
fn test_which_non_executable() {
    // Shouldn't return non-executable files.
    let f = TestFixture::new();
    f.touch("b/another", "").unwrap();
    assert!(_which(&f, "another").is_err());
}

#[test]
#[cfg(unix)]
fn test_which_absolute_non_executable() {
    // Shouldn't return non-executable files, even if given an absolute path.
    let f = TestFixture::new();
    let b = f.touch("b/another", "").unwrap();
    assert!(_which(&f, b).is_err());
}

#[test]
#[cfg(unix)]
fn test_which_relative_non_executable() {
    // Shouldn't return non-executable files.
    let f = TestFixture::new();
    f.touch("b/another", "").unwrap();
    assert!(_which(&f, "b/another").is_err());
}

#[test]
fn test_failure() {
    let f = TestFixture::new();

    let run = || -> which::Result<PathBuf> {
        let p = _which(&f, "./b/bin")?;
        Ok(p.into_path_buf())
    };

    let _ = run();
}

#[test]
#[cfg(windows)]
fn windows_no_extension_but_executable() {
    let this_executable = std::env::current_exe().unwrap();
    let new_name = this_executable.parent().unwrap().join("test_executable");
    std::fs::copy(&this_executable, &new_name).unwrap();
    let found_executable = which::which_in_global(
        new_name.file_name().unwrap(),
        Some(this_executable.parent().unwrap()),
    )
    .unwrap()
    .next()
    .unwrap();
    assert_eq!(found_executable, new_name);
    std::fs::remove_file(new_name).unwrap();
}
