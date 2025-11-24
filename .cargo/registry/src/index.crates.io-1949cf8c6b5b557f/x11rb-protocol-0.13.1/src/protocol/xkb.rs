// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `xkb` X11 extension.

#![allow(clippy::too_many_arguments)]
// The code generator is simpler if it can always use conversions
#![allow(clippy::useless_conversion)]

#[allow(unused_imports)]
use alloc::borrow::Cow;
#[allow(unused_imports)]
use core::convert::TryInto;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryFrom;
use crate::errors::ParseError;
#[allow(unused_imports)]
use crate::x11_utils::TryIntoUSize;
use crate::BufWithFds;
#[allow(unused_imports)]
use crate::utils::{RawFdContainer, pretty_print_bitmask, pretty_print_enum};
#[allow(unused_imports)]
use crate::x11_utils::{Request, RequestHeader, Serialize, TryParse, TryParseFd};
#[allow(unused_imports)]
use super::xproto;

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "XKEYBOARD";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 0);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Const(u8);
impl Const {
    pub const MAX_LEGAL_KEY_CODE: Self = Self(255);
    pub const PER_KEY_BIT_ARRAY_SIZE: Self = Self(32);
    pub const KEY_NAME_LENGTH: Self = Self(4);
}
impl From<Const> for u8 {
    #[inline]
    fn from(input: Const) -> Self {
        input.0
    }
}
impl From<Const> for Option<u8> {
    #[inline]
    fn from(input: Const) -> Self {
        Some(input.0)
    }
}
impl From<Const> for u16 {
    #[inline]
    fn from(input: Const) -> Self {
        u16::from(input.0)
    }
}
impl From<Const> for Option<u16> {
    #[inline]
    fn from(input: Const) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Const> for u32 {
    #[inline]
    fn from(input: Const) -> Self {
        u32::from(input.0)
    }
}
impl From<Const> for Option<u32> {
    #[inline]
    fn from(input: Const) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Const {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Const  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::MAX_LEGAL_KEY_CODE.0.into(), "MAX_LEGAL_KEY_CODE", "MaxLegalKeyCode"),
            (Self::PER_KEY_BIT_ARRAY_SIZE.0.into(), "PER_KEY_BIT_ARRAY_SIZE", "PerKeyBitArraySize"),
            (Self::KEY_NAME_LENGTH.0.into(), "KEY_NAME_LENGTH", "KeyNameLength"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventType(u16);
impl EventType {
    pub const NEW_KEYBOARD_NOTIFY: Self = Self(1 << 0);
    pub const MAP_NOTIFY: Self = Self(1 << 1);
    pub const STATE_NOTIFY: Self = Self(1 << 2);
    pub const CONTROLS_NOTIFY: Self = Self(1 << 3);
    pub const INDICATOR_STATE_NOTIFY: Self = Self(1 << 4);
    pub const INDICATOR_MAP_NOTIFY: Self = Self(1 << 5);
    pub const NAMES_NOTIFY: Self = Self(1 << 6);
    pub const COMPAT_MAP_NOTIFY: Self = Self(1 << 7);
    pub const BELL_NOTIFY: Self = Self(1 << 8);
    pub const ACTION_MESSAGE: Self = Self(1 << 9);
    pub const ACCESS_X_NOTIFY: Self = Self(1 << 10);
    pub const EXTENSION_DEVICE_NOTIFY: Self = Self(1 << 11);
}
impl From<EventType> for u16 {
    #[inline]
    fn from(input: EventType) -> Self {
        input.0
    }
}
impl From<EventType> for Option<u16> {
    #[inline]
    fn from(input: EventType) -> Self {
        Some(input.0)
    }
}
impl From<EventType> for u32 {
    #[inline]
    fn from(input: EventType) -> Self {
        u32::from(input.0)
    }
}
impl From<EventType> for Option<u32> {
    #[inline]
    fn from(input: EventType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for EventType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for EventType {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for EventType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NEW_KEYBOARD_NOTIFY.0.into(), "NEW_KEYBOARD_NOTIFY", "NewKeyboardNotify"),
            (Self::MAP_NOTIFY.0.into(), "MAP_NOTIFY", "MapNotify"),
            (Self::STATE_NOTIFY.0.into(), "STATE_NOTIFY", "StateNotify"),
            (Self::CONTROLS_NOTIFY.0.into(), "CONTROLS_NOTIFY", "ControlsNotify"),
            (Self::INDICATOR_STATE_NOTIFY.0.into(), "INDICATOR_STATE_NOTIFY", "IndicatorStateNotify"),
            (Self::INDICATOR_MAP_NOTIFY.0.into(), "INDICATOR_MAP_NOTIFY", "IndicatorMapNotify"),
            (Self::NAMES_NOTIFY.0.into(), "NAMES_NOTIFY", "NamesNotify"),
            (Self::COMPAT_MAP_NOTIFY.0.into(), "COMPAT_MAP_NOTIFY", "CompatMapNotify"),
            (Self::BELL_NOTIFY.0.into(), "BELL_NOTIFY", "BellNotify"),
            (Self::ACTION_MESSAGE.0.into(), "ACTION_MESSAGE", "ActionMessage"),
            (Self::ACCESS_X_NOTIFY.0.into(), "ACCESS_X_NOTIFY", "AccessXNotify"),
            (Self::EXTENSION_DEVICE_NOTIFY.0.into(), "EXTENSION_DEVICE_NOTIFY", "ExtensionDeviceNotify"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(EventType, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NKNDetail(u16);
impl NKNDetail {
    pub const KEYCODES: Self = Self(1 << 0);
    pub const GEOMETRY: Self = Self(1 << 1);
    pub const DEVICE_ID: Self = Self(1 << 2);
}
impl From<NKNDetail> for u16 {
    #[inline]
    fn from(input: NKNDetail) -> Self {
        input.0
    }
}
impl From<NKNDetail> for Option<u16> {
    #[inline]
    fn from(input: NKNDetail) -> Self {
        Some(input.0)
    }
}
impl From<NKNDetail> for u32 {
    #[inline]
    fn from(input: NKNDetail) -> Self {
        u32::from(input.0)
    }
}
impl From<NKNDetail> for Option<u32> {
    #[inline]
    fn from(input: NKNDetail) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for NKNDetail {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for NKNDetail {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for NKNDetail  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KEYCODES.0.into(), "KEYCODES", "Keycodes"),
            (Self::GEOMETRY.0.into(), "GEOMETRY", "Geometry"),
            (Self::DEVICE_ID.0.into(), "DEVICE_ID", "DeviceID"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(NKNDetail, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AXNDetail(u16);
impl AXNDetail {
    pub const SK_PRESS: Self = Self(1 << 0);
    pub const SK_ACCEPT: Self = Self(1 << 1);
    pub const SK_REJECT: Self = Self(1 << 2);
    pub const SK_RELEASE: Self = Self(1 << 3);
    pub const BK_ACCEPT: Self = Self(1 << 4);
    pub const BK_REJECT: Self = Self(1 << 5);
    pub const AXK_WARNING: Self = Self(1 << 6);
}
impl From<AXNDetail> for u16 {
    #[inline]
    fn from(input: AXNDetail) -> Self {
        input.0
    }
}
impl From<AXNDetail> for Option<u16> {
    #[inline]
    fn from(input: AXNDetail) -> Self {
        Some(input.0)
    }
}
impl From<AXNDetail> for u32 {
    #[inline]
    fn from(input: AXNDetail) -> Self {
        u32::from(input.0)
    }
}
impl From<AXNDetail> for Option<u32> {
    #[inline]
    fn from(input: AXNDetail) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for AXNDetail {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for AXNDetail {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for AXNDetail  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SK_PRESS.0.into(), "SK_PRESS", "SKPress"),
            (Self::SK_ACCEPT.0.into(), "SK_ACCEPT", "SKAccept"),
            (Self::SK_REJECT.0.into(), "SK_REJECT", "SKReject"),
            (Self::SK_RELEASE.0.into(), "SK_RELEASE", "SKRelease"),
            (Self::BK_ACCEPT.0.into(), "BK_ACCEPT", "BKAccept"),
            (Self::BK_REJECT.0.into(), "BK_REJECT", "BKReject"),
            (Self::AXK_WARNING.0.into(), "AXK_WARNING", "AXKWarning"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(AXNDetail, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapPart(u16);
impl MapPart {
    pub const KEY_TYPES: Self = Self(1 << 0);
    pub const KEY_SYMS: Self = Self(1 << 1);
    pub const MODIFIER_MAP: Self = Self(1 << 2);
    pub const EXPLICIT_COMPONENTS: Self = Self(1 << 3);
    pub const KEY_ACTIONS: Self = Self(1 << 4);
    pub const KEY_BEHAVIORS: Self = Self(1 << 5);
    pub const VIRTUAL_MODS: Self = Self(1 << 6);
    pub const VIRTUAL_MOD_MAP: Self = Self(1 << 7);
}
impl From<MapPart> for u16 {
    #[inline]
    fn from(input: MapPart) -> Self {
        input.0
    }
}
impl From<MapPart> for Option<u16> {
    #[inline]
    fn from(input: MapPart) -> Self {
        Some(input.0)
    }
}
impl From<MapPart> for u32 {
    #[inline]
    fn from(input: MapPart) -> Self {
        u32::from(input.0)
    }
}
impl From<MapPart> for Option<u32> {
    #[inline]
    fn from(input: MapPart) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for MapPart {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for MapPart {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for MapPart  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KEY_TYPES.0.into(), "KEY_TYPES", "KeyTypes"),
            (Self::KEY_SYMS.0.into(), "KEY_SYMS", "KeySyms"),
            (Self::MODIFIER_MAP.0.into(), "MODIFIER_MAP", "ModifierMap"),
            (Self::EXPLICIT_COMPONENTS.0.into(), "EXPLICIT_COMPONENTS", "ExplicitComponents"),
            (Self::KEY_ACTIONS.0.into(), "KEY_ACTIONS", "KeyActions"),
            (Self::KEY_BEHAVIORS.0.into(), "KEY_BEHAVIORS", "KeyBehaviors"),
            (Self::VIRTUAL_MODS.0.into(), "VIRTUAL_MODS", "VirtualMods"),
            (Self::VIRTUAL_MOD_MAP.0.into(), "VIRTUAL_MOD_MAP", "VirtualModMap"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(MapPart, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetMapFlags(u16);
impl SetMapFlags {
    pub const RESIZE_TYPES: Self = Self(1 << 0);
    pub const RECOMPUTE_ACTIONS: Self = Self(1 << 1);
}
impl From<SetMapFlags> for u16 {
    #[inline]
    fn from(input: SetMapFlags) -> Self {
        input.0
    }
}
impl From<SetMapFlags> for Option<u16> {
    #[inline]
    fn from(input: SetMapFlags) -> Self {
        Some(input.0)
    }
}
impl From<SetMapFlags> for u32 {
    #[inline]
    fn from(input: SetMapFlags) -> Self {
        u32::from(input.0)
    }
}
impl From<SetMapFlags> for Option<u32> {
    #[inline]
    fn from(input: SetMapFlags) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SetMapFlags {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for SetMapFlags {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SetMapFlags  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::RESIZE_TYPES.0.into(), "RESIZE_TYPES", "ResizeTypes"),
            (Self::RECOMPUTE_ACTIONS.0.into(), "RECOMPUTE_ACTIONS", "RecomputeActions"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SetMapFlags, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatePart(u16);
impl StatePart {
    pub const MODIFIER_STATE: Self = Self(1 << 0);
    pub const MODIFIER_BASE: Self = Self(1 << 1);
    pub const MODIFIER_LATCH: Self = Self(1 << 2);
    pub const MODIFIER_LOCK: Self = Self(1 << 3);
    pub const GROUP_STATE: Self = Self(1 << 4);
    pub const GROUP_BASE: Self = Self(1 << 5);
    pub const GROUP_LATCH: Self = Self(1 << 6);
    pub const GROUP_LOCK: Self = Self(1 << 7);
    pub const COMPAT_STATE: Self = Self(1 << 8);
    pub const GRAB_MODS: Self = Self(1 << 9);
    pub const COMPAT_GRAB_MODS: Self = Self(1 << 10);
    pub const LOOKUP_MODS: Self = Self(1 << 11);
    pub const COMPAT_LOOKUP_MODS: Self = Self(1 << 12);
    pub const POINTER_BUTTONS: Self = Self(1 << 13);
}
impl From<StatePart> for u16 {
    #[inline]
    fn from(input: StatePart) -> Self {
        input.0
    }
}
impl From<StatePart> for Option<u16> {
    #[inline]
    fn from(input: StatePart) -> Self {
        Some(input.0)
    }
}
impl From<StatePart> for u32 {
    #[inline]
    fn from(input: StatePart) -> Self {
        u32::from(input.0)
    }
}
impl From<StatePart> for Option<u32> {
    #[inline]
    fn from(input: StatePart) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for StatePart {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for StatePart {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for StatePart  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::MODIFIER_STATE.0.into(), "MODIFIER_STATE", "ModifierState"),
            (Self::MODIFIER_BASE.0.into(), "MODIFIER_BASE", "ModifierBase"),
            (Self::MODIFIER_LATCH.0.into(), "MODIFIER_LATCH", "ModifierLatch"),
            (Self::MODIFIER_LOCK.0.into(), "MODIFIER_LOCK", "ModifierLock"),
            (Self::GROUP_STATE.0.into(), "GROUP_STATE", "GroupState"),
            (Self::GROUP_BASE.0.into(), "GROUP_BASE", "GroupBase"),
            (Self::GROUP_LATCH.0.into(), "GROUP_LATCH", "GroupLatch"),
            (Self::GROUP_LOCK.0.into(), "GROUP_LOCK", "GroupLock"),
            (Self::COMPAT_STATE.0.into(), "COMPAT_STATE", "CompatState"),
            (Self::GRAB_MODS.0.into(), "GRAB_MODS", "GrabMods"),
            (Self::COMPAT_GRAB_MODS.0.into(), "COMPAT_GRAB_MODS", "CompatGrabMods"),
            (Self::LOOKUP_MODS.0.into(), "LOOKUP_MODS", "LookupMods"),
            (Self::COMPAT_LOOKUP_MODS.0.into(), "COMPAT_LOOKUP_MODS", "CompatLookupMods"),
            (Self::POINTER_BUTTONS.0.into(), "POINTER_BUTTONS", "PointerButtons"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(StatePart, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoolCtrl(u32);
impl BoolCtrl {
    pub const REPEAT_KEYS: Self = Self(1 << 0);
    pub const SLOW_KEYS: Self = Self(1 << 1);
    pub const BOUNCE_KEYS: Self = Self(1 << 2);
    pub const STICKY_KEYS: Self = Self(1 << 3);
    pub const MOUSE_KEYS: Self = Self(1 << 4);
    pub const MOUSE_KEYS_ACCEL: Self = Self(1 << 5);
    pub const ACCESS_X_KEYS: Self = Self(1 << 6);
    pub const ACCESS_X_TIMEOUT_MASK: Self = Self(1 << 7);
    pub const ACCESS_X_FEEDBACK_MASK: Self = Self(1 << 8);
    pub const AUDIBLE_BELL_MASK: Self = Self(1 << 9);
    pub const OVERLAY1_MASK: Self = Self(1 << 10);
    pub const OVERLAY2_MASK: Self = Self(1 << 11);
    pub const IGNORE_GROUP_LOCK_MASK: Self = Self(1 << 12);
}
impl From<BoolCtrl> for u32 {
    #[inline]
    fn from(input: BoolCtrl) -> Self {
        input.0
    }
}
impl From<BoolCtrl> for Option<u32> {
    #[inline]
    fn from(input: BoolCtrl) -> Self {
        Some(input.0)
    }
}
impl From<u8> for BoolCtrl {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for BoolCtrl {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for BoolCtrl {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BoolCtrl  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::REPEAT_KEYS.0, "REPEAT_KEYS", "RepeatKeys"),
            (Self::SLOW_KEYS.0, "SLOW_KEYS", "SlowKeys"),
            (Self::BOUNCE_KEYS.0, "BOUNCE_KEYS", "BounceKeys"),
            (Self::STICKY_KEYS.0, "STICKY_KEYS", "StickyKeys"),
            (Self::MOUSE_KEYS.0, "MOUSE_KEYS", "MouseKeys"),
            (Self::MOUSE_KEYS_ACCEL.0, "MOUSE_KEYS_ACCEL", "MouseKeysAccel"),
            (Self::ACCESS_X_KEYS.0, "ACCESS_X_KEYS", "AccessXKeys"),
            (Self::ACCESS_X_TIMEOUT_MASK.0, "ACCESS_X_TIMEOUT_MASK", "AccessXTimeoutMask"),
            (Self::ACCESS_X_FEEDBACK_MASK.0, "ACCESS_X_FEEDBACK_MASK", "AccessXFeedbackMask"),
            (Self::AUDIBLE_BELL_MASK.0, "AUDIBLE_BELL_MASK", "AudibleBellMask"),
            (Self::OVERLAY1_MASK.0, "OVERLAY1_MASK", "Overlay1Mask"),
            (Self::OVERLAY2_MASK.0, "OVERLAY2_MASK", "Overlay2Mask"),
            (Self::IGNORE_GROUP_LOCK_MASK.0, "IGNORE_GROUP_LOCK_MASK", "IgnoreGroupLockMask"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(BoolCtrl, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Control(u32);
impl Control {
    pub const GROUPS_WRAP: Self = Self(1 << 27);
    pub const INTERNAL_MODS: Self = Self(1 << 28);
    pub const IGNORE_LOCK_MODS: Self = Self(1 << 29);
    pub const PER_KEY_REPEAT: Self = Self(1 << 30);
    pub const CONTROLS_ENABLED: Self = Self(1 << 31);
}
impl From<Control> for u32 {
    #[inline]
    fn from(input: Control) -> Self {
        input.0
    }
}
impl From<Control> for Option<u32> {
    #[inline]
    fn from(input: Control) -> Self {
        Some(input.0)
    }
}
impl From<u8> for Control {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for Control {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for Control {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Control  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::GROUPS_WRAP.0, "GROUPS_WRAP", "GroupsWrap"),
            (Self::INTERNAL_MODS.0, "INTERNAL_MODS", "InternalMods"),
            (Self::IGNORE_LOCK_MODS.0, "IGNORE_LOCK_MODS", "IgnoreLockMods"),
            (Self::PER_KEY_REPEAT.0, "PER_KEY_REPEAT", "PerKeyRepeat"),
            (Self::CONTROLS_ENABLED.0, "CONTROLS_ENABLED", "ControlsEnabled"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(Control, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AXOption(u16);
impl AXOption {
    pub const SK_PRESS_FB: Self = Self(1 << 0);
    pub const SK_ACCEPT_FB: Self = Self(1 << 1);
    pub const FEATURE_FB: Self = Self(1 << 2);
    pub const SLOW_WARN_FB: Self = Self(1 << 3);
    pub const INDICATOR_FB: Self = Self(1 << 4);
    pub const STICKY_KEYS_FB: Self = Self(1 << 5);
    pub const TWO_KEYS: Self = Self(1 << 6);
    pub const LATCH_TO_LOCK: Self = Self(1 << 7);
    pub const SK_RELEASE_FB: Self = Self(1 << 8);
    pub const SK_REJECT_FB: Self = Self(1 << 9);
    pub const BK_REJECT_FB: Self = Self(1 << 10);
    pub const DUMB_BELL: Self = Self(1 << 11);
}
impl From<AXOption> for u16 {
    #[inline]
    fn from(input: AXOption) -> Self {
        input.0
    }
}
impl From<AXOption> for Option<u16> {
    #[inline]
    fn from(input: AXOption) -> Self {
        Some(input.0)
    }
}
impl From<AXOption> for u32 {
    #[inline]
    fn from(input: AXOption) -> Self {
        u32::from(input.0)
    }
}
impl From<AXOption> for Option<u32> {
    #[inline]
    fn from(input: AXOption) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for AXOption {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for AXOption {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for AXOption  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SK_PRESS_FB.0.into(), "SK_PRESS_FB", "SKPressFB"),
            (Self::SK_ACCEPT_FB.0.into(), "SK_ACCEPT_FB", "SKAcceptFB"),
            (Self::FEATURE_FB.0.into(), "FEATURE_FB", "FeatureFB"),
            (Self::SLOW_WARN_FB.0.into(), "SLOW_WARN_FB", "SlowWarnFB"),
            (Self::INDICATOR_FB.0.into(), "INDICATOR_FB", "IndicatorFB"),
            (Self::STICKY_KEYS_FB.0.into(), "STICKY_KEYS_FB", "StickyKeysFB"),
            (Self::TWO_KEYS.0.into(), "TWO_KEYS", "TwoKeys"),
            (Self::LATCH_TO_LOCK.0.into(), "LATCH_TO_LOCK", "LatchToLock"),
            (Self::SK_RELEASE_FB.0.into(), "SK_RELEASE_FB", "SKReleaseFB"),
            (Self::SK_REJECT_FB.0.into(), "SK_REJECT_FB", "SKRejectFB"),
            (Self::BK_REJECT_FB.0.into(), "BK_REJECT_FB", "BKRejectFB"),
            (Self::DUMB_BELL.0.into(), "DUMB_BELL", "DumbBell"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(AXOption, u16);

pub type DeviceSpec = u16;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LedClassResult(u16);
impl LedClassResult {
    pub const KBD_FEEDBACK_CLASS: Self = Self(0);
    pub const LED_FEEDBACK_CLASS: Self = Self(4);
}
impl From<LedClassResult> for u16 {
    #[inline]
    fn from(input: LedClassResult) -> Self {
        input.0
    }
}
impl From<LedClassResult> for Option<u16> {
    #[inline]
    fn from(input: LedClassResult) -> Self {
        Some(input.0)
    }
}
impl From<LedClassResult> for u32 {
    #[inline]
    fn from(input: LedClassResult) -> Self {
        u32::from(input.0)
    }
}
impl From<LedClassResult> for Option<u32> {
    #[inline]
    fn from(input: LedClassResult) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for LedClassResult {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for LedClassResult {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for LedClassResult  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KBD_FEEDBACK_CLASS.0.into(), "KBD_FEEDBACK_CLASS", "KbdFeedbackClass"),
            (Self::LED_FEEDBACK_CLASS.0.into(), "LED_FEEDBACK_CLASS", "LedFeedbackClass"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LedClass(u16);
impl LedClass {
    pub const KBD_FEEDBACK_CLASS: Self = Self(0);
    pub const LED_FEEDBACK_CLASS: Self = Self(4);
    pub const DFLT_XI_CLASS: Self = Self(768);
    pub const ALL_XI_CLASSES: Self = Self(1280);
}
impl From<LedClass> for u16 {
    #[inline]
    fn from(input: LedClass) -> Self {
        input.0
    }
}
impl From<LedClass> for Option<u16> {
    #[inline]
    fn from(input: LedClass) -> Self {
        Some(input.0)
    }
}
impl From<LedClass> for u32 {
    #[inline]
    fn from(input: LedClass) -> Self {
        u32::from(input.0)
    }
}
impl From<LedClass> for Option<u32> {
    #[inline]
    fn from(input: LedClass) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for LedClass {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for LedClass {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for LedClass  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KBD_FEEDBACK_CLASS.0.into(), "KBD_FEEDBACK_CLASS", "KbdFeedbackClass"),
            (Self::LED_FEEDBACK_CLASS.0.into(), "LED_FEEDBACK_CLASS", "LedFeedbackClass"),
            (Self::DFLT_XI_CLASS.0.into(), "DFLT_XI_CLASS", "DfltXIClass"),
            (Self::ALL_XI_CLASSES.0.into(), "ALL_XI_CLASSES", "AllXIClasses"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

pub type LedClassSpec = u16;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BellClassResult(u8);
impl BellClassResult {
    pub const KBD_FEEDBACK_CLASS: Self = Self(0);
    pub const BELL_FEEDBACK_CLASS: Self = Self(5);
}
impl From<BellClassResult> for u8 {
    #[inline]
    fn from(input: BellClassResult) -> Self {
        input.0
    }
}
impl From<BellClassResult> for Option<u8> {
    #[inline]
    fn from(input: BellClassResult) -> Self {
        Some(input.0)
    }
}
impl From<BellClassResult> for u16 {
    #[inline]
    fn from(input: BellClassResult) -> Self {
        u16::from(input.0)
    }
}
impl From<BellClassResult> for Option<u16> {
    #[inline]
    fn from(input: BellClassResult) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<BellClassResult> for u32 {
    #[inline]
    fn from(input: BellClassResult) -> Self {
        u32::from(input.0)
    }
}
impl From<BellClassResult> for Option<u32> {
    #[inline]
    fn from(input: BellClassResult) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for BellClassResult {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BellClassResult  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KBD_FEEDBACK_CLASS.0.into(), "KBD_FEEDBACK_CLASS", "KbdFeedbackClass"),
            (Self::BELL_FEEDBACK_CLASS.0.into(), "BELL_FEEDBACK_CLASS", "BellFeedbackClass"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BellClass(u16);
impl BellClass {
    pub const KBD_FEEDBACK_CLASS: Self = Self(0);
    pub const BELL_FEEDBACK_CLASS: Self = Self(5);
    pub const DFLT_XI_CLASS: Self = Self(768);
}
impl From<BellClass> for u16 {
    #[inline]
    fn from(input: BellClass) -> Self {
        input.0
    }
}
impl From<BellClass> for Option<u16> {
    #[inline]
    fn from(input: BellClass) -> Self {
        Some(input.0)
    }
}
impl From<BellClass> for u32 {
    #[inline]
    fn from(input: BellClass) -> Self {
        u32::from(input.0)
    }
}
impl From<BellClass> for Option<u32> {
    #[inline]
    fn from(input: BellClass) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for BellClass {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for BellClass {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BellClass  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KBD_FEEDBACK_CLASS.0.into(), "KBD_FEEDBACK_CLASS", "KbdFeedbackClass"),
            (Self::BELL_FEEDBACK_CLASS.0.into(), "BELL_FEEDBACK_CLASS", "BellFeedbackClass"),
            (Self::DFLT_XI_CLASS.0.into(), "DFLT_XI_CLASS", "DfltXIClass"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

pub type BellClassSpec = u16;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ID(u16);
impl ID {
    pub const USE_CORE_KBD: Self = Self(256);
    pub const USE_CORE_PTR: Self = Self(512);
    pub const DFLT_XI_CLASS: Self = Self(768);
    pub const DFLT_XI_ID: Self = Self(1024);
    pub const ALL_XI_CLASS: Self = Self(1280);
    pub const ALL_XI_ID: Self = Self(1536);
    pub const XI_NONE: Self = Self(65280);
}
impl From<ID> for u16 {
    #[inline]
    fn from(input: ID) -> Self {
        input.0
    }
}
impl From<ID> for Option<u16> {
    #[inline]
    fn from(input: ID) -> Self {
        Some(input.0)
    }
}
impl From<ID> for u32 {
    #[inline]
    fn from(input: ID) -> Self {
        u32::from(input.0)
    }
}
impl From<ID> for Option<u32> {
    #[inline]
    fn from(input: ID) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ID {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ID {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ID  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::USE_CORE_KBD.0.into(), "USE_CORE_KBD", "UseCoreKbd"),
            (Self::USE_CORE_PTR.0.into(), "USE_CORE_PTR", "UseCorePtr"),
            (Self::DFLT_XI_CLASS.0.into(), "DFLT_XI_CLASS", "DfltXIClass"),
            (Self::DFLT_XI_ID.0.into(), "DFLT_XI_ID", "DfltXIId"),
            (Self::ALL_XI_CLASS.0.into(), "ALL_XI_CLASS", "AllXIClass"),
            (Self::ALL_XI_ID.0.into(), "ALL_XI_ID", "AllXIId"),
            (Self::XI_NONE.0.into(), "XI_NONE", "XINone"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

pub type IDSpec = u16;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Group(u8);
impl Group {
    pub const M1: Self = Self(0);
    pub const M2: Self = Self(1);
    pub const M3: Self = Self(2);
    pub const M4: Self = Self(3);
}
impl From<Group> for u8 {
    #[inline]
    fn from(input: Group) -> Self {
        input.0
    }
}
impl From<Group> for Option<u8> {
    #[inline]
    fn from(input: Group) -> Self {
        Some(input.0)
    }
}
impl From<Group> for u16 {
    #[inline]
    fn from(input: Group) -> Self {
        u16::from(input.0)
    }
}
impl From<Group> for Option<u16> {
    #[inline]
    fn from(input: Group) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Group> for u32 {
    #[inline]
    fn from(input: Group) -> Self {
        u32::from(input.0)
    }
}
impl From<Group> for Option<u32> {
    #[inline]
    fn from(input: Group) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Group {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Group  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M4.0.into(), "M4", "M4"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Groups(u8);
impl Groups {
    pub const ANY: Self = Self(254);
    pub const ALL: Self = Self(255);
}
impl From<Groups> for u8 {
    #[inline]
    fn from(input: Groups) -> Self {
        input.0
    }
}
impl From<Groups> for Option<u8> {
    #[inline]
    fn from(input: Groups) -> Self {
        Some(input.0)
    }
}
impl From<Groups> for u16 {
    #[inline]
    fn from(input: Groups) -> Self {
        u16::from(input.0)
    }
}
impl From<Groups> for Option<u16> {
    #[inline]
    fn from(input: Groups) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Groups> for u32 {
    #[inline]
    fn from(input: Groups) -> Self {
        u32::from(input.0)
    }
}
impl From<Groups> for Option<u32> {
    #[inline]
    fn from(input: Groups) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Groups {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Groups  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ANY.0.into(), "ANY", "Any"),
            (Self::ALL.0.into(), "ALL", "All"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetOfGroup(u8);
impl SetOfGroup {
    pub const GROUP1: Self = Self(1 << 0);
    pub const GROUP2: Self = Self(1 << 1);
    pub const GROUP3: Self = Self(1 << 2);
    pub const GROUP4: Self = Self(1 << 3);
}
impl From<SetOfGroup> for u8 {
    #[inline]
    fn from(input: SetOfGroup) -> Self {
        input.0
    }
}
impl From<SetOfGroup> for Option<u8> {
    #[inline]
    fn from(input: SetOfGroup) -> Self {
        Some(input.0)
    }
}
impl From<SetOfGroup> for u16 {
    #[inline]
    fn from(input: SetOfGroup) -> Self {
        u16::from(input.0)
    }
}
impl From<SetOfGroup> for Option<u16> {
    #[inline]
    fn from(input: SetOfGroup) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SetOfGroup> for u32 {
    #[inline]
    fn from(input: SetOfGroup) -> Self {
        u32::from(input.0)
    }
}
impl From<SetOfGroup> for Option<u32> {
    #[inline]
    fn from(input: SetOfGroup) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SetOfGroup {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SetOfGroup  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::GROUP1.0.into(), "GROUP1", "Group1"),
            (Self::GROUP2.0.into(), "GROUP2", "Group2"),
            (Self::GROUP3.0.into(), "GROUP3", "Group3"),
            (Self::GROUP4.0.into(), "GROUP4", "Group4"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SetOfGroup, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetOfGroups(u8);
impl SetOfGroups {
    pub const ANY: Self = Self(1 << 7);
}
impl From<SetOfGroups> for u8 {
    #[inline]
    fn from(input: SetOfGroups) -> Self {
        input.0
    }
}
impl From<SetOfGroups> for Option<u8> {
    #[inline]
    fn from(input: SetOfGroups) -> Self {
        Some(input.0)
    }
}
impl From<SetOfGroups> for u16 {
    #[inline]
    fn from(input: SetOfGroups) -> Self {
        u16::from(input.0)
    }
}
impl From<SetOfGroups> for Option<u16> {
    #[inline]
    fn from(input: SetOfGroups) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SetOfGroups> for u32 {
    #[inline]
    fn from(input: SetOfGroups) -> Self {
        u32::from(input.0)
    }
}
impl From<SetOfGroups> for Option<u32> {
    #[inline]
    fn from(input: SetOfGroups) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SetOfGroups {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SetOfGroups  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ANY.0.into(), "ANY", "Any"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SetOfGroups, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GroupsWrap(u8);
impl GroupsWrap {
    pub const WRAP_INTO_RANGE: Self = Self(0);
    pub const CLAMP_INTO_RANGE: Self = Self(1 << 6);
    pub const REDIRECT_INTO_RANGE: Self = Self(1 << 7);
}
impl From<GroupsWrap> for u8 {
    #[inline]
    fn from(input: GroupsWrap) -> Self {
        input.0
    }
}
impl From<GroupsWrap> for Option<u8> {
    #[inline]
    fn from(input: GroupsWrap) -> Self {
        Some(input.0)
    }
}
impl From<GroupsWrap> for u16 {
    #[inline]
    fn from(input: GroupsWrap) -> Self {
        u16::from(input.0)
    }
}
impl From<GroupsWrap> for Option<u16> {
    #[inline]
    fn from(input: GroupsWrap) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<GroupsWrap> for u32 {
    #[inline]
    fn from(input: GroupsWrap) -> Self {
        u32::from(input.0)
    }
}
impl From<GroupsWrap> for Option<u32> {
    #[inline]
    fn from(input: GroupsWrap) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for GroupsWrap {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GroupsWrap  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::WRAP_INTO_RANGE.0.into(), "WRAP_INTO_RANGE", "WrapIntoRange"),
            (Self::CLAMP_INTO_RANGE.0.into(), "CLAMP_INTO_RANGE", "ClampIntoRange"),
            (Self::REDIRECT_INTO_RANGE.0.into(), "REDIRECT_INTO_RANGE", "RedirectIntoRange"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(GroupsWrap, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VModsHigh(u8);
impl VModsHigh {
    pub const M15: Self = Self(1 << 7);
    pub const M14: Self = Self(1 << 6);
    pub const M13: Self = Self(1 << 5);
    pub const M12: Self = Self(1 << 4);
    pub const M11: Self = Self(1 << 3);
    pub const M10: Self = Self(1 << 2);
    pub const M9: Self = Self(1 << 1);
    pub const M8: Self = Self(1 << 0);
}
impl From<VModsHigh> for u8 {
    #[inline]
    fn from(input: VModsHigh) -> Self {
        input.0
    }
}
impl From<VModsHigh> for Option<u8> {
    #[inline]
    fn from(input: VModsHigh) -> Self {
        Some(input.0)
    }
}
impl From<VModsHigh> for u16 {
    #[inline]
    fn from(input: VModsHigh) -> Self {
        u16::from(input.0)
    }
}
impl From<VModsHigh> for Option<u16> {
    #[inline]
    fn from(input: VModsHigh) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<VModsHigh> for u32 {
    #[inline]
    fn from(input: VModsHigh) -> Self {
        u32::from(input.0)
    }
}
impl From<VModsHigh> for Option<u32> {
    #[inline]
    fn from(input: VModsHigh) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for VModsHigh {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for VModsHigh  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::M15.0.into(), "M15", "M15"),
            (Self::M14.0.into(), "M14", "M14"),
            (Self::M13.0.into(), "M13", "M13"),
            (Self::M12.0.into(), "M12", "M12"),
            (Self::M11.0.into(), "M11", "M11"),
            (Self::M10.0.into(), "M10", "M10"),
            (Self::M9.0.into(), "M9", "M9"),
            (Self::M8.0.into(), "M8", "M8"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(VModsHigh, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VModsLow(u8);
impl VModsLow {
    pub const M7: Self = Self(1 << 7);
    pub const M6: Self = Self(1 << 6);
    pub const M5: Self = Self(1 << 5);
    pub const M4: Self = Self(1 << 4);
    pub const M3: Self = Self(1 << 3);
    pub const M2: Self = Self(1 << 2);
    pub const M1: Self = Self(1 << 1);
    pub const M0: Self = Self(1 << 0);
}
impl From<VModsLow> for u8 {
    #[inline]
    fn from(input: VModsLow) -> Self {
        input.0
    }
}
impl From<VModsLow> for Option<u8> {
    #[inline]
    fn from(input: VModsLow) -> Self {
        Some(input.0)
    }
}
impl From<VModsLow> for u16 {
    #[inline]
    fn from(input: VModsLow) -> Self {
        u16::from(input.0)
    }
}
impl From<VModsLow> for Option<u16> {
    #[inline]
    fn from(input: VModsLow) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<VModsLow> for u32 {
    #[inline]
    fn from(input: VModsLow) -> Self {
        u32::from(input.0)
    }
}
impl From<VModsLow> for Option<u32> {
    #[inline]
    fn from(input: VModsLow) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for VModsLow {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for VModsLow  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::M7.0.into(), "M7", "M7"),
            (Self::M6.0.into(), "M6", "M6"),
            (Self::M5.0.into(), "M5", "M5"),
            (Self::M4.0.into(), "M4", "M4"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M0.0.into(), "M0", "M0"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(VModsLow, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VMod(u16);
impl VMod {
    pub const M15: Self = Self(1 << 15);
    pub const M14: Self = Self(1 << 14);
    pub const M13: Self = Self(1 << 13);
    pub const M12: Self = Self(1 << 12);
    pub const M11: Self = Self(1 << 11);
    pub const M10: Self = Self(1 << 10);
    pub const M9: Self = Self(1 << 9);
    pub const M8: Self = Self(1 << 8);
    pub const M7: Self = Self(1 << 7);
    pub const M6: Self = Self(1 << 6);
    pub const M5: Self = Self(1 << 5);
    pub const M4: Self = Self(1 << 4);
    pub const M3: Self = Self(1 << 3);
    pub const M2: Self = Self(1 << 2);
    pub const M1: Self = Self(1 << 1);
    pub const M0: Self = Self(1 << 0);
}
impl From<VMod> for u16 {
    #[inline]
    fn from(input: VMod) -> Self {
        input.0
    }
}
impl From<VMod> for Option<u16> {
    #[inline]
    fn from(input: VMod) -> Self {
        Some(input.0)
    }
}
impl From<VMod> for u32 {
    #[inline]
    fn from(input: VMod) -> Self {
        u32::from(input.0)
    }
}
impl From<VMod> for Option<u32> {
    #[inline]
    fn from(input: VMod) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for VMod {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for VMod {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for VMod  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::M15.0.into(), "M15", "M15"),
            (Self::M14.0.into(), "M14", "M14"),
            (Self::M13.0.into(), "M13", "M13"),
            (Self::M12.0.into(), "M12", "M12"),
            (Self::M11.0.into(), "M11", "M11"),
            (Self::M10.0.into(), "M10", "M10"),
            (Self::M9.0.into(), "M9", "M9"),
            (Self::M8.0.into(), "M8", "M8"),
            (Self::M7.0.into(), "M7", "M7"),
            (Self::M6.0.into(), "M6", "M6"),
            (Self::M5.0.into(), "M5", "M5"),
            (Self::M4.0.into(), "M4", "M4"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M0.0.into(), "M0", "M0"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(VMod, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Explicit(u8);
impl Explicit {
    pub const V_MOD_MAP: Self = Self(1 << 7);
    pub const BEHAVIOR: Self = Self(1 << 6);
    pub const AUTO_REPEAT: Self = Self(1 << 5);
    pub const INTERPRET: Self = Self(1 << 4);
    pub const KEY_TYPE4: Self = Self(1 << 3);
    pub const KEY_TYPE3: Self = Self(1 << 2);
    pub const KEY_TYPE2: Self = Self(1 << 1);
    pub const KEY_TYPE1: Self = Self(1 << 0);
}
impl From<Explicit> for u8 {
    #[inline]
    fn from(input: Explicit) -> Self {
        input.0
    }
}
impl From<Explicit> for Option<u8> {
    #[inline]
    fn from(input: Explicit) -> Self {
        Some(input.0)
    }
}
impl From<Explicit> for u16 {
    #[inline]
    fn from(input: Explicit) -> Self {
        u16::from(input.0)
    }
}
impl From<Explicit> for Option<u16> {
    #[inline]
    fn from(input: Explicit) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Explicit> for u32 {
    #[inline]
    fn from(input: Explicit) -> Self {
        u32::from(input.0)
    }
}
impl From<Explicit> for Option<u32> {
    #[inline]
    fn from(input: Explicit) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Explicit {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Explicit  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::V_MOD_MAP.0.into(), "V_MOD_MAP", "VModMap"),
            (Self::BEHAVIOR.0.into(), "BEHAVIOR", "Behavior"),
            (Self::AUTO_REPEAT.0.into(), "AUTO_REPEAT", "AutoRepeat"),
            (Self::INTERPRET.0.into(), "INTERPRET", "Interpret"),
            (Self::KEY_TYPE4.0.into(), "KEY_TYPE4", "KeyType4"),
            (Self::KEY_TYPE3.0.into(), "KEY_TYPE3", "KeyType3"),
            (Self::KEY_TYPE2.0.into(), "KEY_TYPE2", "KeyType2"),
            (Self::KEY_TYPE1.0.into(), "KEY_TYPE1", "KeyType1"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(Explicit, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SymInterpretMatch(u8);
impl SymInterpretMatch {
    pub const NONE_OF: Self = Self(0);
    pub const ANY_OF_OR_NONE: Self = Self(1);
    pub const ANY_OF: Self = Self(2);
    pub const ALL_OF: Self = Self(3);
    pub const EXACTLY: Self = Self(4);
}
impl From<SymInterpretMatch> for u8 {
    #[inline]
    fn from(input: SymInterpretMatch) -> Self {
        input.0
    }
}
impl From<SymInterpretMatch> for Option<u8> {
    #[inline]
    fn from(input: SymInterpretMatch) -> Self {
        Some(input.0)
    }
}
impl From<SymInterpretMatch> for u16 {
    #[inline]
    fn from(input: SymInterpretMatch) -> Self {
        u16::from(input.0)
    }
}
impl From<SymInterpretMatch> for Option<u16> {
    #[inline]
    fn from(input: SymInterpretMatch) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SymInterpretMatch> for u32 {
    #[inline]
    fn from(input: SymInterpretMatch) -> Self {
        u32::from(input.0)
    }
}
impl From<SymInterpretMatch> for Option<u32> {
    #[inline]
    fn from(input: SymInterpretMatch) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SymInterpretMatch {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SymInterpretMatch  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE_OF.0.into(), "NONE_OF", "NoneOf"),
            (Self::ANY_OF_OR_NONE.0.into(), "ANY_OF_OR_NONE", "AnyOfOrNone"),
            (Self::ANY_OF.0.into(), "ANY_OF", "AnyOf"),
            (Self::ALL_OF.0.into(), "ALL_OF", "AllOf"),
            (Self::EXACTLY.0.into(), "EXACTLY", "Exactly"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SymInterpMatch(u8);
impl SymInterpMatch {
    pub const LEVEL_ONE_ONLY: Self = Self(1 << 7);
    pub const OP_MASK: Self = Self(127);
}
impl From<SymInterpMatch> for u8 {
    #[inline]
    fn from(input: SymInterpMatch) -> Self {
        input.0
    }
}
impl From<SymInterpMatch> for Option<u8> {
    #[inline]
    fn from(input: SymInterpMatch) -> Self {
        Some(input.0)
    }
}
impl From<SymInterpMatch> for u16 {
    #[inline]
    fn from(input: SymInterpMatch) -> Self {
        u16::from(input.0)
    }
}
impl From<SymInterpMatch> for Option<u16> {
    #[inline]
    fn from(input: SymInterpMatch) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SymInterpMatch> for u32 {
    #[inline]
    fn from(input: SymInterpMatch) -> Self {
        u32::from(input.0)
    }
}
impl From<SymInterpMatch> for Option<u32> {
    #[inline]
    fn from(input: SymInterpMatch) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SymInterpMatch {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SymInterpMatch  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::LEVEL_ONE_ONLY.0.into(), "LEVEL_ONE_ONLY", "LevelOneOnly"),
            (Self::OP_MASK.0.into(), "OP_MASK", "OpMask"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IMFlag(u8);
impl IMFlag {
    pub const NO_EXPLICIT: Self = Self(1 << 7);
    pub const NO_AUTOMATIC: Self = Self(1 << 6);
    pub const LED_DRIVES_KB: Self = Self(1 << 5);
}
impl From<IMFlag> for u8 {
    #[inline]
    fn from(input: IMFlag) -> Self {
        input.0
    }
}
impl From<IMFlag> for Option<u8> {
    #[inline]
    fn from(input: IMFlag) -> Self {
        Some(input.0)
    }
}
impl From<IMFlag> for u16 {
    #[inline]
    fn from(input: IMFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<IMFlag> for Option<u16> {
    #[inline]
    fn from(input: IMFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<IMFlag> for u32 {
    #[inline]
    fn from(input: IMFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<IMFlag> for Option<u32> {
    #[inline]
    fn from(input: IMFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for IMFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for IMFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NO_EXPLICIT.0.into(), "NO_EXPLICIT", "NoExplicit"),
            (Self::NO_AUTOMATIC.0.into(), "NO_AUTOMATIC", "NoAutomatic"),
            (Self::LED_DRIVES_KB.0.into(), "LED_DRIVES_KB", "LEDDrivesKB"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(IMFlag, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IMModsWhich(u8);
impl IMModsWhich {
    pub const USE_COMPAT: Self = Self(1 << 4);
    pub const USE_EFFECTIVE: Self = Self(1 << 3);
    pub const USE_LOCKED: Self = Self(1 << 2);
    pub const USE_LATCHED: Self = Self(1 << 1);
    pub const USE_BASE: Self = Self(1 << 0);
}
impl From<IMModsWhich> for u8 {
    #[inline]
    fn from(input: IMModsWhich) -> Self {
        input.0
    }
}
impl From<IMModsWhich> for Option<u8> {
    #[inline]
    fn from(input: IMModsWhich) -> Self {
        Some(input.0)
    }
}
impl From<IMModsWhich> for u16 {
    #[inline]
    fn from(input: IMModsWhich) -> Self {
        u16::from(input.0)
    }
}
impl From<IMModsWhich> for Option<u16> {
    #[inline]
    fn from(input: IMModsWhich) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<IMModsWhich> for u32 {
    #[inline]
    fn from(input: IMModsWhich) -> Self {
        u32::from(input.0)
    }
}
impl From<IMModsWhich> for Option<u32> {
    #[inline]
    fn from(input: IMModsWhich) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for IMModsWhich {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for IMModsWhich  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::USE_COMPAT.0.into(), "USE_COMPAT", "UseCompat"),
            (Self::USE_EFFECTIVE.0.into(), "USE_EFFECTIVE", "UseEffective"),
            (Self::USE_LOCKED.0.into(), "USE_LOCKED", "UseLocked"),
            (Self::USE_LATCHED.0.into(), "USE_LATCHED", "UseLatched"),
            (Self::USE_BASE.0.into(), "USE_BASE", "UseBase"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(IMModsWhich, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IMGroupsWhich(u8);
impl IMGroupsWhich {
    pub const USE_COMPAT: Self = Self(1 << 4);
    pub const USE_EFFECTIVE: Self = Self(1 << 3);
    pub const USE_LOCKED: Self = Self(1 << 2);
    pub const USE_LATCHED: Self = Self(1 << 1);
    pub const USE_BASE: Self = Self(1 << 0);
}
impl From<IMGroupsWhich> for u8 {
    #[inline]
    fn from(input: IMGroupsWhich) -> Self {
        input.0
    }
}
impl From<IMGroupsWhich> for Option<u8> {
    #[inline]
    fn from(input: IMGroupsWhich) -> Self {
        Some(input.0)
    }
}
impl From<IMGroupsWhich> for u16 {
    #[inline]
    fn from(input: IMGroupsWhich) -> Self {
        u16::from(input.0)
    }
}
impl From<IMGroupsWhich> for Option<u16> {
    #[inline]
    fn from(input: IMGroupsWhich) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<IMGroupsWhich> for u32 {
    #[inline]
    fn from(input: IMGroupsWhich) -> Self {
        u32::from(input.0)
    }
}
impl From<IMGroupsWhich> for Option<u32> {
    #[inline]
    fn from(input: IMGroupsWhich) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for IMGroupsWhich {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for IMGroupsWhich  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::USE_COMPAT.0.into(), "USE_COMPAT", "UseCompat"),
            (Self::USE_EFFECTIVE.0.into(), "USE_EFFECTIVE", "UseEffective"),
            (Self::USE_LOCKED.0.into(), "USE_LOCKED", "UseLocked"),
            (Self::USE_LATCHED.0.into(), "USE_LATCHED", "UseLatched"),
            (Self::USE_BASE.0.into(), "USE_BASE", "UseBase"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(IMGroupsWhich, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndicatorMap {
    pub flags: IMFlag,
    pub which_groups: IMGroupsWhich,
    pub groups: SetOfGroup,
    pub which_mods: IMModsWhich,
    pub mods: xproto::ModMask,
    pub real_mods: xproto::ModMask,
    pub vmods: VMod,
    pub ctrls: BoolCtrl,
}
impl_debug_if_no_extra_traits!(IndicatorMap, "IndicatorMap");
impl TryParse for IndicatorMap {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (which_groups, remaining) = u8::try_parse(remaining)?;
        let (groups, remaining) = u8::try_parse(remaining)?;
        let (which_mods, remaining) = u8::try_parse(remaining)?;
        let (mods, remaining) = u8::try_parse(remaining)?;
        let (real_mods, remaining) = u8::try_parse(remaining)?;
        let (vmods, remaining) = u16::try_parse(remaining)?;
        let (ctrls, remaining) = u32::try_parse(remaining)?;
        let flags = flags.into();
        let which_groups = which_groups.into();
        let groups = groups.into();
        let which_mods = which_mods.into();
        let mods = mods.into();
        let real_mods = real_mods.into();
        let vmods = vmods.into();
        let ctrls = ctrls.into();
        let result = IndicatorMap { flags, which_groups, groups, which_mods, mods, real_mods, vmods, ctrls };
        Ok((result, remaining))
    }
}
impl Serialize for IndicatorMap {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let flags_bytes = u8::from(self.flags).serialize();
        let which_groups_bytes = u8::from(self.which_groups).serialize();
        let groups_bytes = u8::from(self.groups).serialize();
        let which_mods_bytes = u8::from(self.which_mods).serialize();
        let mods_bytes = (u16::from(self.mods) as u8).serialize();
        let real_mods_bytes = (u16::from(self.real_mods) as u8).serialize();
        let vmods_bytes = u16::from(self.vmods).serialize();
        let ctrls_bytes = u32::from(self.ctrls).serialize();
        [
            flags_bytes[0],
            which_groups_bytes[0],
            groups_bytes[0],
            which_mods_bytes[0],
            mods_bytes[0],
            real_mods_bytes[0],
            vmods_bytes[0],
            vmods_bytes[1],
            ctrls_bytes[0],
            ctrls_bytes[1],
            ctrls_bytes[2],
            ctrls_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        u8::from(self.flags).serialize_into(bytes);
        u8::from(self.which_groups).serialize_into(bytes);
        u8::from(self.groups).serialize_into(bytes);
        u8::from(self.which_mods).serialize_into(bytes);
        (u16::from(self.mods) as u8).serialize_into(bytes);
        (u16::from(self.real_mods) as u8).serialize_into(bytes);
        u16::from(self.vmods).serialize_into(bytes);
        u32::from(self.ctrls).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CMDetail(u8);
impl CMDetail {
    pub const SYM_INTERP: Self = Self(1 << 0);
    pub const GROUP_COMPAT: Self = Self(1 << 1);
}
impl From<CMDetail> for u8 {
    #[inline]
    fn from(input: CMDetail) -> Self {
        input.0
    }
}
impl From<CMDetail> for Option<u8> {
    #[inline]
    fn from(input: CMDetail) -> Self {
        Some(input.0)
    }
}
impl From<CMDetail> for u16 {
    #[inline]
    fn from(input: CMDetail) -> Self {
        u16::from(input.0)
    }
}
impl From<CMDetail> for Option<u16> {
    #[inline]
    fn from(input: CMDetail) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<CMDetail> for u32 {
    #[inline]
    fn from(input: CMDetail) -> Self {
        u32::from(input.0)
    }
}
impl From<CMDetail> for Option<u32> {
    #[inline]
    fn from(input: CMDetail) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for CMDetail {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CMDetail  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SYM_INTERP.0.into(), "SYM_INTERP", "SymInterp"),
            (Self::GROUP_COMPAT.0.into(), "GROUP_COMPAT", "GroupCompat"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(CMDetail, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NameDetail(u32);
impl NameDetail {
    pub const KEYCODES: Self = Self(1 << 0);
    pub const GEOMETRY: Self = Self(1 << 1);
    pub const SYMBOLS: Self = Self(1 << 2);
    pub const PHYS_SYMBOLS: Self = Self(1 << 3);
    pub const TYPES: Self = Self(1 << 4);
    pub const COMPAT: Self = Self(1 << 5);
    pub const KEY_TYPE_NAMES: Self = Self(1 << 6);
    pub const KT_LEVEL_NAMES: Self = Self(1 << 7);
    pub const INDICATOR_NAMES: Self = Self(1 << 8);
    pub const KEY_NAMES: Self = Self(1 << 9);
    pub const KEY_ALIASES: Self = Self(1 << 10);
    pub const VIRTUAL_MOD_NAMES: Self = Self(1 << 11);
    pub const GROUP_NAMES: Self = Self(1 << 12);
    pub const RG_NAMES: Self = Self(1 << 13);
}
impl From<NameDetail> for u32 {
    #[inline]
    fn from(input: NameDetail) -> Self {
        input.0
    }
}
impl From<NameDetail> for Option<u32> {
    #[inline]
    fn from(input: NameDetail) -> Self {
        Some(input.0)
    }
}
impl From<u8> for NameDetail {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for NameDetail {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for NameDetail {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for NameDetail  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KEYCODES.0, "KEYCODES", "Keycodes"),
            (Self::GEOMETRY.0, "GEOMETRY", "Geometry"),
            (Self::SYMBOLS.0, "SYMBOLS", "Symbols"),
            (Self::PHYS_SYMBOLS.0, "PHYS_SYMBOLS", "PhysSymbols"),
            (Self::TYPES.0, "TYPES", "Types"),
            (Self::COMPAT.0, "COMPAT", "Compat"),
            (Self::KEY_TYPE_NAMES.0, "KEY_TYPE_NAMES", "KeyTypeNames"),
            (Self::KT_LEVEL_NAMES.0, "KT_LEVEL_NAMES", "KTLevelNames"),
            (Self::INDICATOR_NAMES.0, "INDICATOR_NAMES", "IndicatorNames"),
            (Self::KEY_NAMES.0, "KEY_NAMES", "KeyNames"),
            (Self::KEY_ALIASES.0, "KEY_ALIASES", "KeyAliases"),
            (Self::VIRTUAL_MOD_NAMES.0, "VIRTUAL_MOD_NAMES", "VirtualModNames"),
            (Self::GROUP_NAMES.0, "GROUP_NAMES", "GroupNames"),
            (Self::RG_NAMES.0, "RG_NAMES", "RGNames"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(NameDetail, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GBNDetail(u16);
impl GBNDetail {
    pub const TYPES: Self = Self(1 << 0);
    pub const COMPAT_MAP: Self = Self(1 << 1);
    pub const CLIENT_SYMBOLS: Self = Self(1 << 2);
    pub const SERVER_SYMBOLS: Self = Self(1 << 3);
    pub const INDICATOR_MAPS: Self = Self(1 << 4);
    pub const KEY_NAMES: Self = Self(1 << 5);
    pub const GEOMETRY: Self = Self(1 << 6);
    pub const OTHER_NAMES: Self = Self(1 << 7);
}
impl From<GBNDetail> for u16 {
    #[inline]
    fn from(input: GBNDetail) -> Self {
        input.0
    }
}
impl From<GBNDetail> for Option<u16> {
    #[inline]
    fn from(input: GBNDetail) -> Self {
        Some(input.0)
    }
}
impl From<GBNDetail> for u32 {
    #[inline]
    fn from(input: GBNDetail) -> Self {
        u32::from(input.0)
    }
}
impl From<GBNDetail> for Option<u32> {
    #[inline]
    fn from(input: GBNDetail) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for GBNDetail {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for GBNDetail {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GBNDetail  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::TYPES.0.into(), "TYPES", "Types"),
            (Self::COMPAT_MAP.0.into(), "COMPAT_MAP", "CompatMap"),
            (Self::CLIENT_SYMBOLS.0.into(), "CLIENT_SYMBOLS", "ClientSymbols"),
            (Self::SERVER_SYMBOLS.0.into(), "SERVER_SYMBOLS", "ServerSymbols"),
            (Self::INDICATOR_MAPS.0.into(), "INDICATOR_MAPS", "IndicatorMaps"),
            (Self::KEY_NAMES.0.into(), "KEY_NAMES", "KeyNames"),
            (Self::GEOMETRY.0.into(), "GEOMETRY", "Geometry"),
            (Self::OTHER_NAMES.0.into(), "OTHER_NAMES", "OtherNames"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(GBNDetail, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct XIFeature(u16);
impl XIFeature {
    pub const KEYBOARDS: Self = Self(1 << 0);
    pub const BUTTON_ACTIONS: Self = Self(1 << 1);
    pub const INDICATOR_NAMES: Self = Self(1 << 2);
    pub const INDICATOR_MAPS: Self = Self(1 << 3);
    pub const INDICATOR_STATE: Self = Self(1 << 4);
}
impl From<XIFeature> for u16 {
    #[inline]
    fn from(input: XIFeature) -> Self {
        input.0
    }
}
impl From<XIFeature> for Option<u16> {
    #[inline]
    fn from(input: XIFeature) -> Self {
        Some(input.0)
    }
}
impl From<XIFeature> for u32 {
    #[inline]
    fn from(input: XIFeature) -> Self {
        u32::from(input.0)
    }
}
impl From<XIFeature> for Option<u32> {
    #[inline]
    fn from(input: XIFeature) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for XIFeature {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for XIFeature {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for XIFeature  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KEYBOARDS.0.into(), "KEYBOARDS", "Keyboards"),
            (Self::BUTTON_ACTIONS.0.into(), "BUTTON_ACTIONS", "ButtonActions"),
            (Self::INDICATOR_NAMES.0.into(), "INDICATOR_NAMES", "IndicatorNames"),
            (Self::INDICATOR_MAPS.0.into(), "INDICATOR_MAPS", "IndicatorMaps"),
            (Self::INDICATOR_STATE.0.into(), "INDICATOR_STATE", "IndicatorState"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(XIFeature, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PerClientFlag(u32);
impl PerClientFlag {
    pub const DETECTABLE_AUTO_REPEAT: Self = Self(1 << 0);
    pub const GRABS_USE_XKB_STATE: Self = Self(1 << 1);
    pub const AUTO_RESET_CONTROLS: Self = Self(1 << 2);
    pub const LOOKUP_STATE_WHEN_GRABBED: Self = Self(1 << 3);
    pub const SEND_EVENT_USES_XKB_STATE: Self = Self(1 << 4);
}
impl From<PerClientFlag> for u32 {
    #[inline]
    fn from(input: PerClientFlag) -> Self {
        input.0
    }
}
impl From<PerClientFlag> for Option<u32> {
    #[inline]
    fn from(input: PerClientFlag) -> Self {
        Some(input.0)
    }
}
impl From<u8> for PerClientFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for PerClientFlag {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for PerClientFlag {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PerClientFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::DETECTABLE_AUTO_REPEAT.0, "DETECTABLE_AUTO_REPEAT", "DetectableAutoRepeat"),
            (Self::GRABS_USE_XKB_STATE.0, "GRABS_USE_XKB_STATE", "GrabsUseXKBState"),
            (Self::AUTO_RESET_CONTROLS.0, "AUTO_RESET_CONTROLS", "AutoResetControls"),
            (Self::LOOKUP_STATE_WHEN_GRABBED.0, "LOOKUP_STATE_WHEN_GRABBED", "LookupStateWhenGrabbed"),
            (Self::SEND_EVENT_USES_XKB_STATE.0, "SEND_EVENT_USES_XKB_STATE", "SendEventUsesXKBState"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(PerClientFlag, u32);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModDef {
    pub mask: xproto::ModMask,
    pub real_mods: xproto::ModMask,
    pub vmods: VMod,
}
impl_debug_if_no_extra_traits!(ModDef, "ModDef");
impl TryParse for ModDef {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (mask, remaining) = u8::try_parse(remaining)?;
        let (real_mods, remaining) = u8::try_parse(remaining)?;
        let (vmods, remaining) = u16::try_parse(remaining)?;
        let mask = mask.into();
        let real_mods = real_mods.into();
        let vmods = vmods.into();
        let result = ModDef { mask, real_mods, vmods };
        Ok((result, remaining))
    }
}
impl Serialize for ModDef {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let mask_bytes = (u16::from(self.mask) as u8).serialize();
        let real_mods_bytes = (u16::from(self.real_mods) as u8).serialize();
        let vmods_bytes = u16::from(self.vmods).serialize();
        [
            mask_bytes[0],
            real_mods_bytes[0],
            vmods_bytes[0],
            vmods_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        (u16::from(self.mask) as u8).serialize_into(bytes);
        (u16::from(self.real_mods) as u8).serialize_into(bytes);
        u16::from(self.vmods).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyName {
    pub name: [u8; 4],
}
impl_debug_if_no_extra_traits!(KeyName, "KeyName");
impl TryParse for KeyName {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let result = KeyName { name };
        Ok((result, remaining))
    }
}
impl Serialize for KeyName {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        [
            self.name[0],
            self.name[1],
            self.name[2],
            self.name[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        bytes.extend_from_slice(&self.name);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyAlias {
    pub real: [u8; 4],
    pub alias: [u8; 4],
}
impl_debug_if_no_extra_traits!(KeyAlias, "KeyAlias");
impl TryParse for KeyAlias {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (real, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let (alias, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let result = KeyAlias { real, alias };
        Ok((result, remaining))
    }
}
impl Serialize for KeyAlias {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        [
            self.real[0],
            self.real[1],
            self.real[2],
            self.real[3],
            self.alias[0],
            self.alias[1],
            self.alias[2],
            self.alias[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        bytes.extend_from_slice(&self.real);
        bytes.extend_from_slice(&self.alias);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CountedString16 {
    pub string: Vec<u8>,
    pub alignment_pad: Vec<u8>,
}
impl_debug_if_no_extra_traits!(CountedString16, "CountedString16");
impl TryParse for CountedString16 {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (length, remaining) = u16::try_parse(remaining)?;
        let (string, remaining) = crate::x11_utils::parse_u8_list(remaining, length.try_to_usize()?)?;
        let string = string.to_vec();
        let (alignment_pad, remaining) = crate::x11_utils::parse_u8_list(remaining, (u32::from(length).checked_add(5u32).ok_or(ParseError::InvalidExpression)? & (!3u32)).checked_sub(u32::from(length).checked_add(2u32).ok_or(ParseError::InvalidExpression)?).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let alignment_pad = alignment_pad.to_vec();
        let result = CountedString16 { string, alignment_pad };
        Ok((result, remaining))
    }
}
impl Serialize for CountedString16 {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        let length = u16::try_from(self.string.len()).expect("`string` has too many elements");
        length.serialize_into(bytes);
        bytes.extend_from_slice(&self.string);
        assert_eq!(self.alignment_pad.len(), usize::try_from((u32::from(length).checked_add(5u32).unwrap() & (!3u32)).checked_sub(u32::from(length).checked_add(2u32).unwrap()).unwrap()).unwrap(), "`alignment_pad` has an incorrect length");
        bytes.extend_from_slice(&self.alignment_pad);
    }
}
impl CountedString16 {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `string` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u16 {
        self.string.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KTMapEntry {
    pub active: bool,
    pub mods_mask: xproto::ModMask,
    pub level: u8,
    pub mods_mods: xproto::ModMask,
    pub mods_vmods: VMod,
}
impl_debug_if_no_extra_traits!(KTMapEntry, "KTMapEntry");
impl TryParse for KTMapEntry {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (active, remaining) = bool::try_parse(remaining)?;
        let (mods_mask, remaining) = u8::try_parse(remaining)?;
        let (level, remaining) = u8::try_parse(remaining)?;
        let (mods_mods, remaining) = u8::try_parse(remaining)?;
        let (mods_vmods, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let mods_mask = mods_mask.into();
        let mods_mods = mods_mods.into();
        let mods_vmods = mods_vmods.into();
        let result = KTMapEntry { active, mods_mask, level, mods_mods, mods_vmods };
        Ok((result, remaining))
    }
}
impl Serialize for KTMapEntry {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let active_bytes = self.active.serialize();
        let mods_mask_bytes = (u16::from(self.mods_mask) as u8).serialize();
        let level_bytes = self.level.serialize();
        let mods_mods_bytes = (u16::from(self.mods_mods) as u8).serialize();
        let mods_vmods_bytes = u16::from(self.mods_vmods).serialize();
        [
            active_bytes[0],
            mods_mask_bytes[0],
            level_bytes[0],
            mods_mods_bytes[0],
            mods_vmods_bytes[0],
            mods_vmods_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.active.serialize_into(bytes);
        (u16::from(self.mods_mask) as u8).serialize_into(bytes);
        self.level.serialize_into(bytes);
        (u16::from(self.mods_mods) as u8).serialize_into(bytes);
        u16::from(self.mods_vmods).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyType {
    pub mods_mask: xproto::ModMask,
    pub mods_mods: xproto::ModMask,
    pub mods_vmods: VMod,
    pub num_levels: u8,
    pub has_preserve: bool,
    pub map: Vec<KTMapEntry>,
    pub preserve: Vec<ModDef>,
}
impl_debug_if_no_extra_traits!(KeyType, "KeyType");
impl TryParse for KeyType {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (mods_mask, remaining) = u8::try_parse(remaining)?;
        let (mods_mods, remaining) = u8::try_parse(remaining)?;
        let (mods_vmods, remaining) = u16::try_parse(remaining)?;
        let (num_levels, remaining) = u8::try_parse(remaining)?;
        let (n_map_entries, remaining) = u8::try_parse(remaining)?;
        let (has_preserve, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (map, remaining) = crate::x11_utils::parse_list::<KTMapEntry>(remaining, n_map_entries.try_to_usize()?)?;
        let (preserve, remaining) = crate::x11_utils::parse_list::<ModDef>(remaining, u32::from(has_preserve).checked_mul(u32::from(n_map_entries)).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let mods_mask = mods_mask.into();
        let mods_mods = mods_mods.into();
        let mods_vmods = mods_vmods.into();
        let result = KeyType { mods_mask, mods_mods, mods_vmods, num_levels, has_preserve, map, preserve };
        Ok((result, remaining))
    }
}
impl Serialize for KeyType {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        (u16::from(self.mods_mask) as u8).serialize_into(bytes);
        (u16::from(self.mods_mods) as u8).serialize_into(bytes);
        u16::from(self.mods_vmods).serialize_into(bytes);
        self.num_levels.serialize_into(bytes);
        let n_map_entries = u8::try_from(self.map.len()).expect("`map` has too many elements");
        n_map_entries.serialize_into(bytes);
        self.has_preserve.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.map.serialize_into(bytes);
        assert_eq!(self.preserve.len(), usize::try_from(u32::from(self.has_preserve).checked_mul(u32::from(n_map_entries)).unwrap()).unwrap(), "`preserve` has an incorrect length");
        self.preserve.serialize_into(bytes);
    }
}
impl KeyType {
    /// Get the value of the `nMapEntries` field.
    ///
    /// The `nMapEntries` field is used as the length field of the `map` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_map_entries(&self) -> u8 {
        self.map.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeySymMap {
    pub kt_index: [u8; 4],
    pub group_info: u8,
    pub width: u8,
    pub syms: Vec<xproto::Keysym>,
}
impl_debug_if_no_extra_traits!(KeySymMap, "KeySymMap");
impl TryParse for KeySymMap {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (kt_index, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let (group_info, remaining) = u8::try_parse(remaining)?;
        let (width, remaining) = u8::try_parse(remaining)?;
        let (n_syms, remaining) = u16::try_parse(remaining)?;
        let (syms, remaining) = crate::x11_utils::parse_list::<xproto::Keysym>(remaining, n_syms.try_to_usize()?)?;
        let result = KeySymMap { kt_index, group_info, width, syms };
        Ok((result, remaining))
    }
}
impl Serialize for KeySymMap {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        bytes.extend_from_slice(&self.kt_index);
        self.group_info.serialize_into(bytes);
        self.width.serialize_into(bytes);
        let n_syms = u16::try_from(self.syms.len()).expect("`syms` has too many elements");
        n_syms.serialize_into(bytes);
        self.syms.serialize_into(bytes);
    }
}
impl KeySymMap {
    /// Get the value of the `nSyms` field.
    ///
    /// The `nSyms` field is used as the length field of the `syms` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_syms(&self) -> u16 {
        self.syms.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CommonBehavior {
    pub type_: u8,
    pub data: u8,
}
impl_debug_if_no_extra_traits!(CommonBehavior, "CommonBehavior");
impl TryParse for CommonBehavior {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (data, remaining) = u8::try_parse(remaining)?;
        let result = CommonBehavior { type_, data };
        Ok((result, remaining))
    }
}
impl Serialize for CommonBehavior {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let type_bytes = self.type_.serialize();
        let data_bytes = self.data.serialize();
        [
            type_bytes[0],
            data_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.type_.serialize_into(bytes);
        self.data.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DefaultBehavior {
    pub type_: u8,
}
impl_debug_if_no_extra_traits!(DefaultBehavior, "DefaultBehavior");
impl TryParse for DefaultBehavior {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let result = DefaultBehavior { type_ };
        Ok((result, remaining))
    }
}
impl Serialize for DefaultBehavior {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let type_bytes = self.type_.serialize();
        [
            type_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.type_.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}

pub type LockBehavior = DefaultBehavior;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RadioGroupBehavior {
    pub type_: u8,
    pub group: u8,
}
impl_debug_if_no_extra_traits!(RadioGroupBehavior, "RadioGroupBehavior");
impl TryParse for RadioGroupBehavior {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (group, remaining) = u8::try_parse(remaining)?;
        let result = RadioGroupBehavior { type_, group };
        Ok((result, remaining))
    }
}
impl Serialize for RadioGroupBehavior {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let type_bytes = self.type_.serialize();
        let group_bytes = self.group.serialize();
        [
            type_bytes[0],
            group_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.type_.serialize_into(bytes);
        self.group.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayBehavior {
    pub type_: u8,
    pub key: xproto::Keycode,
}
impl_debug_if_no_extra_traits!(OverlayBehavior, "OverlayBehavior");
impl TryParse for OverlayBehavior {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let result = OverlayBehavior { type_, key };
        Ok((result, remaining))
    }
}
impl Serialize for OverlayBehavior {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let type_bytes = self.type_.serialize();
        let key_bytes = self.key.serialize();
        [
            type_bytes[0],
            key_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.type_.serialize_into(bytes);
        self.key.serialize_into(bytes);
    }
}

pub type PermamentLockBehavior = LockBehavior;

pub type PermamentRadioGroupBehavior = RadioGroupBehavior;

pub type PermamentOverlayBehavior = OverlayBehavior;

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Behavior([u8; 2]);
impl Behavior {
    pub fn as_common(&self) -> CommonBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<CommonBehavior, ParseError> {
            let (common, remaining) = CommonBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(common)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_default(&self) -> DefaultBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<DefaultBehavior, ParseError> {
            let (default, remaining) = DefaultBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(default)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lock(&self) -> LockBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<LockBehavior, ParseError> {
            let (lock, remaining) = LockBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(lock)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_radio_group(&self) -> RadioGroupBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<RadioGroupBehavior, ParseError> {
            let (radio_group, remaining) = RadioGroupBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(radio_group)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_overlay1(&self) -> OverlayBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<OverlayBehavior, ParseError> {
            let (overlay1, remaining) = OverlayBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(overlay1)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_overlay2(&self) -> OverlayBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<OverlayBehavior, ParseError> {
            let (overlay2, remaining) = OverlayBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(overlay2)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_permament_lock(&self) -> PermamentLockBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<PermamentLockBehavior, ParseError> {
            let (permament_lock, remaining) = PermamentLockBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(permament_lock)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_permament_radio_group(&self) -> PermamentRadioGroupBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<PermamentRadioGroupBehavior, ParseError> {
            let (permament_radio_group, remaining) = PermamentRadioGroupBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(permament_radio_group)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_permament_overlay1(&self) -> PermamentOverlayBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<PermamentOverlayBehavior, ParseError> {
            let (permament_overlay1, remaining) = PermamentOverlayBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(permament_overlay1)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_permament_overlay2(&self) -> PermamentOverlayBehavior {
        fn do_the_parse(remaining: &[u8]) -> Result<PermamentOverlayBehavior, ParseError> {
            let (permament_overlay2, remaining) = PermamentOverlayBehavior::try_parse(remaining)?;
            let _ = remaining;
            Ok(permament_overlay2)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_type(&self) -> u8 {
        fn do_the_parse(remaining: &[u8]) -> Result<u8, ParseError> {
            let (type_, remaining) = u8::try_parse(remaining)?;
            let _ = remaining;
            Ok(type_)
        }
        do_the_parse(&self.0).unwrap()
    }
}
impl Serialize for Behavior {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        self.0
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }
}
impl TryParse for Behavior {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let inner: [u8; 2] = value.get(..2)
            .ok_or(ParseError::InsufficientData)?
            .try_into()
            .unwrap();
        let result = Behavior(inner);
        Ok((result, &value[2..]))
    }
}
impl From<CommonBehavior> for Behavior {
    fn from(common: CommonBehavior) -> Self {
        let common_bytes = common.serialize();
        Self(common_bytes)
    }
}
impl From<DefaultBehavior> for Behavior {
    fn from(default: DefaultBehavior) -> Self {
        let default_bytes = default.serialize();
        Self(default_bytes)
    }
}
impl From<RadioGroupBehavior> for Behavior {
    fn from(radio_group: RadioGroupBehavior) -> Self {
        let radio_group_bytes = radio_group.serialize();
        Self(radio_group_bytes)
    }
}
impl From<OverlayBehavior> for Behavior {
    fn from(overlay1: OverlayBehavior) -> Self {
        let overlay1_bytes = overlay1.serialize();
        Self(overlay1_bytes)
    }
}
impl From<u8> for Behavior {
    fn from(type_: u8) -> Self {
        let type_bytes = type_.serialize();
        let value = [
            type_bytes[0],
            0,
        ];
        Self(value)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BehaviorType(u8);
impl BehaviorType {
    pub const DEFAULT: Self = Self(0);
    pub const LOCK: Self = Self(1);
    pub const RADIO_GROUP: Self = Self(2);
    pub const OVERLAY1: Self = Self(3);
    pub const OVERLAY2: Self = Self(4);
    pub const PERMAMENT_LOCK: Self = Self(129);
    pub const PERMAMENT_RADIO_GROUP: Self = Self(130);
    pub const PERMAMENT_OVERLAY1: Self = Self(131);
    pub const PERMAMENT_OVERLAY2: Self = Self(132);
}
impl From<BehaviorType> for u8 {
    #[inline]
    fn from(input: BehaviorType) -> Self {
        input.0
    }
}
impl From<BehaviorType> for Option<u8> {
    #[inline]
    fn from(input: BehaviorType) -> Self {
        Some(input.0)
    }
}
impl From<BehaviorType> for u16 {
    #[inline]
    fn from(input: BehaviorType) -> Self {
        u16::from(input.0)
    }
}
impl From<BehaviorType> for Option<u16> {
    #[inline]
    fn from(input: BehaviorType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<BehaviorType> for u32 {
    #[inline]
    fn from(input: BehaviorType) -> Self {
        u32::from(input.0)
    }
}
impl From<BehaviorType> for Option<u32> {
    #[inline]
    fn from(input: BehaviorType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for BehaviorType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BehaviorType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::DEFAULT.0.into(), "DEFAULT", "Default"),
            (Self::LOCK.0.into(), "LOCK", "Lock"),
            (Self::RADIO_GROUP.0.into(), "RADIO_GROUP", "RadioGroup"),
            (Self::OVERLAY1.0.into(), "OVERLAY1", "Overlay1"),
            (Self::OVERLAY2.0.into(), "OVERLAY2", "Overlay2"),
            (Self::PERMAMENT_LOCK.0.into(), "PERMAMENT_LOCK", "PermamentLock"),
            (Self::PERMAMENT_RADIO_GROUP.0.into(), "PERMAMENT_RADIO_GROUP", "PermamentRadioGroup"),
            (Self::PERMAMENT_OVERLAY1.0.into(), "PERMAMENT_OVERLAY1", "PermamentOverlay1"),
            (Self::PERMAMENT_OVERLAY2.0.into(), "PERMAMENT_OVERLAY2", "PermamentOverlay2"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetBehavior {
    pub keycode: xproto::Keycode,
    pub behavior: Behavior,
}
impl_debug_if_no_extra_traits!(SetBehavior, "SetBehavior");
impl TryParse for SetBehavior {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (behavior, remaining) = Behavior::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let result = SetBehavior { keycode, behavior };
        Ok((result, remaining))
    }
}
impl Serialize for SetBehavior {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let keycode_bytes = self.keycode.serialize();
        let behavior_bytes = self.behavior.serialize();
        [
            keycode_bytes[0],
            behavior_bytes[0],
            behavior_bytes[1],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.keycode.serialize_into(bytes);
        self.behavior.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetExplicit {
    pub keycode: xproto::Keycode,
    pub explicit: Explicit,
}
impl_debug_if_no_extra_traits!(SetExplicit, "SetExplicit");
impl TryParse for SetExplicit {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (explicit, remaining) = u8::try_parse(remaining)?;
        let explicit = explicit.into();
        let result = SetExplicit { keycode, explicit };
        Ok((result, remaining))
    }
}
impl Serialize for SetExplicit {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let keycode_bytes = self.keycode.serialize();
        let explicit_bytes = u8::from(self.explicit).serialize();
        [
            keycode_bytes[0],
            explicit_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.keycode.serialize_into(bytes);
        u8::from(self.explicit).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyModMap {
    pub keycode: xproto::Keycode,
    pub mods: xproto::ModMask,
}
impl_debug_if_no_extra_traits!(KeyModMap, "KeyModMap");
impl TryParse for KeyModMap {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (mods, remaining) = u8::try_parse(remaining)?;
        let mods = mods.into();
        let result = KeyModMap { keycode, mods };
        Ok((result, remaining))
    }
}
impl Serialize for KeyModMap {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let keycode_bytes = self.keycode.serialize();
        let mods_bytes = (u16::from(self.mods) as u8).serialize();
        [
            keycode_bytes[0],
            mods_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.keycode.serialize_into(bytes);
        (u16::from(self.mods) as u8).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyVModMap {
    pub keycode: xproto::Keycode,
    pub vmods: VMod,
}
impl_debug_if_no_extra_traits!(KeyVModMap, "KeyVModMap");
impl TryParse for KeyVModMap {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (vmods, remaining) = u16::try_parse(remaining)?;
        let vmods = vmods.into();
        let result = KeyVModMap { keycode, vmods };
        Ok((result, remaining))
    }
}
impl Serialize for KeyVModMap {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let keycode_bytes = self.keycode.serialize();
        let vmods_bytes = u16::from(self.vmods).serialize();
        [
            keycode_bytes[0],
            0,
            vmods_bytes[0],
            vmods_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.keycode.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        u16::from(self.vmods).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KTSetMapEntry {
    pub level: u8,
    pub real_mods: xproto::ModMask,
    pub virtual_mods: VMod,
}
impl_debug_if_no_extra_traits!(KTSetMapEntry, "KTSetMapEntry");
impl TryParse for KTSetMapEntry {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (level, remaining) = u8::try_parse(remaining)?;
        let (real_mods, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let real_mods = real_mods.into();
        let virtual_mods = virtual_mods.into();
        let result = KTSetMapEntry { level, real_mods, virtual_mods };
        Ok((result, remaining))
    }
}
impl Serialize for KTSetMapEntry {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let level_bytes = self.level.serialize();
        let real_mods_bytes = (u16::from(self.real_mods) as u8).serialize();
        let virtual_mods_bytes = u16::from(self.virtual_mods).serialize();
        [
            level_bytes[0],
            real_mods_bytes[0],
            virtual_mods_bytes[0],
            virtual_mods_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.level.serialize_into(bytes);
        (u16::from(self.real_mods) as u8).serialize_into(bytes);
        u16::from(self.virtual_mods).serialize_into(bytes);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetKeyType {
    pub mask: xproto::ModMask,
    pub real_mods: xproto::ModMask,
    pub virtual_mods: VMod,
    pub num_levels: u8,
    pub preserve: bool,
    pub entries: Vec<KTSetMapEntry>,
    pub preserve_entries: Vec<KTSetMapEntry>,
}
impl_debug_if_no_extra_traits!(SetKeyType, "SetKeyType");
impl TryParse for SetKeyType {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (mask, remaining) = u8::try_parse(remaining)?;
        let (real_mods, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let (num_levels, remaining) = u8::try_parse(remaining)?;
        let (n_map_entries, remaining) = u8::try_parse(remaining)?;
        let (preserve, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (entries, remaining) = crate::x11_utils::parse_list::<KTSetMapEntry>(remaining, n_map_entries.try_to_usize()?)?;
        let (preserve_entries, remaining) = crate::x11_utils::parse_list::<KTSetMapEntry>(remaining, u32::from(preserve).checked_mul(u32::from(n_map_entries)).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let mask = mask.into();
        let real_mods = real_mods.into();
        let virtual_mods = virtual_mods.into();
        let result = SetKeyType { mask, real_mods, virtual_mods, num_levels, preserve, entries, preserve_entries };
        Ok((result, remaining))
    }
}
impl Serialize for SetKeyType {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        (u16::from(self.mask) as u8).serialize_into(bytes);
        (u16::from(self.real_mods) as u8).serialize_into(bytes);
        u16::from(self.virtual_mods).serialize_into(bytes);
        self.num_levels.serialize_into(bytes);
        let n_map_entries = u8::try_from(self.entries.len()).expect("`entries` has too many elements");
        n_map_entries.serialize_into(bytes);
        self.preserve.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.entries.serialize_into(bytes);
        assert_eq!(self.preserve_entries.len(), usize::try_from(u32::from(self.preserve).checked_mul(u32::from(n_map_entries)).unwrap()).unwrap(), "`preserve_entries` has an incorrect length");
        self.preserve_entries.serialize_into(bytes);
    }
}
impl SetKeyType {
    /// Get the value of the `nMapEntries` field.
    ///
    /// The `nMapEntries` field is used as the length field of the `entries` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_map_entries(&self) -> u8 {
        self.entries.len()
            .try_into().unwrap()
    }
}

pub type String8 = u8;

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Outline {
    pub corner_radius: u8,
    pub points: Vec<xproto::Point>,
}
impl_debug_if_no_extra_traits!(Outline, "Outline");
impl TryParse for Outline {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (n_points, remaining) = u8::try_parse(remaining)?;
        let (corner_radius, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (points, remaining) = crate::x11_utils::parse_list::<xproto::Point>(remaining, n_points.try_to_usize()?)?;
        let result = Outline { corner_radius, points };
        Ok((result, remaining))
    }
}
impl Serialize for Outline {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        let n_points = u8::try_from(self.points.len()).expect("`points` has too many elements");
        n_points.serialize_into(bytes);
        self.corner_radius.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.points.serialize_into(bytes);
    }
}
impl Outline {
    /// Get the value of the `nPoints` field.
    ///
    /// The `nPoints` field is used as the length field of the `points` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_points(&self) -> u8 {
        self.points.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shape {
    pub name: xproto::Atom,
    pub primary_ndx: u8,
    pub approx_ndx: u8,
    pub outlines: Vec<Outline>,
}
impl_debug_if_no_extra_traits!(Shape, "Shape");
impl TryParse for Shape {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (n_outlines, remaining) = u8::try_parse(remaining)?;
        let (primary_ndx, remaining) = u8::try_parse(remaining)?;
        let (approx_ndx, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (outlines, remaining) = crate::x11_utils::parse_list::<Outline>(remaining, n_outlines.try_to_usize()?)?;
        let result = Shape { name, primary_ndx, approx_ndx, outlines };
        Ok((result, remaining))
    }
}
impl Serialize for Shape {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.name.serialize_into(bytes);
        let n_outlines = u8::try_from(self.outlines.len()).expect("`outlines` has too many elements");
        n_outlines.serialize_into(bytes);
        self.primary_ndx.serialize_into(bytes);
        self.approx_ndx.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.outlines.serialize_into(bytes);
    }
}
impl Shape {
    /// Get the value of the `nOutlines` field.
    ///
    /// The `nOutlines` field is used as the length field of the `outlines` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_outlines(&self) -> u8 {
        self.outlines.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Key {
    pub name: [String8; 4],
    pub gap: i16,
    pub shape_ndx: u8,
    pub color_ndx: u8,
}
impl_debug_if_no_extra_traits!(Key, "Key");
impl TryParse for Key {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let (gap, remaining) = i16::try_parse(remaining)?;
        let (shape_ndx, remaining) = u8::try_parse(remaining)?;
        let (color_ndx, remaining) = u8::try_parse(remaining)?;
        let result = Key { name, gap, shape_ndx, color_ndx };
        Ok((result, remaining))
    }
}
impl Serialize for Key {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let gap_bytes = self.gap.serialize();
        let shape_ndx_bytes = self.shape_ndx.serialize();
        let color_ndx_bytes = self.color_ndx.serialize();
        [
            self.name[0],
            self.name[1],
            self.name[2],
            self.name[3],
            gap_bytes[0],
            gap_bytes[1],
            shape_ndx_bytes[0],
            color_ndx_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        bytes.extend_from_slice(&self.name);
        self.gap.serialize_into(bytes);
        self.shape_ndx.serialize_into(bytes);
        self.color_ndx.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayKey {
    pub over: [String8; 4],
    pub under: [String8; 4],
}
impl_debug_if_no_extra_traits!(OverlayKey, "OverlayKey");
impl TryParse for OverlayKey {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (over, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let (under, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let result = OverlayKey { over, under };
        Ok((result, remaining))
    }
}
impl Serialize for OverlayKey {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        [
            self.over[0],
            self.over[1],
            self.over[2],
            self.over[3],
            self.under[0],
            self.under[1],
            self.under[2],
            self.under[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        bytes.extend_from_slice(&self.over);
        bytes.extend_from_slice(&self.under);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayRow {
    pub row_under: u8,
    pub keys: Vec<OverlayKey>,
}
impl_debug_if_no_extra_traits!(OverlayRow, "OverlayRow");
impl TryParse for OverlayRow {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (row_under, remaining) = u8::try_parse(remaining)?;
        let (n_keys, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (keys, remaining) = crate::x11_utils::parse_list::<OverlayKey>(remaining, n_keys.try_to_usize()?)?;
        let result = OverlayRow { row_under, keys };
        Ok((result, remaining))
    }
}
impl Serialize for OverlayRow {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.row_under.serialize_into(bytes);
        let n_keys = u8::try_from(self.keys.len()).expect("`keys` has too many elements");
        n_keys.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.keys.serialize_into(bytes);
    }
}
impl OverlayRow {
    /// Get the value of the `nKeys` field.
    ///
    /// The `nKeys` field is used as the length field of the `keys` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_keys(&self) -> u8 {
        self.keys.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Overlay {
    pub name: xproto::Atom,
    pub rows: Vec<OverlayRow>,
}
impl_debug_if_no_extra_traits!(Overlay, "Overlay");
impl TryParse for Overlay {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (n_rows, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (rows, remaining) = crate::x11_utils::parse_list::<OverlayRow>(remaining, n_rows.try_to_usize()?)?;
        let result = Overlay { name, rows };
        Ok((result, remaining))
    }
}
impl Serialize for Overlay {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.name.serialize_into(bytes);
        let n_rows = u8::try_from(self.rows.len()).expect("`rows` has too many elements");
        n_rows.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        self.rows.serialize_into(bytes);
    }
}
impl Overlay {
    /// Get the value of the `nRows` field.
    ///
    /// The `nRows` field is used as the length field of the `rows` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_rows(&self) -> u8 {
        self.rows.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Row {
    pub top: i16,
    pub left: i16,
    pub vertical: bool,
    pub keys: Vec<Key>,
}
impl_debug_if_no_extra_traits!(Row, "Row");
impl TryParse for Row {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (top, remaining) = i16::try_parse(remaining)?;
        let (left, remaining) = i16::try_parse(remaining)?;
        let (n_keys, remaining) = u8::try_parse(remaining)?;
        let (vertical, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (keys, remaining) = crate::x11_utils::parse_list::<Key>(remaining, n_keys.try_to_usize()?)?;
        let result = Row { top, left, vertical, keys };
        Ok((result, remaining))
    }
}
impl Serialize for Row {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.top.serialize_into(bytes);
        self.left.serialize_into(bytes);
        let n_keys = u8::try_from(self.keys.len()).expect("`keys` has too many elements");
        n_keys.serialize_into(bytes);
        self.vertical.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.keys.serialize_into(bytes);
    }
}
impl Row {
    /// Get the value of the `nKeys` field.
    ///
    /// The `nKeys` field is used as the length field of the `keys` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_keys(&self) -> u8 {
        self.keys.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DoodadType(u8);
impl DoodadType {
    pub const OUTLINE: Self = Self(1);
    pub const SOLID: Self = Self(2);
    pub const TEXT: Self = Self(3);
    pub const INDICATOR: Self = Self(4);
    pub const LOGO: Self = Self(5);
}
impl From<DoodadType> for u8 {
    #[inline]
    fn from(input: DoodadType) -> Self {
        input.0
    }
}
impl From<DoodadType> for Option<u8> {
    #[inline]
    fn from(input: DoodadType) -> Self {
        Some(input.0)
    }
}
impl From<DoodadType> for u16 {
    #[inline]
    fn from(input: DoodadType) -> Self {
        u16::from(input.0)
    }
}
impl From<DoodadType> for Option<u16> {
    #[inline]
    fn from(input: DoodadType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<DoodadType> for u32 {
    #[inline]
    fn from(input: DoodadType) -> Self {
        u32::from(input.0)
    }
}
impl From<DoodadType> for Option<u32> {
    #[inline]
    fn from(input: DoodadType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for DoodadType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for DoodadType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::OUTLINE.0.into(), "OUTLINE", "Outline"),
            (Self::SOLID.0.into(), "SOLID", "Solid"),
            (Self::TEXT.0.into(), "TEXT", "Text"),
            (Self::INDICATOR.0.into(), "INDICATOR", "Indicator"),
            (Self::LOGO.0.into(), "LOGO", "Logo"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Listing {
    pub flags: u16,
    pub string: Vec<String8>,
}
impl_debug_if_no_extra_traits!(Listing, "Listing");
impl TryParse for Listing {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (flags, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u16::try_parse(remaining)?;
        let (string, remaining) = crate::x11_utils::parse_u8_list(remaining, length.try_to_usize()?)?;
        let string = string.to_vec();
        // Align offset to multiple of 2
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (2 - (offset % 2)) % 2;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let result = Listing { flags, string };
        Ok((result, remaining))
    }
}
impl Serialize for Listing {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.flags.serialize_into(bytes);
        let length = u16::try_from(self.string.len()).expect("`string` has too many elements");
        length.serialize_into(bytes);
        bytes.extend_from_slice(&self.string);
        bytes.extend_from_slice(&[0; 1][..(2 - (bytes.len() % 2)) % 2]);
    }
}
impl Listing {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `string` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u16 {
        self.string.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceLedInfo {
    pub led_class: LedClass,
    pub led_id: IDSpec,
    pub names_present: u32,
    pub maps_present: u32,
    pub phys_indicators: u32,
    pub state: u32,
    pub names: Vec<xproto::Atom>,
    pub maps: Vec<IndicatorMap>,
}
impl_debug_if_no_extra_traits!(DeviceLedInfo, "DeviceLedInfo");
impl TryParse for DeviceLedInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (led_class, remaining) = LedClassSpec::try_parse(remaining)?;
        let (led_id, remaining) = IDSpec::try_parse(remaining)?;
        let (names_present, remaining) = u32::try_parse(remaining)?;
        let (maps_present, remaining) = u32::try_parse(remaining)?;
        let (phys_indicators, remaining) = u32::try_parse(remaining)?;
        let (state, remaining) = u32::try_parse(remaining)?;
        let (names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(names_present).count_ones().try_to_usize()?)?;
        let (maps, remaining) = crate::x11_utils::parse_list::<IndicatorMap>(remaining, u32::from(maps_present).count_ones().try_to_usize()?)?;
        let led_class = led_class.into();
        let result = DeviceLedInfo { led_class, led_id, names_present, maps_present, phys_indicators, state, names, maps };
        Ok((result, remaining))
    }
}
impl Serialize for DeviceLedInfo {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        LedClassSpec::from(self.led_class).serialize_into(bytes);
        self.led_id.serialize_into(bytes);
        self.names_present.serialize_into(bytes);
        self.maps_present.serialize_into(bytes);
        self.phys_indicators.serialize_into(bytes);
        self.state.serialize_into(bytes);
        assert_eq!(self.names.len(), usize::try_from(u32::from(self.names_present).count_ones()).unwrap(), "`names` has an incorrect length");
        self.names.serialize_into(bytes);
        assert_eq!(self.maps.len(), usize::try_from(u32::from(self.maps_present).count_ones()).unwrap(), "`maps` has an incorrect length");
        self.maps.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Error(u8);
impl Error {
    pub const BAD_DEVICE: Self = Self(255);
    pub const BAD_CLASS: Self = Self(254);
    pub const BAD_ID: Self = Self(253);
}
impl From<Error> for u8 {
    #[inline]
    fn from(input: Error) -> Self {
        input.0
    }
}
impl From<Error> for Option<u8> {
    #[inline]
    fn from(input: Error) -> Self {
        Some(input.0)
    }
}
impl From<Error> for u16 {
    #[inline]
    fn from(input: Error) -> Self {
        u16::from(input.0)
    }
}
impl From<Error> for Option<u16> {
    #[inline]
    fn from(input: Error) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Error> for u32 {
    #[inline]
    fn from(input: Error) -> Self {
        u32::from(input.0)
    }
}
impl From<Error> for Option<u32> {
    #[inline]
    fn from(input: Error) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Error {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Error  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::BAD_DEVICE.0.into(), "BAD_DEVICE", "BadDevice"),
            (Self::BAD_CLASS.0.into(), "BAD_CLASS", "BadClass"),
            (Self::BAD_ID.0.into(), "BAD_ID", "BadId"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the Keyboard error
pub const KEYBOARD_ERROR: u8 = 0;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SA(u8);
impl SA {
    pub const CLEAR_LOCKS: Self = Self(1 << 0);
    pub const LATCH_TO_LOCK: Self = Self(1 << 1);
    pub const USE_MOD_MAP_MODS: Self = Self(1 << 2);
    pub const GROUP_ABSOLUTE: Self = Self(1 << 2);
}
impl From<SA> for u8 {
    #[inline]
    fn from(input: SA) -> Self {
        input.0
    }
}
impl From<SA> for Option<u8> {
    #[inline]
    fn from(input: SA) -> Self {
        Some(input.0)
    }
}
impl From<SA> for u16 {
    #[inline]
    fn from(input: SA) -> Self {
        u16::from(input.0)
    }
}
impl From<SA> for Option<u16> {
    #[inline]
    fn from(input: SA) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SA> for u32 {
    #[inline]
    fn from(input: SA) -> Self {
        u32::from(input.0)
    }
}
impl From<SA> for Option<u32> {
    #[inline]
    fn from(input: SA) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SA {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SA  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CLEAR_LOCKS.0.into(), "CLEAR_LOCKS", "ClearLocks"),
            (Self::LATCH_TO_LOCK.0.into(), "LATCH_TO_LOCK", "LatchToLock"),
            (Self::USE_MOD_MAP_MODS.0.into(), "USE_MOD_MAP_MODS", "UseModMapMods"),
            (Self::GROUP_ABSOLUTE.0.into(), "GROUP_ABSOLUTE", "GroupAbsolute"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SA, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAType(u8);
impl SAType {
    pub const NO_ACTION: Self = Self(0);
    pub const SET_MODS: Self = Self(1);
    pub const LATCH_MODS: Self = Self(2);
    pub const LOCK_MODS: Self = Self(3);
    pub const SET_GROUP: Self = Self(4);
    pub const LATCH_GROUP: Self = Self(5);
    pub const LOCK_GROUP: Self = Self(6);
    pub const MOVE_PTR: Self = Self(7);
    pub const PTR_BTN: Self = Self(8);
    pub const LOCK_PTR_BTN: Self = Self(9);
    pub const SET_PTR_DFLT: Self = Self(10);
    pub const ISO_LOCK: Self = Self(11);
    pub const TERMINATE: Self = Self(12);
    pub const SWITCH_SCREEN: Self = Self(13);
    pub const SET_CONTROLS: Self = Self(14);
    pub const LOCK_CONTROLS: Self = Self(15);
    pub const ACTION_MESSAGE: Self = Self(16);
    pub const REDIRECT_KEY: Self = Self(17);
    pub const DEVICE_BTN: Self = Self(18);
    pub const LOCK_DEVICE_BTN: Self = Self(19);
    pub const DEVICE_VALUATOR: Self = Self(20);
}
impl From<SAType> for u8 {
    #[inline]
    fn from(input: SAType) -> Self {
        input.0
    }
}
impl From<SAType> for Option<u8> {
    #[inline]
    fn from(input: SAType) -> Self {
        Some(input.0)
    }
}
impl From<SAType> for u16 {
    #[inline]
    fn from(input: SAType) -> Self {
        u16::from(input.0)
    }
}
impl From<SAType> for Option<u16> {
    #[inline]
    fn from(input: SAType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SAType> for u32 {
    #[inline]
    fn from(input: SAType) -> Self {
        u32::from(input.0)
    }
}
impl From<SAType> for Option<u32> {
    #[inline]
    fn from(input: SAType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SAType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SAType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NO_ACTION.0.into(), "NO_ACTION", "NoAction"),
            (Self::SET_MODS.0.into(), "SET_MODS", "SetMods"),
            (Self::LATCH_MODS.0.into(), "LATCH_MODS", "LatchMods"),
            (Self::LOCK_MODS.0.into(), "LOCK_MODS", "LockMods"),
            (Self::SET_GROUP.0.into(), "SET_GROUP", "SetGroup"),
            (Self::LATCH_GROUP.0.into(), "LATCH_GROUP", "LatchGroup"),
            (Self::LOCK_GROUP.0.into(), "LOCK_GROUP", "LockGroup"),
            (Self::MOVE_PTR.0.into(), "MOVE_PTR", "MovePtr"),
            (Self::PTR_BTN.0.into(), "PTR_BTN", "PtrBtn"),
            (Self::LOCK_PTR_BTN.0.into(), "LOCK_PTR_BTN", "LockPtrBtn"),
            (Self::SET_PTR_DFLT.0.into(), "SET_PTR_DFLT", "SetPtrDflt"),
            (Self::ISO_LOCK.0.into(), "ISO_LOCK", "ISOLock"),
            (Self::TERMINATE.0.into(), "TERMINATE", "Terminate"),
            (Self::SWITCH_SCREEN.0.into(), "SWITCH_SCREEN", "SwitchScreen"),
            (Self::SET_CONTROLS.0.into(), "SET_CONTROLS", "SetControls"),
            (Self::LOCK_CONTROLS.0.into(), "LOCK_CONTROLS", "LockControls"),
            (Self::ACTION_MESSAGE.0.into(), "ACTION_MESSAGE", "ActionMessage"),
            (Self::REDIRECT_KEY.0.into(), "REDIRECT_KEY", "RedirectKey"),
            (Self::DEVICE_BTN.0.into(), "DEVICE_BTN", "DeviceBtn"),
            (Self::LOCK_DEVICE_BTN.0.into(), "LOCK_DEVICE_BTN", "LockDeviceBtn"),
            (Self::DEVICE_VALUATOR.0.into(), "DEVICE_VALUATOR", "DeviceValuator"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SANoAction {
    pub type_: SAType,
}
impl_debug_if_no_extra_traits!(SANoAction, "SANoAction");
impl TryParse for SANoAction {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(7..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let result = SANoAction { type_ };
        Ok((result, remaining))
    }
}
impl Serialize for SANoAction {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        [
            type_bytes[0],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 7]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SASetMods {
    pub type_: SAType,
    pub flags: SA,
    pub mask: xproto::ModMask,
    pub real_mods: xproto::ModMask,
    pub vmods_high: VModsHigh,
    pub vmods_low: VModsLow,
}
impl_debug_if_no_extra_traits!(SASetMods, "SASetMods");
impl TryParse for SASetMods {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (mask, remaining) = u8::try_parse(remaining)?;
        let (real_mods, remaining) = u8::try_parse(remaining)?;
        let (vmods_high, remaining) = u8::try_parse(remaining)?;
        let (vmods_low, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let flags = flags.into();
        let mask = mask.into();
        let real_mods = real_mods.into();
        let vmods_high = vmods_high.into();
        let vmods_low = vmods_low.into();
        let result = SASetMods { type_, flags, mask, real_mods, vmods_high, vmods_low };
        Ok((result, remaining))
    }
}
impl Serialize for SASetMods {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        let mask_bytes = (u16::from(self.mask) as u8).serialize();
        let real_mods_bytes = (u16::from(self.real_mods) as u8).serialize();
        let vmods_high_bytes = u8::from(self.vmods_high).serialize();
        let vmods_low_bytes = u8::from(self.vmods_low).serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            mask_bytes[0],
            real_mods_bytes[0],
            vmods_high_bytes[0],
            vmods_low_bytes[0],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        (u16::from(self.mask) as u8).serialize_into(bytes);
        (u16::from(self.real_mods) as u8).serialize_into(bytes);
        u8::from(self.vmods_high).serialize_into(bytes);
        u8::from(self.vmods_low).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

pub type SALatchMods = SASetMods;

pub type SALockMods = SASetMods;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SASetGroup {
    pub type_: SAType,
    pub flags: SA,
    pub group: i8,
}
impl_debug_if_no_extra_traits!(SASetGroup, "SASetGroup");
impl TryParse for SASetGroup {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (group, remaining) = i8::try_parse(remaining)?;
        let remaining = remaining.get(5..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let flags = flags.into();
        let result = SASetGroup { type_, flags, group };
        Ok((result, remaining))
    }
}
impl Serialize for SASetGroup {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        let group_bytes = self.group.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            group_bytes[0],
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        self.group.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 5]);
    }
}

pub type SALatchGroup = SASetGroup;

pub type SALockGroup = SASetGroup;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAMovePtrFlag(u8);
impl SAMovePtrFlag {
    pub const NO_ACCELERATION: Self = Self(1 << 0);
    pub const MOVE_ABSOLUTE_X: Self = Self(1 << 1);
    pub const MOVE_ABSOLUTE_Y: Self = Self(1 << 2);
}
impl From<SAMovePtrFlag> for u8 {
    #[inline]
    fn from(input: SAMovePtrFlag) -> Self {
        input.0
    }
}
impl From<SAMovePtrFlag> for Option<u8> {
    #[inline]
    fn from(input: SAMovePtrFlag) -> Self {
        Some(input.0)
    }
}
impl From<SAMovePtrFlag> for u16 {
    #[inline]
    fn from(input: SAMovePtrFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<SAMovePtrFlag> for Option<u16> {
    #[inline]
    fn from(input: SAMovePtrFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SAMovePtrFlag> for u32 {
    #[inline]
    fn from(input: SAMovePtrFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<SAMovePtrFlag> for Option<u32> {
    #[inline]
    fn from(input: SAMovePtrFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SAMovePtrFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SAMovePtrFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NO_ACCELERATION.0.into(), "NO_ACCELERATION", "NoAcceleration"),
            (Self::MOVE_ABSOLUTE_X.0.into(), "MOVE_ABSOLUTE_X", "MoveAbsoluteX"),
            (Self::MOVE_ABSOLUTE_Y.0.into(), "MOVE_ABSOLUTE_Y", "MoveAbsoluteY"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SAMovePtrFlag, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAMovePtr {
    pub type_: SAType,
    pub flags: SAMovePtrFlag,
    pub x_high: i8,
    pub x_low: u8,
    pub y_high: i8,
    pub y_low: u8,
}
impl_debug_if_no_extra_traits!(SAMovePtr, "SAMovePtr");
impl TryParse for SAMovePtr {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (x_high, remaining) = i8::try_parse(remaining)?;
        let (x_low, remaining) = u8::try_parse(remaining)?;
        let (y_high, remaining) = i8::try_parse(remaining)?;
        let (y_low, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let flags = flags.into();
        let result = SAMovePtr { type_, flags, x_high, x_low, y_high, y_low };
        Ok((result, remaining))
    }
}
impl Serialize for SAMovePtr {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        let x_high_bytes = self.x_high.serialize();
        let x_low_bytes = self.x_low.serialize();
        let y_high_bytes = self.y_high.serialize();
        let y_low_bytes = self.y_low.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            x_high_bytes[0],
            x_low_bytes[0],
            y_high_bytes[0],
            y_low_bytes[0],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        self.x_high.serialize_into(bytes);
        self.x_low.serialize_into(bytes);
        self.y_high.serialize_into(bytes);
        self.y_low.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAPtrBtn {
    pub type_: SAType,
    pub flags: u8,
    pub count: u8,
    pub button: u8,
}
impl_debug_if_no_extra_traits!(SAPtrBtn, "SAPtrBtn");
impl TryParse for SAPtrBtn {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (count, remaining) = u8::try_parse(remaining)?;
        let (button, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let result = SAPtrBtn { type_, flags, count, button };
        Ok((result, remaining))
    }
}
impl Serialize for SAPtrBtn {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = self.flags.serialize();
        let count_bytes = self.count.serialize();
        let button_bytes = self.button.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            count_bytes[0],
            button_bytes[0],
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        self.flags.serialize_into(bytes);
        self.count.serialize_into(bytes);
        self.button.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SALockPtrBtn {
    pub type_: SAType,
    pub flags: u8,
    pub button: u8,
}
impl_debug_if_no_extra_traits!(SALockPtrBtn, "SALockPtrBtn");
impl TryParse for SALockPtrBtn {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (button, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let result = SALockPtrBtn { type_, flags, button };
        Ok((result, remaining))
    }
}
impl Serialize for SALockPtrBtn {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = self.flags.serialize();
        let button_bytes = self.button.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            0,
            button_bytes[0],
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        self.flags.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.button.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SASetPtrDfltFlag(u8);
impl SASetPtrDfltFlag {
    pub const DFLT_BTN_ABSOLUTE: Self = Self(1 << 2);
    pub const AFFECT_DFLT_BUTTON: Self = Self(1 << 0);
}
impl From<SASetPtrDfltFlag> for u8 {
    #[inline]
    fn from(input: SASetPtrDfltFlag) -> Self {
        input.0
    }
}
impl From<SASetPtrDfltFlag> for Option<u8> {
    #[inline]
    fn from(input: SASetPtrDfltFlag) -> Self {
        Some(input.0)
    }
}
impl From<SASetPtrDfltFlag> for u16 {
    #[inline]
    fn from(input: SASetPtrDfltFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<SASetPtrDfltFlag> for Option<u16> {
    #[inline]
    fn from(input: SASetPtrDfltFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SASetPtrDfltFlag> for u32 {
    #[inline]
    fn from(input: SASetPtrDfltFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<SASetPtrDfltFlag> for Option<u32> {
    #[inline]
    fn from(input: SASetPtrDfltFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SASetPtrDfltFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SASetPtrDfltFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::DFLT_BTN_ABSOLUTE.0.into(), "DFLT_BTN_ABSOLUTE", "DfltBtnAbsolute"),
            (Self::AFFECT_DFLT_BUTTON.0.into(), "AFFECT_DFLT_BUTTON", "AffectDfltButton"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SASetPtrDfltFlag, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SASetPtrDflt {
    pub type_: SAType,
    pub flags: SASetPtrDfltFlag,
    pub affect: SASetPtrDfltFlag,
    pub value: i8,
}
impl_debug_if_no_extra_traits!(SASetPtrDflt, "SASetPtrDflt");
impl TryParse for SASetPtrDflt {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (affect, remaining) = u8::try_parse(remaining)?;
        let (value, remaining) = i8::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let flags = flags.into();
        let affect = affect.into();
        let result = SASetPtrDflt { type_, flags, affect, value };
        Ok((result, remaining))
    }
}
impl Serialize for SASetPtrDflt {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        let affect_bytes = u8::from(self.affect).serialize();
        let value_bytes = self.value.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            affect_bytes[0],
            value_bytes[0],
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        u8::from(self.affect).serialize_into(bytes);
        self.value.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAIsoLockFlag(u8);
impl SAIsoLockFlag {
    pub const NO_LOCK: Self = Self(1 << 0);
    pub const NO_UNLOCK: Self = Self(1 << 1);
    pub const USE_MOD_MAP_MODS: Self = Self(1 << 2);
    pub const GROUP_ABSOLUTE: Self = Self(1 << 2);
    pub const ISO_DFLT_IS_GROUP: Self = Self(1 << 3);
}
impl From<SAIsoLockFlag> for u8 {
    #[inline]
    fn from(input: SAIsoLockFlag) -> Self {
        input.0
    }
}
impl From<SAIsoLockFlag> for Option<u8> {
    #[inline]
    fn from(input: SAIsoLockFlag) -> Self {
        Some(input.0)
    }
}
impl From<SAIsoLockFlag> for u16 {
    #[inline]
    fn from(input: SAIsoLockFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<SAIsoLockFlag> for Option<u16> {
    #[inline]
    fn from(input: SAIsoLockFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SAIsoLockFlag> for u32 {
    #[inline]
    fn from(input: SAIsoLockFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<SAIsoLockFlag> for Option<u32> {
    #[inline]
    fn from(input: SAIsoLockFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SAIsoLockFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SAIsoLockFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NO_LOCK.0.into(), "NO_LOCK", "NoLock"),
            (Self::NO_UNLOCK.0.into(), "NO_UNLOCK", "NoUnlock"),
            (Self::USE_MOD_MAP_MODS.0.into(), "USE_MOD_MAP_MODS", "UseModMapMods"),
            (Self::GROUP_ABSOLUTE.0.into(), "GROUP_ABSOLUTE", "GroupAbsolute"),
            (Self::ISO_DFLT_IS_GROUP.0.into(), "ISO_DFLT_IS_GROUP", "ISODfltIsGroup"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SAIsoLockFlag, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAIsoLockNoAffect(u8);
impl SAIsoLockNoAffect {
    pub const CTRLS: Self = Self(1 << 3);
    pub const PTR: Self = Self(1 << 4);
    pub const GROUP: Self = Self(1 << 5);
    pub const MODS: Self = Self(1 << 6);
}
impl From<SAIsoLockNoAffect> for u8 {
    #[inline]
    fn from(input: SAIsoLockNoAffect) -> Self {
        input.0
    }
}
impl From<SAIsoLockNoAffect> for Option<u8> {
    #[inline]
    fn from(input: SAIsoLockNoAffect) -> Self {
        Some(input.0)
    }
}
impl From<SAIsoLockNoAffect> for u16 {
    #[inline]
    fn from(input: SAIsoLockNoAffect) -> Self {
        u16::from(input.0)
    }
}
impl From<SAIsoLockNoAffect> for Option<u16> {
    #[inline]
    fn from(input: SAIsoLockNoAffect) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SAIsoLockNoAffect> for u32 {
    #[inline]
    fn from(input: SAIsoLockNoAffect) -> Self {
        u32::from(input.0)
    }
}
impl From<SAIsoLockNoAffect> for Option<u32> {
    #[inline]
    fn from(input: SAIsoLockNoAffect) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SAIsoLockNoAffect {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SAIsoLockNoAffect  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CTRLS.0.into(), "CTRLS", "Ctrls"),
            (Self::PTR.0.into(), "PTR", "Ptr"),
            (Self::GROUP.0.into(), "GROUP", "Group"),
            (Self::MODS.0.into(), "MODS", "Mods"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SAIsoLockNoAffect, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAIsoLock {
    pub type_: SAType,
    pub flags: SAIsoLockFlag,
    pub mask: xproto::ModMask,
    pub real_mods: xproto::ModMask,
    pub group: i8,
    pub affect: SAIsoLockNoAffect,
    pub vmods_high: VModsHigh,
    pub vmods_low: VModsLow,
}
impl_debug_if_no_extra_traits!(SAIsoLock, "SAIsoLock");
impl TryParse for SAIsoLock {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (mask, remaining) = u8::try_parse(remaining)?;
        let (real_mods, remaining) = u8::try_parse(remaining)?;
        let (group, remaining) = i8::try_parse(remaining)?;
        let (affect, remaining) = u8::try_parse(remaining)?;
        let (vmods_high, remaining) = u8::try_parse(remaining)?;
        let (vmods_low, remaining) = u8::try_parse(remaining)?;
        let type_ = type_.into();
        let flags = flags.into();
        let mask = mask.into();
        let real_mods = real_mods.into();
        let affect = affect.into();
        let vmods_high = vmods_high.into();
        let vmods_low = vmods_low.into();
        let result = SAIsoLock { type_, flags, mask, real_mods, group, affect, vmods_high, vmods_low };
        Ok((result, remaining))
    }
}
impl Serialize for SAIsoLock {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        let mask_bytes = (u16::from(self.mask) as u8).serialize();
        let real_mods_bytes = (u16::from(self.real_mods) as u8).serialize();
        let group_bytes = self.group.serialize();
        let affect_bytes = u8::from(self.affect).serialize();
        let vmods_high_bytes = u8::from(self.vmods_high).serialize();
        let vmods_low_bytes = u8::from(self.vmods_low).serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            mask_bytes[0],
            real_mods_bytes[0],
            group_bytes[0],
            affect_bytes[0],
            vmods_high_bytes[0],
            vmods_low_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        (u16::from(self.mask) as u8).serialize_into(bytes);
        (u16::from(self.real_mods) as u8).serialize_into(bytes);
        self.group.serialize_into(bytes);
        u8::from(self.affect).serialize_into(bytes);
        u8::from(self.vmods_high).serialize_into(bytes);
        u8::from(self.vmods_low).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SATerminate {
    pub type_: SAType,
}
impl_debug_if_no_extra_traits!(SATerminate, "SATerminate");
impl TryParse for SATerminate {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(7..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let result = SATerminate { type_ };
        Ok((result, remaining))
    }
}
impl Serialize for SATerminate {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        [
            type_bytes[0],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 7]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwitchScreenFlag(u8);
impl SwitchScreenFlag {
    pub const APPLICATION: Self = Self(1 << 0);
    pub const ABSOLUTE: Self = Self(1 << 2);
}
impl From<SwitchScreenFlag> for u8 {
    #[inline]
    fn from(input: SwitchScreenFlag) -> Self {
        input.0
    }
}
impl From<SwitchScreenFlag> for Option<u8> {
    #[inline]
    fn from(input: SwitchScreenFlag) -> Self {
        Some(input.0)
    }
}
impl From<SwitchScreenFlag> for u16 {
    #[inline]
    fn from(input: SwitchScreenFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<SwitchScreenFlag> for Option<u16> {
    #[inline]
    fn from(input: SwitchScreenFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SwitchScreenFlag> for u32 {
    #[inline]
    fn from(input: SwitchScreenFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<SwitchScreenFlag> for Option<u32> {
    #[inline]
    fn from(input: SwitchScreenFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SwitchScreenFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SwitchScreenFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::APPLICATION.0.into(), "APPLICATION", "Application"),
            (Self::ABSOLUTE.0.into(), "ABSOLUTE", "Absolute"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(SwitchScreenFlag, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SASwitchScreen {
    pub type_: SAType,
    pub flags: u8,
    pub new_screen: i8,
}
impl_debug_if_no_extra_traits!(SASwitchScreen, "SASwitchScreen");
impl TryParse for SASwitchScreen {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (new_screen, remaining) = i8::try_parse(remaining)?;
        let remaining = remaining.get(5..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let result = SASwitchScreen { type_, flags, new_screen };
        Ok((result, remaining))
    }
}
impl Serialize for SASwitchScreen {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = self.flags.serialize();
        let new_screen_bytes = self.new_screen.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            new_screen_bytes[0],
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        self.flags.serialize_into(bytes);
        self.new_screen.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 5]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoolCtrlsHigh(u8);
impl BoolCtrlsHigh {
    pub const ACCESS_X_FEEDBACK: Self = Self(1 << 0);
    pub const AUDIBLE_BELL: Self = Self(1 << 1);
    pub const OVERLAY1: Self = Self(1 << 2);
    pub const OVERLAY2: Self = Self(1 << 3);
    pub const IGNORE_GROUP_LOCK: Self = Self(1 << 4);
}
impl From<BoolCtrlsHigh> for u8 {
    #[inline]
    fn from(input: BoolCtrlsHigh) -> Self {
        input.0
    }
}
impl From<BoolCtrlsHigh> for Option<u8> {
    #[inline]
    fn from(input: BoolCtrlsHigh) -> Self {
        Some(input.0)
    }
}
impl From<BoolCtrlsHigh> for u16 {
    #[inline]
    fn from(input: BoolCtrlsHigh) -> Self {
        u16::from(input.0)
    }
}
impl From<BoolCtrlsHigh> for Option<u16> {
    #[inline]
    fn from(input: BoolCtrlsHigh) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<BoolCtrlsHigh> for u32 {
    #[inline]
    fn from(input: BoolCtrlsHigh) -> Self {
        u32::from(input.0)
    }
}
impl From<BoolCtrlsHigh> for Option<u32> {
    #[inline]
    fn from(input: BoolCtrlsHigh) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for BoolCtrlsHigh {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BoolCtrlsHigh  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ACCESS_X_FEEDBACK.0.into(), "ACCESS_X_FEEDBACK", "AccessXFeedback"),
            (Self::AUDIBLE_BELL.0.into(), "AUDIBLE_BELL", "AudibleBell"),
            (Self::OVERLAY1.0.into(), "OVERLAY1", "Overlay1"),
            (Self::OVERLAY2.0.into(), "OVERLAY2", "Overlay2"),
            (Self::IGNORE_GROUP_LOCK.0.into(), "IGNORE_GROUP_LOCK", "IgnoreGroupLock"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(BoolCtrlsHigh, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoolCtrlsLow(u8);
impl BoolCtrlsLow {
    pub const REPEAT_KEYS: Self = Self(1 << 0);
    pub const SLOW_KEYS: Self = Self(1 << 1);
    pub const BOUNCE_KEYS: Self = Self(1 << 2);
    pub const STICKY_KEYS: Self = Self(1 << 3);
    pub const MOUSE_KEYS: Self = Self(1 << 4);
    pub const MOUSE_KEYS_ACCEL: Self = Self(1 << 5);
    pub const ACCESS_X_KEYS: Self = Self(1 << 6);
    pub const ACCESS_X_TIMEOUT: Self = Self(1 << 7);
}
impl From<BoolCtrlsLow> for u8 {
    #[inline]
    fn from(input: BoolCtrlsLow) -> Self {
        input.0
    }
}
impl From<BoolCtrlsLow> for Option<u8> {
    #[inline]
    fn from(input: BoolCtrlsLow) -> Self {
        Some(input.0)
    }
}
impl From<BoolCtrlsLow> for u16 {
    #[inline]
    fn from(input: BoolCtrlsLow) -> Self {
        u16::from(input.0)
    }
}
impl From<BoolCtrlsLow> for Option<u16> {
    #[inline]
    fn from(input: BoolCtrlsLow) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<BoolCtrlsLow> for u32 {
    #[inline]
    fn from(input: BoolCtrlsLow) -> Self {
        u32::from(input.0)
    }
}
impl From<BoolCtrlsLow> for Option<u32> {
    #[inline]
    fn from(input: BoolCtrlsLow) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for BoolCtrlsLow {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BoolCtrlsLow  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::REPEAT_KEYS.0.into(), "REPEAT_KEYS", "RepeatKeys"),
            (Self::SLOW_KEYS.0.into(), "SLOW_KEYS", "SlowKeys"),
            (Self::BOUNCE_KEYS.0.into(), "BOUNCE_KEYS", "BounceKeys"),
            (Self::STICKY_KEYS.0.into(), "STICKY_KEYS", "StickyKeys"),
            (Self::MOUSE_KEYS.0.into(), "MOUSE_KEYS", "MouseKeys"),
            (Self::MOUSE_KEYS_ACCEL.0.into(), "MOUSE_KEYS_ACCEL", "MouseKeysAccel"),
            (Self::ACCESS_X_KEYS.0.into(), "ACCESS_X_KEYS", "AccessXKeys"),
            (Self::ACCESS_X_TIMEOUT.0.into(), "ACCESS_X_TIMEOUT", "AccessXTimeout"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(BoolCtrlsLow, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SASetControls {
    pub type_: SAType,
    pub bool_ctrls_high: BoolCtrlsHigh,
    pub bool_ctrls_low: BoolCtrlsLow,
}
impl_debug_if_no_extra_traits!(SASetControls, "SASetControls");
impl TryParse for SASetControls {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (bool_ctrls_high, remaining) = u8::try_parse(remaining)?;
        let (bool_ctrls_low, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let bool_ctrls_high = bool_ctrls_high.into();
        let bool_ctrls_low = bool_ctrls_low.into();
        let result = SASetControls { type_, bool_ctrls_high, bool_ctrls_low };
        Ok((result, remaining))
    }
}
impl Serialize for SASetControls {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let bool_ctrls_high_bytes = u8::from(self.bool_ctrls_high).serialize();
        let bool_ctrls_low_bytes = u8::from(self.bool_ctrls_low).serialize();
        [
            type_bytes[0],
            0,
            0,
            0,
            bool_ctrls_high_bytes[0],
            bool_ctrls_low_bytes[0],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        u8::from(self.bool_ctrls_high).serialize_into(bytes);
        u8::from(self.bool_ctrls_low).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

pub type SALockControls = SASetControls;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionMessageFlag(u8);
impl ActionMessageFlag {
    pub const ON_PRESS: Self = Self(1 << 0);
    pub const ON_RELEASE: Self = Self(1 << 1);
    pub const GEN_KEY_EVENT: Self = Self(1 << 2);
}
impl From<ActionMessageFlag> for u8 {
    #[inline]
    fn from(input: ActionMessageFlag) -> Self {
        input.0
    }
}
impl From<ActionMessageFlag> for Option<u8> {
    #[inline]
    fn from(input: ActionMessageFlag) -> Self {
        Some(input.0)
    }
}
impl From<ActionMessageFlag> for u16 {
    #[inline]
    fn from(input: ActionMessageFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<ActionMessageFlag> for Option<u16> {
    #[inline]
    fn from(input: ActionMessageFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ActionMessageFlag> for u32 {
    #[inline]
    fn from(input: ActionMessageFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<ActionMessageFlag> for Option<u32> {
    #[inline]
    fn from(input: ActionMessageFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ActionMessageFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ActionMessageFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ON_PRESS.0.into(), "ON_PRESS", "OnPress"),
            (Self::ON_RELEASE.0.into(), "ON_RELEASE", "OnRelease"),
            (Self::GEN_KEY_EVENT.0.into(), "GEN_KEY_EVENT", "GenKeyEvent"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(ActionMessageFlag, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAActionMessage {
    pub type_: SAType,
    pub flags: ActionMessageFlag,
    pub message: [u8; 6],
}
impl_debug_if_no_extra_traits!(SAActionMessage, "SAActionMessage");
impl TryParse for SAActionMessage {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (message, remaining) = crate::x11_utils::parse_u8_array::<6>(remaining)?;
        let type_ = type_.into();
        let flags = flags.into();
        let result = SAActionMessage { type_, flags, message };
        Ok((result, remaining))
    }
}
impl Serialize for SAActionMessage {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            self.message[0],
            self.message[1],
            self.message[2],
            self.message[3],
            self.message[4],
            self.message[5],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        bytes.extend_from_slice(&self.message);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SARedirectKey {
    pub type_: SAType,
    pub newkey: xproto::Keycode,
    pub mask: xproto::ModMask,
    pub real_modifiers: xproto::ModMask,
    pub vmods_mask_high: VModsHigh,
    pub vmods_mask_low: VModsLow,
    pub vmods_high: VModsHigh,
    pub vmods_low: VModsLow,
}
impl_debug_if_no_extra_traits!(SARedirectKey, "SARedirectKey");
impl TryParse for SARedirectKey {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (newkey, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (mask, remaining) = u8::try_parse(remaining)?;
        let (real_modifiers, remaining) = u8::try_parse(remaining)?;
        let (vmods_mask_high, remaining) = u8::try_parse(remaining)?;
        let (vmods_mask_low, remaining) = u8::try_parse(remaining)?;
        let (vmods_high, remaining) = u8::try_parse(remaining)?;
        let (vmods_low, remaining) = u8::try_parse(remaining)?;
        let type_ = type_.into();
        let mask = mask.into();
        let real_modifiers = real_modifiers.into();
        let vmods_mask_high = vmods_mask_high.into();
        let vmods_mask_low = vmods_mask_low.into();
        let vmods_high = vmods_high.into();
        let vmods_low = vmods_low.into();
        let result = SARedirectKey { type_, newkey, mask, real_modifiers, vmods_mask_high, vmods_mask_low, vmods_high, vmods_low };
        Ok((result, remaining))
    }
}
impl Serialize for SARedirectKey {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let newkey_bytes = self.newkey.serialize();
        let mask_bytes = (u16::from(self.mask) as u8).serialize();
        let real_modifiers_bytes = (u16::from(self.real_modifiers) as u8).serialize();
        let vmods_mask_high_bytes = u8::from(self.vmods_mask_high).serialize();
        let vmods_mask_low_bytes = u8::from(self.vmods_mask_low).serialize();
        let vmods_high_bytes = u8::from(self.vmods_high).serialize();
        let vmods_low_bytes = u8::from(self.vmods_low).serialize();
        [
            type_bytes[0],
            newkey_bytes[0],
            mask_bytes[0],
            real_modifiers_bytes[0],
            vmods_mask_high_bytes[0],
            vmods_mask_low_bytes[0],
            vmods_high_bytes[0],
            vmods_low_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        self.newkey.serialize_into(bytes);
        (u16::from(self.mask) as u8).serialize_into(bytes);
        (u16::from(self.real_modifiers) as u8).serialize_into(bytes);
        u8::from(self.vmods_mask_high).serialize_into(bytes);
        u8::from(self.vmods_mask_low).serialize_into(bytes);
        u8::from(self.vmods_high).serialize_into(bytes);
        u8::from(self.vmods_low).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SADeviceBtn {
    pub type_: SAType,
    pub flags: u8,
    pub count: u8,
    pub button: u8,
    pub device: u8,
}
impl_debug_if_no_extra_traits!(SADeviceBtn, "SADeviceBtn");
impl TryParse for SADeviceBtn {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (count, remaining) = u8::try_parse(remaining)?;
        let (button, remaining) = u8::try_parse(remaining)?;
        let (device, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let result = SADeviceBtn { type_, flags, count, button, device };
        Ok((result, remaining))
    }
}
impl Serialize for SADeviceBtn {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = self.flags.serialize();
        let count_bytes = self.count.serialize();
        let button_bytes = self.button.serialize();
        let device_bytes = self.device.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            count_bytes[0],
            button_bytes[0],
            device_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        self.flags.serialize_into(bytes);
        self.count.serialize_into(bytes);
        self.button.serialize_into(bytes);
        self.device.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LockDeviceFlags(u8);
impl LockDeviceFlags {
    pub const NO_LOCK: Self = Self(1 << 0);
    pub const NO_UNLOCK: Self = Self(1 << 1);
}
impl From<LockDeviceFlags> for u8 {
    #[inline]
    fn from(input: LockDeviceFlags) -> Self {
        input.0
    }
}
impl From<LockDeviceFlags> for Option<u8> {
    #[inline]
    fn from(input: LockDeviceFlags) -> Self {
        Some(input.0)
    }
}
impl From<LockDeviceFlags> for u16 {
    #[inline]
    fn from(input: LockDeviceFlags) -> Self {
        u16::from(input.0)
    }
}
impl From<LockDeviceFlags> for Option<u16> {
    #[inline]
    fn from(input: LockDeviceFlags) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<LockDeviceFlags> for u32 {
    #[inline]
    fn from(input: LockDeviceFlags) -> Self {
        u32::from(input.0)
    }
}
impl From<LockDeviceFlags> for Option<u32> {
    #[inline]
    fn from(input: LockDeviceFlags) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for LockDeviceFlags {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for LockDeviceFlags  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NO_LOCK.0.into(), "NO_LOCK", "NoLock"),
            (Self::NO_UNLOCK.0.into(), "NO_UNLOCK", "NoUnlock"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(LockDeviceFlags, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SALockDeviceBtn {
    pub type_: SAType,
    pub flags: LockDeviceFlags,
    pub button: u8,
    pub device: u8,
}
impl_debug_if_no_extra_traits!(SALockDeviceBtn, "SALockDeviceBtn");
impl TryParse for SALockDeviceBtn {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (button, remaining) = u8::try_parse(remaining)?;
        let (device, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let flags = flags.into();
        let result = SALockDeviceBtn { type_, flags, button, device };
        Ok((result, remaining))
    }
}
impl Serialize for SALockDeviceBtn {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let flags_bytes = u8::from(self.flags).serialize();
        let button_bytes = self.button.serialize();
        let device_bytes = self.device.serialize();
        [
            type_bytes[0],
            flags_bytes[0],
            0,
            button_bytes[0],
            device_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.button.serialize_into(bytes);
        self.device.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SAValWhat(u8);
impl SAValWhat {
    pub const IGNORE_VAL: Self = Self(0);
    pub const SET_VAL_MIN: Self = Self(1);
    pub const SET_VAL_CENTER: Self = Self(2);
    pub const SET_VAL_MAX: Self = Self(3);
    pub const SET_VAL_RELATIVE: Self = Self(4);
    pub const SET_VAL_ABSOLUTE: Self = Self(5);
}
impl From<SAValWhat> for u8 {
    #[inline]
    fn from(input: SAValWhat) -> Self {
        input.0
    }
}
impl From<SAValWhat> for Option<u8> {
    #[inline]
    fn from(input: SAValWhat) -> Self {
        Some(input.0)
    }
}
impl From<SAValWhat> for u16 {
    #[inline]
    fn from(input: SAValWhat) -> Self {
        u16::from(input.0)
    }
}
impl From<SAValWhat> for Option<u16> {
    #[inline]
    fn from(input: SAValWhat) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SAValWhat> for u32 {
    #[inline]
    fn from(input: SAValWhat) -> Self {
        u32::from(input.0)
    }
}
impl From<SAValWhat> for Option<u32> {
    #[inline]
    fn from(input: SAValWhat) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SAValWhat {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SAValWhat  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::IGNORE_VAL.0.into(), "IGNORE_VAL", "IgnoreVal"),
            (Self::SET_VAL_MIN.0.into(), "SET_VAL_MIN", "SetValMin"),
            (Self::SET_VAL_CENTER.0.into(), "SET_VAL_CENTER", "SetValCenter"),
            (Self::SET_VAL_MAX.0.into(), "SET_VAL_MAX", "SetValMax"),
            (Self::SET_VAL_RELATIVE.0.into(), "SET_VAL_RELATIVE", "SetValRelative"),
            (Self::SET_VAL_ABSOLUTE.0.into(), "SET_VAL_ABSOLUTE", "SetValAbsolute"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SADeviceValuator {
    pub type_: SAType,
    pub device: u8,
    pub val1what: SAValWhat,
    pub val1index: u8,
    pub val1value: u8,
    pub val2what: SAValWhat,
    pub val2index: u8,
    pub val2value: u8,
}
impl_debug_if_no_extra_traits!(SADeviceValuator, "SADeviceValuator");
impl TryParse for SADeviceValuator {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (device, remaining) = u8::try_parse(remaining)?;
        let (val1what, remaining) = u8::try_parse(remaining)?;
        let (val1index, remaining) = u8::try_parse(remaining)?;
        let (val1value, remaining) = u8::try_parse(remaining)?;
        let (val2what, remaining) = u8::try_parse(remaining)?;
        let (val2index, remaining) = u8::try_parse(remaining)?;
        let (val2value, remaining) = u8::try_parse(remaining)?;
        let type_ = type_.into();
        let val1what = val1what.into();
        let val2what = val2what.into();
        let result = SADeviceValuator { type_, device, val1what, val1index, val1value, val2what, val2index, val2value };
        Ok((result, remaining))
    }
}
impl Serialize for SADeviceValuator {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        let device_bytes = self.device.serialize();
        let val1what_bytes = u8::from(self.val1what).serialize();
        let val1index_bytes = self.val1index.serialize();
        let val1value_bytes = self.val1value.serialize();
        let val2what_bytes = u8::from(self.val2what).serialize();
        let val2index_bytes = self.val2index.serialize();
        let val2value_bytes = self.val2value.serialize();
        [
            type_bytes[0],
            device_bytes[0],
            val1what_bytes[0],
            val1index_bytes[0],
            val1value_bytes[0],
            val2what_bytes[0],
            val2index_bytes[0],
            val2value_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        self.device.serialize_into(bytes);
        u8::from(self.val1what).serialize_into(bytes);
        self.val1index.serialize_into(bytes);
        self.val1value.serialize_into(bytes);
        u8::from(self.val2what).serialize_into(bytes);
        self.val2index.serialize_into(bytes);
        self.val2value.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SIAction {
    pub type_: SAType,
    pub data: [u8; 7],
}
impl_debug_if_no_extra_traits!(SIAction, "SIAction");
impl TryParse for SIAction {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (data, remaining) = crate::x11_utils::parse_u8_array::<7>(remaining)?;
        let type_ = type_.into();
        let result = SIAction { type_, data };
        Ok((result, remaining))
    }
}
impl Serialize for SIAction {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let type_bytes = u8::from(self.type_).serialize();
        [
            type_bytes[0],
            self.data[0],
            self.data[1],
            self.data[2],
            self.data[3],
            self.data[4],
            self.data[5],
            self.data[6],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u8::from(self.type_).serialize_into(bytes);
        bytes.extend_from_slice(&self.data);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SymInterpret {
    pub sym: xproto::Keysym,
    pub mods: xproto::ModMask,
    pub match_: u8,
    pub virtual_mod: VModsLow,
    pub flags: u8,
    pub action: SIAction,
}
impl_debug_if_no_extra_traits!(SymInterpret, "SymInterpret");
impl TryParse for SymInterpret {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (sym, remaining) = xproto::Keysym::try_parse(remaining)?;
        let (mods, remaining) = u8::try_parse(remaining)?;
        let (match_, remaining) = u8::try_parse(remaining)?;
        let (virtual_mod, remaining) = u8::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let (action, remaining) = SIAction::try_parse(remaining)?;
        let mods = mods.into();
        let virtual_mod = virtual_mod.into();
        let result = SymInterpret { sym, mods, match_, virtual_mod, flags, action };
        Ok((result, remaining))
    }
}
impl Serialize for SymInterpret {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let sym_bytes = self.sym.serialize();
        let mods_bytes = (u16::from(self.mods) as u8).serialize();
        let match_bytes = self.match_.serialize();
        let virtual_mod_bytes = u8::from(self.virtual_mod).serialize();
        let flags_bytes = self.flags.serialize();
        let action_bytes = self.action.serialize();
        [
            sym_bytes[0],
            sym_bytes[1],
            sym_bytes[2],
            sym_bytes[3],
            mods_bytes[0],
            match_bytes[0],
            virtual_mod_bytes[0],
            flags_bytes[0],
            action_bytes[0],
            action_bytes[1],
            action_bytes[2],
            action_bytes[3],
            action_bytes[4],
            action_bytes[5],
            action_bytes[6],
            action_bytes[7],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.sym.serialize_into(bytes);
        (u16::from(self.mods) as u8).serialize_into(bytes);
        self.match_.serialize_into(bytes);
        u8::from(self.virtual_mod).serialize_into(bytes);
        self.flags.serialize_into(bytes);
        self.action.serialize_into(bytes);
    }
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Action([u8; 8]);
impl Action {
    pub fn as_noaction(&self) -> SANoAction {
        fn do_the_parse(remaining: &[u8]) -> Result<SANoAction, ParseError> {
            let (noaction, remaining) = SANoAction::try_parse(remaining)?;
            let _ = remaining;
            Ok(noaction)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_setmods(&self) -> SASetMods {
        fn do_the_parse(remaining: &[u8]) -> Result<SASetMods, ParseError> {
            let (setmods, remaining) = SASetMods::try_parse(remaining)?;
            let _ = remaining;
            Ok(setmods)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_latchmods(&self) -> SALatchMods {
        fn do_the_parse(remaining: &[u8]) -> Result<SALatchMods, ParseError> {
            let (latchmods, remaining) = SALatchMods::try_parse(remaining)?;
            let _ = remaining;
            Ok(latchmods)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lockmods(&self) -> SALockMods {
        fn do_the_parse(remaining: &[u8]) -> Result<SALockMods, ParseError> {
            let (lockmods, remaining) = SALockMods::try_parse(remaining)?;
            let _ = remaining;
            Ok(lockmods)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_setgroup(&self) -> SASetGroup {
        fn do_the_parse(remaining: &[u8]) -> Result<SASetGroup, ParseError> {
            let (setgroup, remaining) = SASetGroup::try_parse(remaining)?;
            let _ = remaining;
            Ok(setgroup)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_latchgroup(&self) -> SALatchGroup {
        fn do_the_parse(remaining: &[u8]) -> Result<SALatchGroup, ParseError> {
            let (latchgroup, remaining) = SALatchGroup::try_parse(remaining)?;
            let _ = remaining;
            Ok(latchgroup)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lockgroup(&self) -> SALockGroup {
        fn do_the_parse(remaining: &[u8]) -> Result<SALockGroup, ParseError> {
            let (lockgroup, remaining) = SALockGroup::try_parse(remaining)?;
            let _ = remaining;
            Ok(lockgroup)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_moveptr(&self) -> SAMovePtr {
        fn do_the_parse(remaining: &[u8]) -> Result<SAMovePtr, ParseError> {
            let (moveptr, remaining) = SAMovePtr::try_parse(remaining)?;
            let _ = remaining;
            Ok(moveptr)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_ptrbtn(&self) -> SAPtrBtn {
        fn do_the_parse(remaining: &[u8]) -> Result<SAPtrBtn, ParseError> {
            let (ptrbtn, remaining) = SAPtrBtn::try_parse(remaining)?;
            let _ = remaining;
            Ok(ptrbtn)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lockptrbtn(&self) -> SALockPtrBtn {
        fn do_the_parse(remaining: &[u8]) -> Result<SALockPtrBtn, ParseError> {
            let (lockptrbtn, remaining) = SALockPtrBtn::try_parse(remaining)?;
            let _ = remaining;
            Ok(lockptrbtn)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_setptrdflt(&self) -> SASetPtrDflt {
        fn do_the_parse(remaining: &[u8]) -> Result<SASetPtrDflt, ParseError> {
            let (setptrdflt, remaining) = SASetPtrDflt::try_parse(remaining)?;
            let _ = remaining;
            Ok(setptrdflt)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_isolock(&self) -> SAIsoLock {
        fn do_the_parse(remaining: &[u8]) -> Result<SAIsoLock, ParseError> {
            let (isolock, remaining) = SAIsoLock::try_parse(remaining)?;
            let _ = remaining;
            Ok(isolock)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_terminate(&self) -> SATerminate {
        fn do_the_parse(remaining: &[u8]) -> Result<SATerminate, ParseError> {
            let (terminate, remaining) = SATerminate::try_parse(remaining)?;
            let _ = remaining;
            Ok(terminate)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_switchscreen(&self) -> SASwitchScreen {
        fn do_the_parse(remaining: &[u8]) -> Result<SASwitchScreen, ParseError> {
            let (switchscreen, remaining) = SASwitchScreen::try_parse(remaining)?;
            let _ = remaining;
            Ok(switchscreen)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_setcontrols(&self) -> SASetControls {
        fn do_the_parse(remaining: &[u8]) -> Result<SASetControls, ParseError> {
            let (setcontrols, remaining) = SASetControls::try_parse(remaining)?;
            let _ = remaining;
            Ok(setcontrols)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lockcontrols(&self) -> SALockControls {
        fn do_the_parse(remaining: &[u8]) -> Result<SALockControls, ParseError> {
            let (lockcontrols, remaining) = SALockControls::try_parse(remaining)?;
            let _ = remaining;
            Ok(lockcontrols)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_message(&self) -> SAActionMessage {
        fn do_the_parse(remaining: &[u8]) -> Result<SAActionMessage, ParseError> {
            let (message, remaining) = SAActionMessage::try_parse(remaining)?;
            let _ = remaining;
            Ok(message)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_redirect(&self) -> SARedirectKey {
        fn do_the_parse(remaining: &[u8]) -> Result<SARedirectKey, ParseError> {
            let (redirect, remaining) = SARedirectKey::try_parse(remaining)?;
            let _ = remaining;
            Ok(redirect)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_devbtn(&self) -> SADeviceBtn {
        fn do_the_parse(remaining: &[u8]) -> Result<SADeviceBtn, ParseError> {
            let (devbtn, remaining) = SADeviceBtn::try_parse(remaining)?;
            let _ = remaining;
            Ok(devbtn)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lockdevbtn(&self) -> SALockDeviceBtn {
        fn do_the_parse(remaining: &[u8]) -> Result<SALockDeviceBtn, ParseError> {
            let (lockdevbtn, remaining) = SALockDeviceBtn::try_parse(remaining)?;
            let _ = remaining;
            Ok(lockdevbtn)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_devval(&self) -> SADeviceValuator {
        fn do_the_parse(remaining: &[u8]) -> Result<SADeviceValuator, ParseError> {
            let (devval, remaining) = SADeviceValuator::try_parse(remaining)?;
            let _ = remaining;
            Ok(devval)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_type(&self) -> SAType {
        fn do_the_parse(remaining: &[u8]) -> Result<SAType, ParseError> {
            let (type_, remaining) = u8::try_parse(remaining)?;
            let type_ = type_.into();
            let _ = remaining;
            Ok(type_)
        }
        do_the_parse(&self.0).unwrap()
    }
}
impl Serialize for Action {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        self.0
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }
}
impl TryParse for Action {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let inner: [u8; 8] = value.get(..8)
            .ok_or(ParseError::InsufficientData)?
            .try_into()
            .unwrap();
        let result = Action(inner);
        Ok((result, &value[8..]))
    }
}
impl From<SANoAction> for Action {
    fn from(noaction: SANoAction) -> Self {
        let noaction_bytes = noaction.serialize();
        Self(noaction_bytes)
    }
}
impl From<SASetMods> for Action {
    fn from(setmods: SASetMods) -> Self {
        let setmods_bytes = setmods.serialize();
        Self(setmods_bytes)
    }
}
impl From<SASetGroup> for Action {
    fn from(setgroup: SASetGroup) -> Self {
        let setgroup_bytes = setgroup.serialize();
        Self(setgroup_bytes)
    }
}
impl From<SAMovePtr> for Action {
    fn from(moveptr: SAMovePtr) -> Self {
        let moveptr_bytes = moveptr.serialize();
        Self(moveptr_bytes)
    }
}
impl From<SAPtrBtn> for Action {
    fn from(ptrbtn: SAPtrBtn) -> Self {
        let ptrbtn_bytes = ptrbtn.serialize();
        Self(ptrbtn_bytes)
    }
}
impl From<SALockPtrBtn> for Action {
    fn from(lockptrbtn: SALockPtrBtn) -> Self {
        let lockptrbtn_bytes = lockptrbtn.serialize();
        Self(lockptrbtn_bytes)
    }
}
impl From<SASetPtrDflt> for Action {
    fn from(setptrdflt: SASetPtrDflt) -> Self {
        let setptrdflt_bytes = setptrdflt.serialize();
        Self(setptrdflt_bytes)
    }
}
impl From<SAIsoLock> for Action {
    fn from(isolock: SAIsoLock) -> Self {
        let isolock_bytes = isolock.serialize();
        Self(isolock_bytes)
    }
}
impl From<SATerminate> for Action {
    fn from(terminate: SATerminate) -> Self {
        let terminate_bytes = terminate.serialize();
        Self(terminate_bytes)
    }
}
impl From<SASwitchScreen> for Action {
    fn from(switchscreen: SASwitchScreen) -> Self {
        let switchscreen_bytes = switchscreen.serialize();
        Self(switchscreen_bytes)
    }
}
impl From<SASetControls> for Action {
    fn from(setcontrols: SASetControls) -> Self {
        let setcontrols_bytes = setcontrols.serialize();
        Self(setcontrols_bytes)
    }
}
impl From<SAActionMessage> for Action {
    fn from(message: SAActionMessage) -> Self {
        let message_bytes = message.serialize();
        Self(message_bytes)
    }
}
impl From<SARedirectKey> for Action {
    fn from(redirect: SARedirectKey) -> Self {
        let redirect_bytes = redirect.serialize();
        Self(redirect_bytes)
    }
}
impl From<SADeviceBtn> for Action {
    fn from(devbtn: SADeviceBtn) -> Self {
        let devbtn_bytes = devbtn.serialize();
        Self(devbtn_bytes)
    }
}
impl From<SALockDeviceBtn> for Action {
    fn from(lockdevbtn: SALockDeviceBtn) -> Self {
        let lockdevbtn_bytes = lockdevbtn.serialize();
        Self(lockdevbtn_bytes)
    }
}
impl From<SADeviceValuator> for Action {
    fn from(devval: SADeviceValuator) -> Self {
        let devval_bytes = devval.serialize();
        Self(devval_bytes)
    }
}
impl From<SAType> for Action {
    fn from(type_: SAType) -> Self {
        let type_bytes = u8::from(type_).serialize();
        let value = [
            type_bytes[0],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        Self(value)
    }
}

/// Opcode for the UseExtension request
pub const USE_EXTENSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UseExtensionRequest {
    pub wanted_major: u16,
    pub wanted_minor: u16,
}
impl_debug_if_no_extra_traits!(UseExtensionRequest, "UseExtensionRequest");
impl UseExtensionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let wanted_major_bytes = self.wanted_major.serialize();
        let wanted_minor_bytes = self.wanted_minor.serialize();
        let mut request0 = vec![
            major_opcode,
            USE_EXTENSION_REQUEST,
            0,
            0,
            wanted_major_bytes[0],
            wanted_major_bytes[1],
            wanted_minor_bytes[0],
            wanted_minor_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != USE_EXTENSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (wanted_major, remaining) = u16::try_parse(value)?;
        let (wanted_minor, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(UseExtensionRequest {
            wanted_major,
            wanted_minor,
        })
    }
}
impl Request for UseExtensionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for UseExtensionRequest {
    type Reply = UseExtensionReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UseExtensionReply {
    pub supported: bool,
    pub sequence: u16,
    pub length: u32,
    pub server_major: u16,
    pub server_minor: u16,
}
impl_debug_if_no_extra_traits!(UseExtensionReply, "UseExtensionReply");
impl TryParse for UseExtensionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (supported, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (server_major, remaining) = u16::try_parse(remaining)?;
        let (server_minor, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = UseExtensionReply { supported, sequence, length, server_major, server_minor };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for UseExtensionReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let supported_bytes = self.supported.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let server_major_bytes = self.server_major.serialize();
        let server_minor_bytes = self.server_minor.serialize();
        [
            response_type_bytes[0],
            supported_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            server_major_bytes[0],
            server_major_bytes[1],
            server_minor_bytes[0],
            server_minor_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.supported.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.server_major.serialize_into(bytes);
        self.server_minor.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxNewKeyboardNotify {
    pub affect_new_keyboard: NKNDetail,
    pub new_keyboard_details: NKNDetail,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxNewKeyboardNotify, "SelectEventsAuxNewKeyboardNotify");
impl TryParse for SelectEventsAuxNewKeyboardNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_new_keyboard, remaining) = u16::try_parse(remaining)?;
        let (new_keyboard_details, remaining) = u16::try_parse(remaining)?;
        let affect_new_keyboard = affect_new_keyboard.into();
        let new_keyboard_details = new_keyboard_details.into();
        let result = SelectEventsAuxNewKeyboardNotify { affect_new_keyboard, new_keyboard_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxNewKeyboardNotify {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let affect_new_keyboard_bytes = u16::from(self.affect_new_keyboard).serialize();
        let new_keyboard_details_bytes = u16::from(self.new_keyboard_details).serialize();
        [
            affect_new_keyboard_bytes[0],
            affect_new_keyboard_bytes[1],
            new_keyboard_details_bytes[0],
            new_keyboard_details_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        u16::from(self.affect_new_keyboard).serialize_into(bytes);
        u16::from(self.new_keyboard_details).serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxStateNotify {
    pub affect_state: StatePart,
    pub state_details: StatePart,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxStateNotify, "SelectEventsAuxStateNotify");
impl TryParse for SelectEventsAuxStateNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_state, remaining) = u16::try_parse(remaining)?;
        let (state_details, remaining) = u16::try_parse(remaining)?;
        let affect_state = affect_state.into();
        let state_details = state_details.into();
        let result = SelectEventsAuxStateNotify { affect_state, state_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxStateNotify {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let affect_state_bytes = u16::from(self.affect_state).serialize();
        let state_details_bytes = u16::from(self.state_details).serialize();
        [
            affect_state_bytes[0],
            affect_state_bytes[1],
            state_details_bytes[0],
            state_details_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        u16::from(self.affect_state).serialize_into(bytes);
        u16::from(self.state_details).serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxControlsNotify {
    pub affect_ctrls: Control,
    pub ctrl_details: Control,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxControlsNotify, "SelectEventsAuxControlsNotify");
impl TryParse for SelectEventsAuxControlsNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_ctrls, remaining) = u32::try_parse(remaining)?;
        let (ctrl_details, remaining) = u32::try_parse(remaining)?;
        let affect_ctrls = affect_ctrls.into();
        let ctrl_details = ctrl_details.into();
        let result = SelectEventsAuxControlsNotify { affect_ctrls, ctrl_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxControlsNotify {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let affect_ctrls_bytes = u32::from(self.affect_ctrls).serialize();
        let ctrl_details_bytes = u32::from(self.ctrl_details).serialize();
        [
            affect_ctrls_bytes[0],
            affect_ctrls_bytes[1],
            affect_ctrls_bytes[2],
            affect_ctrls_bytes[3],
            ctrl_details_bytes[0],
            ctrl_details_bytes[1],
            ctrl_details_bytes[2],
            ctrl_details_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u32::from(self.affect_ctrls).serialize_into(bytes);
        u32::from(self.ctrl_details).serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxIndicatorStateNotify {
    pub affect_indicator_state: u32,
    pub indicator_state_details: u32,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxIndicatorStateNotify, "SelectEventsAuxIndicatorStateNotify");
impl TryParse for SelectEventsAuxIndicatorStateNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_indicator_state, remaining) = u32::try_parse(remaining)?;
        let (indicator_state_details, remaining) = u32::try_parse(remaining)?;
        let result = SelectEventsAuxIndicatorStateNotify { affect_indicator_state, indicator_state_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxIndicatorStateNotify {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let affect_indicator_state_bytes = self.affect_indicator_state.serialize();
        let indicator_state_details_bytes = self.indicator_state_details.serialize();
        [
            affect_indicator_state_bytes[0],
            affect_indicator_state_bytes[1],
            affect_indicator_state_bytes[2],
            affect_indicator_state_bytes[3],
            indicator_state_details_bytes[0],
            indicator_state_details_bytes[1],
            indicator_state_details_bytes[2],
            indicator_state_details_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.affect_indicator_state.serialize_into(bytes);
        self.indicator_state_details.serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxIndicatorMapNotify {
    pub affect_indicator_map: u32,
    pub indicator_map_details: u32,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxIndicatorMapNotify, "SelectEventsAuxIndicatorMapNotify");
impl TryParse for SelectEventsAuxIndicatorMapNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_indicator_map, remaining) = u32::try_parse(remaining)?;
        let (indicator_map_details, remaining) = u32::try_parse(remaining)?;
        let result = SelectEventsAuxIndicatorMapNotify { affect_indicator_map, indicator_map_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxIndicatorMapNotify {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let affect_indicator_map_bytes = self.affect_indicator_map.serialize();
        let indicator_map_details_bytes = self.indicator_map_details.serialize();
        [
            affect_indicator_map_bytes[0],
            affect_indicator_map_bytes[1],
            affect_indicator_map_bytes[2],
            affect_indicator_map_bytes[3],
            indicator_map_details_bytes[0],
            indicator_map_details_bytes[1],
            indicator_map_details_bytes[2],
            indicator_map_details_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.affect_indicator_map.serialize_into(bytes);
        self.indicator_map_details.serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxNamesNotify {
    pub affect_names: NameDetail,
    pub names_details: NameDetail,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxNamesNotify, "SelectEventsAuxNamesNotify");
impl TryParse for SelectEventsAuxNamesNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_names, remaining) = u16::try_parse(remaining)?;
        let (names_details, remaining) = u16::try_parse(remaining)?;
        let affect_names = affect_names.into();
        let names_details = names_details.into();
        let result = SelectEventsAuxNamesNotify { affect_names, names_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxNamesNotify {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let affect_names_bytes = (u32::from(self.affect_names) as u16).serialize();
        let names_details_bytes = (u32::from(self.names_details) as u16).serialize();
        [
            affect_names_bytes[0],
            affect_names_bytes[1],
            names_details_bytes[0],
            names_details_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        (u32::from(self.affect_names) as u16).serialize_into(bytes);
        (u32::from(self.names_details) as u16).serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxCompatMapNotify {
    pub affect_compat: CMDetail,
    pub compat_details: CMDetail,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxCompatMapNotify, "SelectEventsAuxCompatMapNotify");
impl TryParse for SelectEventsAuxCompatMapNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_compat, remaining) = u8::try_parse(remaining)?;
        let (compat_details, remaining) = u8::try_parse(remaining)?;
        let affect_compat = affect_compat.into();
        let compat_details = compat_details.into();
        let result = SelectEventsAuxCompatMapNotify { affect_compat, compat_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxCompatMapNotify {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let affect_compat_bytes = u8::from(self.affect_compat).serialize();
        let compat_details_bytes = u8::from(self.compat_details).serialize();
        [
            affect_compat_bytes[0],
            compat_details_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        u8::from(self.affect_compat).serialize_into(bytes);
        u8::from(self.compat_details).serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxBellNotify {
    pub affect_bell: u8,
    pub bell_details: u8,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxBellNotify, "SelectEventsAuxBellNotify");
impl TryParse for SelectEventsAuxBellNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_bell, remaining) = u8::try_parse(remaining)?;
        let (bell_details, remaining) = u8::try_parse(remaining)?;
        let result = SelectEventsAuxBellNotify { affect_bell, bell_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxBellNotify {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let affect_bell_bytes = self.affect_bell.serialize();
        let bell_details_bytes = self.bell_details.serialize();
        [
            affect_bell_bytes[0],
            bell_details_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.affect_bell.serialize_into(bytes);
        self.bell_details.serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxActionMessage {
    pub affect_msg_details: u8,
    pub msg_details: u8,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxActionMessage, "SelectEventsAuxActionMessage");
impl TryParse for SelectEventsAuxActionMessage {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_msg_details, remaining) = u8::try_parse(remaining)?;
        let (msg_details, remaining) = u8::try_parse(remaining)?;
        let result = SelectEventsAuxActionMessage { affect_msg_details, msg_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxActionMessage {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let affect_msg_details_bytes = self.affect_msg_details.serialize();
        let msg_details_bytes = self.msg_details.serialize();
        [
            affect_msg_details_bytes[0],
            msg_details_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.affect_msg_details.serialize_into(bytes);
        self.msg_details.serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxAccessXNotify {
    pub affect_access_x: AXNDetail,
    pub access_x_details: AXNDetail,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxAccessXNotify, "SelectEventsAuxAccessXNotify");
impl TryParse for SelectEventsAuxAccessXNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_access_x, remaining) = u16::try_parse(remaining)?;
        let (access_x_details, remaining) = u16::try_parse(remaining)?;
        let affect_access_x = affect_access_x.into();
        let access_x_details = access_x_details.into();
        let result = SelectEventsAuxAccessXNotify { affect_access_x, access_x_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxAccessXNotify {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let affect_access_x_bytes = u16::from(self.affect_access_x).serialize();
        let access_x_details_bytes = u16::from(self.access_x_details).serialize();
        [
            affect_access_x_bytes[0],
            affect_access_x_bytes[1],
            access_x_details_bytes[0],
            access_x_details_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        u16::from(self.affect_access_x).serialize_into(bytes);
        u16::from(self.access_x_details).serialize_into(bytes);
    }
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAuxExtensionDeviceNotify {
    pub affect_ext_dev: XIFeature,
    pub extdev_details: XIFeature,
}
impl_debug_if_no_extra_traits!(SelectEventsAuxExtensionDeviceNotify, "SelectEventsAuxExtensionDeviceNotify");
impl TryParse for SelectEventsAuxExtensionDeviceNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (affect_ext_dev, remaining) = u16::try_parse(remaining)?;
        let (extdev_details, remaining) = u16::try_parse(remaining)?;
        let affect_ext_dev = affect_ext_dev.into();
        let extdev_details = extdev_details.into();
        let result = SelectEventsAuxExtensionDeviceNotify { affect_ext_dev, extdev_details };
        Ok((result, remaining))
    }
}
impl Serialize for SelectEventsAuxExtensionDeviceNotify {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let affect_ext_dev_bytes = u16::from(self.affect_ext_dev).serialize();
        let extdev_details_bytes = u16::from(self.extdev_details).serialize();
        [
            affect_ext_dev_bytes[0],
            affect_ext_dev_bytes[1],
            extdev_details_bytes[0],
            extdev_details_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        u16::from(self.affect_ext_dev).serialize_into(bytes);
        u16::from(self.extdev_details).serialize_into(bytes);
    }
}
/// Auxiliary and optional information for the `select_events` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsAux {
    pub new_keyboard_notify: Option<SelectEventsAuxNewKeyboardNotify>,
    pub state_notify: Option<SelectEventsAuxStateNotify>,
    pub controls_notify: Option<SelectEventsAuxControlsNotify>,
    pub indicator_state_notify: Option<SelectEventsAuxIndicatorStateNotify>,
    pub indicator_map_notify: Option<SelectEventsAuxIndicatorMapNotify>,
    pub names_notify: Option<SelectEventsAuxNamesNotify>,
    pub compat_map_notify: Option<SelectEventsAuxCompatMapNotify>,
    pub bell_notify: Option<SelectEventsAuxBellNotify>,
    pub action_message: Option<SelectEventsAuxActionMessage>,
    pub access_x_notify: Option<SelectEventsAuxAccessXNotify>,
    pub extension_device_notify: Option<SelectEventsAuxExtensionDeviceNotify>,
}
impl_debug_if_no_extra_traits!(SelectEventsAux, "SelectEventsAux");
impl SelectEventsAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], affect_which: u16, clear: u16, select_all: u16) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u16::from(affect_which) & ((!u16::from(clear)) & (!u16::from(select_all)));
        let mut outer_remaining = value;
        let new_keyboard_notify = if switch_expr & u16::from(EventType::NEW_KEYBOARD_NOTIFY) != 0 {
            let (new_keyboard_notify, new_remaining) = SelectEventsAuxNewKeyboardNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(new_keyboard_notify)
        } else {
            None
        };
        let state_notify = if switch_expr & u16::from(EventType::STATE_NOTIFY) != 0 {
            let (state_notify, new_remaining) = SelectEventsAuxStateNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(state_notify)
        } else {
            None
        };
        let controls_notify = if switch_expr & u16::from(EventType::CONTROLS_NOTIFY) != 0 {
            let (controls_notify, new_remaining) = SelectEventsAuxControlsNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(controls_notify)
        } else {
            None
        };
        let indicator_state_notify = if switch_expr & u16::from(EventType::INDICATOR_STATE_NOTIFY) != 0 {
            let (indicator_state_notify, new_remaining) = SelectEventsAuxIndicatorStateNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(indicator_state_notify)
        } else {
            None
        };
        let indicator_map_notify = if switch_expr & u16::from(EventType::INDICATOR_MAP_NOTIFY) != 0 {
            let (indicator_map_notify, new_remaining) = SelectEventsAuxIndicatorMapNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(indicator_map_notify)
        } else {
            None
        };
        let names_notify = if switch_expr & u16::from(EventType::NAMES_NOTIFY) != 0 {
            let (names_notify, new_remaining) = SelectEventsAuxNamesNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(names_notify)
        } else {
            None
        };
        let compat_map_notify = if switch_expr & u16::from(EventType::COMPAT_MAP_NOTIFY) != 0 {
            let (compat_map_notify, new_remaining) = SelectEventsAuxCompatMapNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(compat_map_notify)
        } else {
            None
        };
        let bell_notify = if switch_expr & u16::from(EventType::BELL_NOTIFY) != 0 {
            let (bell_notify, new_remaining) = SelectEventsAuxBellNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(bell_notify)
        } else {
            None
        };
        let action_message = if switch_expr & u16::from(EventType::ACTION_MESSAGE) != 0 {
            let (action_message, new_remaining) = SelectEventsAuxActionMessage::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(action_message)
        } else {
            None
        };
        let access_x_notify = if switch_expr & u16::from(EventType::ACCESS_X_NOTIFY) != 0 {
            let (access_x_notify, new_remaining) = SelectEventsAuxAccessXNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(access_x_notify)
        } else {
            None
        };
        let extension_device_notify = if switch_expr & u16::from(EventType::EXTENSION_DEVICE_NOTIFY) != 0 {
            let (extension_device_notify, new_remaining) = SelectEventsAuxExtensionDeviceNotify::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(extension_device_notify)
        } else {
            None
        };
        let result = SelectEventsAux { new_keyboard_notify, state_notify, controls_notify, indicator_state_notify, indicator_map_notify, names_notify, compat_map_notify, bell_notify, action_message, access_x_notify, extension_device_notify };
        Ok((result, outer_remaining))
    }
}
impl SelectEventsAux {
    #[allow(dead_code)]
    fn serialize(&self, affect_which: u16, clear: u16, select_all: u16) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u16::from(affect_which), u16::from(clear), u16::from(select_all));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, affect_which: u16, clear: u16, select_all: u16) {
        assert_eq!(self.switch_expr(), u16::from(affect_which) & ((!u16::from(clear)) & (!u16::from(select_all))), "switch `details` has an inconsistent discriminant");
        if let Some(ref new_keyboard_notify) = self.new_keyboard_notify {
            new_keyboard_notify.serialize_into(bytes);
        }
        if let Some(ref state_notify) = self.state_notify {
            state_notify.serialize_into(bytes);
        }
        if let Some(ref controls_notify) = self.controls_notify {
            controls_notify.serialize_into(bytes);
        }
        if let Some(ref indicator_state_notify) = self.indicator_state_notify {
            indicator_state_notify.serialize_into(bytes);
        }
        if let Some(ref indicator_map_notify) = self.indicator_map_notify {
            indicator_map_notify.serialize_into(bytes);
        }
        if let Some(ref names_notify) = self.names_notify {
            names_notify.serialize_into(bytes);
        }
        if let Some(ref compat_map_notify) = self.compat_map_notify {
            compat_map_notify.serialize_into(bytes);
        }
        if let Some(ref bell_notify) = self.bell_notify {
            bell_notify.serialize_into(bytes);
        }
        if let Some(ref action_message) = self.action_message {
            action_message.serialize_into(bytes);
        }
        if let Some(ref access_x_notify) = self.access_x_notify {
            access_x_notify.serialize_into(bytes);
        }
        if let Some(ref extension_device_notify) = self.extension_device_notify {
            extension_device_notify.serialize_into(bytes);
        }
    }
}
impl SelectEventsAux {
    fn switch_expr(&self) -> u16 {
        let mut expr_value = 0;
        if self.new_keyboard_notify.is_some() {
            expr_value |= u16::from(EventType::NEW_KEYBOARD_NOTIFY);
        }
        if self.state_notify.is_some() {
            expr_value |= u16::from(EventType::STATE_NOTIFY);
        }
        if self.controls_notify.is_some() {
            expr_value |= u16::from(EventType::CONTROLS_NOTIFY);
        }
        if self.indicator_state_notify.is_some() {
            expr_value |= u16::from(EventType::INDICATOR_STATE_NOTIFY);
        }
        if self.indicator_map_notify.is_some() {
            expr_value |= u16::from(EventType::INDICATOR_MAP_NOTIFY);
        }
        if self.names_notify.is_some() {
            expr_value |= u16::from(EventType::NAMES_NOTIFY);
        }
        if self.compat_map_notify.is_some() {
            expr_value |= u16::from(EventType::COMPAT_MAP_NOTIFY);
        }
        if self.bell_notify.is_some() {
            expr_value |= u16::from(EventType::BELL_NOTIFY);
        }
        if self.action_message.is_some() {
            expr_value |= u16::from(EventType::ACTION_MESSAGE);
        }
        if self.access_x_notify.is_some() {
            expr_value |= u16::from(EventType::ACCESS_X_NOTIFY);
        }
        if self.extension_device_notify.is_some() {
            expr_value |= u16::from(EventType::EXTENSION_DEVICE_NOTIFY);
        }
        expr_value
    }
}
impl SelectEventsAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `new_keyboard_notify` field of this structure.
    #[must_use]
    pub fn new_keyboard_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxNewKeyboardNotify>> {
        self.new_keyboard_notify = value.into();
        self
    }
    /// Set the `state_notify` field of this structure.
    #[must_use]
    pub fn state_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxStateNotify>> {
        self.state_notify = value.into();
        self
    }
    /// Set the `controls_notify` field of this structure.
    #[must_use]
    pub fn controls_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxControlsNotify>> {
        self.controls_notify = value.into();
        self
    }
    /// Set the `indicator_state_notify` field of this structure.
    #[must_use]
    pub fn indicator_state_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxIndicatorStateNotify>> {
        self.indicator_state_notify = value.into();
        self
    }
    /// Set the `indicator_map_notify` field of this structure.
    #[must_use]
    pub fn indicator_map_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxIndicatorMapNotify>> {
        self.indicator_map_notify = value.into();
        self
    }
    /// Set the `names_notify` field of this structure.
    #[must_use]
    pub fn names_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxNamesNotify>> {
        self.names_notify = value.into();
        self
    }
    /// Set the `compat_map_notify` field of this structure.
    #[must_use]
    pub fn compat_map_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxCompatMapNotify>> {
        self.compat_map_notify = value.into();
        self
    }
    /// Set the `bell_notify` field of this structure.
    #[must_use]
    pub fn bell_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxBellNotify>> {
        self.bell_notify = value.into();
        self
    }
    /// Set the `action_message` field of this structure.
    #[must_use]
    pub fn action_message<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxActionMessage>> {
        self.action_message = value.into();
        self
    }
    /// Set the `access_x_notify` field of this structure.
    #[must_use]
    pub fn access_x_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxAccessXNotify>> {
        self.access_x_notify = value.into();
        self
    }
    /// Set the `extension_device_notify` field of this structure.
    #[must_use]
    pub fn extension_device_notify<I>(mut self, value: I) -> Self where I: Into<Option<SelectEventsAuxExtensionDeviceNotify>> {
        self.extension_device_notify = value.into();
        self
    }
}

/// Opcode for the SelectEvents request
pub const SELECT_EVENTS_REQUEST: u8 = 1;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectEventsRequest<'input> {
    pub device_spec: DeviceSpec,
    pub clear: EventType,
    pub select_all: EventType,
    pub affect_map: MapPart,
    pub map: MapPart,
    pub details: Cow<'input, SelectEventsAux>,
}
impl_debug_if_no_extra_traits!(SelectEventsRequest<'_>, "SelectEventsRequest");
impl<'input> SelectEventsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let affect_which: u16 = self.details.switch_expr() | (u16::from(self.clear) | u16::from(self.select_all));
        let affect_which_bytes = affect_which.serialize();
        let clear_bytes = u16::from(self.clear).serialize();
        let select_all_bytes = u16::from(self.select_all).serialize();
        let affect_map_bytes = u16::from(self.affect_map).serialize();
        let map_bytes = u16::from(self.map).serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_EVENTS_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            affect_which_bytes[0],
            affect_which_bytes[1],
            clear_bytes[0],
            clear_bytes[1],
            select_all_bytes[0],
            select_all_bytes[1],
            affect_map_bytes[0],
            affect_map_bytes[1],
            map_bytes[0],
            map_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let details_bytes = self.details.serialize(u16::from(affect_which), u16::from(self.clear), u16::from(self.select_all));
        let length_so_far = length_so_far + details_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), details_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SELECT_EVENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (affect_which, remaining) = u16::try_parse(remaining)?;
        let (clear, remaining) = u16::try_parse(remaining)?;
        let clear = clear.into();
        let (select_all, remaining) = u16::try_parse(remaining)?;
        let select_all = select_all.into();
        let (affect_map, remaining) = u16::try_parse(remaining)?;
        let affect_map = affect_map.into();
        let (map, remaining) = u16::try_parse(remaining)?;
        let map = map.into();
        let (details, remaining) = SelectEventsAux::try_parse(remaining, u16::from(affect_which), u16::from(clear), u16::from(select_all))?;
        let _ = remaining;
        Ok(SelectEventsRequest {
            device_spec,
            clear,
            select_all,
            affect_map,
            map,
            details: Cow::Owned(details),
        })
    }
    /// Clone all borrowed data in this SelectEventsRequest.
    pub fn into_owned(self) -> SelectEventsRequest<'static> {
        SelectEventsRequest {
            device_spec: self.device_spec,
            clear: self.clear,
            select_all: self.select_all,
            affect_map: self.affect_map,
            map: self.map,
            details: Cow::Owned(self.details.into_owned()),
        }
    }
}
impl<'input> Request for SelectEventsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SelectEventsRequest<'input> {
}

/// Opcode for the Bell request
pub const BELL_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BellRequest {
    pub device_spec: DeviceSpec,
    pub bell_class: BellClassSpec,
    pub bell_id: IDSpec,
    pub percent: i8,
    pub force_sound: bool,
    pub event_only: bool,
    pub pitch: i16,
    pub duration: i16,
    pub name: xproto::Atom,
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(BellRequest, "BellRequest");
impl BellRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let bell_class_bytes = self.bell_class.serialize();
        let bell_id_bytes = self.bell_id.serialize();
        let percent_bytes = self.percent.serialize();
        let force_sound_bytes = self.force_sound.serialize();
        let event_only_bytes = self.event_only.serialize();
        let pitch_bytes = self.pitch.serialize();
        let duration_bytes = self.duration.serialize();
        let name_bytes = self.name.serialize();
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            BELL_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            bell_class_bytes[0],
            bell_class_bytes[1],
            bell_id_bytes[0],
            bell_id_bytes[1],
            percent_bytes[0],
            force_sound_bytes[0],
            event_only_bytes[0],
            0,
            pitch_bytes[0],
            pitch_bytes[1],
            duration_bytes[0],
            duration_bytes[1],
            0,
            0,
            name_bytes[0],
            name_bytes[1],
            name_bytes[2],
            name_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != BELL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (bell_class, remaining) = BellClassSpec::try_parse(remaining)?;
        let (bell_id, remaining) = IDSpec::try_parse(remaining)?;
        let (percent, remaining) = i8::try_parse(remaining)?;
        let (force_sound, remaining) = bool::try_parse(remaining)?;
        let (event_only, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (pitch, remaining) = i16::try_parse(remaining)?;
        let (duration, remaining) = i16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let _ = remaining;
        Ok(BellRequest {
            device_spec,
            bell_class,
            bell_id,
            percent,
            force_sound,
            event_only,
            pitch,
            duration,
            name,
            window,
        })
    }
}
impl Request for BellRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for BellRequest {
}

/// Opcode for the GetState request
pub const GET_STATE_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetStateRequest {
    pub device_spec: DeviceSpec,
}
impl_debug_if_no_extra_traits!(GetStateRequest, "GetStateRequest");
impl GetStateRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_STATE_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_STATE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetStateRequest {
            device_spec,
        })
    }
}
impl Request for GetStateRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetStateRequest {
    type Reply = GetStateReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetStateReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub mods: xproto::ModMask,
    pub base_mods: xproto::ModMask,
    pub latched_mods: xproto::ModMask,
    pub locked_mods: xproto::ModMask,
    pub group: Group,
    pub locked_group: Group,
    pub base_group: i16,
    pub latched_group: i16,
    pub compat_state: xproto::ModMask,
    pub grab_mods: xproto::ModMask,
    pub compat_grab_mods: xproto::ModMask,
    pub lookup_mods: xproto::ModMask,
    pub compat_lookup_mods: xproto::ModMask,
    pub ptr_btn_state: xproto::KeyButMask,
}
impl_debug_if_no_extra_traits!(GetStateReply, "GetStateReply");
impl TryParse for GetStateReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (mods, remaining) = u8::try_parse(remaining)?;
        let (base_mods, remaining) = u8::try_parse(remaining)?;
        let (latched_mods, remaining) = u8::try_parse(remaining)?;
        let (locked_mods, remaining) = u8::try_parse(remaining)?;
        let (group, remaining) = u8::try_parse(remaining)?;
        let (locked_group, remaining) = u8::try_parse(remaining)?;
        let (base_group, remaining) = i16::try_parse(remaining)?;
        let (latched_group, remaining) = i16::try_parse(remaining)?;
        let (compat_state, remaining) = u8::try_parse(remaining)?;
        let (grab_mods, remaining) = u8::try_parse(remaining)?;
        let (compat_grab_mods, remaining) = u8::try_parse(remaining)?;
        let (lookup_mods, remaining) = u8::try_parse(remaining)?;
        let (compat_lookup_mods, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (ptr_btn_state, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(6..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let mods = mods.into();
        let base_mods = base_mods.into();
        let latched_mods = latched_mods.into();
        let locked_mods = locked_mods.into();
        let group = group.into();
        let locked_group = locked_group.into();
        let compat_state = compat_state.into();
        let grab_mods = grab_mods.into();
        let compat_grab_mods = compat_grab_mods.into();
        let lookup_mods = lookup_mods.into();
        let compat_lookup_mods = compat_lookup_mods.into();
        let ptr_btn_state = ptr_btn_state.into();
        let result = GetStateReply { device_id, sequence, length, mods, base_mods, latched_mods, locked_mods, group, locked_group, base_group, latched_group, compat_state, grab_mods, compat_grab_mods, lookup_mods, compat_lookup_mods, ptr_btn_state };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetStateReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let device_id_bytes = self.device_id.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let mods_bytes = (u16::from(self.mods) as u8).serialize();
        let base_mods_bytes = (u16::from(self.base_mods) as u8).serialize();
        let latched_mods_bytes = (u16::from(self.latched_mods) as u8).serialize();
        let locked_mods_bytes = (u16::from(self.locked_mods) as u8).serialize();
        let group_bytes = u8::from(self.group).serialize();
        let locked_group_bytes = u8::from(self.locked_group).serialize();
        let base_group_bytes = self.base_group.serialize();
        let latched_group_bytes = self.latched_group.serialize();
        let compat_state_bytes = (u16::from(self.compat_state) as u8).serialize();
        let grab_mods_bytes = (u16::from(self.grab_mods) as u8).serialize();
        let compat_grab_mods_bytes = (u16::from(self.compat_grab_mods) as u8).serialize();
        let lookup_mods_bytes = (u16::from(self.lookup_mods) as u8).serialize();
        let compat_lookup_mods_bytes = (u16::from(self.compat_lookup_mods) as u8).serialize();
        let ptr_btn_state_bytes = u16::from(self.ptr_btn_state).serialize();
        [
            response_type_bytes[0],
            device_id_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            mods_bytes[0],
            base_mods_bytes[0],
            latched_mods_bytes[0],
            locked_mods_bytes[0],
            group_bytes[0],
            locked_group_bytes[0],
            base_group_bytes[0],
            base_group_bytes[1],
            latched_group_bytes[0],
            latched_group_bytes[1],
            compat_state_bytes[0],
            grab_mods_bytes[0],
            compat_grab_mods_bytes[0],
            lookup_mods_bytes[0],
            compat_lookup_mods_bytes[0],
            0,
            ptr_btn_state_bytes[0],
            ptr_btn_state_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        (u16::from(self.mods) as u8).serialize_into(bytes);
        (u16::from(self.base_mods) as u8).serialize_into(bytes);
        (u16::from(self.latched_mods) as u8).serialize_into(bytes);
        (u16::from(self.locked_mods) as u8).serialize_into(bytes);
        u8::from(self.group).serialize_into(bytes);
        u8::from(self.locked_group).serialize_into(bytes);
        self.base_group.serialize_into(bytes);
        self.latched_group.serialize_into(bytes);
        (u16::from(self.compat_state) as u8).serialize_into(bytes);
        (u16::from(self.grab_mods) as u8).serialize_into(bytes);
        (u16::from(self.compat_grab_mods) as u8).serialize_into(bytes);
        (u16::from(self.lookup_mods) as u8).serialize_into(bytes);
        (u16::from(self.compat_lookup_mods) as u8).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        u16::from(self.ptr_btn_state).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 6]);
    }
}

/// Opcode for the LatchLockState request
pub const LATCH_LOCK_STATE_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LatchLockStateRequest {
    pub device_spec: DeviceSpec,
    pub affect_mod_locks: xproto::ModMask,
    pub mod_locks: xproto::ModMask,
    pub lock_group: bool,
    pub group_lock: Group,
    pub affect_mod_latches: xproto::ModMask,
    pub latch_group: bool,
    pub group_latch: u16,
}
impl_debug_if_no_extra_traits!(LatchLockStateRequest, "LatchLockStateRequest");
impl LatchLockStateRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let affect_mod_locks_bytes = (u16::from(self.affect_mod_locks) as u8).serialize();
        let mod_locks_bytes = (u16::from(self.mod_locks) as u8).serialize();
        let lock_group_bytes = self.lock_group.serialize();
        let group_lock_bytes = u8::from(self.group_lock).serialize();
        let affect_mod_latches_bytes = (u16::from(self.affect_mod_latches) as u8).serialize();
        let latch_group_bytes = self.latch_group.serialize();
        let group_latch_bytes = self.group_latch.serialize();
        let mut request0 = vec![
            major_opcode,
            LATCH_LOCK_STATE_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            affect_mod_locks_bytes[0],
            mod_locks_bytes[0],
            lock_group_bytes[0],
            group_lock_bytes[0],
            affect_mod_latches_bytes[0],
            0,
            0,
            latch_group_bytes[0],
            group_latch_bytes[0],
            group_latch_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != LATCH_LOCK_STATE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (affect_mod_locks, remaining) = u8::try_parse(remaining)?;
        let affect_mod_locks = affect_mod_locks.into();
        let (mod_locks, remaining) = u8::try_parse(remaining)?;
        let mod_locks = mod_locks.into();
        let (lock_group, remaining) = bool::try_parse(remaining)?;
        let (group_lock, remaining) = u8::try_parse(remaining)?;
        let group_lock = group_lock.into();
        let (affect_mod_latches, remaining) = u8::try_parse(remaining)?;
        let affect_mod_latches = affect_mod_latches.into();
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (latch_group, remaining) = bool::try_parse(remaining)?;
        let (group_latch, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(LatchLockStateRequest {
            device_spec,
            affect_mod_locks,
            mod_locks,
            lock_group,
            group_lock,
            affect_mod_latches,
            latch_group,
            group_latch,
        })
    }
}
impl Request for LatchLockStateRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for LatchLockStateRequest {
}

/// Opcode for the GetControls request
pub const GET_CONTROLS_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetControlsRequest {
    pub device_spec: DeviceSpec,
}
impl_debug_if_no_extra_traits!(GetControlsRequest, "GetControlsRequest");
impl GetControlsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CONTROLS_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_CONTROLS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetControlsRequest {
            device_spec,
        })
    }
}
impl Request for GetControlsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetControlsRequest {
    type Reply = GetControlsReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetControlsReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub mouse_keys_dflt_btn: u8,
    pub num_groups: u8,
    pub groups_wrap: u8,
    pub internal_mods_mask: xproto::ModMask,
    pub ignore_lock_mods_mask: xproto::ModMask,
    pub internal_mods_real_mods: xproto::ModMask,
    pub ignore_lock_mods_real_mods: xproto::ModMask,
    pub internal_mods_vmods: VMod,
    pub ignore_lock_mods_vmods: VMod,
    pub repeat_delay: u16,
    pub repeat_interval: u16,
    pub slow_keys_delay: u16,
    pub debounce_delay: u16,
    pub mouse_keys_delay: u16,
    pub mouse_keys_interval: u16,
    pub mouse_keys_time_to_max: u16,
    pub mouse_keys_max_speed: u16,
    pub mouse_keys_curve: i16,
    pub access_x_option: AXOption,
    pub access_x_timeout: u16,
    pub access_x_timeout_options_mask: AXOption,
    pub access_x_timeout_options_values: AXOption,
    pub access_x_timeout_mask: BoolCtrl,
    pub access_x_timeout_values: BoolCtrl,
    pub enabled_controls: BoolCtrl,
    pub per_key_repeat: [u8; 32],
}
impl_debug_if_no_extra_traits!(GetControlsReply, "GetControlsReply");
impl TryParse for GetControlsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (mouse_keys_dflt_btn, remaining) = u8::try_parse(remaining)?;
        let (num_groups, remaining) = u8::try_parse(remaining)?;
        let (groups_wrap, remaining) = u8::try_parse(remaining)?;
        let (internal_mods_mask, remaining) = u8::try_parse(remaining)?;
        let (ignore_lock_mods_mask, remaining) = u8::try_parse(remaining)?;
        let (internal_mods_real_mods, remaining) = u8::try_parse(remaining)?;
        let (ignore_lock_mods_real_mods, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (internal_mods_vmods, remaining) = u16::try_parse(remaining)?;
        let (ignore_lock_mods_vmods, remaining) = u16::try_parse(remaining)?;
        let (repeat_delay, remaining) = u16::try_parse(remaining)?;
        let (repeat_interval, remaining) = u16::try_parse(remaining)?;
        let (slow_keys_delay, remaining) = u16::try_parse(remaining)?;
        let (debounce_delay, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_delay, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_interval, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_time_to_max, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_max_speed, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_curve, remaining) = i16::try_parse(remaining)?;
        let (access_x_option, remaining) = u16::try_parse(remaining)?;
        let (access_x_timeout, remaining) = u16::try_parse(remaining)?;
        let (access_x_timeout_options_mask, remaining) = u16::try_parse(remaining)?;
        let (access_x_timeout_options_values, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (access_x_timeout_mask, remaining) = u32::try_parse(remaining)?;
        let (access_x_timeout_values, remaining) = u32::try_parse(remaining)?;
        let (enabled_controls, remaining) = u32::try_parse(remaining)?;
        let (per_key_repeat, remaining) = crate::x11_utils::parse_u8_array::<32>(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let internal_mods_mask = internal_mods_mask.into();
        let ignore_lock_mods_mask = ignore_lock_mods_mask.into();
        let internal_mods_real_mods = internal_mods_real_mods.into();
        let ignore_lock_mods_real_mods = ignore_lock_mods_real_mods.into();
        let internal_mods_vmods = internal_mods_vmods.into();
        let ignore_lock_mods_vmods = ignore_lock_mods_vmods.into();
        let access_x_option = access_x_option.into();
        let access_x_timeout_options_mask = access_x_timeout_options_mask.into();
        let access_x_timeout_options_values = access_x_timeout_options_values.into();
        let access_x_timeout_mask = access_x_timeout_mask.into();
        let access_x_timeout_values = access_x_timeout_values.into();
        let enabled_controls = enabled_controls.into();
        let result = GetControlsReply { device_id, sequence, length, mouse_keys_dflt_btn, num_groups, groups_wrap, internal_mods_mask, ignore_lock_mods_mask, internal_mods_real_mods, ignore_lock_mods_real_mods, internal_mods_vmods, ignore_lock_mods_vmods, repeat_delay, repeat_interval, slow_keys_delay, debounce_delay, mouse_keys_delay, mouse_keys_interval, mouse_keys_time_to_max, mouse_keys_max_speed, mouse_keys_curve, access_x_option, access_x_timeout, access_x_timeout_options_mask, access_x_timeout_options_values, access_x_timeout_mask, access_x_timeout_values, enabled_controls, per_key_repeat };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetControlsReply {
    type Bytes = [u8; 92];
    fn serialize(&self) -> [u8; 92] {
        let response_type_bytes = &[1];
        let device_id_bytes = self.device_id.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let mouse_keys_dflt_btn_bytes = self.mouse_keys_dflt_btn.serialize();
        let num_groups_bytes = self.num_groups.serialize();
        let groups_wrap_bytes = self.groups_wrap.serialize();
        let internal_mods_mask_bytes = (u16::from(self.internal_mods_mask) as u8).serialize();
        let ignore_lock_mods_mask_bytes = (u16::from(self.ignore_lock_mods_mask) as u8).serialize();
        let internal_mods_real_mods_bytes = (u16::from(self.internal_mods_real_mods) as u8).serialize();
        let ignore_lock_mods_real_mods_bytes = (u16::from(self.ignore_lock_mods_real_mods) as u8).serialize();
        let internal_mods_vmods_bytes = u16::from(self.internal_mods_vmods).serialize();
        let ignore_lock_mods_vmods_bytes = u16::from(self.ignore_lock_mods_vmods).serialize();
        let repeat_delay_bytes = self.repeat_delay.serialize();
        let repeat_interval_bytes = self.repeat_interval.serialize();
        let slow_keys_delay_bytes = self.slow_keys_delay.serialize();
        let debounce_delay_bytes = self.debounce_delay.serialize();
        let mouse_keys_delay_bytes = self.mouse_keys_delay.serialize();
        let mouse_keys_interval_bytes = self.mouse_keys_interval.serialize();
        let mouse_keys_time_to_max_bytes = self.mouse_keys_time_to_max.serialize();
        let mouse_keys_max_speed_bytes = self.mouse_keys_max_speed.serialize();
        let mouse_keys_curve_bytes = self.mouse_keys_curve.serialize();
        let access_x_option_bytes = u16::from(self.access_x_option).serialize();
        let access_x_timeout_bytes = self.access_x_timeout.serialize();
        let access_x_timeout_options_mask_bytes = u16::from(self.access_x_timeout_options_mask).serialize();
        let access_x_timeout_options_values_bytes = u16::from(self.access_x_timeout_options_values).serialize();
        let access_x_timeout_mask_bytes = u32::from(self.access_x_timeout_mask).serialize();
        let access_x_timeout_values_bytes = u32::from(self.access_x_timeout_values).serialize();
        let enabled_controls_bytes = u32::from(self.enabled_controls).serialize();
        [
            response_type_bytes[0],
            device_id_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            mouse_keys_dflt_btn_bytes[0],
            num_groups_bytes[0],
            groups_wrap_bytes[0],
            internal_mods_mask_bytes[0],
            ignore_lock_mods_mask_bytes[0],
            internal_mods_real_mods_bytes[0],
            ignore_lock_mods_real_mods_bytes[0],
            0,
            internal_mods_vmods_bytes[0],
            internal_mods_vmods_bytes[1],
            ignore_lock_mods_vmods_bytes[0],
            ignore_lock_mods_vmods_bytes[1],
            repeat_delay_bytes[0],
            repeat_delay_bytes[1],
            repeat_interval_bytes[0],
            repeat_interval_bytes[1],
            slow_keys_delay_bytes[0],
            slow_keys_delay_bytes[1],
            debounce_delay_bytes[0],
            debounce_delay_bytes[1],
            mouse_keys_delay_bytes[0],
            mouse_keys_delay_bytes[1],
            mouse_keys_interval_bytes[0],
            mouse_keys_interval_bytes[1],
            mouse_keys_time_to_max_bytes[0],
            mouse_keys_time_to_max_bytes[1],
            mouse_keys_max_speed_bytes[0],
            mouse_keys_max_speed_bytes[1],
            mouse_keys_curve_bytes[0],
            mouse_keys_curve_bytes[1],
            access_x_option_bytes[0],
            access_x_option_bytes[1],
            access_x_timeout_bytes[0],
            access_x_timeout_bytes[1],
            access_x_timeout_options_mask_bytes[0],
            access_x_timeout_options_mask_bytes[1],
            access_x_timeout_options_values_bytes[0],
            access_x_timeout_options_values_bytes[1],
            0,
            0,
            access_x_timeout_mask_bytes[0],
            access_x_timeout_mask_bytes[1],
            access_x_timeout_mask_bytes[2],
            access_x_timeout_mask_bytes[3],
            access_x_timeout_values_bytes[0],
            access_x_timeout_values_bytes[1],
            access_x_timeout_values_bytes[2],
            access_x_timeout_values_bytes[3],
            enabled_controls_bytes[0],
            enabled_controls_bytes[1],
            enabled_controls_bytes[2],
            enabled_controls_bytes[3],
            self.per_key_repeat[0],
            self.per_key_repeat[1],
            self.per_key_repeat[2],
            self.per_key_repeat[3],
            self.per_key_repeat[4],
            self.per_key_repeat[5],
            self.per_key_repeat[6],
            self.per_key_repeat[7],
            self.per_key_repeat[8],
            self.per_key_repeat[9],
            self.per_key_repeat[10],
            self.per_key_repeat[11],
            self.per_key_repeat[12],
            self.per_key_repeat[13],
            self.per_key_repeat[14],
            self.per_key_repeat[15],
            self.per_key_repeat[16],
            self.per_key_repeat[17],
            self.per_key_repeat[18],
            self.per_key_repeat[19],
            self.per_key_repeat[20],
            self.per_key_repeat[21],
            self.per_key_repeat[22],
            self.per_key_repeat[23],
            self.per_key_repeat[24],
            self.per_key_repeat[25],
            self.per_key_repeat[26],
            self.per_key_repeat[27],
            self.per_key_repeat[28],
            self.per_key_repeat[29],
            self.per_key_repeat[30],
            self.per_key_repeat[31],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(92);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.mouse_keys_dflt_btn.serialize_into(bytes);
        self.num_groups.serialize_into(bytes);
        self.groups_wrap.serialize_into(bytes);
        (u16::from(self.internal_mods_mask) as u8).serialize_into(bytes);
        (u16::from(self.ignore_lock_mods_mask) as u8).serialize_into(bytes);
        (u16::from(self.internal_mods_real_mods) as u8).serialize_into(bytes);
        (u16::from(self.ignore_lock_mods_real_mods) as u8).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        u16::from(self.internal_mods_vmods).serialize_into(bytes);
        u16::from(self.ignore_lock_mods_vmods).serialize_into(bytes);
        self.repeat_delay.serialize_into(bytes);
        self.repeat_interval.serialize_into(bytes);
        self.slow_keys_delay.serialize_into(bytes);
        self.debounce_delay.serialize_into(bytes);
        self.mouse_keys_delay.serialize_into(bytes);
        self.mouse_keys_interval.serialize_into(bytes);
        self.mouse_keys_time_to_max.serialize_into(bytes);
        self.mouse_keys_max_speed.serialize_into(bytes);
        self.mouse_keys_curve.serialize_into(bytes);
        u16::from(self.access_x_option).serialize_into(bytes);
        self.access_x_timeout.serialize_into(bytes);
        u16::from(self.access_x_timeout_options_mask).serialize_into(bytes);
        u16::from(self.access_x_timeout_options_values).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        u32::from(self.access_x_timeout_mask).serialize_into(bytes);
        u32::from(self.access_x_timeout_values).serialize_into(bytes);
        u32::from(self.enabled_controls).serialize_into(bytes);
        bytes.extend_from_slice(&self.per_key_repeat);
    }
}

/// Opcode for the SetControls request
pub const SET_CONTROLS_REQUEST: u8 = 7;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetControlsRequest<'input> {
    pub device_spec: DeviceSpec,
    pub affect_internal_real_mods: xproto::ModMask,
    pub internal_real_mods: xproto::ModMask,
    pub affect_ignore_lock_real_mods: xproto::ModMask,
    pub ignore_lock_real_mods: xproto::ModMask,
    pub affect_internal_virtual_mods: VMod,
    pub internal_virtual_mods: VMod,
    pub affect_ignore_lock_virtual_mods: VMod,
    pub ignore_lock_virtual_mods: VMod,
    pub mouse_keys_dflt_btn: u8,
    pub groups_wrap: u8,
    pub access_x_options: AXOption,
    pub affect_enabled_controls: BoolCtrl,
    pub enabled_controls: BoolCtrl,
    pub change_controls: Control,
    pub repeat_delay: u16,
    pub repeat_interval: u16,
    pub slow_keys_delay: u16,
    pub debounce_delay: u16,
    pub mouse_keys_delay: u16,
    pub mouse_keys_interval: u16,
    pub mouse_keys_time_to_max: u16,
    pub mouse_keys_max_speed: u16,
    pub mouse_keys_curve: i16,
    pub access_x_timeout: u16,
    pub access_x_timeout_mask: BoolCtrl,
    pub access_x_timeout_values: BoolCtrl,
    pub access_x_timeout_options_mask: AXOption,
    pub access_x_timeout_options_values: AXOption,
    pub per_key_repeat: Cow<'input, [u8; 32]>,
}
impl_debug_if_no_extra_traits!(SetControlsRequest<'_>, "SetControlsRequest");
impl<'input> SetControlsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 2]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let affect_internal_real_mods_bytes = (u16::from(self.affect_internal_real_mods) as u8).serialize();
        let internal_real_mods_bytes = (u16::from(self.internal_real_mods) as u8).serialize();
        let affect_ignore_lock_real_mods_bytes = (u16::from(self.affect_ignore_lock_real_mods) as u8).serialize();
        let ignore_lock_real_mods_bytes = (u16::from(self.ignore_lock_real_mods) as u8).serialize();
        let affect_internal_virtual_mods_bytes = u16::from(self.affect_internal_virtual_mods).serialize();
        let internal_virtual_mods_bytes = u16::from(self.internal_virtual_mods).serialize();
        let affect_ignore_lock_virtual_mods_bytes = u16::from(self.affect_ignore_lock_virtual_mods).serialize();
        let ignore_lock_virtual_mods_bytes = u16::from(self.ignore_lock_virtual_mods).serialize();
        let mouse_keys_dflt_btn_bytes = self.mouse_keys_dflt_btn.serialize();
        let groups_wrap_bytes = self.groups_wrap.serialize();
        let access_x_options_bytes = u16::from(self.access_x_options).serialize();
        let affect_enabled_controls_bytes = u32::from(self.affect_enabled_controls).serialize();
        let enabled_controls_bytes = u32::from(self.enabled_controls).serialize();
        let change_controls_bytes = u32::from(self.change_controls).serialize();
        let repeat_delay_bytes = self.repeat_delay.serialize();
        let repeat_interval_bytes = self.repeat_interval.serialize();
        let slow_keys_delay_bytes = self.slow_keys_delay.serialize();
        let debounce_delay_bytes = self.debounce_delay.serialize();
        let mouse_keys_delay_bytes = self.mouse_keys_delay.serialize();
        let mouse_keys_interval_bytes = self.mouse_keys_interval.serialize();
        let mouse_keys_time_to_max_bytes = self.mouse_keys_time_to_max.serialize();
        let mouse_keys_max_speed_bytes = self.mouse_keys_max_speed.serialize();
        let mouse_keys_curve_bytes = self.mouse_keys_curve.serialize();
        let access_x_timeout_bytes = self.access_x_timeout.serialize();
        let access_x_timeout_mask_bytes = u32::from(self.access_x_timeout_mask).serialize();
        let access_x_timeout_values_bytes = u32::from(self.access_x_timeout_values).serialize();
        let access_x_timeout_options_mask_bytes = u16::from(self.access_x_timeout_options_mask).serialize();
        let access_x_timeout_options_values_bytes = u16::from(self.access_x_timeout_options_values).serialize();
        let mut request0 = vec![
            major_opcode,
            SET_CONTROLS_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            affect_internal_real_mods_bytes[0],
            internal_real_mods_bytes[0],
            affect_ignore_lock_real_mods_bytes[0],
            ignore_lock_real_mods_bytes[0],
            affect_internal_virtual_mods_bytes[0],
            affect_internal_virtual_mods_bytes[1],
            internal_virtual_mods_bytes[0],
            internal_virtual_mods_bytes[1],
            affect_ignore_lock_virtual_mods_bytes[0],
            affect_ignore_lock_virtual_mods_bytes[1],
            ignore_lock_virtual_mods_bytes[0],
            ignore_lock_virtual_mods_bytes[1],
            mouse_keys_dflt_btn_bytes[0],
            groups_wrap_bytes[0],
            access_x_options_bytes[0],
            access_x_options_bytes[1],
            0,
            0,
            affect_enabled_controls_bytes[0],
            affect_enabled_controls_bytes[1],
            affect_enabled_controls_bytes[2],
            affect_enabled_controls_bytes[3],
            enabled_controls_bytes[0],
            enabled_controls_bytes[1],
            enabled_controls_bytes[2],
            enabled_controls_bytes[3],
            change_controls_bytes[0],
            change_controls_bytes[1],
            change_controls_bytes[2],
            change_controls_bytes[3],
            repeat_delay_bytes[0],
            repeat_delay_bytes[1],
            repeat_interval_bytes[0],
            repeat_interval_bytes[1],
            slow_keys_delay_bytes[0],
            slow_keys_delay_bytes[1],
            debounce_delay_bytes[0],
            debounce_delay_bytes[1],
            mouse_keys_delay_bytes[0],
            mouse_keys_delay_bytes[1],
            mouse_keys_interval_bytes[0],
            mouse_keys_interval_bytes[1],
            mouse_keys_time_to_max_bytes[0],
            mouse_keys_time_to_max_bytes[1],
            mouse_keys_max_speed_bytes[0],
            mouse_keys_max_speed_bytes[1],
            mouse_keys_curve_bytes[0],
            mouse_keys_curve_bytes[1],
            access_x_timeout_bytes[0],
            access_x_timeout_bytes[1],
            access_x_timeout_mask_bytes[0],
            access_x_timeout_mask_bytes[1],
            access_x_timeout_mask_bytes[2],
            access_x_timeout_mask_bytes[3],
            access_x_timeout_values_bytes[0],
            access_x_timeout_values_bytes[1],
            access_x_timeout_values_bytes[2],
            access_x_timeout_values_bytes[3],
            access_x_timeout_options_mask_bytes[0],
            access_x_timeout_options_mask_bytes[1],
            access_x_timeout_options_values_bytes[0],
            access_x_timeout_options_values_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.per_key_repeat.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), Cow::Owned(self.per_key_repeat.to_vec())], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_CONTROLS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (affect_internal_real_mods, remaining) = u8::try_parse(remaining)?;
        let affect_internal_real_mods = affect_internal_real_mods.into();
        let (internal_real_mods, remaining) = u8::try_parse(remaining)?;
        let internal_real_mods = internal_real_mods.into();
        let (affect_ignore_lock_real_mods, remaining) = u8::try_parse(remaining)?;
        let affect_ignore_lock_real_mods = affect_ignore_lock_real_mods.into();
        let (ignore_lock_real_mods, remaining) = u8::try_parse(remaining)?;
        let ignore_lock_real_mods = ignore_lock_real_mods.into();
        let (affect_internal_virtual_mods, remaining) = u16::try_parse(remaining)?;
        let affect_internal_virtual_mods = affect_internal_virtual_mods.into();
        let (internal_virtual_mods, remaining) = u16::try_parse(remaining)?;
        let internal_virtual_mods = internal_virtual_mods.into();
        let (affect_ignore_lock_virtual_mods, remaining) = u16::try_parse(remaining)?;
        let affect_ignore_lock_virtual_mods = affect_ignore_lock_virtual_mods.into();
        let (ignore_lock_virtual_mods, remaining) = u16::try_parse(remaining)?;
        let ignore_lock_virtual_mods = ignore_lock_virtual_mods.into();
        let (mouse_keys_dflt_btn, remaining) = u8::try_parse(remaining)?;
        let (groups_wrap, remaining) = u8::try_parse(remaining)?;
        let (access_x_options, remaining) = u16::try_parse(remaining)?;
        let access_x_options = access_x_options.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (affect_enabled_controls, remaining) = u32::try_parse(remaining)?;
        let affect_enabled_controls = affect_enabled_controls.into();
        let (enabled_controls, remaining) = u32::try_parse(remaining)?;
        let enabled_controls = enabled_controls.into();
        let (change_controls, remaining) = u32::try_parse(remaining)?;
        let change_controls = change_controls.into();
        let (repeat_delay, remaining) = u16::try_parse(remaining)?;
        let (repeat_interval, remaining) = u16::try_parse(remaining)?;
        let (slow_keys_delay, remaining) = u16::try_parse(remaining)?;
        let (debounce_delay, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_delay, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_interval, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_time_to_max, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_max_speed, remaining) = u16::try_parse(remaining)?;
        let (mouse_keys_curve, remaining) = i16::try_parse(remaining)?;
        let (access_x_timeout, remaining) = u16::try_parse(remaining)?;
        let (access_x_timeout_mask, remaining) = u32::try_parse(remaining)?;
        let access_x_timeout_mask = access_x_timeout_mask.into();
        let (access_x_timeout_values, remaining) = u32::try_parse(remaining)?;
        let access_x_timeout_values = access_x_timeout_values.into();
        let (access_x_timeout_options_mask, remaining) = u16::try_parse(remaining)?;
        let access_x_timeout_options_mask = access_x_timeout_options_mask.into();
        let (access_x_timeout_options_values, remaining) = u16::try_parse(remaining)?;
        let access_x_timeout_options_values = access_x_timeout_options_values.into();
        let (per_key_repeat, remaining) = crate::x11_utils::parse_u8_array_ref::<32>(remaining)?;
        let _ = remaining;
        Ok(SetControlsRequest {
            device_spec,
            affect_internal_real_mods,
            internal_real_mods,
            affect_ignore_lock_real_mods,
            ignore_lock_real_mods,
            affect_internal_virtual_mods,
            internal_virtual_mods,
            affect_ignore_lock_virtual_mods,
            ignore_lock_virtual_mods,
            mouse_keys_dflt_btn,
            groups_wrap,
            access_x_options,
            affect_enabled_controls,
            enabled_controls,
            change_controls,
            repeat_delay,
            repeat_interval,
            slow_keys_delay,
            debounce_delay,
            mouse_keys_delay,
            mouse_keys_interval,
            mouse_keys_time_to_max,
            mouse_keys_max_speed,
            mouse_keys_curve,
            access_x_timeout,
            access_x_timeout_mask,
            access_x_timeout_values,
            access_x_timeout_options_mask,
            access_x_timeout_options_values,
            per_key_repeat: Cow::Borrowed(per_key_repeat),
        })
    }
    /// Clone all borrowed data in this SetControlsRequest.
    pub fn into_owned(self) -> SetControlsRequest<'static> {
        SetControlsRequest {
            device_spec: self.device_spec,
            affect_internal_real_mods: self.affect_internal_real_mods,
            internal_real_mods: self.internal_real_mods,
            affect_ignore_lock_real_mods: self.affect_ignore_lock_real_mods,
            ignore_lock_real_mods: self.ignore_lock_real_mods,
            affect_internal_virtual_mods: self.affect_internal_virtual_mods,
            internal_virtual_mods: self.internal_virtual_mods,
            affect_ignore_lock_virtual_mods: self.affect_ignore_lock_virtual_mods,
            ignore_lock_virtual_mods: self.ignore_lock_virtual_mods,
            mouse_keys_dflt_btn: self.mouse_keys_dflt_btn,
            groups_wrap: self.groups_wrap,
            access_x_options: self.access_x_options,
            affect_enabled_controls: self.affect_enabled_controls,
            enabled_controls: self.enabled_controls,
            change_controls: self.change_controls,
            repeat_delay: self.repeat_delay,
            repeat_interval: self.repeat_interval,
            slow_keys_delay: self.slow_keys_delay,
            debounce_delay: self.debounce_delay,
            mouse_keys_delay: self.mouse_keys_delay,
            mouse_keys_interval: self.mouse_keys_interval,
            mouse_keys_time_to_max: self.mouse_keys_time_to_max,
            mouse_keys_max_speed: self.mouse_keys_max_speed,
            mouse_keys_curve: self.mouse_keys_curve,
            access_x_timeout: self.access_x_timeout,
            access_x_timeout_mask: self.access_x_timeout_mask,
            access_x_timeout_values: self.access_x_timeout_values,
            access_x_timeout_options_mask: self.access_x_timeout_options_mask,
            access_x_timeout_options_values: self.access_x_timeout_options_values,
            per_key_repeat: Cow::Owned(self.per_key_repeat.into_owned()),
        }
    }
}
impl<'input> Request for SetControlsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetControlsRequest<'input> {
}

/// Opcode for the GetMap request
pub const GET_MAP_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMapRequest {
    pub device_spec: DeviceSpec,
    pub full: MapPart,
    pub partial: MapPart,
    pub first_type: u8,
    pub n_types: u8,
    pub first_key_sym: xproto::Keycode,
    pub n_key_syms: u8,
    pub first_key_action: xproto::Keycode,
    pub n_key_actions: u8,
    pub first_key_behavior: xproto::Keycode,
    pub n_key_behaviors: u8,
    pub virtual_mods: VMod,
    pub first_key_explicit: xproto::Keycode,
    pub n_key_explicit: u8,
    pub first_mod_map_key: xproto::Keycode,
    pub n_mod_map_keys: u8,
    pub first_v_mod_map_key: xproto::Keycode,
    pub n_v_mod_map_keys: u8,
}
impl_debug_if_no_extra_traits!(GetMapRequest, "GetMapRequest");
impl GetMapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let full_bytes = u16::from(self.full).serialize();
        let partial_bytes = u16::from(self.partial).serialize();
        let first_type_bytes = self.first_type.serialize();
        let n_types_bytes = self.n_types.serialize();
        let first_key_sym_bytes = self.first_key_sym.serialize();
        let n_key_syms_bytes = self.n_key_syms.serialize();
        let first_key_action_bytes = self.first_key_action.serialize();
        let n_key_actions_bytes = self.n_key_actions.serialize();
        let first_key_behavior_bytes = self.first_key_behavior.serialize();
        let n_key_behaviors_bytes = self.n_key_behaviors.serialize();
        let virtual_mods_bytes = u16::from(self.virtual_mods).serialize();
        let first_key_explicit_bytes = self.first_key_explicit.serialize();
        let n_key_explicit_bytes = self.n_key_explicit.serialize();
        let first_mod_map_key_bytes = self.first_mod_map_key.serialize();
        let n_mod_map_keys_bytes = self.n_mod_map_keys.serialize();
        let first_v_mod_map_key_bytes = self.first_v_mod_map_key.serialize();
        let n_v_mod_map_keys_bytes = self.n_v_mod_map_keys.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_MAP_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            full_bytes[0],
            full_bytes[1],
            partial_bytes[0],
            partial_bytes[1],
            first_type_bytes[0],
            n_types_bytes[0],
            first_key_sym_bytes[0],
            n_key_syms_bytes[0],
            first_key_action_bytes[0],
            n_key_actions_bytes[0],
            first_key_behavior_bytes[0],
            n_key_behaviors_bytes[0],
            virtual_mods_bytes[0],
            virtual_mods_bytes[1],
            first_key_explicit_bytes[0],
            n_key_explicit_bytes[0],
            first_mod_map_key_bytes[0],
            n_mod_map_keys_bytes[0],
            first_v_mod_map_key_bytes[0],
            n_v_mod_map_keys_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_MAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (full, remaining) = u16::try_parse(remaining)?;
        let full = full.into();
        let (partial, remaining) = u16::try_parse(remaining)?;
        let partial = partial.into();
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (first_key_sym, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_syms, remaining) = u8::try_parse(remaining)?;
        let (first_key_action, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_actions, remaining) = u8::try_parse(remaining)?;
        let (first_key_behavior, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let virtual_mods = virtual_mods.into();
        let (first_key_explicit, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (first_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (first_v_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetMapRequest {
            device_spec,
            full,
            partial,
            first_type,
            n_types,
            first_key_sym,
            n_key_syms,
            first_key_action,
            n_key_actions,
            first_key_behavior,
            n_key_behaviors,
            virtual_mods,
            first_key_explicit,
            n_key_explicit,
            first_mod_map_key,
            n_mod_map_keys,
            first_v_mod_map_key,
            n_v_mod_map_keys,
        })
    }
}
impl Request for GetMapRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetMapRequest {
    type Reply = GetMapReply;
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMapMapKeyActions {
    pub acts_rtrn_count: Vec<u8>,
    pub acts_rtrn_acts: Vec<Action>,
}
impl_debug_if_no_extra_traits!(GetMapMapKeyActions, "GetMapMapKeyActions");
impl GetMapMapKeyActions {
    pub fn try_parse(remaining: &[u8], n_key_actions: u8, total_actions: u16) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (acts_rtrn_count, remaining) = crate::x11_utils::parse_u8_list(remaining, n_key_actions.try_to_usize()?)?;
        let acts_rtrn_count = acts_rtrn_count.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (acts_rtrn_acts, remaining) = crate::x11_utils::parse_list::<Action>(remaining, total_actions.try_to_usize()?)?;
        let result = GetMapMapKeyActions { acts_rtrn_count, acts_rtrn_acts };
        Ok((result, remaining))
    }
}
impl GetMapMapKeyActions {
    #[allow(dead_code)]
    fn serialize(&self, n_key_actions: u8, total_actions: u16) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u8::from(n_key_actions), u16::from(total_actions));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, n_key_actions: u8, total_actions: u16) {
        assert_eq!(self.acts_rtrn_count.len(), usize::try_from(n_key_actions).unwrap(), "`acts_rtrn_count` has an incorrect length");
        bytes.extend_from_slice(&self.acts_rtrn_count);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        assert_eq!(self.acts_rtrn_acts.len(), usize::try_from(total_actions).unwrap(), "`acts_rtrn_acts` has an incorrect length");
        self.acts_rtrn_acts.serialize_into(bytes);
    }
}
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMapMap {
    pub types_rtrn: Option<Vec<KeyType>>,
    pub syms_rtrn: Option<Vec<KeySymMap>>,
    pub key_actions: Option<GetMapMapKeyActions>,
    pub behaviors_rtrn: Option<Vec<SetBehavior>>,
    pub vmods_rtrn: Option<Vec<xproto::ModMask>>,
    pub explicit_rtrn: Option<Vec<SetExplicit>>,
    pub modmap_rtrn: Option<Vec<KeyModMap>>,
    pub vmodmap_rtrn: Option<Vec<KeyVModMap>>,
}
impl_debug_if_no_extra_traits!(GetMapMap, "GetMapMap");
impl GetMapMap {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u16::from(present);
        let mut outer_remaining = value;
        let types_rtrn = if switch_expr & u16::from(MapPart::KEY_TYPES) != 0 {
            let remaining = outer_remaining;
            let (types_rtrn, remaining) = crate::x11_utils::parse_list::<KeyType>(remaining, n_types.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(types_rtrn)
        } else {
            None
        };
        let syms_rtrn = if switch_expr & u16::from(MapPart::KEY_SYMS) != 0 {
            let remaining = outer_remaining;
            let (syms_rtrn, remaining) = crate::x11_utils::parse_list::<KeySymMap>(remaining, n_key_syms.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(syms_rtrn)
        } else {
            None
        };
        let key_actions = if switch_expr & u16::from(MapPart::KEY_ACTIONS) != 0 {
            let (key_actions, new_remaining) = GetMapMapKeyActions::try_parse(outer_remaining, n_key_actions, total_actions)?;
            outer_remaining = new_remaining;
            Some(key_actions)
        } else {
            None
        };
        let behaviors_rtrn = if switch_expr & u16::from(MapPart::KEY_BEHAVIORS) != 0 {
            let remaining = outer_remaining;
            let (behaviors_rtrn, remaining) = crate::x11_utils::parse_list::<SetBehavior>(remaining, total_key_behaviors.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(behaviors_rtrn)
        } else {
            None
        };
        let vmods_rtrn = if switch_expr & u16::from(MapPart::VIRTUAL_MODS) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let mut remaining = remaining;
            let list_length = u32::from(virtual_mods).count_ones().try_to_usize()?;
            let mut vmods_rtrn = Vec::with_capacity(list_length);
            for _ in 0..list_length {
                let (v, new_remaining) = u8::try_parse(remaining)?;
                let v = v.into();
                remaining = new_remaining;
                vmods_rtrn.push(v);
            }
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(vmods_rtrn)
        } else {
            None
        };
        let explicit_rtrn = if switch_expr & u16::from(MapPart::EXPLICIT_COMPONENTS) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let (explicit_rtrn, remaining) = crate::x11_utils::parse_list::<SetExplicit>(remaining, total_key_explicit.try_to_usize()?)?;
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(explicit_rtrn)
        } else {
            None
        };
        let modmap_rtrn = if switch_expr & u16::from(MapPart::MODIFIER_MAP) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let (modmap_rtrn, remaining) = crate::x11_utils::parse_list::<KeyModMap>(remaining, total_mod_map_keys.try_to_usize()?)?;
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(modmap_rtrn)
        } else {
            None
        };
        let vmodmap_rtrn = if switch_expr & u16::from(MapPart::VIRTUAL_MOD_MAP) != 0 {
            let remaining = outer_remaining;
            let (vmodmap_rtrn, remaining) = crate::x11_utils::parse_list::<KeyVModMap>(remaining, total_v_mod_map_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(vmodmap_rtrn)
        } else {
            None
        };
        let result = GetMapMap { types_rtrn, syms_rtrn, key_actions, behaviors_rtrn, vmods_rtrn, explicit_rtrn, modmap_rtrn, vmodmap_rtrn };
        Ok((result, outer_remaining))
    }
}
impl GetMapMap {
    #[allow(dead_code)]
    fn serialize(&self, present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u16::from(present), u8::from(n_types), u8::from(n_key_syms), u8::from(n_key_actions), u16::from(total_actions), u8::from(total_key_behaviors), u16::from(virtual_mods), u8::from(total_key_explicit), u8::from(total_mod_map_keys), u8::from(total_v_mod_map_keys));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) {
        assert_eq!(self.switch_expr(), u16::from(present), "switch `map` has an inconsistent discriminant");
        if let Some(ref types_rtrn) = self.types_rtrn {
            assert_eq!(types_rtrn.len(), usize::try_from(n_types).unwrap(), "`types_rtrn` has an incorrect length");
            types_rtrn.serialize_into(bytes);
        }
        if let Some(ref syms_rtrn) = self.syms_rtrn {
            assert_eq!(syms_rtrn.len(), usize::try_from(n_key_syms).unwrap(), "`syms_rtrn` has an incorrect length");
            syms_rtrn.serialize_into(bytes);
        }
        if let Some(ref key_actions) = self.key_actions {
            key_actions.serialize_into(bytes, u8::from(n_key_actions), u16::from(total_actions));
        }
        if let Some(ref behaviors_rtrn) = self.behaviors_rtrn {
            assert_eq!(behaviors_rtrn.len(), usize::try_from(total_key_behaviors).unwrap(), "`behaviors_rtrn` has an incorrect length");
            behaviors_rtrn.serialize_into(bytes);
        }
        if let Some(ref vmods_rtrn) = self.vmods_rtrn {
            assert_eq!(vmods_rtrn.len(), usize::try_from(u32::from(virtual_mods).count_ones()).unwrap(), "`vmods_rtrn` has an incorrect length");
            for element in vmods_rtrn.iter().copied() {
                (u16::from(element) as u8).serialize_into(bytes);
            }
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref explicit_rtrn) = self.explicit_rtrn {
            assert_eq!(explicit_rtrn.len(), usize::try_from(total_key_explicit).unwrap(), "`explicit_rtrn` has an incorrect length");
            explicit_rtrn.serialize_into(bytes);
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref modmap_rtrn) = self.modmap_rtrn {
            assert_eq!(modmap_rtrn.len(), usize::try_from(total_mod_map_keys).unwrap(), "`modmap_rtrn` has an incorrect length");
            modmap_rtrn.serialize_into(bytes);
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref vmodmap_rtrn) = self.vmodmap_rtrn {
            assert_eq!(vmodmap_rtrn.len(), usize::try_from(total_v_mod_map_keys).unwrap(), "`vmodmap_rtrn` has an incorrect length");
            vmodmap_rtrn.serialize_into(bytes);
        }
    }
}
impl GetMapMap {
    fn switch_expr(&self) -> u16 {
        let mut expr_value = 0;
        if self.types_rtrn.is_some() {
            expr_value |= u16::from(MapPart::KEY_TYPES);
        }
        if self.syms_rtrn.is_some() {
            expr_value |= u16::from(MapPart::KEY_SYMS);
        }
        if self.key_actions.is_some() {
            expr_value |= u16::from(MapPart::KEY_ACTIONS);
        }
        if self.behaviors_rtrn.is_some() {
            expr_value |= u16::from(MapPart::KEY_BEHAVIORS);
        }
        if self.vmods_rtrn.is_some() {
            expr_value |= u16::from(MapPart::VIRTUAL_MODS);
        }
        if self.explicit_rtrn.is_some() {
            expr_value |= u16::from(MapPart::EXPLICIT_COMPONENTS);
        }
        if self.modmap_rtrn.is_some() {
            expr_value |= u16::from(MapPart::MODIFIER_MAP);
        }
        if self.vmodmap_rtrn.is_some() {
            expr_value |= u16::from(MapPart::VIRTUAL_MOD_MAP);
        }
        expr_value
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMapReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub min_key_code: xproto::Keycode,
    pub max_key_code: xproto::Keycode,
    pub first_type: u8,
    pub n_types: u8,
    pub total_types: u8,
    pub first_key_sym: xproto::Keycode,
    pub total_syms: u16,
    pub n_key_syms: u8,
    pub first_key_action: xproto::Keycode,
    pub total_actions: u16,
    pub n_key_actions: u8,
    pub first_key_behavior: xproto::Keycode,
    pub n_key_behaviors: u8,
    pub total_key_behaviors: u8,
    pub first_key_explicit: xproto::Keycode,
    pub n_key_explicit: u8,
    pub total_key_explicit: u8,
    pub first_mod_map_key: xproto::Keycode,
    pub n_mod_map_keys: u8,
    pub total_mod_map_keys: u8,
    pub first_v_mod_map_key: xproto::Keycode,
    pub n_v_mod_map_keys: u8,
    pub total_v_mod_map_keys: u8,
    pub virtual_mods: VMod,
    pub map: GetMapMap,
}
impl_debug_if_no_extra_traits!(GetMapReply, "GetMapReply");
impl TryParse for GetMapReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (present, remaining) = u16::try_parse(remaining)?;
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (total_types, remaining) = u8::try_parse(remaining)?;
        let (first_key_sym, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (total_syms, remaining) = u16::try_parse(remaining)?;
        let (n_key_syms, remaining) = u8::try_parse(remaining)?;
        let (first_key_action, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (total_actions, remaining) = u16::try_parse(remaining)?;
        let (n_key_actions, remaining) = u8::try_parse(remaining)?;
        let (first_key_behavior, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (total_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (first_key_explicit, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (total_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (first_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (total_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (first_v_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (total_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let (map, remaining) = GetMapMap::try_parse(remaining, u16::from(present), u8::from(n_types), u8::from(n_key_syms), u8::from(n_key_actions), u16::from(total_actions), u8::from(total_key_behaviors), u16::from(virtual_mods), u8::from(total_key_explicit), u8::from(total_mod_map_keys), u8::from(total_v_mod_map_keys))?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let virtual_mods = virtual_mods.into();
        let result = GetMapReply { device_id, sequence, length, min_key_code, max_key_code, first_type, n_types, total_types, first_key_sym, total_syms, n_key_syms, first_key_action, total_actions, n_key_actions, first_key_behavior, n_key_behaviors, total_key_behaviors, first_key_explicit, n_key_explicit, total_key_explicit, first_mod_map_key, n_mod_map_keys, total_mod_map_keys, first_v_mod_map_key, n_v_mod_map_keys, total_v_mod_map_keys, virtual_mods, map };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetMapReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(40);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.min_key_code.serialize_into(bytes);
        self.max_key_code.serialize_into(bytes);
        let present: u16 = self.map.switch_expr();
        present.serialize_into(bytes);
        self.first_type.serialize_into(bytes);
        self.n_types.serialize_into(bytes);
        self.total_types.serialize_into(bytes);
        self.first_key_sym.serialize_into(bytes);
        self.total_syms.serialize_into(bytes);
        self.n_key_syms.serialize_into(bytes);
        self.first_key_action.serialize_into(bytes);
        self.total_actions.serialize_into(bytes);
        self.n_key_actions.serialize_into(bytes);
        self.first_key_behavior.serialize_into(bytes);
        self.n_key_behaviors.serialize_into(bytes);
        self.total_key_behaviors.serialize_into(bytes);
        self.first_key_explicit.serialize_into(bytes);
        self.n_key_explicit.serialize_into(bytes);
        self.total_key_explicit.serialize_into(bytes);
        self.first_mod_map_key.serialize_into(bytes);
        self.n_mod_map_keys.serialize_into(bytes);
        self.total_mod_map_keys.serialize_into(bytes);
        self.first_v_mod_map_key.serialize_into(bytes);
        self.n_v_mod_map_keys.serialize_into(bytes);
        self.total_v_mod_map_keys.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        u16::from(self.virtual_mods).serialize_into(bytes);
        self.map.serialize_into(bytes, u16::from(present), u8::from(self.n_types), u8::from(self.n_key_syms), u8::from(self.n_key_actions), u16::from(self.total_actions), u8::from(self.total_key_behaviors), u16::from(self.virtual_mods), u8::from(self.total_key_explicit), u8::from(self.total_mod_map_keys), u8::from(self.total_v_mod_map_keys));
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetMapAuxKeyActions {
    pub actions_count: Vec<u8>,
    pub actions: Vec<Action>,
}
impl_debug_if_no_extra_traits!(SetMapAuxKeyActions, "SetMapAuxKeyActions");
impl SetMapAuxKeyActions {
    pub fn try_parse(remaining: &[u8], n_key_actions: u8, total_actions: u16) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (actions_count, remaining) = crate::x11_utils::parse_u8_list(remaining, n_key_actions.try_to_usize()?)?;
        let actions_count = actions_count.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (actions, remaining) = crate::x11_utils::parse_list::<Action>(remaining, total_actions.try_to_usize()?)?;
        let result = SetMapAuxKeyActions { actions_count, actions };
        Ok((result, remaining))
    }
}
impl SetMapAuxKeyActions {
    #[allow(dead_code)]
    fn serialize(&self, n_key_actions: u8, total_actions: u16) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u8::from(n_key_actions), u16::from(total_actions));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, n_key_actions: u8, total_actions: u16) {
        assert_eq!(self.actions_count.len(), usize::try_from(n_key_actions).unwrap(), "`actions_count` has an incorrect length");
        bytes.extend_from_slice(&self.actions_count);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        assert_eq!(self.actions.len(), usize::try_from(total_actions).unwrap(), "`actions` has an incorrect length");
        self.actions.serialize_into(bytes);
    }
}
/// Auxiliary and optional information for the `set_map` function
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetMapAux {
    pub types: Option<Vec<SetKeyType>>,
    pub syms: Option<Vec<KeySymMap>>,
    pub key_actions: Option<SetMapAuxKeyActions>,
    pub behaviors: Option<Vec<SetBehavior>>,
    pub vmods: Option<Vec<u8>>,
    pub explicit: Option<Vec<SetExplicit>>,
    pub modmap: Option<Vec<KeyModMap>>,
    pub vmodmap: Option<Vec<KeyVModMap>>,
}
impl_debug_if_no_extra_traits!(SetMapAux, "SetMapAux");
impl SetMapAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u16::from(present);
        let mut outer_remaining = value;
        let types = if switch_expr & u16::from(MapPart::KEY_TYPES) != 0 {
            let remaining = outer_remaining;
            let (types, remaining) = crate::x11_utils::parse_list::<SetKeyType>(remaining, n_types.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(types)
        } else {
            None
        };
        let syms = if switch_expr & u16::from(MapPart::KEY_SYMS) != 0 {
            let remaining = outer_remaining;
            let (syms, remaining) = crate::x11_utils::parse_list::<KeySymMap>(remaining, n_key_syms.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(syms)
        } else {
            None
        };
        let key_actions = if switch_expr & u16::from(MapPart::KEY_ACTIONS) != 0 {
            let (key_actions, new_remaining) = SetMapAuxKeyActions::try_parse(outer_remaining, n_key_actions, total_actions)?;
            outer_remaining = new_remaining;
            Some(key_actions)
        } else {
            None
        };
        let behaviors = if switch_expr & u16::from(MapPart::KEY_BEHAVIORS) != 0 {
            let remaining = outer_remaining;
            let (behaviors, remaining) = crate::x11_utils::parse_list::<SetBehavior>(remaining, total_key_behaviors.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(behaviors)
        } else {
            None
        };
        let vmods = if switch_expr & u16::from(MapPart::VIRTUAL_MODS) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let (vmods, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(virtual_mods).count_ones().try_to_usize()?)?;
            let vmods = vmods.to_vec();
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(vmods)
        } else {
            None
        };
        let explicit = if switch_expr & u16::from(MapPart::EXPLICIT_COMPONENTS) != 0 {
            let remaining = outer_remaining;
            let (explicit, remaining) = crate::x11_utils::parse_list::<SetExplicit>(remaining, total_key_explicit.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(explicit)
        } else {
            None
        };
        let modmap = if switch_expr & u16::from(MapPart::MODIFIER_MAP) != 0 {
            let remaining = outer_remaining;
            let (modmap, remaining) = crate::x11_utils::parse_list::<KeyModMap>(remaining, total_mod_map_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(modmap)
        } else {
            None
        };
        let vmodmap = if switch_expr & u16::from(MapPart::VIRTUAL_MOD_MAP) != 0 {
            let remaining = outer_remaining;
            let (vmodmap, remaining) = crate::x11_utils::parse_list::<KeyVModMap>(remaining, total_v_mod_map_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(vmodmap)
        } else {
            None
        };
        let result = SetMapAux { types, syms, key_actions, behaviors, vmods, explicit, modmap, vmodmap };
        Ok((result, outer_remaining))
    }
}
impl SetMapAux {
    #[allow(dead_code)]
    fn serialize(&self, present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u16::from(present), u8::from(n_types), u8::from(n_key_syms), u8::from(n_key_actions), u16::from(total_actions), u8::from(total_key_behaviors), u16::from(virtual_mods), u8::from(total_key_explicit), u8::from(total_mod_map_keys), u8::from(total_v_mod_map_keys));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) {
        assert_eq!(self.switch_expr(), u16::from(present), "switch `values` has an inconsistent discriminant");
        if let Some(ref types) = self.types {
            assert_eq!(types.len(), usize::try_from(n_types).unwrap(), "`types` has an incorrect length");
            types.serialize_into(bytes);
        }
        if let Some(ref syms) = self.syms {
            assert_eq!(syms.len(), usize::try_from(n_key_syms).unwrap(), "`syms` has an incorrect length");
            syms.serialize_into(bytes);
        }
        if let Some(ref key_actions) = self.key_actions {
            key_actions.serialize_into(bytes, u8::from(n_key_actions), u16::from(total_actions));
        }
        if let Some(ref behaviors) = self.behaviors {
            assert_eq!(behaviors.len(), usize::try_from(total_key_behaviors).unwrap(), "`behaviors` has an incorrect length");
            behaviors.serialize_into(bytes);
        }
        if let Some(ref vmods) = self.vmods {
            assert_eq!(vmods.len(), usize::try_from(u32::from(virtual_mods).count_ones()).unwrap(), "`vmods` has an incorrect length");
            bytes.extend_from_slice(&vmods);
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref explicit) = self.explicit {
            assert_eq!(explicit.len(), usize::try_from(total_key_explicit).unwrap(), "`explicit` has an incorrect length");
            explicit.serialize_into(bytes);
        }
        if let Some(ref modmap) = self.modmap {
            assert_eq!(modmap.len(), usize::try_from(total_mod_map_keys).unwrap(), "`modmap` has an incorrect length");
            modmap.serialize_into(bytes);
        }
        if let Some(ref vmodmap) = self.vmodmap {
            assert_eq!(vmodmap.len(), usize::try_from(total_v_mod_map_keys).unwrap(), "`vmodmap` has an incorrect length");
            vmodmap.serialize_into(bytes);
        }
    }
}
impl SetMapAux {
    fn switch_expr(&self) -> u16 {
        let mut expr_value = 0;
        if self.types.is_some() {
            expr_value |= u16::from(MapPart::KEY_TYPES);
        }
        if self.syms.is_some() {
            expr_value |= u16::from(MapPart::KEY_SYMS);
        }
        if self.key_actions.is_some() {
            expr_value |= u16::from(MapPart::KEY_ACTIONS);
        }
        if self.behaviors.is_some() {
            expr_value |= u16::from(MapPart::KEY_BEHAVIORS);
        }
        if self.vmods.is_some() {
            expr_value |= u16::from(MapPart::VIRTUAL_MODS);
        }
        if self.explicit.is_some() {
            expr_value |= u16::from(MapPart::EXPLICIT_COMPONENTS);
        }
        if self.modmap.is_some() {
            expr_value |= u16::from(MapPart::MODIFIER_MAP);
        }
        if self.vmodmap.is_some() {
            expr_value |= u16::from(MapPart::VIRTUAL_MOD_MAP);
        }
        expr_value
    }
}
impl SetMapAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `types` field of this structure.
    #[must_use]
    pub fn types<I>(mut self, value: I) -> Self where I: Into<Option<Vec<SetKeyType>>> {
        self.types = value.into();
        self
    }
    /// Set the `syms` field of this structure.
    #[must_use]
    pub fn syms<I>(mut self, value: I) -> Self where I: Into<Option<Vec<KeySymMap>>> {
        self.syms = value.into();
        self
    }
    /// Set the `key_actions` field of this structure.
    #[must_use]
    pub fn key_actions<I>(mut self, value: I) -> Self where I: Into<Option<SetMapAuxKeyActions>> {
        self.key_actions = value.into();
        self
    }
    /// Set the `behaviors` field of this structure.
    #[must_use]
    pub fn behaviors<I>(mut self, value: I) -> Self where I: Into<Option<Vec<SetBehavior>>> {
        self.behaviors = value.into();
        self
    }
    /// Set the `vmods` field of this structure.
    #[must_use]
    pub fn vmods<I>(mut self, value: I) -> Self where I: Into<Option<Vec<u8>>> {
        self.vmods = value.into();
        self
    }
    /// Set the `explicit` field of this structure.
    #[must_use]
    pub fn explicit<I>(mut self, value: I) -> Self where I: Into<Option<Vec<SetExplicit>>> {
        self.explicit = value.into();
        self
    }
    /// Set the `modmap` field of this structure.
    #[must_use]
    pub fn modmap<I>(mut self, value: I) -> Self where I: Into<Option<Vec<KeyModMap>>> {
        self.modmap = value.into();
        self
    }
    /// Set the `vmodmap` field of this structure.
    #[must_use]
    pub fn vmodmap<I>(mut self, value: I) -> Self where I: Into<Option<Vec<KeyVModMap>>> {
        self.vmodmap = value.into();
        self
    }
}

/// Opcode for the SetMap request
pub const SET_MAP_REQUEST: u8 = 9;
#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetMapRequest<'input> {
    pub device_spec: DeviceSpec,
    pub flags: SetMapFlags,
    pub min_key_code: xproto::Keycode,
    pub max_key_code: xproto::Keycode,
    pub first_type: u8,
    pub n_types: u8,
    pub first_key_sym: xproto::Keycode,
    pub n_key_syms: u8,
    pub total_syms: u16,
    pub first_key_action: xproto::Keycode,
    pub n_key_actions: u8,
    pub total_actions: u16,
    pub first_key_behavior: xproto::Keycode,
    pub n_key_behaviors: u8,
    pub total_key_behaviors: u8,
    pub first_key_explicit: xproto::Keycode,
    pub n_key_explicit: u8,
    pub total_key_explicit: u8,
    pub first_mod_map_key: xproto::Keycode,
    pub n_mod_map_keys: u8,
    pub total_mod_map_keys: u8,
    pub first_v_mod_map_key: xproto::Keycode,
    pub n_v_mod_map_keys: u8,
    pub total_v_mod_map_keys: u8,
    pub virtual_mods: VMod,
    pub values: Cow<'input, SetMapAux>,
}
impl_debug_if_no_extra_traits!(SetMapRequest<'_>, "SetMapRequest");
impl<'input> SetMapRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let present: u16 = self.values.switch_expr();
        let present_bytes = present.serialize();
        let flags_bytes = u16::from(self.flags).serialize();
        let min_key_code_bytes = self.min_key_code.serialize();
        let max_key_code_bytes = self.max_key_code.serialize();
        let first_type_bytes = self.first_type.serialize();
        let n_types_bytes = self.n_types.serialize();
        let first_key_sym_bytes = self.first_key_sym.serialize();
        let n_key_syms_bytes = self.n_key_syms.serialize();
        let total_syms_bytes = self.total_syms.serialize();
        let first_key_action_bytes = self.first_key_action.serialize();
        let n_key_actions_bytes = self.n_key_actions.serialize();
        let total_actions_bytes = self.total_actions.serialize();
        let first_key_behavior_bytes = self.first_key_behavior.serialize();
        let n_key_behaviors_bytes = self.n_key_behaviors.serialize();
        let total_key_behaviors_bytes = self.total_key_behaviors.serialize();
        let first_key_explicit_bytes = self.first_key_explicit.serialize();
        let n_key_explicit_bytes = self.n_key_explicit.serialize();
        let total_key_explicit_bytes = self.total_key_explicit.serialize();
        let first_mod_map_key_bytes = self.first_mod_map_key.serialize();
        let n_mod_map_keys_bytes = self.n_mod_map_keys.serialize();
        let total_mod_map_keys_bytes = self.total_mod_map_keys.serialize();
        let first_v_mod_map_key_bytes = self.first_v_mod_map_key.serialize();
        let n_v_mod_map_keys_bytes = self.n_v_mod_map_keys.serialize();
        let total_v_mod_map_keys_bytes = self.total_v_mod_map_keys.serialize();
        let virtual_mods_bytes = u16::from(self.virtual_mods).serialize();
        let mut request0 = vec![
            major_opcode,
            SET_MAP_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            present_bytes[0],
            present_bytes[1],
            flags_bytes[0],
            flags_bytes[1],
            min_key_code_bytes[0],
            max_key_code_bytes[0],
            first_type_bytes[0],
            n_types_bytes[0],
            first_key_sym_bytes[0],
            n_key_syms_bytes[0],
            total_syms_bytes[0],
            total_syms_bytes[1],
            first_key_action_bytes[0],
            n_key_actions_bytes[0],
            total_actions_bytes[0],
            total_actions_bytes[1],
            first_key_behavior_bytes[0],
            n_key_behaviors_bytes[0],
            total_key_behaviors_bytes[0],
            first_key_explicit_bytes[0],
            n_key_explicit_bytes[0],
            total_key_explicit_bytes[0],
            first_mod_map_key_bytes[0],
            n_mod_map_keys_bytes[0],
            total_mod_map_keys_bytes[0],
            first_v_mod_map_key_bytes[0],
            n_v_mod_map_keys_bytes[0],
            total_v_mod_map_keys_bytes[0],
            virtual_mods_bytes[0],
            virtual_mods_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let values_bytes = self.values.serialize(u16::from(present), u8::from(self.n_types), u8::from(self.n_key_syms), u8::from(self.n_key_actions), u16::from(self.total_actions), u8::from(self.total_key_behaviors), u16::from(self.virtual_mods), u8::from(self.total_key_explicit), u8::from(self.total_mod_map_keys), u8::from(self.total_v_mod_map_keys));
        let length_so_far = length_so_far + values_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), values_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_MAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (present, remaining) = u16::try_parse(remaining)?;
        let (flags, remaining) = u16::try_parse(remaining)?;
        let flags = flags.into();
        let (min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (first_key_sym, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_syms, remaining) = u8::try_parse(remaining)?;
        let (total_syms, remaining) = u16::try_parse(remaining)?;
        let (first_key_action, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_actions, remaining) = u8::try_parse(remaining)?;
        let (total_actions, remaining) = u16::try_parse(remaining)?;
        let (first_key_behavior, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (total_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (first_key_explicit, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (total_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (first_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (total_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (first_v_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (total_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let virtual_mods = virtual_mods.into();
        let (values, remaining) = SetMapAux::try_parse(remaining, u16::from(present), u8::from(n_types), u8::from(n_key_syms), u8::from(n_key_actions), u16::from(total_actions), u8::from(total_key_behaviors), u16::from(virtual_mods), u8::from(total_key_explicit), u8::from(total_mod_map_keys), u8::from(total_v_mod_map_keys))?;
        let _ = remaining;
        Ok(SetMapRequest {
            device_spec,
            flags,
            min_key_code,
            max_key_code,
            first_type,
            n_types,
            first_key_sym,
            n_key_syms,
            total_syms,
            first_key_action,
            n_key_actions,
            total_actions,
            first_key_behavior,
            n_key_behaviors,
            total_key_behaviors,
            first_key_explicit,
            n_key_explicit,
            total_key_explicit,
            first_mod_map_key,
            n_mod_map_keys,
            total_mod_map_keys,
            first_v_mod_map_key,
            n_v_mod_map_keys,
            total_v_mod_map_keys,
            virtual_mods,
            values: Cow::Owned(values),
        })
    }
    /// Clone all borrowed data in this SetMapRequest.
    pub fn into_owned(self) -> SetMapRequest<'static> {
        SetMapRequest {
            device_spec: self.device_spec,
            flags: self.flags,
            min_key_code: self.min_key_code,
            max_key_code: self.max_key_code,
            first_type: self.first_type,
            n_types: self.n_types,
            first_key_sym: self.first_key_sym,
            n_key_syms: self.n_key_syms,
            total_syms: self.total_syms,
            first_key_action: self.first_key_action,
            n_key_actions: self.n_key_actions,
            total_actions: self.total_actions,
            first_key_behavior: self.first_key_behavior,
            n_key_behaviors: self.n_key_behaviors,
            total_key_behaviors: self.total_key_behaviors,
            first_key_explicit: self.first_key_explicit,
            n_key_explicit: self.n_key_explicit,
            total_key_explicit: self.total_key_explicit,
            first_mod_map_key: self.first_mod_map_key,
            n_mod_map_keys: self.n_mod_map_keys,
            total_mod_map_keys: self.total_mod_map_keys,
            first_v_mod_map_key: self.first_v_mod_map_key,
            n_v_mod_map_keys: self.n_v_mod_map_keys,
            total_v_mod_map_keys: self.total_v_mod_map_keys,
            virtual_mods: self.virtual_mods,
            values: Cow::Owned(self.values.into_owned()),
        }
    }
}
impl<'input> Request for SetMapRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetMapRequest<'input> {
}

/// Opcode for the GetCompatMap request
pub const GET_COMPAT_MAP_REQUEST: u8 = 10;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCompatMapRequest {
    pub device_spec: DeviceSpec,
    pub groups: SetOfGroup,
    pub get_all_si: bool,
    pub first_si: u16,
    pub n_si: u16,
}
impl_debug_if_no_extra_traits!(GetCompatMapRequest, "GetCompatMapRequest");
impl GetCompatMapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let groups_bytes = u8::from(self.groups).serialize();
        let get_all_si_bytes = self.get_all_si.serialize();
        let first_si_bytes = self.first_si.serialize();
        let n_si_bytes = self.n_si.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_COMPAT_MAP_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            groups_bytes[0],
            get_all_si_bytes[0],
            first_si_bytes[0],
            first_si_bytes[1],
            n_si_bytes[0],
            n_si_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_COMPAT_MAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (groups, remaining) = u8::try_parse(remaining)?;
        let groups = groups.into();
        let (get_all_si, remaining) = bool::try_parse(remaining)?;
        let (first_si, remaining) = u16::try_parse(remaining)?;
        let (n_si, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetCompatMapRequest {
            device_spec,
            groups,
            get_all_si,
            first_si,
            n_si,
        })
    }
}
impl Request for GetCompatMapRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetCompatMapRequest {
    type Reply = GetCompatMapReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCompatMapReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub groups_rtrn: SetOfGroup,
    pub first_si_rtrn: u16,
    pub n_total_si: u16,
    pub si_rtrn: Vec<SymInterpret>,
    pub group_rtrn: Vec<ModDef>,
}
impl_debug_if_no_extra_traits!(GetCompatMapReply, "GetCompatMapReply");
impl TryParse for GetCompatMapReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (groups_rtrn, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (first_si_rtrn, remaining) = u16::try_parse(remaining)?;
        let (n_si_rtrn, remaining) = u16::try_parse(remaining)?;
        let (n_total_si, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (si_rtrn, remaining) = crate::x11_utils::parse_list::<SymInterpret>(remaining, n_si_rtrn.try_to_usize()?)?;
        let (group_rtrn, remaining) = crate::x11_utils::parse_list::<ModDef>(remaining, u32::from(groups_rtrn).count_ones().try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let groups_rtrn = groups_rtrn.into();
        let result = GetCompatMapReply { device_id, sequence, length, groups_rtrn, first_si_rtrn, n_total_si, si_rtrn, group_rtrn };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetCompatMapReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        u8::from(self.groups_rtrn).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.first_si_rtrn.serialize_into(bytes);
        let n_si_rtrn = u16::try_from(self.si_rtrn.len()).expect("`si_rtrn` has too many elements");
        n_si_rtrn.serialize_into(bytes);
        self.n_total_si.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        self.si_rtrn.serialize_into(bytes);
        assert_eq!(self.group_rtrn.len(), usize::try_from(u32::from(self.groups_rtrn).count_ones()).unwrap(), "`group_rtrn` has an incorrect length");
        self.group_rtrn.serialize_into(bytes);
    }
}
impl GetCompatMapReply {
    /// Get the value of the `nSIRtrn` field.
    ///
    /// The `nSIRtrn` field is used as the length field of the `si_rtrn` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_si_rtrn(&self) -> u16 {
        self.si_rtrn.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetCompatMap request
pub const SET_COMPAT_MAP_REQUEST: u8 = 11;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetCompatMapRequest<'input> {
    pub device_spec: DeviceSpec,
    pub recompute_actions: bool,
    pub truncate_si: bool,
    pub groups: SetOfGroup,
    pub first_si: u16,
    pub si: Cow<'input, [SymInterpret]>,
    pub group_maps: Cow<'input, [ModDef]>,
}
impl_debug_if_no_extra_traits!(SetCompatMapRequest<'_>, "SetCompatMapRequest");
impl<'input> SetCompatMapRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let recompute_actions_bytes = self.recompute_actions.serialize();
        let truncate_si_bytes = self.truncate_si.serialize();
        let groups_bytes = u8::from(self.groups).serialize();
        let first_si_bytes = self.first_si.serialize();
        let n_si = u16::try_from(self.si.len()).expect("`si` has too many elements");
        let n_si_bytes = n_si.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_COMPAT_MAP_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            recompute_actions_bytes[0],
            truncate_si_bytes[0],
            groups_bytes[0],
            first_si_bytes[0],
            first_si_bytes[1],
            n_si_bytes[0],
            n_si_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let si_bytes = self.si.serialize();
        let length_so_far = length_so_far + si_bytes.len();
        assert_eq!(self.group_maps.len(), usize::try_from(u32::from(self.groups).count_ones()).unwrap(), "`group_maps` has an incorrect length");
        let group_maps_bytes = self.group_maps.serialize();
        let length_so_far = length_so_far + group_maps_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), si_bytes.into(), group_maps_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_COMPAT_MAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (recompute_actions, remaining) = bool::try_parse(remaining)?;
        let (truncate_si, remaining) = bool::try_parse(remaining)?;
        let (groups, remaining) = u8::try_parse(remaining)?;
        let groups = groups.into();
        let (first_si, remaining) = u16::try_parse(remaining)?;
        let (n_si, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (si, remaining) = crate::x11_utils::parse_list::<SymInterpret>(remaining, n_si.try_to_usize()?)?;
        let (group_maps, remaining) = crate::x11_utils::parse_list::<ModDef>(remaining, u32::from(groups).count_ones().try_to_usize()?)?;
        let _ = remaining;
        Ok(SetCompatMapRequest {
            device_spec,
            recompute_actions,
            truncate_si,
            groups,
            first_si,
            si: Cow::Owned(si),
            group_maps: Cow::Owned(group_maps),
        })
    }
    /// Clone all borrowed data in this SetCompatMapRequest.
    pub fn into_owned(self) -> SetCompatMapRequest<'static> {
        SetCompatMapRequest {
            device_spec: self.device_spec,
            recompute_actions: self.recompute_actions,
            truncate_si: self.truncate_si,
            groups: self.groups,
            first_si: self.first_si,
            si: Cow::Owned(self.si.into_owned()),
            group_maps: Cow::Owned(self.group_maps.into_owned()),
        }
    }
}
impl<'input> Request for SetCompatMapRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetCompatMapRequest<'input> {
}

/// Opcode for the GetIndicatorState request
pub const GET_INDICATOR_STATE_REQUEST: u8 = 12;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetIndicatorStateRequest {
    pub device_spec: DeviceSpec,
}
impl_debug_if_no_extra_traits!(GetIndicatorStateRequest, "GetIndicatorStateRequest");
impl GetIndicatorStateRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_INDICATOR_STATE_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_INDICATOR_STATE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetIndicatorStateRequest {
            device_spec,
        })
    }
}
impl Request for GetIndicatorStateRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetIndicatorStateRequest {
    type Reply = GetIndicatorStateReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetIndicatorStateReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub state: u32,
}
impl_debug_if_no_extra_traits!(GetIndicatorStateReply, "GetIndicatorStateReply");
impl TryParse for GetIndicatorStateReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (state, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetIndicatorStateReply { device_id, sequence, length, state };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetIndicatorStateReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let device_id_bytes = self.device_id.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let state_bytes = self.state.serialize();
        [
            response_type_bytes[0],
            device_id_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            state_bytes[0],
            state_bytes[1],
            state_bytes[2],
            state_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.state.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

/// Opcode for the GetIndicatorMap request
pub const GET_INDICATOR_MAP_REQUEST: u8 = 13;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetIndicatorMapRequest {
    pub device_spec: DeviceSpec,
    pub which: u32,
}
impl_debug_if_no_extra_traits!(GetIndicatorMapRequest, "GetIndicatorMapRequest");
impl GetIndicatorMapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let which_bytes = self.which.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_INDICATOR_MAP_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
            which_bytes[0],
            which_bytes[1],
            which_bytes[2],
            which_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_INDICATOR_MAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetIndicatorMapRequest {
            device_spec,
            which,
        })
    }
}
impl Request for GetIndicatorMapRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetIndicatorMapRequest {
    type Reply = GetIndicatorMapReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetIndicatorMapReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub which: u32,
    pub real_indicators: u32,
    pub n_indicators: u8,
    pub maps: Vec<IndicatorMap>,
}
impl_debug_if_no_extra_traits!(GetIndicatorMapReply, "GetIndicatorMapReply");
impl TryParse for GetIndicatorMapReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let (real_indicators, remaining) = u32::try_parse(remaining)?;
        let (n_indicators, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(15..).ok_or(ParseError::InsufficientData)?;
        let (maps, remaining) = crate::x11_utils::parse_list::<IndicatorMap>(remaining, u32::from(which).count_ones().try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetIndicatorMapReply { device_id, sequence, length, which, real_indicators, n_indicators, maps };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetIndicatorMapReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.which.serialize_into(bytes);
        self.real_indicators.serialize_into(bytes);
        self.n_indicators.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 15]);
        assert_eq!(self.maps.len(), usize::try_from(u32::from(self.which).count_ones()).unwrap(), "`maps` has an incorrect length");
        self.maps.serialize_into(bytes);
    }
}

/// Opcode for the SetIndicatorMap request
pub const SET_INDICATOR_MAP_REQUEST: u8 = 14;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetIndicatorMapRequest<'input> {
    pub device_spec: DeviceSpec,
    pub which: u32,
    pub maps: Cow<'input, [IndicatorMap]>,
}
impl_debug_if_no_extra_traits!(SetIndicatorMapRequest<'_>, "SetIndicatorMapRequest");
impl<'input> SetIndicatorMapRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let which_bytes = self.which.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_INDICATOR_MAP_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
            which_bytes[0],
            which_bytes[1],
            which_bytes[2],
            which_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(self.maps.len(), usize::try_from(u32::from(self.which).count_ones()).unwrap(), "`maps` has an incorrect length");
        let maps_bytes = self.maps.serialize();
        let length_so_far = length_so_far + maps_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), maps_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_INDICATOR_MAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let (maps, remaining) = crate::x11_utils::parse_list::<IndicatorMap>(remaining, u32::from(which).count_ones().try_to_usize()?)?;
        let _ = remaining;
        Ok(SetIndicatorMapRequest {
            device_spec,
            which,
            maps: Cow::Owned(maps),
        })
    }
    /// Clone all borrowed data in this SetIndicatorMapRequest.
    pub fn into_owned(self) -> SetIndicatorMapRequest<'static> {
        SetIndicatorMapRequest {
            device_spec: self.device_spec,
            which: self.which,
            maps: Cow::Owned(self.maps.into_owned()),
        }
    }
}
impl<'input> Request for SetIndicatorMapRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetIndicatorMapRequest<'input> {
}

/// Opcode for the GetNamedIndicator request
pub const GET_NAMED_INDICATOR_REQUEST: u8 = 15;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNamedIndicatorRequest {
    pub device_spec: DeviceSpec,
    pub led_class: LedClass,
    pub led_id: IDSpec,
    pub indicator: xproto::Atom,
}
impl_debug_if_no_extra_traits!(GetNamedIndicatorRequest, "GetNamedIndicatorRequest");
impl GetNamedIndicatorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let led_class_bytes = LedClassSpec::from(self.led_class).serialize();
        let led_id_bytes = self.led_id.serialize();
        let indicator_bytes = self.indicator.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_NAMED_INDICATOR_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            led_class_bytes[0],
            led_class_bytes[1],
            led_id_bytes[0],
            led_id_bytes[1],
            0,
            0,
            indicator_bytes[0],
            indicator_bytes[1],
            indicator_bytes[2],
            indicator_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_NAMED_INDICATOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (led_class, remaining) = LedClassSpec::try_parse(remaining)?;
        let led_class = led_class.into();
        let (led_id, remaining) = IDSpec::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (indicator, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetNamedIndicatorRequest {
            device_spec,
            led_class,
            led_id,
            indicator,
        })
    }
}
impl Request for GetNamedIndicatorRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetNamedIndicatorRequest {
    type Reply = GetNamedIndicatorReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNamedIndicatorReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub indicator: xproto::Atom,
    pub found: bool,
    pub on: bool,
    pub real_indicator: bool,
    pub ndx: u8,
    pub map_flags: IMFlag,
    pub map_which_groups: IMGroupsWhich,
    pub map_groups: SetOfGroups,
    pub map_which_mods: IMModsWhich,
    pub map_mods: xproto::ModMask,
    pub map_real_mods: xproto::ModMask,
    pub map_vmod: VMod,
    pub map_ctrls: BoolCtrl,
    pub supported: bool,
}
impl_debug_if_no_extra_traits!(GetNamedIndicatorReply, "GetNamedIndicatorReply");
impl TryParse for GetNamedIndicatorReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (indicator, remaining) = xproto::Atom::try_parse(remaining)?;
        let (found, remaining) = bool::try_parse(remaining)?;
        let (on, remaining) = bool::try_parse(remaining)?;
        let (real_indicator, remaining) = bool::try_parse(remaining)?;
        let (ndx, remaining) = u8::try_parse(remaining)?;
        let (map_flags, remaining) = u8::try_parse(remaining)?;
        let (map_which_groups, remaining) = u8::try_parse(remaining)?;
        let (map_groups, remaining) = u8::try_parse(remaining)?;
        let (map_which_mods, remaining) = u8::try_parse(remaining)?;
        let (map_mods, remaining) = u8::try_parse(remaining)?;
        let (map_real_mods, remaining) = u8::try_parse(remaining)?;
        let (map_vmod, remaining) = u16::try_parse(remaining)?;
        let (map_ctrls, remaining) = u32::try_parse(remaining)?;
        let (supported, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let map_flags = map_flags.into();
        let map_which_groups = map_which_groups.into();
        let map_groups = map_groups.into();
        let map_which_mods = map_which_mods.into();
        let map_mods = map_mods.into();
        let map_real_mods = map_real_mods.into();
        let map_vmod = map_vmod.into();
        let map_ctrls = map_ctrls.into();
        let result = GetNamedIndicatorReply { device_id, sequence, length, indicator, found, on, real_indicator, ndx, map_flags, map_which_groups, map_groups, map_which_mods, map_mods, map_real_mods, map_vmod, map_ctrls, supported };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetNamedIndicatorReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let device_id_bytes = self.device_id.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let indicator_bytes = self.indicator.serialize();
        let found_bytes = self.found.serialize();
        let on_bytes = self.on.serialize();
        let real_indicator_bytes = self.real_indicator.serialize();
        let ndx_bytes = self.ndx.serialize();
        let map_flags_bytes = u8::from(self.map_flags).serialize();
        let map_which_groups_bytes = u8::from(self.map_which_groups).serialize();
        let map_groups_bytes = u8::from(self.map_groups).serialize();
        let map_which_mods_bytes = u8::from(self.map_which_mods).serialize();
        let map_mods_bytes = (u16::from(self.map_mods) as u8).serialize();
        let map_real_mods_bytes = (u16::from(self.map_real_mods) as u8).serialize();
        let map_vmod_bytes = u16::from(self.map_vmod).serialize();
        let map_ctrls_bytes = u32::from(self.map_ctrls).serialize();
        let supported_bytes = self.supported.serialize();
        [
            response_type_bytes[0],
            device_id_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            indicator_bytes[0],
            indicator_bytes[1],
            indicator_bytes[2],
            indicator_bytes[3],
            found_bytes[0],
            on_bytes[0],
            real_indicator_bytes[0],
            ndx_bytes[0],
            map_flags_bytes[0],
            map_which_groups_bytes[0],
            map_groups_bytes[0],
            map_which_mods_bytes[0],
            map_mods_bytes[0],
            map_real_mods_bytes[0],
            map_vmod_bytes[0],
            map_vmod_bytes[1],
            map_ctrls_bytes[0],
            map_ctrls_bytes[1],
            map_ctrls_bytes[2],
            map_ctrls_bytes[3],
            supported_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.indicator.serialize_into(bytes);
        self.found.serialize_into(bytes);
        self.on.serialize_into(bytes);
        self.real_indicator.serialize_into(bytes);
        self.ndx.serialize_into(bytes);
        u8::from(self.map_flags).serialize_into(bytes);
        u8::from(self.map_which_groups).serialize_into(bytes);
        u8::from(self.map_groups).serialize_into(bytes);
        u8::from(self.map_which_mods).serialize_into(bytes);
        (u16::from(self.map_mods) as u8).serialize_into(bytes);
        (u16::from(self.map_real_mods) as u8).serialize_into(bytes);
        u16::from(self.map_vmod).serialize_into(bytes);
        u32::from(self.map_ctrls).serialize_into(bytes);
        self.supported.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}

/// Opcode for the SetNamedIndicator request
pub const SET_NAMED_INDICATOR_REQUEST: u8 = 16;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetNamedIndicatorRequest {
    pub device_spec: DeviceSpec,
    pub led_class: LedClass,
    pub led_id: IDSpec,
    pub indicator: xproto::Atom,
    pub set_state: bool,
    pub on: bool,
    pub set_map: bool,
    pub create_map: bool,
    pub map_flags: IMFlag,
    pub map_which_groups: IMGroupsWhich,
    pub map_groups: SetOfGroups,
    pub map_which_mods: IMModsWhich,
    pub map_real_mods: xproto::ModMask,
    pub map_vmods: VMod,
    pub map_ctrls: BoolCtrl,
}
impl_debug_if_no_extra_traits!(SetNamedIndicatorRequest, "SetNamedIndicatorRequest");
impl SetNamedIndicatorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let led_class_bytes = LedClassSpec::from(self.led_class).serialize();
        let led_id_bytes = self.led_id.serialize();
        let indicator_bytes = self.indicator.serialize();
        let set_state_bytes = self.set_state.serialize();
        let on_bytes = self.on.serialize();
        let set_map_bytes = self.set_map.serialize();
        let create_map_bytes = self.create_map.serialize();
        let map_flags_bytes = u8::from(self.map_flags).serialize();
        let map_which_groups_bytes = u8::from(self.map_which_groups).serialize();
        let map_groups_bytes = u8::from(self.map_groups).serialize();
        let map_which_mods_bytes = u8::from(self.map_which_mods).serialize();
        let map_real_mods_bytes = (u16::from(self.map_real_mods) as u8).serialize();
        let map_vmods_bytes = u16::from(self.map_vmods).serialize();
        let map_ctrls_bytes = u32::from(self.map_ctrls).serialize();
        let mut request0 = vec![
            major_opcode,
            SET_NAMED_INDICATOR_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            led_class_bytes[0],
            led_class_bytes[1],
            led_id_bytes[0],
            led_id_bytes[1],
            0,
            0,
            indicator_bytes[0],
            indicator_bytes[1],
            indicator_bytes[2],
            indicator_bytes[3],
            set_state_bytes[0],
            on_bytes[0],
            set_map_bytes[0],
            create_map_bytes[0],
            0,
            map_flags_bytes[0],
            map_which_groups_bytes[0],
            map_groups_bytes[0],
            map_which_mods_bytes[0],
            map_real_mods_bytes[0],
            map_vmods_bytes[0],
            map_vmods_bytes[1],
            map_ctrls_bytes[0],
            map_ctrls_bytes[1],
            map_ctrls_bytes[2],
            map_ctrls_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_NAMED_INDICATOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (led_class, remaining) = LedClassSpec::try_parse(remaining)?;
        let led_class = led_class.into();
        let (led_id, remaining) = IDSpec::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (indicator, remaining) = xproto::Atom::try_parse(remaining)?;
        let (set_state, remaining) = bool::try_parse(remaining)?;
        let (on, remaining) = bool::try_parse(remaining)?;
        let (set_map, remaining) = bool::try_parse(remaining)?;
        let (create_map, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (map_flags, remaining) = u8::try_parse(remaining)?;
        let map_flags = map_flags.into();
        let (map_which_groups, remaining) = u8::try_parse(remaining)?;
        let map_which_groups = map_which_groups.into();
        let (map_groups, remaining) = u8::try_parse(remaining)?;
        let map_groups = map_groups.into();
        let (map_which_mods, remaining) = u8::try_parse(remaining)?;
        let map_which_mods = map_which_mods.into();
        let (map_real_mods, remaining) = u8::try_parse(remaining)?;
        let map_real_mods = map_real_mods.into();
        let (map_vmods, remaining) = u16::try_parse(remaining)?;
        let map_vmods = map_vmods.into();
        let (map_ctrls, remaining) = u32::try_parse(remaining)?;
        let map_ctrls = map_ctrls.into();
        let _ = remaining;
        Ok(SetNamedIndicatorRequest {
            device_spec,
            led_class,
            led_id,
            indicator,
            set_state,
            on,
            set_map,
            create_map,
            map_flags,
            map_which_groups,
            map_groups,
            map_which_mods,
            map_real_mods,
            map_vmods,
            map_ctrls,
        })
    }
}
impl Request for SetNamedIndicatorRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetNamedIndicatorRequest {
}

/// Opcode for the GetNames request
pub const GET_NAMES_REQUEST: u8 = 17;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNamesRequest {
    pub device_spec: DeviceSpec,
    pub which: NameDetail,
}
impl_debug_if_no_extra_traits!(GetNamesRequest, "GetNamesRequest");
impl GetNamesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let which_bytes = u32::from(self.which).serialize();
        let mut request0 = vec![
            major_opcode,
            GET_NAMES_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
            which_bytes[0],
            which_bytes[1],
            which_bytes[2],
            which_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_NAMES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let which = which.into();
        let _ = remaining;
        Ok(GetNamesRequest {
            device_spec,
            which,
        })
    }
}
impl Request for GetNamesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetNamesRequest {
    type Reply = GetNamesReply;
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNamesValueListKTLevelNames {
    pub n_levels_per_type: Vec<u8>,
    pub kt_level_names: Vec<xproto::Atom>,
}
impl_debug_if_no_extra_traits!(GetNamesValueListKTLevelNames, "GetNamesValueListKTLevelNames");
impl GetNamesValueListKTLevelNames {
    pub fn try_parse(remaining: &[u8], n_types: u8) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (n_levels_per_type, remaining) = crate::x11_utils::parse_u8_list(remaining, n_types.try_to_usize()?)?;
        let n_levels_per_type = n_levels_per_type.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (kt_level_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_levels_per_type.iter().try_fold(0u32, |acc, x| acc.checked_add(u32::from(*x)).ok_or(ParseError::InvalidExpression))?.try_to_usize()?)?;
        let result = GetNamesValueListKTLevelNames { n_levels_per_type, kt_level_names };
        Ok((result, remaining))
    }
}
impl GetNamesValueListKTLevelNames {
    #[allow(dead_code)]
    fn serialize(&self, n_types: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u8::from(n_types));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, n_types: u8) {
        assert_eq!(self.n_levels_per_type.len(), usize::try_from(n_types).unwrap(), "`n_levels_per_type` has an incorrect length");
        bytes.extend_from_slice(&self.n_levels_per_type);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        assert_eq!(self.kt_level_names.len(), usize::try_from(self.n_levels_per_type.iter().fold(0u32, |acc, x| acc.checked_add(u32::from(*x)).unwrap())).unwrap(), "`kt_level_names` has an incorrect length");
        self.kt_level_names.serialize_into(bytes);
    }
}
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNamesValueList {
    pub keycodes_name: Option<xproto::Atom>,
    pub geometry_name: Option<xproto::Atom>,
    pub symbols_name: Option<xproto::Atom>,
    pub phys_symbols_name: Option<xproto::Atom>,
    pub types_name: Option<xproto::Atom>,
    pub compat_name: Option<xproto::Atom>,
    pub type_names: Option<Vec<xproto::Atom>>,
    pub kt_level_names: Option<GetNamesValueListKTLevelNames>,
    pub indicator_names: Option<Vec<xproto::Atom>>,
    pub virtual_mod_names: Option<Vec<xproto::Atom>>,
    pub groups: Option<Vec<xproto::Atom>>,
    pub key_names: Option<Vec<KeyName>>,
    pub key_aliases: Option<Vec<KeyAlias>>,
    pub radio_group_names: Option<Vec<xproto::Atom>>,
}
impl_debug_if_no_extra_traits!(GetNamesValueList, "GetNamesValueList");
impl GetNamesValueList {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(which);
        let mut outer_remaining = value;
        let keycodes_name = if switch_expr & u32::from(NameDetail::KEYCODES) != 0 {
            let remaining = outer_remaining;
            let (keycodes_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(keycodes_name)
        } else {
            None
        };
        let geometry_name = if switch_expr & u32::from(NameDetail::GEOMETRY) != 0 {
            let remaining = outer_remaining;
            let (geometry_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(geometry_name)
        } else {
            None
        };
        let symbols_name = if switch_expr & u32::from(NameDetail::SYMBOLS) != 0 {
            let remaining = outer_remaining;
            let (symbols_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(symbols_name)
        } else {
            None
        };
        let phys_symbols_name = if switch_expr & u32::from(NameDetail::PHYS_SYMBOLS) != 0 {
            let remaining = outer_remaining;
            let (phys_symbols_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(phys_symbols_name)
        } else {
            None
        };
        let types_name = if switch_expr & u32::from(NameDetail::TYPES) != 0 {
            let remaining = outer_remaining;
            let (types_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(types_name)
        } else {
            None
        };
        let compat_name = if switch_expr & u32::from(NameDetail::COMPAT) != 0 {
            let remaining = outer_remaining;
            let (compat_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(compat_name)
        } else {
            None
        };
        let type_names = if switch_expr & u32::from(NameDetail::KEY_TYPE_NAMES) != 0 {
            let remaining = outer_remaining;
            let (type_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_types.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(type_names)
        } else {
            None
        };
        let kt_level_names = if switch_expr & u32::from(NameDetail::KT_LEVEL_NAMES) != 0 {
            let (kt_level_names, new_remaining) = GetNamesValueListKTLevelNames::try_parse(outer_remaining, n_types)?;
            outer_remaining = new_remaining;
            Some(kt_level_names)
        } else {
            None
        };
        let indicator_names = if switch_expr & u32::from(NameDetail::INDICATOR_NAMES) != 0 {
            let remaining = outer_remaining;
            let (indicator_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(indicators).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(indicator_names)
        } else {
            None
        };
        let virtual_mod_names = if switch_expr & u32::from(NameDetail::VIRTUAL_MOD_NAMES) != 0 {
            let remaining = outer_remaining;
            let (virtual_mod_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(virtual_mods).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(virtual_mod_names)
        } else {
            None
        };
        let groups = if switch_expr & u32::from(NameDetail::GROUP_NAMES) != 0 {
            let remaining = outer_remaining;
            let (groups, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(group_names).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(groups)
        } else {
            None
        };
        let key_names = if switch_expr & u32::from(NameDetail::KEY_NAMES) != 0 {
            let remaining = outer_remaining;
            let (key_names, remaining) = crate::x11_utils::parse_list::<KeyName>(remaining, n_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(key_names)
        } else {
            None
        };
        let key_aliases = if switch_expr & u32::from(NameDetail::KEY_ALIASES) != 0 {
            let remaining = outer_remaining;
            let (key_aliases, remaining) = crate::x11_utils::parse_list::<KeyAlias>(remaining, n_key_aliases.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(key_aliases)
        } else {
            None
        };
        let radio_group_names = if switch_expr & u32::from(NameDetail::RG_NAMES) != 0 {
            let remaining = outer_remaining;
            let (radio_group_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_radio_groups.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(radio_group_names)
        } else {
            None
        };
        let result = GetNamesValueList { keycodes_name, geometry_name, symbols_name, phys_symbols_name, types_name, compat_name, type_names, kt_level_names, indicator_names, virtual_mod_names, groups, key_names, key_aliases, radio_group_names };
        Ok((result, outer_remaining))
    }
}
impl GetNamesValueList {
    #[allow(dead_code)]
    fn serialize(&self, which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(which), u8::from(n_types), u32::from(indicators), u16::from(virtual_mods), u8::from(group_names), u8::from(n_keys), u8::from(n_key_aliases), u8::from(n_radio_groups));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) {
        assert_eq!(self.switch_expr(), u32::from(which), "switch `value_list` has an inconsistent discriminant");
        if let Some(keycodes_name) = self.keycodes_name {
            keycodes_name.serialize_into(bytes);
        }
        if let Some(geometry_name) = self.geometry_name {
            geometry_name.serialize_into(bytes);
        }
        if let Some(symbols_name) = self.symbols_name {
            symbols_name.serialize_into(bytes);
        }
        if let Some(phys_symbols_name) = self.phys_symbols_name {
            phys_symbols_name.serialize_into(bytes);
        }
        if let Some(types_name) = self.types_name {
            types_name.serialize_into(bytes);
        }
        if let Some(compat_name) = self.compat_name {
            compat_name.serialize_into(bytes);
        }
        if let Some(ref type_names) = self.type_names {
            assert_eq!(type_names.len(), usize::try_from(n_types).unwrap(), "`type_names` has an incorrect length");
            type_names.serialize_into(bytes);
        }
        if let Some(ref kt_level_names) = self.kt_level_names {
            kt_level_names.serialize_into(bytes, u8::from(n_types));
        }
        if let Some(ref indicator_names) = self.indicator_names {
            assert_eq!(indicator_names.len(), usize::try_from(u32::from(indicators).count_ones()).unwrap(), "`indicator_names` has an incorrect length");
            indicator_names.serialize_into(bytes);
        }
        if let Some(ref virtual_mod_names) = self.virtual_mod_names {
            assert_eq!(virtual_mod_names.len(), usize::try_from(u32::from(virtual_mods).count_ones()).unwrap(), "`virtual_mod_names` has an incorrect length");
            virtual_mod_names.serialize_into(bytes);
        }
        if let Some(ref groups) = self.groups {
            assert_eq!(groups.len(), usize::try_from(u32::from(group_names).count_ones()).unwrap(), "`groups` has an incorrect length");
            groups.serialize_into(bytes);
        }
        if let Some(ref key_names) = self.key_names {
            assert_eq!(key_names.len(), usize::try_from(n_keys).unwrap(), "`key_names` has an incorrect length");
            key_names.serialize_into(bytes);
        }
        if let Some(ref key_aliases) = self.key_aliases {
            assert_eq!(key_aliases.len(), usize::try_from(n_key_aliases).unwrap(), "`key_aliases` has an incorrect length");
            key_aliases.serialize_into(bytes);
        }
        if let Some(ref radio_group_names) = self.radio_group_names {
            assert_eq!(radio_group_names.len(), usize::try_from(n_radio_groups).unwrap(), "`radio_group_names` has an incorrect length");
            radio_group_names.serialize_into(bytes);
        }
    }
}
impl GetNamesValueList {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.keycodes_name.is_some() {
            expr_value |= u32::from(NameDetail::KEYCODES);
        }
        if self.geometry_name.is_some() {
            expr_value |= u32::from(NameDetail::GEOMETRY);
        }
        if self.symbols_name.is_some() {
            expr_value |= u32::from(NameDetail::SYMBOLS);
        }
        if self.phys_symbols_name.is_some() {
            expr_value |= u32::from(NameDetail::PHYS_SYMBOLS);
        }
        if self.types_name.is_some() {
            expr_value |= u32::from(NameDetail::TYPES);
        }
        if self.compat_name.is_some() {
            expr_value |= u32::from(NameDetail::COMPAT);
        }
        if self.type_names.is_some() {
            expr_value |= u32::from(NameDetail::KEY_TYPE_NAMES);
        }
        if self.kt_level_names.is_some() {
            expr_value |= u32::from(NameDetail::KT_LEVEL_NAMES);
        }
        if self.indicator_names.is_some() {
            expr_value |= u32::from(NameDetail::INDICATOR_NAMES);
        }
        if self.virtual_mod_names.is_some() {
            expr_value |= u32::from(NameDetail::VIRTUAL_MOD_NAMES);
        }
        if self.groups.is_some() {
            expr_value |= u32::from(NameDetail::GROUP_NAMES);
        }
        if self.key_names.is_some() {
            expr_value |= u32::from(NameDetail::KEY_NAMES);
        }
        if self.key_aliases.is_some() {
            expr_value |= u32::from(NameDetail::KEY_ALIASES);
        }
        if self.radio_group_names.is_some() {
            expr_value |= u32::from(NameDetail::RG_NAMES);
        }
        expr_value
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetNamesReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub min_key_code: xproto::Keycode,
    pub max_key_code: xproto::Keycode,
    pub n_types: u8,
    pub group_names: SetOfGroup,
    pub virtual_mods: VMod,
    pub first_key: xproto::Keycode,
    pub n_keys: u8,
    pub indicators: u32,
    pub n_radio_groups: u8,
    pub n_key_aliases: u8,
    pub n_kt_levels: u16,
    pub value_list: GetNamesValueList,
}
impl_debug_if_no_extra_traits!(GetNamesReply, "GetNamesReply");
impl TryParse for GetNamesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let (min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (group_names, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let (first_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_keys, remaining) = u8::try_parse(remaining)?;
        let (indicators, remaining) = u32::try_parse(remaining)?;
        let (n_radio_groups, remaining) = u8::try_parse(remaining)?;
        let (n_key_aliases, remaining) = u8::try_parse(remaining)?;
        let (n_kt_levels, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (value_list, remaining) = GetNamesValueList::try_parse(remaining, u32::from(which), u8::from(n_types), u32::from(indicators), u16::from(virtual_mods), u8::from(group_names), u8::from(n_keys), u8::from(n_key_aliases), u8::from(n_radio_groups))?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let group_names = group_names.into();
        let virtual_mods = virtual_mods.into();
        let result = GetNamesReply { device_id, sequence, length, min_key_code, max_key_code, n_types, group_names, virtual_mods, first_key, n_keys, indicators, n_radio_groups, n_key_aliases, n_kt_levels, value_list };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetNamesReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let which: u32 = self.value_list.switch_expr();
        which.serialize_into(bytes);
        self.min_key_code.serialize_into(bytes);
        self.max_key_code.serialize_into(bytes);
        self.n_types.serialize_into(bytes);
        u8::from(self.group_names).serialize_into(bytes);
        u16::from(self.virtual_mods).serialize_into(bytes);
        self.first_key.serialize_into(bytes);
        self.n_keys.serialize_into(bytes);
        self.indicators.serialize_into(bytes);
        self.n_radio_groups.serialize_into(bytes);
        self.n_key_aliases.serialize_into(bytes);
        self.n_kt_levels.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.value_list.serialize_into(bytes, u32::from(which), u8::from(self.n_types), u32::from(self.indicators), u16::from(self.virtual_mods), u8::from(self.group_names), u8::from(self.n_keys), u8::from(self.n_key_aliases), u8::from(self.n_radio_groups));
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetNamesAuxKTLevelNames {
    pub n_levels_per_type: Vec<u8>,
    pub kt_level_names: Vec<xproto::Atom>,
}
impl_debug_if_no_extra_traits!(SetNamesAuxKTLevelNames, "SetNamesAuxKTLevelNames");
impl SetNamesAuxKTLevelNames {
    pub fn try_parse(remaining: &[u8], n_types: u8) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (n_levels_per_type, remaining) = crate::x11_utils::parse_u8_list(remaining, n_types.try_to_usize()?)?;
        let n_levels_per_type = n_levels_per_type.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (kt_level_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_levels_per_type.iter().try_fold(0u32, |acc, x| acc.checked_add(u32::from(*x)).ok_or(ParseError::InvalidExpression))?.try_to_usize()?)?;
        let result = SetNamesAuxKTLevelNames { n_levels_per_type, kt_level_names };
        Ok((result, remaining))
    }
}
impl SetNamesAuxKTLevelNames {
    #[allow(dead_code)]
    fn serialize(&self, n_types: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u8::from(n_types));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, n_types: u8) {
        assert_eq!(self.n_levels_per_type.len(), usize::try_from(n_types).unwrap(), "`n_levels_per_type` has an incorrect length");
        bytes.extend_from_slice(&self.n_levels_per_type);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        assert_eq!(self.kt_level_names.len(), usize::try_from(self.n_levels_per_type.iter().fold(0u32, |acc, x| acc.checked_add(u32::from(*x)).unwrap())).unwrap(), "`kt_level_names` has an incorrect length");
        self.kt_level_names.serialize_into(bytes);
    }
}
/// Auxiliary and optional information for the `set_names` function
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetNamesAux {
    pub keycodes_name: Option<xproto::Atom>,
    pub geometry_name: Option<xproto::Atom>,
    pub symbols_name: Option<xproto::Atom>,
    pub phys_symbols_name: Option<xproto::Atom>,
    pub types_name: Option<xproto::Atom>,
    pub compat_name: Option<xproto::Atom>,
    pub type_names: Option<Vec<xproto::Atom>>,
    pub kt_level_names: Option<SetNamesAuxKTLevelNames>,
    pub indicator_names: Option<Vec<xproto::Atom>>,
    pub virtual_mod_names: Option<Vec<xproto::Atom>>,
    pub groups: Option<Vec<xproto::Atom>>,
    pub key_names: Option<Vec<KeyName>>,
    pub key_aliases: Option<Vec<KeyAlias>>,
    pub radio_group_names: Option<Vec<xproto::Atom>>,
}
impl_debug_if_no_extra_traits!(SetNamesAux, "SetNamesAux");
impl SetNamesAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(which);
        let mut outer_remaining = value;
        let keycodes_name = if switch_expr & u32::from(NameDetail::KEYCODES) != 0 {
            let remaining = outer_remaining;
            let (keycodes_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(keycodes_name)
        } else {
            None
        };
        let geometry_name = if switch_expr & u32::from(NameDetail::GEOMETRY) != 0 {
            let remaining = outer_remaining;
            let (geometry_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(geometry_name)
        } else {
            None
        };
        let symbols_name = if switch_expr & u32::from(NameDetail::SYMBOLS) != 0 {
            let remaining = outer_remaining;
            let (symbols_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(symbols_name)
        } else {
            None
        };
        let phys_symbols_name = if switch_expr & u32::from(NameDetail::PHYS_SYMBOLS) != 0 {
            let remaining = outer_remaining;
            let (phys_symbols_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(phys_symbols_name)
        } else {
            None
        };
        let types_name = if switch_expr & u32::from(NameDetail::TYPES) != 0 {
            let remaining = outer_remaining;
            let (types_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(types_name)
        } else {
            None
        };
        let compat_name = if switch_expr & u32::from(NameDetail::COMPAT) != 0 {
            let remaining = outer_remaining;
            let (compat_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(compat_name)
        } else {
            None
        };
        let type_names = if switch_expr & u32::from(NameDetail::KEY_TYPE_NAMES) != 0 {
            let remaining = outer_remaining;
            let (type_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_types.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(type_names)
        } else {
            None
        };
        let kt_level_names = if switch_expr & u32::from(NameDetail::KT_LEVEL_NAMES) != 0 {
            let (kt_level_names, new_remaining) = SetNamesAuxKTLevelNames::try_parse(outer_remaining, n_types)?;
            outer_remaining = new_remaining;
            Some(kt_level_names)
        } else {
            None
        };
        let indicator_names = if switch_expr & u32::from(NameDetail::INDICATOR_NAMES) != 0 {
            let remaining = outer_remaining;
            let (indicator_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(indicators).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(indicator_names)
        } else {
            None
        };
        let virtual_mod_names = if switch_expr & u32::from(NameDetail::VIRTUAL_MOD_NAMES) != 0 {
            let remaining = outer_remaining;
            let (virtual_mod_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(virtual_mods).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(virtual_mod_names)
        } else {
            None
        };
        let groups = if switch_expr & u32::from(NameDetail::GROUP_NAMES) != 0 {
            let remaining = outer_remaining;
            let (groups, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(group_names).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(groups)
        } else {
            None
        };
        let key_names = if switch_expr & u32::from(NameDetail::KEY_NAMES) != 0 {
            let remaining = outer_remaining;
            let (key_names, remaining) = crate::x11_utils::parse_list::<KeyName>(remaining, n_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(key_names)
        } else {
            None
        };
        let key_aliases = if switch_expr & u32::from(NameDetail::KEY_ALIASES) != 0 {
            let remaining = outer_remaining;
            let (key_aliases, remaining) = crate::x11_utils::parse_list::<KeyAlias>(remaining, n_key_aliases.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(key_aliases)
        } else {
            None
        };
        let radio_group_names = if switch_expr & u32::from(NameDetail::RG_NAMES) != 0 {
            let remaining = outer_remaining;
            let (radio_group_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_radio_groups.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(radio_group_names)
        } else {
            None
        };
        let result = SetNamesAux { keycodes_name, geometry_name, symbols_name, phys_symbols_name, types_name, compat_name, type_names, kt_level_names, indicator_names, virtual_mod_names, groups, key_names, key_aliases, radio_group_names };
        Ok((result, outer_remaining))
    }
}
impl SetNamesAux {
    #[allow(dead_code)]
    fn serialize(&self, which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(which), u8::from(n_types), u32::from(indicators), u16::from(virtual_mods), u8::from(group_names), u8::from(n_keys), u8::from(n_key_aliases), u8::from(n_radio_groups));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) {
        assert_eq!(self.switch_expr(), u32::from(which), "switch `values` has an inconsistent discriminant");
        if let Some(keycodes_name) = self.keycodes_name {
            keycodes_name.serialize_into(bytes);
        }
        if let Some(geometry_name) = self.geometry_name {
            geometry_name.serialize_into(bytes);
        }
        if let Some(symbols_name) = self.symbols_name {
            symbols_name.serialize_into(bytes);
        }
        if let Some(phys_symbols_name) = self.phys_symbols_name {
            phys_symbols_name.serialize_into(bytes);
        }
        if let Some(types_name) = self.types_name {
            types_name.serialize_into(bytes);
        }
        if let Some(compat_name) = self.compat_name {
            compat_name.serialize_into(bytes);
        }
        if let Some(ref type_names) = self.type_names {
            assert_eq!(type_names.len(), usize::try_from(n_types).unwrap(), "`type_names` has an incorrect length");
            type_names.serialize_into(bytes);
        }
        if let Some(ref kt_level_names) = self.kt_level_names {
            kt_level_names.serialize_into(bytes, u8::from(n_types));
        }
        if let Some(ref indicator_names) = self.indicator_names {
            assert_eq!(indicator_names.len(), usize::try_from(u32::from(indicators).count_ones()).unwrap(), "`indicator_names` has an incorrect length");
            indicator_names.serialize_into(bytes);
        }
        if let Some(ref virtual_mod_names) = self.virtual_mod_names {
            assert_eq!(virtual_mod_names.len(), usize::try_from(u32::from(virtual_mods).count_ones()).unwrap(), "`virtual_mod_names` has an incorrect length");
            virtual_mod_names.serialize_into(bytes);
        }
        if let Some(ref groups) = self.groups {
            assert_eq!(groups.len(), usize::try_from(u32::from(group_names).count_ones()).unwrap(), "`groups` has an incorrect length");
            groups.serialize_into(bytes);
        }
        if let Some(ref key_names) = self.key_names {
            assert_eq!(key_names.len(), usize::try_from(n_keys).unwrap(), "`key_names` has an incorrect length");
            key_names.serialize_into(bytes);
        }
        if let Some(ref key_aliases) = self.key_aliases {
            assert_eq!(key_aliases.len(), usize::try_from(n_key_aliases).unwrap(), "`key_aliases` has an incorrect length");
            key_aliases.serialize_into(bytes);
        }
        if let Some(ref radio_group_names) = self.radio_group_names {
            assert_eq!(radio_group_names.len(), usize::try_from(n_radio_groups).unwrap(), "`radio_group_names` has an incorrect length");
            radio_group_names.serialize_into(bytes);
        }
    }
}
impl SetNamesAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.keycodes_name.is_some() {
            expr_value |= u32::from(NameDetail::KEYCODES);
        }
        if self.geometry_name.is_some() {
            expr_value |= u32::from(NameDetail::GEOMETRY);
        }
        if self.symbols_name.is_some() {
            expr_value |= u32::from(NameDetail::SYMBOLS);
        }
        if self.phys_symbols_name.is_some() {
            expr_value |= u32::from(NameDetail::PHYS_SYMBOLS);
        }
        if self.types_name.is_some() {
            expr_value |= u32::from(NameDetail::TYPES);
        }
        if self.compat_name.is_some() {
            expr_value |= u32::from(NameDetail::COMPAT);
        }
        if self.type_names.is_some() {
            expr_value |= u32::from(NameDetail::KEY_TYPE_NAMES);
        }
        if self.kt_level_names.is_some() {
            expr_value |= u32::from(NameDetail::KT_LEVEL_NAMES);
        }
        if self.indicator_names.is_some() {
            expr_value |= u32::from(NameDetail::INDICATOR_NAMES);
        }
        if self.virtual_mod_names.is_some() {
            expr_value |= u32::from(NameDetail::VIRTUAL_MOD_NAMES);
        }
        if self.groups.is_some() {
            expr_value |= u32::from(NameDetail::GROUP_NAMES);
        }
        if self.key_names.is_some() {
            expr_value |= u32::from(NameDetail::KEY_NAMES);
        }
        if self.key_aliases.is_some() {
            expr_value |= u32::from(NameDetail::KEY_ALIASES);
        }
        if self.radio_group_names.is_some() {
            expr_value |= u32::from(NameDetail::RG_NAMES);
        }
        expr_value
    }
}
impl SetNamesAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `keycodes_name` field of this structure.
    #[must_use]
    pub fn keycodes_name<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.keycodes_name = value.into();
        self
    }
    /// Set the `geometry_name` field of this structure.
    #[must_use]
    pub fn geometry_name<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.geometry_name = value.into();
        self
    }
    /// Set the `symbols_name` field of this structure.
    #[must_use]
    pub fn symbols_name<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.symbols_name = value.into();
        self
    }
    /// Set the `phys_symbols_name` field of this structure.
    #[must_use]
    pub fn phys_symbols_name<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.phys_symbols_name = value.into();
        self
    }
    /// Set the `types_name` field of this structure.
    #[must_use]
    pub fn types_name<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.types_name = value.into();
        self
    }
    /// Set the `compat_name` field of this structure.
    #[must_use]
    pub fn compat_name<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.compat_name = value.into();
        self
    }
    /// Set the `type_names` field of this structure.
    #[must_use]
    pub fn type_names<I>(mut self, value: I) -> Self where I: Into<Option<Vec<xproto::Atom>>> {
        self.type_names = value.into();
        self
    }
    /// Set the `kt_level_names` field of this structure.
    #[must_use]
    pub fn kt_level_names<I>(mut self, value: I) -> Self where I: Into<Option<SetNamesAuxKTLevelNames>> {
        self.kt_level_names = value.into();
        self
    }
    /// Set the `indicator_names` field of this structure.
    #[must_use]
    pub fn indicator_names<I>(mut self, value: I) -> Self where I: Into<Option<Vec<xproto::Atom>>> {
        self.indicator_names = value.into();
        self
    }
    /// Set the `virtual_mod_names` field of this structure.
    #[must_use]
    pub fn virtual_mod_names<I>(mut self, value: I) -> Self where I: Into<Option<Vec<xproto::Atom>>> {
        self.virtual_mod_names = value.into();
        self
    }
    /// Set the `groups` field of this structure.
    #[must_use]
    pub fn groups<I>(mut self, value: I) -> Self where I: Into<Option<Vec<xproto::Atom>>> {
        self.groups = value.into();
        self
    }
    /// Set the `key_names` field of this structure.
    #[must_use]
    pub fn key_names<I>(mut self, value: I) -> Self where I: Into<Option<Vec<KeyName>>> {
        self.key_names = value.into();
        self
    }
    /// Set the `key_aliases` field of this structure.
    #[must_use]
    pub fn key_aliases<I>(mut self, value: I) -> Self where I: Into<Option<Vec<KeyAlias>>> {
        self.key_aliases = value.into();
        self
    }
    /// Set the `radio_group_names` field of this structure.
    #[must_use]
    pub fn radio_group_names<I>(mut self, value: I) -> Self where I: Into<Option<Vec<xproto::Atom>>> {
        self.radio_group_names = value.into();
        self
    }
}

/// Opcode for the SetNames request
pub const SET_NAMES_REQUEST: u8 = 18;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetNamesRequest<'input> {
    pub device_spec: DeviceSpec,
    pub virtual_mods: VMod,
    pub first_type: u8,
    pub n_types: u8,
    pub first_kt_levelt: u8,
    pub n_kt_levels: u8,
    pub indicators: u32,
    pub group_names: SetOfGroup,
    pub n_radio_groups: u8,
    pub first_key: xproto::Keycode,
    pub n_keys: u8,
    pub n_key_aliases: u8,
    pub total_kt_level_names: u16,
    pub values: Cow<'input, SetNamesAux>,
}
impl_debug_if_no_extra_traits!(SetNamesRequest<'_>, "SetNamesRequest");
impl<'input> SetNamesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let virtual_mods_bytes = u16::from(self.virtual_mods).serialize();
        let which: u32 = self.values.switch_expr();
        let which_bytes = which.serialize();
        let first_type_bytes = self.first_type.serialize();
        let n_types_bytes = self.n_types.serialize();
        let first_kt_levelt_bytes = self.first_kt_levelt.serialize();
        let n_kt_levels_bytes = self.n_kt_levels.serialize();
        let indicators_bytes = self.indicators.serialize();
        let group_names_bytes = u8::from(self.group_names).serialize();
        let n_radio_groups_bytes = self.n_radio_groups.serialize();
        let first_key_bytes = self.first_key.serialize();
        let n_keys_bytes = self.n_keys.serialize();
        let n_key_aliases_bytes = self.n_key_aliases.serialize();
        let total_kt_level_names_bytes = self.total_kt_level_names.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_NAMES_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            virtual_mods_bytes[0],
            virtual_mods_bytes[1],
            which_bytes[0],
            which_bytes[1],
            which_bytes[2],
            which_bytes[3],
            first_type_bytes[0],
            n_types_bytes[0],
            first_kt_levelt_bytes[0],
            n_kt_levels_bytes[0],
            indicators_bytes[0],
            indicators_bytes[1],
            indicators_bytes[2],
            indicators_bytes[3],
            group_names_bytes[0],
            n_radio_groups_bytes[0],
            first_key_bytes[0],
            n_keys_bytes[0],
            n_key_aliases_bytes[0],
            0,
            total_kt_level_names_bytes[0],
            total_kt_level_names_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let values_bytes = self.values.serialize(u32::from(which), u8::from(self.n_types), u32::from(self.indicators), u16::from(self.virtual_mods), u8::from(self.group_names), u8::from(self.n_keys), u8::from(self.n_key_aliases), u8::from(self.n_radio_groups));
        let length_so_far = length_so_far + values_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), values_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_NAMES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let virtual_mods = virtual_mods.into();
        let (which, remaining) = u32::try_parse(remaining)?;
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (first_kt_levelt, remaining) = u8::try_parse(remaining)?;
        let (n_kt_levels, remaining) = u8::try_parse(remaining)?;
        let (indicators, remaining) = u32::try_parse(remaining)?;
        let (group_names, remaining) = u8::try_parse(remaining)?;
        let group_names = group_names.into();
        let (n_radio_groups, remaining) = u8::try_parse(remaining)?;
        let (first_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_keys, remaining) = u8::try_parse(remaining)?;
        let (n_key_aliases, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (total_kt_level_names, remaining) = u16::try_parse(remaining)?;
        let (values, remaining) = SetNamesAux::try_parse(remaining, u32::from(which), u8::from(n_types), u32::from(indicators), u16::from(virtual_mods), u8::from(group_names), u8::from(n_keys), u8::from(n_key_aliases), u8::from(n_radio_groups))?;
        let _ = remaining;
        Ok(SetNamesRequest {
            device_spec,
            virtual_mods,
            first_type,
            n_types,
            first_kt_levelt,
            n_kt_levels,
            indicators,
            group_names,
            n_radio_groups,
            first_key,
            n_keys,
            n_key_aliases,
            total_kt_level_names,
            values: Cow::Owned(values),
        })
    }
    /// Clone all borrowed data in this SetNamesRequest.
    pub fn into_owned(self) -> SetNamesRequest<'static> {
        SetNamesRequest {
            device_spec: self.device_spec,
            virtual_mods: self.virtual_mods,
            first_type: self.first_type,
            n_types: self.n_types,
            first_kt_levelt: self.first_kt_levelt,
            n_kt_levels: self.n_kt_levels,
            indicators: self.indicators,
            group_names: self.group_names,
            n_radio_groups: self.n_radio_groups,
            first_key: self.first_key,
            n_keys: self.n_keys,
            n_key_aliases: self.n_key_aliases,
            total_kt_level_names: self.total_kt_level_names,
            values: Cow::Owned(self.values.into_owned()),
        }
    }
}
impl<'input> Request for SetNamesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetNamesRequest<'input> {
}

/// Opcode for the PerClientFlags request
pub const PER_CLIENT_FLAGS_REQUEST: u8 = 21;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PerClientFlagsRequest {
    pub device_spec: DeviceSpec,
    pub change: PerClientFlag,
    pub value: PerClientFlag,
    pub ctrls_to_change: BoolCtrl,
    pub auto_ctrls: BoolCtrl,
    pub auto_ctrls_values: BoolCtrl,
}
impl_debug_if_no_extra_traits!(PerClientFlagsRequest, "PerClientFlagsRequest");
impl PerClientFlagsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let change_bytes = u32::from(self.change).serialize();
        let value_bytes = u32::from(self.value).serialize();
        let ctrls_to_change_bytes = u32::from(self.ctrls_to_change).serialize();
        let auto_ctrls_bytes = u32::from(self.auto_ctrls).serialize();
        let auto_ctrls_values_bytes = u32::from(self.auto_ctrls_values).serialize();
        let mut request0 = vec![
            major_opcode,
            PER_CLIENT_FLAGS_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            0,
            0,
            change_bytes[0],
            change_bytes[1],
            change_bytes[2],
            change_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
            ctrls_to_change_bytes[0],
            ctrls_to_change_bytes[1],
            ctrls_to_change_bytes[2],
            ctrls_to_change_bytes[3],
            auto_ctrls_bytes[0],
            auto_ctrls_bytes[1],
            auto_ctrls_bytes[2],
            auto_ctrls_bytes[3],
            auto_ctrls_values_bytes[0],
            auto_ctrls_values_bytes[1],
            auto_ctrls_values_bytes[2],
            auto_ctrls_values_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != PER_CLIENT_FLAGS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (change, remaining) = u32::try_parse(remaining)?;
        let change = change.into();
        let (value, remaining) = u32::try_parse(remaining)?;
        let value = value.into();
        let (ctrls_to_change, remaining) = u32::try_parse(remaining)?;
        let ctrls_to_change = ctrls_to_change.into();
        let (auto_ctrls, remaining) = u32::try_parse(remaining)?;
        let auto_ctrls = auto_ctrls.into();
        let (auto_ctrls_values, remaining) = u32::try_parse(remaining)?;
        let auto_ctrls_values = auto_ctrls_values.into();
        let _ = remaining;
        Ok(PerClientFlagsRequest {
            device_spec,
            change,
            value,
            ctrls_to_change,
            auto_ctrls,
            auto_ctrls_values,
        })
    }
}
impl Request for PerClientFlagsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for PerClientFlagsRequest {
    type Reply = PerClientFlagsReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PerClientFlagsReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub supported: PerClientFlag,
    pub value: PerClientFlag,
    pub auto_ctrls: BoolCtrl,
    pub auto_ctrls_values: BoolCtrl,
}
impl_debug_if_no_extra_traits!(PerClientFlagsReply, "PerClientFlagsReply");
impl TryParse for PerClientFlagsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (supported, remaining) = u32::try_parse(remaining)?;
        let (value, remaining) = u32::try_parse(remaining)?;
        let (auto_ctrls, remaining) = u32::try_parse(remaining)?;
        let (auto_ctrls_values, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let supported = supported.into();
        let value = value.into();
        let auto_ctrls = auto_ctrls.into();
        let auto_ctrls_values = auto_ctrls_values.into();
        let result = PerClientFlagsReply { device_id, sequence, length, supported, value, auto_ctrls, auto_ctrls_values };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for PerClientFlagsReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let device_id_bytes = self.device_id.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let supported_bytes = u32::from(self.supported).serialize();
        let value_bytes = u32::from(self.value).serialize();
        let auto_ctrls_bytes = u32::from(self.auto_ctrls).serialize();
        let auto_ctrls_values_bytes = u32::from(self.auto_ctrls_values).serialize();
        [
            response_type_bytes[0],
            device_id_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            supported_bytes[0],
            supported_bytes[1],
            supported_bytes[2],
            supported_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
            auto_ctrls_bytes[0],
            auto_ctrls_bytes[1],
            auto_ctrls_bytes[2],
            auto_ctrls_bytes[3],
            auto_ctrls_values_bytes[0],
            auto_ctrls_values_bytes[1],
            auto_ctrls_values_bytes[2],
            auto_ctrls_values_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        u32::from(self.supported).serialize_into(bytes);
        u32::from(self.value).serialize_into(bytes);
        u32::from(self.auto_ctrls).serialize_into(bytes);
        u32::from(self.auto_ctrls_values).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
    }
}

/// Opcode for the ListComponents request
pub const LIST_COMPONENTS_REQUEST: u8 = 22;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListComponentsRequest {
    pub device_spec: DeviceSpec,
    pub max_names: u16,
}
impl_debug_if_no_extra_traits!(ListComponentsRequest, "ListComponentsRequest");
impl ListComponentsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let max_names_bytes = self.max_names.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_COMPONENTS_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            max_names_bytes[0],
            max_names_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != LIST_COMPONENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (max_names, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(ListComponentsRequest {
            device_spec,
            max_names,
        })
    }
}
impl Request for ListComponentsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListComponentsRequest {
    type Reply = ListComponentsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListComponentsReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub extra: u16,
    pub keymaps: Vec<Listing>,
    pub keycodes: Vec<Listing>,
    pub types: Vec<Listing>,
    pub compat_maps: Vec<Listing>,
    pub symbols: Vec<Listing>,
    pub geometries: Vec<Listing>,
}
impl_debug_if_no_extra_traits!(ListComponentsReply, "ListComponentsReply");
impl TryParse for ListComponentsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (n_keymaps, remaining) = u16::try_parse(remaining)?;
        let (n_keycodes, remaining) = u16::try_parse(remaining)?;
        let (n_types, remaining) = u16::try_parse(remaining)?;
        let (n_compat_maps, remaining) = u16::try_parse(remaining)?;
        let (n_symbols, remaining) = u16::try_parse(remaining)?;
        let (n_geometries, remaining) = u16::try_parse(remaining)?;
        let (extra, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(10..).ok_or(ParseError::InsufficientData)?;
        let (keymaps, remaining) = crate::x11_utils::parse_list::<Listing>(remaining, n_keymaps.try_to_usize()?)?;
        let (keycodes, remaining) = crate::x11_utils::parse_list::<Listing>(remaining, n_keycodes.try_to_usize()?)?;
        let (types, remaining) = crate::x11_utils::parse_list::<Listing>(remaining, n_types.try_to_usize()?)?;
        let (compat_maps, remaining) = crate::x11_utils::parse_list::<Listing>(remaining, n_compat_maps.try_to_usize()?)?;
        let (symbols, remaining) = crate::x11_utils::parse_list::<Listing>(remaining, n_symbols.try_to_usize()?)?;
        let (geometries, remaining) = crate::x11_utils::parse_list::<Listing>(remaining, n_geometries.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListComponentsReply { device_id, sequence, length, extra, keymaps, keycodes, types, compat_maps, symbols, geometries };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListComponentsReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let n_keymaps = u16::try_from(self.keymaps.len()).expect("`keymaps` has too many elements");
        n_keymaps.serialize_into(bytes);
        let n_keycodes = u16::try_from(self.keycodes.len()).expect("`keycodes` has too many elements");
        n_keycodes.serialize_into(bytes);
        let n_types = u16::try_from(self.types.len()).expect("`types` has too many elements");
        n_types.serialize_into(bytes);
        let n_compat_maps = u16::try_from(self.compat_maps.len()).expect("`compat_maps` has too many elements");
        n_compat_maps.serialize_into(bytes);
        let n_symbols = u16::try_from(self.symbols.len()).expect("`symbols` has too many elements");
        n_symbols.serialize_into(bytes);
        let n_geometries = u16::try_from(self.geometries.len()).expect("`geometries` has too many elements");
        n_geometries.serialize_into(bytes);
        self.extra.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 10]);
        self.keymaps.serialize_into(bytes);
        self.keycodes.serialize_into(bytes);
        self.types.serialize_into(bytes);
        self.compat_maps.serialize_into(bytes);
        self.symbols.serialize_into(bytes);
        self.geometries.serialize_into(bytes);
    }
}
impl ListComponentsReply {
    /// Get the value of the `nKeymaps` field.
    ///
    /// The `nKeymaps` field is used as the length field of the `keymaps` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_keymaps(&self) -> u16 {
        self.keymaps.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nKeycodes` field.
    ///
    /// The `nKeycodes` field is used as the length field of the `keycodes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_keycodes(&self) -> u16 {
        self.keycodes.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nTypes` field.
    ///
    /// The `nTypes` field is used as the length field of the `types` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_types(&self) -> u16 {
        self.types.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nCompatMaps` field.
    ///
    /// The `nCompatMaps` field is used as the length field of the `compatMaps` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_compat_maps(&self) -> u16 {
        self.compat_maps.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nSymbols` field.
    ///
    /// The `nSymbols` field is used as the length field of the `symbols` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_symbols(&self) -> u16 {
        self.symbols.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nGeometries` field.
    ///
    /// The `nGeometries` field is used as the length field of the `geometries` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_geometries(&self) -> u16 {
        self.geometries.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetKbdByName request
pub const GET_KBD_BY_NAME_REQUEST: u8 = 23;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRequest {
    pub device_spec: DeviceSpec,
    pub need: GBNDetail,
    pub want: GBNDetail,
    pub load: bool,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRequest, "GetKbdByNameRequest");
impl GetKbdByNameRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let need_bytes = u16::from(self.need).serialize();
        let want_bytes = u16::from(self.want).serialize();
        let load_bytes = self.load.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_KBD_BY_NAME_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            need_bytes[0],
            need_bytes[1],
            want_bytes[0],
            want_bytes[1],
            load_bytes[0],
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_KBD_BY_NAME_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (need, remaining) = u16::try_parse(remaining)?;
        let need = need.into();
        let (want, remaining) = u16::try_parse(remaining)?;
        let want = want.into();
        let (load, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetKbdByNameRequest {
            device_spec,
            need,
            want,
            load,
        })
    }
}
impl Request for GetKbdByNameRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetKbdByNameRequest {
    type Reply = GetKbdByNameReply;
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesTypesMapKeyActions {
    pub acts_rtrn_count: Vec<u8>,
    pub acts_rtrn_acts: Vec<Action>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesTypesMapKeyActions, "GetKbdByNameRepliesTypesMapKeyActions");
impl GetKbdByNameRepliesTypesMapKeyActions {
    pub fn try_parse(remaining: &[u8], n_key_actions: u8, total_actions: u16) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (acts_rtrn_count, remaining) = crate::x11_utils::parse_u8_list(remaining, n_key_actions.try_to_usize()?)?;
        let acts_rtrn_count = acts_rtrn_count.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (acts_rtrn_acts, remaining) = crate::x11_utils::parse_list::<Action>(remaining, total_actions.try_to_usize()?)?;
        let result = GetKbdByNameRepliesTypesMapKeyActions { acts_rtrn_count, acts_rtrn_acts };
        Ok((result, remaining))
    }
}
impl GetKbdByNameRepliesTypesMapKeyActions {
    #[allow(dead_code)]
    fn serialize(&self, n_key_actions: u8, total_actions: u16) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u8::from(n_key_actions), u16::from(total_actions));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, n_key_actions: u8, total_actions: u16) {
        assert_eq!(self.acts_rtrn_count.len(), usize::try_from(n_key_actions).unwrap(), "`acts_rtrn_count` has an incorrect length");
        bytes.extend_from_slice(&self.acts_rtrn_count);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        assert_eq!(self.acts_rtrn_acts.len(), usize::try_from(total_actions).unwrap(), "`acts_rtrn_acts` has an incorrect length");
        self.acts_rtrn_acts.serialize_into(bytes);
    }
}
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesTypesMap {
    pub types_rtrn: Option<Vec<KeyType>>,
    pub syms_rtrn: Option<Vec<KeySymMap>>,
    pub key_actions: Option<GetKbdByNameRepliesTypesMapKeyActions>,
    pub behaviors_rtrn: Option<Vec<SetBehavior>>,
    pub vmods_rtrn: Option<Vec<xproto::ModMask>>,
    pub explicit_rtrn: Option<Vec<SetExplicit>>,
    pub modmap_rtrn: Option<Vec<KeyModMap>>,
    pub vmodmap_rtrn: Option<Vec<KeyVModMap>>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesTypesMap, "GetKbdByNameRepliesTypesMap");
impl GetKbdByNameRepliesTypesMap {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u16::from(present);
        let mut outer_remaining = value;
        let types_rtrn = if switch_expr & u16::from(MapPart::KEY_TYPES) != 0 {
            let remaining = outer_remaining;
            let (types_rtrn, remaining) = crate::x11_utils::parse_list::<KeyType>(remaining, n_types.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(types_rtrn)
        } else {
            None
        };
        let syms_rtrn = if switch_expr & u16::from(MapPart::KEY_SYMS) != 0 {
            let remaining = outer_remaining;
            let (syms_rtrn, remaining) = crate::x11_utils::parse_list::<KeySymMap>(remaining, n_key_syms.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(syms_rtrn)
        } else {
            None
        };
        let key_actions = if switch_expr & u16::from(MapPart::KEY_ACTIONS) != 0 {
            let (key_actions, new_remaining) = GetKbdByNameRepliesTypesMapKeyActions::try_parse(outer_remaining, n_key_actions, total_actions)?;
            outer_remaining = new_remaining;
            Some(key_actions)
        } else {
            None
        };
        let behaviors_rtrn = if switch_expr & u16::from(MapPart::KEY_BEHAVIORS) != 0 {
            let remaining = outer_remaining;
            let (behaviors_rtrn, remaining) = crate::x11_utils::parse_list::<SetBehavior>(remaining, total_key_behaviors.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(behaviors_rtrn)
        } else {
            None
        };
        let vmods_rtrn = if switch_expr & u16::from(MapPart::VIRTUAL_MODS) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let mut remaining = remaining;
            let list_length = u32::from(virtual_mods).count_ones().try_to_usize()?;
            let mut vmods_rtrn = Vec::with_capacity(list_length);
            for _ in 0..list_length {
                let (v, new_remaining) = u8::try_parse(remaining)?;
                let v = v.into();
                remaining = new_remaining;
                vmods_rtrn.push(v);
            }
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(vmods_rtrn)
        } else {
            None
        };
        let explicit_rtrn = if switch_expr & u16::from(MapPart::EXPLICIT_COMPONENTS) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let (explicit_rtrn, remaining) = crate::x11_utils::parse_list::<SetExplicit>(remaining, total_key_explicit.try_to_usize()?)?;
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(explicit_rtrn)
        } else {
            None
        };
        let modmap_rtrn = if switch_expr & u16::from(MapPart::MODIFIER_MAP) != 0 {
            let remaining = outer_remaining;
            let value = remaining;
            let (modmap_rtrn, remaining) = crate::x11_utils::parse_list::<KeyModMap>(remaining, total_mod_map_keys.try_to_usize()?)?;
            // Align offset to multiple of 4
            let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
            let misalignment = (4 - (offset % 4)) % 4;
            let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
            outer_remaining = remaining;
            Some(modmap_rtrn)
        } else {
            None
        };
        let vmodmap_rtrn = if switch_expr & u16::from(MapPart::VIRTUAL_MOD_MAP) != 0 {
            let remaining = outer_remaining;
            let (vmodmap_rtrn, remaining) = crate::x11_utils::parse_list::<KeyVModMap>(remaining, total_v_mod_map_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(vmodmap_rtrn)
        } else {
            None
        };
        let result = GetKbdByNameRepliesTypesMap { types_rtrn, syms_rtrn, key_actions, behaviors_rtrn, vmods_rtrn, explicit_rtrn, modmap_rtrn, vmodmap_rtrn };
        Ok((result, outer_remaining))
    }
}
impl GetKbdByNameRepliesTypesMap {
    #[allow(dead_code)]
    fn serialize(&self, present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u16::from(present), u8::from(n_types), u8::from(n_key_syms), u8::from(n_key_actions), u16::from(total_actions), u8::from(total_key_behaviors), u16::from(virtual_mods), u8::from(total_key_explicit), u8::from(total_mod_map_keys), u8::from(total_v_mod_map_keys));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, present: u16, n_types: u8, n_key_syms: u8, n_key_actions: u8, total_actions: u16, total_key_behaviors: u8, virtual_mods: u16, total_key_explicit: u8, total_mod_map_keys: u8, total_v_mod_map_keys: u8) {
        assert_eq!(self.switch_expr(), u16::from(present), "switch `map` has an inconsistent discriminant");
        if let Some(ref types_rtrn) = self.types_rtrn {
            assert_eq!(types_rtrn.len(), usize::try_from(n_types).unwrap(), "`types_rtrn` has an incorrect length");
            types_rtrn.serialize_into(bytes);
        }
        if let Some(ref syms_rtrn) = self.syms_rtrn {
            assert_eq!(syms_rtrn.len(), usize::try_from(n_key_syms).unwrap(), "`syms_rtrn` has an incorrect length");
            syms_rtrn.serialize_into(bytes);
        }
        if let Some(ref key_actions) = self.key_actions {
            key_actions.serialize_into(bytes, u8::from(n_key_actions), u16::from(total_actions));
        }
        if let Some(ref behaviors_rtrn) = self.behaviors_rtrn {
            assert_eq!(behaviors_rtrn.len(), usize::try_from(total_key_behaviors).unwrap(), "`behaviors_rtrn` has an incorrect length");
            behaviors_rtrn.serialize_into(bytes);
        }
        if let Some(ref vmods_rtrn) = self.vmods_rtrn {
            assert_eq!(vmods_rtrn.len(), usize::try_from(u32::from(virtual_mods).count_ones()).unwrap(), "`vmods_rtrn` has an incorrect length");
            for element in vmods_rtrn.iter().copied() {
                (u16::from(element) as u8).serialize_into(bytes);
            }
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref explicit_rtrn) = self.explicit_rtrn {
            assert_eq!(explicit_rtrn.len(), usize::try_from(total_key_explicit).unwrap(), "`explicit_rtrn` has an incorrect length");
            explicit_rtrn.serialize_into(bytes);
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref modmap_rtrn) = self.modmap_rtrn {
            assert_eq!(modmap_rtrn.len(), usize::try_from(total_mod_map_keys).unwrap(), "`modmap_rtrn` has an incorrect length");
            modmap_rtrn.serialize_into(bytes);
            bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        }
        if let Some(ref vmodmap_rtrn) = self.vmodmap_rtrn {
            assert_eq!(vmodmap_rtrn.len(), usize::try_from(total_v_mod_map_keys).unwrap(), "`vmodmap_rtrn` has an incorrect length");
            vmodmap_rtrn.serialize_into(bytes);
        }
    }
}
impl GetKbdByNameRepliesTypesMap {
    fn switch_expr(&self) -> u16 {
        let mut expr_value = 0;
        if self.types_rtrn.is_some() {
            expr_value |= u16::from(MapPart::KEY_TYPES);
        }
        if self.syms_rtrn.is_some() {
            expr_value |= u16::from(MapPart::KEY_SYMS);
        }
        if self.key_actions.is_some() {
            expr_value |= u16::from(MapPart::KEY_ACTIONS);
        }
        if self.behaviors_rtrn.is_some() {
            expr_value |= u16::from(MapPart::KEY_BEHAVIORS);
        }
        if self.vmods_rtrn.is_some() {
            expr_value |= u16::from(MapPart::VIRTUAL_MODS);
        }
        if self.explicit_rtrn.is_some() {
            expr_value |= u16::from(MapPart::EXPLICIT_COMPONENTS);
        }
        if self.modmap_rtrn.is_some() {
            expr_value |= u16::from(MapPart::MODIFIER_MAP);
        }
        if self.vmodmap_rtrn.is_some() {
            expr_value |= u16::from(MapPart::VIRTUAL_MOD_MAP);
        }
        expr_value
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesTypes {
    pub getmap_type: u8,
    pub type_device_id: u8,
    pub getmap_sequence: u16,
    pub getmap_length: u32,
    pub type_min_key_code: xproto::Keycode,
    pub type_max_key_code: xproto::Keycode,
    pub first_type: u8,
    pub n_types: u8,
    pub total_types: u8,
    pub first_key_sym: xproto::Keycode,
    pub total_syms: u16,
    pub n_key_syms: u8,
    pub first_key_action: xproto::Keycode,
    pub total_actions: u16,
    pub n_key_actions: u8,
    pub first_key_behavior: xproto::Keycode,
    pub n_key_behaviors: u8,
    pub total_key_behaviors: u8,
    pub first_key_explicit: xproto::Keycode,
    pub n_key_explicit: u8,
    pub total_key_explicit: u8,
    pub first_mod_map_key: xproto::Keycode,
    pub n_mod_map_keys: u8,
    pub total_mod_map_keys: u8,
    pub first_v_mod_map_key: xproto::Keycode,
    pub n_v_mod_map_keys: u8,
    pub total_v_mod_map_keys: u8,
    pub virtual_mods: VMod,
    pub map: GetKbdByNameRepliesTypesMap,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesTypes, "GetKbdByNameRepliesTypes");
impl TryParse for GetKbdByNameRepliesTypes {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (getmap_type, remaining) = u8::try_parse(remaining)?;
        let (type_device_id, remaining) = u8::try_parse(remaining)?;
        let (getmap_sequence, remaining) = u16::try_parse(remaining)?;
        let (getmap_length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (type_min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (type_max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (present, remaining) = u16::try_parse(remaining)?;
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (total_types, remaining) = u8::try_parse(remaining)?;
        let (first_key_sym, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (total_syms, remaining) = u16::try_parse(remaining)?;
        let (n_key_syms, remaining) = u8::try_parse(remaining)?;
        let (first_key_action, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (total_actions, remaining) = u16::try_parse(remaining)?;
        let (n_key_actions, remaining) = u8::try_parse(remaining)?;
        let (first_key_behavior, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (total_key_behaviors, remaining) = u8::try_parse(remaining)?;
        let (first_key_explicit, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (total_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (first_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (total_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (first_v_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (total_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let (map, remaining) = GetKbdByNameRepliesTypesMap::try_parse(remaining, u16::from(present), u8::from(n_types), u8::from(n_key_syms), u8::from(n_key_actions), u16::from(total_actions), u8::from(total_key_behaviors), u16::from(virtual_mods), u8::from(total_key_explicit), u8::from(total_mod_map_keys), u8::from(total_v_mod_map_keys))?;
        let virtual_mods = virtual_mods.into();
        let result = GetKbdByNameRepliesTypes { getmap_type, type_device_id, getmap_sequence, getmap_length, type_min_key_code, type_max_key_code, first_type, n_types, total_types, first_key_sym, total_syms, n_key_syms, first_key_action, total_actions, n_key_actions, first_key_behavior, n_key_behaviors, total_key_behaviors, first_key_explicit, n_key_explicit, total_key_explicit, first_mod_map_key, n_mod_map_keys, total_mod_map_keys, first_v_mod_map_key, n_v_mod_map_keys, total_v_mod_map_keys, virtual_mods, map };
        Ok((result, remaining))
    }
}
impl Serialize for GetKbdByNameRepliesTypes {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(40);
        self.getmap_type.serialize_into(bytes);
        self.type_device_id.serialize_into(bytes);
        self.getmap_sequence.serialize_into(bytes);
        self.getmap_length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.type_min_key_code.serialize_into(bytes);
        self.type_max_key_code.serialize_into(bytes);
        let present: u16 = self.map.switch_expr();
        present.serialize_into(bytes);
        self.first_type.serialize_into(bytes);
        self.n_types.serialize_into(bytes);
        self.total_types.serialize_into(bytes);
        self.first_key_sym.serialize_into(bytes);
        self.total_syms.serialize_into(bytes);
        self.n_key_syms.serialize_into(bytes);
        self.first_key_action.serialize_into(bytes);
        self.total_actions.serialize_into(bytes);
        self.n_key_actions.serialize_into(bytes);
        self.first_key_behavior.serialize_into(bytes);
        self.n_key_behaviors.serialize_into(bytes);
        self.total_key_behaviors.serialize_into(bytes);
        self.first_key_explicit.serialize_into(bytes);
        self.n_key_explicit.serialize_into(bytes);
        self.total_key_explicit.serialize_into(bytes);
        self.first_mod_map_key.serialize_into(bytes);
        self.n_mod_map_keys.serialize_into(bytes);
        self.total_mod_map_keys.serialize_into(bytes);
        self.first_v_mod_map_key.serialize_into(bytes);
        self.n_v_mod_map_keys.serialize_into(bytes);
        self.total_v_mod_map_keys.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        u16::from(self.virtual_mods).serialize_into(bytes);
        self.map.serialize_into(bytes, u16::from(present), u8::from(self.n_types), u8::from(self.n_key_syms), u8::from(self.n_key_actions), u16::from(self.total_actions), u8::from(self.total_key_behaviors), u16::from(self.virtual_mods), u8::from(self.total_key_explicit), u8::from(self.total_mod_map_keys), u8::from(self.total_v_mod_map_keys));
    }
}
#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesCompatMap {
    pub compatmap_type: u8,
    pub compat_device_id: u8,
    pub compatmap_sequence: u16,
    pub compatmap_length: u32,
    pub groups_rtrn: SetOfGroup,
    pub first_si_rtrn: u16,
    pub n_total_si: u16,
    pub si_rtrn: Vec<SymInterpret>,
    pub group_rtrn: Vec<ModDef>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesCompatMap, "GetKbdByNameRepliesCompatMap");
impl TryParse for GetKbdByNameRepliesCompatMap {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (compatmap_type, remaining) = u8::try_parse(remaining)?;
        let (compat_device_id, remaining) = u8::try_parse(remaining)?;
        let (compatmap_sequence, remaining) = u16::try_parse(remaining)?;
        let (compatmap_length, remaining) = u32::try_parse(remaining)?;
        let (groups_rtrn, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (first_si_rtrn, remaining) = u16::try_parse(remaining)?;
        let (n_si_rtrn, remaining) = u16::try_parse(remaining)?;
        let (n_total_si, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (si_rtrn, remaining) = crate::x11_utils::parse_list::<SymInterpret>(remaining, n_si_rtrn.try_to_usize()?)?;
        let (group_rtrn, remaining) = crate::x11_utils::parse_list::<ModDef>(remaining, u32::from(groups_rtrn).count_ones().try_to_usize()?)?;
        let groups_rtrn = groups_rtrn.into();
        let result = GetKbdByNameRepliesCompatMap { compatmap_type, compat_device_id, compatmap_sequence, compatmap_length, groups_rtrn, first_si_rtrn, n_total_si, si_rtrn, group_rtrn };
        Ok((result, remaining))
    }
}
impl Serialize for GetKbdByNameRepliesCompatMap {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.compatmap_type.serialize_into(bytes);
        self.compat_device_id.serialize_into(bytes);
        self.compatmap_sequence.serialize_into(bytes);
        self.compatmap_length.serialize_into(bytes);
        u8::from(self.groups_rtrn).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.first_si_rtrn.serialize_into(bytes);
        let n_si_rtrn = u16::try_from(self.si_rtrn.len()).expect("`si_rtrn` has too many elements");
        n_si_rtrn.serialize_into(bytes);
        self.n_total_si.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        self.si_rtrn.serialize_into(bytes);
        assert_eq!(self.group_rtrn.len(), usize::try_from(u32::from(self.groups_rtrn).count_ones()).unwrap(), "`group_rtrn` has an incorrect length");
        self.group_rtrn.serialize_into(bytes);
    }
}
impl GetKbdByNameRepliesCompatMap {
    /// Get the value of the `nSIRtrn` field.
    ///
    /// The `nSIRtrn` field is used as the length field of the `si_rtrn` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_si_rtrn(&self) -> u16 {
        self.si_rtrn.len()
            .try_into().unwrap()
    }
}
#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesIndicatorMaps {
    pub indicatormap_type: u8,
    pub indicator_device_id: u8,
    pub indicatormap_sequence: u16,
    pub indicatormap_length: u32,
    pub which: u32,
    pub real_indicators: u32,
    pub maps: Vec<IndicatorMap>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesIndicatorMaps, "GetKbdByNameRepliesIndicatorMaps");
impl TryParse for GetKbdByNameRepliesIndicatorMaps {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (indicatormap_type, remaining) = u8::try_parse(remaining)?;
        let (indicator_device_id, remaining) = u8::try_parse(remaining)?;
        let (indicatormap_sequence, remaining) = u16::try_parse(remaining)?;
        let (indicatormap_length, remaining) = u32::try_parse(remaining)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let (real_indicators, remaining) = u32::try_parse(remaining)?;
        let (n_indicators, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(15..).ok_or(ParseError::InsufficientData)?;
        let (maps, remaining) = crate::x11_utils::parse_list::<IndicatorMap>(remaining, n_indicators.try_to_usize()?)?;
        let result = GetKbdByNameRepliesIndicatorMaps { indicatormap_type, indicator_device_id, indicatormap_sequence, indicatormap_length, which, real_indicators, maps };
        Ok((result, remaining))
    }
}
impl Serialize for GetKbdByNameRepliesIndicatorMaps {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.indicatormap_type.serialize_into(bytes);
        self.indicator_device_id.serialize_into(bytes);
        self.indicatormap_sequence.serialize_into(bytes);
        self.indicatormap_length.serialize_into(bytes);
        self.which.serialize_into(bytes);
        self.real_indicators.serialize_into(bytes);
        let n_indicators = u8::try_from(self.maps.len()).expect("`maps` has too many elements");
        n_indicators.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 15]);
        self.maps.serialize_into(bytes);
    }
}
impl GetKbdByNameRepliesIndicatorMaps {
    /// Get the value of the `nIndicators` field.
    ///
    /// The `nIndicators` field is used as the length field of the `maps` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_indicators(&self) -> u8 {
        self.maps.len()
            .try_into().unwrap()
    }
}
#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesKeyNamesValueListKTLevelNames {
    pub n_levels_per_type: Vec<u8>,
    pub kt_level_names: Vec<xproto::Atom>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesKeyNamesValueListKTLevelNames, "GetKbdByNameRepliesKeyNamesValueListKTLevelNames");
impl GetKbdByNameRepliesKeyNamesValueListKTLevelNames {
    pub fn try_parse(remaining: &[u8], n_types: u8) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (n_levels_per_type, remaining) = crate::x11_utils::parse_u8_list(remaining, n_types.try_to_usize()?)?;
        let n_levels_per_type = n_levels_per_type.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (kt_level_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_levels_per_type.iter().try_fold(0u32, |acc, x| acc.checked_add(u32::from(*x)).ok_or(ParseError::InvalidExpression))?.try_to_usize()?)?;
        let result = GetKbdByNameRepliesKeyNamesValueListKTLevelNames { n_levels_per_type, kt_level_names };
        Ok((result, remaining))
    }
}
impl GetKbdByNameRepliesKeyNamesValueListKTLevelNames {
    #[allow(dead_code)]
    fn serialize(&self, n_types: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u8::from(n_types));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, n_types: u8) {
        assert_eq!(self.n_levels_per_type.len(), usize::try_from(n_types).unwrap(), "`n_levels_per_type` has an incorrect length");
        bytes.extend_from_slice(&self.n_levels_per_type);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        assert_eq!(self.kt_level_names.len(), usize::try_from(self.n_levels_per_type.iter().fold(0u32, |acc, x| acc.checked_add(u32::from(*x)).unwrap())).unwrap(), "`kt_level_names` has an incorrect length");
        self.kt_level_names.serialize_into(bytes);
    }
}
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesKeyNamesValueList {
    pub keycodes_name: Option<xproto::Atom>,
    pub geometry_name: Option<xproto::Atom>,
    pub symbols_name: Option<xproto::Atom>,
    pub phys_symbols_name: Option<xproto::Atom>,
    pub types_name: Option<xproto::Atom>,
    pub compat_name: Option<xproto::Atom>,
    pub type_names: Option<Vec<xproto::Atom>>,
    pub kt_level_names: Option<GetKbdByNameRepliesKeyNamesValueListKTLevelNames>,
    pub indicator_names: Option<Vec<xproto::Atom>>,
    pub virtual_mod_names: Option<Vec<xproto::Atom>>,
    pub groups: Option<Vec<xproto::Atom>>,
    pub key_names: Option<Vec<KeyName>>,
    pub key_aliases: Option<Vec<KeyAlias>>,
    pub radio_group_names: Option<Vec<xproto::Atom>>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesKeyNamesValueList, "GetKbdByNameRepliesKeyNamesValueList");
impl GetKbdByNameRepliesKeyNamesValueList {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(which);
        let mut outer_remaining = value;
        let keycodes_name = if switch_expr & u32::from(NameDetail::KEYCODES) != 0 {
            let remaining = outer_remaining;
            let (keycodes_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(keycodes_name)
        } else {
            None
        };
        let geometry_name = if switch_expr & u32::from(NameDetail::GEOMETRY) != 0 {
            let remaining = outer_remaining;
            let (geometry_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(geometry_name)
        } else {
            None
        };
        let symbols_name = if switch_expr & u32::from(NameDetail::SYMBOLS) != 0 {
            let remaining = outer_remaining;
            let (symbols_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(symbols_name)
        } else {
            None
        };
        let phys_symbols_name = if switch_expr & u32::from(NameDetail::PHYS_SYMBOLS) != 0 {
            let remaining = outer_remaining;
            let (phys_symbols_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(phys_symbols_name)
        } else {
            None
        };
        let types_name = if switch_expr & u32::from(NameDetail::TYPES) != 0 {
            let remaining = outer_remaining;
            let (types_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(types_name)
        } else {
            None
        };
        let compat_name = if switch_expr & u32::from(NameDetail::COMPAT) != 0 {
            let remaining = outer_remaining;
            let (compat_name, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(compat_name)
        } else {
            None
        };
        let type_names = if switch_expr & u32::from(NameDetail::KEY_TYPE_NAMES) != 0 {
            let remaining = outer_remaining;
            let (type_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_types.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(type_names)
        } else {
            None
        };
        let kt_level_names = if switch_expr & u32::from(NameDetail::KT_LEVEL_NAMES) != 0 {
            let (kt_level_names, new_remaining) = GetKbdByNameRepliesKeyNamesValueListKTLevelNames::try_parse(outer_remaining, n_types)?;
            outer_remaining = new_remaining;
            Some(kt_level_names)
        } else {
            None
        };
        let indicator_names = if switch_expr & u32::from(NameDetail::INDICATOR_NAMES) != 0 {
            let remaining = outer_remaining;
            let (indicator_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(indicators).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(indicator_names)
        } else {
            None
        };
        let virtual_mod_names = if switch_expr & u32::from(NameDetail::VIRTUAL_MOD_NAMES) != 0 {
            let remaining = outer_remaining;
            let (virtual_mod_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(virtual_mods).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(virtual_mod_names)
        } else {
            None
        };
        let groups = if switch_expr & u32::from(NameDetail::GROUP_NAMES) != 0 {
            let remaining = outer_remaining;
            let (groups, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, u32::from(group_names).count_ones().try_to_usize()?)?;
            outer_remaining = remaining;
            Some(groups)
        } else {
            None
        };
        let key_names = if switch_expr & u32::from(NameDetail::KEY_NAMES) != 0 {
            let remaining = outer_remaining;
            let (key_names, remaining) = crate::x11_utils::parse_list::<KeyName>(remaining, n_keys.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(key_names)
        } else {
            None
        };
        let key_aliases = if switch_expr & u32::from(NameDetail::KEY_ALIASES) != 0 {
            let remaining = outer_remaining;
            let (key_aliases, remaining) = crate::x11_utils::parse_list::<KeyAlias>(remaining, n_key_aliases.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(key_aliases)
        } else {
            None
        };
        let radio_group_names = if switch_expr & u32::from(NameDetail::RG_NAMES) != 0 {
            let remaining = outer_remaining;
            let (radio_group_names, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, n_radio_groups.try_to_usize()?)?;
            outer_remaining = remaining;
            Some(radio_group_names)
        } else {
            None
        };
        let result = GetKbdByNameRepliesKeyNamesValueList { keycodes_name, geometry_name, symbols_name, phys_symbols_name, types_name, compat_name, type_names, kt_level_names, indicator_names, virtual_mod_names, groups, key_names, key_aliases, radio_group_names };
        Ok((result, outer_remaining))
    }
}
impl GetKbdByNameRepliesKeyNamesValueList {
    #[allow(dead_code)]
    fn serialize(&self, which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(which), u8::from(n_types), u32::from(indicators), u16::from(virtual_mods), u8::from(group_names), u8::from(n_keys), u8::from(n_key_aliases), u8::from(n_radio_groups));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, which: u32, n_types: u8, indicators: u32, virtual_mods: u16, group_names: u8, n_keys: u8, n_key_aliases: u8, n_radio_groups: u8) {
        assert_eq!(self.switch_expr(), u32::from(which), "switch `value_list` has an inconsistent discriminant");
        if let Some(keycodes_name) = self.keycodes_name {
            keycodes_name.serialize_into(bytes);
        }
        if let Some(geometry_name) = self.geometry_name {
            geometry_name.serialize_into(bytes);
        }
        if let Some(symbols_name) = self.symbols_name {
            symbols_name.serialize_into(bytes);
        }
        if let Some(phys_symbols_name) = self.phys_symbols_name {
            phys_symbols_name.serialize_into(bytes);
        }
        if let Some(types_name) = self.types_name {
            types_name.serialize_into(bytes);
        }
        if let Some(compat_name) = self.compat_name {
            compat_name.serialize_into(bytes);
        }
        if let Some(ref type_names) = self.type_names {
            assert_eq!(type_names.len(), usize::try_from(n_types).unwrap(), "`type_names` has an incorrect length");
            type_names.serialize_into(bytes);
        }
        if let Some(ref kt_level_names) = self.kt_level_names {
            kt_level_names.serialize_into(bytes, u8::from(n_types));
        }
        if let Some(ref indicator_names) = self.indicator_names {
            assert_eq!(indicator_names.len(), usize::try_from(u32::from(indicators).count_ones()).unwrap(), "`indicator_names` has an incorrect length");
            indicator_names.serialize_into(bytes);
        }
        if let Some(ref virtual_mod_names) = self.virtual_mod_names {
            assert_eq!(virtual_mod_names.len(), usize::try_from(u32::from(virtual_mods).count_ones()).unwrap(), "`virtual_mod_names` has an incorrect length");
            virtual_mod_names.serialize_into(bytes);
        }
        if let Some(ref groups) = self.groups {
            assert_eq!(groups.len(), usize::try_from(u32::from(group_names).count_ones()).unwrap(), "`groups` has an incorrect length");
            groups.serialize_into(bytes);
        }
        if let Some(ref key_names) = self.key_names {
            assert_eq!(key_names.len(), usize::try_from(n_keys).unwrap(), "`key_names` has an incorrect length");
            key_names.serialize_into(bytes);
        }
        if let Some(ref key_aliases) = self.key_aliases {
            assert_eq!(key_aliases.len(), usize::try_from(n_key_aliases).unwrap(), "`key_aliases` has an incorrect length");
            key_aliases.serialize_into(bytes);
        }
        if let Some(ref radio_group_names) = self.radio_group_names {
            assert_eq!(radio_group_names.len(), usize::try_from(n_radio_groups).unwrap(), "`radio_group_names` has an incorrect length");
            radio_group_names.serialize_into(bytes);
        }
    }
}
impl GetKbdByNameRepliesKeyNamesValueList {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.keycodes_name.is_some() {
            expr_value |= u32::from(NameDetail::KEYCODES);
        }
        if self.geometry_name.is_some() {
            expr_value |= u32::from(NameDetail::GEOMETRY);
        }
        if self.symbols_name.is_some() {
            expr_value |= u32::from(NameDetail::SYMBOLS);
        }
        if self.phys_symbols_name.is_some() {
            expr_value |= u32::from(NameDetail::PHYS_SYMBOLS);
        }
        if self.types_name.is_some() {
            expr_value |= u32::from(NameDetail::TYPES);
        }
        if self.compat_name.is_some() {
            expr_value |= u32::from(NameDetail::COMPAT);
        }
        if self.type_names.is_some() {
            expr_value |= u32::from(NameDetail::KEY_TYPE_NAMES);
        }
        if self.kt_level_names.is_some() {
            expr_value |= u32::from(NameDetail::KT_LEVEL_NAMES);
        }
        if self.indicator_names.is_some() {
            expr_value |= u32::from(NameDetail::INDICATOR_NAMES);
        }
        if self.virtual_mod_names.is_some() {
            expr_value |= u32::from(NameDetail::VIRTUAL_MOD_NAMES);
        }
        if self.groups.is_some() {
            expr_value |= u32::from(NameDetail::GROUP_NAMES);
        }
        if self.key_names.is_some() {
            expr_value |= u32::from(NameDetail::KEY_NAMES);
        }
        if self.key_aliases.is_some() {
            expr_value |= u32::from(NameDetail::KEY_ALIASES);
        }
        if self.radio_group_names.is_some() {
            expr_value |= u32::from(NameDetail::RG_NAMES);
        }
        expr_value
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesKeyNames {
    pub keyname_type: u8,
    pub key_device_id: u8,
    pub keyname_sequence: u16,
    pub keyname_length: u32,
    pub key_min_key_code: xproto::Keycode,
    pub key_max_key_code: xproto::Keycode,
    pub n_types: u8,
    pub group_names: SetOfGroup,
    pub virtual_mods: VMod,
    pub first_key: xproto::Keycode,
    pub n_keys: u8,
    pub indicators: u32,
    pub n_radio_groups: u8,
    pub n_key_aliases: u8,
    pub n_kt_levels: u16,
    pub value_list: GetKbdByNameRepliesKeyNamesValueList,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesKeyNames, "GetKbdByNameRepliesKeyNames");
impl TryParse for GetKbdByNameRepliesKeyNames {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (keyname_type, remaining) = u8::try_parse(remaining)?;
        let (key_device_id, remaining) = u8::try_parse(remaining)?;
        let (keyname_sequence, remaining) = u16::try_parse(remaining)?;
        let (keyname_length, remaining) = u32::try_parse(remaining)?;
        let (which, remaining) = u32::try_parse(remaining)?;
        let (key_min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (key_max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (group_names, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let (first_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_keys, remaining) = u8::try_parse(remaining)?;
        let (indicators, remaining) = u32::try_parse(remaining)?;
        let (n_radio_groups, remaining) = u8::try_parse(remaining)?;
        let (n_key_aliases, remaining) = u8::try_parse(remaining)?;
        let (n_kt_levels, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (value_list, remaining) = GetKbdByNameRepliesKeyNamesValueList::try_parse(remaining, u32::from(which), u8::from(n_types), u32::from(indicators), u16::from(virtual_mods), u8::from(group_names), u8::from(n_keys), u8::from(n_key_aliases), u8::from(n_radio_groups))?;
        let group_names = group_names.into();
        let virtual_mods = virtual_mods.into();
        let result = GetKbdByNameRepliesKeyNames { keyname_type, key_device_id, keyname_sequence, keyname_length, key_min_key_code, key_max_key_code, n_types, group_names, virtual_mods, first_key, n_keys, indicators, n_radio_groups, n_key_aliases, n_kt_levels, value_list };
        Ok((result, remaining))
    }
}
impl Serialize for GetKbdByNameRepliesKeyNames {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.keyname_type.serialize_into(bytes);
        self.key_device_id.serialize_into(bytes);
        self.keyname_sequence.serialize_into(bytes);
        self.keyname_length.serialize_into(bytes);
        let which: u32 = self.value_list.switch_expr();
        which.serialize_into(bytes);
        self.key_min_key_code.serialize_into(bytes);
        self.key_max_key_code.serialize_into(bytes);
        self.n_types.serialize_into(bytes);
        u8::from(self.group_names).serialize_into(bytes);
        u16::from(self.virtual_mods).serialize_into(bytes);
        self.first_key.serialize_into(bytes);
        self.n_keys.serialize_into(bytes);
        self.indicators.serialize_into(bytes);
        self.n_radio_groups.serialize_into(bytes);
        self.n_key_aliases.serialize_into(bytes);
        self.n_kt_levels.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.value_list.serialize_into(bytes, u32::from(which), u8::from(self.n_types), u32::from(self.indicators), u16::from(self.virtual_mods), u8::from(self.group_names), u8::from(self.n_keys), u8::from(self.n_key_aliases), u8::from(self.n_radio_groups));
    }
}
#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameRepliesGeometry {
    pub geometry_type: u8,
    pub geometry_device_id: u8,
    pub geometry_sequence: u16,
    pub geometry_length: u32,
    pub name: xproto::Atom,
    pub geometry_found: bool,
    pub width_mm: u16,
    pub height_mm: u16,
    pub n_properties: u16,
    pub n_colors: u16,
    pub n_shapes: u16,
    pub n_sections: u16,
    pub n_doodads: u16,
    pub n_key_aliases: u16,
    pub base_color_ndx: u8,
    pub label_color_ndx: u8,
    pub label_font: CountedString16,
}
impl_debug_if_no_extra_traits!(GetKbdByNameRepliesGeometry, "GetKbdByNameRepliesGeometry");
impl TryParse for GetKbdByNameRepliesGeometry {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (geometry_type, remaining) = u8::try_parse(remaining)?;
        let (geometry_device_id, remaining) = u8::try_parse(remaining)?;
        let (geometry_sequence, remaining) = u16::try_parse(remaining)?;
        let (geometry_length, remaining) = u32::try_parse(remaining)?;
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (geometry_found, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (width_mm, remaining) = u16::try_parse(remaining)?;
        let (height_mm, remaining) = u16::try_parse(remaining)?;
        let (n_properties, remaining) = u16::try_parse(remaining)?;
        let (n_colors, remaining) = u16::try_parse(remaining)?;
        let (n_shapes, remaining) = u16::try_parse(remaining)?;
        let (n_sections, remaining) = u16::try_parse(remaining)?;
        let (n_doodads, remaining) = u16::try_parse(remaining)?;
        let (n_key_aliases, remaining) = u16::try_parse(remaining)?;
        let (base_color_ndx, remaining) = u8::try_parse(remaining)?;
        let (label_color_ndx, remaining) = u8::try_parse(remaining)?;
        let (label_font, remaining) = CountedString16::try_parse(remaining)?;
        let result = GetKbdByNameRepliesGeometry { geometry_type, geometry_device_id, geometry_sequence, geometry_length, name, geometry_found, width_mm, height_mm, n_properties, n_colors, n_shapes, n_sections, n_doodads, n_key_aliases, base_color_ndx, label_color_ndx, label_font };
        Ok((result, remaining))
    }
}
impl Serialize for GetKbdByNameRepliesGeometry {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.geometry_type.serialize_into(bytes);
        self.geometry_device_id.serialize_into(bytes);
        self.geometry_sequence.serialize_into(bytes);
        self.geometry_length.serialize_into(bytes);
        self.name.serialize_into(bytes);
        self.geometry_found.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.width_mm.serialize_into(bytes);
        self.height_mm.serialize_into(bytes);
        self.n_properties.serialize_into(bytes);
        self.n_colors.serialize_into(bytes);
        self.n_shapes.serialize_into(bytes);
        self.n_sections.serialize_into(bytes);
        self.n_doodads.serialize_into(bytes);
        self.n_key_aliases.serialize_into(bytes);
        self.base_color_ndx.serialize_into(bytes);
        self.label_color_ndx.serialize_into(bytes);
        self.label_font.serialize_into(bytes);
    }
}
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameReplies {
    pub types: Option<GetKbdByNameRepliesTypes>,
    pub compat_map: Option<GetKbdByNameRepliesCompatMap>,
    pub indicator_maps: Option<GetKbdByNameRepliesIndicatorMaps>,
    pub key_names: Option<GetKbdByNameRepliesKeyNames>,
    pub geometry: Option<GetKbdByNameRepliesGeometry>,
}
impl_debug_if_no_extra_traits!(GetKbdByNameReplies, "GetKbdByNameReplies");
impl GetKbdByNameReplies {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], reported: u16) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u16::from(reported);
        let mut outer_remaining = value;
        let types = if switch_expr & u16::from(GBNDetail::TYPES) != 0 || switch_expr & u16::from(GBNDetail::CLIENT_SYMBOLS) != 0 || switch_expr & u16::from(GBNDetail::SERVER_SYMBOLS) != 0 {
            let (types, new_remaining) = GetKbdByNameRepliesTypes::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(types)
        } else {
            None
        };
        let compat_map = if switch_expr & u16::from(GBNDetail::COMPAT_MAP) != 0 {
            let (compat_map, new_remaining) = GetKbdByNameRepliesCompatMap::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(compat_map)
        } else {
            None
        };
        let indicator_maps = if switch_expr & u16::from(GBNDetail::INDICATOR_MAPS) != 0 {
            let (indicator_maps, new_remaining) = GetKbdByNameRepliesIndicatorMaps::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(indicator_maps)
        } else {
            None
        };
        let key_names = if switch_expr & u16::from(GBNDetail::KEY_NAMES) != 0 || switch_expr & u16::from(GBNDetail::OTHER_NAMES) != 0 {
            let (key_names, new_remaining) = GetKbdByNameRepliesKeyNames::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(key_names)
        } else {
            None
        };
        let geometry = if switch_expr & u16::from(GBNDetail::GEOMETRY) != 0 {
            let (geometry, new_remaining) = GetKbdByNameRepliesGeometry::try_parse(outer_remaining)?;
            outer_remaining = new_remaining;
            Some(geometry)
        } else {
            None
        };
        let result = GetKbdByNameReplies { types, compat_map, indicator_maps, key_names, geometry };
        Ok((result, outer_remaining))
    }
}
impl GetKbdByNameReplies {
    #[allow(dead_code)]
    fn serialize(&self, reported: u16) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u16::from(reported));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, reported: u16) {
        let _ = reported;
        if let Some(ref types) = self.types {
            types.serialize_into(bytes);
        }
        if let Some(ref compat_map) = self.compat_map {
            compat_map.serialize_into(bytes);
        }
        if let Some(ref indicator_maps) = self.indicator_maps {
            indicator_maps.serialize_into(bytes);
        }
        if let Some(ref key_names) = self.key_names {
            key_names.serialize_into(bytes);
        }
        if let Some(ref geometry) = self.geometry {
            geometry.serialize_into(bytes);
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKbdByNameReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub min_key_code: xproto::Keycode,
    pub max_key_code: xproto::Keycode,
    pub loaded: bool,
    pub new_keyboard: bool,
    pub found: GBNDetail,
    pub reported: GBNDetail,
    pub replies: GetKbdByNameReplies,
}
impl_debug_if_no_extra_traits!(GetKbdByNameReply, "GetKbdByNameReply");
impl TryParse for GetKbdByNameReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (loaded, remaining) = bool::try_parse(remaining)?;
        let (new_keyboard, remaining) = bool::try_parse(remaining)?;
        let (found, remaining) = u16::try_parse(remaining)?;
        let (reported, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (replies, remaining) = GetKbdByNameReplies::try_parse(remaining, u16::from(reported))?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let found = found.into();
        let reported = reported.into();
        let result = GetKbdByNameReply { device_id, sequence, length, min_key_code, max_key_code, loaded, new_keyboard, found, reported, replies };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetKbdByNameReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.min_key_code.serialize_into(bytes);
        self.max_key_code.serialize_into(bytes);
        self.loaded.serialize_into(bytes);
        self.new_keyboard.serialize_into(bytes);
        u16::from(self.found).serialize_into(bytes);
        u16::from(self.reported).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        self.replies.serialize_into(bytes, u16::from(self.reported));
    }
}

/// Opcode for the GetDeviceInfo request
pub const GET_DEVICE_INFO_REQUEST: u8 = 24;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceInfoRequest {
    pub device_spec: DeviceSpec,
    pub wanted: XIFeature,
    pub all_buttons: bool,
    pub first_button: u8,
    pub n_buttons: u8,
    pub led_class: LedClass,
    pub led_id: IDSpec,
}
impl_debug_if_no_extra_traits!(GetDeviceInfoRequest, "GetDeviceInfoRequest");
impl GetDeviceInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let wanted_bytes = u16::from(self.wanted).serialize();
        let all_buttons_bytes = self.all_buttons.serialize();
        let first_button_bytes = self.first_button.serialize();
        let n_buttons_bytes = self.n_buttons.serialize();
        let led_class_bytes = LedClassSpec::from(self.led_class).serialize();
        let led_id_bytes = self.led_id.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_DEVICE_INFO_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            wanted_bytes[0],
            wanted_bytes[1],
            all_buttons_bytes[0],
            first_button_bytes[0],
            n_buttons_bytes[0],
            0,
            led_class_bytes[0],
            led_class_bytes[1],
            led_id_bytes[0],
            led_id_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_DEVICE_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (wanted, remaining) = u16::try_parse(remaining)?;
        let wanted = wanted.into();
        let (all_buttons, remaining) = bool::try_parse(remaining)?;
        let (first_button, remaining) = u8::try_parse(remaining)?;
        let (n_buttons, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (led_class, remaining) = LedClassSpec::try_parse(remaining)?;
        let led_class = led_class.into();
        let (led_id, remaining) = IDSpec::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetDeviceInfoRequest {
            device_spec,
            wanted,
            all_buttons,
            first_button,
            n_buttons,
            led_class,
            led_id,
        })
    }
}
impl Request for GetDeviceInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetDeviceInfoRequest {
    type Reply = GetDeviceInfoReply;
}

#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceInfoReply {
    pub device_id: u8,
    pub sequence: u16,
    pub length: u32,
    pub present: XIFeature,
    pub supported: XIFeature,
    pub unsupported: XIFeature,
    pub first_btn_wanted: u8,
    pub n_btns_wanted: u8,
    pub first_btn_rtrn: u8,
    pub total_btns: u8,
    pub has_own_state: bool,
    pub dflt_kbd_fb: u16,
    pub dflt_led_fb: u16,
    pub dev_type: xproto::Atom,
    pub name: Vec<String8>,
    pub btn_actions: Vec<Action>,
    pub leds: Vec<DeviceLedInfo>,
}
impl_debug_if_no_extra_traits!(GetDeviceInfoReply, "GetDeviceInfoReply");
impl TryParse for GetDeviceInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let value = remaining;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (present, remaining) = u16::try_parse(remaining)?;
        let (supported, remaining) = u16::try_parse(remaining)?;
        let (unsupported, remaining) = u16::try_parse(remaining)?;
        let (n_device_led_f_bs, remaining) = u16::try_parse(remaining)?;
        let (first_btn_wanted, remaining) = u8::try_parse(remaining)?;
        let (n_btns_wanted, remaining) = u8::try_parse(remaining)?;
        let (first_btn_rtrn, remaining) = u8::try_parse(remaining)?;
        let (n_btns_rtrn, remaining) = u8::try_parse(remaining)?;
        let (total_btns, remaining) = u8::try_parse(remaining)?;
        let (has_own_state, remaining) = bool::try_parse(remaining)?;
        let (dflt_kbd_fb, remaining) = u16::try_parse(remaining)?;
        let (dflt_led_fb, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (dev_type, remaining) = xproto::Atom::try_parse(remaining)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let name = name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (btn_actions, remaining) = crate::x11_utils::parse_list::<Action>(remaining, n_btns_rtrn.try_to_usize()?)?;
        let (leds, remaining) = crate::x11_utils::parse_list::<DeviceLedInfo>(remaining, n_device_led_f_bs.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let present = present.into();
        let supported = supported.into();
        let unsupported = unsupported.into();
        let result = GetDeviceInfoReply { device_id, sequence, length, present, supported, unsupported, first_btn_wanted, n_btns_wanted, first_btn_rtrn, total_btns, has_own_state, dflt_kbd_fb, dflt_led_fb, dev_type, name, btn_actions, leds };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetDeviceInfoReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(34);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.device_id.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        u16::from(self.present).serialize_into(bytes);
        u16::from(self.supported).serialize_into(bytes);
        u16::from(self.unsupported).serialize_into(bytes);
        let n_device_led_f_bs = u16::try_from(self.leds.len()).expect("`leds` has too many elements");
        n_device_led_f_bs.serialize_into(bytes);
        self.first_btn_wanted.serialize_into(bytes);
        self.n_btns_wanted.serialize_into(bytes);
        self.first_btn_rtrn.serialize_into(bytes);
        let n_btns_rtrn = u8::try_from(self.btn_actions.len()).expect("`btn_actions` has too many elements");
        n_btns_rtrn.serialize_into(bytes);
        self.total_btns.serialize_into(bytes);
        self.has_own_state.serialize_into(bytes);
        self.dflt_kbd_fb.serialize_into(bytes);
        self.dflt_led_fb.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.dev_type.serialize_into(bytes);
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        name_len.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        self.btn_actions.serialize_into(bytes);
        self.leds.serialize_into(bytes);
    }
}
impl GetDeviceInfoReply {
    /// Get the value of the `nDeviceLedFBs` field.
    ///
    /// The `nDeviceLedFBs` field is used as the length field of the `leds` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_device_led_f_bs(&self) -> u16 {
        self.leds.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nBtnsRtrn` field.
    ///
    /// The `nBtnsRtrn` field is used as the length field of the `btnActions` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_btns_rtrn(&self) -> u8 {
        self.btn_actions.len()
            .try_into().unwrap()
    }
    /// Get the value of the `nameLen` field.
    ///
    /// The `nameLen` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn name_len(&self) -> u16 {
        self.name.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetDeviceInfo request
pub const SET_DEVICE_INFO_REQUEST: u8 = 25;
#[derive(Clone)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDeviceInfoRequest<'input> {
    pub device_spec: DeviceSpec,
    pub first_btn: u8,
    pub change: XIFeature,
    pub btn_actions: Cow<'input, [Action]>,
    pub leds: Cow<'input, [DeviceLedInfo]>,
}
impl_debug_if_no_extra_traits!(SetDeviceInfoRequest<'_>, "SetDeviceInfoRequest");
impl<'input> SetDeviceInfoRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let device_spec_bytes = self.device_spec.serialize();
        let first_btn_bytes = self.first_btn.serialize();
        let n_btns = u8::try_from(self.btn_actions.len()).expect("`btn_actions` has too many elements");
        let n_btns_bytes = n_btns.serialize();
        let change_bytes = u16::from(self.change).serialize();
        let n_device_led_f_bs = u16::try_from(self.leds.len()).expect("`leds` has too many elements");
        let n_device_led_f_bs_bytes = n_device_led_f_bs.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_DEVICE_INFO_REQUEST,
            0,
            0,
            device_spec_bytes[0],
            device_spec_bytes[1],
            first_btn_bytes[0],
            n_btns_bytes[0],
            change_bytes[0],
            change_bytes[1],
            n_device_led_f_bs_bytes[0],
            n_device_led_f_bs_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let btn_actions_bytes = self.btn_actions.serialize();
        let length_so_far = length_so_far + btn_actions_bytes.len();
        let leds_bytes = self.leds.serialize();
        let length_so_far = length_so_far + leds_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), btn_actions_bytes.into(), leds_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_DEVICE_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device_spec, remaining) = DeviceSpec::try_parse(value)?;
        let (first_btn, remaining) = u8::try_parse(remaining)?;
        let (n_btns, remaining) = u8::try_parse(remaining)?;
        let (change, remaining) = u16::try_parse(remaining)?;
        let change = change.into();
        let (n_device_led_f_bs, remaining) = u16::try_parse(remaining)?;
        let (btn_actions, remaining) = crate::x11_utils::parse_list::<Action>(remaining, n_btns.try_to_usize()?)?;
        let (leds, remaining) = crate::x11_utils::parse_list::<DeviceLedInfo>(remaining, n_device_led_f_bs.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetDeviceInfoRequest {
            device_spec,
            first_btn,
            change,
            btn_actions: Cow::Owned(btn_actions),
            leds: Cow::Owned(leds),
        })
    }
    /// Clone all borrowed data in this SetDeviceInfoRequest.
    pub fn into_owned(self) -> SetDeviceInfoRequest<'static> {
        SetDeviceInfoRequest {
            device_spec: self.device_spec,
            first_btn: self.first_btn,
            change: self.change,
            btn_actions: Cow::Owned(self.btn_actions.into_owned()),
            leds: Cow::Owned(self.leds.into_owned()),
        }
    }
}
impl<'input> Request for SetDeviceInfoRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetDeviceInfoRequest<'input> {
}

/// Opcode for the SetDebuggingFlags request
pub const SET_DEBUGGING_FLAGS_REQUEST: u8 = 101;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDebuggingFlagsRequest<'input> {
    pub affect_flags: u32,
    pub flags: u32,
    pub affect_ctrls: u32,
    pub ctrls: u32,
    pub message: Cow<'input, [String8]>,
}
impl_debug_if_no_extra_traits!(SetDebuggingFlagsRequest<'_>, "SetDebuggingFlagsRequest");
impl<'input> SetDebuggingFlagsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let msg_length = u16::try_from(self.message.len()).expect("`message` has too many elements");
        let msg_length_bytes = msg_length.serialize();
        let affect_flags_bytes = self.affect_flags.serialize();
        let flags_bytes = self.flags.serialize();
        let affect_ctrls_bytes = self.affect_ctrls.serialize();
        let ctrls_bytes = self.ctrls.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_DEBUGGING_FLAGS_REQUEST,
            0,
            0,
            msg_length_bytes[0],
            msg_length_bytes[1],
            0,
            0,
            affect_flags_bytes[0],
            affect_flags_bytes[1],
            affect_flags_bytes[2],
            affect_flags_bytes[3],
            flags_bytes[0],
            flags_bytes[1],
            flags_bytes[2],
            flags_bytes[3],
            affect_ctrls_bytes[0],
            affect_ctrls_bytes[1],
            affect_ctrls_bytes[2],
            affect_ctrls_bytes[3],
            ctrls_bytes[0],
            ctrls_bytes[1],
            ctrls_bytes[2],
            ctrls_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.message.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.message, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_DEBUGGING_FLAGS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (msg_length, remaining) = u16::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (affect_flags, remaining) = u32::try_parse(remaining)?;
        let (flags, remaining) = u32::try_parse(remaining)?;
        let (affect_ctrls, remaining) = u32::try_parse(remaining)?;
        let (ctrls, remaining) = u32::try_parse(remaining)?;
        let (message, remaining) = crate::x11_utils::parse_u8_list(remaining, msg_length.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetDebuggingFlagsRequest {
            affect_flags,
            flags,
            affect_ctrls,
            ctrls,
            message: Cow::Borrowed(message),
        })
    }
    /// Clone all borrowed data in this SetDebuggingFlagsRequest.
    pub fn into_owned(self) -> SetDebuggingFlagsRequest<'static> {
        SetDebuggingFlagsRequest {
            affect_flags: self.affect_flags,
            flags: self.flags,
            affect_ctrls: self.affect_ctrls,
            ctrls: self.ctrls,
            message: Cow::Owned(self.message.into_owned()),
        }
    }
}
impl<'input> Request for SetDebuggingFlagsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for SetDebuggingFlagsRequest<'input> {
    type Reply = SetDebuggingFlagsReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDebuggingFlagsReply {
    pub sequence: u16,
    pub length: u32,
    pub current_flags: u32,
    pub current_ctrls: u32,
    pub supported_flags: u32,
    pub supported_ctrls: u32,
}
impl_debug_if_no_extra_traits!(SetDebuggingFlagsReply, "SetDebuggingFlagsReply");
impl TryParse for SetDebuggingFlagsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (current_flags, remaining) = u32::try_parse(remaining)?;
        let (current_ctrls, remaining) = u32::try_parse(remaining)?;
        let (supported_flags, remaining) = u32::try_parse(remaining)?;
        let (supported_ctrls, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = SetDebuggingFlagsReply { sequence, length, current_flags, current_ctrls, supported_flags, supported_ctrls };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SetDebuggingFlagsReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let current_flags_bytes = self.current_flags.serialize();
        let current_ctrls_bytes = self.current_ctrls.serialize();
        let supported_flags_bytes = self.supported_flags.serialize();
        let supported_ctrls_bytes = self.supported_ctrls.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            current_flags_bytes[0],
            current_flags_bytes[1],
            current_flags_bytes[2],
            current_flags_bytes[3],
            current_ctrls_bytes[0],
            current_ctrls_bytes[1],
            current_ctrls_bytes[2],
            current_ctrls_bytes[3],
            supported_flags_bytes[0],
            supported_flags_bytes[1],
            supported_flags_bytes[2],
            supported_flags_bytes[3],
            supported_ctrls_bytes[0],
            supported_ctrls_bytes[1],
            supported_ctrls_bytes[2],
            supported_ctrls_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.current_flags.serialize_into(bytes);
        self.current_ctrls.serialize_into(bytes);
        self.supported_flags.serialize_into(bytes);
        self.supported_ctrls.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
    }
}

/// Opcode for the NewKeyboardNotify event
pub const NEW_KEYBOARD_NOTIFY_EVENT: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NewKeyboardNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub old_device_id: u8,
    pub min_key_code: xproto::Keycode,
    pub max_key_code: xproto::Keycode,
    pub old_min_key_code: xproto::Keycode,
    pub old_max_key_code: xproto::Keycode,
    pub request_major: u8,
    pub request_minor: u8,
    pub changed: NKNDetail,
}
impl_debug_if_no_extra_traits!(NewKeyboardNotifyEvent, "NewKeyboardNotifyEvent");
impl TryParse for NewKeyboardNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (old_device_id, remaining) = u8::try_parse(remaining)?;
        let (min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (old_min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (old_max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (request_major, remaining) = u8::try_parse(remaining)?;
        let (request_minor, remaining) = u8::try_parse(remaining)?;
        let (changed, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(14..).ok_or(ParseError::InsufficientData)?;
        let changed = changed.into();
        let result = NewKeyboardNotifyEvent { response_type, xkb_type, sequence, time, device_id, old_device_id, min_key_code, max_key_code, old_min_key_code, old_max_key_code, request_major, request_minor, changed };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for NewKeyboardNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let old_device_id_bytes = self.old_device_id.serialize();
        let min_key_code_bytes = self.min_key_code.serialize();
        let max_key_code_bytes = self.max_key_code.serialize();
        let old_min_key_code_bytes = self.old_min_key_code.serialize();
        let old_max_key_code_bytes = self.old_max_key_code.serialize();
        let request_major_bytes = self.request_major.serialize();
        let request_minor_bytes = self.request_minor.serialize();
        let changed_bytes = u16::from(self.changed).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            old_device_id_bytes[0],
            min_key_code_bytes[0],
            max_key_code_bytes[0],
            old_min_key_code_bytes[0],
            old_max_key_code_bytes[0],
            request_major_bytes[0],
            request_minor_bytes[0],
            changed_bytes[0],
            changed_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        self.old_device_id.serialize_into(bytes);
        self.min_key_code.serialize_into(bytes);
        self.max_key_code.serialize_into(bytes);
        self.old_min_key_code.serialize_into(bytes);
        self.old_max_key_code.serialize_into(bytes);
        self.request_major.serialize_into(bytes);
        self.request_minor.serialize_into(bytes);
        u16::from(self.changed).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 14]);
    }
}
impl From<&NewKeyboardNotifyEvent> for [u8; 32] {
    fn from(input: &NewKeyboardNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let old_device_id_bytes = input.old_device_id.serialize();
        let min_key_code_bytes = input.min_key_code.serialize();
        let max_key_code_bytes = input.max_key_code.serialize();
        let old_min_key_code_bytes = input.old_min_key_code.serialize();
        let old_max_key_code_bytes = input.old_max_key_code.serialize();
        let request_major_bytes = input.request_major.serialize();
        let request_minor_bytes = input.request_minor.serialize();
        let changed_bytes = u16::from(input.changed).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            old_device_id_bytes[0],
            min_key_code_bytes[0],
            max_key_code_bytes[0],
            old_min_key_code_bytes[0],
            old_max_key_code_bytes[0],
            request_major_bytes[0],
            request_minor_bytes[0],
            changed_bytes[0],
            changed_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<NewKeyboardNotifyEvent> for [u8; 32] {
    fn from(input: NewKeyboardNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the MapNotify event
pub const MAP_NOTIFY_EVENT: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub ptr_btn_actions: u8,
    pub changed: MapPart,
    pub min_key_code: xproto::Keycode,
    pub max_key_code: xproto::Keycode,
    pub first_type: u8,
    pub n_types: u8,
    pub first_key_sym: xproto::Keycode,
    pub n_key_syms: u8,
    pub first_key_act: xproto::Keycode,
    pub n_key_acts: u8,
    pub first_key_behavior: xproto::Keycode,
    pub n_key_behavior: u8,
    pub first_key_explicit: xproto::Keycode,
    pub n_key_explicit: u8,
    pub first_mod_map_key: xproto::Keycode,
    pub n_mod_map_keys: u8,
    pub first_v_mod_map_key: xproto::Keycode,
    pub n_v_mod_map_keys: u8,
    pub virtual_mods: VMod,
}
impl_debug_if_no_extra_traits!(MapNotifyEvent, "MapNotifyEvent");
impl TryParse for MapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (ptr_btn_actions, remaining) = u8::try_parse(remaining)?;
        let (changed, remaining) = u16::try_parse(remaining)?;
        let (min_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (max_key_code, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (first_key_sym, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_syms, remaining) = u8::try_parse(remaining)?;
        let (first_key_act, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_acts, remaining) = u8::try_parse(remaining)?;
        let (first_key_behavior, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_behavior, remaining) = u8::try_parse(remaining)?;
        let (first_key_explicit, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_key_explicit, remaining) = u8::try_parse(remaining)?;
        let (first_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (first_v_mod_map_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_v_mod_map_keys, remaining) = u8::try_parse(remaining)?;
        let (virtual_mods, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let changed = changed.into();
        let virtual_mods = virtual_mods.into();
        let result = MapNotifyEvent { response_type, xkb_type, sequence, time, device_id, ptr_btn_actions, changed, min_key_code, max_key_code, first_type, n_types, first_key_sym, n_key_syms, first_key_act, n_key_acts, first_key_behavior, n_key_behavior, first_key_explicit, n_key_explicit, first_mod_map_key, n_mod_map_keys, first_v_mod_map_key, n_v_mod_map_keys, virtual_mods };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for MapNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let ptr_btn_actions_bytes = self.ptr_btn_actions.serialize();
        let changed_bytes = u16::from(self.changed).serialize();
        let min_key_code_bytes = self.min_key_code.serialize();
        let max_key_code_bytes = self.max_key_code.serialize();
        let first_type_bytes = self.first_type.serialize();
        let n_types_bytes = self.n_types.serialize();
        let first_key_sym_bytes = self.first_key_sym.serialize();
        let n_key_syms_bytes = self.n_key_syms.serialize();
        let first_key_act_bytes = self.first_key_act.serialize();
        let n_key_acts_bytes = self.n_key_acts.serialize();
        let first_key_behavior_bytes = self.first_key_behavior.serialize();
        let n_key_behavior_bytes = self.n_key_behavior.serialize();
        let first_key_explicit_bytes = self.first_key_explicit.serialize();
        let n_key_explicit_bytes = self.n_key_explicit.serialize();
        let first_mod_map_key_bytes = self.first_mod_map_key.serialize();
        let n_mod_map_keys_bytes = self.n_mod_map_keys.serialize();
        let first_v_mod_map_key_bytes = self.first_v_mod_map_key.serialize();
        let n_v_mod_map_keys_bytes = self.n_v_mod_map_keys.serialize();
        let virtual_mods_bytes = u16::from(self.virtual_mods).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            ptr_btn_actions_bytes[0],
            changed_bytes[0],
            changed_bytes[1],
            min_key_code_bytes[0],
            max_key_code_bytes[0],
            first_type_bytes[0],
            n_types_bytes[0],
            first_key_sym_bytes[0],
            n_key_syms_bytes[0],
            first_key_act_bytes[0],
            n_key_acts_bytes[0],
            first_key_behavior_bytes[0],
            n_key_behavior_bytes[0],
            first_key_explicit_bytes[0],
            n_key_explicit_bytes[0],
            first_mod_map_key_bytes[0],
            n_mod_map_keys_bytes[0],
            first_v_mod_map_key_bytes[0],
            n_v_mod_map_keys_bytes[0],
            virtual_mods_bytes[0],
            virtual_mods_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        self.ptr_btn_actions.serialize_into(bytes);
        u16::from(self.changed).serialize_into(bytes);
        self.min_key_code.serialize_into(bytes);
        self.max_key_code.serialize_into(bytes);
        self.first_type.serialize_into(bytes);
        self.n_types.serialize_into(bytes);
        self.first_key_sym.serialize_into(bytes);
        self.n_key_syms.serialize_into(bytes);
        self.first_key_act.serialize_into(bytes);
        self.n_key_acts.serialize_into(bytes);
        self.first_key_behavior.serialize_into(bytes);
        self.n_key_behavior.serialize_into(bytes);
        self.first_key_explicit.serialize_into(bytes);
        self.n_key_explicit.serialize_into(bytes);
        self.first_mod_map_key.serialize_into(bytes);
        self.n_mod_map_keys.serialize_into(bytes);
        self.first_v_mod_map_key.serialize_into(bytes);
        self.n_v_mod_map_keys.serialize_into(bytes);
        u16::from(self.virtual_mods).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}
impl From<&MapNotifyEvent> for [u8; 32] {
    fn from(input: &MapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let ptr_btn_actions_bytes = input.ptr_btn_actions.serialize();
        let changed_bytes = u16::from(input.changed).serialize();
        let min_key_code_bytes = input.min_key_code.serialize();
        let max_key_code_bytes = input.max_key_code.serialize();
        let first_type_bytes = input.first_type.serialize();
        let n_types_bytes = input.n_types.serialize();
        let first_key_sym_bytes = input.first_key_sym.serialize();
        let n_key_syms_bytes = input.n_key_syms.serialize();
        let first_key_act_bytes = input.first_key_act.serialize();
        let n_key_acts_bytes = input.n_key_acts.serialize();
        let first_key_behavior_bytes = input.first_key_behavior.serialize();
        let n_key_behavior_bytes = input.n_key_behavior.serialize();
        let first_key_explicit_bytes = input.first_key_explicit.serialize();
        let n_key_explicit_bytes = input.n_key_explicit.serialize();
        let first_mod_map_key_bytes = input.first_mod_map_key.serialize();
        let n_mod_map_keys_bytes = input.n_mod_map_keys.serialize();
        let first_v_mod_map_key_bytes = input.first_v_mod_map_key.serialize();
        let n_v_mod_map_keys_bytes = input.n_v_mod_map_keys.serialize();
        let virtual_mods_bytes = u16::from(input.virtual_mods).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            ptr_btn_actions_bytes[0],
            changed_bytes[0],
            changed_bytes[1],
            min_key_code_bytes[0],
            max_key_code_bytes[0],
            first_type_bytes[0],
            n_types_bytes[0],
            first_key_sym_bytes[0],
            n_key_syms_bytes[0],
            first_key_act_bytes[0],
            n_key_acts_bytes[0],
            first_key_behavior_bytes[0],
            n_key_behavior_bytes[0],
            first_key_explicit_bytes[0],
            n_key_explicit_bytes[0],
            first_mod_map_key_bytes[0],
            n_mod_map_keys_bytes[0],
            first_v_mod_map_key_bytes[0],
            n_v_mod_map_keys_bytes[0],
            virtual_mods_bytes[0],
            virtual_mods_bytes[1],
            0,
            0,
        ]
    }
}
impl From<MapNotifyEvent> for [u8; 32] {
    fn from(input: MapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the StateNotify event
pub const STATE_NOTIFY_EVENT: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub mods: xproto::ModMask,
    pub base_mods: xproto::ModMask,
    pub latched_mods: xproto::ModMask,
    pub locked_mods: xproto::ModMask,
    pub group: Group,
    pub base_group: i16,
    pub latched_group: i16,
    pub locked_group: Group,
    pub compat_state: xproto::ModMask,
    pub grab_mods: xproto::ModMask,
    pub compat_grab_mods: xproto::ModMask,
    pub lookup_mods: xproto::ModMask,
    pub compat_loockup_mods: xproto::ModMask,
    pub ptr_btn_state: xproto::KeyButMask,
    pub changed: StatePart,
    pub keycode: xproto::Keycode,
    pub event_type: u8,
    pub request_major: u8,
    pub request_minor: u8,
}
impl_debug_if_no_extra_traits!(StateNotifyEvent, "StateNotifyEvent");
impl TryParse for StateNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (mods, remaining) = u8::try_parse(remaining)?;
        let (base_mods, remaining) = u8::try_parse(remaining)?;
        let (latched_mods, remaining) = u8::try_parse(remaining)?;
        let (locked_mods, remaining) = u8::try_parse(remaining)?;
        let (group, remaining) = u8::try_parse(remaining)?;
        let (base_group, remaining) = i16::try_parse(remaining)?;
        let (latched_group, remaining) = i16::try_parse(remaining)?;
        let (locked_group, remaining) = u8::try_parse(remaining)?;
        let (compat_state, remaining) = u8::try_parse(remaining)?;
        let (grab_mods, remaining) = u8::try_parse(remaining)?;
        let (compat_grab_mods, remaining) = u8::try_parse(remaining)?;
        let (lookup_mods, remaining) = u8::try_parse(remaining)?;
        let (compat_loockup_mods, remaining) = u8::try_parse(remaining)?;
        let (ptr_btn_state, remaining) = u16::try_parse(remaining)?;
        let (changed, remaining) = u16::try_parse(remaining)?;
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (event_type, remaining) = u8::try_parse(remaining)?;
        let (request_major, remaining) = u8::try_parse(remaining)?;
        let (request_minor, remaining) = u8::try_parse(remaining)?;
        let mods = mods.into();
        let base_mods = base_mods.into();
        let latched_mods = latched_mods.into();
        let locked_mods = locked_mods.into();
        let group = group.into();
        let locked_group = locked_group.into();
        let compat_state = compat_state.into();
        let grab_mods = grab_mods.into();
        let compat_grab_mods = compat_grab_mods.into();
        let lookup_mods = lookup_mods.into();
        let compat_loockup_mods = compat_loockup_mods.into();
        let ptr_btn_state = ptr_btn_state.into();
        let changed = changed.into();
        let result = StateNotifyEvent { response_type, xkb_type, sequence, time, device_id, mods, base_mods, latched_mods, locked_mods, group, base_group, latched_group, locked_group, compat_state, grab_mods, compat_grab_mods, lookup_mods, compat_loockup_mods, ptr_btn_state, changed, keycode, event_type, request_major, request_minor };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for StateNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let mods_bytes = (u16::from(self.mods) as u8).serialize();
        let base_mods_bytes = (u16::from(self.base_mods) as u8).serialize();
        let latched_mods_bytes = (u16::from(self.latched_mods) as u8).serialize();
        let locked_mods_bytes = (u16::from(self.locked_mods) as u8).serialize();
        let group_bytes = u8::from(self.group).serialize();
        let base_group_bytes = self.base_group.serialize();
        let latched_group_bytes = self.latched_group.serialize();
        let locked_group_bytes = u8::from(self.locked_group).serialize();
        let compat_state_bytes = (u16::from(self.compat_state) as u8).serialize();
        let grab_mods_bytes = (u16::from(self.grab_mods) as u8).serialize();
        let compat_grab_mods_bytes = (u16::from(self.compat_grab_mods) as u8).serialize();
        let lookup_mods_bytes = (u16::from(self.lookup_mods) as u8).serialize();
        let compat_loockup_mods_bytes = (u16::from(self.compat_loockup_mods) as u8).serialize();
        let ptr_btn_state_bytes = u16::from(self.ptr_btn_state).serialize();
        let changed_bytes = u16::from(self.changed).serialize();
        let keycode_bytes = self.keycode.serialize();
        let event_type_bytes = self.event_type.serialize();
        let request_major_bytes = self.request_major.serialize();
        let request_minor_bytes = self.request_minor.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            mods_bytes[0],
            base_mods_bytes[0],
            latched_mods_bytes[0],
            locked_mods_bytes[0],
            group_bytes[0],
            base_group_bytes[0],
            base_group_bytes[1],
            latched_group_bytes[0],
            latched_group_bytes[1],
            locked_group_bytes[0],
            compat_state_bytes[0],
            grab_mods_bytes[0],
            compat_grab_mods_bytes[0],
            lookup_mods_bytes[0],
            compat_loockup_mods_bytes[0],
            ptr_btn_state_bytes[0],
            ptr_btn_state_bytes[1],
            changed_bytes[0],
            changed_bytes[1],
            keycode_bytes[0],
            event_type_bytes[0],
            request_major_bytes[0],
            request_minor_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        (u16::from(self.mods) as u8).serialize_into(bytes);
        (u16::from(self.base_mods) as u8).serialize_into(bytes);
        (u16::from(self.latched_mods) as u8).serialize_into(bytes);
        (u16::from(self.locked_mods) as u8).serialize_into(bytes);
        u8::from(self.group).serialize_into(bytes);
        self.base_group.serialize_into(bytes);
        self.latched_group.serialize_into(bytes);
        u8::from(self.locked_group).serialize_into(bytes);
        (u16::from(self.compat_state) as u8).serialize_into(bytes);
        (u16::from(self.grab_mods) as u8).serialize_into(bytes);
        (u16::from(self.compat_grab_mods) as u8).serialize_into(bytes);
        (u16::from(self.lookup_mods) as u8).serialize_into(bytes);
        (u16::from(self.compat_loockup_mods) as u8).serialize_into(bytes);
        u16::from(self.ptr_btn_state).serialize_into(bytes);
        u16::from(self.changed).serialize_into(bytes);
        self.keycode.serialize_into(bytes);
        self.event_type.serialize_into(bytes);
        self.request_major.serialize_into(bytes);
        self.request_minor.serialize_into(bytes);
    }
}
impl From<&StateNotifyEvent> for [u8; 32] {
    fn from(input: &StateNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let mods_bytes = (u16::from(input.mods) as u8).serialize();
        let base_mods_bytes = (u16::from(input.base_mods) as u8).serialize();
        let latched_mods_bytes = (u16::from(input.latched_mods) as u8).serialize();
        let locked_mods_bytes = (u16::from(input.locked_mods) as u8).serialize();
        let group_bytes = u8::from(input.group).serialize();
        let base_group_bytes = input.base_group.serialize();
        let latched_group_bytes = input.latched_group.serialize();
        let locked_group_bytes = u8::from(input.locked_group).serialize();
        let compat_state_bytes = (u16::from(input.compat_state) as u8).serialize();
        let grab_mods_bytes = (u16::from(input.grab_mods) as u8).serialize();
        let compat_grab_mods_bytes = (u16::from(input.compat_grab_mods) as u8).serialize();
        let lookup_mods_bytes = (u16::from(input.lookup_mods) as u8).serialize();
        let compat_loockup_mods_bytes = (u16::from(input.compat_loockup_mods) as u8).serialize();
        let ptr_btn_state_bytes = u16::from(input.ptr_btn_state).serialize();
        let changed_bytes = u16::from(input.changed).serialize();
        let keycode_bytes = input.keycode.serialize();
        let event_type_bytes = input.event_type.serialize();
        let request_major_bytes = input.request_major.serialize();
        let request_minor_bytes = input.request_minor.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            mods_bytes[0],
            base_mods_bytes[0],
            latched_mods_bytes[0],
            locked_mods_bytes[0],
            group_bytes[0],
            base_group_bytes[0],
            base_group_bytes[1],
            latched_group_bytes[0],
            latched_group_bytes[1],
            locked_group_bytes[0],
            compat_state_bytes[0],
            grab_mods_bytes[0],
            compat_grab_mods_bytes[0],
            lookup_mods_bytes[0],
            compat_loockup_mods_bytes[0],
            ptr_btn_state_bytes[0],
            ptr_btn_state_bytes[1],
            changed_bytes[0],
            changed_bytes[1],
            keycode_bytes[0],
            event_type_bytes[0],
            request_major_bytes[0],
            request_minor_bytes[0],
        ]
    }
}
impl From<StateNotifyEvent> for [u8; 32] {
    fn from(input: StateNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ControlsNotify event
pub const CONTROLS_NOTIFY_EVENT: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ControlsNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub num_groups: u8,
    pub changed_controls: Control,
    pub enabled_controls: BoolCtrl,
    pub enabled_control_changes: BoolCtrl,
    pub keycode: xproto::Keycode,
    pub event_type: u8,
    pub request_major: u8,
    pub request_minor: u8,
}
impl_debug_if_no_extra_traits!(ControlsNotifyEvent, "ControlsNotifyEvent");
impl TryParse for ControlsNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (num_groups, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (changed_controls, remaining) = u32::try_parse(remaining)?;
        let (enabled_controls, remaining) = u32::try_parse(remaining)?;
        let (enabled_control_changes, remaining) = u32::try_parse(remaining)?;
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (event_type, remaining) = u8::try_parse(remaining)?;
        let (request_major, remaining) = u8::try_parse(remaining)?;
        let (request_minor, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let changed_controls = changed_controls.into();
        let enabled_controls = enabled_controls.into();
        let enabled_control_changes = enabled_control_changes.into();
        let result = ControlsNotifyEvent { response_type, xkb_type, sequence, time, device_id, num_groups, changed_controls, enabled_controls, enabled_control_changes, keycode, event_type, request_major, request_minor };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ControlsNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let num_groups_bytes = self.num_groups.serialize();
        let changed_controls_bytes = u32::from(self.changed_controls).serialize();
        let enabled_controls_bytes = u32::from(self.enabled_controls).serialize();
        let enabled_control_changes_bytes = u32::from(self.enabled_control_changes).serialize();
        let keycode_bytes = self.keycode.serialize();
        let event_type_bytes = self.event_type.serialize();
        let request_major_bytes = self.request_major.serialize();
        let request_minor_bytes = self.request_minor.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            num_groups_bytes[0],
            0,
            0,
            changed_controls_bytes[0],
            changed_controls_bytes[1],
            changed_controls_bytes[2],
            changed_controls_bytes[3],
            enabled_controls_bytes[0],
            enabled_controls_bytes[1],
            enabled_controls_bytes[2],
            enabled_controls_bytes[3],
            enabled_control_changes_bytes[0],
            enabled_control_changes_bytes[1],
            enabled_control_changes_bytes[2],
            enabled_control_changes_bytes[3],
            keycode_bytes[0],
            event_type_bytes[0],
            request_major_bytes[0],
            request_minor_bytes[0],
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        self.num_groups.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        u32::from(self.changed_controls).serialize_into(bytes);
        u32::from(self.enabled_controls).serialize_into(bytes);
        u32::from(self.enabled_control_changes).serialize_into(bytes);
        self.keycode.serialize_into(bytes);
        self.event_type.serialize_into(bytes);
        self.request_major.serialize_into(bytes);
        self.request_minor.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
    }
}
impl From<&ControlsNotifyEvent> for [u8; 32] {
    fn from(input: &ControlsNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let num_groups_bytes = input.num_groups.serialize();
        let changed_controls_bytes = u32::from(input.changed_controls).serialize();
        let enabled_controls_bytes = u32::from(input.enabled_controls).serialize();
        let enabled_control_changes_bytes = u32::from(input.enabled_control_changes).serialize();
        let keycode_bytes = input.keycode.serialize();
        let event_type_bytes = input.event_type.serialize();
        let request_major_bytes = input.request_major.serialize();
        let request_minor_bytes = input.request_minor.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            num_groups_bytes[0],
            0,
            0,
            changed_controls_bytes[0],
            changed_controls_bytes[1],
            changed_controls_bytes[2],
            changed_controls_bytes[3],
            enabled_controls_bytes[0],
            enabled_controls_bytes[1],
            enabled_controls_bytes[2],
            enabled_controls_bytes[3],
            enabled_control_changes_bytes[0],
            enabled_control_changes_bytes[1],
            enabled_control_changes_bytes[2],
            enabled_control_changes_bytes[3],
            keycode_bytes[0],
            event_type_bytes[0],
            request_major_bytes[0],
            request_minor_bytes[0],
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<ControlsNotifyEvent> for [u8; 32] {
    fn from(input: ControlsNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the IndicatorStateNotify event
pub const INDICATOR_STATE_NOTIFY_EVENT: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndicatorStateNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub state: u32,
    pub state_changed: u32,
}
impl_debug_if_no_extra_traits!(IndicatorStateNotifyEvent, "IndicatorStateNotifyEvent");
impl TryParse for IndicatorStateNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (state, remaining) = u32::try_parse(remaining)?;
        let (state_changed, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let result = IndicatorStateNotifyEvent { response_type, xkb_type, sequence, time, device_id, state, state_changed };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for IndicatorStateNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let state_bytes = self.state.serialize();
        let state_changed_bytes = self.state_changed.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            0,
            0,
            state_bytes[0],
            state_bytes[1],
            state_bytes[2],
            state_bytes[3],
            state_changed_bytes[0],
            state_changed_bytes[1],
            state_changed_bytes[2],
            state_changed_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        self.state.serialize_into(bytes);
        self.state_changed.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
    }
}
impl From<&IndicatorStateNotifyEvent> for [u8; 32] {
    fn from(input: &IndicatorStateNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let state_bytes = input.state.serialize();
        let state_changed_bytes = input.state_changed.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            0,
            0,
            state_bytes[0],
            state_bytes[1],
            state_bytes[2],
            state_bytes[3],
            state_changed_bytes[0],
            state_changed_bytes[1],
            state_changed_bytes[2],
            state_changed_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<IndicatorStateNotifyEvent> for [u8; 32] {
    fn from(input: IndicatorStateNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the IndicatorMapNotify event
pub const INDICATOR_MAP_NOTIFY_EVENT: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndicatorMapNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub state: u32,
    pub map_changed: u32,
}
impl_debug_if_no_extra_traits!(IndicatorMapNotifyEvent, "IndicatorMapNotifyEvent");
impl TryParse for IndicatorMapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (state, remaining) = u32::try_parse(remaining)?;
        let (map_changed, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let result = IndicatorMapNotifyEvent { response_type, xkb_type, sequence, time, device_id, state, map_changed };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for IndicatorMapNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let state_bytes = self.state.serialize();
        let map_changed_bytes = self.map_changed.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            0,
            0,
            state_bytes[0],
            state_bytes[1],
            state_bytes[2],
            state_bytes[3],
            map_changed_bytes[0],
            map_changed_bytes[1],
            map_changed_bytes[2],
            map_changed_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        self.state.serialize_into(bytes);
        self.map_changed.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
    }
}
impl From<&IndicatorMapNotifyEvent> for [u8; 32] {
    fn from(input: &IndicatorMapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let state_bytes = input.state.serialize();
        let map_changed_bytes = input.map_changed.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            0,
            0,
            state_bytes[0],
            state_bytes[1],
            state_bytes[2],
            state_bytes[3],
            map_changed_bytes[0],
            map_changed_bytes[1],
            map_changed_bytes[2],
            map_changed_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<IndicatorMapNotifyEvent> for [u8; 32] {
    fn from(input: IndicatorMapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the NamesNotify event
pub const NAMES_NOTIFY_EVENT: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamesNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub changed: NameDetail,
    pub first_type: u8,
    pub n_types: u8,
    pub first_level_name: u8,
    pub n_level_names: u8,
    pub n_radio_groups: u8,
    pub n_key_aliases: u8,
    pub changed_group_names: SetOfGroup,
    pub changed_virtual_mods: VMod,
    pub first_key: xproto::Keycode,
    pub n_keys: u8,
    pub changed_indicators: u32,
}
impl_debug_if_no_extra_traits!(NamesNotifyEvent, "NamesNotifyEvent");
impl TryParse for NamesNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (changed, remaining) = u16::try_parse(remaining)?;
        let (first_type, remaining) = u8::try_parse(remaining)?;
        let (n_types, remaining) = u8::try_parse(remaining)?;
        let (first_level_name, remaining) = u8::try_parse(remaining)?;
        let (n_level_names, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (n_radio_groups, remaining) = u8::try_parse(remaining)?;
        let (n_key_aliases, remaining) = u8::try_parse(remaining)?;
        let (changed_group_names, remaining) = u8::try_parse(remaining)?;
        let (changed_virtual_mods, remaining) = u16::try_parse(remaining)?;
        let (first_key, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (n_keys, remaining) = u8::try_parse(remaining)?;
        let (changed_indicators, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let changed = changed.into();
        let changed_group_names = changed_group_names.into();
        let changed_virtual_mods = changed_virtual_mods.into();
        let result = NamesNotifyEvent { response_type, xkb_type, sequence, time, device_id, changed, first_type, n_types, first_level_name, n_level_names, n_radio_groups, n_key_aliases, changed_group_names, changed_virtual_mods, first_key, n_keys, changed_indicators };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for NamesNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let changed_bytes = (u32::from(self.changed) as u16).serialize();
        let first_type_bytes = self.first_type.serialize();
        let n_types_bytes = self.n_types.serialize();
        let first_level_name_bytes = self.first_level_name.serialize();
        let n_level_names_bytes = self.n_level_names.serialize();
        let n_radio_groups_bytes = self.n_radio_groups.serialize();
        let n_key_aliases_bytes = self.n_key_aliases.serialize();
        let changed_group_names_bytes = u8::from(self.changed_group_names).serialize();
        let changed_virtual_mods_bytes = u16::from(self.changed_virtual_mods).serialize();
        let first_key_bytes = self.first_key.serialize();
        let n_keys_bytes = self.n_keys.serialize();
        let changed_indicators_bytes = self.changed_indicators.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            changed_bytes[0],
            changed_bytes[1],
            first_type_bytes[0],
            n_types_bytes[0],
            first_level_name_bytes[0],
            n_level_names_bytes[0],
            0,
            n_radio_groups_bytes[0],
            n_key_aliases_bytes[0],
            changed_group_names_bytes[0],
            changed_virtual_mods_bytes[0],
            changed_virtual_mods_bytes[1],
            first_key_bytes[0],
            n_keys_bytes[0],
            changed_indicators_bytes[0],
            changed_indicators_bytes[1],
            changed_indicators_bytes[2],
            changed_indicators_bytes[3],
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        (u32::from(self.changed) as u16).serialize_into(bytes);
        self.first_type.serialize_into(bytes);
        self.n_types.serialize_into(bytes);
        self.first_level_name.serialize_into(bytes);
        self.n_level_names.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.n_radio_groups.serialize_into(bytes);
        self.n_key_aliases.serialize_into(bytes);
        u8::from(self.changed_group_names).serialize_into(bytes);
        u16::from(self.changed_virtual_mods).serialize_into(bytes);
        self.first_key.serialize_into(bytes);
        self.n_keys.serialize_into(bytes);
        self.changed_indicators.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
    }
}
impl From<&NamesNotifyEvent> for [u8; 32] {
    fn from(input: &NamesNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let changed_bytes = (u32::from(input.changed) as u16).serialize();
        let first_type_bytes = input.first_type.serialize();
        let n_types_bytes = input.n_types.serialize();
        let first_level_name_bytes = input.first_level_name.serialize();
        let n_level_names_bytes = input.n_level_names.serialize();
        let n_radio_groups_bytes = input.n_radio_groups.serialize();
        let n_key_aliases_bytes = input.n_key_aliases.serialize();
        let changed_group_names_bytes = u8::from(input.changed_group_names).serialize();
        let changed_virtual_mods_bytes = u16::from(input.changed_virtual_mods).serialize();
        let first_key_bytes = input.first_key.serialize();
        let n_keys_bytes = input.n_keys.serialize();
        let changed_indicators_bytes = input.changed_indicators.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            changed_bytes[0],
            changed_bytes[1],
            first_type_bytes[0],
            n_types_bytes[0],
            first_level_name_bytes[0],
            n_level_names_bytes[0],
            0,
            n_radio_groups_bytes[0],
            n_key_aliases_bytes[0],
            changed_group_names_bytes[0],
            changed_virtual_mods_bytes[0],
            changed_virtual_mods_bytes[1],
            first_key_bytes[0],
            n_keys_bytes[0],
            changed_indicators_bytes[0],
            changed_indicators_bytes[1],
            changed_indicators_bytes[2],
            changed_indicators_bytes[3],
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<NamesNotifyEvent> for [u8; 32] {
    fn from(input: NamesNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the CompatMapNotify event
pub const COMPAT_MAP_NOTIFY_EVENT: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompatMapNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub changed_groups: SetOfGroup,
    pub first_si: u16,
    pub n_si: u16,
    pub n_total_si: u16,
}
impl_debug_if_no_extra_traits!(CompatMapNotifyEvent, "CompatMapNotifyEvent");
impl TryParse for CompatMapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (changed_groups, remaining) = u8::try_parse(remaining)?;
        let (first_si, remaining) = u16::try_parse(remaining)?;
        let (n_si, remaining) = u16::try_parse(remaining)?;
        let (n_total_si, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let changed_groups = changed_groups.into();
        let result = CompatMapNotifyEvent { response_type, xkb_type, sequence, time, device_id, changed_groups, first_si, n_si, n_total_si };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CompatMapNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let changed_groups_bytes = u8::from(self.changed_groups).serialize();
        let first_si_bytes = self.first_si.serialize();
        let n_si_bytes = self.n_si.serialize();
        let n_total_si_bytes = self.n_total_si.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            changed_groups_bytes[0],
            first_si_bytes[0],
            first_si_bytes[1],
            n_si_bytes[0],
            n_si_bytes[1],
            n_total_si_bytes[0],
            n_total_si_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        u8::from(self.changed_groups).serialize_into(bytes);
        self.first_si.serialize_into(bytes);
        self.n_si.serialize_into(bytes);
        self.n_total_si.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
    }
}
impl From<&CompatMapNotifyEvent> for [u8; 32] {
    fn from(input: &CompatMapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let changed_groups_bytes = u8::from(input.changed_groups).serialize();
        let first_si_bytes = input.first_si.serialize();
        let n_si_bytes = input.n_si.serialize();
        let n_total_si_bytes = input.n_total_si.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            changed_groups_bytes[0],
            first_si_bytes[0],
            first_si_bytes[1],
            n_si_bytes[0],
            n_si_bytes[1],
            n_total_si_bytes[0],
            n_total_si_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<CompatMapNotifyEvent> for [u8; 32] {
    fn from(input: CompatMapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the BellNotify event
pub const BELL_NOTIFY_EVENT: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BellNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub bell_class: BellClassResult,
    pub bell_id: u8,
    pub percent: u8,
    pub pitch: u16,
    pub duration: u16,
    pub name: xproto::Atom,
    pub window: xproto::Window,
    pub event_only: bool,
}
impl_debug_if_no_extra_traits!(BellNotifyEvent, "BellNotifyEvent");
impl TryParse for BellNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (bell_class, remaining) = u8::try_parse(remaining)?;
        let (bell_id, remaining) = u8::try_parse(remaining)?;
        let (percent, remaining) = u8::try_parse(remaining)?;
        let (pitch, remaining) = u16::try_parse(remaining)?;
        let (duration, remaining) = u16::try_parse(remaining)?;
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (event_only, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(7..).ok_or(ParseError::InsufficientData)?;
        let bell_class = bell_class.into();
        let result = BellNotifyEvent { response_type, xkb_type, sequence, time, device_id, bell_class, bell_id, percent, pitch, duration, name, window, event_only };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for BellNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let bell_class_bytes = u8::from(self.bell_class).serialize();
        let bell_id_bytes = self.bell_id.serialize();
        let percent_bytes = self.percent.serialize();
        let pitch_bytes = self.pitch.serialize();
        let duration_bytes = self.duration.serialize();
        let name_bytes = self.name.serialize();
        let window_bytes = self.window.serialize();
        let event_only_bytes = self.event_only.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            bell_class_bytes[0],
            bell_id_bytes[0],
            percent_bytes[0],
            pitch_bytes[0],
            pitch_bytes[1],
            duration_bytes[0],
            duration_bytes[1],
            name_bytes[0],
            name_bytes[1],
            name_bytes[2],
            name_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            event_only_bytes[0],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        u8::from(self.bell_class).serialize_into(bytes);
        self.bell_id.serialize_into(bytes);
        self.percent.serialize_into(bytes);
        self.pitch.serialize_into(bytes);
        self.duration.serialize_into(bytes);
        self.name.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.event_only.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 7]);
    }
}
impl From<&BellNotifyEvent> for [u8; 32] {
    fn from(input: &BellNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let bell_class_bytes = u8::from(input.bell_class).serialize();
        let bell_id_bytes = input.bell_id.serialize();
        let percent_bytes = input.percent.serialize();
        let pitch_bytes = input.pitch.serialize();
        let duration_bytes = input.duration.serialize();
        let name_bytes = input.name.serialize();
        let window_bytes = input.window.serialize();
        let event_only_bytes = input.event_only.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            bell_class_bytes[0],
            bell_id_bytes[0],
            percent_bytes[0],
            pitch_bytes[0],
            pitch_bytes[1],
            duration_bytes[0],
            duration_bytes[1],
            name_bytes[0],
            name_bytes[1],
            name_bytes[2],
            name_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            event_only_bytes[0],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<BellNotifyEvent> for [u8; 32] {
    fn from(input: BellNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ActionMessage event
pub const ACTION_MESSAGE_EVENT: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionMessageEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub keycode: xproto::Keycode,
    pub press: bool,
    pub key_event_follows: bool,
    pub mods: xproto::ModMask,
    pub group: Group,
    pub message: [String8; 8],
}
impl_debug_if_no_extra_traits!(ActionMessageEvent, "ActionMessageEvent");
impl TryParse for ActionMessageEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (press, remaining) = bool::try_parse(remaining)?;
        let (key_event_follows, remaining) = bool::try_parse(remaining)?;
        let (mods, remaining) = u8::try_parse(remaining)?;
        let (group, remaining) = u8::try_parse(remaining)?;
        let (message, remaining) = crate::x11_utils::parse_u8_array::<8>(remaining)?;
        let remaining = remaining.get(10..).ok_or(ParseError::InsufficientData)?;
        let mods = mods.into();
        let group = group.into();
        let result = ActionMessageEvent { response_type, xkb_type, sequence, time, device_id, keycode, press, key_event_follows, mods, group, message };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ActionMessageEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let keycode_bytes = self.keycode.serialize();
        let press_bytes = self.press.serialize();
        let key_event_follows_bytes = self.key_event_follows.serialize();
        let mods_bytes = (u16::from(self.mods) as u8).serialize();
        let group_bytes = u8::from(self.group).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            keycode_bytes[0],
            press_bytes[0],
            key_event_follows_bytes[0],
            mods_bytes[0],
            group_bytes[0],
            self.message[0],
            self.message[1],
            self.message[2],
            self.message[3],
            self.message[4],
            self.message[5],
            self.message[6],
            self.message[7],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        self.keycode.serialize_into(bytes);
        self.press.serialize_into(bytes);
        self.key_event_follows.serialize_into(bytes);
        (u16::from(self.mods) as u8).serialize_into(bytes);
        u8::from(self.group).serialize_into(bytes);
        bytes.extend_from_slice(&self.message);
        bytes.extend_from_slice(&[0; 10]);
    }
}
impl From<&ActionMessageEvent> for [u8; 32] {
    fn from(input: &ActionMessageEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let keycode_bytes = input.keycode.serialize();
        let press_bytes = input.press.serialize();
        let key_event_follows_bytes = input.key_event_follows.serialize();
        let mods_bytes = (u16::from(input.mods) as u8).serialize();
        let group_bytes = u8::from(input.group).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            keycode_bytes[0],
            press_bytes[0],
            key_event_follows_bytes[0],
            mods_bytes[0],
            group_bytes[0],
            input.message[0],
            input.message[1],
            input.message[2],
            input.message[3],
            input.message[4],
            input.message[5],
            input.message[6],
            input.message[7],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<ActionMessageEvent> for [u8; 32] {
    fn from(input: ActionMessageEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the AccessXNotify event
pub const ACCESS_X_NOTIFY_EVENT: u8 = 10;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccessXNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub keycode: xproto::Keycode,
    pub detailt: AXNDetail,
    pub slow_keys_delay: u16,
    pub debounce_delay: u16,
}
impl_debug_if_no_extra_traits!(AccessXNotifyEvent, "AccessXNotifyEvent");
impl TryParse for AccessXNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let (keycode, remaining) = xproto::Keycode::try_parse(remaining)?;
        let (detailt, remaining) = u16::try_parse(remaining)?;
        let (slow_keys_delay, remaining) = u16::try_parse(remaining)?;
        let (debounce_delay, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let detailt = detailt.into();
        let result = AccessXNotifyEvent { response_type, xkb_type, sequence, time, device_id, keycode, detailt, slow_keys_delay, debounce_delay };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AccessXNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let keycode_bytes = self.keycode.serialize();
        let detailt_bytes = u16::from(self.detailt).serialize();
        let slow_keys_delay_bytes = self.slow_keys_delay.serialize();
        let debounce_delay_bytes = self.debounce_delay.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            keycode_bytes[0],
            detailt_bytes[0],
            detailt_bytes[1],
            slow_keys_delay_bytes[0],
            slow_keys_delay_bytes[1],
            debounce_delay_bytes[0],
            debounce_delay_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        self.keycode.serialize_into(bytes);
        u16::from(self.detailt).serialize_into(bytes);
        self.slow_keys_delay.serialize_into(bytes);
        self.debounce_delay.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
    }
}
impl From<&AccessXNotifyEvent> for [u8; 32] {
    fn from(input: &AccessXNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let keycode_bytes = input.keycode.serialize();
        let detailt_bytes = u16::from(input.detailt).serialize();
        let slow_keys_delay_bytes = input.slow_keys_delay.serialize();
        let debounce_delay_bytes = input.debounce_delay.serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            keycode_bytes[0],
            detailt_bytes[0],
            detailt_bytes[1],
            slow_keys_delay_bytes[0],
            slow_keys_delay_bytes[1],
            debounce_delay_bytes[0],
            debounce_delay_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<AccessXNotifyEvent> for [u8; 32] {
    fn from(input: AccessXNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ExtensionDeviceNotify event
pub const EXTENSION_DEVICE_NOTIFY_EVENT: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtensionDeviceNotifyEvent {
    pub response_type: u8,
    pub xkb_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub device_id: u8,
    pub reason: XIFeature,
    pub led_class: LedClassResult,
    pub led_id: u16,
    pub leds_defined: u32,
    pub led_state: u32,
    pub first_button: u8,
    pub n_buttons: u8,
    pub supported: XIFeature,
    pub unsupported: XIFeature,
}
impl_debug_if_no_extra_traits!(ExtensionDeviceNotifyEvent, "ExtensionDeviceNotifyEvent");
impl TryParse for ExtensionDeviceNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (xkb_type, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (device_id, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (reason, remaining) = u16::try_parse(remaining)?;
        let (led_class, remaining) = u16::try_parse(remaining)?;
        let (led_id, remaining) = u16::try_parse(remaining)?;
        let (leds_defined, remaining) = u32::try_parse(remaining)?;
        let (led_state, remaining) = u32::try_parse(remaining)?;
        let (first_button, remaining) = u8::try_parse(remaining)?;
        let (n_buttons, remaining) = u8::try_parse(remaining)?;
        let (supported, remaining) = u16::try_parse(remaining)?;
        let (unsupported, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let reason = reason.into();
        let led_class = led_class.into();
        let supported = supported.into();
        let unsupported = unsupported.into();
        let result = ExtensionDeviceNotifyEvent { response_type, xkb_type, sequence, time, device_id, reason, led_class, led_id, leds_defined, led_state, first_button, n_buttons, supported, unsupported };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ExtensionDeviceNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let xkb_type_bytes = self.xkb_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let device_id_bytes = self.device_id.serialize();
        let reason_bytes = u16::from(self.reason).serialize();
        let led_class_bytes = u16::from(self.led_class).serialize();
        let led_id_bytes = self.led_id.serialize();
        let leds_defined_bytes = self.leds_defined.serialize();
        let led_state_bytes = self.led_state.serialize();
        let first_button_bytes = self.first_button.serialize();
        let n_buttons_bytes = self.n_buttons.serialize();
        let supported_bytes = u16::from(self.supported).serialize();
        let unsupported_bytes = u16::from(self.unsupported).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            reason_bytes[0],
            reason_bytes[1],
            led_class_bytes[0],
            led_class_bytes[1],
            led_id_bytes[0],
            led_id_bytes[1],
            leds_defined_bytes[0],
            leds_defined_bytes[1],
            leds_defined_bytes[2],
            leds_defined_bytes[3],
            led_state_bytes[0],
            led_state_bytes[1],
            led_state_bytes[2],
            led_state_bytes[3],
            first_button_bytes[0],
            n_buttons_bytes[0],
            supported_bytes[0],
            supported_bytes[1],
            unsupported_bytes[0],
            unsupported_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.xkb_type.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.device_id.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        u16::from(self.reason).serialize_into(bytes);
        u16::from(self.led_class).serialize_into(bytes);
        self.led_id.serialize_into(bytes);
        self.leds_defined.serialize_into(bytes);
        self.led_state.serialize_into(bytes);
        self.first_button.serialize_into(bytes);
        self.n_buttons.serialize_into(bytes);
        u16::from(self.supported).serialize_into(bytes);
        u16::from(self.unsupported).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}
impl From<&ExtensionDeviceNotifyEvent> for [u8; 32] {
    fn from(input: &ExtensionDeviceNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let xkb_type_bytes = input.xkb_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let device_id_bytes = input.device_id.serialize();
        let reason_bytes = u16::from(input.reason).serialize();
        let led_class_bytes = u16::from(input.led_class).serialize();
        let led_id_bytes = input.led_id.serialize();
        let leds_defined_bytes = input.leds_defined.serialize();
        let led_state_bytes = input.led_state.serialize();
        let first_button_bytes = input.first_button.serialize();
        let n_buttons_bytes = input.n_buttons.serialize();
        let supported_bytes = u16::from(input.supported).serialize();
        let unsupported_bytes = u16::from(input.unsupported).serialize();
        [
            response_type_bytes[0],
            xkb_type_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            device_id_bytes[0],
            0,
            reason_bytes[0],
            reason_bytes[1],
            led_class_bytes[0],
            led_class_bytes[1],
            led_id_bytes[0],
            led_id_bytes[1],
            leds_defined_bytes[0],
            leds_defined_bytes[1],
            leds_defined_bytes[2],
            leds_defined_bytes[3],
            led_state_bytes[0],
            led_state_bytes[1],
            led_state_bytes[2],
            led_state_bytes[3],
            first_button_bytes[0],
            n_buttons_bytes[0],
            supported_bytes[0],
            supported_bytes[1],
            unsupported_bytes[0],
            unsupported_bytes[1],
            0,
            0,
        ]
    }
}
impl From<ExtensionDeviceNotifyEvent> for [u8; 32] {
    fn from(input: ExtensionDeviceNotifyEvent) -> Self {
        Self::from(&input)
    }
}

