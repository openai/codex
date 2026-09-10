//! Fixed helper exit phases for safe diagnostics when its framed output closes.
//! The same-build handshake keeps these codes aligned between parent and helper.

#[derive(Clone, Copy, Debug)]
#[repr(i32)]
pub enum HelperExitStage {
    ControlRead = 20,
    ControlQueue = 21,
    ParentGone = 22,
    Runtime = 23,
    Transport = 24,
    OpenDevices = 25,
    AudioIngress = 26,
    AudioService = 27,
    InspectAudio = 28,
    AudioControls = 29,
    Reply = 30,
    Shutdown = 31,
    ControlSequence = 32,
    Playout = 33,
    Render = 35,
    Capture = 36,
    Device = 37,
    Send = 39,
}

impl HelperExitStage {
    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            20 => Self::ControlRead,
            21 => Self::ControlQueue,
            22 => Self::ParentGone,
            23 => Self::Runtime,
            24 => Self::Transport,
            25 => Self::OpenDevices,
            26 => Self::AudioIngress,
            27 => Self::AudioService,
            28 => Self::InspectAudio,
            29 => Self::AudioControls,
            30 => Self::Reply,
            31 => Self::Shutdown,
            32 => Self::ControlSequence,
            33 => Self::Playout,
            35 => Self::Render,
            36 => Self::Capture,
            37 => Self::Device,
            39 => Self::Send,
            _ => return None,
        })
    }
}
