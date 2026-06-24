use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceReport {
    user_namespace: String,
    mount_namespace: String,
    pid_namespace: String,
    network_namespace: String,
    ipc_namespace: String,
    uts_namespace: String,
    proc_pid_one_namespace: Option<String>,
    proc_file_system_type: String,
    overflow_uid: String,
    overflow_gid: String,
    effective_uid: u32,
    effective_gid: u32,
    effective_capabilities: String,
    permitted_capabilities: String,
    inheritable_capabilities: String,
    ambient_capabilities: String,
    no_new_privileges: u32,
    seccomp_mode: u32,
    connect_succeeded: Option<bool>,
    allowed_write_succeeded: Option<bool>,
    denied_read_succeeded: Option<bool>,
    denied_write_succeeded: Option<bool>,
}

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut connect = None;
    let mut allowed_write = None;
    let mut denied_write = None;
    let mut exit_code = 0;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--connect") => {
                connect = Some(
                    args.next()
                        .ok_or("--connect requires an address")?
                        .into_string()
                        .map_err(|_| "connect address must be valid UTF-8")?,
                );
            }
            Some("--allowed-write") => {
                allowed_write = Some(PathBuf::from(
                    args.next().ok_or("--allowed-write requires a path")?,
                ));
            }
            Some("--denied-write") => {
                denied_write = Some(PathBuf::from(
                    args.next().ok_or("--denied-write requires a path")?,
                ));
            }
            Some("--exit-code") => {
                exit_code = args
                    .next()
                    .ok_or("--exit-code requires a value")?
                    .to_str()
                    .ok_or("--exit-code must be valid UTF-8")?
                    .parse::<u8>()?;
            }
            _ => return Err(format!("unexpected argument: {}", arg.to_string_lossy()).into()),
        }
    }

    let status = std::fs::read_to_string("/proc/self/status")?;
    let denied_read_succeeded = denied_write
        .as_deref()
        .map(|path| std::fs::read(path).is_ok());
    let report = NamespaceReport {
        user_namespace: namespace("user")?,
        mount_namespace: namespace("mnt")?,
        pid_namespace: namespace("pid")?,
        network_namespace: namespace("net")?,
        ipc_namespace: namespace("ipc")?,
        uts_namespace: namespace("uts")?,
        proc_pid_one_namespace: std::fs::read_link("/proc/1/ns/pid")
            .ok()
            .map(|path| path.to_string_lossy().into_owned()),
        proc_file_system_type: proc_file_system_type()?,
        overflow_uid: std::fs::read_to_string("/proc/sys/kernel/overflowuid")?
            .trim()
            .to_string(),
        overflow_gid: std::fs::read_to_string("/proc/sys/kernel/overflowgid")?
            .trim()
            .to_string(),
        effective_uid: status_id(&status, "Uid")?,
        effective_gid: status_id(&status, "Gid")?,
        effective_capabilities: status_field(&status, "CapEff")?.to_string(),
        permitted_capabilities: status_field(&status, "CapPrm")?.to_string(),
        inheritable_capabilities: status_field(&status, "CapInh")?.to_string(),
        ambient_capabilities: status_field(&status, "CapAmb")?.to_string(),
        no_new_privileges: status_number(&status, "NoNewPrivs")?,
        seccomp_mode: status_number(&status, "Seccomp")?,
        connect_succeeded: connect
            .map(|address| connect_to(address.as_str()))
            .transpose()?,
        allowed_write_succeeded: allowed_write.map(|path| append_probe(path.as_path())),
        denied_read_succeeded,
        denied_write_succeeded: denied_write.map(|path| append_probe(path.as_path())),
    };

    println!("{}", serde_json::to_string(&report)?);
    std::io::stdout().flush()?;
    Ok(ExitCode::from(exit_code))
}

fn namespace(name: &str) -> std::io::Result<String> {
    std::fs::read_link(format!("/proc/self/ns/{name}"))
        .map(|path| path.to_string_lossy().into_owned())
}

fn proc_file_system_type() -> Result<String, Box<dyn std::error::Error>> {
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")?;
    for line in mountinfo.lines() {
        let Some((mount, file_system)) = line.split_once(" - ") else {
            continue;
        };
        if mount.split_whitespace().nth(4) == Some("/proc") {
            return file_system
                .split_whitespace()
                .next()
                .map(str::to_string)
                .ok_or_else(|| "proc mount is missing its filesystem type".into());
        }
    }
    Err("/proc is not present in mountinfo".into())
}

fn status_field<'a>(status: &'a str, name: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}:")))
        .map(str::trim)
        .ok_or_else(|| format!("/proc/self/status is missing {name}").into())
}

fn status_number(status: &str, name: &str) -> Result<u32, Box<dyn std::error::Error>> {
    status_field(status, name)?
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("/proc/self/status field {name} is empty"))?
        .parse()
        .map_err(Into::into)
}

fn status_id(status: &str, name: &str) -> Result<u32, Box<dyn std::error::Error>> {
    status_field(status, name)?
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("/proc/self/status field {name} has no effective ID"))?
        .parse()
        .map_err(Into::into)
}

fn connect_to(address: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let address: SocketAddr = address.parse()?;
    Ok(TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok())
}

fn append_probe(path: &std::path::Path) -> bool {
    OpenOptions::new()
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(b"changed"))
        .is_ok()
}
