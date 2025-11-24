::windows_targets::link!("dhcpcsvc.dll" "system" fn McastApiCleanup() -> ());
::windows_targets::link!("dhcpcsvc.dll" "system" fn McastApiStartup(version : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dhcpcsvc.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn McastEnumerateScopes(addrfamily : u16, requery : super::super::Foundation:: BOOL, pscopelist : *mut MCAST_SCOPE_ENTRY, pscopelen : *mut u32, pscopecount : *mut u32) -> u32);
::windows_targets::link!("dhcpcsvc.dll" "system" fn McastGenUID(prequestid : *mut MCAST_CLIENT_UID) -> u32);
::windows_targets::link!("dhcpcsvc.dll" "system" fn McastReleaseAddress(addrfamily : u16, prequestid : *mut MCAST_CLIENT_UID, preleaserequest : *mut MCAST_LEASE_REQUEST) -> u32);
::windows_targets::link!("dhcpcsvc.dll" "system" fn McastRenewAddress(addrfamily : u16, prequestid : *mut MCAST_CLIENT_UID, prenewrequest : *mut MCAST_LEASE_REQUEST, prenewresponse : *mut MCAST_LEASE_RESPONSE) -> u32);
::windows_targets::link!("dhcpcsvc.dll" "system" fn McastRequestAddress(addrfamily : u16, prequestid : *mut MCAST_CLIENT_UID, pscopectx : *mut MCAST_SCOPE_CTX, paddrrequest : *mut MCAST_LEASE_REQUEST, paddrresponse : *mut MCAST_LEASE_RESPONSE) -> u32);
pub const MCAST_API_CURRENT_VERSION: i32 = 1i32;
pub const MCAST_API_VERSION_0: i32 = 0i32;
pub const MCAST_API_VERSION_1: i32 = 1i32;
pub const MCAST_CLIENT_ID_LEN: u32 = 17u32;
#[repr(C)]
pub union IPNG_ADDRESS {
    pub IpAddrV4: u32,
    pub IpAddrV6: [u8; 16],
}
impl ::core::marker::Copy for IPNG_ADDRESS {}
impl ::core::clone::Clone for IPNG_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MCAST_CLIENT_UID {
    pub ClientUID: *mut u8,
    pub ClientUIDLength: u32,
}
impl ::core::marker::Copy for MCAST_CLIENT_UID {}
impl ::core::clone::Clone for MCAST_CLIENT_UID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MCAST_LEASE_REQUEST {
    pub LeaseStartTime: i32,
    pub MaxLeaseStartTime: i32,
    pub LeaseDuration: u32,
    pub MinLeaseDuration: u32,
    pub ServerAddress: IPNG_ADDRESS,
    pub MinAddrCount: u16,
    pub AddrCount: u16,
    pub pAddrBuf: *mut u8,
}
impl ::core::marker::Copy for MCAST_LEASE_REQUEST {}
impl ::core::clone::Clone for MCAST_LEASE_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MCAST_LEASE_RESPONSE {
    pub LeaseStartTime: i32,
    pub LeaseEndTime: i32,
    pub ServerAddress: IPNG_ADDRESS,
    pub AddrCount: u16,
    pub pAddrBuf: *mut u8,
}
impl ::core::marker::Copy for MCAST_LEASE_RESPONSE {}
impl ::core::clone::Clone for MCAST_LEASE_RESPONSE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MCAST_SCOPE_CTX {
    pub ScopeID: IPNG_ADDRESS,
    pub Interface: IPNG_ADDRESS,
    pub ServerID: IPNG_ADDRESS,
}
impl ::core::marker::Copy for MCAST_SCOPE_CTX {}
impl ::core::clone::Clone for MCAST_SCOPE_CTX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MCAST_SCOPE_ENTRY {
    pub ScopeCtx: MCAST_SCOPE_CTX,
    pub LastAddr: IPNG_ADDRESS,
    pub TTL: u32,
    pub ScopeDesc: super::super::Foundation::UNICODE_STRING,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MCAST_SCOPE_ENTRY {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MCAST_SCOPE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
