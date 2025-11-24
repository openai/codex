#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzAccessCheck(flags : AUTHZ_ACCESS_CHECK_FLAGS, hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, prequest : *const AUTHZ_ACCESS_REQUEST, hauditevent : AUTHZ_AUDIT_EVENT_HANDLE, psecuritydescriptor : super:: PSECURITY_DESCRIPTOR, optionalsecuritydescriptorarray : *const super:: PSECURITY_DESCRIPTOR, optionalsecuritydescriptorcount : u32, preply : *mut AUTHZ_ACCESS_REPLY, phaccesscheckresults : *mut AUTHZ_ACCESS_CHECK_RESULTS_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzAddSidsToContext(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, sids : *const super:: SID_AND_ATTRIBUTES, sidcount : u32, restrictedsids : *const super:: SID_AND_ATTRIBUTES, restrictedsidcount : u32, phnewauthzclientcontext : *mut AUTHZ_CLIENT_CONTEXT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzCachedAccessCheck(flags : u32, haccesscheckresults : AUTHZ_ACCESS_CHECK_RESULTS_HANDLE, prequest : *const AUTHZ_ACCESS_REQUEST, hauditevent : AUTHZ_AUDIT_EVENT_HANDLE, preply : *mut AUTHZ_ACCESS_REPLY) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzEnumerateSecurityEventSources(dwflags : u32, buffer : *mut AUTHZ_SOURCE_SCHEMA_REGISTRATION, pdwcount : *mut u32, pdwlength : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzEvaluateSacl(authzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, prequest : *const AUTHZ_ACCESS_REQUEST, sacl : *const super:: ACL, grantedaccess : u32, accessgranted : super::super::Foundation:: BOOL, pbgenerateaudit : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzFreeAuditEvent(hauditevent : AUTHZ_AUDIT_EVENT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzFreeCentralAccessPolicyCache() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzFreeContext(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzFreeHandle(haccesscheckresults : AUTHZ_ACCESS_CHECK_RESULTS_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzFreeResourceManager(hauthzresourcemanager : AUTHZ_RESOURCE_MANAGER_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzGetInformationFromContext(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, infoclass : AUTHZ_CONTEXT_INFORMATION_CLASS, buffersize : u32, psizerequired : *mut u32, buffer : *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeCompoundContext(usercontext : AUTHZ_CLIENT_CONTEXT_HANDLE, devicecontext : AUTHZ_CLIENT_CONTEXT_HANDLE, phcompoundcontext : *mut AUTHZ_CLIENT_CONTEXT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeContextFromAuthzContext(flags : u32, hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, pexpirationtime : *const i64, identifier : super::super::Foundation:: LUID, dynamicgroupargs : *const ::core::ffi::c_void, phnewauthzclientcontext : *mut AUTHZ_CLIENT_CONTEXT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeContextFromSid(flags : u32, usersid : super::super::Foundation:: PSID, hauthzresourcemanager : AUTHZ_RESOURCE_MANAGER_HANDLE, pexpirationtime : *const i64, identifier : super::super::Foundation:: LUID, dynamicgroupargs : *const ::core::ffi::c_void, phauthzclientcontext : *mut AUTHZ_CLIENT_CONTEXT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeContextFromToken(flags : u32, tokenhandle : super::super::Foundation:: HANDLE, hauthzresourcemanager : AUTHZ_RESOURCE_MANAGER_HANDLE, pexpirationtime : *const i64, identifier : super::super::Foundation:: LUID, dynamicgroupargs : *const ::core::ffi::c_void, phauthzclientcontext : *mut AUTHZ_CLIENT_CONTEXT_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeObjectAccessAuditEvent(flags : AUTHZ_INITIALIZE_OBJECT_ACCESS_AUDIT_EVENT_FLAGS, hauditeventtype : AUTHZ_AUDIT_EVENT_TYPE_HANDLE, szoperationtype : ::windows_sys::core::PCWSTR, szobjecttype : ::windows_sys::core::PCWSTR, szobjectname : ::windows_sys::core::PCWSTR, szadditionalinfo : ::windows_sys::core::PCWSTR, phauditevent : *mut AUTHZ_AUDIT_EVENT_HANDLE, dwadditionalparametercount : u32, ...) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeObjectAccessAuditEvent2(flags : u32, hauditeventtype : AUTHZ_AUDIT_EVENT_TYPE_HANDLE, szoperationtype : ::windows_sys::core::PCWSTR, szobjecttype : ::windows_sys::core::PCWSTR, szobjectname : ::windows_sys::core::PCWSTR, szadditionalinfo : ::windows_sys::core::PCWSTR, szadditionalinfo2 : ::windows_sys::core::PCWSTR, phauditevent : *mut AUTHZ_AUDIT_EVENT_HANDLE, dwadditionalparametercount : u32, ...) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeRemoteResourceManager(prpcinitinfo : *const AUTHZ_RPC_INIT_INFO_CLIENT, phauthzresourcemanager : *mut AUTHZ_RESOURCE_MANAGER_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeResourceManager(flags : u32, pfndynamicaccesscheck : PFN_AUTHZ_DYNAMIC_ACCESS_CHECK, pfncomputedynamicgroups : PFN_AUTHZ_COMPUTE_DYNAMIC_GROUPS, pfnfreedynamicgroups : PFN_AUTHZ_FREE_DYNAMIC_GROUPS, szresourcemanagername : ::windows_sys::core::PCWSTR, phauthzresourcemanager : *mut AUTHZ_RESOURCE_MANAGER_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInitializeResourceManagerEx(flags : AUTHZ_RESOURCE_MANAGER_FLAGS, pauthzinitinfo : *const AUTHZ_INIT_INFO, phauthzresourcemanager : *mut AUTHZ_RESOURCE_MANAGER_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzInstallSecurityEventSource(dwflags : u32, pregistration : *const AUTHZ_SOURCE_SCHEMA_REGISTRATION) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzModifyClaims(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, claimclass : AUTHZ_CONTEXT_INFORMATION_CLASS, pclaimoperations : *const AUTHZ_SECURITY_ATTRIBUTE_OPERATION, pclaims : *const AUTHZ_SECURITY_ATTRIBUTES_INFORMATION) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzModifySecurityAttributes(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, poperations : *const AUTHZ_SECURITY_ATTRIBUTE_OPERATION, pattributes : *const AUTHZ_SECURITY_ATTRIBUTES_INFORMATION) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzModifySids(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, sidclass : AUTHZ_CONTEXT_INFORMATION_CLASS, psidoperations : *const AUTHZ_SID_OPERATION, psids : *const super:: TOKEN_GROUPS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzOpenObjectAudit(flags : u32, hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, prequest : *const AUTHZ_ACCESS_REQUEST, hauditevent : AUTHZ_AUDIT_EVENT_HANDLE, psecuritydescriptor : super:: PSECURITY_DESCRIPTOR, optionalsecuritydescriptorarray : *const super:: PSECURITY_DESCRIPTOR, optionalsecuritydescriptorcount : u32, preply : *const AUTHZ_ACCESS_REPLY) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"] fn AuthzRegisterCapChangeNotification(phcapchangesubscription : *mut AUTHZ_CAP_CHANGE_SUBSCRIPTION_HANDLE, pfncapchangecallback : super::super::System::Threading:: LPTHREAD_START_ROUTINE, pcallbackcontext : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzRegisterSecurityEventSource(dwflags : u32, szeventsourcename : ::windows_sys::core::PCWSTR, pheventprovider : *mut AUTHZ_SECURITY_EVENT_PROVIDER_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzReportSecurityEvent(dwflags : u32, heventprovider : AUTHZ_SECURITY_EVENT_PROVIDER_HANDLE, dwauditid : u32, pusersid : super::super::Foundation:: PSID, dwcount : u32, ...) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzReportSecurityEventFromParams(dwflags : u32, heventprovider : AUTHZ_SECURITY_EVENT_PROVIDER_HANDLE, dwauditid : u32, pusersid : super::super::Foundation:: PSID, pparams : *const AUDIT_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzSetAppContainerInformation(hauthzclientcontext : AUTHZ_CLIENT_CONTEXT_HANDLE, pappcontainersid : super::super::Foundation:: PSID, capabilitycount : u32, pcapabilitysids : *const super:: SID_AND_ATTRIBUTES) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzUninstallSecurityEventSource(dwflags : u32, szeventsourcename : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzUnregisterCapChangeNotification(hcapchangesubscription : AUTHZ_CAP_CHANGE_SUBSCRIPTION_HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("authz.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AuthzUnregisterSecurityEventSource(dwflags : u32, pheventprovider : *mut AUTHZ_SECURITY_EVENT_PROVIDER_HANDLE) -> super::super::Foundation:: BOOL);
::windows_targets::link!("advapi32.dll" "system" fn BuildExplicitAccessWithNameA(pexplicitaccess : *mut EXPLICIT_ACCESS_A, ptrusteename : ::windows_sys::core::PCSTR, accesspermissions : u32, accessmode : ACCESS_MODE, inheritance : super:: ACE_FLAGS) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildExplicitAccessWithNameW(pexplicitaccess : *mut EXPLICIT_ACCESS_W, ptrusteename : ::windows_sys::core::PCWSTR, accesspermissions : u32, accessmode : ACCESS_MODE, inheritance : super:: ACE_FLAGS) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildImpersonateExplicitAccessWithNameA(pexplicitaccess : *mut EXPLICIT_ACCESS_A, ptrusteename : ::windows_sys::core::PCSTR, ptrustee : *const TRUSTEE_A, accesspermissions : u32, accessmode : ACCESS_MODE, inheritance : u32) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildImpersonateExplicitAccessWithNameW(pexplicitaccess : *mut EXPLICIT_ACCESS_W, ptrusteename : ::windows_sys::core::PCWSTR, ptrustee : *const TRUSTEE_W, accesspermissions : u32, accessmode : ACCESS_MODE, inheritance : u32) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildImpersonateTrusteeA(ptrustee : *mut TRUSTEE_A, pimpersonatetrustee : *const TRUSTEE_A) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildImpersonateTrusteeW(ptrustee : *mut TRUSTEE_W, pimpersonatetrustee : *const TRUSTEE_W) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildSecurityDescriptorA(powner : *const TRUSTEE_A, pgroup : *const TRUSTEE_A, ccountofaccessentries : u32, plistofaccessentries : *const EXPLICIT_ACCESS_A, ccountofauditentries : u32, plistofauditentries : *const EXPLICIT_ACCESS_A, poldsd : super:: PSECURITY_DESCRIPTOR, psizenewsd : *mut u32, pnewsd : *mut super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildSecurityDescriptorW(powner : *const TRUSTEE_W, pgroup : *const TRUSTEE_W, ccountofaccessentries : u32, plistofaccessentries : *const EXPLICIT_ACCESS_W, ccountofauditentries : u32, plistofauditentries : *const EXPLICIT_ACCESS_W, poldsd : super:: PSECURITY_DESCRIPTOR, psizenewsd : *mut u32, pnewsd : *mut super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("advapi32.dll" "system" fn BuildTrusteeWithNameA(ptrustee : *mut TRUSTEE_A, pname : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildTrusteeWithNameW(ptrustee : *mut TRUSTEE_W, pname : ::windows_sys::core::PCWSTR) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildTrusteeWithObjectsAndNameA(ptrustee : *mut TRUSTEE_A, pobjname : *const OBJECTS_AND_NAME_A, objecttype : SE_OBJECT_TYPE, objecttypename : ::windows_sys::core::PCSTR, inheritedobjecttypename : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("advapi32.dll" "system" fn BuildTrusteeWithObjectsAndNameW(ptrustee : *mut TRUSTEE_W, pobjname : *const OBJECTS_AND_NAME_W, objecttype : SE_OBJECT_TYPE, objecttypename : ::windows_sys::core::PCWSTR, inheritedobjecttypename : ::windows_sys::core::PCWSTR, name : ::windows_sys::core::PCWSTR) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildTrusteeWithObjectsAndSidA(ptrustee : *mut TRUSTEE_A, pobjsid : *const OBJECTS_AND_SID, pobjectguid : *const ::windows_sys::core::GUID, pinheritedobjectguid : *const ::windows_sys::core::GUID, psid : super::super::Foundation:: PSID) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildTrusteeWithObjectsAndSidW(ptrustee : *mut TRUSTEE_W, pobjsid : *const OBJECTS_AND_SID, pobjectguid : *const ::windows_sys::core::GUID, pinheritedobjectguid : *const ::windows_sys::core::GUID, psid : super::super::Foundation:: PSID) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildTrusteeWithSidA(ptrustee : *mut TRUSTEE_A, psid : super::super::Foundation:: PSID) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BuildTrusteeWithSidW(ptrustee : *mut TRUSTEE_W, psid : super::super::Foundation:: PSID) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertSecurityDescriptorToStringSecurityDescriptorA(securitydescriptor : super:: PSECURITY_DESCRIPTOR, requestedstringsdrevision : u32, securityinformation : super:: OBJECT_SECURITY_INFORMATION, stringsecuritydescriptor : *mut ::windows_sys::core::PSTR, stringsecuritydescriptorlen : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertSecurityDescriptorToStringSecurityDescriptorW(securitydescriptor : super:: PSECURITY_DESCRIPTOR, requestedstringsdrevision : u32, securityinformation : super:: OBJECT_SECURITY_INFORMATION, stringsecuritydescriptor : *mut ::windows_sys::core::PWSTR, stringsecuritydescriptorlen : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertSidToStringSidA(sid : super::super::Foundation:: PSID, stringsid : *mut ::windows_sys::core::PSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertSidToStringSidW(sid : super::super::Foundation:: PSID, stringsid : *mut ::windows_sys::core::PWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertStringSecurityDescriptorToSecurityDescriptorA(stringsecuritydescriptor : ::windows_sys::core::PCSTR, stringsdrevision : u32, securitydescriptor : *mut super:: PSECURITY_DESCRIPTOR, securitydescriptorsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertStringSecurityDescriptorToSecurityDescriptorW(stringsecuritydescriptor : ::windows_sys::core::PCWSTR, stringsdrevision : u32, securitydescriptor : *mut super:: PSECURITY_DESCRIPTOR, securitydescriptorsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertStringSidToSidA(stringsid : ::windows_sys::core::PCSTR, sid : *mut super::super::Foundation:: PSID) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ConvertStringSidToSidW(stringsid : ::windows_sys::core::PCWSTR, sid : *mut super::super::Foundation:: PSID) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FreeInheritedFromArray(pinheritarray : *const INHERITED_FROMW, acecnt : u16, pfnarray : *const FN_OBJECT_MGR_FUNCTS) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetAuditedPermissionsFromAclA(pacl : *const super:: ACL, ptrustee : *const TRUSTEE_A, psuccessfulauditedrights : *mut u32, pfailedauditrights : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetAuditedPermissionsFromAclW(pacl : *const super:: ACL, ptrustee : *const TRUSTEE_W, psuccessfulauditedrights : *mut u32, pfailedauditrights : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetEffectiveRightsFromAclA(pacl : *const super:: ACL, ptrustee : *const TRUSTEE_A, paccessrights : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetEffectiveRightsFromAclW(pacl : *const super:: ACL, ptrustee : *const TRUSTEE_W, paccessrights : *mut u32) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetExplicitEntriesFromAclA(pacl : *const super:: ACL, pccountofexplicitentries : *mut u32, plistofexplicitentries : *mut *mut EXPLICIT_ACCESS_A) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetExplicitEntriesFromAclW(pacl : *const super:: ACL, pccountofexplicitentries : *mut u32, plistofexplicitentries : *mut *mut EXPLICIT_ACCESS_W) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetInheritanceSourceA(pobjectname : ::windows_sys::core::PCSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, container : super::super::Foundation:: BOOL, pobjectclassguids : *const *const ::windows_sys::core::GUID, guidcount : u32, pacl : *const super:: ACL, pfnarray : *const FN_OBJECT_MGR_FUNCTS, pgenericmapping : *const super:: GENERIC_MAPPING, pinheritarray : *mut INHERITED_FROMA) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetInheritanceSourceW(pobjectname : ::windows_sys::core::PCWSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, container : super::super::Foundation:: BOOL, pobjectclassguids : *const *const ::windows_sys::core::GUID, guidcount : u32, pacl : *const super:: ACL, pfnarray : *const FN_OBJECT_MGR_FUNCTS, pgenericmapping : *const super:: GENERIC_MAPPING, pinheritarray : *mut INHERITED_FROMW) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("advapi32.dll" "system" fn GetMultipleTrusteeA(ptrustee : *const TRUSTEE_A) -> *mut TRUSTEE_A);
::windows_targets::link!("advapi32.dll" "system" fn GetMultipleTrusteeOperationA(ptrustee : *const TRUSTEE_A) -> MULTIPLE_TRUSTEE_OPERATION);
::windows_targets::link!("advapi32.dll" "system" fn GetMultipleTrusteeOperationW(ptrustee : *const TRUSTEE_W) -> MULTIPLE_TRUSTEE_OPERATION);
::windows_targets::link!("advapi32.dll" "system" fn GetMultipleTrusteeW(ptrustee : *const TRUSTEE_W) -> *mut TRUSTEE_W);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNamedSecurityInfoA(pobjectname : ::windows_sys::core::PCSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, ppsidowner : *mut super::super::Foundation:: PSID, ppsidgroup : *mut super::super::Foundation:: PSID, ppdacl : *mut *mut super:: ACL, ppsacl : *mut *mut super:: ACL, ppsecuritydescriptor : *mut super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNamedSecurityInfoW(pobjectname : ::windows_sys::core::PCWSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, ppsidowner : *mut super::super::Foundation:: PSID, ppsidgroup : *mut super::super::Foundation:: PSID, ppdacl : *mut *mut super:: ACL, ppsacl : *mut *mut super:: ACL, ppsecuritydescriptor : *mut super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetSecurityInfo(handle : super::super::Foundation:: HANDLE, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, ppsidowner : *mut super::super::Foundation:: PSID, ppsidgroup : *mut super::super::Foundation:: PSID, ppdacl : *mut *mut super:: ACL, ppsacl : *mut *mut super:: ACL, ppsecuritydescriptor : *mut super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
::windows_targets::link!("advapi32.dll" "system" fn GetTrusteeFormA(ptrustee : *const TRUSTEE_A) -> TRUSTEE_FORM);
::windows_targets::link!("advapi32.dll" "system" fn GetTrusteeFormW(ptrustee : *const TRUSTEE_W) -> TRUSTEE_FORM);
::windows_targets::link!("advapi32.dll" "system" fn GetTrusteeNameA(ptrustee : *const TRUSTEE_A) -> ::windows_sys::core::PSTR);
::windows_targets::link!("advapi32.dll" "system" fn GetTrusteeNameW(ptrustee : *const TRUSTEE_W) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("advapi32.dll" "system" fn GetTrusteeTypeA(ptrustee : *const TRUSTEE_A) -> TRUSTEE_TYPE);
::windows_targets::link!("advapi32.dll" "system" fn GetTrusteeTypeW(ptrustee : *const TRUSTEE_W) -> TRUSTEE_TYPE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LookupSecurityDescriptorPartsA(ppowner : *mut *mut TRUSTEE_A, ppgroup : *mut *mut TRUSTEE_A, pccountofaccessentries : *mut u32, pplistofaccessentries : *mut *mut EXPLICIT_ACCESS_A, pccountofauditentries : *mut u32, pplistofauditentries : *mut *mut EXPLICIT_ACCESS_A, psd : super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LookupSecurityDescriptorPartsW(ppowner : *mut *mut TRUSTEE_W, ppgroup : *mut *mut TRUSTEE_W, pccountofaccessentries : *mut u32, pplistofaccessentries : *mut *mut EXPLICIT_ACCESS_W, pccountofauditentries : *mut u32, pplistofauditentries : *mut *mut EXPLICIT_ACCESS_W, psd : super:: PSECURITY_DESCRIPTOR) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetEntriesInAclA(ccountofexplicitentries : u32, plistofexplicitentries : *const EXPLICIT_ACCESS_A, oldacl : *const super:: ACL, newacl : *mut *mut super:: ACL) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetEntriesInAclW(ccountofexplicitentries : u32, plistofexplicitentries : *const EXPLICIT_ACCESS_W, oldacl : *const super:: ACL, newacl : *mut *mut super:: ACL) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetNamedSecurityInfoA(pobjectname : ::windows_sys::core::PCSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, psidowner : super::super::Foundation:: PSID, psidgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetNamedSecurityInfoW(pobjectname : ::windows_sys::core::PCWSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, psidowner : super::super::Foundation:: PSID, psidgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetSecurityInfo(handle : super::super::Foundation:: HANDLE, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, psidowner : super::super::Foundation:: PSID, psidgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TreeResetNamedSecurityInfoA(pobjectname : ::windows_sys::core::PCSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, powner : super::super::Foundation:: PSID, pgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL, keepexplicit : super::super::Foundation:: BOOL, fnprogress : FN_PROGRESS, progressinvokesetting : PROG_INVOKE_SETTING, args : *const ::core::ffi::c_void) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TreeResetNamedSecurityInfoW(pobjectname : ::windows_sys::core::PCWSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, powner : super::super::Foundation:: PSID, pgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL, keepexplicit : super::super::Foundation:: BOOL, fnprogress : FN_PROGRESS, progressinvokesetting : PROG_INVOKE_SETTING, args : *const ::core::ffi::c_void) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TreeSetNamedSecurityInfoA(pobjectname : ::windows_sys::core::PCSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, powner : super::super::Foundation:: PSID, pgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL, dwaction : TREE_SEC_INFO, fnprogress : FN_PROGRESS, progressinvokesetting : PROG_INVOKE_SETTING, args : *const ::core::ffi::c_void) -> super::super::Foundation:: WIN32_ERROR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TreeSetNamedSecurityInfoW(pobjectname : ::windows_sys::core::PCWSTR, objecttype : SE_OBJECT_TYPE, securityinfo : super:: OBJECT_SECURITY_INFORMATION, powner : super::super::Foundation:: PSID, pgroup : super::super::Foundation:: PSID, pdacl : *const super:: ACL, psacl : *const super:: ACL, dwaction : TREE_SEC_INFO, fnprogress : FN_PROGRESS, progressinvokesetting : PROG_INVOKE_SETTING, args : *const ::core::ffi::c_void) -> super::super::Foundation:: WIN32_ERROR);
pub type IAzApplication = *mut ::core::ffi::c_void;
pub type IAzApplication2 = *mut ::core::ffi::c_void;
pub type IAzApplication3 = *mut ::core::ffi::c_void;
pub type IAzApplicationGroup = *mut ::core::ffi::c_void;
pub type IAzApplicationGroup2 = *mut ::core::ffi::c_void;
pub type IAzApplicationGroups = *mut ::core::ffi::c_void;
pub type IAzApplications = *mut ::core::ffi::c_void;
pub type IAzAuthorizationStore = *mut ::core::ffi::c_void;
pub type IAzAuthorizationStore2 = *mut ::core::ffi::c_void;
pub type IAzAuthorizationStore3 = *mut ::core::ffi::c_void;
pub type IAzBizRuleContext = *mut ::core::ffi::c_void;
pub type IAzBizRuleInterfaces = *mut ::core::ffi::c_void;
pub type IAzBizRuleParameters = *mut ::core::ffi::c_void;
pub type IAzClientContext = *mut ::core::ffi::c_void;
pub type IAzClientContext2 = *mut ::core::ffi::c_void;
pub type IAzClientContext3 = *mut ::core::ffi::c_void;
pub type IAzNameResolver = *mut ::core::ffi::c_void;
pub type IAzObjectPicker = *mut ::core::ffi::c_void;
pub type IAzOperation = *mut ::core::ffi::c_void;
pub type IAzOperation2 = *mut ::core::ffi::c_void;
pub type IAzOperations = *mut ::core::ffi::c_void;
pub type IAzPrincipalLocator = *mut ::core::ffi::c_void;
pub type IAzRole = *mut ::core::ffi::c_void;
pub type IAzRoleAssignment = *mut ::core::ffi::c_void;
pub type IAzRoleAssignments = *mut ::core::ffi::c_void;
pub type IAzRoleDefinition = *mut ::core::ffi::c_void;
pub type IAzRoleDefinitions = *mut ::core::ffi::c_void;
pub type IAzRoles = *mut ::core::ffi::c_void;
pub type IAzScope = *mut ::core::ffi::c_void;
pub type IAzScope2 = *mut ::core::ffi::c_void;
pub type IAzScopes = *mut ::core::ffi::c_void;
pub type IAzTask = *mut ::core::ffi::c_void;
pub type IAzTask2 = *mut ::core::ffi::c_void;
pub type IAzTasks = *mut ::core::ffi::c_void;
pub const ACCCTRL_DEFAULT_PROVIDER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Windows NT Access Provider");
pub const ACCCTRL_DEFAULT_PROVIDERA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Windows NT Access Provider");
pub const ACCCTRL_DEFAULT_PROVIDERW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Windows NT Access Provider");
pub const ACTRL_ACCESS_ALLOWED: ACTRL_ACCESS_ENTRY_ACCESS_FLAGS = 1u32;
pub const ACTRL_ACCESS_DENIED: ACTRL_ACCESS_ENTRY_ACCESS_FLAGS = 2u32;
pub const ACTRL_ACCESS_NO_OPTIONS: u32 = 0u32;
pub const ACTRL_ACCESS_PROTECTED: u32 = 1u32;
pub const ACTRL_ACCESS_SUPPORTS_OBJECT_ENTRIES: u32 = 1u32;
pub const ACTRL_AUDIT_FAILURE: ACTRL_ACCESS_ENTRY_ACCESS_FLAGS = 8u32;
pub const ACTRL_AUDIT_SUCCESS: ACTRL_ACCESS_ENTRY_ACCESS_FLAGS = 4u32;
pub const ACTRL_CHANGE_ACCESS: u32 = 536870912u32;
pub const ACTRL_CHANGE_OWNER: u32 = 1073741824u32;
pub const ACTRL_DELETE: u32 = 134217728u32;
pub const ACTRL_DIR_CREATE_CHILD: u32 = 4u32;
pub const ACTRL_DIR_CREATE_OBJECT: u32 = 2u32;
pub const ACTRL_DIR_DELETE_CHILD: u32 = 64u32;
pub const ACTRL_DIR_LIST: u32 = 1u32;
pub const ACTRL_DIR_TRAVERSE: u32 = 32u32;
pub const ACTRL_FILE_APPEND: u32 = 4u32;
pub const ACTRL_FILE_CREATE_PIPE: u32 = 512u32;
pub const ACTRL_FILE_EXECUTE: u32 = 32u32;
pub const ACTRL_FILE_READ: u32 = 1u32;
pub const ACTRL_FILE_READ_ATTRIB: u32 = 128u32;
pub const ACTRL_FILE_READ_PROP: u32 = 8u32;
pub const ACTRL_FILE_WRITE: u32 = 2u32;
pub const ACTRL_FILE_WRITE_ATTRIB: u32 = 256u32;
pub const ACTRL_FILE_WRITE_PROP: u32 = 16u32;
pub const ACTRL_KERNEL_ALERT: u32 = 1024u32;
pub const ACTRL_KERNEL_CONTROL: u32 = 512u32;
pub const ACTRL_KERNEL_DIMPERSONATE: u32 = 32768u32;
pub const ACTRL_KERNEL_DUP_HANDLE: u32 = 32u32;
pub const ACTRL_KERNEL_GET_CONTEXT: u32 = 2048u32;
pub const ACTRL_KERNEL_GET_INFO: u32 = 256u32;
pub const ACTRL_KERNEL_IMPERSONATE: u32 = 16384u32;
pub const ACTRL_KERNEL_PROCESS: u32 = 64u32;
pub const ACTRL_KERNEL_SET_CONTEXT: u32 = 4096u32;
pub const ACTRL_KERNEL_SET_INFO: u32 = 128u32;
pub const ACTRL_KERNEL_TERMINATE: u32 = 1u32;
pub const ACTRL_KERNEL_THREAD: u32 = 2u32;
pub const ACTRL_KERNEL_TOKEN: u32 = 8192u32;
pub const ACTRL_KERNEL_VM: u32 = 4u32;
pub const ACTRL_KERNEL_VM_READ: u32 = 8u32;
pub const ACTRL_KERNEL_VM_WRITE: u32 = 16u32;
pub const ACTRL_PERM_1: u32 = 1u32;
pub const ACTRL_PERM_10: u32 = 512u32;
pub const ACTRL_PERM_11: u32 = 1024u32;
pub const ACTRL_PERM_12: u32 = 2048u32;
pub const ACTRL_PERM_13: u32 = 4096u32;
pub const ACTRL_PERM_14: u32 = 8192u32;
pub const ACTRL_PERM_15: u32 = 16384u32;
pub const ACTRL_PERM_16: u32 = 32768u32;
pub const ACTRL_PERM_17: u32 = 65536u32;
pub const ACTRL_PERM_18: u32 = 131072u32;
pub const ACTRL_PERM_19: u32 = 262144u32;
pub const ACTRL_PERM_2: u32 = 2u32;
pub const ACTRL_PERM_20: u32 = 524288u32;
pub const ACTRL_PERM_3: u32 = 4u32;
pub const ACTRL_PERM_4: u32 = 8u32;
pub const ACTRL_PERM_5: u32 = 16u32;
pub const ACTRL_PERM_6: u32 = 32u32;
pub const ACTRL_PERM_7: u32 = 64u32;
pub const ACTRL_PERM_8: u32 = 128u32;
pub const ACTRL_PERM_9: u32 = 256u32;
pub const ACTRL_PRINT_JADMIN: u32 = 16u32;
pub const ACTRL_PRINT_PADMIN: u32 = 4u32;
pub const ACTRL_PRINT_PUSE: u32 = 8u32;
pub const ACTRL_PRINT_SADMIN: u32 = 1u32;
pub const ACTRL_PRINT_SLIST: u32 = 2u32;
pub const ACTRL_READ_CONTROL: u32 = 268435456u32;
pub const ACTRL_REG_CREATE_CHILD: u32 = 4u32;
pub const ACTRL_REG_LINK: u32 = 32u32;
pub const ACTRL_REG_LIST: u32 = 8u32;
pub const ACTRL_REG_NOTIFY: u32 = 16u32;
pub const ACTRL_REG_QUERY: u32 = 1u32;
pub const ACTRL_REG_SET: u32 = 2u32;
pub const ACTRL_RESERVED: u32 = 0u32;
pub const ACTRL_STD_RIGHTS_ALL: u32 = 4160749568u32;
pub const ACTRL_SVC_GET_INFO: u32 = 1u32;
pub const ACTRL_SVC_INTERROGATE: u32 = 128u32;
pub const ACTRL_SVC_LIST: u32 = 8u32;
pub const ACTRL_SVC_PAUSE: u32 = 64u32;
pub const ACTRL_SVC_SET_INFO: u32 = 2u32;
pub const ACTRL_SVC_START: u32 = 16u32;
pub const ACTRL_SVC_STATUS: u32 = 4u32;
pub const ACTRL_SVC_STOP: u32 = 32u32;
pub const ACTRL_SVC_UCONTROL: u32 = 256u32;
pub const ACTRL_SYNCHRONIZE: u32 = 2147483648u32;
pub const ACTRL_SYSTEM_ACCESS: u32 = 67108864u32;
pub const ACTRL_WIN_CLIPBRD: u32 = 1u32;
pub const ACTRL_WIN_CREATE: u32 = 4u32;
pub const ACTRL_WIN_EXIT: u32 = 256u32;
pub const ACTRL_WIN_GLOBAL_ATOMS: u32 = 2u32;
pub const ACTRL_WIN_LIST: u32 = 16u32;
pub const ACTRL_WIN_LIST_DESK: u32 = 8u32;
pub const ACTRL_WIN_READ_ATTRIBS: u32 = 32u32;
pub const ACTRL_WIN_SCREEN: u32 = 128u32;
pub const ACTRL_WIN_WRITE_ATTRIBS: u32 = 64u32;
pub const APF_AuditFailure: u32 = 0u32;
pub const APF_AuditSuccess: u32 = 1u32;
pub const APF_ValidFlags: u32 = 1u32;
pub const APT_Guid: AUDIT_PARAM_TYPE = 9i32;
pub const APT_Int64: AUDIT_PARAM_TYPE = 11i32;
pub const APT_IpAddress: AUDIT_PARAM_TYPE = 12i32;
pub const APT_LogonId: AUDIT_PARAM_TYPE = 6i32;
pub const APT_LogonIdWithSid: AUDIT_PARAM_TYPE = 13i32;
pub const APT_Luid: AUDIT_PARAM_TYPE = 8i32;
pub const APT_None: AUDIT_PARAM_TYPE = 1i32;
pub const APT_ObjectTypeList: AUDIT_PARAM_TYPE = 7i32;
pub const APT_Pointer: AUDIT_PARAM_TYPE = 4i32;
pub const APT_Sid: AUDIT_PARAM_TYPE = 5i32;
pub const APT_String: AUDIT_PARAM_TYPE = 2i32;
pub const APT_Time: AUDIT_PARAM_TYPE = 10i32;
pub const APT_Ulong: AUDIT_PARAM_TYPE = 3i32;
pub const AP_ParamTypeBits: u32 = 8u32;
pub const AP_ParamTypeMask: i32 = 255i32;
pub const AUDIT_TYPE_LEGACY: u32 = 1u32;
pub const AUDIT_TYPE_WMI: u32 = 2u32;
pub const AUTHZP_WPD_EVENT: u32 = 16u32;
pub const AUTHZ_ACCESS_CHECK_NO_DEEP_COPY_SD: AUTHZ_ACCESS_CHECK_FLAGS = 1u32;
pub const AUTHZ_ALLOW_MULTIPLE_SOURCE_INSTANCES: u32 = 1u32;
pub const AUTHZ_AUDIT_INSTANCE_INFORMATION: u32 = 2u32;
pub const AUTHZ_COMPUTE_PRIVILEGES: u32 = 8u32;
pub const AUTHZ_FLAG_ALLOW_MULTIPLE_SOURCE_INSTANCES: u32 = 1u32;
pub const AUTHZ_GENERATE_FAILURE_AUDIT: AUTHZ_GENERATE_RESULTS = 2u32;
pub const AUTHZ_GENERATE_SUCCESS_AUDIT: AUTHZ_GENERATE_RESULTS = 1u32;
pub const AUTHZ_INIT_INFO_VERSION_V1: u32 = 1u32;
pub const AUTHZ_MIGRATED_LEGACY_PUBLISHER: u32 = 2u32;
pub const AUTHZ_NO_ALLOC_STRINGS: AUTHZ_INITIALIZE_OBJECT_ACCESS_AUDIT_EVENT_FLAGS = 4u32;
pub const AUTHZ_NO_FAILURE_AUDIT: AUTHZ_INITIALIZE_OBJECT_ACCESS_AUDIT_EVENT_FLAGS = 2u32;
pub const AUTHZ_NO_SUCCESS_AUDIT: AUTHZ_INITIALIZE_OBJECT_ACCESS_AUDIT_EVENT_FLAGS = 1u32;
pub const AUTHZ_REQUIRE_S4U_LOGON: u32 = 4u32;
pub const AUTHZ_RM_FLAG_INITIALIZE_UNDER_IMPERSONATION: AUTHZ_RESOURCE_MANAGER_FLAGS = 2u32;
pub const AUTHZ_RM_FLAG_NO_AUDIT: AUTHZ_RESOURCE_MANAGER_FLAGS = 1u32;
pub const AUTHZ_RM_FLAG_NO_CENTRAL_ACCESS_POLICIES: AUTHZ_RESOURCE_MANAGER_FLAGS = 4u32;
pub const AUTHZ_RPC_INIT_INFO_CLIENT_VERSION_V1: u32 = 1u32;
pub const AUTHZ_SECURITY_ATTRIBUTES_INFORMATION_VERSION: u32 = 1u32;
pub const AUTHZ_SECURITY_ATTRIBUTES_INFORMATION_VERSION_V1: u32 = 1u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_NON_INHERITABLE: AUTHZ_SECURITY_ATTRIBUTE_FLAGS = 1u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_OPERATION_ADD: AUTHZ_SECURITY_ATTRIBUTE_OPERATION = 2i32;
pub const AUTHZ_SECURITY_ATTRIBUTE_OPERATION_DELETE: AUTHZ_SECURITY_ATTRIBUTE_OPERATION = 3i32;
pub const AUTHZ_SECURITY_ATTRIBUTE_OPERATION_NONE: AUTHZ_SECURITY_ATTRIBUTE_OPERATION = 0i32;
pub const AUTHZ_SECURITY_ATTRIBUTE_OPERATION_REPLACE: AUTHZ_SECURITY_ATTRIBUTE_OPERATION = 4i32;
pub const AUTHZ_SECURITY_ATTRIBUTE_OPERATION_REPLACE_ALL: AUTHZ_SECURITY_ATTRIBUTE_OPERATION = 1i32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_BOOLEAN: u32 = 6u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_FQBN: u32 = 4u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_INT64: u32 = 1u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_INVALID: u32 = 0u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_OCTET_STRING: u32 = 16u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_SID: u32 = 5u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_STRING: u32 = 3u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_TYPE_UINT64: u32 = 2u32;
pub const AUTHZ_SECURITY_ATTRIBUTE_VALUE_CASE_SENSITIVE: AUTHZ_SECURITY_ATTRIBUTE_FLAGS = 2u32;
pub const AUTHZ_SID_OPERATION_ADD: AUTHZ_SID_OPERATION = 2i32;
pub const AUTHZ_SID_OPERATION_DELETE: AUTHZ_SID_OPERATION = 3i32;
pub const AUTHZ_SID_OPERATION_NONE: AUTHZ_SID_OPERATION = 0i32;
pub const AUTHZ_SID_OPERATION_REPLACE: AUTHZ_SID_OPERATION = 4i32;
pub const AUTHZ_SID_OPERATION_REPLACE_ALL: AUTHZ_SID_OPERATION = 1i32;
pub const AUTHZ_SKIP_TOKEN_GROUPS: u32 = 2u32;
pub const AUTHZ_WPD_CATEGORY_FLAG: u32 = 16u32;
pub const AZ_AZSTORE_DEFAULT_DOMAIN_TIMEOUT: AZ_PROP_CONSTANTS = 15000i32;
pub const AZ_AZSTORE_DEFAULT_MAX_SCRIPT_ENGINES: AZ_PROP_CONSTANTS = 120i32;
pub const AZ_AZSTORE_DEFAULT_SCRIPT_ENGINE_TIMEOUT: AZ_PROP_CONSTANTS = 45000i32;
pub const AZ_AZSTORE_FLAG_AUDIT_IS_CRITICAL: AZ_PROP_CONSTANTS = 8i32;
pub const AZ_AZSTORE_FLAG_BATCH_UPDATE: AZ_PROP_CONSTANTS = 4i32;
pub const AZ_AZSTORE_FLAG_CREATE: AZ_PROP_CONSTANTS = 1i32;
pub const AZ_AZSTORE_FLAG_MANAGE_ONLY_PASSIVE_SUBMIT: AZ_PROP_CONSTANTS = 32768i32;
pub const AZ_AZSTORE_FLAG_MANAGE_STORE_ONLY: AZ_PROP_CONSTANTS = 2i32;
pub const AZ_AZSTORE_FORCE_APPLICATION_CLOSE: AZ_PROP_CONSTANTS = 16i32;
pub const AZ_AZSTORE_MIN_DOMAIN_TIMEOUT: AZ_PROP_CONSTANTS = 500i32;
pub const AZ_AZSTORE_MIN_SCRIPT_ENGINE_TIMEOUT: AZ_PROP_CONSTANTS = 5000i32;
pub const AZ_AZSTORE_NT6_FUNCTION_LEVEL: AZ_PROP_CONSTANTS = 32i32;
pub const AZ_CLIENT_CONTEXT_GET_GROUPS_STORE_LEVEL_ONLY: AZ_PROP_CONSTANTS = 2i32;
pub const AZ_CLIENT_CONTEXT_GET_GROUP_RECURSIVE: AZ_PROP_CONSTANTS = 2i32;
pub const AZ_CLIENT_CONTEXT_SKIP_GROUP: AZ_PROP_CONSTANTS = 1i32;
pub const AZ_CLIENT_CONTEXT_SKIP_LDAP_QUERY: AZ_PROP_CONSTANTS = 1i32;
pub const AZ_GROUPTYPE_BASIC: AZ_PROP_CONSTANTS = 2i32;
pub const AZ_GROUPTYPE_BIZRULE: AZ_PROP_CONSTANTS = 3i32;
pub const AZ_GROUPTYPE_LDAP_QUERY: AZ_PROP_CONSTANTS = 1i32;
pub const AZ_MAX_APPLICATION_DATA_LENGTH: AZ_PROP_CONSTANTS = 4096i32;
pub const AZ_MAX_APPLICATION_NAME_LENGTH: AZ_PROP_CONSTANTS = 512i32;
pub const AZ_MAX_APPLICATION_VERSION_LENGTH: AZ_PROP_CONSTANTS = 512i32;
pub const AZ_MAX_BIZRULE_STRING: AZ_PROP_CONSTANTS = 65536i32;
pub const AZ_MAX_DESCRIPTION_LENGTH: AZ_PROP_CONSTANTS = 1024i32;
pub const AZ_MAX_GROUP_BIZRULE_IMPORTED_PATH_LENGTH: AZ_PROP_CONSTANTS = 512i32;
pub const AZ_MAX_GROUP_BIZRULE_LANGUAGE_LENGTH: AZ_PROP_CONSTANTS = 64i32;
pub const AZ_MAX_GROUP_BIZRULE_LENGTH: AZ_PROP_CONSTANTS = 65536i32;
pub const AZ_MAX_GROUP_LDAP_QUERY_LENGTH: AZ_PROP_CONSTANTS = 4096i32;
pub const AZ_MAX_GROUP_NAME_LENGTH: AZ_PROP_CONSTANTS = 64i32;
pub const AZ_MAX_NAME_LENGTH: AZ_PROP_CONSTANTS = 65536i32;
pub const AZ_MAX_OPERATION_NAME_LENGTH: AZ_PROP_CONSTANTS = 64i32;
pub const AZ_MAX_POLICY_URL_LENGTH: AZ_PROP_CONSTANTS = 65536i32;
pub const AZ_MAX_ROLE_NAME_LENGTH: AZ_PROP_CONSTANTS = 64i32;
pub const AZ_MAX_SCOPE_NAME_LENGTH: AZ_PROP_CONSTANTS = 65536i32;
pub const AZ_MAX_TASK_BIZRULE_IMPORTED_PATH_LENGTH: AZ_PROP_CONSTANTS = 512i32;
pub const AZ_MAX_TASK_BIZRULE_LANGUAGE_LENGTH: AZ_PROP_CONSTANTS = 64i32;
pub const AZ_MAX_TASK_BIZRULE_LENGTH: AZ_PROP_CONSTANTS = 65536i32;
pub const AZ_MAX_TASK_NAME_LENGTH: AZ_PROP_CONSTANTS = 64i32;
pub const AZ_PROP_APPLICATION_AUTHZ_INTERFACE_CLSID: AZ_PROP_CONSTANTS = 800i32;
pub const AZ_PROP_APPLICATION_BIZRULE_ENABLED: AZ_PROP_CONSTANTS = 803i32;
pub const AZ_PROP_APPLICATION_DATA: AZ_PROP_CONSTANTS = 4i32;
pub const AZ_PROP_APPLICATION_NAME: AZ_PROP_CONSTANTS = 802i32;
pub const AZ_PROP_APPLICATION_VERSION: AZ_PROP_CONSTANTS = 801i32;
pub const AZ_PROP_APPLY_STORE_SACL: AZ_PROP_CONSTANTS = 900i32;
pub const AZ_PROP_AZSTORE_DOMAIN_TIMEOUT: AZ_PROP_CONSTANTS = 100i32;
pub const AZ_PROP_AZSTORE_MAJOR_VERSION: AZ_PROP_CONSTANTS = 103i32;
pub const AZ_PROP_AZSTORE_MAX_SCRIPT_ENGINES: AZ_PROP_CONSTANTS = 102i32;
pub const AZ_PROP_AZSTORE_MINOR_VERSION: AZ_PROP_CONSTANTS = 104i32;
pub const AZ_PROP_AZSTORE_SCRIPT_ENGINE_TIMEOUT: AZ_PROP_CONSTANTS = 101i32;
pub const AZ_PROP_AZSTORE_TARGET_MACHINE: AZ_PROP_CONSTANTS = 105i32;
pub const AZ_PROP_AZTORE_IS_ADAM_INSTANCE: AZ_PROP_CONSTANTS = 106i32;
pub const AZ_PROP_CHILD_CREATE: AZ_PROP_CONSTANTS = 5i32;
pub const AZ_PROP_CLIENT_CONTEXT_LDAP_QUERY_DN: AZ_PROP_CONSTANTS = 709i32;
pub const AZ_PROP_CLIENT_CONTEXT_ROLE_FOR_ACCESS_CHECK: AZ_PROP_CONSTANTS = 708i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_CANONICAL: AZ_PROP_CONSTANTS = 704i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_DISPLAY: AZ_PROP_CONSTANTS = 702i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_DN: AZ_PROP_CONSTANTS = 700i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_DNS_SAM_COMPAT: AZ_PROP_CONSTANTS = 707i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_GUID: AZ_PROP_CONSTANTS = 703i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_SAM_COMPAT: AZ_PROP_CONSTANTS = 701i32;
pub const AZ_PROP_CLIENT_CONTEXT_USER_UPN: AZ_PROP_CONSTANTS = 705i32;
pub const AZ_PROP_DELEGATED_POLICY_USERS: AZ_PROP_CONSTANTS = 904i32;
pub const AZ_PROP_DELEGATED_POLICY_USERS_NAME: AZ_PROP_CONSTANTS = 907i32;
pub const AZ_PROP_DESCRIPTION: AZ_PROP_CONSTANTS = 2i32;
pub const AZ_PROP_GENERATE_AUDITS: AZ_PROP_CONSTANTS = 901i32;
pub const AZ_PROP_GROUP_APP_MEMBERS: AZ_PROP_CONSTANTS = 401i32;
pub const AZ_PROP_GROUP_APP_NON_MEMBERS: AZ_PROP_CONSTANTS = 402i32;
pub const AZ_PROP_GROUP_BIZRULE: AZ_PROP_CONSTANTS = 408i32;
pub const AZ_PROP_GROUP_BIZRULE_IMPORTED_PATH: AZ_PROP_CONSTANTS = 410i32;
pub const AZ_PROP_GROUP_BIZRULE_LANGUAGE: AZ_PROP_CONSTANTS = 409i32;
pub const AZ_PROP_GROUP_LDAP_QUERY: AZ_PROP_CONSTANTS = 403i32;
pub const AZ_PROP_GROUP_MEMBERS: AZ_PROP_CONSTANTS = 404i32;
pub const AZ_PROP_GROUP_MEMBERS_NAME: AZ_PROP_CONSTANTS = 406i32;
pub const AZ_PROP_GROUP_NON_MEMBERS: AZ_PROP_CONSTANTS = 405i32;
pub const AZ_PROP_GROUP_NON_MEMBERS_NAME: AZ_PROP_CONSTANTS = 407i32;
pub const AZ_PROP_GROUP_TYPE: AZ_PROP_CONSTANTS = 400i32;
pub const AZ_PROP_NAME: AZ_PROP_CONSTANTS = 1i32;
pub const AZ_PROP_OPERATION_ID: AZ_PROP_CONSTANTS = 200i32;
pub const AZ_PROP_POLICY_ADMINS: AZ_PROP_CONSTANTS = 902i32;
pub const AZ_PROP_POLICY_ADMINS_NAME: AZ_PROP_CONSTANTS = 905i32;
pub const AZ_PROP_POLICY_READERS: AZ_PROP_CONSTANTS = 903i32;
pub const AZ_PROP_POLICY_READERS_NAME: AZ_PROP_CONSTANTS = 906i32;
pub const AZ_PROP_ROLE_APP_MEMBERS: AZ_PROP_CONSTANTS = 500i32;
pub const AZ_PROP_ROLE_MEMBERS: AZ_PROP_CONSTANTS = 501i32;
pub const AZ_PROP_ROLE_MEMBERS_NAME: AZ_PROP_CONSTANTS = 505i32;
pub const AZ_PROP_ROLE_OPERATIONS: AZ_PROP_CONSTANTS = 502i32;
pub const AZ_PROP_ROLE_TASKS: AZ_PROP_CONSTANTS = 504i32;
pub const AZ_PROP_SCOPE_BIZRULES_WRITABLE: AZ_PROP_CONSTANTS = 600i32;
pub const AZ_PROP_SCOPE_CAN_BE_DELEGATED: AZ_PROP_CONSTANTS = 601i32;
pub const AZ_PROP_TASK_BIZRULE: AZ_PROP_CONSTANTS = 301i32;
pub const AZ_PROP_TASK_BIZRULE_IMPORTED_PATH: AZ_PROP_CONSTANTS = 304i32;
pub const AZ_PROP_TASK_BIZRULE_LANGUAGE: AZ_PROP_CONSTANTS = 302i32;
pub const AZ_PROP_TASK_IS_ROLE_DEFINITION: AZ_PROP_CONSTANTS = 305i32;
pub const AZ_PROP_TASK_OPERATIONS: AZ_PROP_CONSTANTS = 300i32;
pub const AZ_PROP_TASK_TASKS: AZ_PROP_CONSTANTS = 303i32;
pub const AZ_PROP_WRITABLE: AZ_PROP_CONSTANTS = 3i32;
pub const AZ_SUBMIT_FLAG_ABORT: AZ_PROP_CONSTANTS = 1i32;
pub const AZ_SUBMIT_FLAG_FLUSH: AZ_PROP_CONSTANTS = 2i32;
pub const AuthzAuditEventInfoAdditionalInfo: AUTHZ_AUDIT_EVENT_INFORMATION_CLASS = 5i32;
pub const AuthzAuditEventInfoFlags: AUTHZ_AUDIT_EVENT_INFORMATION_CLASS = 1i32;
pub const AuthzAuditEventInfoObjectName: AUTHZ_AUDIT_EVENT_INFORMATION_CLASS = 4i32;
pub const AuthzAuditEventInfoObjectType: AUTHZ_AUDIT_EVENT_INFORMATION_CLASS = 3i32;
pub const AuthzAuditEventInfoOperationType: AUTHZ_AUDIT_EVENT_INFORMATION_CLASS = 2i32;
pub const AuthzContextInfoAll: AUTHZ_CONTEXT_INFORMATION_CLASS = 9i32;
pub const AuthzContextInfoAppContainerSid: AUTHZ_CONTEXT_INFORMATION_CLASS = 15i32;
pub const AuthzContextInfoAuthenticationId: AUTHZ_CONTEXT_INFORMATION_CLASS = 10i32;
pub const AuthzContextInfoCapabilitySids: AUTHZ_CONTEXT_INFORMATION_CLASS = 16i32;
pub const AuthzContextInfoDeviceClaims: AUTHZ_CONTEXT_INFORMATION_CLASS = 14i32;
pub const AuthzContextInfoDeviceSids: AUTHZ_CONTEXT_INFORMATION_CLASS = 12i32;
pub const AuthzContextInfoExpirationTime: AUTHZ_CONTEXT_INFORMATION_CLASS = 5i32;
pub const AuthzContextInfoGroupsSids: AUTHZ_CONTEXT_INFORMATION_CLASS = 2i32;
pub const AuthzContextInfoIdentifier: AUTHZ_CONTEXT_INFORMATION_CLASS = 7i32;
pub const AuthzContextInfoPrivileges: AUTHZ_CONTEXT_INFORMATION_CLASS = 4i32;
pub const AuthzContextInfoRestrictedSids: AUTHZ_CONTEXT_INFORMATION_CLASS = 3i32;
pub const AuthzContextInfoSecurityAttributes: AUTHZ_CONTEXT_INFORMATION_CLASS = 11i32;
pub const AuthzContextInfoServerContext: AUTHZ_CONTEXT_INFORMATION_CLASS = 6i32;
pub const AuthzContextInfoSource: AUTHZ_CONTEXT_INFORMATION_CLASS = 8i32;
pub const AuthzContextInfoUserClaims: AUTHZ_CONTEXT_INFORMATION_CLASS = 13i32;
pub const AuthzContextInfoUserSid: AUTHZ_CONTEXT_INFORMATION_CLASS = 1i32;
pub const AzAuthorizationStore: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb2bcff59_a757_4b0b_a1bc_ea69981da69e);
pub const AzBizRuleContext: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c2dc96f_8d51_434b_b33c_379bccae77c3);
pub const AzPrincipalLocator: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x483afb5d_70df_4e16_abdc_a1de4d015a3e);
pub const DENY_ACCESS: ACCESS_MODE = 3i32;
pub const GRANT_ACCESS: ACCESS_MODE = 1i32;
pub const INHERITED_ACCESS_ENTRY: u32 = 16u32;
pub const INHERITED_GRANDPARENT: u32 = 536870912u32;
pub const INHERITED_PARENT: u32 = 268435456u32;
pub const NOT_USED_ACCESS: ACCESS_MODE = 0i32;
pub const NO_MULTIPLE_TRUSTEE: MULTIPLE_TRUSTEE_OPERATION = 0i32;
pub const OLESCRIPT_E_SYNTAX: ::windows_sys::core::HRESULT = -2147352319i32;
pub const ProgressCancelOperation: PROG_INVOKE_SETTING = 4i32;
pub const ProgressInvokeEveryObject: PROG_INVOKE_SETTING = 2i32;
pub const ProgressInvokeNever: PROG_INVOKE_SETTING = 1i32;
pub const ProgressInvokeOnError: PROG_INVOKE_SETTING = 3i32;
pub const ProgressInvokePrePostError: PROG_INVOKE_SETTING = 6i32;
pub const ProgressRetryOperation: PROG_INVOKE_SETTING = 5i32;
pub const REVOKE_ACCESS: ACCESS_MODE = 4i32;
pub const SDDL_ACCESS_ALLOWED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("A");
pub const SDDL_ACCESS_CONTROL_ASSISTANCE_OPS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AA");
pub const SDDL_ACCESS_DENIED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("D");
pub const SDDL_ACCESS_FILTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FL");
pub const SDDL_ACCOUNT_OPERATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AO");
pub const SDDL_ACE_BEGIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("(");
pub const SDDL_ACE_COND_ATTRIBUTE_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("@");
pub const SDDL_ACE_COND_BEGIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("(");
pub const SDDL_ACE_COND_BLOB_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("#");
pub const SDDL_ACE_COND_DEVICE_ATTRIBUTE_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("@DEVICE.");
pub const SDDL_ACE_COND_END: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!(")");
pub const SDDL_ACE_COND_RESOURCE_ATTRIBUTE_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("@RESOURCE.");
pub const SDDL_ACE_COND_SID_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SID");
pub const SDDL_ACE_COND_TOKEN_ATTRIBUTE_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("@TOKEN.");
pub const SDDL_ACE_COND_USER_ATTRIBUTE_PREFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("@USER.");
pub const SDDL_ACE_END: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!(")");
pub const SDDL_ALARM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AL");
pub const SDDL_ALIAS_PREW2KCOMPACC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RU");
pub const SDDL_ALIAS_SIZE: u32 = 2u32;
pub const SDDL_ALL_APP_PACKAGES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AC");
pub const SDDL_ANONYMOUS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AN");
pub const SDDL_AUDIT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AU");
pub const SDDL_AUDIT_FAILURE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FA");
pub const SDDL_AUDIT_SUCCESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SA");
pub const SDDL_AUTHENTICATED_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AU");
pub const SDDL_AUTHORITY_ASSERTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AS");
pub const SDDL_AUTO_INHERITED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AI");
pub const SDDL_AUTO_INHERIT_REQ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AR");
pub const SDDL_BACKUP_OPERATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BO");
pub const SDDL_BLOB: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TX");
pub const SDDL_BOOLEAN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TB");
pub const SDDL_BUILTIN_ADMINISTRATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BA");
pub const SDDL_BUILTIN_GUESTS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BG");
pub const SDDL_BUILTIN_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BU");
pub const SDDL_CALLBACK_ACCESS_ALLOWED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("XA");
pub const SDDL_CALLBACK_ACCESS_DENIED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("XD");
pub const SDDL_CALLBACK_AUDIT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("XU");
pub const SDDL_CALLBACK_OBJECT_ACCESS_ALLOWED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ZA");
pub const SDDL_CERTSVC_DCOM_ACCESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CD");
pub const SDDL_CERT_SERV_ADMINISTRATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CA");
pub const SDDL_CLONEABLE_CONTROLLERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CN");
pub const SDDL_CONTAINER_INHERIT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CI");
pub const SDDL_CONTROL_ACCESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CR");
pub const SDDL_CREATE_CHILD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CC");
pub const SDDL_CREATOR_GROUP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CG");
pub const SDDL_CREATOR_OWNER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CO");
pub const SDDL_CRITICAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CR");
pub const SDDL_CRYPTO_OPERATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CY");
pub const SDDL_DACL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("D");
pub const SDDL_DELETE_CHILD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DC");
pub const SDDL_DELETE_TREE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DT");
pub const SDDL_DELIMINATOR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!(":");
pub const SDDL_DOMAIN_ADMINISTRATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DA");
pub const SDDL_DOMAIN_COMPUTERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DC");
pub const SDDL_DOMAIN_DOMAIN_CONTROLLERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DD");
pub const SDDL_DOMAIN_GUESTS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DG");
pub const SDDL_DOMAIN_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DU");
pub const SDDL_ENTERPRISE_ADMINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EA");
pub const SDDL_ENTERPRISE_DOMAIN_CONTROLLERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ED");
pub const SDDL_ENTERPRISE_KEY_ADMINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EK");
pub const SDDL_ENTERPRISE_RO_DCs: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RO");
pub const SDDL_EVENT_LOG_READERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ER");
pub const SDDL_EVERYONE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WD");
pub const SDDL_FILE_ALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FA");
pub const SDDL_FILE_EXECUTE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FX");
pub const SDDL_FILE_READ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FR");
pub const SDDL_FILE_WRITE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FW");
pub const SDDL_GENERIC_ALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GA");
pub const SDDL_GENERIC_EXECUTE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GX");
pub const SDDL_GENERIC_READ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GR");
pub const SDDL_GENERIC_WRITE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GW");
pub const SDDL_GROUP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("G");
pub const SDDL_GROUP_POLICY_ADMINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PA");
pub const SDDL_HYPER_V_ADMINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HA");
pub const SDDL_IIS_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IS");
pub const SDDL_INHERITED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ID");
pub const SDDL_INHERIT_ONLY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IO");
pub const SDDL_INT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TI");
pub const SDDL_INTERACTIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IU");
pub const SDDL_KEY_ADMINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("KA");
pub const SDDL_KEY_ALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("KA");
pub const SDDL_KEY_EXECUTE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("KX");
pub const SDDL_KEY_READ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("KR");
pub const SDDL_KEY_WRITE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("KW");
pub const SDDL_LIST_CHILDREN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LC");
pub const SDDL_LIST_OBJECT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LO");
pub const SDDL_LOCAL_ADMIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LA");
pub const SDDL_LOCAL_GUEST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LG");
pub const SDDL_LOCAL_SERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LS");
pub const SDDL_LOCAL_SYSTEM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SY");
pub const SDDL_MANDATORY_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ML");
pub const SDDL_ML_HIGH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HI");
pub const SDDL_ML_LOW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LW");
pub const SDDL_ML_MEDIUM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ME");
pub const SDDL_ML_MEDIUM_PLUS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MP");
pub const SDDL_ML_SYSTEM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SI");
pub const SDDL_NETWORK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NU");
pub const SDDL_NETWORK_CONFIGURATION_OPS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NO");
pub const SDDL_NETWORK_SERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NS");
pub const SDDL_NO_EXECUTE_UP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NX");
pub const SDDL_NO_PROPAGATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NP");
pub const SDDL_NO_READ_UP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NR");
pub const SDDL_NO_WRITE_UP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NW");
pub const SDDL_NULL_ACL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NO_ACCESS_CONTROL");
pub const SDDL_OBJECT_ACCESS_ALLOWED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OA");
pub const SDDL_OBJECT_ACCESS_DENIED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OD");
pub const SDDL_OBJECT_ALARM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OL");
pub const SDDL_OBJECT_AUDIT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OU");
pub const SDDL_OBJECT_INHERIT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OI");
pub const SDDL_OWNER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("O");
pub const SDDL_OWNER_RIGHTS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OW");
pub const SDDL_PERFLOG_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LU");
pub const SDDL_PERFMON_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MU");
pub const SDDL_PERSONAL_SELF: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PS");
pub const SDDL_POWER_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PU");
pub const SDDL_PRINTER_OPERATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PO");
pub const SDDL_PROCESS_TRUST_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TL");
pub const SDDL_PROTECTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("P");
pub const SDDL_PROTECTED_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AP");
pub const SDDL_RAS_SERVERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RS");
pub const SDDL_RDS_ENDPOINT_SERVERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ES");
pub const SDDL_RDS_MANAGEMENT_SERVERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MS");
pub const SDDL_RDS_REMOTE_ACCESS_SERVERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RA");
pub const SDDL_READ_CONTROL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RC");
pub const SDDL_READ_PROPERTY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RP");
pub const SDDL_REMOTE_DESKTOP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RD");
pub const SDDL_REMOTE_MANAGEMENT_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RM");
pub const SDDL_REPLICATOR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RE");
pub const SDDL_RESOURCE_ATTRIBUTE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RA");
pub const SDDL_RESTRICTED_CODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RC");
pub const SDDL_REVISION: u32 = 1u32;
pub const SDDL_REVISION_1: u32 = 1u32;
pub const SDDL_SACL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S");
pub const SDDL_SCHEMA_ADMINISTRATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SA");
pub const SDDL_SCOPED_POLICY_ID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SP");
pub const SDDL_SELF_WRITE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SW");
pub const SDDL_SEPERATOR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!(";");
pub const SDDL_SERVER_OPERATORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SO");
pub const SDDL_SERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SU");
pub const SDDL_SERVICE_ASSERTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SS");
pub const SDDL_SID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TD");
pub const SDDL_SPACE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!(" ");
pub const SDDL_STANDARD_DELETE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SD");
pub const SDDL_TRUST_PROTECTED_FILTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TP");
pub const SDDL_UINT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TU");
pub const SDDL_USER_MODE_DRIVERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UD");
pub const SDDL_WRITE_DAC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WD");
pub const SDDL_WRITE_OWNER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WO");
pub const SDDL_WRITE_PROPERTY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WP");
pub const SDDL_WRITE_RESTRICTED_CODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WR");
pub const SDDL_WSTRING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TS");
pub const SET_ACCESS: ACCESS_MODE = 2i32;
pub const SET_AUDIT_FAILURE: ACCESS_MODE = 6i32;
pub const SET_AUDIT_SUCCESS: ACCESS_MODE = 5i32;
pub const SE_DS_OBJECT: SE_OBJECT_TYPE = 8i32;
pub const SE_DS_OBJECT_ALL: SE_OBJECT_TYPE = 9i32;
pub const SE_FILE_OBJECT: SE_OBJECT_TYPE = 1i32;
pub const SE_KERNEL_OBJECT: SE_OBJECT_TYPE = 6i32;
pub const SE_LMSHARE: SE_OBJECT_TYPE = 5i32;
pub const SE_PRINTER: SE_OBJECT_TYPE = 3i32;
pub const SE_PROVIDER_DEFINED_OBJECT: SE_OBJECT_TYPE = 10i32;
pub const SE_REGISTRY_KEY: SE_OBJECT_TYPE = 4i32;
pub const SE_REGISTRY_WOW64_32KEY: SE_OBJECT_TYPE = 12i32;
pub const SE_REGISTRY_WOW64_64KEY: SE_OBJECT_TYPE = 13i32;
pub const SE_SERVICE: SE_OBJECT_TYPE = 2i32;
pub const SE_UNKNOWN_OBJECT_TYPE: SE_OBJECT_TYPE = 0i32;
pub const SE_WINDOW_OBJECT: SE_OBJECT_TYPE = 7i32;
pub const SE_WMIGUID_OBJECT: SE_OBJECT_TYPE = 11i32;
pub const TREE_SEC_INFO_RESET: TREE_SEC_INFO = 2u32;
pub const TREE_SEC_INFO_RESET_KEEP_EXPLICIT: TREE_SEC_INFO = 3u32;
pub const TREE_SEC_INFO_SET: TREE_SEC_INFO = 1u32;
pub const TRUSTEE_ACCESS_ALL: i32 = -1i32;
pub const TRUSTEE_ACCESS_ALLOWED: i32 = 1i32;
pub const TRUSTEE_ACCESS_EXPLICIT: i32 = 1i32;
pub const TRUSTEE_ACCESS_READ: i32 = 2i32;
pub const TRUSTEE_ACCESS_WRITE: i32 = 4i32;
pub const TRUSTEE_BAD_FORM: TRUSTEE_FORM = 2i32;
pub const TRUSTEE_IS_ALIAS: TRUSTEE_TYPE = 4i32;
pub const TRUSTEE_IS_COMPUTER: TRUSTEE_TYPE = 8i32;
pub const TRUSTEE_IS_DELETED: TRUSTEE_TYPE = 6i32;
pub const TRUSTEE_IS_DOMAIN: TRUSTEE_TYPE = 3i32;
pub const TRUSTEE_IS_GROUP: TRUSTEE_TYPE = 2i32;
pub const TRUSTEE_IS_IMPERSONATE: MULTIPLE_TRUSTEE_OPERATION = 1i32;
pub const TRUSTEE_IS_INVALID: TRUSTEE_TYPE = 7i32;
pub const TRUSTEE_IS_NAME: TRUSTEE_FORM = 1i32;
pub const TRUSTEE_IS_OBJECTS_AND_NAME: TRUSTEE_FORM = 4i32;
pub const TRUSTEE_IS_OBJECTS_AND_SID: TRUSTEE_FORM = 3i32;
pub const TRUSTEE_IS_SID: TRUSTEE_FORM = 0i32;
pub const TRUSTEE_IS_UNKNOWN: TRUSTEE_TYPE = 0i32;
pub const TRUSTEE_IS_USER: TRUSTEE_TYPE = 1i32;
pub const TRUSTEE_IS_WELL_KNOWN_GROUP: TRUSTEE_TYPE = 5i32;
pub const _AUTHZ_SS_MAXSIZE: u32 = 128u32;
pub type ACCESS_MODE = i32;
pub type ACTRL_ACCESS_ENTRY_ACCESS_FLAGS = u32;
pub type AUDIT_PARAM_TYPE = i32;
pub type AUTHZ_ACCESS_CHECK_FLAGS = u32;
pub type AUTHZ_AUDIT_EVENT_INFORMATION_CLASS = i32;
pub type AUTHZ_CONTEXT_INFORMATION_CLASS = i32;
pub type AUTHZ_GENERATE_RESULTS = u32;
pub type AUTHZ_INITIALIZE_OBJECT_ACCESS_AUDIT_EVENT_FLAGS = u32;
pub type AUTHZ_RESOURCE_MANAGER_FLAGS = u32;
pub type AUTHZ_SECURITY_ATTRIBUTE_FLAGS = u32;
pub type AUTHZ_SECURITY_ATTRIBUTE_OPERATION = i32;
pub type AUTHZ_SID_OPERATION = i32;
pub type AZ_PROP_CONSTANTS = i32;
pub type MULTIPLE_TRUSTEE_OPERATION = i32;
pub type PROG_INVOKE_SETTING = i32;
pub type SE_OBJECT_TYPE = i32;
pub type TREE_SEC_INFO = u32;
pub type TRUSTEE_FORM = i32;
pub type TRUSTEE_TYPE = i32;
#[repr(C)]
pub struct ACTRL_ACCESSA {
    pub cEntries: u32,
    pub pPropertyAccessList: *mut ACTRL_PROPERTY_ENTRYA,
}
impl ::core::marker::Copy for ACTRL_ACCESSA {}
impl ::core::clone::Clone for ACTRL_ACCESSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESSW {
    pub cEntries: u32,
    pub pPropertyAccessList: *mut ACTRL_PROPERTY_ENTRYW,
}
impl ::core::marker::Copy for ACTRL_ACCESSW {}
impl ::core::clone::Clone for ACTRL_ACCESSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESS_ENTRYA {
    pub Trustee: TRUSTEE_A,
    pub fAccessFlags: ACTRL_ACCESS_ENTRY_ACCESS_FLAGS,
    pub Access: u32,
    pub ProvSpecificAccess: u32,
    pub Inheritance: super::ACE_FLAGS,
    pub lpInheritProperty: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for ACTRL_ACCESS_ENTRYA {}
impl ::core::clone::Clone for ACTRL_ACCESS_ENTRYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESS_ENTRYW {
    pub Trustee: TRUSTEE_W,
    pub fAccessFlags: ACTRL_ACCESS_ENTRY_ACCESS_FLAGS,
    pub Access: u32,
    pub ProvSpecificAccess: u32,
    pub Inheritance: super::ACE_FLAGS,
    pub lpInheritProperty: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ACTRL_ACCESS_ENTRYW {}
impl ::core::clone::Clone for ACTRL_ACCESS_ENTRYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESS_ENTRY_LISTA {
    pub cEntries: u32,
    pub pAccessList: *mut ACTRL_ACCESS_ENTRYA,
}
impl ::core::marker::Copy for ACTRL_ACCESS_ENTRY_LISTA {}
impl ::core::clone::Clone for ACTRL_ACCESS_ENTRY_LISTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESS_ENTRY_LISTW {
    pub cEntries: u32,
    pub pAccessList: *mut ACTRL_ACCESS_ENTRYW,
}
impl ::core::marker::Copy for ACTRL_ACCESS_ENTRY_LISTW {}
impl ::core::clone::Clone for ACTRL_ACCESS_ENTRY_LISTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESS_INFOA {
    pub fAccessPermission: u32,
    pub lpAccessPermissionName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for ACTRL_ACCESS_INFOA {}
impl ::core::clone::Clone for ACTRL_ACCESS_INFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_ACCESS_INFOW {
    pub fAccessPermission: u32,
    pub lpAccessPermissionName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ACTRL_ACCESS_INFOW {}
impl ::core::clone::Clone for ACTRL_ACCESS_INFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_CONTROL_INFOA {
    pub lpControlId: ::windows_sys::core::PSTR,
    pub lpControlName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for ACTRL_CONTROL_INFOA {}
impl ::core::clone::Clone for ACTRL_CONTROL_INFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_CONTROL_INFOW {
    pub lpControlId: ::windows_sys::core::PWSTR,
    pub lpControlName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for ACTRL_CONTROL_INFOW {}
impl ::core::clone::Clone for ACTRL_CONTROL_INFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct ACTRL_OVERLAPPED {
    pub Anonymous: ACTRL_OVERLAPPED_0,
    pub Reserved2: u32,
    pub hEvent: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ACTRL_OVERLAPPED {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ACTRL_OVERLAPPED {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union ACTRL_OVERLAPPED_0 {
    pub Provider: *mut ::core::ffi::c_void,
    pub Reserved1: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for ACTRL_OVERLAPPED_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for ACTRL_OVERLAPPED_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_PROPERTY_ENTRYA {
    pub lpProperty: ::windows_sys::core::PSTR,
    pub pAccessEntryList: *mut ACTRL_ACCESS_ENTRY_LISTA,
    pub fListFlags: u32,
}
impl ::core::marker::Copy for ACTRL_PROPERTY_ENTRYA {}
impl ::core::clone::Clone for ACTRL_PROPERTY_ENTRYA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ACTRL_PROPERTY_ENTRYW {
    pub lpProperty: ::windows_sys::core::PWSTR,
    pub pAccessEntryList: *mut ACTRL_ACCESS_ENTRY_LISTW,
    pub fListFlags: u32,
}
impl ::core::marker::Copy for ACTRL_PROPERTY_ENTRYW {}
impl ::core::clone::Clone for ACTRL_PROPERTY_ENTRYW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUDIT_IP_ADDRESS {
    pub pIpAddress: [u8; 128],
}
impl ::core::marker::Copy for AUDIT_IP_ADDRESS {}
impl ::core::clone::Clone for AUDIT_IP_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUDIT_OBJECT_TYPE {
    pub ObjectType: ::windows_sys::core::GUID,
    pub Flags: u16,
    pub Level: u16,
    pub AccessMask: u32,
}
impl ::core::marker::Copy for AUDIT_OBJECT_TYPE {}
impl ::core::clone::Clone for AUDIT_OBJECT_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUDIT_OBJECT_TYPES {
    pub Count: u16,
    pub Flags: u16,
    pub pObjectTypes: *mut AUDIT_OBJECT_TYPE,
}
impl ::core::marker::Copy for AUDIT_OBJECT_TYPES {}
impl ::core::clone::Clone for AUDIT_OBJECT_TYPES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUDIT_PARAM {
    pub Type: AUDIT_PARAM_TYPE,
    pub Length: u32,
    pub Flags: u32,
    pub Anonymous1: AUDIT_PARAM_0,
    pub Anonymous2: AUDIT_PARAM_1,
}
impl ::core::marker::Copy for AUDIT_PARAM {}
impl ::core::clone::Clone for AUDIT_PARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union AUDIT_PARAM_0 {
    pub Data0: usize,
    pub String: ::windows_sys::core::PWSTR,
    pub u: usize,
    pub psid: *mut super::SID,
    pub pguid: *mut ::windows_sys::core::GUID,
    pub LogonId_LowPart: u32,
    pub pObjectTypes: *mut AUDIT_OBJECT_TYPES,
    pub pIpAddress: *mut AUDIT_IP_ADDRESS,
}
impl ::core::marker::Copy for AUDIT_PARAM_0 {}
impl ::core::clone::Clone for AUDIT_PARAM_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union AUDIT_PARAM_1 {
    pub Data1: usize,
    pub LogonId_HighPart: i32,
}
impl ::core::marker::Copy for AUDIT_PARAM_1 {}
impl ::core::clone::Clone for AUDIT_PARAM_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUDIT_PARAMS {
    pub Length: u32,
    pub Flags: u32,
    pub Count: u16,
    pub Parameters: *mut AUDIT_PARAM,
}
impl ::core::marker::Copy for AUDIT_PARAMS {}
impl ::core::clone::Clone for AUDIT_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
pub type AUTHZ_ACCESS_CHECK_RESULTS_HANDLE = isize;
#[repr(C)]
pub struct AUTHZ_ACCESS_REPLY {
    pub ResultListLength: u32,
    pub GrantedAccessMask: *mut u32,
    pub SaclEvaluationResults: *mut AUTHZ_GENERATE_RESULTS,
    pub Error: *mut u32,
}
impl ::core::marker::Copy for AUTHZ_ACCESS_REPLY {}
impl ::core::clone::Clone for AUTHZ_ACCESS_REPLY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct AUTHZ_ACCESS_REQUEST {
    pub DesiredAccess: u32,
    pub PrincipalSelfSid: super::super::Foundation::PSID,
    pub ObjectTypeList: *mut super::OBJECT_TYPE_LIST,
    pub ObjectTypeListLength: u32,
    pub OptionalArguments: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for AUTHZ_ACCESS_REQUEST {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for AUTHZ_ACCESS_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
pub type AUTHZ_AUDIT_EVENT_HANDLE = isize;
pub type AUTHZ_AUDIT_EVENT_TYPE_HANDLE = isize;
#[repr(C)]
pub struct AUTHZ_AUDIT_EVENT_TYPE_LEGACY {
    pub CategoryId: u16,
    pub AuditId: u16,
    pub ParameterCount: u16,
}
impl ::core::marker::Copy for AUTHZ_AUDIT_EVENT_TYPE_LEGACY {}
impl ::core::clone::Clone for AUTHZ_AUDIT_EVENT_TYPE_LEGACY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct AUTHZ_AUDIT_EVENT_TYPE_OLD {
    pub Version: u32,
    pub dwFlags: u32,
    pub RefCount: i32,
    pub hAudit: usize,
    pub LinkId: super::super::Foundation::LUID,
    pub u: AUTHZ_AUDIT_EVENT_TYPE_UNION,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for AUTHZ_AUDIT_EVENT_TYPE_OLD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for AUTHZ_AUDIT_EVENT_TYPE_OLD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union AUTHZ_AUDIT_EVENT_TYPE_UNION {
    pub Legacy: AUTHZ_AUDIT_EVENT_TYPE_LEGACY,
}
impl ::core::marker::Copy for AUTHZ_AUDIT_EVENT_TYPE_UNION {}
impl ::core::clone::Clone for AUTHZ_AUDIT_EVENT_TYPE_UNION {
    fn clone(&self) -> Self {
        *self
    }
}
pub type AUTHZ_CAP_CHANGE_SUBSCRIPTION_HANDLE = isize;
pub type AUTHZ_CLIENT_CONTEXT_HANDLE = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct AUTHZ_INIT_INFO {
    pub version: u16,
    pub szResourceManagerName: ::windows_sys::core::PCWSTR,
    pub pfnDynamicAccessCheck: PFN_AUTHZ_DYNAMIC_ACCESS_CHECK,
    pub pfnComputeDynamicGroups: PFN_AUTHZ_COMPUTE_DYNAMIC_GROUPS,
    pub pfnFreeDynamicGroups: PFN_AUTHZ_FREE_DYNAMIC_GROUPS,
    pub pfnGetCentralAccessPolicy: PFN_AUTHZ_GET_CENTRAL_ACCESS_POLICY,
    pub pfnFreeCentralAccessPolicy: PFN_AUTHZ_FREE_CENTRAL_ACCESS_POLICY,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for AUTHZ_INIT_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for AUTHZ_INIT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUTHZ_REGISTRATION_OBJECT_TYPE_NAME_OFFSET {
    pub szObjectTypeName: ::windows_sys::core::PWSTR,
    pub dwOffset: u32,
}
impl ::core::marker::Copy for AUTHZ_REGISTRATION_OBJECT_TYPE_NAME_OFFSET {}
impl ::core::clone::Clone for AUTHZ_REGISTRATION_OBJECT_TYPE_NAME_OFFSET {
    fn clone(&self) -> Self {
        *self
    }
}
pub type AUTHZ_RESOURCE_MANAGER_HANDLE = isize;
#[repr(C)]
pub struct AUTHZ_RPC_INIT_INFO_CLIENT {
    pub version: u16,
    pub ObjectUuid: ::windows_sys::core::PWSTR,
    pub ProtSeq: ::windows_sys::core::PWSTR,
    pub NetworkAddr: ::windows_sys::core::PWSTR,
    pub Endpoint: ::windows_sys::core::PWSTR,
    pub Options: ::windows_sys::core::PWSTR,
    pub ServerSpn: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for AUTHZ_RPC_INIT_INFO_CLIENT {}
impl ::core::clone::Clone for AUTHZ_RPC_INIT_INFO_CLIENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUTHZ_SECURITY_ATTRIBUTES_INFORMATION {
    pub Version: u16,
    pub Reserved: u16,
    pub AttributeCount: u32,
    pub Attribute: AUTHZ_SECURITY_ATTRIBUTES_INFORMATION_0,
}
impl ::core::marker::Copy for AUTHZ_SECURITY_ATTRIBUTES_INFORMATION {}
impl ::core::clone::Clone for AUTHZ_SECURITY_ATTRIBUTES_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union AUTHZ_SECURITY_ATTRIBUTES_INFORMATION_0 {
    pub pAttributeV1: *mut AUTHZ_SECURITY_ATTRIBUTE_V1,
}
impl ::core::marker::Copy for AUTHZ_SECURITY_ATTRIBUTES_INFORMATION_0 {}
impl ::core::clone::Clone for AUTHZ_SECURITY_ATTRIBUTES_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUTHZ_SECURITY_ATTRIBUTE_FQBN_VALUE {
    pub Version: u64,
    pub pName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for AUTHZ_SECURITY_ATTRIBUTE_FQBN_VALUE {}
impl ::core::clone::Clone for AUTHZ_SECURITY_ATTRIBUTE_FQBN_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUTHZ_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE {
    pub pValue: *mut ::core::ffi::c_void,
    pub ValueLength: u32,
}
impl ::core::marker::Copy for AUTHZ_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE {}
impl ::core::clone::Clone for AUTHZ_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct AUTHZ_SECURITY_ATTRIBUTE_V1 {
    pub pName: ::windows_sys::core::PWSTR,
    pub ValueType: u16,
    pub Reserved: u16,
    pub Flags: AUTHZ_SECURITY_ATTRIBUTE_FLAGS,
    pub ValueCount: u32,
    pub Values: AUTHZ_SECURITY_ATTRIBUTE_V1_0,
}
impl ::core::marker::Copy for AUTHZ_SECURITY_ATTRIBUTE_V1 {}
impl ::core::clone::Clone for AUTHZ_SECURITY_ATTRIBUTE_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union AUTHZ_SECURITY_ATTRIBUTE_V1_0 {
    pub pInt64: *mut i64,
    pub pUint64: *mut u64,
    pub ppString: *mut ::windows_sys::core::PWSTR,
    pub pFqbn: *mut AUTHZ_SECURITY_ATTRIBUTE_FQBN_VALUE,
    pub pOctetString: *mut AUTHZ_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE,
}
impl ::core::marker::Copy for AUTHZ_SECURITY_ATTRIBUTE_V1_0 {}
impl ::core::clone::Clone for AUTHZ_SECURITY_ATTRIBUTE_V1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type AUTHZ_SECURITY_EVENT_PROVIDER_HANDLE = isize;
#[repr(C)]
pub struct AUTHZ_SOURCE_SCHEMA_REGISTRATION {
    pub dwFlags: u32,
    pub szEventSourceName: ::windows_sys::core::PWSTR,
    pub szEventMessageFile: ::windows_sys::core::PWSTR,
    pub szEventSourceXmlSchemaFile: ::windows_sys::core::PWSTR,
    pub szEventAccessStringsFile: ::windows_sys::core::PWSTR,
    pub szExecutableImagePath: ::windows_sys::core::PWSTR,
    pub Anonymous: AUTHZ_SOURCE_SCHEMA_REGISTRATION_0,
    pub dwObjectTypeNameCount: u32,
    pub ObjectTypeNames: [AUTHZ_REGISTRATION_OBJECT_TYPE_NAME_OFFSET; 1],
}
impl ::core::marker::Copy for AUTHZ_SOURCE_SCHEMA_REGISTRATION {}
impl ::core::clone::Clone for AUTHZ_SOURCE_SCHEMA_REGISTRATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union AUTHZ_SOURCE_SCHEMA_REGISTRATION_0 {
    pub pReserved: *mut ::core::ffi::c_void,
    pub pProviderGuid: *mut ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for AUTHZ_SOURCE_SCHEMA_REGISTRATION_0 {}
impl ::core::clone::Clone for AUTHZ_SOURCE_SCHEMA_REGISTRATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXPLICIT_ACCESS_A {
    pub grfAccessPermissions: u32,
    pub grfAccessMode: ACCESS_MODE,
    pub grfInheritance: super::ACE_FLAGS,
    pub Trustee: TRUSTEE_A,
}
impl ::core::marker::Copy for EXPLICIT_ACCESS_A {}
impl ::core::clone::Clone for EXPLICIT_ACCESS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXPLICIT_ACCESS_W {
    pub grfAccessPermissions: u32,
    pub grfAccessMode: ACCESS_MODE,
    pub grfInheritance: super::ACE_FLAGS,
    pub Trustee: TRUSTEE_W,
}
impl ::core::marker::Copy for EXPLICIT_ACCESS_W {}
impl ::core::clone::Clone for EXPLICIT_ACCESS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FN_OBJECT_MGR_FUNCTS {
    pub Placeholder: u32,
}
impl ::core::marker::Copy for FN_OBJECT_MGR_FUNCTS {}
impl ::core::clone::Clone for FN_OBJECT_MGR_FUNCTS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INHERITED_FROMA {
    pub GenerationGap: i32,
    pub AncestorName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for INHERITED_FROMA {}
impl ::core::clone::Clone for INHERITED_FROMA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct INHERITED_FROMW {
    pub GenerationGap: i32,
    pub AncestorName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for INHERITED_FROMW {}
impl ::core::clone::Clone for INHERITED_FROMW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OBJECTS_AND_NAME_A {
    pub ObjectsPresent: super::SYSTEM_AUDIT_OBJECT_ACE_FLAGS,
    pub ObjectType: SE_OBJECT_TYPE,
    pub ObjectTypeName: ::windows_sys::core::PSTR,
    pub InheritedObjectTypeName: ::windows_sys::core::PSTR,
    pub ptstrName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for OBJECTS_AND_NAME_A {}
impl ::core::clone::Clone for OBJECTS_AND_NAME_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OBJECTS_AND_NAME_W {
    pub ObjectsPresent: super::SYSTEM_AUDIT_OBJECT_ACE_FLAGS,
    pub ObjectType: SE_OBJECT_TYPE,
    pub ObjectTypeName: ::windows_sys::core::PWSTR,
    pub InheritedObjectTypeName: ::windows_sys::core::PWSTR,
    pub ptstrName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for OBJECTS_AND_NAME_W {}
impl ::core::clone::Clone for OBJECTS_AND_NAME_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OBJECTS_AND_SID {
    pub ObjectsPresent: super::SYSTEM_AUDIT_OBJECT_ACE_FLAGS,
    pub ObjectTypeGuid: ::windows_sys::core::GUID,
    pub InheritedObjectTypeGuid: ::windows_sys::core::GUID,
    pub pSid: *mut super::SID,
}
impl ::core::marker::Copy for OBJECTS_AND_SID {}
impl ::core::clone::Clone for OBJECTS_AND_SID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRUSTEE_A {
    pub pMultipleTrustee: *mut TRUSTEE_A,
    pub MultipleTrusteeOperation: MULTIPLE_TRUSTEE_OPERATION,
    pub TrusteeForm: TRUSTEE_FORM,
    pub TrusteeType: TRUSTEE_TYPE,
    pub ptstrName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for TRUSTEE_A {}
impl ::core::clone::Clone for TRUSTEE_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRUSTEE_ACCESSA {
    pub lpProperty: ::windows_sys::core::PSTR,
    pub Access: u32,
    pub fAccessFlags: u32,
    pub fReturnedAccess: u32,
}
impl ::core::marker::Copy for TRUSTEE_ACCESSA {}
impl ::core::clone::Clone for TRUSTEE_ACCESSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRUSTEE_ACCESSW {
    pub lpProperty: ::windows_sys::core::PWSTR,
    pub Access: u32,
    pub fAccessFlags: u32,
    pub fReturnedAccess: u32,
}
impl ::core::marker::Copy for TRUSTEE_ACCESSW {}
impl ::core::clone::Clone for TRUSTEE_ACCESSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TRUSTEE_W {
    pub pMultipleTrustee: *mut TRUSTEE_W,
    pub MultipleTrusteeOperation: MULTIPLE_TRUSTEE_OPERATION,
    pub TrusteeForm: TRUSTEE_FORM,
    pub TrusteeType: TRUSTEE_TYPE,
    pub ptstrName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for TRUSTEE_W {}
impl ::core::clone::Clone for TRUSTEE_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type FN_PROGRESS = ::core::option::Option<unsafe extern "system" fn(pobjectname: ::windows_sys::core::PCWSTR, status: u32, pinvokesetting: *mut PROG_INVOKE_SETTING, args: *const ::core::ffi::c_void, securityset: super::super::Foundation::BOOL) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_AUTHZ_COMPUTE_DYNAMIC_GROUPS = ::core::option::Option<unsafe extern "system" fn(hauthzclientcontext: AUTHZ_CLIENT_CONTEXT_HANDLE, args: *const ::core::ffi::c_void, psidattrarray: *mut *mut super::SID_AND_ATTRIBUTES, psidcount: *mut u32, prestrictedsidattrarray: *mut *mut super::SID_AND_ATTRIBUTES, prestrictedsidcount: *mut u32) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_AUTHZ_DYNAMIC_ACCESS_CHECK = ::core::option::Option<unsafe extern "system" fn(hauthzclientcontext: AUTHZ_CLIENT_CONTEXT_HANDLE, pace: *const super::ACE_HEADER, pargs: *const ::core::ffi::c_void, pbaceapplicable: *mut super::super::Foundation::BOOL) -> super::super::Foundation::BOOL>;
pub type PFN_AUTHZ_FREE_CENTRAL_ACCESS_POLICY = ::core::option::Option<unsafe extern "system" fn(pcentralaccesspolicy: *const ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_AUTHZ_FREE_DYNAMIC_GROUPS = ::core::option::Option<unsafe extern "system" fn(psidattrarray: *const super::SID_AND_ATTRIBUTES) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFN_AUTHZ_GET_CENTRAL_ACCESS_POLICY = ::core::option::Option<unsafe extern "system" fn(hauthzclientcontext: AUTHZ_CLIENT_CONTEXT_HANDLE, capid: super::super::Foundation::PSID, pargs: *const ::core::ffi::c_void, pcentralaccesspolicyapplicable: *mut super::super::Foundation::BOOL, ppcentralaccesspolicy: *mut *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
