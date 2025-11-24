#![deny(rust_2018_idioms)]

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tempfile::{env, tempdir, Builder, NamedTempFile, TempPath};

fn exists<P: AsRef<Path>>(path: P) -> bool {
    std::fs::metadata(path.as_ref()).is_ok()
}

/// For the wasi platforms, `std::env::temp_dir` will panic. For those targets, configure the /tmp
/// directory instead as the base directory for temp files.
fn configure_wasi_temp_dir() {
    if cfg!(target_os = "wasi") {
        let _ = tempfile::env::override_temp_dir(Path::new("/tmp"));
    }
}

#[test]
fn test_prefix() {
    configure_wasi_temp_dir();

    let tmpfile = NamedTempFile::with_prefix("prefix").unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("prefix"));
}

#[test]
fn test_suffix() {
    configure_wasi_temp_dir();

    let tmpfile = NamedTempFile::with_suffix("suffix").unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.ends_with("suffix"));
}

#[test]
fn test_basic() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    write!(tmpfile, "abcde").unwrap();
    tmpfile.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = String::new();
    tmpfile.read_to_string(&mut buf).unwrap();
    assert_eq!("abcde", buf);
}

#[test]
fn test_deleted() {
    configure_wasi_temp_dir();

    let tmpfile = NamedTempFile::new().unwrap();
    let path = tmpfile.path().to_path_buf();
    assert!(exists(&path));
    drop(tmpfile);
    assert!(!exists(&path));
}

#[test]
fn test_persist() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    let old_path = tmpfile.path().to_path_buf();
    let persist_path = env::temp_dir().join("persisted_temporary_file");
    write!(tmpfile, "abcde").unwrap();
    {
        assert!(exists(&old_path));
        let mut f = tmpfile.persist(&persist_path).unwrap();
        assert!(!exists(&old_path));

        // Check original file
        f.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        assert_eq!("abcde", buf);
    }

    {
        // Try opening it at the new path.
        let mut f = File::open(&persist_path).unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        assert_eq!("abcde", buf);
    }
    std::fs::remove_file(&persist_path).unwrap();
}

#[test]
fn test_persist_noclobber() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    let old_path = tmpfile.path().to_path_buf();
    let persist_target = NamedTempFile::new().unwrap();
    let persist_path = persist_target.path().to_path_buf();
    write!(tmpfile, "abcde").unwrap();
    assert!(exists(&old_path));
    {
        tmpfile = tmpfile.persist_noclobber(&persist_path).unwrap_err().into();
        assert!(exists(&old_path));
        std::fs::remove_file(&persist_path).unwrap();
        drop(persist_target);
    }
    tmpfile.persist_noclobber(&persist_path).unwrap();
    // Try opening it at the new path.
    let mut f = File::open(&persist_path).unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();
    assert_eq!("abcde", buf);
    std::fs::remove_file(&persist_path).unwrap();
}

#[test]
fn test_customnamed() {
    configure_wasi_temp_dir();

    let tmpfile = Builder::new()
        .prefix("tmp")
        .suffix(&".rs")
        .rand_bytes(12)
        .tempfile()
        .unwrap();
    let name = tmpfile.path().file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("tmp"));
    assert!(name.ends_with(".rs"));
    assert_eq!(name.len(), 18);
}

#[test]
fn test_append() {
    configure_wasi_temp_dir();

    let mut tmpfile = Builder::new().append(true).tempfile().unwrap();
    tmpfile.write_all(b"a").unwrap();
    tmpfile.seek(SeekFrom::Start(0)).unwrap();
    tmpfile.write_all(b"b").unwrap();

    tmpfile.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = vec![0u8; 1];
    tmpfile.read_exact(&mut buf).unwrap();
    assert_eq!(buf, b"a");
}

#[test]
fn test_reopen() {
    configure_wasi_temp_dir();

    let source = NamedTempFile::new().unwrap();
    let mut first = source.reopen().unwrap();
    let mut second = source.reopen().unwrap();
    drop(source);

    write!(first, "abcde").expect("write failed");
    let mut buf = String::new();
    second.read_to_string(&mut buf).unwrap();
    assert_eq!("abcde", buf);
}

#[test]
fn test_into_file() {
    configure_wasi_temp_dir();

    let mut file = NamedTempFile::new().unwrap();
    let path = file.path().to_owned();
    write!(file, "abcde").expect("write failed");

    assert!(path.exists());
    let mut file = file.into_file();
    assert!(!path.exists());

    file.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();
    assert_eq!("abcde", buf);
}

#[test]
fn test_immut() {
    configure_wasi_temp_dir();

    let tmpfile = NamedTempFile::new().unwrap();
    (&tmpfile).write_all(b"abcde").unwrap();
    (&tmpfile).seek(SeekFrom::Start(0)).unwrap();
    let mut buf = String::new();
    (&tmpfile).read_to_string(&mut buf).unwrap();
    assert_eq!("abcde", buf);
}

#[test]
fn test_temppath() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    write!(tmpfile, "abcde").unwrap();

    let path = tmpfile.into_temp_path();
    assert!(path.is_file());
}

#[test]
fn test_temppath_persist() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    write!(tmpfile, "abcde").unwrap();

    let tmppath = tmpfile.into_temp_path();

    let old_path = tmppath.to_path_buf();
    let persist_path = env::temp_dir().join("persisted_temppath_file");

    {
        assert!(exists(&old_path));
        tmppath.persist(&persist_path).unwrap();
        assert!(!exists(&old_path));
    }

    {
        // Try opening it at the new path.
        let mut f = File::open(&persist_path).unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        assert_eq!("abcde", buf);
    }

    std::fs::remove_file(&persist_path).unwrap();
}

#[test]
fn test_temppath_persist_noclobber() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    write!(tmpfile, "abcde").unwrap();

    let mut tmppath = tmpfile.into_temp_path();

    let old_path = tmppath.to_path_buf();
    let persist_target = NamedTempFile::new().unwrap();
    let persist_path = persist_target.path().to_path_buf();

    assert!(exists(&old_path));

    {
        tmppath = tmppath.persist_noclobber(&persist_path).unwrap_err().into();
        assert!(exists(&old_path));
        std::fs::remove_file(&persist_path).unwrap();
        drop(persist_target);
    }

    tmppath.persist_noclobber(&persist_path).unwrap();

    // Try opening it at the new path.
    let mut f = File::open(&persist_path).unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();
    assert_eq!("abcde", buf);
    std::fs::remove_file(&persist_path).unwrap();
}

#[test]
fn temp_path_from_existing() {
    configure_wasi_temp_dir();

    let tmp_dir = tempdir().unwrap();
    let tmp_file_path_1 = tmp_dir.path().join("testfile1");
    let tmp_file_path_2 = tmp_dir.path().join("testfile2");

    File::create(&tmp_file_path_1).unwrap();
    assert!(tmp_file_path_1.exists(), "Test file 1 hasn't been created");

    File::create(&tmp_file_path_2).unwrap();
    assert!(tmp_file_path_2.exists(), "Test file 2 hasn't been created");

    let tmp_path = TempPath::from_path(&tmp_file_path_1);
    assert!(
        tmp_file_path_1.exists(),
        "Test file has been deleted before dropping TempPath"
    );

    drop(tmp_path);
    assert!(
        !tmp_file_path_1.exists(),
        "Test file exists after dropping TempPath"
    );
    assert!(
        tmp_file_path_2.exists(),
        "Test file 2 has been deleted before dropping TempDir"
    );
}

#[test]
#[allow(unreachable_code)]
fn temp_path_from_argument_types() {
    // This just has to compile
    return;

    TempPath::from_path("");
    TempPath::from_path(String::new());
    TempPath::from_path(OsStr::new(""));
    TempPath::from_path(OsString::new());
    TempPath::from_path(Path::new(""));
    TempPath::from_path(PathBuf::new());
    TempPath::from_path(PathBuf::new().into_boxed_path());
}

#[test]
fn test_write_after_close() {
    configure_wasi_temp_dir();

    let path = NamedTempFile::new().unwrap().into_temp_path();
    File::create(path).unwrap().write_all(b"test").unwrap();
}

#[test]
fn test_change_dir() {
    configure_wasi_temp_dir();

    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    std::env::set_current_dir(&dir_a).expect("failed to change to directory ~");
    let tmpfile = NamedTempFile::new_in(".").unwrap();
    let path = std::env::current_dir().unwrap().join(tmpfile.path());
    std::env::set_current_dir(&dir_b).expect("failed to change to directory B");
    drop(tmpfile);
    assert!(!exists(path));

    drop(dir_a);
    drop(dir_b);
}

#[test]
fn test_change_dir_make() {
    configure_wasi_temp_dir();

    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    std::env::set_current_dir(&dir_a).expect("failed to change to directory A");
    let tmpfile = Builder::new().make_in(".", |p| File::create(p)).unwrap();
    let path = std::env::current_dir().unwrap().join(tmpfile.path());
    std::env::set_current_dir(&dir_b).expect("failed to change to directory B");
    drop(tmpfile);
    assert!(!exists(path));

    drop(dir_a);
    drop(dir_b);
}

#[test]
fn test_into_parts() {
    configure_wasi_temp_dir();

    let mut file = NamedTempFile::new().unwrap();
    write!(file, "abcd").expect("write failed");

    let (mut file, temp_path) = file.into_parts();

    let path = temp_path.to_path_buf();

    assert!(path.exists());
    drop(temp_path);
    assert!(!path.exists());

    write!(file, "efgh").expect("write failed");

    file.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();
    assert_eq!("abcdefgh", buf);
}

#[test]
fn test_from_parts() {
    configure_wasi_temp_dir();

    let mut file = NamedTempFile::new().unwrap();
    write!(file, "abcd").expect("write failed");

    let (file, temp_path) = file.into_parts();

    let file = NamedTempFile::from_parts(file, temp_path);

    assert!(file.path().exists());
}

#[test]
fn test_keep() {
    configure_wasi_temp_dir();

    let mut tmpfile = NamedTempFile::new().unwrap();
    write!(tmpfile, "abcde").unwrap();
    let (mut f, temp_path) = tmpfile.into_parts();
    let path;
    {
        assert!(exists(&temp_path));
        path = temp_path.keep().unwrap();
        assert!(exists(&path));

        // Check original file
        f.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        assert_eq!("abcde", buf);
    }

    {
        // Try opening it again.
        let mut f = File::open(&path).unwrap();
        f.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        assert_eq!("abcde", buf);
    }
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn test_disable_cleanup() {
    configure_wasi_temp_dir();

    // Case 0: never mark as "disable cleanup"
    // Case 1: enable "disable cleanup" in the builder, don't touch it after.
    // Case 2: enable "disable cleanup" in the builder, turn it off after.
    // Case 3: don't enable disable cleanup in the builder, turn it on after.

    for case in 0..4 {
        let in_builder = case & 1 > 0;
        let toggle = case & 2 > 0;
        let mut tmpfile = Builder::new()
            .disable_cleanup(in_builder)
            .tempfile()
            .unwrap();
        write!(tmpfile, "abcde").unwrap();
        if toggle {
            tmpfile.disable_cleanup(!in_builder);
        }

        let path = tmpfile.path().to_owned();
        drop(tmpfile);

        if in_builder ^ toggle {
            // Try opening it again.
            let mut f = File::open(&path).unwrap();
            let mut buf = String::new();
            f.read_to_string(&mut buf).unwrap();
            assert_eq!("abcde", buf);
            std::fs::remove_file(&path).unwrap();
        } else {
            assert!(!path.exists(), "tempfile wasn't deleted");
        }
    }
}

#[test]
fn test_make() {
    configure_wasi_temp_dir();

    let tmpfile = Builder::new().make(|path| File::create(path)).unwrap();

    assert!(tmpfile.path().is_file());
}

#[test]
fn test_make_in() {
    configure_wasi_temp_dir();

    let tmp_dir = tempdir().unwrap();

    let tmpfile = Builder::new()
        .make_in(tmp_dir.path(), |path| File::create(path))
        .unwrap();

    assert!(tmpfile.path().is_file());
    assert_eq!(tmpfile.path().parent(), Some(tmp_dir.path()));
}

#[test]
fn test_make_fnmut() {
    configure_wasi_temp_dir();

    let mut count = 0;

    // Show that an FnMut can be used.
    let tmpfile = Builder::new()
        .make(|path| {
            count += 1;
            File::create(path)
        })
        .unwrap();

    assert!(tmpfile.path().is_file());
}

#[cfg(unix)]
#[test]
fn test_make_uds() {
    use std::os::unix::net::UnixListener;

    let temp_sock = Builder::new()
        .prefix("tmp")
        .suffix(".sock")
        .rand_bytes(12)
        .make(|path| UnixListener::bind(path))
        .unwrap();

    assert!(temp_sock.path().exists());
}

#[cfg(unix)]
#[test]
fn test_make_uds_conflict() {
    use std::io::ErrorKind;
    use std::os::unix::net::UnixListener;

    let sockets = std::iter::repeat_with(|| {
        Builder::new()
            .prefix("tmp")
            .suffix(".sock")
            .rand_bytes(1)
            .make(|path| UnixListener::bind(path))
    })
    .take_while(|r| match r {
        Ok(_) => true,
        Err(e) if matches!(e.kind(), ErrorKind::AddrInUse | ErrorKind::AlreadyExists) => false,
        Err(e) => panic!("unexpected error {e}"),
    })
    .collect::<Result<Vec<_>, _>>()
    .unwrap();

    // Number of sockets we can create. Depends on whether or not the filesystem is case sensitive.

    #[cfg(target_os = "macos")]
    const NUM_FILES: usize = 36;
    #[cfg(not(target_os = "macos"))]
    const NUM_FILES: usize = 62;

    assert_eq!(sockets.len(), NUM_FILES);

    for socket in sockets {
        assert!(socket.path().exists());
    }
}

/// Make sure we re-seed with system randomness if we run into a conflict.
#[test]
fn test_reseed() {
    configure_wasi_temp_dir();

    // Deterministic seed.
    fastrand::seed(42);

    // I need to create 5 conflicts but I can't just make 5 temporary files because we fork the RNG
    // each time we create a file.
    let mut attempts = 0;
    let mut files: Vec<_> = Vec::new();
    let _ = Builder::new().make(|path| -> io::Result<File> {
        if attempts == 5 {
            return Err(io::Error::new(io::ErrorKind::Other, "stop!"));
        }
        attempts += 1;
        let f = File::options()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();

        files.push(NamedTempFile::from_parts(f, TempPath::from_path(path)));
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "fake!"))
    });

    assert_eq!(5, attempts);
    attempts = 0;

    // Re-seed to cause a conflict.
    fastrand::seed(42);

    let _f = Builder::new()
        .make(|path| {
            attempts += 1;
            File::options().write(true).create_new(true).open(path)
        })
        .unwrap();

    // We expect exactly three conflict before we re-seed with system randomness.
    assert_eq!(4, attempts);
}

// Issue #224.
#[test]
fn test_overly_generic_bounds() {
    pub struct Foo<T>(T);

    impl<T> Foo<T>
    where
        T: Sync + Send + 'static,
        for<'a> &'a T: Write + Read,
    {
        pub fn new(foo: T) -> Self {
            Self(foo)
        }
    }

    // Don't really need to run this. Only care if it compiles.
    if let Ok(file) = File::open("i_do_not_exist") {
        let mut f;
        let _x = {
            f = Foo::new(file);
            &mut f
        };
    }
}
