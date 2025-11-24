#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

#[cfg(feature = "std")]
fn get_buildid(path: &Path) -> Result<Option<Vec<u8>>, object::read::Error> {
    use object::Object;
    let file = std::fs::File::open(path).unwrap();
    let reader = object::read::ReadCache::new(file);
    let object = object::read::File::parse(&reader)?;
    object
        .build_id()
        .map(|option| option.map(ToOwned::to_owned))
}

#[cfg(feature = "std")]
#[test]
/// Regression test: used to attempt to allocate 5644418395173552131 bytes
fn get_buildid_bad_elf() {
    let path: PathBuf = [
        "testfiles",
        "elf",
        "yara-fuzzing",
        "crash-7dc27920ae1cb85333e7f2735a45014488134673",
    ]
    .iter()
    .collect();
    let _ = get_buildid(&path);
}

#[cfg(feature = "std")]
#[test]
fn get_buildid_less_bad_elf() {
    let path: PathBuf = [
        "testfiles",
        "elf",
        "yara-fuzzing",
        "crash-f1fd008da535b110853885221ebfaac3f262a1c1e280f10929f7b353c44996c8",
    ]
    .iter()
    .collect();
    let buildid = get_buildid(&path).unwrap().unwrap();
    // ground truth obtained from GNU binutils's readelf
    assert_eq!(
        buildid,
        b"\xf9\xc0\xc6\x05\xd3\x76\xbb\xa5\x7e\x02\xf5\x74\x50\x9d\x16\xcc\xe9\x9c\x1b\xf1"
    );
}
