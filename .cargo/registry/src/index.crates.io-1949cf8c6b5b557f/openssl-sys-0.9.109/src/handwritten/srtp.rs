use super::super::*;
use libc::*;

extern "C" {
    pub fn SSL_CTX_set_tlsext_use_srtp(ctx: *mut SSL_CTX, profiles: *const c_char) -> c_int;
    pub fn SSL_set_tlsext_use_srtp(ssl: *mut SSL, profiles: *const c_char) -> c_int;

    pub fn SSL_get_srtp_profiles(ssl: *mut SSL) -> *mut stack_st_SRTP_PROTECTION_PROFILE;
    pub fn SSL_get_selected_srtp_profile(ssl: *mut SSL) -> *mut SRTP_PROTECTION_PROFILE;
}
