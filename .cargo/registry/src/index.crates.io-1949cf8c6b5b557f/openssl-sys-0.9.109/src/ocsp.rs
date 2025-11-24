use libc::*;

pub const OCSP_REVOKED_STATUS_NOSTATUS: c_int = -1;
pub const OCSP_REVOKED_STATUS_UNSPECIFIED: c_int = 0;
pub const OCSP_REVOKED_STATUS_KEYCOMPROMISE: c_int = 1;
pub const OCSP_REVOKED_STATUS_CACOMPROMISE: c_int = 2;
pub const OCSP_REVOKED_STATUS_AFFILIATIONCHANGED: c_int = 3;
pub const OCSP_REVOKED_STATUS_SUPERSEDED: c_int = 4;
pub const OCSP_REVOKED_STATUS_CESSATIONOFOPERATION: c_int = 5;
pub const OCSP_REVOKED_STATUS_CERTIFICATEHOLD: c_int = 6;
pub const OCSP_REVOKED_STATUS_REMOVEFROMCRL: c_int = 8;

pub const OCSP_NOCERTS: c_ulong = 0x1;
pub const OCSP_NOINTERN: c_ulong = 0x2;
pub const OCSP_NOSIGS: c_ulong = 0x4;
pub const OCSP_NOCHAIN: c_ulong = 0x8;
pub const OCSP_NOVERIFY: c_ulong = 0x10;
pub const OCSP_NOEXPLICIT: c_ulong = 0x20;
pub const OCSP_NOCASIGN: c_ulong = 0x40;
pub const OCSP_NODELEGATED: c_ulong = 0x80;
pub const OCSP_NOCHECKS: c_ulong = 0x100;
pub const OCSP_TRUSTOTHER: c_ulong = 0x200;
pub const OCSP_RESPID_KEY: c_ulong = 0x400;
pub const OCSP_NOTIME: c_ulong = 0x800;

pub const OCSP_RESPONSE_STATUS_SUCCESSFUL: c_int = 0;
pub const OCSP_RESPONSE_STATUS_MALFORMEDREQUEST: c_int = 1;
pub const OCSP_RESPONSE_STATUS_INTERNALERROR: c_int = 2;
pub const OCSP_RESPONSE_STATUS_TRYLATER: c_int = 3;
pub const OCSP_RESPONSE_STATUS_SIGREQUIRED: c_int = 5;
pub const OCSP_RESPONSE_STATUS_UNAUTHORIZED: c_int = 6;

pub const V_OCSP_CERTSTATUS_GOOD: c_int = 0;
pub const V_OCSP_CERTSTATUS_REVOKED: c_int = 1;
pub const V_OCSP_CERTSTATUS_UNKNOWN: c_int = 2;
