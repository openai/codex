//! Exercise native name resolution and ordinary TCP through the two-stage Linux launcher.

use super::*;
use codex_utils_cargo_bin::runfiles;
use pretty_assertions::assert_eq;
use socks5_protocol::CommandRequest;
use socks5_protocol::sync::FromIO;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn transparent_tcp_rejects_incompatible_options_and_untrusted_proxy_ports() {
    let incompatible =
        "transparent TCP requires managed networking and private bubblewrap namespaces";
    let untrusted = "outside the managed sandbox context";
    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.insert("ALL_PROXY".into(), "socks5h://127.0.0.1:32101".into());
    for (context, extra, reason) in [
        (None, None, incompatible),
        (Some("{}"), Some("--use-legacy-landlock"), incompatible),
        (Some("{}"), Some("--inherit-pid-namespace"), incompatible),
        (Some("{}"), Some("--no-proc"), incompatible),
        (Some("{}"), None, untrusted),
        (Some(r#"{"loopbackPorts":[32102]}"#), None, untrusted),
    ] {
        let mut options = vec!["--transparent-tcp"];
        options.extend(extra);
        let output = linux_sandbox_command_with_options(
            &["true"],
            &PermissionProfile::read_only(),
            context,
            env.clone(),
            &options,
        )
        .output()
        .await
        .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(reason),
            "{output:?}"
        );
    }
}

#[tokio::test]
async fn managed_tcp_native_client_uses_attributed_socks_and_preserves_restrictions() {
    // Container device policies can expose the node but deny opening it.
    if let Err(err) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/net/tun")
    {
        if matches!(
            err.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        ) || err.raw_os_error() == Some(libc::ENODEV)
        {
            eprintln!("skipping transparent TCP integration test: /dev/net/tun: {err}");
            return;
        }
        panic!("failed to open /dev/net/tun: {err}");
    }
    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    // Keep distro bwrap private to this child. Other tests exercise bundled discovery.
    // Stage outside the runfiles cwd, which the production launcher rejects for executables.
    let _bwrap_dir = if cfg!(target_arch = "x86_64") && option_env!("BAZEL_PACKAGE").is_some() {
        let bin = tempfile::tempdir().unwrap();
        let files = runfiles::Runfiles::create().expect("managed TCP test runfiles");
        let bwrap = runfiles::rlocation!(
            files,
            "managed_tcp_test_bubblewrap_noble_amd64/usr/bin/bwrap"
        )
        .expect("Ubuntu bubblewrap test fixture");
        codex_utils_cargo_bin::copy_executable(&bwrap, &bin.path().join("bwrap")).unwrap();
        let path = env.get("PATH").map(String::as_str).unwrap_or_default();
        env.insert("PATH".into(), format!("{path}:{}", bin.path().display()));
        Some(bin)
    } else {
        None
    };
    if let Some(reason) = managed_proxy_skip_reason_with_env(env.clone()).await {
        eprintln!("skipping managed TCP sandbox test: {reason}");
        return;
    }
    let proxy = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let proxy_addr = proxy.local_addr().unwrap();
    let host_only = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 2), 0)).unwrap();
    let host_port = host_only.local_addr().unwrap().port().to_string();
    let read_only = NamedTempFile::new_in(std::env::current_dir().unwrap()).unwrap();
    env.insert("ALL_PROXY".into(), format!("socks5h://{proxy_addr}"));
    env.insert(
        PROXY_ATTRIBUTION_TOKEN_ENV_KEY.into(),
        "native-client".into(),
    );
    let serve = |mut stdin: tokio::process::ChildStdin| async move {
        let mut kept = None;
        for (host, allow) in [
            ("pg.example:5432", true),
            ("pg.example:5432", true),
            ("[2001:db8::93]:5432", true),
            ("v6.example:5432", true),
            ("blocked.example:5432", false),
        ] {
            let (mut stream, _) = proxy.accept().await.unwrap();
            let mut preface = Vec::new();
            write_attribution_frame(&mut preface, "native-client").unwrap();
            preface.extend_from_slice(&[5, 1, 0]);
            let mut buf = vec![0; preface.len()];
            stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, preface);
            stream.write_all(&[5, 0]).await.unwrap();
            let mut expected = Vec::new();
            CommandRequest::connect(host.parse().unwrap())
                .write_to(&mut expected)
                .unwrap();
            let mut request = vec![0; expected.len()];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(request, expected);
            if host == "pg.example:5432" {
                let Some(first) = kept.take() else {
                    stdin.write_all(b"open\n").await.unwrap();
                    kept = Some(stream);
                    continue;
                };
                stdin.write_all(b"close\n").await.unwrap();
                assert_eq!(stream.read(&mut [0]).await.unwrap(), 0);
                stream = first;
            }
            let status = if allow { 0 } else { 2 };
            stream
                .write_all(&[5, status, 0, 1, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();
            if allow {
                let mut payload = [0; 4];
                stream.read_exact(&mut payload).await.unwrap();
                assert_eq!(&payload, b"ping");
                stream.write_all(b"pong\n").await.unwrap();
            }
        }
    };
    let script = r#"
        unset ALL_PROXY all_proxy
        if command -v ip >/dev/null; then
            ip link show codextun >/dev/null
            if ip link set codextun down 2>/dev/null; then exit 15; fi
        fi
        exec 3<>/dev/tcp/pg.example/5432
        printf ping >&3
        read -r open
        exec 4<>/dev/tcp/pg.example/5432
        printf queued >&4
        read -r release
        exec 4>&-
        IFS= read -r reply <&3 && test "$reply" = pong || exit 11
        exec 3>&-
        exec 3<>/dev/tcp/2001:db8::93/5432
        printf ping >&3
        IFS= read -r reply <&3 && test "$reply" = pong || exit 11
        exec 3>&-
        # Force an AAAA lookup so IPv4 fallback cannot satisfy the hostname check.
        python3 - <<'PY'
import socket
with socket.socket(socket.AF_INET6, socket.SOCK_STREAM) as client:
    client.connect(("v6.example", 5432))
    client.sendall(b"ping")
    assert client.makefile("rb").readline() == b"pong\n"
PY
        if { exec 4<>/dev/tcp/blocked.example/5432; } 2>/dev/null; then
            if IFS= read -r -t 2 reply <&4; then exit 12; fi
        fi
        if (exec 5<>/dev/tcp/127.0.0.2/"$1") 2>/dev/null; then exit 13; fi
        if (exec 5>>"$2") 2>/dev/null; then exit 14; fi
    "#;
    let mut command = linux_sandbox_command_with_options(
        &[
            "bash",
            "-ec",
            script,
            "probe",
            &host_port,
            read_only.path().to_str().unwrap(),
        ],
        &PermissionProfile::read_only(),
        Some(&format!(r#"{{"loopbackPorts":[{}]}}"#, proxy_addr.port())),
        env,
        &["--transparent-tcp"],
    );
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let peer = tokio::spawn(serve(child.stdin.take().unwrap()));
    let output = tokio::time::timeout(SANDBOX_TIMEOUT, child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    tokio::time::timeout(OPERATION_TIMEOUT, peer)
        .await
        .unwrap()
        .unwrap();
}
