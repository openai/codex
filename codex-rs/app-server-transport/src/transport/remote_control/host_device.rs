#[cfg(any(target_os = "macos", test))]
use serde::Deserialize;

pub(super) const REMOTE_CONTROL_HOST_DEVICE_KIND_HEADER: &str = "x-codex-host-device-kind";
#[cfg(any(target_os = "macos", test))]
const MAC_MINI_HOST_DEVICE_KIND: &str = "mac_mini";

#[cfg(any(target_os = "macos", test))]
#[derive(Deserialize)]
struct MacHardwareProfile {
    #[serde(rename = "SPHardwareDataType")]
    hardware: Vec<MacHardware>,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Deserialize)]
struct MacHardware {
    machine_name: String,
}

#[cfg(any(target_os = "macos", test))]
fn host_device_kind_from_profile(profile: &[u8]) -> serde_json::Result<Option<&'static str>> {
    let profile: MacHardwareProfile = serde_json::from_slice(profile)?;
    Ok(profile
        .hardware
        .first()
        .is_some_and(|hardware| hardware.machine_name == "Mac mini")
        .then_some(MAC_MINI_HOST_DEVICE_KIND))
}

#[cfg(target_os = "macos")]
pub(super) async fn host_device_kind() -> Option<&'static str> {
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::process::Command;
    use tokio::sync::OnceCell;

    static HOST_DEVICE_KIND: OnceCell<Option<&'static str>> = OnceCell::const_new();

    HOST_DEVICE_KIND
        .get_or_try_init(|| async {
            let output = tokio::time::timeout(
                Duration::from_secs(2),
                Command::new("/usr/sbin/system_profiler")
                    .args(["-detailLevel", "mini", "SPHardwareDataType", "-json"])
                    .stdin(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;

            if !output.status.success() {
                return Err(());
            }

            host_device_kind_from_profile(&output.stdout).map_err(|_| ())
        })
        .await
        .ok()
        .copied()
        .flatten()
}

#[cfg(not(target_os = "macos"))]
pub(super) async fn host_device_kind() -> Option<&'static str> {
    None
}

#[cfg(test)]
#[path = "host_device_tests.rs"]
mod tests;
