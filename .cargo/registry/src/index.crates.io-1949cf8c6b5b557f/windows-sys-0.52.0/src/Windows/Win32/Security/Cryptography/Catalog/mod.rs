#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminAcquireContext(phcatadmin : *mut isize, pgsubsystem : *const ::windows_sys::core::GUID, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminAcquireContext2(phcatadmin : *mut isize, pgsubsystem : *const ::windows_sys::core::GUID, pwszhashalgorithm : ::windows_sys::core::PCWSTR, pstronghashpolicy : *const super:: CERT_STRONG_SIGN_PARA, dwflags : u32) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("wintrust.dll" "system" fn CryptCATAdminAddCatalog(hcatadmin : isize, pwszcatalogfile : ::windows_sys::core::PCWSTR, pwszselectbasename : ::windows_sys::core::PCWSTR, dwflags : u32) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminCalcHashFromFileHandle(hfile : super::super::super::Foundation:: HANDLE, pcbhash : *mut u32, pbhash : *mut u8, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminCalcHashFromFileHandle2(hcatadmin : isize, hfile : super::super::super::Foundation:: HANDLE, pcbhash : *mut u32, pbhash : *mut u8, dwflags : u32) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("wintrust.dll" "system" fn CryptCATAdminEnumCatalogFromHash(hcatadmin : isize, pbhash : *const u8, cbhash : u32, dwflags : u32, phprevcatinfo : *mut isize) -> isize);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminPauseServiceForBackup(dwflags : u32, fresume : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminReleaseCatalogContext(hcatadmin : isize, hcatinfo : isize, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminReleaseContext(hcatadmin : isize, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminRemoveCatalog(hcatadmin : isize, pwszcatalogfile : ::windows_sys::core::PCWSTR, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATAdminResolveCatalogPath(hcatadmin : isize, pwszcatalogfile : ::windows_sys::core::PCWSTR, pscatinfo : *mut CATALOG_INFO, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATAllocSortedMemberInfo(hcatalog : super::super::super::Foundation:: HANDLE, pwszreferencetag : ::windows_sys::core::PCWSTR) -> *mut CRYPTCATMEMBER);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATCDFClose(pcdf : *mut CRYPTCATCDF) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATCDFEnumAttributes(pcdf : *mut CRYPTCATCDF, pmember : *mut CRYPTCATMEMBER, pprevattr : *mut CRYPTCATATTRIBUTE, pfnparseerror : PFN_CDF_PARSE_ERROR_CALLBACK) -> *mut CRYPTCATATTRIBUTE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATCDFEnumCatAttributes(pcdf : *mut CRYPTCATCDF, pprevattr : *mut CRYPTCATATTRIBUTE, pfnparseerror : PFN_CDF_PARSE_ERROR_CALLBACK) -> *mut CRYPTCATATTRIBUTE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATCDFEnumMembers(pcdf : *mut CRYPTCATCDF, pprevmember : *mut CRYPTCATMEMBER, pfnparseerror : PFN_CDF_PARSE_ERROR_CALLBACK) -> *mut CRYPTCATMEMBER);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATCDFOpen(pwszfilepath : ::windows_sys::core::PCWSTR, pfnparseerror : PFN_CDF_PARSE_ERROR_CALLBACK) -> *mut CRYPTCATCDF);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATCatalogInfoFromContext(hcatinfo : isize, pscatinfo : *mut CATALOG_INFO, dwflags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATClose(hcatalog : super::super::super::Foundation:: HANDLE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATEnumerateAttr(hcatalog : super::super::super::Foundation:: HANDLE, pcatmember : *mut CRYPTCATMEMBER, pprevattr : *mut CRYPTCATATTRIBUTE) -> *mut CRYPTCATATTRIBUTE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATEnumerateCatAttr(hcatalog : super::super::super::Foundation:: HANDLE, pprevattr : *mut CRYPTCATATTRIBUTE) -> *mut CRYPTCATATTRIBUTE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATEnumerateMember(hcatalog : super::super::super::Foundation:: HANDLE, pprevmember : *mut CRYPTCATMEMBER) -> *mut CRYPTCATMEMBER);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATFreeSortedMemberInfo(hcatalog : super::super::super::Foundation:: HANDLE, pcatmember : *mut CRYPTCATMEMBER) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATGetAttrInfo(hcatalog : super::super::super::Foundation:: HANDLE, pcatmember : *mut CRYPTCATMEMBER, pwszreferencetag : ::windows_sys::core::PCWSTR) -> *mut CRYPTCATATTRIBUTE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATGetCatAttrInfo(hcatalog : super::super::super::Foundation:: HANDLE, pwszreferencetag : ::windows_sys::core::PCWSTR) -> *mut CRYPTCATATTRIBUTE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATGetMemberInfo(hcatalog : super::super::super::Foundation:: HANDLE, pwszreferencetag : ::windows_sys::core::PCWSTR) -> *mut CRYPTCATMEMBER);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATHandleFromStore(pcatstore : *mut CRYPTCATSTORE) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATOpen(pwszfilename : ::windows_sys::core::PCWSTR, fdwopenflags : CRYPTCAT_OPEN_FLAGS, hprov : usize, dwpublicversion : CRYPTCAT_VERSION, dwencodingtype : u32) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATPersistStore(hcatalog : super::super::super::Foundation:: HANDLE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATPutAttrInfo(hcatalog : super::super::super::Foundation:: HANDLE, pcatmember : *mut CRYPTCATMEMBER, pwszreferencetag : ::windows_sys::core::PCWSTR, dwattrtypeandaction : u32, cbdata : u32, pbdata : *mut u8) -> *mut CRYPTCATATTRIBUTE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATPutCatAttrInfo(hcatalog : super::super::super::Foundation:: HANDLE, pwszreferencetag : ::windows_sys::core::PCWSTR, dwattrtypeandaction : u32, cbdata : u32, pbdata : *mut u8) -> *mut CRYPTCATATTRIBUTE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"] fn CryptCATPutMemberInfo(hcatalog : super::super::super::Foundation:: HANDLE, pwszfilename : ::windows_sys::core::PCWSTR, pwszreferencetag : ::windows_sys::core::PCWSTR, pgsubjecttype : *mut ::windows_sys::core::GUID, dwcertversion : u32, cbsipindirectdata : u32, pbsipindirectdata : *mut u8) -> *mut CRYPTCATMEMBER);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CryptCATStoreFromHandle(hcatalog : super::super::super::Foundation:: HANDLE) -> *mut CRYPTCATSTORE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wintrust.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsCatalogFile(hfile : super::super::super::Foundation:: HANDLE, pwszfilename : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOL);
pub const CRYPTCAT_ADDCATALOG_HARDLINK: u32 = 1u32;
pub const CRYPTCAT_ADDCATALOG_NONE: u32 = 0u32;
pub const CRYPTCAT_ATTR_AUTHENTICATED: u32 = 268435456u32;
pub const CRYPTCAT_ATTR_DATAASCII: u32 = 65536u32;
pub const CRYPTCAT_ATTR_DATABASE64: u32 = 131072u32;
pub const CRYPTCAT_ATTR_DATAREPLACE: u32 = 262144u32;
pub const CRYPTCAT_ATTR_NAMEASCII: u32 = 1u32;
pub const CRYPTCAT_ATTR_NAMEOBJID: u32 = 2u32;
pub const CRYPTCAT_ATTR_NO_AUTO_COMPAT_ENTRY: u32 = 16777216u32;
pub const CRYPTCAT_ATTR_UNAUTHENTICATED: u32 = 536870912u32;
pub const CRYPTCAT_E_AREA_ATTRIBUTE: u32 = 131072u32;
pub const CRYPTCAT_E_AREA_HEADER: u32 = 0u32;
pub const CRYPTCAT_E_AREA_MEMBER: u32 = 65536u32;
pub const CRYPTCAT_E_CDF_ATTR_TOOFEWVALUES: u32 = 131074u32;
pub const CRYPTCAT_E_CDF_ATTR_TYPECOMBO: u32 = 131076u32;
pub const CRYPTCAT_E_CDF_BAD_GUID_CONV: u32 = 131073u32;
pub const CRYPTCAT_E_CDF_DUPLICATE: u32 = 2u32;
pub const CRYPTCAT_E_CDF_MEMBER_FILENOTFOUND: u32 = 65540u32;
pub const CRYPTCAT_E_CDF_MEMBER_FILE_PATH: u32 = 65537u32;
pub const CRYPTCAT_E_CDF_MEMBER_INDIRECTDATA: u32 = 65538u32;
pub const CRYPTCAT_E_CDF_TAGNOTFOUND: u32 = 4u32;
pub const CRYPTCAT_E_CDF_UNSUPPORTED: u32 = 1u32;
pub const CRYPTCAT_FILEEXT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CAT");
pub const CRYPTCAT_MAX_MEMBERTAG: u32 = 64u32;
pub const CRYPTCAT_MEMBER_SORTED: u32 = 1073741824u32;
pub const CRYPTCAT_OPEN_ALWAYS: CRYPTCAT_OPEN_FLAGS = 2u32;
pub const CRYPTCAT_OPEN_CREATENEW: CRYPTCAT_OPEN_FLAGS = 1u32;
pub const CRYPTCAT_OPEN_EXCLUDE_PAGE_HASHES: CRYPTCAT_OPEN_FLAGS = 65536u32;
pub const CRYPTCAT_OPEN_EXISTING: CRYPTCAT_OPEN_FLAGS = 4u32;
pub const CRYPTCAT_OPEN_FLAGS_MASK: CRYPTCAT_OPEN_FLAGS = 4294901760u32;
pub const CRYPTCAT_OPEN_INCLUDE_PAGE_HASHES: CRYPTCAT_OPEN_FLAGS = 131072u32;
pub const CRYPTCAT_OPEN_NO_CONTENT_HCRYPTMSG: CRYPTCAT_OPEN_FLAGS = 536870912u32;
pub const CRYPTCAT_OPEN_SORTED: CRYPTCAT_OPEN_FLAGS = 1073741824u32;
pub const CRYPTCAT_OPEN_VERIFYSIGHASH: CRYPTCAT_OPEN_FLAGS = 268435456u32;
pub const CRYPTCAT_VERSION_1: CRYPTCAT_VERSION = 256u32;
pub const CRYPTCAT_VERSION_2: CRYPTCAT_VERSION = 512u32;
pub const szOID_CATALOG_LIST: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.12.1.1");
pub const szOID_CATALOG_LIST_MEMBER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.12.1.2");
pub const szOID_CATALOG_LIST_MEMBER2: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.311.12.1.3");
pub type CRYPTCAT_OPEN_FLAGS = u32;
pub type CRYPTCAT_VERSION = u32;
#[repr(C)]
pub struct CATALOG_INFO {
    pub cbStruct: u32,
    pub wszCatalogFile: [u16; 260],
}
impl ::core::marker::Copy for CATALOG_INFO {}
impl ::core::clone::Clone for CATALOG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CRYPTCATATTRIBUTE {
    pub cbStruct: u32,
    pub pwszReferenceTag: ::windows_sys::core::PWSTR,
    pub dwAttrTypeAndAction: u32,
    pub cbValue: u32,
    pub pbValue: *mut u8,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for CRYPTCATATTRIBUTE {}
impl ::core::clone::Clone for CRYPTCATATTRIBUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CRYPTCATCDF {
    pub cbStruct: u32,
    pub hFile: super::super::super::Foundation::HANDLE,
    pub dwCurFilePos: u32,
    pub dwLastMemberOffset: u32,
    pub fEOF: super::super::super::Foundation::BOOL,
    pub pwszResultDir: ::windows_sys::core::PWSTR,
    pub hCATStore: super::super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CRYPTCATCDF {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CRYPTCATCDF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
pub struct CRYPTCATMEMBER {
    pub cbStruct: u32,
    pub pwszReferenceTag: ::windows_sys::core::PWSTR,
    pub pwszFileName: ::windows_sys::core::PWSTR,
    pub gSubjectType: ::windows_sys::core::GUID,
    pub fdwMemberFlags: u32,
    pub pIndirectData: *mut super::Sip::SIP_INDIRECT_DATA,
    pub dwCertVersion: u32,
    pub dwReserved: u32,
    pub hReserved: super::super::super::Foundation::HANDLE,
    pub sEncodedIndirectData: super::CRYPT_INTEGER_BLOB,
    pub sEncodedMemberInfo: super::CRYPT_INTEGER_BLOB,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for CRYPTCATMEMBER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for CRYPTCATMEMBER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CRYPTCATSTORE {
    pub cbStruct: u32,
    pub dwPublicVersion: u32,
    pub pwszP7File: ::windows_sys::core::PWSTR,
    pub hProv: usize,
    pub dwEncodingType: u32,
    pub fdwStoreFlags: CRYPTCAT_OPEN_FLAGS,
    pub hReserved: super::super::super::Foundation::HANDLE,
    pub hAttrs: super::super::super::Foundation::HANDLE,
    pub hCryptMsg: *mut ::core::ffi::c_void,
    pub hSorted: super::super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CRYPTCATSTORE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CRYPTCATSTORE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography_Sip\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
pub struct MS_ADDINFO_CATALOGMEMBER {
    pub cbStruct: u32,
    pub pStore: *mut CRYPTCATSTORE,
    pub pMember: *mut CRYPTCATMEMBER,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::marker::Copy for MS_ADDINFO_CATALOGMEMBER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography_Sip"))]
impl ::core::clone::Clone for MS_ADDINFO_CATALOGMEMBER {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PFN_CDF_PARSE_ERROR_CALLBACK = ::core::option::Option<unsafe extern "system" fn(dwerrorarea: u32, dwlocalerror: u32, pwszline: ::windows_sys::core::PCWSTR) -> ()>;
