#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseCompressor(compressorhandle : COMPRESSOR_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseDecompressor(decompressorhandle : isize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Compress(compressorhandle : COMPRESSOR_HANDLE, uncompresseddata : *const ::core::ffi::c_void, uncompresseddatasize : usize, compressedbuffer : *mut ::core::ffi::c_void, compressedbuffersize : usize, compresseddatasize : *mut usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateCompressor(algorithm : COMPRESS_ALGORITHM, allocationroutines : *const COMPRESS_ALLOCATION_ROUTINES, compressorhandle : *mut COMPRESSOR_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateDecompressor(algorithm : COMPRESS_ALGORITHM, allocationroutines : *const COMPRESS_ALLOCATION_ROUTINES, decompressorhandle : *mut isize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Decompress(decompressorhandle : isize, compresseddata : *const ::core::ffi::c_void, compresseddatasize : usize, uncompressedbuffer : *mut ::core::ffi::c_void, uncompressedbuffersize : usize, uncompresseddatasize : *mut usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QueryCompressorInformation(compressorhandle : COMPRESSOR_HANDLE, compressinformationclass : COMPRESS_INFORMATION_CLASS, compressinformation : *mut ::core::ffi::c_void, compressinformationsize : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QueryDecompressorInformation(decompressorhandle : isize, compressinformationclass : COMPRESS_INFORMATION_CLASS, compressinformation : *mut ::core::ffi::c_void, compressinformationsize : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResetCompressor(compressorhandle : COMPRESSOR_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResetDecompressor(decompressorhandle : isize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCompressorInformation(compressorhandle : COMPRESSOR_HANDLE, compressinformationclass : COMPRESS_INFORMATION_CLASS, compressinformation : *const ::core::ffi::c_void, compressinformationsize : usize) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cabinet.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetDecompressorInformation(decompressorhandle : isize, compressinformationclass : COMPRESS_INFORMATION_CLASS, compressinformation : *const ::core::ffi::c_void, compressinformationsize : usize) -> super::super::Foundation:: BOOL);
pub const COMPRESS_ALGORITHM_INVALID: u32 = 0u32;
pub const COMPRESS_ALGORITHM_LZMS: COMPRESS_ALGORITHM = 5u32;
pub const COMPRESS_ALGORITHM_MAX: u32 = 6u32;
pub const COMPRESS_ALGORITHM_MSZIP: COMPRESS_ALGORITHM = 2u32;
pub const COMPRESS_ALGORITHM_NULL: u32 = 1u32;
pub const COMPRESS_ALGORITHM_XPRESS: COMPRESS_ALGORITHM = 3u32;
pub const COMPRESS_ALGORITHM_XPRESS_HUFF: COMPRESS_ALGORITHM = 4u32;
pub const COMPRESS_INFORMATION_CLASS_BLOCK_SIZE: COMPRESS_INFORMATION_CLASS = 1i32;
pub const COMPRESS_INFORMATION_CLASS_INVALID: COMPRESS_INFORMATION_CLASS = 0i32;
pub const COMPRESS_INFORMATION_CLASS_LEVEL: COMPRESS_INFORMATION_CLASS = 2i32;
pub const COMPRESS_RAW: u32 = 536870912u32;
pub type COMPRESS_ALGORITHM = u32;
pub type COMPRESS_INFORMATION_CLASS = i32;
pub type COMPRESSOR_HANDLE = isize;
#[repr(C)]
pub struct COMPRESS_ALLOCATION_ROUTINES {
    pub Allocate: PFN_COMPRESS_ALLOCATE,
    pub Free: PFN_COMPRESS_FREE,
    pub UserContext: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for COMPRESS_ALLOCATION_ROUTINES {}
impl ::core::clone::Clone for COMPRESS_ALLOCATION_ROUTINES {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PFN_COMPRESS_ALLOCATE = ::core::option::Option<unsafe extern "system" fn(usercontext: *const ::core::ffi::c_void, size: usize) -> *mut ::core::ffi::c_void>;
pub type PFN_COMPRESS_FREE = ::core::option::Option<unsafe extern "system" fn(usercontext: *const ::core::ffi::c_void, memory: *const ::core::ffi::c_void) -> ()>;
