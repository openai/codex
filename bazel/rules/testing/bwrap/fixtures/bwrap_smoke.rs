#[cfg(not(target_os = "linux"))]
compile_error!("the bwrap smoke fixture can only run on Linux");

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;

/// Namespace classes whose topology is part of this harness's contract.
///
/// The outer bwrap isolates user, mount, PID, IPC, and UTS, but deliberately
/// inherits the VM's writable procfs for nested bwrap setup and its network for
/// loopback. Cgroup and time namespaces stay executor-owned; `*_for_children`
/// entries are alternate PID/time handles rather than separate namespace
/// classes.
const NAMESPACES: &[&str] = &["user", "mnt", "pid", "ipc", "uts", "net"];

fn main() -> Result<()> {
    let mut args = env::args_os();
    let _executable = args.next();
    let mode = args.next().context("missing bwrap smoke fixture mode")?;
    match mode.to_str() {
        Some("wait") => wait(),
        Some("spawn-detached-descendant") => spawn_detached_descendant(),
        Some("detached-wait") => detached_wait(),
        Some("inspect") => inspect(),
        Some("filesystem") => {
            let tmp_dir = args.next().context("missing tmp directory")?;
            let workspace_dir = args.next().context("missing workspace directory")?;
            let root_target = args.next().context("missing root write target")?;
            filesystem(
                Path::new(&tmp_dir),
                Path::new(&workspace_dir),
                Path::new(&root_target),
            )
        }
        Some("connect") => {
            let address = args.next().context("missing listener address")?;
            connect(address.to_string_lossy().as_ref())
        }
        Some("nested") => {
            let bwrap = args.next().context("missing nested bwrap path")?;
            let fixture = args.next().context("missing nested fixture path")?;
            nested(Path::new(&bwrap), Path::new(&fixture))
        }
        _ => anyhow::bail!("unknown bwrap smoke fixture mode: {mode:?}"),
    }
}

fn wait() -> Result<()> {
    println!("BWRAP_TEST_READY");
    std::io::stdout().flush()?;
    loop {
        std::thread::park();
    }
}

fn spawn_detached_descendant() -> Result<()> {
    let _descendant = Command::new(env::current_exe()?)
        .arg("detached-wait")
        .spawn()
        .context("spawn detached descendant")?;
    loop {
        std::thread::park();
    }
}

fn detached_wait() -> Result<()> {
    // SAFETY: setsid has no memory-safety preconditions. This fixture is a
    // forked child and therefore is not already a process-group leader.
    if unsafe { libc::setsid() } == -1 {
        return Err(std::io::Error::last_os_error()).context("detach descendant session");
    }
    let status = fs::read_to_string("/proc/self/status").context("read detached child status")?;
    let vm_pid = status_field(&status, "NSpid")?
        .split_whitespace()
        .next()
        .context("detached child status has no VM-visible PID")?;
    println!("BWRAP_TEST_DETACHED_DESCENDANT_READY={vm_pid}");
    std::io::stdout().flush()?;
    loop {
        std::thread::park();
    }
}

fn inspect() -> Result<()> {
    for (key, value) in inspection()? {
        println!("{key}={value}");
    }
    Ok(())
}

fn inspection() -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    let status = fs::read_to_string("/proc/self/status").context("read /proc/self/status")?;
    values.insert(
        "id.effective_uid".to_string(),
        status_id(&status, "Uid")?.to_string(),
    );
    values.insert(
        "id.effective_gid".to_string(),
        status_id(&status, "Gid")?.to_string(),
    );
    for (name, field) in [
        ("effective", "CapEff"),
        ("permitted", "CapPrm"),
        ("inheritable", "CapInh"),
        ("ambient", "CapAmb"),
    ] {
        values.insert(
            format!("cap.{name}"),
            status_field(&status, field)?.to_string(),
        );
    }
    for namespace in NAMESPACES {
        values.insert(
            format!("ns.{namespace}"),
            namespace_identity("self", namespace)?,
        );
    }
    match namespace_identity("1", "pid") {
        Ok(namespace) => {
            values.insert("proc.1.pid_ns".to_string(), namespace);
        }
        Err(error) => {
            values.insert("proc.1.pid_ns.error".to_string(), error.to_string());
        }
    }
    values.insert(
        "proc.self.pid_ns".to_string(),
        namespace_identity("self", "pid")?,
    );
    for name in ["overflowuid", "overflowgid"] {
        let value = fs::read_to_string(format!("/proc/sys/kernel/{name}"))
            .with_context(|| format!("read /proc/sys/kernel/{name}"))?;
        values.insert(format!("proc.{name}"), value.trim().to_string());
    }
    for name in ["HOME", "CODEX_HOME", "XDG_RUNTIME_DIR", "TMPDIR"] {
        let value = env::var(name).with_context(|| format!("read {name}"))?;
        values.insert(format!("env.{name}"), value);
    }
    Ok(values)
}

fn namespace_identity(pid: &str, namespace: &str) -> Result<String> {
    let path = format!("/proc/{pid}/ns/{namespace}");
    Ok(fs::read_link(&path)
        .with_context(|| format!("read namespace link {path}"))?
        .to_string_lossy()
        .into_owned())
}

fn status_field<'a>(status: &'a str, name: &str) -> Result<&'a str> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}:")))
        .map(str::trim)
        .with_context(|| format!("/proc/self/status is missing {name}"))
}

fn status_id(status: &str, name: &str) -> Result<u32> {
    status_field(status, name)?
        .split_whitespace()
        .nth(1)
        .with_context(|| format!("/proc/self/status field {name} has no effective ID"))?
        .parse()
        .with_context(|| format!("parse effective {name}"))
}

fn filesystem(tmp_dir: &Path, workspace_dir: &Path, root_target: &Path) -> Result<()> {
    fs::create_dir_all(tmp_dir)
        .with_context(|| format!("create private tmp fixture {}", tmp_dir.display()))?;
    println!(
        "tmp.host_sentinel_visible={}",
        tmp_dir.join("host-sentinel.txt").exists()
    );
    for (name, directory) in [("tmp", tmp_dir), ("workspace", workspace_dir)] {
        let writable_file = directory.join("round-trip.txt");
        fs::write(&writable_file, "round-trip")
            .with_context(|| format!("write {}", writable_file.display()))?;
        println!("{name}.value={}", fs::read_to_string(&writable_file)?);
    }

    let error = match fs::write(root_target, "must not be written") {
        Ok(()) => anyhow::bail!("writing through the read-only root unexpectedly succeeded"),
        Err(error) => error,
    };
    println!(
        "root.write_errno={}",
        error
            .raw_os_error()
            .context("root write error had no errno")?
    );
    Ok(())
}

fn connect(address: &str) -> Result<()> {
    let mut stream = TcpStream::connect(address)
        .with_context(|| format!("connect to parent listener at {address}"))?;
    stream.write_all(b"BWRAP_LOOPBACK")?;
    Ok(())
}

fn nested(bwrap: &Path, fixture: &Path) -> Result<()> {
    for (key, value) in inspection()? {
        println!("outer.{key}={value}");
    }
    let output = Command::new(bwrap)
        .args([
            "--new-session",
            "--die-with-parent",
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--unshare-user",
            "--unshare-pid",
            "--unshare-net",
            "--proc",
            "/proc",
            "--",
        ])
        .arg(fixture)
        .arg("inspect")
        .output()
        .context("run raw nested bwrap")?;
    anyhow::ensure!(
        output.status.success(),
        "raw nested bwrap exited with {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    for line in String::from_utf8(output.stdout)?.lines() {
        println!("child.{line}");
    }
    Ok(())
}
