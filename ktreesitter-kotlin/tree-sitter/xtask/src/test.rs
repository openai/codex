use std::{
    env,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{anyhow, Result};
use regex::Regex;

use crate::{bail_on_err, Test};

pub fn run(args: &Test) -> Result<()> {
    let test_flags = if args.address_sanitizer {
        env::set_var("CFLAGS", "-fsanitize=undefined,address");

        // When the Tree-sitter C library is compiled with the address sanitizer, the address
        // sanitizer runtime library needs to be linked into the final test executable. When
        // using Xcode clang, the Rust linker doesn't know where to find that library, so we
        // need to specify linker flags directly.
        let output = Command::new("cc").arg("-print-runtime-dir").output()?;
        bail_on_err(&output, "Failed to get clang runtime dir")?;
        let runtime_dir = String::from_utf8(output.stdout)?;
        if runtime_dir.contains("/Xcode.app/") {
            env::set_var(
                "RUSTFLAGS",
                format!(
                    "-C link-arg=-L{runtime_dir} -C link-arg=-lclang_rt.asan_osx_dynamic -C link-arg=-Wl,-rpath,{runtime_dir}"
                ),
            );
        }

        // Specify a `--target` explicitly. This is required for address sanitizer support.
        let output = Command::new("rustup")
            .arg("show")
            .arg("active-toolchain")
            .output()?;
        bail_on_err(&output, "Failed to get active Rust toolchain")?;
        let toolchain = String::from_utf8(output.stdout)?;
        let re = Regex::new(r"(stable|beta|nightly)-([_a-z0-9-]+).*")?;
        let captures = re
            .captures(&toolchain)
            .ok_or_else(|| anyhow!("Failed to parse toolchain '{toolchain}'"))?;
        let current_target = captures.get(2).unwrap().as_str();
        format!("--target={current_target}")
    } else {
        String::new()
    };
    if let Some(language) = &args.language {
        env::set_var("TREE_SITTER_LANGUAGE", language);
    }
    if let Some(example) = &args.example {
        env::set_var("TREE_SITTER_EXAMPLE_INCLUDE", example);
    }
    if let Some(seed) = args.seed {
        env::set_var("TREE_SITTER_SEED", seed.to_string());
    }
    if let Some(iterations) = args.iterations {
        env::set_var("TREE_SITTER_ITERATIONS", iterations.to_string());
    }
    if args.debug {
        env::set_var("TREE_SITTER_LOG", "1");
    }
    if args.debug_graph {
        env::set_var("TREE_SITTER_LOG_GRAPHS", "1");
    }

    if args.g {
        let mut cargo_cmd = Command::new("cargo");
        cargo_cmd
            .arg("test")
            .arg(test_flags)
            .arg("--no-run")
            .arg("--message-format=json");

        #[cfg(target_os = "windows")]
        cargo_cmd.arg("--").arg("--test-threads=1");

        let cargo_cmd = cargo_cmd.stdout(Stdio::piped()).spawn()?;

        let jq_cmd = Command::new("jq")
            .arg("-rs")
            .arg(r#"map(select(.target.name == "tree_sitter_cli" and .executable))[0].executable"#)
            .stdin(cargo_cmd.stdout.unwrap())
            .output()?;

        let test_binary = String::from_utf8(jq_cmd.stdout)?;

        let mut lldb_cmd = Command::new("lldb");
        lldb_cmd.arg(test_binary.trim()).arg("--").args(&args.args);
        bail_on_err(
            &lldb_cmd.spawn()?.wait_with_output()?,
            &format!("Failed to run {lldb_cmd:?}"),
        )?;
    } else {
        let mut cargo_cmd = Command::new("cargo");
        cargo_cmd.arg("test");
        if args.wasm {
            cargo_cmd.arg("--features").arg("wasm");
        }
        if !test_flags.is_empty() {
            cargo_cmd.arg(test_flags);
        }
        cargo_cmd.args(&args.args);

        #[cfg(target_os = "windows")]
        cargo_cmd.arg("--").arg("--test-threads=1");

        if args.nocapture {
            #[cfg(not(target_os = "windows"))]
            cargo_cmd.arg("--");

            cargo_cmd.arg("--nocapture");
        }
        bail_on_err(
            &cargo_cmd.spawn()?.wait_with_output()?,
            &format!("Failed to run {cargo_cmd:?}"),
        )?;
    }

    Ok(())
}

pub fn run_wasm() -> Result<()> {
    std::env::set_current_dir("lib/binding_web")?;

    let node_modules_dir = Path::new("node_modules");
    let npm = if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    };

    if !node_modules_dir.join("chai").exists() || !node_modules_dir.join("mocha").exists() {
        println!("Installing test dependencies...");
        let output = Command::new(npm).arg("install").output()?;
        bail_on_err(&output, "Failed to install test dependencies")?;
    }

    let child = Command::new(npm).arg("test").spawn()?;
    let output = child.wait_with_output()?;
    bail_on_err(&output, &format!("Failed to run `{npm} test`"))?;

    // Display test results
    let output = String::from_utf8_lossy(&output.stdout);
    for line in output.lines() {
        println!("{line}");
    }

    Ok(())
}
