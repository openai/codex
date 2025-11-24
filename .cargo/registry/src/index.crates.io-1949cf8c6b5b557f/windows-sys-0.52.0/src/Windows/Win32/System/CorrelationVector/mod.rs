::windows_targets::link!("ntdll.dll" "system" fn RtlExtendCorrelationVector(correlationvector : *mut CORRELATION_VECTOR) -> u32);
::windows_targets::link!("ntdll.dll" "system" fn RtlIncrementCorrelationVector(correlationvector : *mut CORRELATION_VECTOR) -> u32);
::windows_targets::link!("ntdll.dll" "system" fn RtlInitializeCorrelationVector(correlationvector : *mut CORRELATION_VECTOR, version : i32, guid : *const ::windows_sys::core::GUID) -> u32);
::windows_targets::link!("ntdll.dll" "system" fn RtlValidateCorrelationVector(vector : *const CORRELATION_VECTOR) -> u32);
pub const RTL_CORRELATION_VECTOR_STRING_LENGTH: u32 = 129u32;
pub const RTL_CORRELATION_VECTOR_V1_LENGTH: u32 = 64u32;
pub const RTL_CORRELATION_VECTOR_V1_PREFIX_LENGTH: u32 = 16u32;
pub const RTL_CORRELATION_VECTOR_V2_LENGTH: u32 = 128u32;
pub const RTL_CORRELATION_VECTOR_V2_PREFIX_LENGTH: u32 = 22u32;
#[repr(C)]
pub struct CORRELATION_VECTOR {
    pub Version: u8,
    pub Vector: [u8; 129],
}
impl ::core::marker::Copy for CORRELATION_VECTOR {}
impl ::core::clone::Clone for CORRELATION_VECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
