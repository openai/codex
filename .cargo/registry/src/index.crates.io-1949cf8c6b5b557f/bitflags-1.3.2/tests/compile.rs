use std::{
    fs,
    ffi::OsStr,
    io,
    path::Path,
};

use walkdir::WalkDir;

#[test]
fn fail() {
    prepare_stderr_files("tests/compile-fail").unwrap();

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/**/*.rs");
}

#[test]
fn pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile-pass/**/*.rs");
}

// Compiler messages may change between versions
// We don't want to have to track these too closely for `bitflags`, but
// having some message to check makes sure user-facing errors are sensical.
// 
// The approach we use is to run the test on all compilers, but only check stderr
// output on beta (which is the next stable release). We do this by default ignoring
// any `.stderr` files in the `compile-fail` directory, and copying `.stderr.beta` files
// when we happen to be running on a beta compiler.
fn prepare_stderr_files(path: impl AsRef<Path>) -> io::Result<()> {
    for entry in WalkDir::new(path) {
        let entry = entry?;

        if entry.path().extension().and_then(OsStr::to_str) == Some("beta") {
            let renamed = entry.path().with_extension("");

            // Unconditionally remove a corresponding `.stderr` file for a `.stderr.beta`
            // file if it exists. On `beta` compilers, we'll recreate it. On other compilers,
            // we don't want to end up checking it anyways.
            if renamed.exists() {
                fs::remove_file(&renamed)?;
            }

            rename_beta_stderr(entry.path(), renamed)?;
        }
    }

    Ok(())
}

#[rustversion::beta]
fn rename_beta_stderr(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    fs::copy(from, to)?;

    Ok(())
}

#[rustversion::not(beta)]
fn rename_beta_stderr(_: impl AsRef<Path>, _: impl AsRef<Path>) -> io::Result<()> {
    Ok(())
}
