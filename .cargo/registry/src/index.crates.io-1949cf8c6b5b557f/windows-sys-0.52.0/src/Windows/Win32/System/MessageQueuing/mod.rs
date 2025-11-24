::windows_targets::link!("mqrt.dll" "system" fn MQADsPathToFormatName(lpwcsadspath : ::windows_sys::core::PCWSTR, lpwcsformatname : ::windows_sys::core::PWSTR, lpdwformatnamelength : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_DistributedTransactionCoordinator")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_System_DistributedTransactionCoordinator\"`"] fn MQBeginTransaction(pptransaction : *mut super::DistributedTransactionCoordinator:: ITransaction) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQCloseCursor(hcursor : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQCloseQueue(hqueue : isize) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQCreateCursor(hqueue : isize, phcursor : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQCreateQueue(psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR, pqueueprops : *mut MQQUEUEPROPS, lpwcsformatname : ::windows_sys::core::PWSTR, lpdwformatnamelength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQDeleteQueue(lpwcsformatname : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQFreeMemory(pvmemory : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQFreeSecurityContext(hsecuritycontext : super::super::Foundation:: HANDLE) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQGetMachineProperties(lpwcsmachinename : ::windows_sys::core::PCWSTR, pguidmachineid : *const ::windows_sys::core::GUID, pqmprops : *mut MQQMPROPS) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn MQGetOverlappedResult(lpoverlapped : *const super::IO:: OVERLAPPED) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQGetPrivateComputerInformation(lpwcscomputername : ::windows_sys::core::PCWSTR, pprivateprops : *mut MQPRIVATEPROPS) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQGetQueueProperties(lpwcsformatname : ::windows_sys::core::PCWSTR, pqueueprops : *mut MQQUEUEPROPS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn MQGetQueueSecurity(lpwcsformatname : ::windows_sys::core::PCWSTR, requestedinformation : u32, psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR, nlength : u32, lpnlengthneeded : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQGetSecurityContext(lpcertbuffer : *const ::core::ffi::c_void, dwcertbufferlength : u32, phsecuritycontext : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQGetSecurityContextEx(lpcertbuffer : *const ::core::ffi::c_void, dwcertbufferlength : u32, phsecuritycontext : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQHandleToFormatName(hqueue : isize, lpwcsformatname : ::windows_sys::core::PWSTR, lpdwformatnamelength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQInstanceToFormatName(pguid : *const ::windows_sys::core::GUID, lpwcsformatname : ::windows_sys::core::PWSTR, lpdwformatnamelength : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQLocateBegin(lpwcscontext : ::windows_sys::core::PCWSTR, prestriction : *const MQRESTRICTION, pcolumns : *const MQCOLUMNSET, psort : *const MQSORTSET, phenum : *mut super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQLocateEnd(henum : super::super::Foundation:: HANDLE) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQLocateNext(henum : super::super::Foundation:: HANDLE, pcprops : *mut u32, apropvar : *mut super::Com::StructuredStorage:: PROPVARIANT) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MQMarkMessageRejected(hqueue : super::super::Foundation:: HANDLE, ulllookupid : u64) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQMgmtAction(pcomputername : ::windows_sys::core::PCWSTR, pobjectname : ::windows_sys::core::PCWSTR, paction : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQMgmtGetInfo(pcomputername : ::windows_sys::core::PCWSTR, pobjectname : ::windows_sys::core::PCWSTR, pmgmtprops : *mut MQMGMTPROPS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_DistributedTransactionCoordinator")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_System_DistributedTransactionCoordinator\"`"] fn MQMoveMessage(hsourcequeue : isize, hdestinationqueue : isize, ulllookupid : u64, ptransaction : super::DistributedTransactionCoordinator:: ITransaction) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQOpenQueue(lpwcsformatname : ::windows_sys::core::PCWSTR, dwaccess : u32, dwsharemode : u32, phqueue : *mut isize) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQPathNameToFormatName(lpwcspathname : ::windows_sys::core::PCWSTR, lpwcsformatname : ::windows_sys::core::PWSTR, lpdwformatnamelength : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQPurgeQueue(hqueue : isize) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_DistributedTransactionCoordinator", feature = "Win32_System_IO", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_DistributedTransactionCoordinator\"`, `\"Win32_System_IO\"`, `\"Win32_System_Variant\"`"] fn MQReceiveMessage(hsource : isize, dwtimeout : u32, dwaction : u32, pmessageprops : *mut MQMSGPROPS, lpoverlapped : *mut super::IO:: OVERLAPPED, fnreceivecallback : PMQRECEIVECALLBACK, hcursor : super::super::Foundation:: HANDLE, ptransaction : super::DistributedTransactionCoordinator:: ITransaction) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_DistributedTransactionCoordinator", feature = "Win32_System_IO", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_DistributedTransactionCoordinator\"`, `\"Win32_System_IO\"`, `\"Win32_System_Variant\"`"] fn MQReceiveMessageByLookupId(hsource : isize, ulllookupid : u64, dwlookupaction : u32, pmessageprops : *mut MQMSGPROPS, lpoverlapped : *mut super::IO:: OVERLAPPED, fnreceivecallback : PMQRECEIVECALLBACK, ptransaction : super::DistributedTransactionCoordinator:: ITransaction) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("mqrt.dll" "system" fn MQRegisterCertificate(dwflags : u32, lpcertbuffer : *const ::core::ffi::c_void, dwcertbufferlength : u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_DistributedTransactionCoordinator", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_DistributedTransactionCoordinator\"`, `\"Win32_System_Variant\"`"] fn MQSendMessage(hdestinationqueue : isize, pmessageprops : *const MQMSGPROPS, ptransaction : super::DistributedTransactionCoordinator:: ITransaction) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"] fn MQSetQueueProperties(lpwcsformatname : ::windows_sys::core::PCWSTR, pqueueprops : *mut MQQUEUEPROPS) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("mqrt.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn MQSetQueueSecurity(lpwcsformatname : ::windows_sys::core::PCWSTR, securityinformation : super::super::Security:: OBJECT_SECURITY_INFORMATION, psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR) -> ::windows_sys::core::HRESULT);
pub type IMSMQApplication = *mut ::core::ffi::c_void;
pub type IMSMQApplication2 = *mut ::core::ffi::c_void;
pub type IMSMQApplication3 = *mut ::core::ffi::c_void;
pub type IMSMQCollection = *mut ::core::ffi::c_void;
pub type IMSMQCoordinatedTransactionDispenser = *mut ::core::ffi::c_void;
pub type IMSMQCoordinatedTransactionDispenser2 = *mut ::core::ffi::c_void;
pub type IMSMQCoordinatedTransactionDispenser3 = *mut ::core::ffi::c_void;
pub type IMSMQDestination = *mut ::core::ffi::c_void;
pub type IMSMQEvent = *mut ::core::ffi::c_void;
pub type IMSMQEvent2 = *mut ::core::ffi::c_void;
pub type IMSMQEvent3 = *mut ::core::ffi::c_void;
pub type IMSMQManagement = *mut ::core::ffi::c_void;
pub type IMSMQMessage = *mut ::core::ffi::c_void;
pub type IMSMQMessage2 = *mut ::core::ffi::c_void;
pub type IMSMQMessage3 = *mut ::core::ffi::c_void;
pub type IMSMQMessage4 = *mut ::core::ffi::c_void;
pub type IMSMQOutgoingQueueManagement = *mut ::core::ffi::c_void;
pub type IMSMQPrivateDestination = *mut ::core::ffi::c_void;
pub type IMSMQPrivateEvent = *mut ::core::ffi::c_void;
pub type IMSMQQuery = *mut ::core::ffi::c_void;
pub type IMSMQQuery2 = *mut ::core::ffi::c_void;
pub type IMSMQQuery3 = *mut ::core::ffi::c_void;
pub type IMSMQQuery4 = *mut ::core::ffi::c_void;
pub type IMSMQQueue = *mut ::core::ffi::c_void;
pub type IMSMQQueue2 = *mut ::core::ffi::c_void;
pub type IMSMQQueue3 = *mut ::core::ffi::c_void;
pub type IMSMQQueue4 = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfo = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfo2 = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfo3 = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfo4 = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfos = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfos2 = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfos3 = *mut ::core::ffi::c_void;
pub type IMSMQQueueInfos4 = *mut ::core::ffi::c_void;
pub type IMSMQQueueManagement = *mut ::core::ffi::c_void;
pub type IMSMQTransaction = *mut ::core::ffi::c_void;
pub type IMSMQTransaction2 = *mut ::core::ffi::c_void;
pub type IMSMQTransaction3 = *mut ::core::ffi::c_void;
pub type IMSMQTransactionDispenser = *mut ::core::ffi::c_void;
pub type IMSMQTransactionDispenser2 = *mut ::core::ffi::c_void;
pub type IMSMQTransactionDispenser3 = *mut ::core::ffi::c_void;
pub type _DMSMQEventEvents = *mut ::core::ffi::c_void;
pub const DEFAULT_M_ACKNOWLEDGE: MQDEFAULT = 0i32;
pub const DEFAULT_M_APPSPECIFIC: MQDEFAULT = 0i32;
pub const DEFAULT_M_AUTH_LEVEL: MQDEFAULT = 0i32;
pub const DEFAULT_M_DELIVERY: MQDEFAULT = 0i32;
pub const DEFAULT_M_JOURNAL: MQDEFAULT = 0i32;
pub const DEFAULT_M_LOOKUPID: MQDEFAULT = 0i32;
pub const DEFAULT_M_PRIORITY: MQDEFAULT = 3i32;
pub const DEFAULT_M_PRIV_LEVEL: MQDEFAULT = 0i32;
pub const DEFAULT_M_SENDERID_TYPE: MQDEFAULT = 1i32;
pub const DEFAULT_Q_AUTHENTICATE: MQDEFAULT = 0i32;
pub const DEFAULT_Q_BASEPRIORITY: MQDEFAULT = 0i32;
pub const DEFAULT_Q_JOURNAL: MQDEFAULT = 0i32;
pub const DEFAULT_Q_JOURNAL_QUOTA: MQDEFAULT = -1i32;
pub const DEFAULT_Q_PRIV_LEVEL: MQDEFAULT = 1i32;
pub const DEFAULT_Q_QUOTA: MQDEFAULT = -1i32;
pub const DEFAULT_Q_TRANSACTION: MQDEFAULT = 0i32;
pub const LONG_LIVED: u32 = 4294967294u32;
pub const MACHINE_ACTION_CONNECT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CONNECT");
pub const MACHINE_ACTION_DISCONNECT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DISCONNECT");
pub const MACHINE_ACTION_TIDY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TIDY");
pub const MGMT_QUEUE_CORRECT_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("YES");
pub const MGMT_QUEUE_FOREIGN_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("YES");
pub const MGMT_QUEUE_INCORRECT_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NO");
pub const MGMT_QUEUE_LOCAL_LOCATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LOCAL");
pub const MGMT_QUEUE_NOT_FOREIGN_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NO");
pub const MGMT_QUEUE_NOT_TRANSACTIONAL_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NO");
pub const MGMT_QUEUE_REMOTE_LOCATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("REMOTE");
pub const MGMT_QUEUE_STATE_CONNECTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CONNECTED");
pub const MGMT_QUEUE_STATE_DISCONNECTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DISCONNECTED");
pub const MGMT_QUEUE_STATE_DISCONNECTING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DISCONNECTING");
pub const MGMT_QUEUE_STATE_LOCAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LOCAL CONNECTION");
pub const MGMT_QUEUE_STATE_LOCKED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LOCKED");
pub const MGMT_QUEUE_STATE_NEED_VALIDATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NEED VALIDATION");
pub const MGMT_QUEUE_STATE_NONACTIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("INACTIVE");
pub const MGMT_QUEUE_STATE_ONHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ONHOLD");
pub const MGMT_QUEUE_STATE_WAITING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WAITING");
pub const MGMT_QUEUE_TRANSACTIONAL_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("YES");
pub const MGMT_QUEUE_TYPE_CONNECTOR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CONNECTOR");
pub const MGMT_QUEUE_TYPE_MACHINE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MACHINE");
pub const MGMT_QUEUE_TYPE_MULTICAST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MULTICAST");
pub const MGMT_QUEUE_TYPE_PRIVATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PRIVATE");
pub const MGMT_QUEUE_TYPE_PUBLIC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PUBLIC");
pub const MGMT_QUEUE_UNKNOWN_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UNKNOWN");
pub const MO_MACHINE_TOKEN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MACHINE");
pub const MO_QUEUE_TOKEN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("QUEUE");
pub const MQCERT_REGISTER_ALWAYS: MQCERT_REGISTER = 1i32;
pub const MQCERT_REGISTER_IF_NOT_EXIST: MQCERT_REGISTER = 2i32;
pub const MQCONN_BIND_SOCKET_FAILURE: MQConnectionState = -2147483645i32;
pub const MQCONN_CONNECT_SOCKET_FAILURE: MQConnectionState = -2147483644i32;
pub const MQCONN_CREATE_SOCKET_FAILURE: MQConnectionState = -2147483646i32;
pub const MQCONN_ESTABLISH_PACKET_RECEIVED: MQConnectionState = 1i32;
pub const MQCONN_INVALID_SERVER_CERT: MQConnectionState = -2147483639i32;
pub const MQCONN_LIMIT_REACHED: MQConnectionState = -2147483638i32;
pub const MQCONN_NAME_RESOLUTION_FAILURE: MQConnectionState = -2147483640i32;
pub const MQCONN_NOFAILURE: MQConnectionState = 0i32;
pub const MQCONN_NOT_READY: MQConnectionState = -2147483641i32;
pub const MQCONN_OUT_OF_MEMORY: MQConnectionState = -2147483635i32;
pub const MQCONN_PING_FAILURE: MQConnectionState = -2147483647i32;
pub const MQCONN_READY: MQConnectionState = 2i32;
pub const MQCONN_REFUSED_BY_OTHER_SIDE: MQConnectionState = -2147483637i32;
pub const MQCONN_ROUTING_FAILURE: MQConnectionState = -2147483636i32;
pub const MQCONN_SEND_FAILURE: MQConnectionState = -2147483642i32;
pub const MQCONN_TCP_NOT_ENABLED: MQConnectionState = -2147483643i32;
pub const MQCONN_UNKNOWN_FAILURE: MQConnectionState = -2147483648i32;
pub const MQMSG_ACKNOWLEDGMENT_FULL_REACH_QUEUE: MQMSGACKNOWLEDGEMENT = 5i32;
pub const MQMSG_ACKNOWLEDGMENT_FULL_RECEIVE: MQMSGACKNOWLEDGEMENT = 14i32;
pub const MQMSG_ACKNOWLEDGMENT_NACK_REACH_QUEUE: MQMSGACKNOWLEDGEMENT = 4i32;
pub const MQMSG_ACKNOWLEDGMENT_NACK_RECEIVE: MQMSGACKNOWLEDGEMENT = 12i32;
pub const MQMSG_ACKNOWLEDGMENT_NEG_ARRIVAL: MQMSGACKNOWLEDGEMENT = 4i32;
pub const MQMSG_ACKNOWLEDGMENT_NEG_RECEIVE: MQMSGACKNOWLEDGEMENT = 8i32;
pub const MQMSG_ACKNOWLEDGMENT_NONE: MQMSGACKNOWLEDGEMENT = 0i32;
pub const MQMSG_ACKNOWLEDGMENT_POS_ARRIVAL: MQMSGACKNOWLEDGEMENT = 1i32;
pub const MQMSG_ACKNOWLEDGMENT_POS_RECEIVE: MQMSGACKNOWLEDGEMENT = 2i32;
pub const MQMSG_AUTHENTICATED_QM_MESSAGE: u32 = 11u32;
pub const MQMSG_AUTHENTICATED_SIG10: MQMSGAUTHENTICATION = 1i32;
pub const MQMSG_AUTHENTICATED_SIG20: MQMSGAUTHENTICATION = 3i32;
pub const MQMSG_AUTHENTICATED_SIG30: MQMSGAUTHENTICATION = 5i32;
pub const MQMSG_AUTHENTICATED_SIGXML: MQMSGAUTHENTICATION = 9i32;
pub const MQMSG_AUTHENTICATION_NOT_REQUESTED: MQMSGAUTHENTICATION = 0i32;
pub const MQMSG_AUTHENTICATION_REQUESTED: MQMSGAUTHENTICATION = 1i32;
pub const MQMSG_AUTHENTICATION_REQUESTED_EX: MQMSGAUTHENTICATION = 3i32;
pub const MQMSG_AUTH_LEVEL_ALWAYS: MQMSGAUTHLEVEL = 1i32;
pub const MQMSG_AUTH_LEVEL_MSMQ10: MQMSGAUTHLEVEL = 2i32;
pub const MQMSG_AUTH_LEVEL_MSMQ20: MQMSGAUTHLEVEL = 4i32;
pub const MQMSG_AUTH_LEVEL_NONE: MQMSGAUTHLEVEL = 0i32;
pub const MQMSG_AUTH_LEVEL_SIG10: MQMSGAUTHLEVEL = 2i32;
pub const MQMSG_AUTH_LEVEL_SIG20: MQMSGAUTHLEVEL = 4i32;
pub const MQMSG_AUTH_LEVEL_SIG30: MQMSGAUTHLEVEL = 8i32;
pub const MQMSG_CALG_DES: MQCALG = 26113i32;
pub const MQMSG_CALG_DSS_SIGN: MQCALG = 8704i32;
pub const MQMSG_CALG_MAC: MQCALG = 32773i32;
pub const MQMSG_CALG_MD2: MQCALG = 32769i32;
pub const MQMSG_CALG_MD4: MQCALG = 32770i32;
pub const MQMSG_CALG_MD5: MQCALG = 32771i32;
pub const MQMSG_CALG_RC2: MQCALG = 26114i32;
pub const MQMSG_CALG_RC4: MQCALG = 26625i32;
pub const MQMSG_CALG_RSA_KEYX: MQCALG = 41984i32;
pub const MQMSG_CALG_RSA_SIGN: MQCALG = 9216i32;
pub const MQMSG_CALG_SEAL: MQCALG = 26626i32;
pub const MQMSG_CALG_SHA: MQCALG = 32772i32;
pub const MQMSG_CALG_SHA1: MQCALG = 32772i32;
pub const MQMSG_CLASS_ACK_REACH_QUEUE: MQMSGCLASS = 2i32;
pub const MQMSG_CLASS_ACK_RECEIVE: MQMSGCLASS = 16384i32;
pub const MQMSG_CLASS_NACK_ACCESS_DENIED: MQMSGCLASS = 32772i32;
pub const MQMSG_CLASS_NACK_BAD_DST_Q: MQMSGCLASS = 32768i32;
pub const MQMSG_CLASS_NACK_BAD_ENCRYPTION: MQMSGCLASS = 32775i32;
pub const MQMSG_CLASS_NACK_BAD_SIGNATURE: MQMSGCLASS = 32774i32;
pub const MQMSG_CLASS_NACK_COULD_NOT_ENCRYPT: MQMSGCLASS = 32776i32;
pub const MQMSG_CLASS_NACK_HOP_COUNT_EXCEEDED: MQMSGCLASS = 32773i32;
pub const MQMSG_CLASS_NACK_NOT_TRANSACTIONAL_MSG: MQMSGCLASS = 32778i32;
pub const MQMSG_CLASS_NACK_NOT_TRANSACTIONAL_Q: MQMSGCLASS = 32777i32;
pub const MQMSG_CLASS_NACK_PURGED: MQMSGCLASS = 32769i32;
pub const MQMSG_CLASS_NACK_Q_DELETED: MQMSGCLASS = 49152i32;
pub const MQMSG_CLASS_NACK_Q_EXCEED_QUOTA: MQMSGCLASS = 32771i32;
pub const MQMSG_CLASS_NACK_Q_PURGED: MQMSGCLASS = 49153i32;
pub const MQMSG_CLASS_NACK_REACH_QUEUE_TIMEOUT: MQMSGCLASS = 32770i32;
pub const MQMSG_CLASS_NACK_RECEIVE_TIMEOUT: MQMSGCLASS = 49154i32;
pub const MQMSG_CLASS_NACK_RECEIVE_TIMEOUT_AT_SENDER: MQMSGCLASS = 49155i32;
pub const MQMSG_CLASS_NACK_SOURCE_COMPUTER_GUID_CHANGED: MQMSGCLASS = 32780i32;
pub const MQMSG_CLASS_NACK_UNSUPPORTED_CRYPTO_PROVIDER: MQMSGCLASS = 32779i32;
pub const MQMSG_CLASS_NORMAL: MQMSGCLASS = 0i32;
pub const MQMSG_CLASS_REPORT: MQMSGCLASS = 1i32;
pub const MQMSG_CORRELATIONID_SIZE: MQMSGIDSIZE = 20i32;
pub const MQMSG_CURRENT: MQMSGCURSOR = 1i32;
pub const MQMSG_DEADLETTER: MQMSGJOURNAL = 1i32;
pub const MQMSG_DELIVERY_EXPRESS: MQMSGDELIVERY = 0i32;
pub const MQMSG_DELIVERY_RECOVERABLE: MQMSGDELIVERY = 1i32;
pub const MQMSG_FIRST: MQMSGCURSOR = 0i32;
pub const MQMSG_FIRST_IN_XACT: u32 = 1u32;
pub const MQMSG_JOURNAL: MQMSGJOURNAL = 2i32;
pub const MQMSG_JOURNAL_NONE: MQMSGJOURNAL = 0i32;
pub const MQMSG_LAST_IN_XACT: u32 = 1u32;
pub const MQMSG_MSGID_SIZE: MQMSGIDSIZE = 20i32;
pub const MQMSG_NEXT: MQMSGCURSOR = 2i32;
pub const MQMSG_NOT_FIRST_IN_XACT: u32 = 0u32;
pub const MQMSG_NOT_LAST_IN_XACT: u32 = 0u32;
pub const MQMSG_PRIV_LEVEL_BODY_AES: u32 = 5u32;
pub const MQMSG_PRIV_LEVEL_BODY_BASE: MQMSGPRIVLEVEL = 1i32;
pub const MQMSG_PRIV_LEVEL_BODY_ENHANCED: MQMSGPRIVLEVEL = 3i32;
pub const MQMSG_PRIV_LEVEL_NONE: MQMSGPRIVLEVEL = 0i32;
pub const MQMSG_SENDERID_TYPE_NONE: MQMSGSENDERIDTYPE = 0i32;
pub const MQMSG_SENDERID_TYPE_SID: MQMSGSENDERIDTYPE = 1i32;
pub const MQMSG_SEND_ROUTE_TO_REPORT_QUEUE: MQMSGTRACE = 1i32;
pub const MQMSG_TRACE_NONE: MQMSGTRACE = 0i32;
pub const MQMSG_XACTID_SIZE: MQMSGIDSIZE = 20i32;
pub const MQSEC_CHANGE_QUEUE_PERMISSIONS: MQQUEUEACCESSMASK = 262144u32;
pub const MQSEC_DELETE_JOURNAL_MESSAGE: MQQUEUEACCESSMASK = 8u32;
pub const MQSEC_DELETE_MESSAGE: MQQUEUEACCESSMASK = 1u32;
pub const MQSEC_DELETE_QUEUE: MQQUEUEACCESSMASK = 65536u32;
pub const MQSEC_GET_QUEUE_PERMISSIONS: MQQUEUEACCESSMASK = 131072u32;
pub const MQSEC_GET_QUEUE_PROPERTIES: MQQUEUEACCESSMASK = 32u32;
pub const MQSEC_PEEK_MESSAGE: MQQUEUEACCESSMASK = 2u32;
pub const MQSEC_QUEUE_GENERIC_ALL: MQQUEUEACCESSMASK = 983103u32;
pub const MQSEC_QUEUE_GENERIC_EXECUTE: MQQUEUEACCESSMASK = 0u32;
pub const MQSEC_QUEUE_GENERIC_READ: MQQUEUEACCESSMASK = 131115u32;
pub const MQSEC_QUEUE_GENERIC_WRITE: MQQUEUEACCESSMASK = 131108u32;
pub const MQSEC_RECEIVE_JOURNAL_MESSAGE: MQQUEUEACCESSMASK = 10u32;
pub const MQSEC_RECEIVE_MESSAGE: MQQUEUEACCESSMASK = 3u32;
pub const MQSEC_SET_QUEUE_PROPERTIES: MQQUEUEACCESSMASK = 16u32;
pub const MQSEC_TAKE_QUEUE_OWNERSHIP: MQQUEUEACCESSMASK = 524288u32;
pub const MQSEC_WRITE_MESSAGE: MQQUEUEACCESSMASK = 4u32;
pub const MQ_ACTION_PEEK_CURRENT: u32 = 2147483648u32;
pub const MQ_ACTION_PEEK_NEXT: u32 = 2147483649u32;
pub const MQ_ACTION_RECEIVE: u32 = 0u32;
pub const MQ_ADMIN_ACCESS: MQACCESS = 128i32;
pub const MQ_AUTHENTICATE: MQAUTHENTICATE = 1i32;
pub const MQ_AUTHENTICATE_NONE: MQAUTHENTICATE = 0i32;
pub const MQ_CORRUPTED_QUEUE_WAS_DELETED: MQERROR = -1072824216i32;
pub const MQ_DENY_NONE: MQSHARE = 0i32;
pub const MQ_DENY_RECEIVE_SHARE: MQSHARE = 1i32;
pub const MQ_ERROR: MQERROR = -1072824319i32;
pub const MQ_ERROR_ACCESS_DENIED: MQERROR = -1072824283i32;
pub const MQ_ERROR_BAD_SECURITY_CONTEXT: MQERROR = -1072824267i32;
pub const MQ_ERROR_BAD_XML_FORMAT: MQERROR = -1072824174i32;
pub const MQ_ERROR_BUFFER_OVERFLOW: MQERROR = -1072824294i32;
pub const MQ_ERROR_CANNOT_CREATE_CERT_STORE: MQERROR = -1072824209i32;
pub const MQ_ERROR_CANNOT_CREATE_HASH_EX: MQERROR = -1072824191i32;
pub const MQ_ERROR_CANNOT_CREATE_ON_GC: MQERROR = -1072824201i32;
pub const MQ_ERROR_CANNOT_CREATE_PSC_OBJECTS: MQERROR = -1072824171i32;
pub const MQ_ERROR_CANNOT_DELETE_PSC_OBJECTS: MQERROR = -1072824189i32;
pub const MQ_ERROR_CANNOT_GET_DN: MQERROR = -1072824194i32;
pub const MQ_ERROR_CANNOT_GRANT_ADD_GUID: MQERROR = -1072824206i32;
pub const MQ_ERROR_CANNOT_HASH_DATA_EX: MQERROR = -1072824193i32;
pub const MQ_ERROR_CANNOT_IMPERSONATE_CLIENT: MQERROR = -1072824284i32;
pub const MQ_ERROR_CANNOT_JOIN_DOMAIN: MQERROR = -1072824202i32;
pub const MQ_ERROR_CANNOT_LOAD_MQAD: MQERROR = -1072824187i32;
pub const MQ_ERROR_CANNOT_LOAD_MQDSSRV: MQERROR = -1072824186i32;
pub const MQ_ERROR_CANNOT_LOAD_MSMQOCM: MQERROR = -1072824205i32;
pub const MQ_ERROR_CANNOT_OPEN_CERT_STORE: MQERROR = -1072824208i32;
pub const MQ_ERROR_CANNOT_SET_CRYPTO_SEC_DESCR: MQERROR = -1072824212i32;
pub const MQ_ERROR_CANNOT_SIGN_DATA_EX: MQERROR = -1072824192i32;
pub const MQ_ERROR_CANNOT_UPDATE_PSC_OBJECTS: MQERROR = -1072824170i32;
pub const MQ_ERROR_CANT_CREATE_CERT_STORE: MQERROR = -1072824209i32;
pub const MQ_ERROR_CANT_OPEN_CERT_STORE: MQERROR = -1072824208i32;
pub const MQ_ERROR_CANT_RESOLVE_SITES: MQERROR = -1072824183i32;
pub const MQ_ERROR_CERTIFICATE_NOT_PROVIDED: MQERROR = -1072824211i32;
pub const MQ_ERROR_COMPUTER_DOES_NOT_SUPPORT_ENCRYPTION: MQERROR = -1072824269i32;
pub const MQ_ERROR_CORRUPTED_INTERNAL_CERTIFICATE: MQERROR = -1072824275i32;
pub const MQ_ERROR_CORRUPTED_PERSONAL_CERT_STORE: MQERROR = -1072824271i32;
pub const MQ_ERROR_CORRUPTED_SECURITY_DATA: MQERROR = -1072824272i32;
pub const MQ_ERROR_COULD_NOT_GET_ACCOUNT_INFO: MQERROR = -1072824265i32;
pub const MQ_ERROR_COULD_NOT_GET_USER_SID: MQERROR = -1072824266i32;
pub const MQ_ERROR_DELETE_CN_IN_USE: MQERROR = -1072824248i32;
pub const MQ_ERROR_DEPEND_WKS_LICENSE_OVERFLOW: MQERROR = -1072824217i32;
pub const MQ_ERROR_DS_BIND_ROOT_FOREST: MQERROR = -1072824177i32;
pub const MQ_ERROR_DS_ERROR: MQERROR = -1072824253i32;
pub const MQ_ERROR_DS_IS_FULL: MQERROR = -1072824254i32;
pub const MQ_ERROR_DS_LOCAL_USER: MQERROR = -1072824176i32;
pub const MQ_ERROR_DTC_CONNECT: MQERROR = -1072824244i32;
pub const MQ_ERROR_ENCRYPTION_PROVIDER_NOT_SUPPORTED: MQERROR = -1072824213i32;
pub const MQ_ERROR_FAIL_VERIFY_SIGNATURE_EX: MQERROR = -1072824190i32;
pub const MQ_ERROR_FORMATNAME_BUFFER_TOO_SMALL: MQERROR = -1072824289i32;
pub const MQ_ERROR_GC_NEEDED: MQERROR = -1072824178i32;
pub const MQ_ERROR_GUID_NOT_MATCHING: MQERROR = -1072824200i32;
pub const MQ_ERROR_ILLEGAL_CONTEXT: MQERROR = -1072824229i32;
pub const MQ_ERROR_ILLEGAL_CURSOR_ACTION: MQERROR = -1072824292i32;
pub const MQ_ERROR_ILLEGAL_ENTERPRISE_OPERATION: MQERROR = -1072824207i32;
pub const MQ_ERROR_ILLEGAL_FORMATNAME: MQERROR = -1072824290i32;
pub const MQ_ERROR_ILLEGAL_MQCOLUMNS: MQERROR = -1072824264i32;
pub const MQ_ERROR_ILLEGAL_MQPRIVATEPROPS: MQERROR = -1072824197i32;
pub const MQ_ERROR_ILLEGAL_MQQMPROPS: MQERROR = -1072824255i32;
pub const MQ_ERROR_ILLEGAL_MQQUEUEPROPS: MQERROR = -1072824259i32;
pub const MQ_ERROR_ILLEGAL_OPERATION: MQERROR = -1072824220i32;
pub const MQ_ERROR_ILLEGAL_PROPERTY_SIZE: MQERROR = -1072824261i32;
pub const MQ_ERROR_ILLEGAL_PROPERTY_VALUE: MQERROR = -1072824296i32;
pub const MQ_ERROR_ILLEGAL_PROPERTY_VT: MQERROR = -1072824295i32;
pub const MQ_ERROR_ILLEGAL_PROPID: MQERROR = -1072824263i32;
pub const MQ_ERROR_ILLEGAL_QUEUE_PATHNAME: MQERROR = -1072824300i32;
pub const MQ_ERROR_ILLEGAL_RELATION: MQERROR = -1072824262i32;
pub const MQ_ERROR_ILLEGAL_RESTRICTION_PROPID: MQERROR = -1072824260i32;
pub const MQ_ERROR_ILLEGAL_SECURITY_DESCRIPTOR: MQERROR = -1072824287i32;
pub const MQ_ERROR_ILLEGAL_SORT: MQERROR = -1072824304i32;
pub const MQ_ERROR_ILLEGAL_SORT_PROPID: MQERROR = -1072824228i32;
pub const MQ_ERROR_ILLEGAL_USER: MQERROR = -1072824303i32;
pub const MQ_ERROR_INSUFFICIENT_PROPERTIES: MQERROR = -1072824257i32;
pub const MQ_ERROR_INSUFFICIENT_RESOURCES: MQERROR = -1072824281i32;
pub const MQ_ERROR_INTERNAL_USER_CERT_EXIST: MQERROR = -1072824274i32;
pub const MQ_ERROR_INVALID_CERTIFICATE: MQERROR = -1072824276i32;
pub const MQ_ERROR_INVALID_HANDLE: MQERROR = -1072824313i32;
pub const MQ_ERROR_INVALID_OWNER: MQERROR = -1072824252i32;
pub const MQ_ERROR_INVALID_PARAMETER: MQERROR = -1072824314i32;
pub const MQ_ERROR_IO_TIMEOUT: MQERROR = -1072824293i32;
pub const MQ_ERROR_LABEL_BUFFER_TOO_SMALL: MQERROR = -1072824226i32;
pub const MQ_ERROR_LABEL_TOO_LONG: MQERROR = -1072824227i32;
pub const MQ_ERROR_MACHINE_EXISTS: MQERROR = -1072824256i32;
pub const MQ_ERROR_MACHINE_NOT_FOUND: MQERROR = -1072824307i32;
pub const MQ_ERROR_MESSAGE_ALREADY_RECEIVED: MQERROR = -1072824291i32;
pub const MQ_ERROR_MESSAGE_LOCKED_UNDER_TRANSACTION: ::windows_sys::core::HRESULT = -1072824164i32;
pub const MQ_ERROR_MESSAGE_NOT_AUTHENTICATED: ::windows_sys::core::HRESULT = -1072824165i32;
pub const MQ_ERROR_MESSAGE_NOT_FOUND: MQERROR = -1072824184i32;
pub const MQ_ERROR_MESSAGE_STORAGE_FAILED: MQERROR = -1072824278i32;
pub const MQ_ERROR_MISSING_CONNECTOR_TYPE: MQERROR = -1072824235i32;
pub const MQ_ERROR_MQIS_READONLY_MODE: MQERROR = -1072824224i32;
pub const MQ_ERROR_MQIS_SERVER_EMPTY: MQERROR = -1072824225i32;
pub const MQ_ERROR_MULTI_SORT_KEYS: MQERROR = -1072824179i32;
pub const MQ_ERROR_NOT_A_CORRECT_OBJECT_CLASS: MQERROR = -1072824180i32;
pub const MQ_ERROR_NOT_SUPPORTED_BY_DEPENDENT_CLIENTS: MQERROR = -1072824182i32;
pub const MQ_ERROR_NO_DS: MQERROR = -1072824301i32;
pub const MQ_ERROR_NO_ENTRY_POINT_MSMQOCM: MQERROR = -1072824204i32;
pub const MQ_ERROR_NO_GC_IN_DOMAIN: MQERROR = -1072824196i32;
pub const MQ_ERROR_NO_INTERNAL_USER_CERT: MQERROR = -1072824273i32;
pub const MQ_ERROR_NO_MQUSER_OU: MQERROR = -1072824188i32;
pub const MQ_ERROR_NO_MSMQ_SERVERS_ON_DC: MQERROR = -1072824203i32;
pub const MQ_ERROR_NO_MSMQ_SERVERS_ON_GC: MQERROR = -1072824195i32;
pub const MQ_ERROR_NO_RESPONSE_FROM_OBJECT_SERVER: MQERROR = -1072824247i32;
pub const MQ_ERROR_OBJECT_SERVER_NOT_AVAILABLE: MQERROR = -1072824246i32;
pub const MQ_ERROR_OPERATION_CANCELLED: MQERROR = -1072824312i32;
pub const MQ_ERROR_OPERATION_NOT_SUPPORTED_BY_REMOTE_COMPUTER: MQERROR = -1072824181i32;
pub const MQ_ERROR_PRIVILEGE_NOT_HELD: MQERROR = -1072824282i32;
pub const MQ_ERROR_PROPERTIES_CONFLICT: MQERROR = -1072824185i32;
pub const MQ_ERROR_PROPERTY: MQERROR = -1072824318i32;
pub const MQ_ERROR_PROPERTY_NOTALLOWED: MQERROR = -1072824258i32;
pub const MQ_ERROR_PROV_NAME_BUFFER_TOO_SMALL: MQERROR = -1072824221i32;
pub const MQ_ERROR_PUBLIC_KEY_DOES_NOT_EXIST: MQERROR = -1072824198i32;
pub const MQ_ERROR_PUBLIC_KEY_NOT_FOUND: MQERROR = -1072824199i32;
pub const MQ_ERROR_QUEUE_DELETED: MQERROR = -1072824230i32;
pub const MQ_ERROR_QUEUE_EXISTS: MQERROR = -1072824315i32;
pub const MQ_ERROR_QUEUE_NOT_ACTIVE: MQERROR = -1072824316i32;
pub const MQ_ERROR_QUEUE_NOT_AVAILABLE: MQERROR = -1072824245i32;
pub const MQ_ERROR_QUEUE_NOT_FOUND: MQERROR = -1072824317i32;
pub const MQ_ERROR_Q_ADS_PROPERTY_NOT_SUPPORTED: MQERROR = -1072824175i32;
pub const MQ_ERROR_Q_DNS_PROPERTY_NOT_SUPPORTED: MQERROR = -1072824210i32;
pub const MQ_ERROR_REMOTE_MACHINE_NOT_AVAILABLE: MQERROR = -1072824215i32;
pub const MQ_ERROR_RESOLVE_ADDRESS: ::windows_sys::core::HRESULT = -1072824167i32;
pub const MQ_ERROR_RESULT_BUFFER_TOO_SMALL: MQERROR = -1072824250i32;
pub const MQ_ERROR_SECURITY_DESCRIPTOR_TOO_SMALL: MQERROR = -1072824285i32;
pub const MQ_ERROR_SENDERID_BUFFER_TOO_SMALL: MQERROR = -1072824286i32;
pub const MQ_ERROR_SENDER_CERT_BUFFER_TOO_SMALL: MQERROR = -1072824277i32;
pub const MQ_ERROR_SERVICE_NOT_AVAILABLE: MQERROR = -1072824309i32;
pub const MQ_ERROR_SHARING_VIOLATION: MQERROR = -1072824311i32;
pub const MQ_ERROR_SIGNATURE_BUFFER_TOO_SMALL: MQERROR = -1072824222i32;
pub const MQ_ERROR_STALE_HANDLE: MQERROR = -1072824234i32;
pub const MQ_ERROR_SYMM_KEY_BUFFER_TOO_SMALL: MQERROR = -1072824223i32;
pub const MQ_ERROR_TOO_MANY_PROPERTIES: ::windows_sys::core::HRESULT = -1072824166i32;
pub const MQ_ERROR_TRANSACTION_ENLIST: MQERROR = -1072824232i32;
pub const MQ_ERROR_TRANSACTION_IMPORT: MQERROR = -1072824242i32;
pub const MQ_ERROR_TRANSACTION_SEQUENCE: MQERROR = -1072824239i32;
pub const MQ_ERROR_TRANSACTION_USAGE: MQERROR = -1072824240i32;
pub const MQ_ERROR_UNINITIALIZED_OBJECT: MQERROR = -1072824172i32;
pub const MQ_ERROR_UNSUPPORTED_ACCESS_MODE: MQERROR = -1072824251i32;
pub const MQ_ERROR_UNSUPPORTED_CLASS: MQERROR = -1072824173i32;
pub const MQ_ERROR_UNSUPPORTED_FORMATNAME_OPERATION: MQERROR = -1072824288i32;
pub const MQ_ERROR_UNSUPPORTED_OPERATION: MQERROR = -1072824214i32;
pub const MQ_ERROR_USER_BUFFER_TOO_SMALL: MQERROR = -1072824280i32;
pub const MQ_ERROR_WKS_CANT_SERVE_CLIENT: MQERROR = -1072824218i32;
pub const MQ_ERROR_WRITE_NOT_ALLOWED: MQERROR = -1072824219i32;
pub const MQ_INFORMATION_DUPLICATE_PROPERTY: MQWARNING = 1074659333i32;
pub const MQ_INFORMATION_FORMATNAME_BUFFER_TOO_SMALL: MQWARNING = 1074659337i32;
pub const MQ_INFORMATION_ILLEGAL_PROPERTY: MQWARNING = 1074659330i32;
pub const MQ_INFORMATION_INTERNAL_USER_CERT_EXIST: MQWARNING = 1074659338i32;
pub const MQ_INFORMATION_OPERATION_PENDING: MQWARNING = 1074659334i32;
pub const MQ_INFORMATION_OWNER_IGNORED: MQWARNING = 1074659339i32;
pub const MQ_INFORMATION_PROPERTY: MQWARNING = 1074659329i32;
pub const MQ_INFORMATION_PROPERTY_IGNORED: MQWARNING = 1074659331i32;
pub const MQ_INFORMATION_UNSUPPORTED_PROPERTY: MQWARNING = 1074659332i32;
pub const MQ_JOURNAL: MQJOURNAL = 1i32;
pub const MQ_JOURNAL_NONE: MQJOURNAL = 0i32;
pub const MQ_LOOKUP_PEEK_CURRENT: u32 = 1073741840u32;
pub const MQ_LOOKUP_PEEK_FIRST: u32 = 1073741844u32;
pub const MQ_LOOKUP_PEEK_LAST: u32 = 1073741848u32;
pub const MQ_LOOKUP_PEEK_NEXT: u32 = 1073741841u32;
pub const MQ_LOOKUP_PEEK_PREV: u32 = 1073741842u32;
pub const MQ_LOOKUP_RECEIVE_ALLOW_PEEK: u32 = 1073742112u32;
pub const MQ_LOOKUP_RECEIVE_CURRENT: u32 = 1073741856u32;
pub const MQ_LOOKUP_RECEIVE_FIRST: u32 = 1073741860u32;
pub const MQ_LOOKUP_RECEIVE_LAST: u32 = 1073741864u32;
pub const MQ_LOOKUP_RECEIVE_NEXT: u32 = 1073741857u32;
pub const MQ_LOOKUP_RECEIVE_PREV: u32 = 1073741858u32;
pub const MQ_MAX_MSG_LABEL_LEN: MQMSGMAX = 249i32;
pub const MQ_MAX_PRIORITY: MQPRIORITY = 7i32;
pub const MQ_MAX_Q_LABEL_LEN: MQMAX = 124i32;
pub const MQ_MAX_Q_NAME_LEN: MQMAX = 124i32;
pub const MQ_MIN_PRIORITY: MQPRIORITY = 0i32;
pub const MQ_MOVE_ACCESS: u32 = 4u32;
pub const MQ_MTS_TRANSACTION: MQTRANSACTION = 1i32;
pub const MQ_NO_TRANSACTION: MQTRANSACTION = 0i32;
pub const MQ_OK: ::windows_sys::core::HRESULT = 0i32;
pub const MQ_PEEK_ACCESS: MQACCESS = 32i32;
pub const MQ_PRIV_LEVEL_BODY: MQPRIVLEVEL = 2i32;
pub const MQ_PRIV_LEVEL_NONE: MQPRIVLEVEL = 0i32;
pub const MQ_PRIV_LEVEL_OPTIONAL: MQPRIVLEVEL = 1i32;
pub const MQ_QTYPE_REPORT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x55ee8f32_cce9_11cf_b108_0020afd61ce9);
pub const MQ_QTYPE_TEST: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x55ee8f33_cce9_11cf_b108_0020afd61ce9);
pub const MQ_QUEUE_STATE_CONNECTED: QUEUE_STATE = 6i32;
pub const MQ_QUEUE_STATE_DISCONNECTED: QUEUE_STATE = 1i32;
pub const MQ_QUEUE_STATE_DISCONNECTING: QUEUE_STATE = 7i32;
pub const MQ_QUEUE_STATE_LOCAL_CONNECTION: QUEUE_STATE = 0i32;
pub const MQ_QUEUE_STATE_LOCKED: QUEUE_STATE = 8i32;
pub const MQ_QUEUE_STATE_NEEDVALIDATE: QUEUE_STATE = 3i32;
pub const MQ_QUEUE_STATE_NONACTIVE: QUEUE_STATE = 5i32;
pub const MQ_QUEUE_STATE_ONHOLD: QUEUE_STATE = 4i32;
pub const MQ_QUEUE_STATE_WAITING: QUEUE_STATE = 2i32;
pub const MQ_RECEIVE_ACCESS: MQACCESS = 1i32;
pub const MQ_SEND_ACCESS: MQACCESS = 2i32;
pub const MQ_SINGLE_MESSAGE: MQTRANSACTION = 3i32;
pub const MQ_STATUS_FOREIGN: FOREIGN_STATUS = 0i32;
pub const MQ_STATUS_NOT_FOREIGN: FOREIGN_STATUS = 1i32;
pub const MQ_STATUS_UNKNOWN: FOREIGN_STATUS = 2i32;
pub const MQ_TRANSACTIONAL: MQTRANSACTIONAL = 1i32;
pub const MQ_TRANSACTIONAL_NONE: MQTRANSACTIONAL = 0i32;
pub const MQ_TYPE_CONNECTOR: QUEUE_TYPE = 3i32;
pub const MQ_TYPE_MACHINE: QUEUE_TYPE = 2i32;
pub const MQ_TYPE_MULTICAST: QUEUE_TYPE = 4i32;
pub const MQ_TYPE_PRIVATE: QUEUE_TYPE = 1i32;
pub const MQ_TYPE_PUBLIC: QUEUE_TYPE = 0i32;
pub const MQ_XACT_STATUS_NOT_XACT: XACT_STATUS = 1i32;
pub const MQ_XACT_STATUS_UNKNOWN: XACT_STATUS = 2i32;
pub const MQ_XACT_STATUS_XACT: XACT_STATUS = 0i32;
pub const MQ_XA_TRANSACTION: MQTRANSACTION = 2i32;
pub const MSMQApplication: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e086_dccd_11d0_aa4b_0060970debae);
pub const MSMQCollection: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf72b9031_2f0c_43e8_924e_e6052cdc493f);
pub const MSMQCoordinatedTransactionDispenser: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e082_dccd_11d0_aa4b_0060970debae);
pub const MSMQDestination: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeba96b18_2168_11d3_898c_00e02c074f6b);
pub const MSMQEvent: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e07a_dccd_11d0_aa4b_0060970debae);
pub const MSMQManagement: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x39ce96fe_f4c5_4484_a143_4c2d5d324229);
pub const MSMQMessage: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e075_dccd_11d0_aa4b_0060970debae);
pub const MSMQOutgoingQueueManagement: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0188401c_247a_4fed_99c6_bf14119d7055);
pub const MSMQQuery: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e073_dccd_11d0_aa4b_0060970debae);
pub const MSMQQueue: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e079_dccd_11d0_aa4b_0060970debae);
pub const MSMQQueueInfo: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e07c_dccd_11d0_aa4b_0060970debae);
pub const MSMQQueueInfos: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e07e_dccd_11d0_aa4b_0060970debae);
pub const MSMQQueueManagement: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x33b6d07e_f27d_42fa_b2d7_bf82e11e9374);
pub const MSMQTransaction: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e080_dccd_11d0_aa4b_0060970debae);
pub const MSMQTransactionDispenser: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7d6e084_dccd_11d0_aa4b_0060970debae);
pub const MSMQ_CONNECTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CONNECTED");
pub const MSMQ_DISCONNECTED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DISCONNECTED");
pub const PREQ: u32 = 4u32;
pub const PRGE: u32 = 3u32;
pub const PRGT: u32 = 2u32;
pub const PRLE: u32 = 1u32;
pub const PRLT: u32 = 0u32;
pub const PRNE: u32 = 5u32;
pub const PROPID_MGMT_MSMQ_ACTIVEQUEUES: u32 = 1u32;
pub const PROPID_MGMT_MSMQ_BASE: u32 = 0u32;
pub const PROPID_MGMT_MSMQ_BYTES_IN_ALL_QUEUES: u32 = 6u32;
pub const PROPID_MGMT_MSMQ_CONNECTED: u32 = 4u32;
pub const PROPID_MGMT_MSMQ_DSSERVER: u32 = 3u32;
pub const PROPID_MGMT_MSMQ_PRIVATEQ: u32 = 2u32;
pub const PROPID_MGMT_MSMQ_TYPE: u32 = 5u32;
pub const PROPID_MGMT_QUEUE_BASE: u32 = 0u32;
pub const PROPID_MGMT_QUEUE_BYTES_IN_JOURNAL: u32 = 10u32;
pub const PROPID_MGMT_QUEUE_BYTES_IN_QUEUE: u32 = 8u32;
pub const PROPID_MGMT_QUEUE_CONNECTION_HISTORY: u32 = 25u32;
pub const PROPID_MGMT_QUEUE_EOD_FIRST_NON_ACK: u32 = 16u32;
pub const PROPID_MGMT_QUEUE_EOD_LAST_ACK: u32 = 13u32;
pub const PROPID_MGMT_QUEUE_EOD_LAST_ACK_COUNT: u32 = 15u32;
pub const PROPID_MGMT_QUEUE_EOD_LAST_ACK_TIME: u32 = 14u32;
pub const PROPID_MGMT_QUEUE_EOD_LAST_NON_ACK: u32 = 17u32;
pub const PROPID_MGMT_QUEUE_EOD_NEXT_SEQ: u32 = 18u32;
pub const PROPID_MGMT_QUEUE_EOD_NO_ACK_COUNT: u32 = 20u32;
pub const PROPID_MGMT_QUEUE_EOD_NO_READ_COUNT: u32 = 19u32;
pub const PROPID_MGMT_QUEUE_EOD_RESEND_COUNT: u32 = 23u32;
pub const PROPID_MGMT_QUEUE_EOD_RESEND_INTERVAL: u32 = 22u32;
pub const PROPID_MGMT_QUEUE_EOD_RESEND_TIME: u32 = 21u32;
pub const PROPID_MGMT_QUEUE_EOD_SOURCE_INFO: u32 = 24u32;
pub const PROPID_MGMT_QUEUE_FOREIGN: u32 = 6u32;
pub const PROPID_MGMT_QUEUE_FORMATNAME: u32 = 2u32;
pub const PROPID_MGMT_QUEUE_JOURNAL_MESSAGE_COUNT: u32 = 9u32;
pub const PROPID_MGMT_QUEUE_JOURNAL_USED_QUOTA: u32 = 10u32;
pub const PROPID_MGMT_QUEUE_LOCATION: u32 = 4u32;
pub const PROPID_MGMT_QUEUE_MESSAGE_COUNT: u32 = 7u32;
pub const PROPID_MGMT_QUEUE_NEXTHOPS: u32 = 12u32;
pub const PROPID_MGMT_QUEUE_PATHNAME: u32 = 1u32;
pub const PROPID_MGMT_QUEUE_STATE: u32 = 11u32;
pub const PROPID_MGMT_QUEUE_SUBQUEUE_COUNT: u32 = 26u32;
pub const PROPID_MGMT_QUEUE_SUBQUEUE_NAMES: u32 = 27u32;
pub const PROPID_MGMT_QUEUE_TYPE: u32 = 3u32;
pub const PROPID_MGMT_QUEUE_USED_QUOTA: u32 = 8u32;
pub const PROPID_MGMT_QUEUE_XACT: u32 = 5u32;
pub const PROPID_M_ABORT_COUNT: u32 = 69u32;
pub const PROPID_M_ACKNOWLEDGE: u32 = 6u32;
pub const PROPID_M_ADMIN_QUEUE: u32 = 17u32;
pub const PROPID_M_ADMIN_QUEUE_LEN: u32 = 18u32;
pub const PROPID_M_APPSPECIFIC: u32 = 8u32;
pub const PROPID_M_ARRIVEDTIME: u32 = 32u32;
pub const PROPID_M_AUTHENTICATED: u32 = 25u32;
pub const PROPID_M_AUTHENTICATED_EX: u32 = 53u32;
pub const PROPID_M_AUTH_LEVEL: u32 = 24u32;
pub const PROPID_M_BASE: u32 = 0u32;
pub const PROPID_M_BODY: u32 = 9u32;
pub const PROPID_M_BODY_SIZE: u32 = 10u32;
pub const PROPID_M_BODY_TYPE: u32 = 42u32;
pub const PROPID_M_CLASS: u32 = 1u32;
pub const PROPID_M_COMPOUND_MESSAGE: u32 = 63u32;
pub const PROPID_M_COMPOUND_MESSAGE_SIZE: u32 = 64u32;
pub const PROPID_M_CONNECTOR_TYPE: u32 = 38u32;
pub const PROPID_M_CORRELATIONID: u32 = 3u32;
pub const PROPID_M_CORRELATIONID_SIZE: u32 = 20u32;
pub const PROPID_M_DEADLETTER_QUEUE: u32 = 67u32;
pub const PROPID_M_DEADLETTER_QUEUE_LEN: u32 = 68u32;
pub const PROPID_M_DELIVERY: u32 = 5u32;
pub const PROPID_M_DEST_FORMAT_NAME: u32 = 58u32;
pub const PROPID_M_DEST_FORMAT_NAME_LEN: u32 = 59u32;
pub const PROPID_M_DEST_QUEUE: u32 = 33u32;
pub const PROPID_M_DEST_QUEUE_LEN: u32 = 34u32;
pub const PROPID_M_DEST_SYMM_KEY: u32 = 43u32;
pub const PROPID_M_DEST_SYMM_KEY_LEN: u32 = 44u32;
pub const PROPID_M_ENCRYPTION_ALG: u32 = 27u32;
pub const PROPID_M_EXTENSION: u32 = 35u32;
pub const PROPID_M_EXTENSION_LEN: u32 = 36u32;
pub const PROPID_M_FIRST_IN_XACT: u32 = 50u32;
pub const PROPID_M_HASH_ALG: u32 = 26u32;
pub const PROPID_M_JOURNAL: u32 = 7u32;
pub const PROPID_M_LABEL: u32 = 11u32;
pub const PROPID_M_LABEL_LEN: u32 = 12u32;
pub const PROPID_M_LAST_IN_XACT: u32 = 51u32;
pub const PROPID_M_LAST_MOVE_TIME: u32 = 75u32;
pub const PROPID_M_LOOKUPID: u32 = 60u32;
pub const PROPID_M_MOVE_COUNT: u32 = 70u32;
pub const PROPID_M_MSGID: u32 = 2u32;
pub const PROPID_M_MSGID_SIZE: u32 = 20u32;
pub const PROPID_M_PRIORITY: u32 = 4u32;
pub const PROPID_M_PRIV_LEVEL: u32 = 23u32;
pub const PROPID_M_PROV_NAME: u32 = 48u32;
pub const PROPID_M_PROV_NAME_LEN: u32 = 49u32;
pub const PROPID_M_PROV_TYPE: u32 = 47u32;
pub const PROPID_M_RESP_FORMAT_NAME: u32 = 54u32;
pub const PROPID_M_RESP_FORMAT_NAME_LEN: u32 = 55u32;
pub const PROPID_M_RESP_QUEUE: u32 = 15u32;
pub const PROPID_M_RESP_QUEUE_LEN: u32 = 16u32;
pub const PROPID_M_SECURITY_CONTEXT: u32 = 37u32;
pub const PROPID_M_SENDERID: u32 = 20u32;
pub const PROPID_M_SENDERID_LEN: u32 = 21u32;
pub const PROPID_M_SENDERID_TYPE: u32 = 22u32;
pub const PROPID_M_SENDER_CERT: u32 = 28u32;
pub const PROPID_M_SENDER_CERT_LEN: u32 = 29u32;
pub const PROPID_M_SENTTIME: u32 = 31u32;
pub const PROPID_M_SIGNATURE: u32 = 45u32;
pub const PROPID_M_SIGNATURE_LEN: u32 = 46u32;
pub const PROPID_M_SOAP_BODY: u32 = 66u32;
pub const PROPID_M_SOAP_ENVELOPE: u32 = 61u32;
pub const PROPID_M_SOAP_ENVELOPE_LEN: u32 = 62u32;
pub const PROPID_M_SOAP_HEADER: u32 = 65u32;
pub const PROPID_M_SRC_MACHINE_ID: u32 = 30u32;
pub const PROPID_M_TIME_TO_BE_RECEIVED: u32 = 14u32;
pub const PROPID_M_TIME_TO_REACH_QUEUE: u32 = 13u32;
pub const PROPID_M_TRACE: u32 = 41u32;
pub const PROPID_M_VERSION: u32 = 19u32;
pub const PROPID_M_XACTID: u32 = 52u32;
pub const PROPID_M_XACTID_SIZE: u32 = 20u32;
pub const PROPID_M_XACT_STATUS_QUEUE: u32 = 39u32;
pub const PROPID_M_XACT_STATUS_QUEUE_LEN: u32 = 40u32;
pub const PROPID_PC_BASE: u32 = 5800u32;
pub const PROPID_PC_DS_ENABLED: u32 = 5802u32;
pub const PROPID_PC_VERSION: u32 = 5801u32;
pub const PROPID_QM_BASE: u32 = 200u32;
pub const PROPID_QM_CONNECTION: u32 = 204u32;
pub const PROPID_QM_ENCRYPTION_PK: u32 = 205u32;
pub const PROPID_QM_ENCRYPTION_PK_AES: u32 = 244u32;
pub const PROPID_QM_ENCRYPTION_PK_BASE: u32 = 231u32;
pub const PROPID_QM_ENCRYPTION_PK_ENHANCED: u32 = 232u32;
pub const PROPID_QM_MACHINE_ID: u32 = 202u32;
pub const PROPID_QM_PATHNAME: u32 = 203u32;
pub const PROPID_QM_PATHNAME_DNS: u32 = 233u32;
pub const PROPID_QM_SITE_ID: u32 = 201u32;
pub const PROPID_Q_ADS_PATH: u32 = 126u32;
pub const PROPID_Q_AUTHENTICATE: u32 = 111u32;
pub const PROPID_Q_BASE: u32 = 100u32;
pub const PROPID_Q_BASEPRIORITY: u32 = 106u32;
pub const PROPID_Q_CREATE_TIME: u32 = 109u32;
pub const PROPID_Q_INSTANCE: u32 = 101u32;
pub const PROPID_Q_JOURNAL: u32 = 104u32;
pub const PROPID_Q_JOURNAL_QUOTA: u32 = 107u32;
pub const PROPID_Q_LABEL: u32 = 108u32;
pub const PROPID_Q_MODIFY_TIME: u32 = 110u32;
pub const PROPID_Q_MULTICAST_ADDRESS: u32 = 125u32;
pub const PROPID_Q_PATHNAME: u32 = 103u32;
pub const PROPID_Q_PATHNAME_DNS: u32 = 124u32;
pub const PROPID_Q_PRIV_LEVEL: u32 = 112u32;
pub const PROPID_Q_QUOTA: u32 = 105u32;
pub const PROPID_Q_TRANSACTION: u32 = 113u32;
pub const PROPID_Q_TYPE: u32 = 102u32;
pub const QUERY_SORTASCEND: u32 = 0u32;
pub const QUERY_SORTDESCEND: u32 = 1u32;
pub const QUEUE_ACTION_EOD_RESEND: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EOD_RESEND");
pub const QUEUE_ACTION_PAUSE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PAUSE");
pub const QUEUE_ACTION_RESUME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RESUME");
pub const REL_EQ: RELOPS = 1i32;
pub const REL_GE: RELOPS = 6i32;
pub const REL_GT: RELOPS = 4i32;
pub const REL_LE: RELOPS = 5i32;
pub const REL_LT: RELOPS = 3i32;
pub const REL_NEQ: RELOPS = 2i32;
pub const REL_NOP: RELOPS = 0i32;
pub type FOREIGN_STATUS = i32;
pub type MQACCESS = i32;
pub type MQAUTHENTICATE = i32;
pub type MQCALG = i32;
pub type MQCERT_REGISTER = i32;
pub type MQConnectionState = i32;
pub type MQDEFAULT = i32;
pub type MQERROR = i32;
pub type MQJOURNAL = i32;
pub type MQMAX = i32;
pub type MQMSGACKNOWLEDGEMENT = i32;
pub type MQMSGAUTHENTICATION = i32;
pub type MQMSGAUTHLEVEL = i32;
pub type MQMSGCLASS = i32;
pub type MQMSGCURSOR = i32;
pub type MQMSGDELIVERY = i32;
pub type MQMSGIDSIZE = i32;
pub type MQMSGJOURNAL = i32;
pub type MQMSGMAX = i32;
pub type MQMSGPRIVLEVEL = i32;
pub type MQMSGSENDERIDTYPE = i32;
pub type MQMSGTRACE = i32;
pub type MQPRIORITY = i32;
pub type MQPRIVLEVEL = i32;
pub type MQQUEUEACCESSMASK = u32;
pub type MQSHARE = i32;
pub type MQTRANSACTION = i32;
pub type MQTRANSACTIONAL = i32;
pub type MQWARNING = i32;
pub type QUEUE_STATE = i32;
pub type QUEUE_TYPE = i32;
pub type RELOPS = i32;
pub type XACT_STATUS = i32;
#[repr(C)]
pub struct MQCOLUMNSET {
    pub cCol: u32,
    pub aCol: *mut u32,
}
impl ::core::marker::Copy for MQCOLUMNSET {}
impl ::core::clone::Clone for MQCOLUMNSET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQMGMTPROPS {
    pub cProp: u32,
    pub aPropID: *mut u32,
    pub aPropVar: *mut super::Com::StructuredStorage::PROPVARIANT,
    pub aStatus: *mut ::windows_sys::core::HRESULT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQMGMTPROPS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQMGMTPROPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQMSGPROPS {
    pub cProp: u32,
    pub aPropID: *mut u32,
    pub aPropVar: *mut super::Com::StructuredStorage::PROPVARIANT,
    pub aStatus: *mut ::windows_sys::core::HRESULT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQMSGPROPS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQMSGPROPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQPRIVATEPROPS {
    pub cProp: u32,
    pub aPropID: *mut u32,
    pub aPropVar: *mut super::Com::StructuredStorage::PROPVARIANT,
    pub aStatus: *mut ::windows_sys::core::HRESULT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQPRIVATEPROPS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQPRIVATEPROPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQPROPERTYRESTRICTION {
    pub rel: u32,
    pub prop: u32,
    pub prval: super::Com::StructuredStorage::PROPVARIANT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQPROPERTYRESTRICTION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQPROPERTYRESTRICTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQQMPROPS {
    pub cProp: u32,
    pub aPropID: *mut u32,
    pub aPropVar: *mut super::Com::StructuredStorage::PROPVARIANT,
    pub aStatus: *mut ::windows_sys::core::HRESULT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQQMPROPS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQQMPROPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQQUEUEPROPS {
    pub cProp: u32,
    pub aPropID: *mut u32,
    pub aPropVar: *mut super::Com::StructuredStorage::PROPVARIANT,
    pub aStatus: *mut ::windows_sys::core::HRESULT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQQUEUEPROPS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQQUEUEPROPS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
pub struct MQRESTRICTION {
    pub cRes: u32,
    pub paPropRes: *mut MQPROPERTYRESTRICTION,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for MQRESTRICTION {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for MQRESTRICTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MQSORTKEY {
    pub propColumn: u32,
    pub dwOrder: u32,
}
impl ::core::marker::Copy for MQSORTKEY {}
impl ::core::clone::Clone for MQSORTKEY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MQSORTSET {
    pub cCol: u32,
    pub aCol: *mut MQSORTKEY,
}
impl ::core::marker::Copy for MQSORTSET {}
impl ::core::clone::Clone for MQSORTSET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEQUENCE_INFO {
    pub SeqID: i64,
    pub SeqNo: u32,
    pub PrevNo: u32,
}
impl ::core::marker::Copy for SEQUENCE_INFO {}
impl ::core::clone::Clone for SEQUENCE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_IO\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_IO", feature = "Win32_System_Variant"))]
pub type PMQRECEIVECALLBACK = ::core::option::Option<unsafe extern "system" fn(hrstatus: ::windows_sys::core::HRESULT, hsource: isize, dwtimeout: u32, dwaction: u32, pmessageprops: *mut MQMSGPROPS, lpoverlapped: *mut super::IO::OVERLAPPED, hcursor: super::super::Foundation::HANDLE) -> ()>;
