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

        apple_targets: { any(ios, macos, watchos, tvos) },
        bsd: { any(freebsd, dragonfly, netbsd, openbsd, apple_targets) },
        linux_android: { any(android, linux) },
        freebsdlike: { any(dragonfly, freebsd) },
        netbsdlike: { any(netbsd, openbsd) },
        solarish: { any(illumos, solaris) },
    }
}
