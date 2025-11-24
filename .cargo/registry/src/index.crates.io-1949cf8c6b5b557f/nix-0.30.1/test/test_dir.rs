use nix::dir::{Dir, Type};
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use std::fs::File;
use tempfile::tempdir;

#[cfg(test)]
fn flags() -> OFlag {
    #[cfg(solarish)]
    let f = OFlag::O_RDONLY | OFlag::O_CLOEXEC;

    #[cfg(not(solarish))]
    let f = OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_DIRECTORY;

    f
}

#[test]
fn read() {
    let tmp = tempdir().unwrap();
    File::create(tmp.path().join("foo")).unwrap();
    std::os::unix::fs::symlink("foo", tmp.path().join("bar")).unwrap();
    let mut dir = Dir::open(tmp.path(), flags(), Mode::empty()).unwrap();
    let mut entries: Vec<_> = dir.iter().map(|e| e.unwrap()).collect();
    entries.sort_by(|a, b| a.file_name().cmp(b.file_name()));
    let entry_names: Vec<_> = entries
        .iter()
        .map(|e| e.file_name().to_str().unwrap().to_owned())
        .collect();
    assert_eq!(&entry_names[..], &[".", "..", "bar", "foo"]);

    // Check file types. The system is allowed to return DT_UNKNOWN (aka None here) but if it does
    // return a type, ensure it's correct.
    assert!(&[Some(Type::Directory), None].contains(&entries[0].file_type())); // .: dir
    assert!(&[Some(Type::Directory), None].contains(&entries[1].file_type())); // ..: dir
    assert!(&[Some(Type::Symlink), None].contains(&entries[2].file_type())); // bar: symlink
    assert!(&[Some(Type::File), None].contains(&entries[3].file_type())); // foo: regular file
}

#[test]
fn rewind() {
    let tmp = tempdir().unwrap();
    let mut dir = Dir::open(tmp.path(), flags(), Mode::empty()).unwrap();
    let entries1: Vec<_> = dir
        .iter()
        .map(|e| e.unwrap().file_name().to_owned())
        .collect();
    let entries2: Vec<_> = dir
        .iter()
        .map(|e| e.unwrap().file_name().to_owned())
        .collect();
    let entries3: Vec<_> = dir
        .into_iter()
        .map(|e| e.unwrap().file_name().to_owned())
        .collect();
    assert_eq!(entries1, entries2);
    assert_eq!(entries2, entries3);
}
