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

    // Below are custom cfg values set during some CI steps.
    println!("cargo:rustc-check-cfg=cfg(fbsd14)");
    println!("cargo:rustc-check-cfg=cfg(qemu)");
    // Cygwin target, added in 1.86
    println!("cargo:rustc-check-cfg=cfg(target_os, values(\"cygwin\"))");
}
