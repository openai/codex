::windows_targets::link!("wldap32.dll" "cdecl" fn LdapGetLastError() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LdapMapErrorToWin32(ldaperror : u32) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("wldap32.dll" "cdecl" fn LdapUTF8ToUnicode(lpsrcstr : ::windows_sys::core::PCSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PWSTR, cchdest : i32) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn LdapUnicodeToUTF8(lpsrcstr : ::windows_sys::core::PCWSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PSTR, cchdest : i32) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_alloc_t(options : i32) -> *mut BerElement);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_bvdup(pberval : *mut LDAP_BERVAL) -> *mut LDAP_BERVAL);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_bvecfree(pberval : *mut *mut LDAP_BERVAL) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_bvfree(bv : *mut LDAP_BERVAL) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_first_element(pberelement : *mut BerElement, plen : *mut u32, ppopaque : *mut *mut u8) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_flatten(pberelement : *mut BerElement, pberval : *mut *mut LDAP_BERVAL) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_free(pberelement : *mut BerElement, fbuf : i32) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_init(pberval : *mut LDAP_BERVAL) -> *mut BerElement);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_next_element(pberelement : *mut BerElement, plen : *mut u32, opaque : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_peek_tag(pberelement : *mut BerElement, plen : *mut u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_printf(pberelement : *mut BerElement, fmt : ::windows_sys::core::PCSTR, ...) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_scanf(pberelement : *mut BerElement, fmt : ::windows_sys::core::PCSTR, ...) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ber_skip_tag(pberelement : *mut BerElement, plen : *mut u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn cldap_open(hostname : ::windows_sys::core::PCSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn cldap_openA(hostname : ::windows_sys::core::PCSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn cldap_openW(hostname : ::windows_sys::core::PCWSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_abandon(ld : *mut LDAP, msgid : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_add(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_addA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_addW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attrs : *mut *mut LDAPModW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_add_ext(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_add_extA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_add_extW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attrs : *mut *mut LDAPModW, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_add_ext_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_add_ext_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_add_ext_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attrs : *mut *mut LDAPModW, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_add_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_add_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attrs : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_add_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attrs : *mut *mut LDAPModW) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_bind(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, cred : ::windows_sys::core::PCSTR, method : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_bindA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, cred : ::windows_sys::core::PCSTR, method : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_bindW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, cred : ::windows_sys::core::PCWSTR, method : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_bind_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, cred : ::windows_sys::core::PCSTR, method : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_bind_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, cred : ::windows_sys::core::PCSTR, method : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_bind_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, cred : ::windows_sys::core::PCWSTR, method : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_check_filterA(ld : *mut LDAP, searchfilter : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_check_filterW(ld : *mut LDAP, searchfilter : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_cleanup(hinstance : super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_close_extended_op(ld : *mut LDAP, messagenumber : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_compare(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_compareA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_compareW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attr : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_compare_ext(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_compare_extA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, data : *const LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_compare_extW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attr : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PCWSTR, data : *const LDAP_BERVAL, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_compare_ext_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_compare_ext_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, data : *const LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_compare_ext_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attr : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PCWSTR, data : *const LDAP_BERVAL, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_compare_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_compare_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, attr : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_compare_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, attr : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_conn_from_msg(primaryconn : *mut LDAP, res : *mut LDAPMessage) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_connect(ld : *mut LDAP, timeout : *mut LDAP_TIMEVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_control_free(control : *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_control_freeA(controls : *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_control_freeW(control : *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_controls_free(controls : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_controls_freeA(controls : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_controls_freeW(control : *mut *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_count_entries(ld : *mut LDAP, res : *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_count_references(ld : *mut LDAP, res : *mut LDAPMessage) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_count_values(vals : *const ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_count_valuesA(vals : *const ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_count_valuesW(vals : *const ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_count_values_len(vals : *mut *mut LDAP_BERVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_page_control(externalhandle : *mut LDAP, pagesize : u32, cookie : *mut LDAP_BERVAL, iscritical : u8, control : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_page_controlA(externalhandle : *mut LDAP, pagesize : u32, cookie : *mut LDAP_BERVAL, iscritical : u8, control : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_page_controlW(externalhandle : *mut LDAP, pagesize : u32, cookie : *mut LDAP_BERVAL, iscritical : u8, control : *mut *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_sort_control(externalhandle : *mut LDAP, sortkeys : *mut *mut LDAPSortKeyA, iscritical : u8, control : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_sort_controlA(externalhandle : *mut LDAP, sortkeys : *mut *mut LDAPSortKeyA, iscritical : u8, control : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_sort_controlW(externalhandle : *mut LDAP, sortkeys : *mut *mut LDAPSortKeyW, iscritical : u8, control : *mut *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_vlv_controlA(externalhandle : *mut LDAP, vlvinfo : *mut LDAPVLVInfo, iscritical : u8, control : *mut *mut LDAPControlA) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_create_vlv_controlW(externalhandle : *mut LDAP, vlvinfo : *mut LDAPVLVInfo, iscritical : u8, control : *mut *mut LDAPControlW) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_delete(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_deleteA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_deleteW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_delete_ext(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_delete_extA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_delete_extW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_delete_ext_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_delete_ext_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_delete_ext_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_delete_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_delete_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_delete_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_dn2ufn(dn : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_dn2ufnA(dn : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_dn2ufnW(dn : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_encode_sort_controlA(externalhandle : *mut LDAP, sortkeys : *mut *mut LDAPSortKeyA, control : *mut LDAPControlA, criticality : super::super::Foundation:: BOOLEAN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_encode_sort_controlW(externalhandle : *mut LDAP, sortkeys : *mut *mut LDAPSortKeyW, control : *mut LDAPControlW, criticality : super::super::Foundation:: BOOLEAN) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_err2string(err : u32) -> ::windows_sys::core::PSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_err2stringA(err : u32) -> ::windows_sys::core::PSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_err2stringW(err : u32) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_escape_filter_element(sourcefilterelement : ::windows_sys::core::PCSTR, sourcelength : u32, destfilterelement : ::windows_sys::core::PSTR, destlength : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_escape_filter_elementA(sourcefilterelement : ::windows_sys::core::PCSTR, sourcelength : u32, destfilterelement : ::windows_sys::core::PSTR, destlength : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_escape_filter_elementW(sourcefilterelement : ::windows_sys::core::PCSTR, sourcelength : u32, destfilterelement : ::windows_sys::core::PWSTR, destlength : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_explode_dn(dn : ::windows_sys::core::PCSTR, notypes : u32) -> *mut ::windows_sys::core::PSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_explode_dnA(dn : ::windows_sys::core::PCSTR, notypes : u32) -> *mut ::windows_sys::core::PSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_explode_dnW(dn : ::windows_sys::core::PCWSTR, notypes : u32) -> *mut ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_extended_operation(ld : *mut LDAP, oid : ::windows_sys::core::PCSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_extended_operationA(ld : *mut LDAP, oid : ::windows_sys::core::PCSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_extended_operationW(ld : *mut LDAP, oid : ::windows_sys::core::PCWSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_extended_operation_sA(externalhandle : *mut LDAP, oid : ::windows_sys::core::PCSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, returnedoid : *mut ::windows_sys::core::PSTR, returneddata : *mut *mut LDAP_BERVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_extended_operation_sW(externalhandle : *mut LDAP, oid : ::windows_sys::core::PCWSTR, data : *mut LDAP_BERVAL, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, returnedoid : *mut ::windows_sys::core::PWSTR, returneddata : *mut *mut LDAP_BERVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_first_attribute(ld : *mut LDAP, entry : *mut LDAPMessage, ptr : *mut *mut BerElement) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_first_attributeA(ld : *mut LDAP, entry : *mut LDAPMessage, ptr : *mut *mut BerElement) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_first_attributeW(ld : *mut LDAP, entry : *mut LDAPMessage, ptr : *mut *mut BerElement) -> ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_first_entry(ld : *mut LDAP, res : *mut LDAPMessage) -> *mut LDAPMessage);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_first_reference(ld : *mut LDAP, res : *mut LDAPMessage) -> *mut LDAPMessage);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_free_controls(controls : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_free_controlsA(controls : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_free_controlsW(controls : *mut *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_dn(ld : *mut LDAP, entry : *mut LDAPMessage) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_dnA(ld : *mut LDAP, entry : *mut LDAPMessage) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_dnW(ld : *mut LDAP, entry : *mut LDAPMessage) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_get_next_page(externalhandle : *mut LDAP, searchhandle : PLDAPSearch, pagesize : u32, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_next_page_s(externalhandle : *mut LDAP, searchhandle : PLDAPSearch, timeout : *mut LDAP_TIMEVAL, pagesize : u32, totalcount : *mut u32, results : *mut *mut LDAPMessage) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_get_option(ld : *mut LDAP, option : i32, outvalue : *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_get_optionW(ld : *mut LDAP, option : i32, outvalue : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_paged_count(externalhandle : *mut LDAP, searchblock : PLDAPSearch, totalcount : *mut u32, results : *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_values(ld : *mut LDAP, entry : *mut LDAPMessage, attr : ::windows_sys::core::PCSTR) -> *mut ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_valuesA(ld : *mut LDAP, entry : *mut LDAPMessage, attr : ::windows_sys::core::PCSTR) -> *mut ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_valuesW(ld : *mut LDAP, entry : *mut LDAPMessage, attr : ::windows_sys::core::PCWSTR) -> *mut ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_values_len(externalhandle : *mut LDAP, message : *mut LDAPMessage, attr : ::windows_sys::core::PCSTR) -> *mut *mut LDAP_BERVAL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_values_lenA(externalhandle : *mut LDAP, message : *mut LDAPMessage, attr : ::windows_sys::core::PCSTR) -> *mut *mut LDAP_BERVAL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_get_values_lenW(externalhandle : *mut LDAP, message : *mut LDAPMessage, attr : ::windows_sys::core::PCWSTR) -> *mut *mut LDAP_BERVAL);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_init(hostname : ::windows_sys::core::PCSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_initA(hostname : ::windows_sys::core::PCSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_initW(hostname : ::windows_sys::core::PCWSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_memfree(block : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_memfreeA(block : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_memfreeW(block : ::windows_sys::core::PCWSTR) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modify(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modifyA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modifyW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, mods : *mut *mut LDAPModW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_modify_ext(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_modify_extA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_modify_extW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, mods : *mut *mut LDAPModW, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_modify_ext_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_modify_ext_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_modify_ext_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, mods : *mut *mut LDAPModW, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modify_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modify_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, mods : *mut *mut LDAPModA) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modify_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, mods : *mut *mut LDAPModW) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn2(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR, deleteoldrdn : i32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn2A(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR, deleteoldrdn : i32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn2W(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCWSTR, newdistinguishedname : ::windows_sys::core::PCWSTR, deleteoldrdn : i32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn2_s(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR, deleteoldrdn : i32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn2_sA(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR, deleteoldrdn : i32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn2_sW(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCWSTR, newdistinguishedname : ::windows_sys::core::PCWSTR, deleteoldrdn : i32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdnA(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdnW(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCWSTR, newdistinguishedname : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn_s(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn_sA(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, newdistinguishedname : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_modrdn_sW(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCWSTR, newdistinguishedname : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_msgfree(res : *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_next_attribute(ld : *mut LDAP, entry : *mut LDAPMessage, ptr : *mut BerElement) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_next_attributeA(ld : *mut LDAP, entry : *mut LDAPMessage, ptr : *mut BerElement) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_next_attributeW(ld : *mut LDAP, entry : *mut LDAPMessage, ptr : *mut BerElement) -> ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_next_entry(ld : *mut LDAP, entry : *mut LDAPMessage) -> *mut LDAPMessage);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_next_reference(ld : *mut LDAP, entry : *mut LDAPMessage) -> *mut LDAPMessage);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_open(hostname : ::windows_sys::core::PCSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_openA(hostname : ::windows_sys::core::PCSTR, portnumber : u32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_openW(hostname : ::windows_sys::core::PCWSTR, portnumber : u32) -> *mut LDAP);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_extended_resultA(connection : *mut LDAP, resultmessage : *mut LDAPMessage, resultoid : *mut ::windows_sys::core::PSTR, resultdata : *mut *mut LDAP_BERVAL, freeit : super::super::Foundation:: BOOLEAN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_extended_resultW(connection : *mut LDAP, resultmessage : *mut LDAPMessage, resultoid : *mut ::windows_sys::core::PWSTR, resultdata : *mut *mut LDAP_BERVAL, freeit : super::super::Foundation:: BOOLEAN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_page_control(externalhandle : *mut LDAP, servercontrols : *mut *mut LDAPControlA, totalcount : *mut u32, cookie : *mut *mut LDAP_BERVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_page_controlA(externalhandle : *mut LDAP, servercontrols : *mut *mut LDAPControlA, totalcount : *mut u32, cookie : *mut *mut LDAP_BERVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_page_controlW(externalhandle : *mut LDAP, servercontrols : *mut *mut LDAPControlW, totalcount : *mut u32, cookie : *mut *mut LDAP_BERVAL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_reference(connection : *mut LDAP, resultmessage : *mut LDAPMessage, referrals : *mut *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_referenceA(connection : *mut LDAP, resultmessage : *mut LDAPMessage, referrals : *mut *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_referenceW(connection : *mut LDAP, resultmessage : *mut LDAPMessage, referrals : *mut *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_result(connection : *mut LDAP, resultmessage : *mut LDAPMessage, returncode : *mut u32, matcheddns : *mut ::windows_sys::core::PSTR, errormessage : *mut ::windows_sys::core::PSTR, referrals : *mut *mut ::windows_sys::core::PSTR, servercontrols : *mut *mut *mut LDAPControlA, freeit : super::super::Foundation:: BOOLEAN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_resultA(connection : *mut LDAP, resultmessage : *mut LDAPMessage, returncode : *mut u32, matcheddns : *mut ::windows_sys::core::PSTR, errormessage : *mut ::windows_sys::core::PSTR, referrals : *mut *mut *mut i8, servercontrols : *mut *mut *mut LDAPControlA, freeit : super::super::Foundation:: BOOLEAN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_resultW(connection : *mut LDAP, resultmessage : *mut LDAPMessage, returncode : *mut u32, matcheddns : *mut ::windows_sys::core::PWSTR, errormessage : *mut ::windows_sys::core::PWSTR, referrals : *mut *mut *mut u16, servercontrols : *mut *mut *mut LDAPControlW, freeit : super::super::Foundation:: BOOLEAN) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_sort_control(externalhandle : *mut LDAP, control : *mut *mut LDAPControlA, result : *mut u32, attribute : *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_sort_controlA(externalhandle : *mut LDAP, control : *mut *mut LDAPControlA, result : *mut u32, attribute : *mut ::windows_sys::core::PSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_sort_controlW(externalhandle : *mut LDAP, control : *mut *mut LDAPControlW, result : *mut u32, attribute : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_vlv_controlA(externalhandle : *mut LDAP, control : *mut *mut LDAPControlA, targetpos : *mut u32, listcount : *mut u32, context : *mut *mut LDAP_BERVAL, errcode : *mut i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_parse_vlv_controlW(externalhandle : *mut LDAP, control : *mut *mut LDAPControlW, targetpos : *mut u32, listcount : *mut u32, context : *mut *mut LDAP_BERVAL, errcode : *mut i32) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_perror(ld : *mut LDAP, msg : ::windows_sys::core::PCSTR) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_rename_ext(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, newrdn : ::windows_sys::core::PCSTR, newparent : ::windows_sys::core::PCSTR, deleteoldrdn : i32, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_rename_extA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, newrdn : ::windows_sys::core::PCSTR, newparent : ::windows_sys::core::PCSTR, deleteoldrdn : i32, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_rename_extW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, newrdn : ::windows_sys::core::PCWSTR, newparent : ::windows_sys::core::PCWSTR, deleteoldrdn : i32, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_rename_ext_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, newrdn : ::windows_sys::core::PCSTR, newparent : ::windows_sys::core::PCSTR, deleteoldrdn : i32, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_rename_ext_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, newrdn : ::windows_sys::core::PCSTR, newparent : ::windows_sys::core::PCSTR, deleteoldrdn : i32, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_rename_ext_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, newrdn : ::windows_sys::core::PCWSTR, newparent : ::windows_sys::core::PCWSTR, deleteoldrdn : i32, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_result(ld : *mut LDAP, msgid : u32, all : u32, timeout : *const LDAP_TIMEVAL, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_result2error(ld : *mut LDAP, res : *mut LDAPMessage, freeit : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_sasl_bindA(externalhandle : *mut LDAP, distname : ::windows_sys::core::PCSTR, authmechanism : ::windows_sys::core::PCSTR, cred : *const LDAP_BERVAL, serverctrls : *mut *mut LDAPControlA, clientctrls : *mut *mut LDAPControlA, messagenumber : *mut i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_sasl_bindW(externalhandle : *mut LDAP, distname : ::windows_sys::core::PCWSTR, authmechanism : ::windows_sys::core::PCWSTR, cred : *const LDAP_BERVAL, serverctrls : *mut *mut LDAPControlW, clientctrls : *mut *mut LDAPControlW, messagenumber : *mut i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_sasl_bind_sA(externalhandle : *mut LDAP, distname : ::windows_sys::core::PCSTR, authmechanism : ::windows_sys::core::PCSTR, cred : *const LDAP_BERVAL, serverctrls : *mut *mut LDAPControlA, clientctrls : *mut *mut LDAPControlA, serverdata : *mut *mut LDAP_BERVAL) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_sasl_bind_sW(externalhandle : *mut LDAP, distname : ::windows_sys::core::PCWSTR, authmechanism : ::windows_sys::core::PCWSTR, cred : *const LDAP_BERVAL, serverctrls : *mut *mut LDAPControlW, clientctrls : *mut *mut LDAPControlW, serverdata : *mut *mut LDAP_BERVAL) -> i32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_search(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_searchA(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_searchW(ld : *mut LDAP, base : ::windows_sys::core::PCWSTR, scope : u32, filter : ::windows_sys::core::PCWSTR, attrs : *const *const u16, attrsonly : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_search_abandon_page(externalhandle : *mut LDAP, searchblock : PLDAPSearch) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_ext(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, servercontrols : *const *const LDAPControlA, clientcontrols : *const *const LDAPControlA, timelimit : u32, sizelimit : u32, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_extA(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, servercontrols : *const *const LDAPControlA, clientcontrols : *const *const LDAPControlA, timelimit : u32, sizelimit : u32, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_extW(ld : *mut LDAP, base : ::windows_sys::core::PCWSTR, scope : u32, filter : ::windows_sys::core::PCWSTR, attrs : *const *const u16, attrsonly : u32, servercontrols : *const *const LDAPControlW, clientcontrols : *const *const LDAPControlW, timelimit : u32, sizelimit : u32, messagenumber : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_ext_s(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, servercontrols : *const *const LDAPControlA, clientcontrols : *const *const LDAPControlA, timeout : *mut LDAP_TIMEVAL, sizelimit : u32, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_ext_sA(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, servercontrols : *const *const LDAPControlA, clientcontrols : *const *const LDAPControlA, timeout : *mut LDAP_TIMEVAL, sizelimit : u32, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_ext_sW(ld : *mut LDAP, base : ::windows_sys::core::PCWSTR, scope : u32, filter : ::windows_sys::core::PCWSTR, attrs : *const *const u16, attrsonly : u32, servercontrols : *const *const LDAPControlW, clientcontrols : *const *const LDAPControlW, timeout : *mut LDAP_TIMEVAL, sizelimit : u32, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_init_page(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, scopeofsearch : u32, searchfilter : ::windows_sys::core::PCSTR, attributelist : *mut *mut i8, attributesonly : u32, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, pagetimelimit : u32, totalsizelimit : u32, sortkeys : *mut *mut LDAPSortKeyA) -> PLDAPSearch);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_init_pageA(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCSTR, scopeofsearch : u32, searchfilter : ::windows_sys::core::PCSTR, attributelist : *const *const i8, attributesonly : u32, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA, pagetimelimit : u32, totalsizelimit : u32, sortkeys : *mut *mut LDAPSortKeyA) -> PLDAPSearch);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_init_pageW(externalhandle : *mut LDAP, distinguishedname : ::windows_sys::core::PCWSTR, scopeofsearch : u32, searchfilter : ::windows_sys::core::PCWSTR, attributelist : *const *const u16, attributesonly : u32, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW, pagetimelimit : u32, totalsizelimit : u32, sortkeys : *mut *mut LDAPSortKeyW) -> PLDAPSearch);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_s(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_sA(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_sW(ld : *mut LDAP, base : ::windows_sys::core::PCWSTR, scope : u32, filter : ::windows_sys::core::PCWSTR, attrs : *const *const u16, attrsonly : u32, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_st(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, timeout : *mut LDAP_TIMEVAL, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_stA(ld : *mut LDAP, base : ::windows_sys::core::PCSTR, scope : u32, filter : ::windows_sys::core::PCSTR, attrs : *const *const i8, attrsonly : u32, timeout : *mut LDAP_TIMEVAL, res : *mut *mut LDAPMessage) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_search_stW(ld : *mut LDAP, base : ::windows_sys::core::PCWSTR, scope : u32, filter : ::windows_sys::core::PCWSTR, attrs : *const *const u16, attrsonly : u32, timeout : *mut LDAP_TIMEVAL, res : *mut *mut LDAPMessage) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_set_dbg_flags(newflags : u32) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_set_dbg_routine(debugprintroutine : DBGPRINT) -> ());
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_set_option(ld : *mut LDAP, option : i32, invalue : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_set_optionW(ld : *mut LDAP, option : i32, invalue : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_simple_bind(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, passwd : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_simple_bindA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, passwd : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_simple_bindW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, passwd : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_simple_bind_s(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, passwd : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_simple_bind_sA(ld : *mut LDAP, dn : ::windows_sys::core::PCSTR, passwd : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_simple_bind_sW(ld : *mut LDAP, dn : ::windows_sys::core::PCWSTR, passwd : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_sslinit(hostname : ::windows_sys::core::PCSTR, portnumber : u32, secure : i32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_sslinitA(hostname : ::windows_sys::core::PCSTR, portnumber : u32, secure : i32) -> *mut LDAP);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_sslinitW(hostname : ::windows_sys::core::PCWSTR, portnumber : u32, secure : i32) -> *mut LDAP);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_start_tls_sA(externalhandle : *mut LDAP, serverreturnvalue : *mut u32, result : *mut *mut LDAPMessage, servercontrols : *mut *mut LDAPControlA, clientcontrols : *mut *mut LDAPControlA) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_start_tls_sW(externalhandle : *mut LDAP, serverreturnvalue : *mut u32, result : *mut *mut LDAPMessage, servercontrols : *mut *mut LDAPControlW, clientcontrols : *mut *mut LDAPControlW) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_startup(version : *mut LDAP_VERSION_INFO, instance : *mut super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("wldap32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ldap_stop_tls_s(externalhandle : *mut LDAP) -> super::super::Foundation:: BOOLEAN);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_ufn2dn(ufn : ::windows_sys::core::PCSTR, pdn : *mut ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_ufn2dnA(ufn : ::windows_sys::core::PCSTR, pdn : *mut ::windows_sys::core::PSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_ufn2dnW(ufn : ::windows_sys::core::PCWSTR, pdn : *mut ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_unbind(ld : *mut LDAP) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_unbind_s(ld : *mut LDAP) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_value_free(vals : *const ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_value_freeA(vals : *const ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_value_freeW(vals : *const ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("wldap32.dll" "cdecl" fn ldap_value_free_len(vals : *mut *mut LDAP_BERVAL) -> u32);
pub const LAPI_MAJOR_VER1: u32 = 1u32;
pub const LAPI_MINOR_VER1: u32 = 1u32;
pub const LBER_DEFAULT: i32 = -1i32;
pub const LBER_ERROR: i32 = -1i32;
pub const LBER_TRANSLATE_STRINGS: u32 = 4u32;
pub const LBER_USE_DER: u32 = 1u32;
pub const LBER_USE_INDEFINITE_LEN: u32 = 2u32;
pub const LDAP_ABANDON_CMD: i32 = 80i32;
pub const LDAP_ADD_CMD: i32 = 104i32;
pub const LDAP_ADMIN_LIMIT_EXCEEDED: LDAP_RETCODE = 11i32;
pub const LDAP_AFFECTS_MULTIPLE_DSAS: LDAP_RETCODE = 71i32;
pub const LDAP_ALIAS_DEREF_PROBLEM: LDAP_RETCODE = 36i32;
pub const LDAP_ALIAS_PROBLEM: LDAP_RETCODE = 33i32;
pub const LDAP_ALREADY_EXISTS: LDAP_RETCODE = 68i32;
pub const LDAP_API_FEATURE_VIRTUAL_LIST_VIEW: u32 = 1001u32;
pub const LDAP_API_INFO_VERSION: u32 = 1u32;
pub const LDAP_API_VERSION: u32 = 2004u32;
pub const LDAP_ATTRIBUTE_OR_VALUE_EXISTS: LDAP_RETCODE = 20i32;
pub const LDAP_AUTH_METHOD_NOT_SUPPORTED: LDAP_RETCODE = 7i32;
pub const LDAP_AUTH_OTHERKIND: i32 = 134i32;
pub const LDAP_AUTH_SASL: i32 = 131i32;
pub const LDAP_AUTH_SIMPLE: i32 = 128i32;
pub const LDAP_AUTH_UNKNOWN: LDAP_RETCODE = 86i32;
pub const LDAP_BIND_CMD: i32 = 96i32;
pub const LDAP_BUSY: LDAP_RETCODE = 51i32;
pub const LDAP_CAP_ACTIVE_DIRECTORY_ADAM_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1851");
pub const LDAP_CAP_ACTIVE_DIRECTORY_ADAM_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1851");
pub const LDAP_CAP_ACTIVE_DIRECTORY_LDAP_INTEG_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1791");
pub const LDAP_CAP_ACTIVE_DIRECTORY_LDAP_INTEG_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1791");
pub const LDAP_CAP_ACTIVE_DIRECTORY_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.800");
pub const LDAP_CAP_ACTIVE_DIRECTORY_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.800");
pub const LDAP_CAP_ACTIVE_DIRECTORY_PARTIAL_SECRETS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1920");
pub const LDAP_CAP_ACTIVE_DIRECTORY_PARTIAL_SECRETS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1920");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V51_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1670");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V51_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1670");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V60_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1935");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V60_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1935");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V61_OID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1935");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V61_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1935");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V61_R2_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2080");
pub const LDAP_CAP_ACTIVE_DIRECTORY_V61_R2_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2080");
pub const LDAP_CAP_ACTIVE_DIRECTORY_W8_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2237");
pub const LDAP_CAP_ACTIVE_DIRECTORY_W8_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2237");
pub const LDAP_CHASE_EXTERNAL_REFERRALS: u32 = 64u32;
pub const LDAP_CHASE_SUBORDINATE_REFERRALS: u32 = 32u32;
pub const LDAP_CLIENT_LOOP: LDAP_RETCODE = 96i32;
pub const LDAP_COMPARE_CMD: i32 = 110i32;
pub const LDAP_COMPARE_FALSE: LDAP_RETCODE = 5i32;
pub const LDAP_COMPARE_TRUE: LDAP_RETCODE = 6i32;
pub const LDAP_CONFIDENTIALITY_REQUIRED: LDAP_RETCODE = 13i32;
pub const LDAP_CONNECT_ERROR: LDAP_RETCODE = 91i32;
pub const LDAP_CONSTRAINT_VIOLATION: LDAP_RETCODE = 19i32;
pub const LDAP_CONTROL_NOT_FOUND: LDAP_RETCODE = 93i32;
pub const LDAP_CONTROL_REFERRALS: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.616");
pub const LDAP_CONTROL_REFERRALS_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.616");
pub const LDAP_CONTROL_VLVREQUEST: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("2.16.840.1.113730.3.4.9");
pub const LDAP_CONTROL_VLVREQUEST_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("2.16.840.1.113730.3.4.9");
pub const LDAP_CONTROL_VLVRESPONSE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("2.16.840.1.113730.3.4.10");
pub const LDAP_CONTROL_VLVRESPONSE_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("2.16.840.1.113730.3.4.10");
pub const LDAP_DECODING_ERROR: LDAP_RETCODE = 84i32;
pub const LDAP_DELETE_CMD: i32 = 74i32;
pub const LDAP_DEREF_ALWAYS: u32 = 3u32;
pub const LDAP_DEREF_FINDING: u32 = 2u32;
pub const LDAP_DEREF_NEVER: u32 = 0u32;
pub const LDAP_DEREF_SEARCHING: u32 = 1u32;
pub const LDAP_DIRSYNC_ANCESTORS_FIRST_ORDER: u32 = 2048u32;
pub const LDAP_DIRSYNC_INCREMENTAL_VALUES: u32 = 2147483648u32;
pub const LDAP_DIRSYNC_OBJECT_SECURITY: u32 = 1u32;
pub const LDAP_DIRSYNC_PUBLIC_DATA_ONLY: u32 = 8192u32;
pub const LDAP_DIRSYNC_ROPAS_DATA_ONLY: u32 = 1073741824u32;
pub const LDAP_ENCODING_ERROR: LDAP_RETCODE = 83i32;
pub const LDAP_EXTENDED_CMD: i32 = 119i32;
pub const LDAP_FEATURE_INFO_VERSION: u32 = 1u32;
pub const LDAP_FILTER_AND: u32 = 160u32;
pub const LDAP_FILTER_APPROX: u32 = 168u32;
pub const LDAP_FILTER_EQUALITY: u32 = 163u32;
pub const LDAP_FILTER_ERROR: LDAP_RETCODE = 87i32;
pub const LDAP_FILTER_EXTENSIBLE: u32 = 169u32;
pub const LDAP_FILTER_GE: u32 = 165u32;
pub const LDAP_FILTER_LE: u32 = 166u32;
pub const LDAP_FILTER_NOT: u32 = 162u32;
pub const LDAP_FILTER_OR: u32 = 161u32;
pub const LDAP_FILTER_PRESENT: u32 = 135u32;
pub const LDAP_FILTER_SUBSTRINGS: u32 = 164u32;
pub const LDAP_GC_PORT: u32 = 3268u32;
pub const LDAP_INAPPROPRIATE_AUTH: LDAP_RETCODE = 48i32;
pub const LDAP_INAPPROPRIATE_MATCHING: LDAP_RETCODE = 18i32;
pub const LDAP_INSUFFICIENT_RIGHTS: LDAP_RETCODE = 50i32;
pub const LDAP_INVALID_CMD: u32 = 255u32;
pub const LDAP_INVALID_CREDENTIALS: LDAP_RETCODE = 49i32;
pub const LDAP_INVALID_DN_SYNTAX: LDAP_RETCODE = 34i32;
pub const LDAP_INVALID_RES: u32 = 255u32;
pub const LDAP_INVALID_SYNTAX: LDAP_RETCODE = 21i32;
pub const LDAP_IS_LEAF: LDAP_RETCODE = 35i32;
pub const LDAP_LOCAL_ERROR: LDAP_RETCODE = 82i32;
pub const LDAP_LOOP_DETECT: LDAP_RETCODE = 54i32;
pub const LDAP_MATCHING_RULE_BIT_AND: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.803");
pub const LDAP_MATCHING_RULE_BIT_AND_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.803");
pub const LDAP_MATCHING_RULE_BIT_OR: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.804");
pub const LDAP_MATCHING_RULE_BIT_OR_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.804");
pub const LDAP_MATCHING_RULE_DN_BINARY_COMPLEX: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2253");
pub const LDAP_MATCHING_RULE_DN_BINARY_COMPLEX_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2253");
pub const LDAP_MATCHING_RULE_TRANSITIVE_EVALUATION: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1941");
pub const LDAP_MATCHING_RULE_TRANSITIVE_EVALUATION_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1941");
pub const LDAP_MODIFY_CMD: i32 = 102i32;
pub const LDAP_MODRDN_CMD: i32 = 108i32;
pub const LDAP_MOD_ADD: u32 = 0u32;
pub const LDAP_MOD_BVALUES: u32 = 128u32;
pub const LDAP_MOD_DELETE: u32 = 1u32;
pub const LDAP_MOD_REPLACE: u32 = 2u32;
pub const LDAP_MORE_RESULTS_TO_RETURN: LDAP_RETCODE = 95i32;
pub const LDAP_MSG_ALL: u32 = 1u32;
pub const LDAP_MSG_ONE: u32 = 0u32;
pub const LDAP_MSG_RECEIVED: u32 = 2u32;
pub const LDAP_NAMING_VIOLATION: LDAP_RETCODE = 64i32;
pub const LDAP_NOT_ALLOWED_ON_NONLEAF: LDAP_RETCODE = 66i32;
pub const LDAP_NOT_ALLOWED_ON_RDN: LDAP_RETCODE = 67i32;
pub const LDAP_NOT_SUPPORTED: LDAP_RETCODE = 92i32;
pub const LDAP_NO_LIMIT: u32 = 0u32;
pub const LDAP_NO_MEMORY: LDAP_RETCODE = 90i32;
pub const LDAP_NO_OBJECT_CLASS_MODS: LDAP_RETCODE = 69i32;
pub const LDAP_NO_RESULTS_RETURNED: LDAP_RETCODE = 94i32;
pub const LDAP_NO_SUCH_ATTRIBUTE: LDAP_RETCODE = 16i32;
pub const LDAP_NO_SUCH_OBJECT: LDAP_RETCODE = 32i32;
pub const LDAP_OBJECT_CLASS_VIOLATION: LDAP_RETCODE = 65i32;
pub const LDAP_OFFSET_RANGE_ERROR: LDAP_RETCODE = 61i32;
pub const LDAP_OPATT_ABANDON_REPL: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("abandonReplication");
pub const LDAP_OPATT_ABANDON_REPL_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("abandonReplication");
pub const LDAP_OPATT_BECOME_DOM_MASTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("becomeDomainMaster");
pub const LDAP_OPATT_BECOME_DOM_MASTER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("becomeDomainMaster");
pub const LDAP_OPATT_BECOME_PDC: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("becomePdc");
pub const LDAP_OPATT_BECOME_PDC_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("becomePdc");
pub const LDAP_OPATT_BECOME_RID_MASTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("becomeRidMaster");
pub const LDAP_OPATT_BECOME_RID_MASTER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("becomeRidMaster");
pub const LDAP_OPATT_BECOME_SCHEMA_MASTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("becomeSchemaMaster");
pub const LDAP_OPATT_BECOME_SCHEMA_MASTER_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("becomeSchemaMaster");
pub const LDAP_OPATT_CONFIG_NAMING_CONTEXT: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("configurationNamingContext");
pub const LDAP_OPATT_CONFIG_NAMING_CONTEXT_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("configurationNamingContext");
pub const LDAP_OPATT_CURRENT_TIME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("currentTime");
pub const LDAP_OPATT_CURRENT_TIME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("currentTime");
pub const LDAP_OPATT_DEFAULT_NAMING_CONTEXT: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("defaultNamingContext");
pub const LDAP_OPATT_DEFAULT_NAMING_CONTEXT_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("defaultNamingContext");
pub const LDAP_OPATT_DNS_HOST_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("dnsHostName");
pub const LDAP_OPATT_DNS_HOST_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("dnsHostName");
pub const LDAP_OPATT_DO_GARBAGE_COLLECTION: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("doGarbageCollection");
pub const LDAP_OPATT_DO_GARBAGE_COLLECTION_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("doGarbageCollection");
pub const LDAP_OPATT_DS_SERVICE_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("dsServiceName");
pub const LDAP_OPATT_DS_SERVICE_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("dsServiceName");
pub const LDAP_OPATT_FIXUP_INHERITANCE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("fixupInheritance");
pub const LDAP_OPATT_FIXUP_INHERITANCE_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("fixupInheritance");
pub const LDAP_OPATT_HIGHEST_COMMITTED_USN: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("highestCommitedUSN");
pub const LDAP_OPATT_HIGHEST_COMMITTED_USN_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("highestCommitedUSN");
pub const LDAP_OPATT_INVALIDATE_RID_POOL: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("invalidateRidPool");
pub const LDAP_OPATT_INVALIDATE_RID_POOL_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("invalidateRidPool");
pub const LDAP_OPATT_LDAP_SERVICE_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ldapServiceName");
pub const LDAP_OPATT_LDAP_SERVICE_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ldapServiceName");
pub const LDAP_OPATT_NAMING_CONTEXTS: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("namingContexts");
pub const LDAP_OPATT_NAMING_CONTEXTS_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("namingContexts");
pub const LDAP_OPATT_RECALC_HIERARCHY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("recalcHierarchy");
pub const LDAP_OPATT_RECALC_HIERARCHY_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("recalcHierarchy");
pub const LDAP_OPATT_ROOT_DOMAIN_NAMING_CONTEXT: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("rootDomainNamingContext");
pub const LDAP_OPATT_ROOT_DOMAIN_NAMING_CONTEXT_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("rootDomainNamingContext");
pub const LDAP_OPATT_SCHEMA_NAMING_CONTEXT: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("schemaNamingContext");
pub const LDAP_OPATT_SCHEMA_NAMING_CONTEXT_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("schemaNamingContext");
pub const LDAP_OPATT_SCHEMA_UPDATE_NOW: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("schemaUpdateNow");
pub const LDAP_OPATT_SCHEMA_UPDATE_NOW_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("schemaUpdateNow");
pub const LDAP_OPATT_SERVER_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("serverName");
pub const LDAP_OPATT_SERVER_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("serverName");
pub const LDAP_OPATT_SUBSCHEMA_SUBENTRY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("subschemaSubentry");
pub const LDAP_OPATT_SUBSCHEMA_SUBENTRY_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("subschemaSubentry");
pub const LDAP_OPATT_SUPPORTED_CAPABILITIES: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("supportedCapabilities");
pub const LDAP_OPATT_SUPPORTED_CAPABILITIES_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("supportedCapabilities");
pub const LDAP_OPATT_SUPPORTED_CONTROL: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("supportedControl");
pub const LDAP_OPATT_SUPPORTED_CONTROL_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("supportedControl");
pub const LDAP_OPATT_SUPPORTED_LDAP_POLICIES: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("supportedLDAPPolicies");
pub const LDAP_OPATT_SUPPORTED_LDAP_POLICIES_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("supportedLDAPPolicies");
pub const LDAP_OPATT_SUPPORTED_LDAP_VERSION: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("supportedLDAPVersion");
pub const LDAP_OPATT_SUPPORTED_LDAP_VERSION_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("supportedLDAPVersion");
pub const LDAP_OPATT_SUPPORTED_SASL_MECHANISM: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("supportedSASLMechanisms");
pub const LDAP_OPATT_SUPPORTED_SASL_MECHANISM_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("supportedSASLMechanisms");
pub const LDAP_OPERATIONS_ERROR: LDAP_RETCODE = 1i32;
pub const LDAP_OPT_API_FEATURE_INFO: u32 = 21u32;
pub const LDAP_OPT_API_INFO: u32 = 0u32;
pub const LDAP_OPT_AREC_EXCLUSIVE: u32 = 152u32;
pub const LDAP_OPT_AUTO_RECONNECT: u32 = 145u32;
pub const LDAP_OPT_CACHE_ENABLE: u32 = 15u32;
pub const LDAP_OPT_CACHE_FN_PTRS: u32 = 13u32;
pub const LDAP_OPT_CACHE_STRATEGY: u32 = 14u32;
pub const LDAP_OPT_CHASE_REFERRALS: u32 = 2u32;
pub const LDAP_OPT_CLIENT_CERTIFICATE: u32 = 128u32;
pub const LDAP_OPT_DEREF: u32 = 2u32;
pub const LDAP_OPT_DESC: u32 = 1u32;
pub const LDAP_OPT_DNS: u32 = 1u32;
pub const LDAP_OPT_DNSDOMAIN_NAME: u32 = 59u32;
pub const LDAP_OPT_ENCRYPT: u32 = 150u32;
pub const LDAP_OPT_ERROR_NUMBER: u32 = 49u32;
pub const LDAP_OPT_ERROR_STRING: u32 = 50u32;
pub const LDAP_OPT_FAST_CONCURRENT_BIND: u32 = 65u32;
pub const LDAP_OPT_GETDSNAME_FLAGS: u32 = 61u32;
pub const LDAP_OPT_HOST_NAME: u32 = 48u32;
pub const LDAP_OPT_HOST_REACHABLE: u32 = 62u32;
pub const LDAP_OPT_IO_FN_PTRS: u32 = 11u32;
pub const LDAP_OPT_PING_KEEP_ALIVE: u32 = 54u32;
pub const LDAP_OPT_PING_LIMIT: u32 = 56u32;
pub const LDAP_OPT_PING_WAIT_TIME: u32 = 55u32;
pub const LDAP_OPT_PROMPT_CREDENTIALS: u32 = 63u32;
pub const LDAP_OPT_PROTOCOL_VERSION: u32 = 17u32;
pub const LDAP_OPT_REBIND_ARG: u32 = 7u32;
pub const LDAP_OPT_REBIND_FN: u32 = 6u32;
pub const LDAP_OPT_REFERRALS: u32 = 8u32;
pub const LDAP_OPT_REFERRAL_CALLBACK: u32 = 112u32;
pub const LDAP_OPT_REFERRAL_HOP_LIMIT: u32 = 16u32;
pub const LDAP_OPT_REF_DEREF_CONN_PER_MSG: u32 = 148u32;
pub const LDAP_OPT_RESTART: u32 = 9u32;
pub const LDAP_OPT_RETURN_REFS: u32 = 4u32;
pub const LDAP_OPT_ROOTDSE_CACHE: u32 = 154u32;
pub const LDAP_OPT_SASL_METHOD: u32 = 151u32;
pub const LDAP_OPT_SCH_FLAGS: u32 = 67u32;
pub const LDAP_OPT_SECURITY_CONTEXT: u32 = 153u32;
pub const LDAP_OPT_SEND_TIMEOUT: u32 = 66u32;
pub const LDAP_OPT_SERVER_CERTIFICATE: u32 = 129u32;
pub const LDAP_OPT_SERVER_ERROR: u32 = 51u32;
pub const LDAP_OPT_SERVER_EXT_ERROR: u32 = 52u32;
pub const LDAP_OPT_SIGN: u32 = 149u32;
pub const LDAP_OPT_SIZELIMIT: u32 = 3u32;
pub const LDAP_OPT_SOCKET_BIND_ADDRESSES: u32 = 68u32;
pub const LDAP_OPT_SSL: u32 = 10u32;
pub const LDAP_OPT_SSL_INFO: u32 = 147u32;
pub const LDAP_OPT_SSPI_FLAGS: u32 = 146u32;
pub const LDAP_OPT_TCP_KEEPALIVE: u32 = 64u32;
pub const LDAP_OPT_THREAD_FN_PTRS: u32 = 5u32;
pub const LDAP_OPT_TIMELIMIT: u32 = 4u32;
pub const LDAP_OPT_TLS: u32 = 10u32;
pub const LDAP_OPT_TLS_INFO: u32 = 147u32;
pub const LDAP_OPT_VERSION: u32 = 17u32;
pub const LDAP_OTHER: LDAP_RETCODE = 80i32;
pub const LDAP_PAGED_RESULT_OID_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.319");
pub const LDAP_PAGED_RESULT_OID_STRING_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.319");
pub const LDAP_PARAM_ERROR: LDAP_RETCODE = 89i32;
pub const LDAP_PARTIAL_RESULTS: LDAP_RETCODE = 9i32;
pub const LDAP_POLICYHINT_APPLY_FULLPWDPOLICY: u32 = 1u32;
pub const LDAP_PORT: u32 = 389u32;
pub const LDAP_PROTOCOL_ERROR: LDAP_RETCODE = 2i32;
pub const LDAP_REFERRAL: LDAP_RETCODE = 10i32;
pub const LDAP_REFERRAL_LIMIT_EXCEEDED: LDAP_RETCODE = 97i32;
pub const LDAP_REFERRAL_V2: LDAP_RETCODE = 9i32;
pub const LDAP_RESULTS_TOO_LARGE: LDAP_RETCODE = 70i32;
pub const LDAP_RES_ADD: i32 = 105i32;
pub const LDAP_RES_ANY: i32 = -1i32;
pub const LDAP_RES_BIND: i32 = 97i32;
pub const LDAP_RES_COMPARE: i32 = 111i32;
pub const LDAP_RES_DELETE: i32 = 107i32;
pub const LDAP_RES_EXTENDED: i32 = 120i32;
pub const LDAP_RES_MODIFY: i32 = 103i32;
pub const LDAP_RES_MODRDN: i32 = 109i32;
pub const LDAP_RES_REFERRAL: i32 = 115i32;
pub const LDAP_RES_SEARCH_ENTRY: i32 = 100i32;
pub const LDAP_RES_SEARCH_RESULT: i32 = 101i32;
pub const LDAP_RES_SESSION: i32 = 114i32;
pub const LDAP_SASL_BIND_IN_PROGRESS: LDAP_RETCODE = 14i32;
pub const LDAP_SCOPE_BASE: u32 = 0u32;
pub const LDAP_SCOPE_ONELEVEL: u32 = 1u32;
pub const LDAP_SCOPE_SUBTREE: u32 = 2u32;
pub const LDAP_SEARCH_CMD: i32 = 99i32;
pub const LDAP_SEARCH_HINT_INDEX_ONLY_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2207");
pub const LDAP_SEARCH_HINT_INDEX_ONLY_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2207");
pub const LDAP_SEARCH_HINT_REQUIRED_INDEX_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2306");
pub const LDAP_SEARCH_HINT_REQUIRED_INDEX_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2306");
pub const LDAP_SEARCH_HINT_SOFT_SIZE_LIMIT_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2210");
pub const LDAP_SEARCH_HINT_SOFT_SIZE_LIMIT_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2210");
pub const LDAP_SERVER_ASQ_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1504");
pub const LDAP_SERVER_ASQ_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1504");
pub const LDAP_SERVER_BATCH_REQUEST_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2212");
pub const LDAP_SERVER_BATCH_REQUEST_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2212");
pub const LDAP_SERVER_BYPASS_QUOTA_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2256");
pub const LDAP_SERVER_BYPASS_QUOTA_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2256");
pub const LDAP_SERVER_CROSSDOM_MOVE_TARGET_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.521");
pub const LDAP_SERVER_CROSSDOM_MOVE_TARGET_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.521");
pub const LDAP_SERVER_DIRSYNC_EX_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2090");
pub const LDAP_SERVER_DIRSYNC_EX_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2090");
pub const LDAP_SERVER_DIRSYNC_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.841");
pub const LDAP_SERVER_DIRSYNC_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.841");
pub const LDAP_SERVER_DN_INPUT_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2026");
pub const LDAP_SERVER_DN_INPUT_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2026");
pub const LDAP_SERVER_DOMAIN_SCOPE_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1339");
pub const LDAP_SERVER_DOMAIN_SCOPE_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1339");
pub const LDAP_SERVER_DOWN: LDAP_RETCODE = 81i32;
pub const LDAP_SERVER_EXPECTED_ENTRY_COUNT_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2211");
pub const LDAP_SERVER_EXPECTED_ENTRY_COUNT_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2211");
pub const LDAP_SERVER_EXTENDED_DN_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.529");
pub const LDAP_SERVER_EXTENDED_DN_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.529");
pub const LDAP_SERVER_FAST_BIND_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1781");
pub const LDAP_SERVER_FAST_BIND_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1781");
pub const LDAP_SERVER_FORCE_UPDATE_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1974");
pub const LDAP_SERVER_FORCE_UPDATE_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1974");
pub const LDAP_SERVER_GET_STATS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.970");
pub const LDAP_SERVER_GET_STATS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.970");
pub const LDAP_SERVER_LAZY_COMMIT_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.619");
pub const LDAP_SERVER_LAZY_COMMIT_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.619");
pub const LDAP_SERVER_LINK_TTL_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2309");
pub const LDAP_SERVER_LINK_TTL_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2309");
pub const LDAP_SERVER_NOTIFICATION_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.528");
pub const LDAP_SERVER_NOTIFICATION_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.528");
pub const LDAP_SERVER_PERMISSIVE_MODIFY_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1413");
pub const LDAP_SERVER_PERMISSIVE_MODIFY_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1413");
pub const LDAP_SERVER_POLICY_HINTS_DEPRECATED_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2066");
pub const LDAP_SERVER_POLICY_HINTS_DEPRECATED_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2066");
pub const LDAP_SERVER_POLICY_HINTS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2239");
pub const LDAP_SERVER_POLICY_HINTS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2239");
pub const LDAP_SERVER_QUOTA_CONTROL_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1852");
pub const LDAP_SERVER_QUOTA_CONTROL_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1852");
pub const LDAP_SERVER_RANGE_OPTION_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.802");
pub const LDAP_SERVER_RANGE_OPTION_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.802");
pub const LDAP_SERVER_RANGE_RETRIEVAL_NOERR_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1948");
pub const LDAP_SERVER_RANGE_RETRIEVAL_NOERR_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1948");
pub const LDAP_SERVER_RESP_SORT_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.474");
pub const LDAP_SERVER_RESP_SORT_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.474");
pub const LDAP_SERVER_SD_FLAGS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.801");
pub const LDAP_SERVER_SD_FLAGS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.801");
pub const LDAP_SERVER_SEARCH_HINTS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2206");
pub const LDAP_SERVER_SEARCH_HINTS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2206");
pub const LDAP_SERVER_SEARCH_OPTIONS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1340");
pub const LDAP_SERVER_SEARCH_OPTIONS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1340");
pub const LDAP_SERVER_SET_OWNER_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2255");
pub const LDAP_SERVER_SET_OWNER_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2255");
pub const LDAP_SERVER_SHOW_DEACTIVATED_LINK_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2065");
pub const LDAP_SERVER_SHOW_DEACTIVATED_LINK_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2065");
pub const LDAP_SERVER_SHOW_DELETED_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.417");
pub const LDAP_SERVER_SHOW_DELETED_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.417");
pub const LDAP_SERVER_SHOW_RECYCLED_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2064");
pub const LDAP_SERVER_SHOW_RECYCLED_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2064");
pub const LDAP_SERVER_SHUTDOWN_NOTIFY_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1907");
pub const LDAP_SERVER_SHUTDOWN_NOTIFY_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1907");
pub const LDAP_SERVER_SORT_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.473");
pub const LDAP_SERVER_SORT_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.473");
pub const LDAP_SERVER_TREE_DELETE_EX_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2204");
pub const LDAP_SERVER_TREE_DELETE_EX_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2204");
pub const LDAP_SERVER_TREE_DELETE_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.805");
pub const LDAP_SERVER_TREE_DELETE_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.805");
pub const LDAP_SERVER_UPDATE_STATS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2205");
pub const LDAP_SERVER_UPDATE_STATS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2205");
pub const LDAP_SERVER_VERIFY_NAME_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.1338");
pub const LDAP_SERVER_VERIFY_NAME_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.1338");
pub const LDAP_SERVER_WHO_AM_I_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.4203.1.11.3");
pub const LDAP_SERVER_WHO_AM_I_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.3.6.1.4.1.4203.1.11.3");
pub const LDAP_SESSION_CMD: i32 = 113i32;
pub const LDAP_SIZELIMIT_EXCEEDED: LDAP_RETCODE = 4i32;
pub const LDAP_SORT_CONTROL_MISSING: LDAP_RETCODE = 60i32;
pub const LDAP_SSL_GC_PORT: u32 = 3269u32;
pub const LDAP_SSL_PORT: u32 = 636u32;
pub const LDAP_START_TLS_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.1466.20037");
pub const LDAP_START_TLS_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.3.6.1.4.1.1466.20037");
pub const LDAP_STRONG_AUTH_REQUIRED: LDAP_RETCODE = 8i32;
pub const LDAP_SUBSTRING_ANY: i32 = 129i32;
pub const LDAP_SUBSTRING_FINAL: i32 = 130i32;
pub const LDAP_SUBSTRING_INITIAL: i32 = 128i32;
pub const LDAP_SUCCESS: LDAP_RETCODE = 0i32;
pub const LDAP_TIMELIMIT_EXCEEDED: LDAP_RETCODE = 3i32;
pub const LDAP_TIMEOUT: LDAP_RETCODE = 85i32;
pub const LDAP_TTL_EXTENDED_OP_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.3.6.1.4.1.1466.101.119.1");
pub const LDAP_TTL_EXTENDED_OP_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.3.6.1.4.1.1466.101.119.1");
pub const LDAP_UNAVAILABLE: LDAP_RETCODE = 52i32;
pub const LDAP_UNAVAILABLE_CRIT_EXTENSION: LDAP_RETCODE = 12i32;
pub const LDAP_UNBIND_CMD: i32 = 66i32;
pub const LDAP_UNDEFINED_TYPE: LDAP_RETCODE = 17i32;
pub const LDAP_UNICODE: u32 = 1u32;
pub const LDAP_UNWILLING_TO_PERFORM: LDAP_RETCODE = 53i32;
pub const LDAP_UPDATE_STATS_INVOCATIONID_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2209");
pub const LDAP_UPDATE_STATS_INVOCATIONID_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2209");
pub const LDAP_UPDATE_STATS_USN_OID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("1.2.840.113556.1.4.2208");
pub const LDAP_UPDATE_STATS_USN_OID_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("1.2.840.113556.1.4.2208");
pub const LDAP_USER_CANCELLED: LDAP_RETCODE = 88i32;
pub const LDAP_VENDOR_NAME: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Microsoft Corporation.");
pub const LDAP_VENDOR_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Microsoft Corporation.");
pub const LDAP_VENDOR_VERSION: u32 = 510u32;
pub const LDAP_VERSION: u32 = 2u32;
pub const LDAP_VERSION1: u32 = 1u32;
pub const LDAP_VERSION2: u32 = 2u32;
pub const LDAP_VERSION3: u32 = 3u32;
pub const LDAP_VERSION_MAX: u32 = 3u32;
pub const LDAP_VERSION_MIN: u32 = 2u32;
pub const LDAP_VIRTUAL_LIST_VIEW_ERROR: LDAP_RETCODE = 76i32;
pub const LDAP_VLVINFO_VERSION: u32 = 1u32;
pub const SERVER_SEARCH_FLAG_DOMAIN_SCOPE: u32 = 1u32;
pub const SERVER_SEARCH_FLAG_PHANTOM_ROOT: u32 = 2u32;
pub type LDAP_RETCODE = i32;
#[repr(C)]
pub struct BerElement {
    pub opaque: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for BerElement {}
impl ::core::clone::Clone for BerElement {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAP {
    pub ld_sb: LDAP_0,
    pub ld_host: ::windows_sys::core::PSTR,
    pub ld_version: u32,
    pub ld_lberoptions: u8,
    pub ld_deref: u32,
    pub ld_timelimit: u32,
    pub ld_sizelimit: u32,
    pub ld_errno: u32,
    pub ld_matched: ::windows_sys::core::PSTR,
    pub ld_error: ::windows_sys::core::PSTR,
    pub ld_msgid: u32,
    pub Reserved3: [u8; 25],
    pub ld_cldaptries: u32,
    pub ld_cldaptimeout: u32,
    pub ld_refhoplimit: u32,
    pub ld_options: u32,
}
impl ::core::marker::Copy for LDAP {}
impl ::core::clone::Clone for LDAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAP_0 {
    pub sb_sd: usize,
    pub Reserved1: [u8; 41],
    pub sb_naddr: usize,
    pub Reserved2: [u8; 24],
}
impl ::core::marker::Copy for LDAP_0 {}
impl ::core::clone::Clone for LDAP_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPAPIFeatureInfoA {
    pub ldapaif_info_version: i32,
    pub ldapaif_name: ::windows_sys::core::PSTR,
    pub ldapaif_version: i32,
}
impl ::core::marker::Copy for LDAPAPIFeatureInfoA {}
impl ::core::clone::Clone for LDAPAPIFeatureInfoA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPAPIFeatureInfoW {
    pub ldapaif_info_version: i32,
    pub ldapaif_name: ::windows_sys::core::PWSTR,
    pub ldapaif_version: i32,
}
impl ::core::marker::Copy for LDAPAPIFeatureInfoW {}
impl ::core::clone::Clone for LDAPAPIFeatureInfoW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPAPIInfoA {
    pub ldapai_info_version: i32,
    pub ldapai_api_version: i32,
    pub ldapai_protocol_version: i32,
    pub ldapai_extensions: *mut *mut i8,
    pub ldapai_vendor_name: ::windows_sys::core::PSTR,
    pub ldapai_vendor_version: i32,
}
impl ::core::marker::Copy for LDAPAPIInfoA {}
impl ::core::clone::Clone for LDAPAPIInfoA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPAPIInfoW {
    pub ldapai_info_version: i32,
    pub ldapai_api_version: i32,
    pub ldapai_protocol_version: i32,
    pub ldapai_extensions: *mut ::windows_sys::core::PWSTR,
    pub ldapai_vendor_name: ::windows_sys::core::PWSTR,
    pub ldapai_vendor_version: i32,
}
impl ::core::marker::Copy for LDAPAPIInfoW {}
impl ::core::clone::Clone for LDAPAPIInfoW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LDAPControlA {
    pub ldctl_oid: ::windows_sys::core::PSTR,
    pub ldctl_value: LDAP_BERVAL,
    pub ldctl_iscritical: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LDAPControlA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LDAPControlA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LDAPControlW {
    pub ldctl_oid: ::windows_sys::core::PWSTR,
    pub ldctl_value: LDAP_BERVAL,
    pub ldctl_iscritical: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LDAPControlW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LDAPControlW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LDAPMessage {
    pub lm_msgid: u32,
    pub lm_msgtype: u32,
    pub lm_ber: *mut ::core::ffi::c_void,
    pub lm_chain: *mut LDAPMessage,
    pub lm_next: *mut LDAPMessage,
    pub lm_time: u32,
    pub Connection: *mut LDAP,
    pub Request: *mut ::core::ffi::c_void,
    pub lm_returncode: u32,
    pub lm_referral: u16,
    pub lm_chased: super::super::Foundation::BOOLEAN,
    pub lm_eom: super::super::Foundation::BOOLEAN,
    pub ConnectionReferenced: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LDAPMessage {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LDAPMessage {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPModA {
    pub mod_op: u32,
    pub mod_type: ::windows_sys::core::PSTR,
    pub mod_vals: LDAPModA_0,
}
impl ::core::marker::Copy for LDAPModA {}
impl ::core::clone::Clone for LDAPModA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union LDAPModA_0 {
    pub modv_strvals: *mut ::windows_sys::core::PSTR,
    pub modv_bvals: *mut *mut LDAP_BERVAL,
}
impl ::core::marker::Copy for LDAPModA_0 {}
impl ::core::clone::Clone for LDAPModA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPModW {
    pub mod_op: u32,
    pub mod_type: ::windows_sys::core::PWSTR,
    pub mod_vals: LDAPModW_0,
}
impl ::core::marker::Copy for LDAPModW {}
impl ::core::clone::Clone for LDAPModW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union LDAPModW_0 {
    pub modv_strvals: *mut ::windows_sys::core::PWSTR,
    pub modv_bvals: *mut *mut LDAP_BERVAL,
}
impl ::core::marker::Copy for LDAPModW_0 {}
impl ::core::clone::Clone for LDAPModW_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LDAPSortKeyA {
    pub sk_attrtype: ::windows_sys::core::PSTR,
    pub sk_matchruleoid: ::windows_sys::core::PSTR,
    pub sk_reverseorder: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LDAPSortKeyA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LDAPSortKeyA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LDAPSortKeyW {
    pub sk_attrtype: ::windows_sys::core::PWSTR,
    pub sk_matchruleoid: ::windows_sys::core::PWSTR,
    pub sk_reverseorder: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LDAPSortKeyW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LDAPSortKeyW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAPVLVInfo {
    pub ldvlv_version: i32,
    pub ldvlv_before_count: u32,
    pub ldvlv_after_count: u32,
    pub ldvlv_offset: u32,
    pub ldvlv_count: u32,
    pub ldvlv_attrvalue: *mut LDAP_BERVAL,
    pub ldvlv_context: *mut LDAP_BERVAL,
    pub ldvlv_extradata: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for LDAPVLVInfo {}
impl ::core::clone::Clone for LDAPVLVInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAP_BERVAL {
    pub bv_len: u32,
    pub bv_val: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for LDAP_BERVAL {}
impl ::core::clone::Clone for LDAP_BERVAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LDAP_REFERRAL_CALLBACK {
    pub SizeOfCallbacks: u32,
    pub QueryForConnection: QUERYFORCONNECTION,
    pub NotifyRoutine: NOTIFYOFNEWCONNECTION,
    pub DereferenceRoutine: DEREFERENCECONNECTION,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LDAP_REFERRAL_CALLBACK {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LDAP_REFERRAL_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAP_TIMEVAL {
    pub tv_sec: i32,
    pub tv_usec: i32,
}
impl ::core::marker::Copy for LDAP_TIMEVAL {}
impl ::core::clone::Clone for LDAP_TIMEVAL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDAP_VERSION_INFO {
    pub lv_size: u32,
    pub lv_major: u32,
    pub lv_minor: u32,
}
impl ::core::marker::Copy for LDAP_VERSION_INFO {}
impl ::core::clone::Clone for LDAP_VERSION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PLDAPSearch = isize;
pub type DBGPRINT = ::core::option::Option<unsafe extern "system" fn(format: ::windows_sys::core::PCSTR) -> u32>;
pub type DEREFERENCECONNECTION = ::core::option::Option<unsafe extern "system" fn(primaryconnection: *mut LDAP, connectiontodereference: *mut LDAP) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type NOTIFYOFNEWCONNECTION = ::core::option::Option<unsafe extern "system" fn(primaryconnection: *mut LDAP, referralfromconnection: *mut LDAP, newdn: ::windows_sys::core::PCWSTR, hostname: ::windows_sys::core::PCSTR, newconnection: *mut LDAP, portnumber: u32, secauthidentity: *mut ::core::ffi::c_void, currentuser: *mut ::core::ffi::c_void, errorcodefrombind: u32) -> super::super::Foundation::BOOLEAN>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Authentication_Identity\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Authentication_Identity", feature = "Win32_Security_Cryptography"))]
pub type QUERYCLIENTCERT = ::core::option::Option<unsafe extern "system" fn(connection: *mut LDAP, trusted_cas: *mut super::super::Security::Authentication::Identity::SecPkgContext_IssuerListInfoEx, ppcertificate: *mut *mut super::super::Security::Cryptography::CERT_CONTEXT) -> super::super::Foundation::BOOLEAN>;
pub type QUERYFORCONNECTION = ::core::option::Option<unsafe extern "system" fn(primaryconnection: *mut LDAP, referralfromconnection: *mut LDAP, newdn: ::windows_sys::core::PCWSTR, hostname: ::windows_sys::core::PCSTR, portnumber: u32, secauthidentity: *mut ::core::ffi::c_void, currentusertoken: *mut ::core::ffi::c_void, connectiontouse: *mut *mut LDAP) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
pub type VERIFYSERVERCERT = ::core::option::Option<unsafe extern "system" fn(connection: *mut LDAP, pservercert: *mut *mut super::super::Security::Cryptography::CERT_CONTEXT) -> super::super::Foundation::BOOLEAN>;
