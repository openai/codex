// rust-lang/rust#101913: when you run your program explicitly via `ld.so`,
// `std::env::current_exe` will return the path of *that* program, and not
// the Rust program itself.

// This behavior is only known to be supported on Linux and FreeBSD, see
// https://mail-index.netbsd.org/tech-toolchain/2024/07/27/msg004469.html

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

fn main() {
    if cfg!(target_os = "netbsd") {
        // NetBSD doesn't support this silliness, so because this is an fn main test,
        // just pass it on there. If we used ui-test or something we'd use
        //@ ignore-netbsd
        return;
    }

    if std::env::var(VAR).is_err() {
        // the parent waits for the child; then we then handle either printing
        // "test result: ok", "test result: ignored", or panicking.
        match parent() {
            Ok(()) => {
                println!("test result: ok");
            }
            Err(EarlyExit::IgnoreTest) => {
                println!("test result: ignored");
            }
            Err(EarlyExit::IoError(e)) => {
                println!("{} parent encountered IoError: {:?}", file!(), e);
                panic!();
            }
        }
    } else {
        // println!("{} running child", file!());
        child().unwrap();
    }
}

const VAR: &str = "__THE_TEST_YOU_ARE_LUKE";

#[derive(Debug)]
enum EarlyExit {
    IgnoreTest,
    IoError(std::io::Error),
}

impl From<std::io::Error> for EarlyExit {
    fn from(e: std::io::Error) -> Self {
        EarlyExit::IoError(e)
    }
}

fn parent() -> Result<(), EarlyExit> {
    // If we cannot re-exec this test, there's no point in trying to do it.
    if common::cannot_reexec_the_test() {
        return Err(EarlyExit::IgnoreTest);
    }

    let me = std::env::current_exe().unwrap();
    let ld_so = find_interpreter(&me)?;

    // use interp to invoke current exe, yielding child test.
    //
    // (if you're curious what you might compare this against, you can try
    // swapping in the below definition for `result`, which is the easy case of
    // not using the ld.so interpreter directly that Rust handled fine even
    // prior to resolution of rust-lang/rust#101913.)
    //
    // let result = Command::new(me).env(VAR, "1").output()?;
    let result = Command::new(ld_so).env(VAR, "1").arg(&me).output().unwrap();

    if result.status.success() {
        return Ok(());
    }
    println!("stdout:\n{}", String::from_utf8_lossy(&result.stdout));
    println!("stderr:\n{}", String::from_utf8_lossy(&result.stderr));
    println!("code: {}", result.status);
    panic!();
}

fn child() -> Result<(), EarlyExit> {
    let bt = backtrace::Backtrace::new();
    println!("{bt:?}");

    let mut found_my_name = false;

    let my_filename = file!();
    'frames: for frame in bt.frames() {
        let symbols = frame.symbols();
        if symbols.is_empty() {
            continue;
        }

        for sym in symbols {
            if let Some(filename) = sym.filename() {
                if filename.ends_with(my_filename) {
                    // huzzah!
                    found_my_name = true;
                    break 'frames;
                }
            }
        }
    }

    assert!(found_my_name);

    Ok(())
}

// we use the `readelf` command to extract the path to the interpreter requested
// by our binary.
//
// if we cannot `readelf` for some reason, or if we fail to parse its output,
// then we will just give up on this test (and not treat it as a test failure).
fn find_interpreter(me: &Path) -> Result<PathBuf, EarlyExit> {
    let result = Command::new("readelf")
        .arg("-l")
        .arg(me)
        .output()
        .map_err(|_| EarlyExit::IgnoreTest)?;
    if result.status.success() {
        let r = BufReader::new(&result.stdout[..]);
        for line in r.lines() {
            let line = line?;
            let line = line.trim();
            let prefix = "[Requesting program interpreter: ";
            if let Some((_, suffix)) = line.split_once(prefix) {
                if let Some((found_path, _)) = suffix.rsplit_once("]") {
                    return Ok(found_path.into());
                }
            }
        }
    }
    Err(EarlyExit::IgnoreTest)
}
