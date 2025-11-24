use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        android: { target_os = "android" },
        dragonfly: { target_os = "dragonfly" },
        ios: { target_os = "ios" },
        freebsd: { target_os = "freebsd" },
        illumos: { target_os = "illumos" },
        linux: { target_os = "linux" },
        macos: { target_os = "macos" },
        netbsd: { target_os = "netbsd" },
        openbsd: { target_os = "openbsd" },
        solaris: { target_os = "solaris" },
        watchos: { target_os = "watchos" },
        tvos: { target_os = "tvos" },
        visionos: { target_os = "visionos" },


        // cfg aliases we would like to use
        apple_targets: { any(ios, macos, watchos, tvos, visionos) },
        bsd: { any(freebsd, dragonfly, netbsd, openbsd, apple_targets) },
        bsd_without_apple: { any(freebsd, dragonfly, netbsd, openbsd) },
        linux_android: { any(android, linux) },
        freebsdlike: { any(dragonfly, freebsd) },
        netbsdlike: { any(netbsd, openbsd) },
        solarish: { any(illumos, solaris) },
    }

    // Below are Nix's custom cfg values that we need to let the compiler know
    println!("cargo:rustc-check-cfg=cfg(apple_targets)");
    println!("cargo:rustc-check-cfg=cfg(bsd)");
    println!("cargo:rustc-check-cfg=cfg(bsd_without_apple)");
    println!("cargo:rustc-check-cfg=cfg(linux_android)");
    println!("cargo:rustc-check-cfg=cfg(freebsdlike)");
    println!("cargo:rustc-check-cfg=cfg(netbsdlike)");
    println!("cargo:rustc-check-cfg=cfg(solarish)");
    println!("cargo:rustc-check-cfg=cfg(fbsd14)");
    println!("cargo:rustc-check-cfg=cfg(qemu)");
}
