fn main() {
    if std::env::var_os("RULES_RUST_BAZEL_BUILD_SCRIPT_RUNNER").is_some()
        && matches!(std::env::var("CARGO_CFG_TARGET_ENV").as_deref(), Ok("gnu"))
    {
        // The Windows Bazel lint/test lane targets `windows-gnullvm`, where
        // `winres` can emit a `resource` link directive without a usable
        // archive in `OUT_DIR`. Skip embedding the manifest there; Cargo's
        // normal MSVC builds still compile it.
        return;
    }

    let mut res = winres::WindowsResource::new();
    res.set_manifest_file("codex-windows-sandbox-setup.manifest");
    let _ = res.compile();
}
