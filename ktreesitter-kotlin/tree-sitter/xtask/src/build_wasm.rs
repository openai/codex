use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    fmt::Write,
    fs,
    path::PathBuf,
    process::Command,
    time::Duration,
};

use anyhow::{anyhow, Result};
use notify::{
    event::{AccessKind, AccessMode},
    EventKind, RecursiveMode,
};
use notify_debouncer_full::new_debouncer;

use crate::{bail_on_err, watch_wasm, BuildWasm, EMSCRIPTEN_TAG};

#[derive(PartialEq, Eq)]
enum EmccSource {
    Native,
    Docker,
    Podman,
}

const EXPORTED_RUNTIME_METHODS: [&str; 8] = [
    "AsciiToString",
    "stringToUTF8",
    "UTF8ToString",
    "lengthBytesUTF8",
    "stringToUTF16",
    "loadWebAssemblyModule",
    "getValue",
    "setValue",
];

pub fn run_wasm(args: &BuildWasm) -> Result<()> {
    let mut emscripten_flags = vec!["-O3", "--minify", "0"];

    if args.debug {
        emscripten_flags.extend(["-s", "ASSERTIONS=1", "-s", "SAFE_HEAP=1", "-O0", "-g"]);
    }

    if args.verbose {
        emscripten_flags.extend(["-s", "VERBOSE=1", "-v"]);
    }

    let emcc_name = if cfg!(windows) { "emcc.bat" } else { "emcc" };

    // Order of preference: emscripten > docker > podman > error
    let source = if !args.docker && Command::new(emcc_name).output().is_ok() {
        EmccSource::Native
    } else if Command::new("docker")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        EmccSource::Docker
    } else if Command::new("podman")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        EmccSource::Podman
    } else {
        return Err(anyhow!(
            "You must have either emcc, docker, or podman on your PATH to run this command"
        ));
    };

    let mut command = match source {
        EmccSource::Native => Command::new(emcc_name),
        EmccSource::Docker | EmccSource::Podman => {
            let mut command = match source {
                EmccSource::Docker => Command::new("docker"),
                EmccSource::Podman => Command::new("podman"),
                _ => unreachable!(),
            };
            command.args(["run", "--rm"]);

            // Mount the root directory as a volume, which is the repo root
            let mut volume_string = OsString::from(std::env::current_dir().unwrap());
            volume_string.push(":/src:Z");
            command.args([OsStr::new("--volume"), &volume_string]);

            // In case `docker` is an alias to `podman`, ensure that podman
            // mounts the current directory as writable by the container
            // user which has the same uid as the host user. Setting the
            // podman-specific variable is more reliable than attempting to
            // detect whether `docker` is an alias for `podman`.
            // see https://docs.podman.io/en/latest/markdown/podman-run.1.html#userns-mode
            command.env("PODMAN_USERNS", "keep-id");

            // Get the current user id so that files created in the docker container will have
            // the same owner.
            #[cfg(unix)]
            {
                #[link(name = "c")]
                extern "C" {
                    fn getuid() -> u32;
                }
                // don't need to set user for podman since PODMAN_USERNS=keep-id is already set
                if source == EmccSource::Docker {
                    let user_id = unsafe { getuid() };
                    command.args(["--user", &user_id.to_string()]);
                }
            };

            // Run `emcc` in a container using the `emscripten-slim` image
            command.args([EMSCRIPTEN_TAG, "emcc"]);
            command
        }
    };

    fs::create_dir_all("target/scratch").unwrap();

    let exported_functions = format!(
        "{}{}",
        fs::read_to_string("lib/src/wasm/stdlib-symbols.txt")?,
        fs::read_to_string("lib/binding_web/lib/exports.txt")?
    )
    .replace('"', "")
    .lines()
    .fold(String::new(), |mut output, line| {
        let _ = write!(output, "_{line}");
        output
    })
    .trim_end_matches(',')
    .to_string();

    let exported_functions = format!("EXPORTED_FUNCTIONS={exported_functions}");
    let exported_runtime_methods = format!(
        "EXPORTED_RUNTIME_METHODS={}",
        EXPORTED_RUNTIME_METHODS.join(",")
    );

    // Clean up old files from prior runs
    for file in [
        "tree-sitter.mjs",
        "tree-sitter.cjs",
        "tree-sitter.wasm",
        "tree-sitter.wasm.map",
    ] {
        fs::remove_file(PathBuf::from("lib/binding_web/lib").join(file)).ok();
    }

    if !args.cjs {
        emscripten_flags.extend(["-s", "EXPORT_ES6=1"]);
    }

    #[rustfmt::skip]
    emscripten_flags.extend([
        "-gsource-map",
        "--source-map-base", ".",
        "-fno-exceptions",
        "-std=c11",
        "-s", "WASM=1",
        "-s", "MODULARIZE=1",
        "-s", "INITIAL_MEMORY=33554432",
        "-s", "ALLOW_MEMORY_GROWTH=1",
        "-s", "SUPPORT_BIG_ENDIAN=1",
        "-s", "MAIN_MODULE=2",
        "-s", "FILESYSTEM=0",
        "-s", "NODEJS_CATCH_EXIT=0",
        "-s", "NODEJS_CATCH_REJECTION=0",
        "-s", &exported_functions,
        "-s", &exported_runtime_methods,
        "-D", "fprintf(...)=",
        "-D", "NDEBUG=",
        "-D", "_POSIX_C_SOURCE=200112L",
        "-D", "_DEFAULT_SOURCE=",
        "-I", "lib/src",
        "-I", "lib/include",
        "--js-library", "lib/binding_web/lib/imports.js",
        "--pre-js",     "lib/binding_web/lib/prefix.js",
        "-o",           if args.cjs { "lib/binding_web/lib/tree-sitter.cjs" } else { "lib/binding_web/lib/tree-sitter.mjs" },
        "lib/src/lib.c",
        "lib/binding_web/lib/tree-sitter.c",
    ]);
    if args.emit_tsd {
        emscripten_flags.extend(["--emit-tsd", "tree-sitter.d.ts"]);
    }

    let command = command.args(&emscripten_flags);

    if args.watch {
        watch_wasm!(|| build_wasm(command));
    } else {
        build_wasm(command)?;
    }

    Ok(())
}

fn build_wasm(cmd: &mut Command) -> Result<()> {
    bail_on_err(
        &cmd.spawn()?.wait_with_output()?,
        "Failed to compile the Tree-sitter WASM library",
    )?;

    Ok(())
}

pub fn run_wasm_stdlib() -> Result<()> {
    let export_flags = include_str!("../../lib/src/wasm/stdlib-symbols.txt")
        .lines()
        .map(|line| format!("-Wl,--export={}", &line[1..line.len() - 2]))
        .collect::<Vec<String>>();

    let mut command = Command::new("docker");

    let output = command
        .args([
            "run",
            "--rm",
            "-v",
            format!("{}:/src", std::env::current_dir().unwrap().display()).as_str(),
            "-w",
            "/src",
            "ghcr.io/webassembly/wasi-sdk",
            "/opt/wasi-sdk/bin/clang",
            "-o",
            "stdlib.wasm",
            "-Os",
            "-fPIC",
            "-Wl,--no-entry",
            "-Wl,--stack-first",
            "-Wl,-z",
            "-Wl,stack-size=65536",
            "-Wl,--import-undefined",
            "-Wl,--import-memory",
            "-Wl,--import-table",
            "-Wl,--strip-debug",
            "-Wl,--export=reset_heap",
            "-Wl,--export=__wasm_call_ctors",
            "-Wl,--export=__stack_pointer",
        ])
        .args(export_flags)
        .arg("lib/src/wasm/stdlib.c")
        .output()?;

    bail_on_err(&output, "Failed to compile the Tree-sitter WASM stdlib")?;

    let xxd = Command::new("xxd")
        .args(["-C", "-i", "stdlib.wasm"])
        .output()?;

    bail_on_err(
        &xxd,
        "Failed to run xxd on the compiled Tree-sitter WASM stdlib",
    )?;

    fs::write("lib/src/wasm/wasm-stdlib.h", xxd.stdout)?;

    fs::rename("stdlib.wasm", "target/stdlib.wasm")?;

    Ok(())
}
