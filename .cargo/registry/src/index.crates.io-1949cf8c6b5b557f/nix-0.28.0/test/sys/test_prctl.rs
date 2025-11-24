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
        let original = prctl::get_timerslack().unwrap();

        let slack = 60_000;
        prctl::set_timerslack(slack).unwrap();
        let res = prctl::get_timerslack().unwrap();
        assert_eq!(slack, res as u64);

        prctl::set_timerslack(original as u64).unwrap();
    }

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

    #[test]
    fn test_get_set_thp_disable() {
        let original = prctl::get_thp_disable().unwrap();

        prctl::set_thp_disable(true).unwrap();
        let thp_disable = prctl::get_thp_disable().unwrap();
        assert!(thp_disable);

        prctl::set_thp_disable(original).unwrap();
    }
}
