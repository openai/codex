fn main() {
    let mut res = winres::WindowsResource::new();
// windows-sandbox-rs/build.rs
    res.set_manifest_file("codex-windows-sandbox-setup.manifest");
    let _ = res.compile();
}
