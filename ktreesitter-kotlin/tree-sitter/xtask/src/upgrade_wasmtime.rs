use std::process::Command;

use anyhow::{Context, Result};
use git2::Repository;
use semver::Version;

use crate::{create_commit, UpgradeWasmtime};

const WASMTIME_RELEASE_URL: &str = "https://github.com/bytecodealliance/wasmtime/releases/download";

fn update_cargo(version: &Version) -> Result<()> {
    let file = std::fs::read_to_string("lib/Cargo.toml")?;
    let mut old_lines = file.lines();
    let mut new_lines = Vec::with_capacity(old_lines.size_hint().0);

    while let Some(line) = old_lines.next() {
        new_lines.push(line.to_string());
        if line == "[dependencies.wasmtime-c-api]" {
            let _ = old_lines.next();
            new_lines.push(format!("version = \"{version}\""));
        }
    }

    std::fs::write("lib/Cargo.toml", new_lines.join("\n") + "\n")?;

    Command::new("cargo")
        .arg("update")
        .status()
        .map(|_| ())
        .with_context(|| "Failed to execute cargo update")
}

fn zig_fetch(lines: &mut Vec<String>, version: &Version, url_suffix: &str) -> Result<()> {
    let url = &format!("{WASMTIME_RELEASE_URL}/v{version}/wasmtime-v{version}-{url_suffix}");
    println!("  Fetching {url}");
    lines.push(format!("      .url = \"{url}\","));

    let output = Command::new("zig")
        .arg("fetch")
        .arg(url)
        .output()
        .with_context(|| format!("Failed to execute zig fetch {url}"))?;

    let hash = String::from_utf8_lossy(&output.stdout);
    lines.push(format!("      .hash = \"{}\",", hash.trim_end()));

    Ok(())
}

fn update_zig(version: &Version) -> Result<()> {
    let file = std::fs::read_to_string("build.zig.zon")?;
    let mut old_lines = file.lines();
    let new_lines = &mut Vec::with_capacity(old_lines.size_hint().0);

    while let Some(line) = old_lines.next() {
        new_lines.push(line.to_string());
        match line {
            "    .wasmtime_c_api_aarch64_android = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "aarch64-android-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_aarch64_linux = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "aarch64-linux-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_aarch64_macos = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "aarch64-macos-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_riscv64gc_linux = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "riscv64gc-linux-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_s390x_linux = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "s390x-linux-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_x86_64_android = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "x86_64-android-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_x86_64_linux = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "x86_64-linux-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_x86_64_macos = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "x86_64-macos-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_x86_64_mingw = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "x86_64-mingw-c-api.zip")?;
            }
            "    .wasmtime_c_api_x86_64_musl = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "x86_64-musl-c-api.tar.xz")?;
            }
            "    .wasmtime_c_api_x86_64_windows = .{" => {
                let (_, _) = (old_lines.next(), old_lines.next());
                zig_fetch(new_lines, version, "x86_64-windows-c-api.zip")?;
            }
            _ => {}
        }
    }

    std::fs::write("build.zig.zon", new_lines.join("\n") + "\n")?;

    Ok(())
}

pub fn run(args: &UpgradeWasmtime) -> Result<()> {
    println!("Upgrading wasmtime for Rust");
    update_cargo(&args.version)?;

    println!("Upgrading wasmtime for Zig");
    update_zig(&args.version)?;

    let repo = Repository::open(".")?;
    create_commit(
        &repo,
        &format!("build(deps): bump wasmtime-c-api to v{}", args.version),
        &["lib/Cargo.toml", "build.zig.zon"],
    )?;

    Ok(())
}
