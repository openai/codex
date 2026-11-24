//! Verify bridge task lifetime and mount cleanup with live proxy connections.

use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::io;
use std::os::unix::process::ExitStatusExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::time::timeout;

#[derive(Clone, Copy, Debug)]
enum Finish {
    Exit(u8),
    KillGroup,
    KillLauncher,
}

struct LiveProxy {
    child: Child,
    upstreams: Vec<TcpStream>,
    socks_listener: TcpListener,
}

fn managed_command(cwd: &Path, profile: &PermissionProfile) -> Command {
    let mut env = create_env_from_core_vars();
    strip_proxy_env(&mut env);
    env.remove(PROXY_ATTRIBUTION_TOKEN_ENV_KEY);
    linux_sandbox_launcher(cwd, profile, /*managed_network*/ None, env)
}

impl LiveProxy {
    async fn start(mut command: Command, connections: usize) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let socks_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let script = r#"
set -eu
proxy=${HTTP_PROXY#*://}
while :; do
    exec {fd}<>/dev/tcp/${proxy%:*}/${proxy##*:}
    printf R >&"$fd"
    IFS= read -r -u "$fd" reply
    case $reply in
        next) ;;
        socks) proxy=${ALL_PROXY#*://} ;;
        *) exit "$reply" ;;
    esac
done
"#;
        let context = ManagedNetworkSandboxContext {
            loopback_ports: vec![
                listener.local_addr()?.port(),
                socks_listener.local_addr()?.port(),
            ],
            ..Default::default()
        };
        let child = command
            .arg("--managed-network")
            .arg(serde_json::to_string(&context)?)
            .env("HTTP_PROXY", format!("http://{}", listener.local_addr()?))
            .env(
                "ALL_PROXY",
                format!("socks5://{}", socks_listener.local_addr()?),
            )
            .process_group(/*pgroup*/ 0)
            .kill_on_drop(true)
            .args(["--", "bash", "-c", script])
            .spawn()?;
        let mut upstreams = Vec::new();
        for n in 1..=connections {
            let (mut stream, _) = listener.accept().await?;
            assert_eq!(stream.read_u8().await?, b'R');
            if n < connections {
                stream.write_all(b"next\n").await?;
            }
            upstreams.push(stream);
        }
        Ok(Self {
            child,
            upstreams,
            socks_listener,
        })
    }

    fn pid(&self) -> i32 {
        self.child.id().unwrap().try_into().unwrap()
    }

    async fn finish(mut self, action: Finish) -> io::Result<()> {
        let pid = self.pid();
        let stream = self.upstreams.last_mut().unwrap();
        let expected = match action {
            Finish::Exit(code) => {
                stream.write_all(format!("{code}\n").as_bytes()).await?;
                (Some(i32::from(code)), None)
            }
            Finish::KillGroup => {
                assert_eq!(unsafe { libc::killpg(pid, libc::SIGKILL) }, 0);
                (None, Some(libc::SIGKILL))
            }
            Finish::KillLauncher => {
                self.child.start_kill()?;
                (None, Some(libc::SIGKILL))
            }
        };
        let output = self.child.wait_with_output().await?;
        assert_eq!(
            (output.status.code(), output.status.signal()),
            expected,
            "{action:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for stream in &mut self.upstreams {
            assert_eq!(stream.read(&mut [0]).await?, 0, "proxy stayed open");
        }
        Ok(())
    }
}

fn host_bridge_tasks(launcher: i32) -> io::Result<Vec<i32>> {
    let mut tasks = Vec::new();
    for thread in fs::read_dir(format!("/proc/{launcher}/task"))? {
        let thread = thread?;
        let tid = thread.file_name().to_str().unwrap().parse::<i32>().unwrap();
        if tid != launcher {
            tasks.push(tid);
        }
        // Include forked bridges. Bubblewrap runs in a separate process group.
        for child in fs::read_to_string(thread.path().join("children"))?.split_whitespace() {
            let pid = child.parse().unwrap();
            if unsafe { libc::getpgid(pid) } == launcher {
                tasks.push(pid);
            }
        }
    }
    Ok(tasks)
}

#[tokio::test]
async fn reaps_bridges_after_exit_and_cancellation() -> io::Result<()> {
    const OBSERVER: &str = "CODEX_TEST_BRIDGE_OBSERVER";
    if std::env::var_os(OBSERVER).is_some() {
        timeout(SANDBOX_TIMEOUT, observe_bridge_teardown()).await??;
        println!("verified bridge teardown");
        return Ok(());
    }
    if let Some(reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {reason}");
        return Ok(());
    }
    // Subreaper state is process-wide. Re-run this test in its own process so PID 1
    // cannot collect leaked children before the test inspects them.
    let name = std::thread::current().name().unwrap().to_owned();
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", &name, "--nocapture"])
        .env(OBSERVER, "1")
        .kill_on_drop(true)
        .output()
        .await?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("verified bridge teardown"));
    Ok(())
}

async fn observe_bridge_teardown() -> io::Result<()> {
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    let tmp = tempfile::tempdir()?;
    let mut leaks = Vec::new();
    for profile in [
        PermissionProfile::workspace_write(),
        PermissionProfile::Disabled,
    ] {
        for ending in [Finish::Exit(0), Finish::KillGroup, Finish::KillLauncher] {
            let command = managed_command(tmp.path(), &profile);
            let proxy = LiveProxy::start(command, /*connections*/ 1).await?;
            let tasks = host_bridge_tasks(proxy.pid())?;
            assert!(tasks.len() >= 2, "expected active and idle bridge tasks");
            proxy.finish(ending).await?;
            for pid in tasks {
                if Path::new(&format!("/proc/{pid}")).try_exists()? {
                    leaks.push((ending, pid));
                }
            }
            while unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) } > 0 {}
        }
    }
    assert!(leaks.is_empty(), "bridge tasks survived: {leaks:?}");

    // This caller previously exec'd bwrap directly. Its inherited signal
    // disposition must still allow the supervisor to recover the command's exit status.
    let mut command = managed_command(tmp.path(), &PermissionProfile::Disabled);
    command.arg("--inherit-pid-namespace");
    unsafe {
        command.pre_exec(|| {
            if libc::signal(libc::SIGCHLD, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let proxy = LiveProxy::start(command, /*connections*/ 1).await?;
    proxy.finish(Finish::Exit(37)).await?;

    // Linux sends the parent-death signal when the thread that forked the launcher exits.
    let mut command = managed_command(tmp.path(), &PermissionProfile::Disabled);
    command.arg("--inherit-pid-namespace");
    let runtime = tokio::runtime::Handle::current();
    let (started, proxy) = tokio::sync::oneshot::channel();
    let caller = std::thread::spawn(move || {
        let proxy = runtime.block_on(async {
            timeout(
                OPERATION_TIMEOUT,
                LiveProxy::start(command, /*connections*/ 1),
            )
            .await?
        });
        let _ = started.send(proxy);
    });
    let mut proxy = proxy.await.expect("caller returned the active proxy")?;
    caller.join().expect("caller thread exited");
    let output = timeout(OPERATION_TIMEOUT, proxy.child.wait_with_output())
        .await
        .expect("launcher survived its caller")?;
    assert_eq!(output.status.signal(), Some(libc::SIGKILL));
    assert_eq!(
        proxy.upstreams[0].read(&mut [0]).await?,
        0,
        "proxy survived its caller"
    );
    Ok(())
}

#[tokio::test]
async fn proxy_fd_pressure_preserves_socks_and_mount_cleanup() -> io::Result<()> {
    if let Some(reason) = managed_proxy_skip_reason().await {
        eprintln!("skipping managed proxy test: {reason}");
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    let protected = temp.path().join(".codex");
    timeout(SANDBOX_TIMEOUT, async {
        for (http_connections, fd_limit, open_socks) in [(3, 20, true), (8, 16, false)] {
            let command = managed_command(temp.path(), &PermissionProfile::workspace_write());
            let mut proxy = LiveProxy::start(command, http_connections).await?;
            assert!(
                protected.try_exists()?,
                "expected a live synthetic mount target"
            );
            let limit = libc::rlimit {
                rlim_cur: fd_limit,
                rlim_max: fd_limit,
            };
            let pid = proxy.pid();
            let result =
                unsafe { libc::prlimit(pid, libc::RLIMIT_NOFILE, &limit, std::ptr::null_mut()) };
            assert_eq!(result, 0, "prlimit: {}", io::Error::last_os_error());
            if open_socks {
                proxy
                    .upstreams
                    .last_mut()
                    .unwrap()
                    .write_all(b"socks\n")
                    .await?;
                let (mut socks, _) = timeout(OPERATION_TIMEOUT, proxy.socks_listener.accept())
                    .await
                    .expect("SOCKS upstream should accept while HTTP connections are open")?;
                assert_eq!(socks.read_u8().await?, b'R');
                proxy.upstreams.push(socks);
            }
            proxy.finish(Finish::Exit(37)).await?;
            assert!(!protected.try_exists()?, "synthetic mount target survived");
        }
        Ok(())
    })
    .await?
}
