#[cfg(feature = "Win32_Media_Audio")]
#[doc = "Required features: `\"Win32_Media_Audio\"`"]
pub mod Audio;
#[cfg(feature = "Win32_Media_DxMediaObjects")]
#[doc = "Required features: `\"Win32_Media_DxMediaObjects\"`"]
pub mod DxMediaObjects;
#[cfg(feature = "Win32_Media_KernelStreaming")]
#[doc = "Required features: `\"Win32_Media_KernelStreaming\"`"]
pub mod KernelStreaming;
#[cfg(feature = "Win32_Media_Multimedia")]
#[doc = "Required features: `\"Win32_Media_Multimedia\"`"]
pub mod Multimedia;
#[cfg(feature = "Win32_Media_Streaming")]
#[doc = "Required features: `\"Win32_Media_Streaming\"`"]
pub mod Streaming;
#[cfg(feature = "Win32_Media_WindowsMediaFormat")]
#[doc = "Required features: `\"Win32_Media_WindowsMediaFormat\"`"]
pub mod WindowsMediaFormat;
::windows_targets::link!("winmm.dll" "system" fn timeBeginPeriod(uperiod : u32) -> u32);
::windows_targets::link!("winmm.dll" "system" fn timeEndPeriod(uperiod : u32) -> u32);
::windows_targets::link!("winmm.dll" "system" fn timeGetDevCaps(ptc : *mut TIMECAPS, cbtc : u32) -> u32);
::windows_targets::link!("winmm.dll" "system" fn timeGetSystemTime(pmmt : *mut MMTIME, cbmmt : u32) -> u32);
::windows_targets::link!("winmm.dll" "system" fn timeGetTime() -> u32);
::windows_targets::link!("winmm.dll" "system" fn timeKillEvent(utimerid : u32) -> u32);
::windows_targets::link!("winmm.dll" "system" fn timeSetEvent(udelay : u32, uresolution : u32, fptc : LPTIMECALLBACK, dwuser : usize, fuevent : u32) -> u32);
pub type IReferenceClock = *mut ::core::ffi::c_void;
pub type IReferenceClock2 = *mut ::core::ffi::c_void;
pub type IReferenceClockTimerControl = *mut ::core::ffi::c_void;
pub const ED_DEVCAP_ATN_READ: TIMECODE_SAMPLE_FLAGS = 5047u32;
pub const ED_DEVCAP_RTC_READ: TIMECODE_SAMPLE_FLAGS = 5050u32;
pub const ED_DEVCAP_TIMECODE_READ: TIMECODE_SAMPLE_FLAGS = 4121u32;
pub const JOYERR_BASE: u32 = 160u32;
pub const MAXERRORLENGTH: u32 = 256u32;
pub const MAXPNAMELEN: u32 = 32u32;
pub const MCIERR_BASE: u32 = 256u32;
pub const MCI_CD_OFFSET: u32 = 1088u32;
pub const MCI_SEQ_OFFSET: u32 = 1216u32;
pub const MCI_STRING_OFFSET: u32 = 512u32;
pub const MCI_VD_OFFSET: u32 = 1024u32;
pub const MCI_WAVE_OFFSET: u32 = 1152u32;
pub const MIDIERR_BASE: u32 = 64u32;
pub const MIXERR_BASE: u32 = 1024u32;
pub const MMSYSERR_ALLOCATED: u32 = 4u32;
pub const MMSYSERR_BADDB: u32 = 14u32;
pub const MMSYSERR_BADDEVICEID: u32 = 2u32;
pub const MMSYSERR_BADERRNUM: u32 = 9u32;
pub const MMSYSERR_BASE: u32 = 0u32;
pub const MMSYSERR_DELETEERROR: u32 = 18u32;
pub const MMSYSERR_ERROR: u32 = 1u32;
pub const MMSYSERR_HANDLEBUSY: u32 = 12u32;
pub const MMSYSERR_INVALFLAG: u32 = 10u32;
pub const MMSYSERR_INVALHANDLE: u32 = 5u32;
pub const MMSYSERR_INVALIDALIAS: u32 = 13u32;
pub const MMSYSERR_INVALPARAM: u32 = 11u32;
pub const MMSYSERR_KEYNOTFOUND: u32 = 15u32;
pub const MMSYSERR_LASTERROR: u32 = 21u32;
pub const MMSYSERR_MOREDATA: u32 = 21u32;
pub const MMSYSERR_NODRIVER: u32 = 6u32;
pub const MMSYSERR_NODRIVERCB: u32 = 20u32;
pub const MMSYSERR_NOERROR: u32 = 0u32;
pub const MMSYSERR_NOMEM: u32 = 7u32;
pub const MMSYSERR_NOTENABLED: u32 = 3u32;
pub const MMSYSERR_NOTSUPPORTED: u32 = 8u32;
pub const MMSYSERR_READERROR: u32 = 16u32;
pub const MMSYSERR_VALNOTFOUND: u32 = 19u32;
pub const MMSYSERR_WRITEERROR: u32 = 17u32;
pub const MM_ADLIB: u32 = 9u32;
pub const MM_DRVM_CLOSE: u32 = 977u32;
pub const MM_DRVM_DATA: u32 = 978u32;
pub const MM_DRVM_ERROR: u32 = 979u32;
pub const MM_DRVM_OPEN: u32 = 976u32;
pub const MM_JOY1BUTTONDOWN: u32 = 949u32;
pub const MM_JOY1BUTTONUP: u32 = 951u32;
pub const MM_JOY1MOVE: u32 = 928u32;
pub const MM_JOY1ZMOVE: u32 = 930u32;
pub const MM_JOY2BUTTONDOWN: u32 = 950u32;
pub const MM_JOY2BUTTONUP: u32 = 952u32;
pub const MM_JOY2MOVE: u32 = 929u32;
pub const MM_JOY2ZMOVE: u32 = 931u32;
pub const MM_MCINOTIFY: u32 = 953u32;
pub const MM_MCISIGNAL: u32 = 971u32;
pub const MM_MICROSOFT: u32 = 1u32;
pub const MM_MIDI_MAPPER: u32 = 1u32;
pub const MM_MIM_CLOSE: u32 = 962u32;
pub const MM_MIM_DATA: u32 = 963u32;
pub const MM_MIM_ERROR: u32 = 965u32;
pub const MM_MIM_LONGDATA: u32 = 964u32;
pub const MM_MIM_LONGERROR: u32 = 966u32;
pub const MM_MIM_MOREDATA: u32 = 972u32;
pub const MM_MIM_OPEN: u32 = 961u32;
pub const MM_MIXM_CONTROL_CHANGE: u32 = 977u32;
pub const MM_MIXM_LINE_CHANGE: u32 = 976u32;
pub const MM_MOM_CLOSE: u32 = 968u32;
pub const MM_MOM_DONE: u32 = 969u32;
pub const MM_MOM_OPEN: u32 = 967u32;
pub const MM_MOM_POSITIONCB: u32 = 970u32;
pub const MM_MPU401_MIDIIN: u32 = 11u32;
pub const MM_MPU401_MIDIOUT: u32 = 10u32;
pub const MM_PC_JOYSTICK: u32 = 12u32;
pub const MM_SNDBLST_MIDIIN: u32 = 4u32;
pub const MM_SNDBLST_MIDIOUT: u32 = 3u32;
pub const MM_SNDBLST_SYNTH: u32 = 5u32;
pub const MM_SNDBLST_WAVEIN: u32 = 7u32;
pub const MM_SNDBLST_WAVEOUT: u32 = 6u32;
pub const MM_STREAM_CLOSE: u32 = 981u32;
pub const MM_STREAM_DONE: u32 = 982u32;
pub const MM_STREAM_ERROR: u32 = 983u32;
pub const MM_STREAM_OPEN: u32 = 980u32;
pub const MM_WAVE_MAPPER: u32 = 2u32;
pub const MM_WIM_CLOSE: u32 = 959u32;
pub const MM_WIM_DATA: u32 = 960u32;
pub const MM_WIM_OPEN: u32 = 958u32;
pub const MM_WOM_CLOSE: u32 = 956u32;
pub const MM_WOM_DONE: u32 = 957u32;
pub const MM_WOM_OPEN: u32 = 955u32;
pub const TIMERR_BASE: u32 = 96u32;
pub const TIMERR_NOCANDO: u32 = 97u32;
pub const TIMERR_NOERROR: u32 = 0u32;
pub const TIMERR_STRUCT: u32 = 129u32;
pub const TIME_BYTES: u32 = 4u32;
pub const TIME_CALLBACK_EVENT_PULSE: u32 = 32u32;
pub const TIME_CALLBACK_EVENT_SET: u32 = 16u32;
pub const TIME_CALLBACK_FUNCTION: u32 = 0u32;
pub const TIME_KILL_SYNCHRONOUS: u32 = 256u32;
pub const TIME_MIDI: u32 = 16u32;
pub const TIME_MS: u32 = 1u32;
pub const TIME_ONESHOT: u32 = 0u32;
pub const TIME_PERIODIC: u32 = 1u32;
pub const TIME_SAMPLES: u32 = 2u32;
pub const TIME_SMPTE: u32 = 8u32;
pub const TIME_TICKS: u32 = 32u32;
pub const WAVERR_BASE: u32 = 32u32;
pub type TIMECODE_SAMPLE_FLAGS = u32;
pub type HTASK = isize;
#[repr(C, packed(1))]
pub struct MMTIME {
    pub wType: u32,
    pub u: MMTIME_0,
}
impl ::core::marker::Copy for MMTIME {}
impl ::core::clone::Clone for MMTIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union MMTIME_0 {
    pub ms: u32,
    pub sample: u32,
    pub cb: u32,
    pub ticks: u32,
    pub smpte: MMTIME_0_1,
    pub midi: MMTIME_0_0,
}
impl ::core::marker::Copy for MMTIME_0 {}
impl ::core::clone::Clone for MMTIME_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MMTIME_0_0 {
    pub songptrpos: u32,
}
impl ::core::marker::Copy for MMTIME_0_0 {}
impl ::core::clone::Clone for MMTIME_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MMTIME_0_1 {
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    pub frame: u8,
    pub fps: u8,
    pub dummy: u8,
    pub pad: [u8; 2],
}
impl ::core::marker::Copy for MMTIME_0_1 {}
impl ::core::clone::Clone for MMTIME_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TIMECAPS {
    pub wPeriodMin: u32,
    pub wPeriodMax: u32,
}
impl ::core::marker::Copy for TIMECAPS {}
impl ::core::clone::Clone for TIMECAPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union TIMECODE {
    pub Anonymous: TIMECODE_0,
    pub qw: u64,
}
impl ::core::marker::Copy for TIMECODE {}
impl ::core::clone::Clone for TIMECODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TIMECODE_0 {
    pub wFrameRate: u16,
    pub wFrameFract: u16,
    pub dwFrames: u32,
}
impl ::core::marker::Copy for TIMECODE_0 {}
impl ::core::clone::Clone for TIMECODE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TIMECODE_SAMPLE {
    pub qwTick: i64,
    pub timecode: TIMECODE,
    pub dwUser: u32,
    pub dwFlags: TIMECODE_SAMPLE_FLAGS,
}
impl ::core::marker::Copy for TIMECODE_SAMPLE {}
impl ::core::clone::Clone for TIMECODE_SAMPLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Media_Multimedia\"`"]
#[cfg(feature = "Win32_Media_Multimedia")]
pub type LPDRVCALLBACK = ::core::option::Option<unsafe extern "system" fn(hdrvr: Multimedia::HDRVR, umsg: u32, dwuser: usize, dw1: usize, dw2: usize) -> ()>;
pub type LPTIMECALLBACK = ::core::option::Option<unsafe extern "system" fn(utimerid: u32, umsg: u32, dwuser: usize, dw1: usize, dw2: usize) -> ()>;
