use codex_core::config::Config;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use tracing::warn;

use crate::app_event::RealtimeAudioDeviceKind;

pub(crate) fn list_realtime_audio_device_names(
    kind: RealtimeAudioDeviceKind,
) -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let mut device_names = Vec::new();
    for device in devices(&host, kind)? {
        let Ok(name) = device.name() else {
            continue;
        };
        if !device_names.contains(&name) {
            device_names.push(name);
        }
    }
    Ok(device_names)
}

pub(crate) fn select_configured_input_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(RealtimeAudioDeviceKind::Microphone, config)
}

pub(crate) fn select_configured_output_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(RealtimeAudioDeviceKind::Speaker, config)
}

fn select_device_and_config(
    kind: RealtimeAudioDeviceKind,
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();
    let configured_name = configured_name(kind, config);
    let selected = configured_name
        .and_then(|name| find_device_by_name(&host, kind, name))
        .or_else(|| {
            let default_device = default_device(&host, kind);
            if let Some(name) = configured_name && default_device.is_some() {
                warn!(
                    "configured {} audio device `{name}` was unavailable; falling back to system default",
                    kind.noun()
                );
            }
            default_device
        })
        .ok_or_else(|| missing_device_error(kind, configured_name))?;

    let stream_config = default_config(&selected, kind)?;
    Ok((selected, stream_config))
}

fn configured_name(kind: RealtimeAudioDeviceKind, config: &Config) -> Option<&str> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => config.realtime_audio.microphone.as_deref(),
        RealtimeAudioDeviceKind::Speaker => config.realtime_audio.speaker.as_deref(),
    }
}

fn find_device_by_name(
    host: &cpal::Host,
    kind: RealtimeAudioDeviceKind,
    name: &str,
) -> Option<cpal::Device> {
    let devices = devices(host, kind).ok()?;
    devices
        .into_iter()
        .find(|device| device.name().ok().as_deref() == Some(name))
}

fn devices(host: &cpal::Host, kind: RealtimeAudioDeviceKind) -> Result<Vec<cpal::Device>, String> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => host
            .input_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate input audio devices: {err}")),
        RealtimeAudioDeviceKind::Speaker => host
            .output_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate output audio devices: {err}")),
    }
}

fn default_device(host: &cpal::Host, kind: RealtimeAudioDeviceKind) -> Option<cpal::Device> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => host.default_input_device(),
        RealtimeAudioDeviceKind::Speaker => host.default_output_device(),
    }
}

fn default_config(
    device: &cpal::Device,
    kind: RealtimeAudioDeviceKind,
) -> Result<cpal::SupportedStreamConfig, String> {
    match kind {
        RealtimeAudioDeviceKind::Microphone => device
            .default_input_config()
            .map_err(|err| format!("failed to get default input config: {err}")),
        RealtimeAudioDeviceKind::Speaker => device
            .default_output_config()
            .map_err(|err| format!("failed to get default output config: {err}")),
    }
}

fn missing_device_error(kind: RealtimeAudioDeviceKind, configured_name: Option<&str>) -> String {
    match (kind, configured_name) {
        (RealtimeAudioDeviceKind::Microphone, Some(name)) => {
            format!(
                "configured microphone `{name}` was unavailable and no default input audio device was found"
            )
        }
        (RealtimeAudioDeviceKind::Speaker, Some(name)) => {
            format!(
                "configured speaker `{name}` was unavailable and no default output audio device was found"
            )
        }
        (RealtimeAudioDeviceKind::Microphone, None) => {
            "no input audio device available".to_string()
        }
        (RealtimeAudioDeviceKind::Speaker, None) => "no output audio device available".to_string(),
    }
}
