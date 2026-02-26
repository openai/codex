use codex_core::config::Config;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use tracing::warn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AudioDeviceKind {
    Input,
    Output,
}

impl AudioDeviceKind {
    fn noun(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }

    fn configured_name(self, config: &Config) -> Option<&str> {
        match self {
            Self::Input => config.realtime_audio.microphone.as_deref(),
            Self::Output => config.realtime_audio.speaker.as_deref(),
        }
    }
}

pub(crate) fn select_configured_input_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(AudioDeviceKind::Input, config)
}

pub(crate) fn select_configured_output_device_and_config(
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    select_device_and_config(AudioDeviceKind::Output, config)
}

fn select_device_and_config(
    kind: AudioDeviceKind,
    config: &Config,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();
    let configured_name = kind.configured_name(config);
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

fn find_device_by_name(
    host: &cpal::Host,
    kind: AudioDeviceKind,
    name: &str,
) -> Option<cpal::Device> {
    let devices = devices(host, kind).ok()?;
    devices
        .into_iter()
        .find(|device| device.name().ok().as_deref() == Some(name))
}

fn devices(host: &cpal::Host, kind: AudioDeviceKind) -> Result<Vec<cpal::Device>, String> {
    match kind {
        AudioDeviceKind::Input => host
            .input_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate input audio devices: {err}")),
        AudioDeviceKind::Output => host
            .output_devices()
            .map(|devices| devices.collect())
            .map_err(|err| format!("failed to enumerate output audio devices: {err}")),
    }
}

fn default_device(host: &cpal::Host, kind: AudioDeviceKind) -> Option<cpal::Device> {
    match kind {
        AudioDeviceKind::Input => host.default_input_device(),
        AudioDeviceKind::Output => host.default_output_device(),
    }
}

fn default_config(
    device: &cpal::Device,
    kind: AudioDeviceKind,
) -> Result<cpal::SupportedStreamConfig, String> {
    match kind {
        AudioDeviceKind::Input => device
            .default_input_config()
            .map_err(|err| format!("failed to get default input config: {err}")),
        AudioDeviceKind::Output => device
            .default_output_config()
            .map_err(|err| format!("failed to get default output config: {err}")),
    }
}

fn missing_device_error(kind: AudioDeviceKind, configured_name: Option<&str>) -> String {
    match (kind, configured_name) {
        (AudioDeviceKind::Input, Some(name)) => format!(
            "configured input audio device `{name}` was unavailable and no default input audio device was found"
        ),
        (AudioDeviceKind::Output, Some(name)) => format!(
            "configured output audio device `{name}` was unavailable and no default output audio device was found"
        ),
        (AudioDeviceKind::Input, None) => "no input audio device available".to_string(),
        (AudioDeviceKind::Output, None) => "no output audio device available".to_string(),
    }
}
