use codex_core::config::Config;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use tracing::warn;

use crate::app_event::BidiAudioDeviceKind;

pub(crate) fn list_bidi_audio_device_names(
    kind: BidiAudioDeviceKind,
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
    select_device_and_config(BidiAudioDeviceKind::Microphone, config)
}

pub(crate) fn select_configured_output_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(BidiAudioDeviceKind::Speaker, config)
}

fn select_device_and_config(
    kind: BidiAudioDeviceKind,
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();
    let configured_name = configured_name(kind, config);
    let selected = configured_name
        .and_then(|name| find_device_by_name(&host, kind, name))
        .or_else(|| {
            let default_device = default_device(&host, kind);
            if let Some(name) = configured_name
                && default_device.is_some()
            {
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

fn configured_name(kind: BidiAudioDeviceKind, config: &Config) -> Option<&str> {
    match kind {
        BidiAudioDeviceKind::Microphone => config.bidi_audio.microphone.as_deref(),
        BidiAudioDeviceKind::Speaker => config.bidi_audio.speaker.as_deref(),
    }
}

fn find_device_by_name(
    host: &cpal::Host,
    kind: BidiAudioDeviceKind,
    name: &str,
) -> Option<cpal::Device> {
    let devices = devices(host, kind).ok()?;
    devices
        .into_iter()
        .find(|device| device.name().ok().as_deref() == Some(name))
}

fn devices(host: &cpal::Host, kind: BidiAudioDeviceKind) -> Result<Vec<cpal::Device>, String> {
    match kind {
        BidiAudioDeviceKind::Microphone => host
            .input_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate input audio devices: {err}")),
        BidiAudioDeviceKind::Speaker => host
            .output_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate output audio devices: {err}")),
    }
}

fn default_device(host: &cpal::Host, kind: BidiAudioDeviceKind) -> Option<cpal::Device> {
    match kind {
        BidiAudioDeviceKind::Microphone => host.default_input_device(),
        BidiAudioDeviceKind::Speaker => host.default_output_device(),
    }
}

fn default_config(
    device: &cpal::Device,
    kind: BidiAudioDeviceKind,
) -> Result<cpal::SupportedStreamConfig, String> {
    match kind {
        BidiAudioDeviceKind::Microphone => device
            .default_input_config()
            .map_err(|err| format!("failed to get default input config: {err}")),
        BidiAudioDeviceKind::Speaker => device
            .default_output_config()
            .map_err(|err| format!("failed to get default output config: {err}")),
    }
}

fn missing_device_error(kind: BidiAudioDeviceKind, configured_name: Option<&str>) -> String {
    match (kind, configured_name) {
        (BidiAudioDeviceKind::Microphone, Some(name)) => format!(
            "configured microphone `{name}` was unavailable and no default input audio device was found"
        ),
        (BidiAudioDeviceKind::Speaker, Some(name)) => format!(
            "configured speaker `{name}` was unavailable and no default output audio device was found"
        ),
        (BidiAudioDeviceKind::Microphone, None) => "no input audio device available".to_string(),
        (BidiAudioDeviceKind::Speaker, None) => "no output audio device available".to_string(),
    }
}
