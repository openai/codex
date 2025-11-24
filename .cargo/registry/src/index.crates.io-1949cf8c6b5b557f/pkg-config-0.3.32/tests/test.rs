use lazy_static::lazy_static;
use pkg_config::Error;
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;

lazy_static! {
    static ref LOCK: Mutex<()> = Mutex::new(());
}

fn reset() {
    for (k, _) in env::vars() {
        if k.contains("DYNAMIC")
            || k.contains("STATIC")
            || k.contains("PKG_CONFIG_ALLOW_CROSS")
            || k.contains("PKG_CONFIG_SYSROOT_DIR")
            || k.contains("FOO_NO_PKG_CONFIG")
        {
            env::remove_var(&k);
        }
    }
    env::remove_var("TARGET");
    env::remove_var("HOST");
    env::set_var("PKG_CONFIG_PATH", env::current_dir().unwrap().join("tests"));
}

fn find(name: &str) -> Result<pkg_config::Library, Error> {
    pkg_config::probe_library(name)
}

#[test]
fn cross_disabled() {
    let _g = LOCK.lock();
    reset();
    env::set_var("TARGET", "foo");
    env::set_var("HOST", "bar");
    match find("foo") {
        Err(Error::CrossCompilation) => {}
        x => panic!("Error::CrossCompilation expected, found `{:?}`", x),
    }
}

#[test]
fn cross_enabled() {
    let _g = LOCK.lock();
    reset();
    env::set_var("TARGET", "foo");
    env::set_var("HOST", "bar");
    env::set_var("PKG_CONFIG_ALLOW_CROSS", "1");
    find("foo").unwrap();
}

#[test]
fn cross_enabled_if_customized() {
    let _g = LOCK.lock();
    reset();
    env::set_var("TARGET", "foo");
    env::set_var("HOST", "bar");
    env::set_var("PKG_CONFIG_SYSROOT_DIR", "/tmp/cross-test");
    find("foo").unwrap();
}

#[test]
fn cross_disabled_if_customized() {
    let _g = LOCK.lock();
    reset();
    env::set_var("TARGET", "foo");
    env::set_var("HOST", "bar");
    env::set_var("PKG_CONFIG_ALLOW_CROSS", "0");
    env::set_var("PKG_CONFIG_SYSROOT_DIR", "/tmp/cross-test");
    match find("foo") {
        Err(Error::CrossCompilation) => {}
        _ => panic!("expected CrossCompilation failure"),
    }
}

#[test]
fn package_disabled() {
    let _g = LOCK.lock();
    reset();
    env::set_var("FOO_NO_PKG_CONFIG", "1");
    match find("foo") {
        Err(Error::EnvNoPkgConfig(name)) => assert_eq!(name, "FOO_NO_PKG_CONFIG"),
        x => panic!("Error::EnvNoPkgConfig expected, found `{:?}`", x),
    }
}

#[test]
fn output_ok() {
    let _g = LOCK.lock();
    reset();
    let lib = find("foo").unwrap();
    assert!(lib.libs.contains(&"gcc".to_string()));
    assert!(lib.libs.contains(&"coregrind-amd64-linux".to_string()));
    assert!(lib.link_paths.contains(&PathBuf::from("/usr/lib/valgrind")));
    assert!(lib
        .include_paths
        .contains(&PathBuf::from("/usr/include/valgrind")));
    assert!(lib.include_paths.contains(&PathBuf::from("/usr/foo")));
}

#[test]
fn escapes() {
    let _g = LOCK.lock();
    reset();
    let lib = find("escape").unwrap();
    assert!(lib
        .include_paths
        .contains(&PathBuf::from("include path with spaces")));
    assert!(lib
        .link_paths
        .contains(&PathBuf::from("link path with spaces")));
    assert_eq!(
        lib.defines.get("A"),
        Some(&Some("\"escaped string' literal\"".to_owned()))
    );
    assert_eq!(
        lib.defines.get("B"),
        Some(&Some("ESCAPED IDENTIFIER".to_owned()))
    );
    assert_eq!(lib.defines.get("FOX"), Some(&Some("🦊".to_owned())));
}

#[test]
fn framework() {
    let _g = LOCK.lock();
    reset();
    let lib = find("framework").unwrap();
    assert!(lib.frameworks.contains(&"foo".to_string()));
    assert!(lib.frameworks.contains(&"bar".to_string()));
    assert!(lib.frameworks.contains(&"baz".to_string()));
    assert!(lib.frameworks.contains(&"foobar".to_string()));
    assert!(lib.frameworks.contains(&"foobaz".to_string()));
    assert!(lib.framework_paths.contains(&PathBuf::from("/usr/lib")));
}

#[test]
fn get_variable() {
    let _g = LOCK.lock();
    reset();
    let prefix = pkg_config::get_variable("foo", "prefix").unwrap();
    assert_eq!(prefix, "/usr");
}

#[test]
fn version() {
    let _g = LOCK.lock();
    reset();
    assert_eq!(&find("foo").unwrap().version[..], "3.10.0.SVN");
}

#[test]
fn atleast_version_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .atleast_version("3.10")
        .probe("foo")
        .unwrap();
}

#[test]
#[should_panic]
fn atleast_version_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .atleast_version("3.11")
        .probe("foo")
        .unwrap();
}

#[test]
fn exactly_version_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .exactly_version("3.10.0.SVN")
        .probe("foo")
        .unwrap();
}

#[test]
#[should_panic]
fn exactly_version_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .exactly_version("3.10.0")
        .probe("foo")
        .unwrap();
}

#[test]
fn range_version_range_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version("4.2.0".."4.4.0")
        .probe("escape")
        .unwrap();
}

#[test]
#[should_panic]
fn range_version_range_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version("4.0.0".."4.2.0")
        .probe("escape")
        .unwrap();
}

#[test]
fn range_version_range_inclusive_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version("4.0.0"..="4.2.0")
        .probe("escape")
        .unwrap();
}

#[test]
#[should_panic]
fn range_version_range_inclusive_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version("3.8.0"..="4.0.0")
        .probe("escape")
        .unwrap();
}

#[test]
fn range_version_range_from_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version("4.0.0"..)
        .probe("escape")
        .unwrap();
}

#[test]
#[should_panic]
fn range_version_range_from_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version("4.4.0"..)
        .probe("escape")
        .unwrap();
}

#[test]
fn range_version_range_to_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version(.."4.4.0")
        .probe("escape")
        .unwrap();
}

#[test]
#[should_panic]
fn range_version_range_to_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version(.."4.2.0")
        .probe("escape")
        .unwrap();
}

#[test]
fn range_version_range_to_inclusive_ok() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version(..="4.2.0")
        .probe("escape")
        .unwrap();
}

#[test]
#[should_panic]
fn range_version_range_to_inclusive_ng() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version(..="4.0.0")
        .probe("escape")
        .unwrap();
}

#[test]
fn range_version_full() {
    let _g = LOCK.lock();
    reset();
    pkg_config::Config::new()
        .range_version(..)
        .probe("escape")
        .unwrap();
}

#[test]
fn rpath() {
    let _g = LOCK.lock();
    reset();
    let lib = find("rpath").unwrap();
    assert!(lib
        .ld_args
        .contains(&vec!["-rpath".to_string(), "/usr/local/lib".to_string(),]));
}
