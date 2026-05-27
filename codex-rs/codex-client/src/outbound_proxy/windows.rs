use std::ffi::c_void;
use std::ptr;

use super::ParsedProxyListDecision;
use super::RequestOrigin;
use super::SystemProxyDecision;
use super::no_proxy_matches_origin;
use super::parse_proxy_list;
use crate::route_diagnostics::RouteFailureClass;
use crate::route_diagnostics::RouteSource;
use windows_sys::Win32::Foundation::FALSE;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::Foundation::TRUE;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_AUTODETECTION_FAILED;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_BAD_AUTO_PROXY_SCRIPT;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_CANNOT_CONNECT;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_CONNECTION_ERROR;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_INVALID_URL;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_LOGIN_FAILURE;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_NAME_NOT_RESOLVED;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SCRIPT_EXECUTION_ERROR;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_CERT_CN_INVALID;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_CERT_DATE_INVALID;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_CERT_REV_FAILED;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_CERT_REVOKED;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_CERT_WRONG_USAGE;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_CHANNEL_ERROR;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_FAILURE;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_INVALID_CA;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_SECURE_INVALID_CERT;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_TIMEOUT;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_UNABLE_TO_DOWNLOAD_SCRIPT;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_UNHANDLED_SCRIPT_TYPE;
use windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_UNRECOGNIZED_SCHEME;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_ACCESS_TYPE_NAMED_PROXY;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_ACCESS_TYPE_NO_PROXY;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_AUTO_DETECT_TYPE_DHCP;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_AUTO_DETECT_TYPE_DNS_A;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_AUTOPROXY_AUTO_DETECT;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_AUTOPROXY_CONFIG_URL;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_AUTOPROXY_OPTIONS;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_CURRENT_USER_IE_PROXY_CONFIG;
use windows_sys::Win32::Networking::WinHttp::WINHTTP_PROXY_INFO;
use windows_sys::Win32::Networking::WinHttp::WinHttpCloseHandle;
use windows_sys::Win32::Networking::WinHttp::WinHttpGetIEProxyConfigForCurrentUser;
use windows_sys::Win32::Networking::WinHttp::WinHttpGetProxyForUrl;
use windows_sys::Win32::Networking::WinHttp::WinHttpOpen;
use windows_sys::core::PWSTR;

pub(super) fn resolve(request_url: &str, origin: &RequestOrigin) -> SystemProxyDecision {
    let ie_config = match current_user_ie_proxy_config() {
        Ok(config) => config,
        Err(failure) => {
            return SystemProxyDecision::Unavailable {
                source: RouteSource::Error,
                failure,
            };
        }
    };

    if let Some(pac_url) = ie_config.auto_config_url.as_deref() {
        let decision = resolve_with_pac_url(request_url, origin, pac_url);
        if !matches!(decision, SystemProxyDecision::Unavailable { .. }) {
            return decision;
        }
    }

    if ie_config.auto_detect {
        let decision = resolve_with_auto_detect(request_url, origin);
        if !matches!(decision, SystemProxyDecision::Unavailable { .. }) {
            return decision;
        }
    }

    if let Some(proxy) = ie_config.static_proxy.as_deref() {
        if ie_config
            .proxy_bypass
            .as_deref()
            .is_some_and(|bypass| proxy_bypass_matches_origin(bypass, origin))
        {
            return SystemProxyDecision::Direct {
                source: RouteSource::WindowsStatic,
            };
        }
        return proxy_list_decision(proxy, origin, RouteSource::WindowsStatic);
    }

    if ie_config.auto_config_url.is_some() || ie_config.auto_detect {
        SystemProxyDecision::Unavailable {
            source: RouteSource::WindowsWinHttpPac,
            failure: RouteFailureClass::PacUnavailable,
        }
    } else {
        SystemProxyDecision::Direct {
            source: RouteSource::Direct,
        }
    }
}

fn resolve_with_pac_url(
    request_url: &str,
    origin: &RequestOrigin,
    pac_url: &str,
) -> SystemProxyDecision {
    let pac_url = wide_null(pac_url);
    let options = WINHTTP_AUTOPROXY_OPTIONS {
        dwFlags: WINHTTP_AUTOPROXY_CONFIG_URL,
        dwAutoDetectFlags: 0,
        lpszAutoConfigUrl: pac_url.as_ptr(),
        lpvReserved: ptr::null_mut(),
        dwReserved: 0,
        fAutoLogonIfChallenged: TRUE,
    };
    resolve_with_winhttp_options(request_url, origin, options)
}

fn resolve_with_auto_detect(request_url: &str, origin: &RequestOrigin) -> SystemProxyDecision {
    let options = WINHTTP_AUTOPROXY_OPTIONS {
        dwFlags: WINHTTP_AUTOPROXY_AUTO_DETECT,
        dwAutoDetectFlags: WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A,
        lpszAutoConfigUrl: ptr::null(),
        lpvReserved: ptr::null_mut(),
        dwReserved: 0,
        fAutoLogonIfChallenged: TRUE,
    };
    resolve_with_winhttp_options(request_url, origin, options)
}

fn resolve_with_winhttp_options(
    request_url: &str,
    origin: &RequestOrigin,
    mut options: WINHTTP_AUTOPROXY_OPTIONS,
) -> SystemProxyDecision {
    let Some(session) = WinHttpSession::open() else {
        return SystemProxyDecision::Unavailable {
            source: RouteSource::WindowsWinHttpPac,
            failure: classify_winhttp_error(last_error()),
        };
    };

    let request_url = wide_null(request_url);
    let mut proxy_info = WINHTTP_PROXY_INFO {
        dwAccessType: WINHTTP_ACCESS_TYPE_NO_PROXY,
        lpszProxy: ptr::null_mut(),
        lpszProxyBypass: ptr::null_mut(),
    };
    let ok = unsafe {
        WinHttpGetProxyForUrl(
            session.0,
            request_url.as_ptr(),
            &mut options,
            &mut proxy_info,
        )
    };
    if ok == FALSE {
        return SystemProxyDecision::Unavailable {
            source: RouteSource::WindowsWinHttpPac,
            failure: classify_winhttp_error(last_error()),
        };
    }

    let proxy_info = ProxyInfo::from_raw(proxy_info);
    if proxy_info.access_type == WINHTTP_ACCESS_TYPE_NO_PROXY {
        return SystemProxyDecision::Direct {
            source: RouteSource::WindowsWinHttpPac,
        };
    }
    if proxy_info.access_type != WINHTTP_ACCESS_TYPE_NAMED_PROXY {
        return SystemProxyDecision::Unavailable {
            source: RouteSource::WindowsWinHttpPac,
            failure: RouteFailureClass::PacUnavailable,
        };
    }
    let Some(proxy) = proxy_info.proxy.as_deref() else {
        return SystemProxyDecision::Unavailable {
            source: RouteSource::WindowsWinHttpPac,
            failure: RouteFailureClass::PacUnavailable,
        };
    };
    proxy_list_decision(proxy, origin, RouteSource::WindowsWinHttpPac)
}

fn proxy_list_decision(
    proxy_list: &str,
    origin: &RequestOrigin,
    source: RouteSource,
) -> SystemProxyDecision {
    match parse_proxy_list(proxy_list, &origin.scheme) {
        ParsedProxyListDecision::Direct => SystemProxyDecision::Direct { source },
        ParsedProxyListDecision::Proxy(url) => SystemProxyDecision::Proxy { source, url },
        ParsedProxyListDecision::UnsupportedScheme => SystemProxyDecision::Unavailable {
            source,
            failure: RouteFailureClass::UnsupportedProxyScheme,
        },
        ParsedProxyListDecision::Unavailable => SystemProxyDecision::Unavailable {
            source,
            failure: RouteFailureClass::PacUnavailable,
        },
    }
}

fn current_user_ie_proxy_config() -> Result<IeProxyConfig, RouteFailureClass> {
    let mut raw = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG {
        fAutoDetect: FALSE,
        lpszAutoConfigUrl: ptr::null_mut(),
        lpszProxy: ptr::null_mut(),
        lpszProxyBypass: ptr::null_mut(),
    };
    let ok = unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut raw) };
    if ok == FALSE {
        return Err(classify_winhttp_error(last_error()));
    }

    let auto_config_url = GlobalWideString::from_raw(raw.lpszAutoConfigUrl).into_string();
    let static_proxy = GlobalWideString::from_raw(raw.lpszProxy).into_string();
    let proxy_bypass = GlobalWideString::from_raw(raw.lpszProxyBypass).into_string();

    Ok(IeProxyConfig {
        auto_detect: raw.fAutoDetect != FALSE,
        auto_config_url,
        static_proxy,
        proxy_bypass,
    })
}

#[derive(Debug)]
struct IeProxyConfig {
    auto_detect: bool,
    auto_config_url: Option<String>,
    static_proxy: Option<String>,
    proxy_bypass: Option<String>,
}

struct ProxyInfo {
    access_type: u32,
    proxy: Option<String>,
    _proxy_bypass: Option<String>,
}

impl ProxyInfo {
    fn from_raw(raw: WINHTTP_PROXY_INFO) -> Self {
        Self {
            access_type: raw.dwAccessType,
            proxy: GlobalWideString::from_raw(raw.lpszProxy).into_string(),
            _proxy_bypass: GlobalWideString::from_raw(raw.lpszProxyBypass).into_string(),
        }
    }
}

struct GlobalWideString(PWSTR);

impl GlobalWideString {
    fn from_raw(ptr: PWSTR) -> Self {
        Self(ptr)
    }

    fn into_string(self) -> Option<String> {
        if self.0.is_null() {
            return None;
        }
        let string = unsafe { wide_ptr_to_string(self.0) };
        if string.is_empty() {
            None
        } else {
            Some(string)
        }
    }
}

impl Drop for GlobalWideString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                GlobalFree(self.0.cast::<c_void>());
            }
        }
    }
}

struct WinHttpSession(*mut c_void);

impl WinHttpSession {
    fn open() -> Option<Self> {
        let agent = wide_null("Codex");
        let handle = unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_NO_PROXY,
                ptr::null(),
                ptr::null(),
                0,
            )
        };
        if handle.is_null() {
            None
        } else {
            Some(Self(handle))
        }
    }
}

impl Drop for WinHttpSession {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn proxy_bypass_matches_origin(proxy_bypass: &str, origin: &RequestOrigin) -> bool {
    proxy_bypass.split([';', ',']).map(str::trim).any(|entry| {
        if entry.eq_ignore_ascii_case("<local>") {
            !origin.host.contains('.')
        } else {
            no_proxy_matches_origin(entry, origin)
        }
    })
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn wide_ptr_to_string(ptr: PWSTR) -> String {
    let mut len = 0;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

fn last_error() -> u32 {
    unsafe { GetLastError() }
}

fn classify_winhttp_error(code: u32) -> RouteFailureClass {
    match code {
        ERROR_WINHTTP_TIMEOUT => RouteFailureClass::ConnectTimeout,
        ERROR_WINHTTP_LOGIN_FAILURE => RouteFailureClass::ProxyAuthenticationRequired,
        ERROR_WINHTTP_AUTODETECTION_FAILED
        | ERROR_WINHTTP_BAD_AUTO_PROXY_SCRIPT
        | ERROR_WINHTTP_SCRIPT_EXECUTION_ERROR
        | ERROR_WINHTTP_UNABLE_TO_DOWNLOAD_SCRIPT
        | ERROR_WINHTTP_UNHANDLED_SCRIPT_TYPE => RouteFailureClass::PacUnavailable,
        ERROR_WINHTTP_SECURE_CERT_CN_INVALID
        | ERROR_WINHTTP_SECURE_CERT_DATE_INVALID
        | ERROR_WINHTTP_SECURE_CERT_REVOKED
        | ERROR_WINHTTP_SECURE_CERT_REV_FAILED
        | ERROR_WINHTTP_SECURE_CERT_WRONG_USAGE
        | ERROR_WINHTTP_SECURE_CHANNEL_ERROR
        | ERROR_WINHTTP_SECURE_FAILURE
        | ERROR_WINHTTP_SECURE_INVALID_CA
        | ERROR_WINHTTP_SECURE_INVALID_CERT => RouteFailureClass::TlsError,
        ERROR_WINHTTP_INVALID_URL | ERROR_WINHTTP_UNRECOGNIZED_SCHEME => {
            RouteFailureClass::InvalidProxyConfig
        }
        ERROR_WINHTTP_CANNOT_CONNECT
        | ERROR_WINHTTP_CONNECTION_ERROR
        | ERROR_WINHTTP_NAME_NOT_RESOLVED => RouteFailureClass::ResolverError,
        _ => RouteFailureClass::ResolverError,
    }
}
