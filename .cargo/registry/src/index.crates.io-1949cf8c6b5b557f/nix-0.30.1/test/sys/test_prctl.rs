#[cfg(target_os = "linux")]
#[cfg(feature = "process")]
mod test_prctl {
    use std::ffi::CStr;

    use nix::sys::prctl;

    #[cfg_attr(qemu, ignore)]
    #[test]
    fn test_get_set_subreaper() {
        let original = prctl::get_child_subreaper().unwrap();

        prctl::set_child_subreaper(true).unwrap();
        let subreaper = prctl::get_child_subreaper().unwrap();
        assert!(subreaper);

        prctl::set_child_subreaper(original).unwrap();
    }

    #[test]
    fn test_get_set_dumpable() {
        let original = prctl::get_dumpable().unwrap();

        prctl::set_dumpable(false).unwrap();
        let dumpable = prctl::get_dumpable().unwrap();
        assert!(!dumpable);

        prctl::set_dumpable(original).unwrap();
    }

    #[test]
    fn test_get_set_keepcaps() {
        let original = prctl::get_keepcaps().unwrap();

        prctl::set_keepcaps(true).unwrap();
        let keepcaps = prctl::get_keepcaps().unwrap();
        assert!(keepcaps);

        prctl::set_keepcaps(original).unwrap();
    }

    #[test]
    fn test_get_set_clear_mce_kill() {
        use prctl::PrctlMCEKillPolicy::*;

        prctl::set_mce_kill(PR_MCE_KILL_LATE).unwrap();
        let mce = prctl::get_mce_kill().unwrap();
        assert_eq!(mce, PR_MCE_KILL_LATE);

        prctl::clear_mce_kill().unwrap();
        let mce = prctl::get_mce_kill().unwrap();
        assert_eq!(mce, PR_MCE_KILL_DEFAULT);
    }

    #[cfg_attr(qemu, ignore)]
    #[test]
    fn test_get_set_pdeathsig() {
        use nix::sys::signal::Signal;

        let original = prctl::get_pdeathsig().unwrap();

        prctl::set_pdeathsig(Signal::SIGUSR1).unwrap();
        let sig = prctl::get_pdeathsig().unwrap();
        assert_eq!(sig, Some(Signal::SIGUSR1));

        prctl::set_pdeathsig(original).unwrap();
    }

    #[test]
    fn test_get_set_name() {
        let original = prctl::get_name().unwrap();

        let long_name =
            CStr::from_bytes_with_nul(b"0123456789abcdefghijklmn\0").unwrap();
        prctl::set_name(long_name).unwrap();
        let res = prctl::get_name().unwrap();

        // name truncated by kernel to TASK_COMM_LEN
        assert_eq!(&long_name.to_str().unwrap()[..15], res.to_str().unwrap());

        let short_name = CStr::from_bytes_with_nul(b"01234567\0").unwrap();
        prctl::set_name(short_name).unwrap();
        let res = prctl::get_name().unwrap();
        assert_eq!(short_name.to_str().unwrap(), res.to_str().unwrap());

        prctl::set_name(&original).unwrap();
    }

    #[cfg_attr(qemu, ignore)]
    #[test]
    fn test_get_set_timerslack() {
        let original = prctl::get_timerslack().unwrap() as libc::c_ulong;

        let slack = 60_000;
        prctl::set_timerslack(slack).unwrap();
        let res = prctl::get_timerslack().unwrap() as libc::c_ulong;
        assert_eq!(slack, res);

        prctl::set_timerslack(original).unwrap();
    }

    // Loongarch need to use a newer QEMU that disabled these PRCTL subcodes/methods.
    // https://github.com/qemu/qemu/commit/220717a6f46a99031a5b1af964bbf4dec1310440
    // So we should ignore them when testing in QEMU environments.
    #[cfg_attr(all(qemu, target_arch = "loongarch64"), ignore)]
    #[test]
    fn test_disable_enable_perf_events() {
        prctl::task_perf_events_disable().unwrap();
        prctl::task_perf_events_enable().unwrap();
    }

    #[test]
    fn test_get_set_no_new_privs() {
        prctl::set_no_new_privs().unwrap();
        let no_new_privs = prctl::get_no_new_privs().unwrap();
        assert!(no_new_privs);
    }

    // Loongarch need to use a newer QEMU that disabled these PRCTL subcodes/methods
    // https://github.com/qemu/qemu/commit/220717a6f46a99031a5b1af964bbf4dec1310440
    // So we should ignore them when testing in QEMU environments.
    #[cfg_attr(all(qemu, target_arch = "loongarch64"), ignore)]
    #[test]
    fn test_get_set_thp_disable() {
        let original = prctl::get_thp_disable().unwrap();

        prctl::set_thp_disable(true).unwrap();
        let thp_disable = prctl::get_thp_disable().unwrap();
        assert!(thp_disable);

        prctl::set_thp_disable(original).unwrap();
    }

    // Ignore this test under QEMU, as it started failing after updating the Linux CI
    // runner image, for reasons unknown.
    //
    // See: https://github.com/nix-rust/nix/issues/2418
    #[test]
    #[cfg_attr(qemu, ignore)]
    fn test_set_vma_anon_name() {
        use nix::errno::Errno;
        use nix::sys::mman;
        use std::num::NonZeroUsize;

        const ONE_K: libc::size_t = 1024;
        let sz = NonZeroUsize::new(ONE_K).unwrap();
        let ptr = unsafe {
            mman::mmap_anonymous(
                None,
                sz,
                mman::ProtFlags::PROT_READ,
                mman::MapFlags::MAP_SHARED,
            )
            .unwrap()
        };
        let err = prctl::set_vma_anon_name(
            ptr,
            sz,
            Some(CStr::from_bytes_with_nul(b"[,$\0").unwrap()),
        )
        .unwrap_err();
        assert_eq!(err, Errno::EINVAL);
        // `CONFIG_ANON_VMA_NAME` kernel config might not be set
        prctl::set_vma_anon_name(
            ptr,
            sz,
            Some(CStr::from_bytes_with_nul(b"Nix\0").unwrap()),
        )
        .unwrap_or_default();
        prctl::set_vma_anon_name(ptr, sz, None).unwrap_or_default();
    }
}
