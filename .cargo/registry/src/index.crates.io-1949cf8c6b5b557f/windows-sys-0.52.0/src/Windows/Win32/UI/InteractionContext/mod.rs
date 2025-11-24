::windows_targets::link!("ninput.dll" "system" fn AddPointerInteractionContext(interactioncontext : HINTERACTIONCONTEXT, pointerid : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Input_Pointer", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("ninput.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Input_Pointer\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn BufferPointerPacketsInteractionContext(interactioncontext : HINTERACTIONCONTEXT, entriescount : u32, pointerinfo : *const super::Input::Pointer:: POINTER_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn CreateInteractionContext(interactioncontext : *mut HINTERACTIONCONTEXT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn DestroyInteractionContext(interactioncontext : HINTERACTIONCONTEXT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetCrossSlideParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, threshold : CROSS_SLIDE_THRESHOLD, distance : *mut f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetHoldParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : HOLD_PARAMETER, value : *mut f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetInertiaParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, inertiaparameter : INERTIA_PARAMETER, value : *mut f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetInteractionConfigurationInteractionContext(interactioncontext : HINTERACTIONCONTEXT, configurationcount : u32, configuration : *mut INTERACTION_CONTEXT_CONFIGURATION) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetMouseWheelParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : MOUSE_WHEEL_PARAMETER, value : *mut f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetPropertyInteractionContext(interactioncontext : HINTERACTIONCONTEXT, contextproperty : INTERACTION_CONTEXT_PROPERTY, value : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Input_Pointer", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("ninput.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Input_Pointer\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn GetStateInteractionContext(interactioncontext : HINTERACTIONCONTEXT, pointerinfo : *const super::Input::Pointer:: POINTER_INFO, state : *mut INTERACTION_STATE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetTapParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : TAP_PARAMETER, value : *mut f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn GetTranslationParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : TRANSLATION_PARAMETER, value : *mut f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn ProcessBufferedPacketsInteractionContext(interactioncontext : HINTERACTIONCONTEXT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn ProcessInertiaInteractionContext(interactioncontext : HINTERACTIONCONTEXT) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Input_Pointer", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("ninput.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Input_Pointer\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn ProcessPointerFramesInteractionContext(interactioncontext : HINTERACTIONCONTEXT, entriescount : u32, pointercount : u32, pointerinfo : *const super::Input::Pointer:: POINTER_INFO) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ninput.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn RegisterOutputCallbackInteractionContext(interactioncontext : HINTERACTIONCONTEXT, outputcallback : INTERACTION_CONTEXT_OUTPUT_CALLBACK, clientdata : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
::windows_targets::link!("ninput.dll" "system" #[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"] fn RegisterOutputCallbackInteractionContext2(interactioncontext : HINTERACTIONCONTEXT, outputcallback : INTERACTION_CONTEXT_OUTPUT_CALLBACK2, clientdata : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn RemovePointerInteractionContext(interactioncontext : HINTERACTIONCONTEXT, pointerid : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn ResetInteractionContext(interactioncontext : HINTERACTIONCONTEXT) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetCrossSlideParametersInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parametercount : u32, crossslideparameters : *const CROSS_SLIDE_PARAMETER) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetHoldParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : HOLD_PARAMETER, value : f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetInertiaParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, inertiaparameter : INERTIA_PARAMETER, value : f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetInteractionConfigurationInteractionContext(interactioncontext : HINTERACTIONCONTEXT, configurationcount : u32, configuration : *const INTERACTION_CONTEXT_CONFIGURATION) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetMouseWheelParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : MOUSE_WHEEL_PARAMETER, value : f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetPivotInteractionContext(interactioncontext : HINTERACTIONCONTEXT, x : f32, y : f32, radius : f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetPropertyInteractionContext(interactioncontext : HINTERACTIONCONTEXT, contextproperty : INTERACTION_CONTEXT_PROPERTY, value : u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetTapParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : TAP_PARAMETER, value : f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn SetTranslationParameterInteractionContext(interactioncontext : HINTERACTIONCONTEXT, parameter : TRANSLATION_PARAMETER, value : f32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("ninput.dll" "system" fn StopInteractionContext(interactioncontext : HINTERACTIONCONTEXT) -> ::windows_sys::core::HRESULT);
pub const CROSS_SLIDE_FLAGS_MAX: CROSS_SLIDE_FLAGS = 4294967295u32;
pub const CROSS_SLIDE_FLAGS_NONE: CROSS_SLIDE_FLAGS = 0u32;
pub const CROSS_SLIDE_FLAGS_REARRANGE: CROSS_SLIDE_FLAGS = 4u32;
pub const CROSS_SLIDE_FLAGS_SELECT: CROSS_SLIDE_FLAGS = 1u32;
pub const CROSS_SLIDE_FLAGS_SPEED_BUMP: CROSS_SLIDE_FLAGS = 2u32;
pub const CROSS_SLIDE_THRESHOLD_COUNT: CROSS_SLIDE_THRESHOLD = 4i32;
pub const CROSS_SLIDE_THRESHOLD_MAX: CROSS_SLIDE_THRESHOLD = -1i32;
pub const CROSS_SLIDE_THRESHOLD_REARRANGE_START: CROSS_SLIDE_THRESHOLD = 3i32;
pub const CROSS_SLIDE_THRESHOLD_SELECT_START: CROSS_SLIDE_THRESHOLD = 0i32;
pub const CROSS_SLIDE_THRESHOLD_SPEED_BUMP_END: CROSS_SLIDE_THRESHOLD = 2i32;
pub const CROSS_SLIDE_THRESHOLD_SPEED_BUMP_START: CROSS_SLIDE_THRESHOLD = 1i32;
pub const HOLD_PARAMETER_MAX: HOLD_PARAMETER = -1i32;
pub const HOLD_PARAMETER_MAX_CONTACT_COUNT: HOLD_PARAMETER = 1i32;
pub const HOLD_PARAMETER_MIN_CONTACT_COUNT: HOLD_PARAMETER = 0i32;
pub const HOLD_PARAMETER_THRESHOLD_RADIUS: HOLD_PARAMETER = 2i32;
pub const HOLD_PARAMETER_THRESHOLD_START_DELAY: HOLD_PARAMETER = 3i32;
pub const INERTIA_PARAMETER_EXPANSION_DECELERATION: INERTIA_PARAMETER = 5i32;
pub const INERTIA_PARAMETER_EXPANSION_EXPANSION: INERTIA_PARAMETER = 6i32;
pub const INERTIA_PARAMETER_MAX: INERTIA_PARAMETER = -1i32;
pub const INERTIA_PARAMETER_ROTATION_ANGLE: INERTIA_PARAMETER = 4i32;
pub const INERTIA_PARAMETER_ROTATION_DECELERATION: INERTIA_PARAMETER = 3i32;
pub const INERTIA_PARAMETER_TRANSLATION_DECELERATION: INERTIA_PARAMETER = 1i32;
pub const INERTIA_PARAMETER_TRANSLATION_DISPLACEMENT: INERTIA_PARAMETER = 2i32;
pub const INTERACTION_CONFIGURATION_FLAG_CROSS_SLIDE: INTERACTION_CONFIGURATION_FLAGS = 1u32;
pub const INTERACTION_CONFIGURATION_FLAG_CROSS_SLIDE_EXACT: INTERACTION_CONFIGURATION_FLAGS = 32u32;
pub const INTERACTION_CONFIGURATION_FLAG_CROSS_SLIDE_HORIZONTAL: INTERACTION_CONFIGURATION_FLAGS = 2u32;
pub const INTERACTION_CONFIGURATION_FLAG_CROSS_SLIDE_REARRANGE: INTERACTION_CONFIGURATION_FLAGS = 16u32;
pub const INTERACTION_CONFIGURATION_FLAG_CROSS_SLIDE_SELECT: INTERACTION_CONFIGURATION_FLAGS = 4u32;
pub const INTERACTION_CONFIGURATION_FLAG_CROSS_SLIDE_SPEED_BUMP: INTERACTION_CONFIGURATION_FLAGS = 8u32;
pub const INTERACTION_CONFIGURATION_FLAG_DRAG: INTERACTION_CONFIGURATION_FLAGS = 1u32;
pub const INTERACTION_CONFIGURATION_FLAG_HOLD: INTERACTION_CONFIGURATION_FLAGS = 1u32;
pub const INTERACTION_CONFIGURATION_FLAG_HOLD_MOUSE: INTERACTION_CONFIGURATION_FLAGS = 2u32;
pub const INTERACTION_CONFIGURATION_FLAG_HOLD_MULTIPLE_FINGER: INTERACTION_CONFIGURATION_FLAGS = 4u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION: INTERACTION_CONFIGURATION_FLAGS = 1u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_EXACT: INTERACTION_CONFIGURATION_FLAGS = 1024u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_MULTIPLE_FINGER_PANNING: INTERACTION_CONFIGURATION_FLAGS = 2048u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_RAILS_X: INTERACTION_CONFIGURATION_FLAGS = 256u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_RAILS_Y: INTERACTION_CONFIGURATION_FLAGS = 512u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_ROTATION: INTERACTION_CONFIGURATION_FLAGS = 8u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_ROTATION_INERTIA: INTERACTION_CONFIGURATION_FLAGS = 64u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_SCALING: INTERACTION_CONFIGURATION_FLAGS = 16u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_SCALING_INERTIA: INTERACTION_CONFIGURATION_FLAGS = 128u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_TRANSLATION_INERTIA: INTERACTION_CONFIGURATION_FLAGS = 32u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_TRANSLATION_X: INTERACTION_CONFIGURATION_FLAGS = 2u32;
pub const INTERACTION_CONFIGURATION_FLAG_MANIPULATION_TRANSLATION_Y: INTERACTION_CONFIGURATION_FLAGS = 4u32;
pub const INTERACTION_CONFIGURATION_FLAG_MAX: INTERACTION_CONFIGURATION_FLAGS = 4294967295u32;
pub const INTERACTION_CONFIGURATION_FLAG_NONE: INTERACTION_CONFIGURATION_FLAGS = 0u32;
pub const INTERACTION_CONFIGURATION_FLAG_SECONDARY_TAP: INTERACTION_CONFIGURATION_FLAGS = 1u32;
pub const INTERACTION_CONFIGURATION_FLAG_TAP: INTERACTION_CONFIGURATION_FLAGS = 1u32;
pub const INTERACTION_CONFIGURATION_FLAG_TAP_DOUBLE: INTERACTION_CONFIGURATION_FLAGS = 2u32;
pub const INTERACTION_CONFIGURATION_FLAG_TAP_MULTIPLE_FINGER: INTERACTION_CONFIGURATION_FLAGS = 4u32;
pub const INTERACTION_CONTEXT_PROPERTY_FILTER_POINTERS: INTERACTION_CONTEXT_PROPERTY = 3i32;
pub const INTERACTION_CONTEXT_PROPERTY_INTERACTION_UI_FEEDBACK: INTERACTION_CONTEXT_PROPERTY = 2i32;
pub const INTERACTION_CONTEXT_PROPERTY_MAX: INTERACTION_CONTEXT_PROPERTY = -1i32;
pub const INTERACTION_CONTEXT_PROPERTY_MEASUREMENT_UNITS: INTERACTION_CONTEXT_PROPERTY = 1i32;
pub const INTERACTION_FLAG_BEGIN: INTERACTION_FLAGS = 1u32;
pub const INTERACTION_FLAG_CANCEL: INTERACTION_FLAGS = 4u32;
pub const INTERACTION_FLAG_END: INTERACTION_FLAGS = 2u32;
pub const INTERACTION_FLAG_INERTIA: INTERACTION_FLAGS = 8u32;
pub const INTERACTION_FLAG_MAX: INTERACTION_FLAGS = 4294967295u32;
pub const INTERACTION_FLAG_NONE: INTERACTION_FLAGS = 0u32;
pub const INTERACTION_ID_CROSS_SLIDE: INTERACTION_ID = 6i32;
pub const INTERACTION_ID_DRAG: INTERACTION_ID = 5i32;
pub const INTERACTION_ID_HOLD: INTERACTION_ID = 4i32;
pub const INTERACTION_ID_MANIPULATION: INTERACTION_ID = 1i32;
pub const INTERACTION_ID_MAX: INTERACTION_ID = -1i32;
pub const INTERACTION_ID_NONE: INTERACTION_ID = 0i32;
pub const INTERACTION_ID_SECONDARY_TAP: INTERACTION_ID = 3i32;
pub const INTERACTION_ID_TAP: INTERACTION_ID = 2i32;
pub const INTERACTION_STATE_IDLE: INTERACTION_STATE = 0i32;
pub const INTERACTION_STATE_IN_INTERACTION: INTERACTION_STATE = 1i32;
pub const INTERACTION_STATE_MAX: INTERACTION_STATE = -1i32;
pub const INTERACTION_STATE_POSSIBLE_DOUBLE_TAP: INTERACTION_STATE = 2i32;
pub const MANIPULATION_RAILS_STATE_FREE: MANIPULATION_RAILS_STATE = 1i32;
pub const MANIPULATION_RAILS_STATE_MAX: MANIPULATION_RAILS_STATE = -1i32;
pub const MANIPULATION_RAILS_STATE_RAILED: MANIPULATION_RAILS_STATE = 2i32;
pub const MANIPULATION_RAILS_STATE_UNDECIDED: MANIPULATION_RAILS_STATE = 0i32;
pub const MOUSE_WHEEL_PARAMETER_CHAR_TRANSLATION_X: MOUSE_WHEEL_PARAMETER = 1i32;
pub const MOUSE_WHEEL_PARAMETER_CHAR_TRANSLATION_Y: MOUSE_WHEEL_PARAMETER = 2i32;
pub const MOUSE_WHEEL_PARAMETER_DELTA_ROTATION: MOUSE_WHEEL_PARAMETER = 4i32;
pub const MOUSE_WHEEL_PARAMETER_DELTA_SCALE: MOUSE_WHEEL_PARAMETER = 3i32;
pub const MOUSE_WHEEL_PARAMETER_MAX: MOUSE_WHEEL_PARAMETER = -1i32;
pub const MOUSE_WHEEL_PARAMETER_PAGE_TRANSLATION_X: MOUSE_WHEEL_PARAMETER = 5i32;
pub const MOUSE_WHEEL_PARAMETER_PAGE_TRANSLATION_Y: MOUSE_WHEEL_PARAMETER = 6i32;
pub const TAP_PARAMETER_MAX: TAP_PARAMETER = -1i32;
pub const TAP_PARAMETER_MAX_CONTACT_COUNT: TAP_PARAMETER = 1i32;
pub const TAP_PARAMETER_MIN_CONTACT_COUNT: TAP_PARAMETER = 0i32;
pub const TRANSLATION_PARAMETER_MAX: TRANSLATION_PARAMETER = -1i32;
pub const TRANSLATION_PARAMETER_MAX_CONTACT_COUNT: TRANSLATION_PARAMETER = 1i32;
pub const TRANSLATION_PARAMETER_MIN_CONTACT_COUNT: TRANSLATION_PARAMETER = 0i32;
pub type CROSS_SLIDE_FLAGS = u32;
pub type CROSS_SLIDE_THRESHOLD = i32;
pub type HOLD_PARAMETER = i32;
pub type INERTIA_PARAMETER = i32;
pub type INTERACTION_CONFIGURATION_FLAGS = u32;
pub type INTERACTION_CONTEXT_PROPERTY = i32;
pub type INTERACTION_FLAGS = u32;
pub type INTERACTION_ID = i32;
pub type INTERACTION_STATE = i32;
pub type MANIPULATION_RAILS_STATE = i32;
pub type MOUSE_WHEEL_PARAMETER = i32;
pub type TAP_PARAMETER = i32;
pub type TRANSLATION_PARAMETER = i32;
#[repr(C)]
pub struct CROSS_SLIDE_PARAMETER {
    pub threshold: CROSS_SLIDE_THRESHOLD,
    pub distance: f32,
}
impl ::core::marker::Copy for CROSS_SLIDE_PARAMETER {}
impl ::core::clone::Clone for CROSS_SLIDE_PARAMETER {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HINTERACTIONCONTEXT = isize;
#[repr(C)]
pub struct INTERACTION_ARGUMENTS_CROSS_SLIDE {
    pub flags: CROSS_SLIDE_FLAGS,
}
impl ::core::marker::Copy for INTERACTION_ARGUMENTS_CROSS_SLIDE {}
impl ::core::clone::Clone for INTERACTION_ARGUMENTS_CROSS_SLIDE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INTERACTION_ARGUMENTS_MANIPULATION {
    pub delta: MANIPULATION_TRANSFORM,
    pub cumulative: MANIPULATION_TRANSFORM,
    pub velocity: MANIPULATION_VELOCITY,
    pub railsState: MANIPULATION_RAILS_STATE,
}
impl ::core::marker::Copy for INTERACTION_ARGUMENTS_MANIPULATION {}
impl ::core::clone::Clone for INTERACTION_ARGUMENTS_MANIPULATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INTERACTION_ARGUMENTS_TAP {
    pub count: u32,
}
impl ::core::marker::Copy for INTERACTION_ARGUMENTS_TAP {}
impl ::core::clone::Clone for INTERACTION_ARGUMENTS_TAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INTERACTION_CONTEXT_CONFIGURATION {
    pub interactionId: INTERACTION_ID,
    pub enable: INTERACTION_CONFIGURATION_FLAGS,
}
impl ::core::marker::Copy for INTERACTION_CONTEXT_CONFIGURATION {}
impl ::core::clone::Clone for INTERACTION_CONTEXT_CONFIGURATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub struct INTERACTION_CONTEXT_OUTPUT {
    pub interactionId: INTERACTION_ID,
    pub interactionFlags: INTERACTION_FLAGS,
    pub inputType: super::WindowsAndMessaging::POINTER_INPUT_TYPE,
    pub x: f32,
    pub y: f32,
    pub arguments: INTERACTION_CONTEXT_OUTPUT_0,
}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::marker::Copy for INTERACTION_CONTEXT_OUTPUT {}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::clone::Clone for INTERACTION_CONTEXT_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub union INTERACTION_CONTEXT_OUTPUT_0 {
    pub manipulation: INTERACTION_ARGUMENTS_MANIPULATION,
    pub tap: INTERACTION_ARGUMENTS_TAP,
    pub crossSlide: INTERACTION_ARGUMENTS_CROSS_SLIDE,
}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::marker::Copy for INTERACTION_CONTEXT_OUTPUT_0 {}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::clone::Clone for INTERACTION_CONTEXT_OUTPUT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub struct INTERACTION_CONTEXT_OUTPUT2 {
    pub interactionId: INTERACTION_ID,
    pub interactionFlags: INTERACTION_FLAGS,
    pub inputType: super::WindowsAndMessaging::POINTER_INPUT_TYPE,
    pub contactCount: u32,
    pub currentContactCount: u32,
    pub x: f32,
    pub y: f32,
    pub arguments: INTERACTION_CONTEXT_OUTPUT2_0,
}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::marker::Copy for INTERACTION_CONTEXT_OUTPUT2 {}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::clone::Clone for INTERACTION_CONTEXT_OUTPUT2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub union INTERACTION_CONTEXT_OUTPUT2_0 {
    pub manipulation: INTERACTION_ARGUMENTS_MANIPULATION,
    pub tap: INTERACTION_ARGUMENTS_TAP,
    pub crossSlide: INTERACTION_ARGUMENTS_CROSS_SLIDE,
}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::marker::Copy for INTERACTION_CONTEXT_OUTPUT2_0 {}
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
impl ::core::clone::Clone for INTERACTION_CONTEXT_OUTPUT2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MANIPULATION_TRANSFORM {
    pub translationX: f32,
    pub translationY: f32,
    pub scale: f32,
    pub expansion: f32,
    pub rotation: f32,
}
impl ::core::marker::Copy for MANIPULATION_TRANSFORM {}
impl ::core::clone::Clone for MANIPULATION_TRANSFORM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MANIPULATION_VELOCITY {
    pub velocityX: f32,
    pub velocityY: f32,
    pub velocityExpansion: f32,
    pub velocityAngular: f32,
}
impl ::core::marker::Copy for MANIPULATION_VELOCITY {}
impl ::core::clone::Clone for MANIPULATION_VELOCITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub type INTERACTION_CONTEXT_OUTPUT_CALLBACK = ::core::option::Option<unsafe extern "system" fn(clientdata: *const ::core::ffi::c_void, output: *const INTERACTION_CONTEXT_OUTPUT) -> ()>;
#[doc = "Required features: `\"Win32_UI_WindowsAndMessaging\"`"]
#[cfg(feature = "Win32_UI_WindowsAndMessaging")]
pub type INTERACTION_CONTEXT_OUTPUT_CALLBACK2 = ::core::option::Option<unsafe extern "system" fn(clientdata: *const ::core::ffi::c_void, output: *const INTERACTION_CONTEXT_OUTPUT2) -> ()>;
