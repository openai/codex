use super::PACKAGE_METADATA_FILENAME;
use super::PATH_DIRNAME;
use super::RESOURCES_DIRNAME;
use super::is_default_files_request;
use super::real_rg_path;
use pretty_assertions::assert_eq;
use std::ffi::OsString;
use std::fs;
use tempfile::TempDir;

#[test]
fn resolves_ripgrep_from_package_resources() {
    let temp_dir = TempDir::new().expect("temp dir");
    let package_dir = temp_dir.path();
    let path_dir = package_dir.join(PATH_DIRNAME);
    let resources_dir = package_dir.join(RESOURCES_DIRNAME);
    fs::create_dir_all(&path_dir).expect("path dir");
    fs::create_dir_all(&resources_dir).expect("resources dir");
    fs::write(package_dir.join(PACKAGE_METADATA_FILENAME), "{}").expect("metadata");

    let executable_name = if cfg!(windows) { "rg.exe" } else { "rg" };
    let shim = path_dir.join(executable_name);
    let real_rg = resources_dir.join(executable_name);
    fs::write(&shim, "shim").expect("shim");
    fs::write(&real_rg, "real rg").expect("real rg");

    assert_eq!(real_rg_path(&shim).expect("real rg path"), real_rg);
}

#[test]
fn rejects_executable_outside_package_path() {
    let temp_dir = TempDir::new().expect("temp dir");
    let executable = temp_dir.path().join("rg");
    fs::write(&executable, "shim").expect("shim");

    let error = real_rg_path(&executable).expect_err("unpackaged shim must fail");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn only_exact_default_files_request_uses_inventory() {
    assert!(is_default_files_request(&[OsString::from("--files")]));
    assert!(!is_default_files_request(&[
        OsString::from("--files"),
        OsString::from("-0")
    ]));
    assert!(!is_default_files_request(&[OsString::from("--hidden")]));
}
