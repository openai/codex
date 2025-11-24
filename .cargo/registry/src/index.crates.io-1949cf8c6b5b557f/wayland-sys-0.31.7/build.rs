use pkg_config::Config;

fn main() {
    if std::env::var_os("CARGO_FEATURE_DLOPEN").is_some() {
        // Do not link to anything
        return;
    }

    if std::env::var_os("CARGO_FEATURE_CLIENT").is_some() {
        Config::new().probe("wayland-client").unwrap();
    }
    if std::env::var_os("CARGO_FEATURE_CURSOR").is_some() {
        Config::new().probe("wayland-cursor").unwrap();
    }
    if std::env::var_os("CARGO_FEATURE_EGL").is_some() {
        Config::new().probe("wayland-egl").unwrap();
    }
    if std::env::var_os("CARGO_FEATURE_SERVER").is_some() {
        Config::new().probe("wayland-server").unwrap();
    }
}
