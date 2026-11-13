//! Verify network policies select and enforce the expected seccomp restrictions.

#![cfg(test)]

use super::NetworkSeccompMode;
use super::ProxyRouting;
use super::install_network_seccomp_filter_on_current_thread;
use super::network_seccomp_mode;
use super::set_no_new_privs;
use super::should_install_network_seccomp;
use codex_protocol::protocol::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;
use seccompiler::BpfProgram;
use seccompiler::SeccompAction;
use seccompiler::SeccompFilter;
use seccompiler::apply_filter;
use std::collections::BTreeMap;
use test_case::test_case;

#[test_case(NetworkSeccompMode::Restricted, libc::EPERM; "restricted")]
#[test_case(NetworkSeccompMode::ProxyRouted(ProxyRouting::Explicit), libc::EPERM; "explicit_proxy")]
#[test_case(NetworkSeccompMode::ProxyRouted(ProxyRouting::TransparentTcp), libc::EPERM; "transparent_tcp")]
#[test_case(NetworkSeccompMode::VmSocketRestricted, libc::ENOSYS; "vm_socket_only")]
fn request_key_respects_network_policy(mode: NetworkSeccompMode, expected_errno: i32) {
    // Filters apply only to this thread, leaving the test runner unrestricted.
    std::thread::spawn(move || {
        set_no_new_privs().expect("set no_new_privs");

        // Intercept the syscall before it can invoke a host helper. A distinct
        // errno also prevents inherited EPERM filters from hiding regressions:
        // the most recently installed matching ERRNO filter takes precedence.
        let baseline: BpfProgram = SeccompFilter::new(
            BTreeMap::from([(libc::SYS_request_key, vec![])]),
            SeccompAction::Allow,
            SeccompAction::Errno(libc::ENOSYS as u32),
            std::env::consts::ARCH
                .try_into()
                .expect("supported architecture"),
        )
        .expect("build baseline filter")
        .try_into()
        .expect("compile baseline filter");
        apply_filter(&baseline).expect("install baseline filter");

        let request_key_result = || {
            // Null pointers also keep the probe harmless if it reaches the kernel.
            let result = unsafe {
                libc::syscall(
                    libc::SYS_request_key,
                    std::ptr::null::<libc::c_char>(),
                    std::ptr::null::<libc::c_char>(),
                    std::ptr::null::<libc::c_char>(),
                    0 as libc::c_long,
                )
            };
            (result, std::io::Error::last_os_error().raw_os_error())
        };
        assert_eq!(request_key_result(), (-1, Some(libc::ENOSYS)));

        install_network_seccomp_filter_on_current_thread(mode, /*managed_network*/ None)
            .expect("install network filter");
        assert_eq!(request_key_result(), (-1, Some(expected_errno)));
    })
    .join()
    .expect("seccomp probe thread");
}

#[test]
fn managed_network_enforces_seccomp_even_for_full_network_policy() {
    assert_eq!(
        should_install_network_seccomp(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ true,
        ),
        true
    );
}

#[test]
fn full_network_policy_without_managed_network_skips_seccomp() {
    assert_eq!(
        should_install_network_seccomp(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ false,
        ),
        false
    );
}

#[test]
fn restricted_network_policy_always_installs_seccomp() {
    assert!(should_install_network_seccomp(
        NetworkSandboxPolicy::Restricted,
        /*allow_network_for_proxy*/ false,
    ));
    assert!(should_install_network_seccomp(
        NetworkSandboxPolicy::Restricted,
        /*allow_network_for_proxy*/ true,
    ));
}

#[test]
fn managed_proxy_routes_use_proxy_routed_seccomp_mode() {
    assert_eq!(
        network_seccomp_mode(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ true,
            Some(ProxyRouting::Explicit),
        ),
        Some(NetworkSeccompMode::ProxyRouted(ProxyRouting::Explicit))
    );
}

#[test]
fn restricted_network_without_proxy_routing_uses_restricted_mode() {
    assert_eq!(
        network_seccomp_mode(
            NetworkSandboxPolicy::Restricted,
            /*allow_network_for_proxy*/ false,
            /*proxy_routed_network*/ None,
        ),
        Some(NetworkSeccompMode::Restricted)
    );
}

#[test]
fn full_network_without_managed_proxy_skips_network_seccomp_mode() {
    assert_eq!(
        network_seccomp_mode(
            NetworkSandboxPolicy::Enabled,
            /*allow_network_for_proxy*/ false,
            /*proxy_routed_network*/ None,
        ),
        None
    );
}

#[cfg(target_arch = "x86_64")]
fn syscall_result(nr: i64, arg0: i64) -> std::result::Result<i64, i32> {
    // Invalid remaining arguments keep the probes from accessing another process
    // or opening a connection. Seccomp must reject denied calls before dispatch.
    let result = unsafe { libc::syscall(nr, arg0, -1_i64, 0_i64, 0_i64, 0_i64, 0_i64) };
    if result == -1 {
        Err(std::io::Error::last_os_error()
            .raw_os_error()
            .expect("syscall failure has an errno"))
    } else {
        Ok(result)
    }
}

#[cfg(target_arch = "x86_64")]
#[test_case::test_case(NetworkSeccompMode::Restricted; "restricted")]
#[test_case::test_case(NetworkSeccompMode::ProxyRouted(ProxyRouting::Explicit); "explicit_proxy")]
#[test_case::test_case(NetworkSeccompMode::ProxyRouted(ProxyRouting::TransparentTcp); "transparent_tcp")]
#[test_case::test_case(NetworkSeccompMode::VmSocketRestricted; "vm_socket_restricted")]
fn x32_restricted_syscalls_are_denied(mode: NetworkSeccompMode) {
    std::thread::spawn(move || {
        super::install_network_seccomp_filter_on_current_thread(
            mode, /*managed_network*/ None,
        )
        .expect("install seccomp filter on the disposable thread");

        let mut denied = vec![
            (libc::SYS_io_uring_setup, 425),
            (libc::SYS_io_uring_enter, 426),
            (libc::SYS_io_uring_register, 427),
        ];
        if mode != NetworkSeccompMode::VmSocketRestricted {
            denied.extend([
                (libc::SYS_ptrace, 521),
                (libc::SYS_process_vm_readv, 539),
                (libc::SYS_process_vm_writev, 540),
            ]);
        }
        if mode == NetworkSeccompMode::Restricted {
            denied.extend([
                (libc::SYS_connect, 42),
                (libc::SYS_accept, 43),
                (libc::SYS_accept4, 288),
                (libc::SYS_bind, 49),
                (libc::SYS_listen, 50),
                (libc::SYS_getpeername, 52),
                (libc::SYS_getsockname, 51),
                (libc::SYS_shutdown, 48),
                (libc::SYS_sendto, 44),
                (libc::SYS_recvmmsg, 537),
                (libc::SYS_sendmmsg, 538),
                (libc::SYS_setsockopt, 541),
                (libc::SYS_getsockopt, 542),
            ]);
        }
        for (native_nr, x32_nr) in denied {
            for nr in [native_nr, native_nr | 0x4000_0000, x32_nr | 0x4000_0000] {
                assert_eq!(
                    syscall_result(nr, /*arg0*/ -1),
                    Err(libc::EPERM),
                    "syscall {nr:#x}"
                );
            }
            if x32_nr != native_nr {
                // These unflagged aliases were callable before Linux 5.4.
                assert_eq!(syscall_result(x32_nr, /*arg0*/ -1), Err(libc::EPERM));
            }
        }
    })
    .join()
    .expect("seccomp probe thread should succeed");
}

#[cfg(target_arch = "x86_64")]
#[test_case::test_case(NetworkSeccompMode::Restricted; "restricted")]
#[test_case::test_case(NetworkSeccompMode::ProxyRouted(ProxyRouting::Explicit); "explicit_proxy")]
#[test_case::test_case(NetworkSeccompMode::ProxyRouted(ProxyRouting::TransparentTcp); "transparent_tcp")]
#[test_case::test_case(NetworkSeccompMode::VmSocketRestricted; "vm_socket_restricted")]
fn x32_preserves_allowed_calls_and_socket_conditions(mode: NetworkSeccompMode) {
    std::thread::spawn(move || {
        let socket_domain = match mode {
            NetworkSeccompMode::Restricted | NetworkSeccompMode::VmSocketRestricted => {
                libc::AF_UNIX
            }
            NetworkSeccompMode::ProxyRouted(_) => libc::AF_INET,
        };
        let mut allowed = vec![
            (libc::SYS_getpid, 0),
            (libc::SYS_socket, i64::from(socket_domain)),
            (libc::SYS_socketpair, i64::from(libc::AF_UNIX)),
        ];
        if mode == NetworkSeccompMode::ProxyRouted(ProxyRouting::TransparentTcp) {
            // syscall_result supplies protocol zero (NETLINK_ROUTE).
            allowed.push((libc::SYS_socket, i64::from(libc::AF_NETLINK)));
        }
        let probes = allowed
            .into_iter()
            .flat_map(|(nr, arg0)| [(nr, arg0), (nr | 0x4000_0000, arg0)])
            .collect::<Vec<_>>();
        // On kernels without x32, permitted x32 calls must keep returning ENOSYS,
        // not EPERM. On x32 kernels, getpid must still return the process ID.
        let expected = probes
            .iter()
            .map(|&(nr, arg0)| syscall_result(nr, arg0))
            .collect::<Vec<_>>();
        super::install_network_seccomp_filter_on_current_thread(
            mode, /*managed_network*/ None,
        )
        .expect("install seccomp filter on the disposable thread");
        let actual = probes
            .iter()
            .map(|&(nr, arg0)| syscall_result(nr, arg0))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        for nr in [libc::SYS_socket, libc::SYS_socketpair] {
            for nr in [nr, nr | 0x4000_0000] {
                assert_eq!(
                    syscall_result(nr, i64::from(libc::AF_VSOCK)),
                    Err(libc::EPERM)
                );
            }
        }
    })
    .join()
    .expect("seccomp probe thread should succeed");
}
