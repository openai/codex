fn main() {
    println!("cargo:rustc-check-cfg=cfg(coverage)");
    println!("cargo:rustc-check-cfg=cfg(unstable_coverage)");

    if std::env::var("CARGO_FEATURE_LOG").ok().is_some() {
        // build the client shim
        cc::Build::new().file("src/sys/client_impl/log_shim.c").compile("log_shim_client");
        println!("cargo:rerun-if-changed=src/sys/client_impl/log_shim.c");
        // build the server shim
        cc::Build::new().file("src/sys/server_impl/log_shim.c").compile("log_shim_server");
        println!("cargo:rerun-if-changed=src/sys/server_impl/log_shim.c");
    }
}
