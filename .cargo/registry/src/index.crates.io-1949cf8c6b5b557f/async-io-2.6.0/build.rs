fn main() {
    let cfg = match autocfg::AutoCfg::new() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("cargo:warning=async-io: failed to detect compiler features: {e}");
            return;
        }
    };

    // We use "no_*" instead of "has_*" here. For (non-Cargo) build processes
    // that don't run build.rs, the negated version gives us a recent
    // feature-set by default.
    if !cfg.probe_rustc_version(1, 87) {
        autocfg::emit("async_io_no_pipe");
    }
}
