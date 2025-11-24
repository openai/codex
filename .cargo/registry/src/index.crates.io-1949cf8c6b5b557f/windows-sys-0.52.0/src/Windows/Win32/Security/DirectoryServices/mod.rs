#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authorization_UI"))]
::windows_targets::link!("dssec.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authorization_UI\"`"] fn DSCreateISecurityInfoObject(pwszobjectpath : ::windows_sys::core::PCWSTR, pwszobjectclass : ::windows_sys::core::PCWSTR, dwflags : u32, ppsi : *mut super::Authorization::UI:: ISecurityInformation, pfnreadsd : PFNREADOBJECTSECURITY, pfnwritesd : PFNWRITEOBJECTSECURITY, lpcontext : super::super::Foundation:: LPARAM) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authorization_UI"))]
::windows_targets::link!("dssec.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authorization_UI\"`"] fn DSCreateISecurityInfoObjectEx(pwszobjectpath : ::windows_sys::core::PCWSTR, pwszobjectclass : ::windows_sys::core::PCWSTR, pwszserver : ::windows_sys::core::PCWSTR, pwszusername : ::windows_sys::core::PCWSTR, pwszpassword : ::windows_sys::core::PCWSTR, dwflags : u32, ppsi : *mut super::Authorization::UI:: ISecurityInformation, pfnreadsd : PFNREADOBJECTSECURITY, pfnwritesd : PFNWRITEOBJECTSECURITY, lpcontext : super::super::Foundation:: LPARAM) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("dssec.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn DSCreateSecurityPage(pwszobjectpath : ::windows_sys::core::PCWSTR, pwszobjectclass : ::windows_sys::core::PCWSTR, dwflags : u32, phpage : *mut super::super::UI::Controls:: HPROPSHEETPAGE, pfnreadsd : PFNREADOBJECTSECURITY, pfnwritesd : PFNWRITEOBJECTSECURITY, lpcontext : super::super::Foundation:: LPARAM) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dssec.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DSEditSecurity(hwndowner : super::super::Foundation:: HWND, pwszobjectpath : ::windows_sys::core::PCWSTR, pwszobjectclass : ::windows_sys::core::PCWSTR, dwflags : u32, pwszcaption : ::windows_sys::core::PCWSTR, pfnreadsd : PFNREADOBJECTSECURITY, pfnwritesd : PFNWRITEOBJECTSECURITY, lpcontext : super::super::Foundation:: LPARAM) -> ::windows_sys::core::HRESULT);
pub const DSSI_IS_ROOT: u32 = 16u32;
pub const DSSI_NO_ACCESS_CHECK: u32 = 2u32;
pub const DSSI_NO_EDIT_OWNER: u32 = 8u32;
pub const DSSI_NO_EDIT_SACL: u32 = 4u32;
pub const DSSI_NO_FILTER: u32 = 32u32;
pub const DSSI_NO_READONLY_MESSAGE: u32 = 64u32;
pub const DSSI_READ_ONLY: u32 = 1u32;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authorization_UI\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authorization_UI"))]
pub type PFNDSCREATEISECINFO = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: u32, param3: *mut super::Authorization::UI::ISecurityInformation, param4: PFNREADOBJECTSECURITY, param5: PFNWRITEOBJECTSECURITY, param6: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authorization_UI\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authorization_UI"))]
pub type PFNDSCREATEISECINFOEX = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: ::windows_sys::core::PCWSTR, param3: ::windows_sys::core::PCWSTR, param4: ::windows_sys::core::PCWSTR, param5: u32, param6: *mut super::Authorization::UI::ISecurityInformation, param7: PFNREADOBJECTSECURITY, param8: PFNWRITEOBJECTSECURITY, param9: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub type PFNDSCREATESECPAGE = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: u32, param3: *mut super::super::UI::Controls::HPROPSHEETPAGE, param4: PFNREADOBJECTSECURITY, param5: PFNWRITEOBJECTSECURITY, param6: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNDSEDITSECURITY = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Foundation::HWND, param1: ::windows_sys::core::PCWSTR, param2: ::windows_sys::core::PCWSTR, param3: u32, param4: ::windows_sys::core::PCWSTR, param5: PFNREADOBJECTSECURITY, param6: PFNWRITEOBJECTSECURITY, param7: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNREADOBJECTSECURITY = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32, param2: *mut super::PSECURITY_DESCRIPTOR, param3: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFNWRITEOBJECTSECURITY = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32, param2: super::PSECURITY_DESCRIPTOR, param3: super::super::Foundation::LPARAM) -> ::windows_sys::core::HRESULT>;
