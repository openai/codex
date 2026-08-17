use std::env;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

use super::super::CheckStatus;
use super::super::DoctorCheck;
use super::platform::desktop_check;

const MAX_EVENTS_PER_CHANNEL: usize = 256;
const MAX_RENDERED_EVENTS: usize = 16;
const MAX_EVENT_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
type Channel = (&'static str, &'static str, &'static [u32]);
type Evidence = (CheckStatus, String);

const CHANNELS: [Channel; 5] = [
    (
        "microsoft_defender",
        "Microsoft-Windows-Windows Defender/Operational",
        &[
            1006, 1007, 1116, 1117, 1121, 1122, 1123, 1124, 1125, 1126, 1127, 1128,
        ],
    ),
    (
        "applocker",
        "Microsoft-Windows-AppLocker/EXE and DLL",
        &[8003, 8004],
    ),
    (
        "applocker",
        "Microsoft-Windows-AppLocker/Packaged app-Execution",
        &[8021, 8022],
    ),
    (
        "applocker",
        "Microsoft-Windows-AppLocker/MSI and Script",
        &[8006, 8007],
    ),
    (
        "windows_app_control",
        "Microsoft-Windows-CodeIntegrity/Operational",
        &[3076, 3077],
    ),
];

pub(super) async fn collect() -> DoctorCheck {
    let Some(system_root) = env::var_os("SystemRoot") else {
        return classify(&[]);
    };
    let wevtutil = Path::new(&system_root).join("System32/wevtutil.exe");
    let mut channels = Vec::with_capacity(CHANNELS.len());
    for pair in CHANNELS.chunks(/*chunk_size*/ 2) {
        let first = query_channel(&wevtutil, pair[0]);
        if let Some(second) = pair.get(/*index*/ 1) {
            let (first, second) = tokio::join!(first, query_channel(&wevtutil, *second));
            channels.extend([first, second]);
        } else {
            channels.push(first.await);
        }
    }
    classify(&channels)
}

async fn query_channel(wevtutil: &Path, channel: Channel) -> Option<Vec<Evidence>> {
    let events = channel
        .2
        .iter()
        .map(|id| format!("EventID={id}"))
        .collect::<Vec<_>>()
        .join(" or ");
    let mut child = Command::new(wevtutil)
        .args(["qe", channel.1])
        .arg(format!(
            "/q:*[System[({events}) and TimeCreated[timediff(@SystemTime) <= 604800000]]]"
        ))
        .arg(format!("/c:{MAX_EVENTS_PER_CHANNEL}"))
        .args(["/rd:true", "/f:xml"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let mut output = Vec::new();
    let status = timeout(Duration::from_secs(10), async {
        let mut reader = child.stdout.take()?.take(MAX_EVENT_OUTPUT_BYTES as u64 + 1);
        reader.read_to_end(&mut output).await.ok()?;
        if output.len() > MAX_EVENT_OUTPUT_BYTES {
            return None;
        }
        child.wait().await.ok()
    })
    .await
    .ok()
    .flatten()?;
    status
        .success()
        .then(|| parse_events(&String::from_utf8_lossy(&output), channel))
}

fn parse_events(xml: &str, (source, _, ids): Channel) -> Vec<Evidence> {
    xml.split("</Event>")
        .take(MAX_EVENTS_PER_CHANNEL)
        .filter_map(|event| {
            let id = element(event, "EventID")?.parse().ok()?;
            if !ids.contains(&id) {
                return None;
            }
            let target = event
                .split("<Data")
                .skip(/*n*/ 1)
                .filter_map(|data| data.split_once('>')?.1.split_once("</Data>").map(|v| v.0))
                .chain(
                    ["FilePath", "FileName", "Image", "PackageName", "PackageFamilyName"]
                        .into_iter()
                        .filter_map(|name| element(event, name)),
                )
                .find_map(event_target)?;
            let timestamp = event.split_once("SystemTime=\"")?.1.split_once('"')?.0;
            let (status, action) = match id {
                1121 | 1123 | 1126 | 1127 | 8004 | 8007 | 8022 | 3077 => {
                    (CheckStatus::Fail, "blocked")
                }
                1122 | 1124 | 1125 | 1128 | 8003 | 8006 | 8021 | 3076 => {
                    (CheckStatus::Warning, "audited")
                }
                1006 | 1116 => (CheckStatus::Warning, "detected"),
                1007 | 1117 => defender_action(event),
                _ => return None,
            };
            Some((
                status,
                format!(
                    "source: {source}; event: {id}; target: {target}; action: {action}; time: {timestamp}"
                ),
            ))
        })
        .collect()
}

fn event_target(value: &str) -> Option<&'static str> {
    let value = value.to_ascii_lowercase();
    let parts = value.split(['\\', '/']).map(str::trim).collect::<Vec<_>>();
    let name = *parts.last()?;
    let package = parts.iter().any(|part| part.starts_with("openai.codex_"));
    let trusted = package || parts.windows(2).any(|pair| pair == ["openai", "codex"]);
    match name {
        "codex-windows-sandbox-setup.exe" => Some("sandbox_setup"),
        "codex-command-runner.exe" => Some("command_runner"),
        "codex.exe" => Some("codex"),
        "codex-desktop.exe" => Some("codex_desktop"),
        "chatgpt.exe" | "electron.exe" if trusted => Some("codex_desktop"),
        "rg.exe" if trusted => Some("ripgrep"),
        _ if package => Some("codex_desktop_package"),
        _ => None,
    }
}

fn defender_action(event: &str) -> (CheckStatus, &'static str) {
    let action = [
        "Action Name",
        "ActionName",
        "Action",
        "Action ID",
        "ActionID",
    ]
    .into_iter()
    .find_map(|name| data_value(event, name))
    .or_else(|| element(event, "ActionName"))
    .unwrap_or_default()
    .to_ascii_lowercase();
    match action.as_str() {
        "quarantine" | "2" => (CheckStatus::Fail, "quarantined"),
        "clean" | "remove" | "block" | "1" | "3" | "10" => (CheckStatus::Fail, "blocked"),
        "allow" | "ignore" | "none" | "no action" | "user defined" | "6" | "8" | "9" | "11" => {
            (CheckStatus::Ok, "allowed")
        }
        _ => (CheckStatus::Warning, "detected"),
    }
}

fn classify(channels: &[Option<Vec<Evidence>>]) -> DoctorCheck {
    let visible = channels.iter().any(Option::is_some);
    let mut status = if visible {
        CheckStatus::Ok
    } else {
        CheckStatus::Warning
    };
    let mut details = Vec::new();
    for (event_status, detail) in channels.iter().flatten().flatten() {
        status = status.max(*event_status);
        if details.len() < MAX_RENDERED_EVENTS {
            details.push(detail.clone());
        }
    }
    let summary = match (status, visible) {
        (CheckStatus::Ok, _) if channels.contains(&None) => "security event coverage is incomplete",
        (CheckStatus::Ok, _) => "no locally visible recent Codex security enforcement was found",
        (CheckStatus::Warning, false) => "security event channels could not be inspected",
        (CheckStatus::Warning, true) => "recent Codex security audit or detection requires review",
        (CheckStatus::Fail, _) => "endpoint security blocked or quarantined a Codex executable",
    };
    let mut check = desktop_check("desktop.security.enforcement", status, summary).details(details);
    if status != CheckStatus::Ok {
        check = check.remediation(
            "ask your organization's security administrator to review endpoint security events and the approved Codex application policy",
        );
    }
    check
}

fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let body = xml.split_once(&format!("<{name}"))?.1.split_once('>')?.1;
    Some(body.split_once(&format!("</{name}>"))?.0.trim())
}

fn data_value<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let body = xml
        .split_once(&format!("Name=\"{name}\""))?
        .1
        .split_once('>')?
        .1;
    Some(body.split_once("</Data>")?.0.trim())
}

#[cfg(test)]
#[path = "windows_security_tests.rs"]
mod tests;
