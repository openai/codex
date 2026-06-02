use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=codex-windows-sandbox-setup.manifest");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }

    let manifest_path = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR should be set for build scripts"),
    )
    .join("codex-windows-sandbox-setup.manifest");

    // Keep this scoped to the setup helper so Codex binaries that link the
    // library do not inherit any resource metadata from this package.
    println!("cargo:rustc-link-arg-bin=codex-windows-sandbox-setup=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bin=codex-windows-sandbox-setup=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
}
