#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OperationEnd(operationendparams : *const OPERATION_END_PARAMETERS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OperationStart(operationstartparams : *const OPERATION_START_PARAMETERS) -> super::super::Foundation:: BOOL);
pub const OPERATION_END_DISCARD: OPERATION_END_PARAMETERS_FLAGS = 1u32;
pub const OPERATION_START_TRACE_CURRENT_THREAD: OPERATION_START_FLAGS = 1u32;
pub type OPERATION_END_PARAMETERS_FLAGS = u32;
pub type OPERATION_START_FLAGS = u32;
#[repr(C)]
pub struct OPERATION_END_PARAMETERS {
    pub Version: u32,
    pub OperationId: u32,
    pub Flags: OPERATION_END_PARAMETERS_FLAGS,
}
impl ::core::marker::Copy for OPERATION_END_PARAMETERS {}
impl ::core::clone::Clone for OPERATION_END_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OPERATION_START_PARAMETERS {
    pub Version: u32,
    pub OperationId: u32,
    pub Flags: OPERATION_START_FLAGS,
}
impl ::core::marker::Copy for OPERATION_START_PARAMETERS {}
impl ::core::clone::Clone for OPERATION_START_PARAMETERS {
    fn clone(&self) -> Self {
        *self
    }
}
