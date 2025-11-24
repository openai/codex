::windows_targets::link!("rpcrt4.dll" "system" fn DceErrorInqTextA(rpcstatus : RPC_STATUS, errortext : ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn DceErrorInqTextW(rpcstatus : RPC_STATUS, errortext : ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn IUnknown_AddRef_Proxy(this : ::windows_sys::core::IUnknown) -> u32);
::windows_targets::link!("rpcrt4.dll" "system" fn IUnknown_QueryInterface_Proxy(this : ::windows_sys::core::IUnknown, riid : *const ::windows_sys::core::GUID, ppvobject : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("rpcrt4.dll" "system" fn IUnknown_Release_Proxy(this : ::windows_sys::core::IUnknown) -> u32);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcAllocate(size : u32) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn I_RpcAsyncAbortCall(pasync : *const RPC_ASYNC_STATE, exceptioncode : u32) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn I_RpcAsyncSetHandle(message : *const RPC_MESSAGE, pasync : *const RPC_ASYNC_STATE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingCopy(sourcebinding : *mut ::core::ffi::c_void, destinationbinding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingCreateNP(servername : ::windows_sys::core::PCWSTR, servicename : ::windows_sys::core::PCWSTR, networkoptions : ::windows_sys::core::PCWSTR, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingHandleToAsyncHandle(binding : *mut ::core::ffi::c_void, asynchandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn I_RpcBindingInqClientTokenAttributes(binding : *const ::core::ffi::c_void, tokenid : *mut super::super::Foundation:: LUID, authenticationid : *mut super::super::Foundation:: LUID, modifiedid : *mut super::super::Foundation:: LUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqDynamicEndpointA(binding : *const ::core::ffi::c_void, dynamicendpoint : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqDynamicEndpointW(binding : *const ::core::ffi::c_void, dynamicendpoint : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqLocalClientPID(binding : *mut ::core::ffi::c_void, pid : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqMarshalledTargetInfo(binding : *const ::core::ffi::c_void, marshalledtargetinfosize : *mut u32, marshalledtargetinfo : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqSecurityContext(binding : *mut ::core::ffi::c_void, securitycontexthandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqSecurityContextKeyInfo(binding : *const ::core::ffi::c_void, keyinfo : *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqTransportType(binding : *mut ::core::ffi::c_void, r#type : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingInqWireIdForSnego(binding : *const ::core::ffi::c_void, wireid : *mut u8) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingIsClientLocal(bindinghandle : *mut ::core::ffi::c_void, clientlocalflag : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingIsServerLocal(binding : *const ::core::ffi::c_void, serverlocalflag : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingSetPrivateOption(hbinding : *const ::core::ffi::c_void, option : u32, optionvalue : usize) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcBindingToStaticStringBindingW(binding : *mut ::core::ffi::c_void, stringbinding : *mut *mut u16) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcClearMutex(mutex : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcDeleteMutex(mutex : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcExceptionFilter(exceptioncode : u32) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcFree(object : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcFreeBuffer(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcFreePipeBuffer(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcGetBuffer(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcGetBufferWithObject(message : *mut RPC_MESSAGE, objectuuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcGetCurrentCallHandle() -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcGetDefaultSD(ppsecuritydescriptor : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcGetExtendedError() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcIfInqTransferSyntaxes(rpcifhandle : *mut ::core::ffi::c_void, transfersyntaxes : *mut RPC_TRANSFER_SYNTAX, transfersyntaxsize : u32, transfersyntaxcount : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcMapWin32Status(status : RPC_STATUS) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcMgmtEnableDedicatedThreadPool() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcNegotiateTransferSyntax(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcNsBindingSetEntryNameA(binding : *const ::core::ffi::c_void, entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcNsBindingSetEntryNameW(binding : *const ::core::ffi::c_void, entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn I_RpcNsGetBuffer(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcNsInterfaceExported(entrynamesyntax : u32, entryname : *const u16, rpcinterfaceinformation : *const RPC_SERVER_INTERFACE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcNsInterfaceUnexported(entrynamesyntax : u32, entryname : *mut u16, rpcinterfaceinformation : *mut RPC_SERVER_INTERFACE) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn I_RpcNsRaiseException(message : *mut RPC_MESSAGE, status : RPC_STATUS) -> ());
::windows_targets::link!("rpcns4.dll" "system" fn I_RpcNsSendReceive(message : *mut RPC_MESSAGE, handle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcOpenClientProcess(binding : *const ::core::ffi::c_void, desiredaccess : u32, clientprocess : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcPauseExecution(milliseconds : u32) -> ());
::windows_targets::link!("rpcns4.dll" "system" fn I_RpcReBindBuffer(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcReallocPipeBuffer(message : *const RPC_MESSAGE, newsize : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcReceive(message : *mut RPC_MESSAGE, size : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcRecordCalloutFailure(rpcstatus : RPC_STATUS, calloutstate : *mut RDR_CALLOUT_STATE, dllname : *mut u16) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcRequestMutex(mutex : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcSend(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcSendReceive(message : *mut RPC_MESSAGE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerCheckClientRestriction(context : *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerDisableExceptionFilter() -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerGetAssociationID(binding : *const ::core::ffi::c_void, associationid : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerInqAddressChangeFn() -> *mut RPC_ADDRESS_CHANGE_FN);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerInqLocalConnAddress(binding : *mut ::core::ffi::c_void, buffer : *mut ::core::ffi::c_void, buffersize : *mut u32, addressformat : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerInqRemoteConnAddress(binding : *mut ::core::ffi::c_void, buffer : *mut ::core::ffi::c_void, buffersize : *mut u32, addressformat : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerInqTransportType(r#type : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerRegisterForwardFunction(pforwardfunction : *mut RPC_FORWARD_FUNCTION) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerSetAddressChangeFn(paddresschangefn : *mut RPC_ADDRESS_CHANGE_FN) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerStartService(protseq : ::windows_sys::core::PCWSTR, endpoint : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerSubscribeForDisconnectNotification(binding : *const ::core::ffi::c_void, hevent : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerSubscribeForDisconnectNotification2(binding : *const ::core::ffi::c_void, hevent : *const ::core::ffi::c_void, subscriptionid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerUnsubscribeForDisconnectNotification(binding : *const ::core::ffi::c_void, subscriptionid : ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerUseProtseq2A(networkaddress : ::windows_sys::core::PCSTR, protseq : ::windows_sys::core::PCSTR, maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void, policy : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerUseProtseq2W(networkaddress : ::windows_sys::core::PCWSTR, protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void, policy : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerUseProtseqEp2A(networkaddress : ::windows_sys::core::PCSTR, protseq : ::windows_sys::core::PCSTR, maxcalls : u32, endpoint : ::windows_sys::core::PCSTR, securitydescriptor : *const ::core::ffi::c_void, policy : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcServerUseProtseqEp2W(networkaddress : ::windows_sys::core::PCWSTR, protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, endpoint : ::windows_sys::core::PCWSTR, securitydescriptor : *const ::core::ffi::c_void, policy : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcSessionStrictContextHandle() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcSsDontSerializeContext() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcSystemHandleTypeSpecificWork(handle : *mut ::core::ffi::c_void, actualtype : u8, idltype : u8, marshaldirection : LRPC_SYSTEM_HANDLE_MARSHAL_DIRECTION) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_RpcTurnOnEEInfoPropagation() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn I_UuidCreate(uuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesBufferHandleReset(handle : *const ::core::ffi::c_void, handlestyle : u32, operation : MIDL_ES_CODE, pbuffer : *const *const i8, buffersize : u32, pencodedsize : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesDecodeBufferHandleCreate(buffer : ::windows_sys::core::PCSTR, buffersize : u32, phandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesDecodeIncrementalHandleCreate(userstate : *mut ::core::ffi::c_void, readfn : MIDL_ES_READ, phandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesEncodeDynBufferHandleCreate(pbuffer : *mut *mut i8, pencodedsize : *mut u32, phandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesEncodeFixedBufferHandleCreate(pbuffer : ::windows_sys::core::PSTR, buffersize : u32, pencodedsize : *mut u32, phandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesEncodeIncrementalHandleCreate(userstate : *mut ::core::ffi::c_void, allocfn : MIDL_ES_ALLOC, writefn : MIDL_ES_WRITE, phandle : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesHandleFree(handle : *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesIncrementalHandleReset(handle : *mut ::core::ffi::c_void, userstate : *mut ::core::ffi::c_void, allocfn : MIDL_ES_ALLOC, writefn : MIDL_ES_WRITE, readfn : MIDL_ES_READ, operation : MIDL_ES_CODE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn MesInqProcEncodingId(handle : *mut ::core::ffi::c_void, pinterfaceid : *mut RPC_SYNTAX_IDENTIFIER, pprocnum : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn NDRCContextBinding(ccontext : isize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn NDRCContextMarshall(ccontext : isize, pbuff : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NDRCContextUnmarshall(pccontext : *mut isize, hbinding : *const ::core::ffi::c_void, pbuff : *const ::core::ffi::c_void, datarepresentation : u32) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NDRSContextMarshall(ccontext : *const NDR_SCONTEXT, pbuff : *mut ::core::ffi::c_void, userrundownin : NDR_RUNDOWN) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NDRSContextMarshall2(bindinghandle : *const ::core::ffi::c_void, ccontext : *const NDR_SCONTEXT, pbuff : *mut ::core::ffi::c_void, userrundownin : NDR_RUNDOWN, ctxguard : *const ::core::ffi::c_void, flags : u32) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NDRSContextMarshallEx(bindinghandle : *const ::core::ffi::c_void, ccontext : *const NDR_SCONTEXT, pbuff : *mut ::core::ffi::c_void, userrundownin : NDR_RUNDOWN) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NDRSContextUnmarshall(pbuff : *const ::core::ffi::c_void, datarepresentation : u32) -> *mut NDR_SCONTEXT);
::windows_targets::link!("rpcrt4.dll" "system" fn NDRSContextUnmarshall2(bindinghandle : *const ::core::ffi::c_void, pbuff : *const ::core::ffi::c_void, datarepresentation : u32, ctxguard : *const ::core::ffi::c_void, flags : u32) -> *mut NDR_SCONTEXT);
::windows_targets::link!("rpcrt4.dll" "system" fn NDRSContextUnmarshallEx(bindinghandle : *const ::core::ffi::c_void, pbuff : *const ::core::ffi::c_void, datarepresentation : u32) -> *mut NDR_SCONTEXT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn Ndr64AsyncClientCall(pproxyinfo : *mut MIDL_STUBLESS_PROXY_INFO, nprocnum : u32, preturnvalue : *mut ::core::ffi::c_void, ...) -> CLIENT_CALL_RETURN);
::windows_targets::link!("rpcrt4.dll" "system" fn Ndr64AsyncServerCall64(prpcmsg : *mut RPC_MESSAGE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn Ndr64AsyncServerCallAll(prpcmsg : *mut RPC_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn Ndr64DcomAsyncClientCall(pproxyinfo : *mut MIDL_STUBLESS_PROXY_INFO, nprocnum : u32, preturnvalue : *mut ::core::ffi::c_void, ...) -> CLIENT_CALL_RETURN);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn Ndr64DcomAsyncStubCall(pthis : super::Com:: IRpcStubBuffer, pchannel : super::Com:: IRpcChannelBuffer, prpcmsg : *mut RPC_MESSAGE, pdwstubphase : *mut u32) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrAllocate(pstubmsg : *mut MIDL_STUB_MESSAGE, len : usize) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrAsyncClientCall(pstubdescriptor : *mut MIDL_STUB_DESC, pformat : *mut u8, ...) -> CLIENT_CALL_RETURN);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrAsyncServerCall(prpcmsg : *mut RPC_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrByteCountPointerBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrByteCountPointerFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrByteCountPointerMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrByteCountPointerUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClearOutParameters(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8, argaddr : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClientCall2(pstubdescriptor : *mut MIDL_STUB_DESC, pformat : *mut u8, ...) -> CLIENT_CALL_RETURN);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClientCall3(pproxyinfo : *mut MIDL_STUBLESS_PROXY_INFO, nprocnum : u32, preturnvalue : *mut ::core::ffi::c_void, ...) -> CLIENT_CALL_RETURN);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClientContextMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, contexthandle : isize, fcheck : i32) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClientContextUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pcontexthandle : *mut isize, bindhandle : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClientInitialize(prpcmsg : *mut RPC_MESSAGE, pstubmsg : *mut MIDL_STUB_MESSAGE, pstubdescriptor : *mut MIDL_STUB_DESC, procnum : u32) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrClientInitializeNew(prpcmsg : *mut RPC_MESSAGE, pstubmsg : *mut MIDL_STUB_MESSAGE, pstubdescriptor : *mut MIDL_STUB_DESC, procnum : u32) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexArrayBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexArrayFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexArrayMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexArrayMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexArrayUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexStructBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexStructFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexStructMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexStructMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrComplexStructUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantArrayBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantArrayFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantArrayMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantArrayMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantArrayUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStringBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStringMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStringMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStringUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStructBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStructFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStructMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStructMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantStructUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingArrayBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingArrayFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingArrayMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingArrayMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingArrayUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingStructBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingStructFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingStructMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingStructMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConformantVaryingStructUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrContextHandleInitialize(pstubmsg : *const MIDL_STUB_MESSAGE, pformat : *const u8) -> *mut NDR_SCONTEXT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrContextHandleSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConvert(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrConvert2(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8, numberparams : i32) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrCorrelationFree(pstubmsg : *mut MIDL_STUB_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrCorrelationInitialize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut ::core::ffi::c_void, cachesize : u32, flags : u32) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrCorrelationPass(pstubmsg : *mut MIDL_STUB_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrCreateServerInterfaceFromStub(pstub : super::Com:: IRpcStubBuffer, pserverif : *mut RPC_SERVER_INTERFACE) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrDcomAsyncClientCall(pstubdescriptor : *mut MIDL_STUB_DESC, pformat : *mut u8, ...) -> CLIENT_CALL_RETURN);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrDcomAsyncStubCall(pthis : super::Com:: IRpcStubBuffer, pchannel : super::Com:: IRpcChannelBuffer, prpcmsg : *mut RPC_MESSAGE, pdwstubphase : *mut u32) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrEncapsulatedUnionBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrEncapsulatedUnionFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrEncapsulatedUnionMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrEncapsulatedUnionMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrEncapsulatedUnionUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrFixedArrayBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrFixedArrayFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrFixedArrayMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrFixedArrayMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrFixedArrayUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrFreeBuffer(pstubmsg : *mut MIDL_STUB_MESSAGE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NdrFullPointerXlatFree(pxlattables : *mut FULL_PTR_XLAT_TABLES) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NdrFullPointerXlatInit(numberofpointers : u32, xlatside : XLAT_SIDE) -> *mut FULL_PTR_XLAT_TABLES);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrGetBuffer(pstubmsg : *mut MIDL_STUB_MESSAGE, bufferlength : u32, handle : *mut ::core::ffi::c_void) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrGetDcomProtocolVersion(pstubmsg : *mut MIDL_STUB_MESSAGE, pversion : *mut RPC_VERSION) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrGetUserMarshalInfo(pflags : *const u32, informationlevel : u32, pmarshalinfo : *mut NDR_USER_MARSHAL_INFO) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrInterfacePointerBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrInterfacePointerFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrInterfacePointerMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrInterfacePointerMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrInterfacePointerUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMapCommAndFaultStatus(pstubmsg : *mut MIDL_STUB_MESSAGE, pcommstatus : *mut u32, pfaultstatus : *mut u32, status : RPC_STATUS) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesProcEncodeDecode(handle : *mut ::core::ffi::c_void, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, ...) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesProcEncodeDecode2(handle : *mut ::core::ffi::c_void, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, ...) -> CLIENT_CALL_RETURN);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "cdecl" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesProcEncodeDecode3(handle : *mut ::core::ffi::c_void, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, nprocnum : u32, preturnvalue : *mut ::core::ffi::c_void, ...) -> CLIENT_CALL_RETURN);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrMesSimpleTypeAlignSize(param0 : *mut ::core::ffi::c_void) -> usize);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesSimpleTypeAlignSizeAll(handle : *mut ::core::ffi::c_void, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO) -> usize);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrMesSimpleTypeDecode(handle : *mut ::core::ffi::c_void, pobject : *mut ::core::ffi::c_void, size : i16) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesSimpleTypeDecodeAll(handle : *mut ::core::ffi::c_void, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, pobject : *mut ::core::ffi::c_void, size : i16) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesSimpleTypeEncode(handle : *mut ::core::ffi::c_void, pstubdesc : *const MIDL_STUB_DESC, pobject : *const ::core::ffi::c_void, size : i16) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesSimpleTypeEncodeAll(handle : *mut ::core::ffi::c_void, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, pobject : *const ::core::ffi::c_void, size : i16) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeAlignSize(handle : *mut ::core::ffi::c_void, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *const ::core::ffi::c_void) -> usize);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeAlignSize2(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *const ::core::ffi::c_void) -> usize);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeAlignSize3(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, arrtypeoffset : *const *const u32, ntypeindex : u32, pobject : *const ::core::ffi::c_void) -> usize);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeDecode(handle : *mut ::core::ffi::c_void, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeDecode2(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeDecode3(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, arrtypeoffset : *const *const u32, ntypeindex : u32, pobject : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeEncode(handle : *mut ::core::ffi::c_void, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeEncode2(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeEncode3(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, arrtypeoffset : *const *const u32, ntypeindex : u32, pobject : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeFree2(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pstubdesc : *const MIDL_STUB_DESC, pformatstring : *mut u8, pobject : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrMesTypeFree3(handle : *mut ::core::ffi::c_void, ppicklinginfo : *const MIDL_TYPE_PICKLING_INFO, pproxyinfo : *const MIDL_STUBLESS_PROXY_INFO, arrtypeoffset : *const *const u32, ntypeindex : u32, pobject : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonConformantStringBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonConformantStringMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonConformantStringMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonConformantStringUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonEncapsulatedUnionBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonEncapsulatedUnionFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonEncapsulatedUnionMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonEncapsulatedUnionMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNonEncapsulatedUnionUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNsGetBuffer(pstubmsg : *mut MIDL_STUB_MESSAGE, bufferlength : u32, handle : *mut ::core::ffi::c_void) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrNsSendReceive(pstubmsg : *mut MIDL_STUB_MESSAGE, pbufferend : *mut u8, pautohandle : *mut *mut ::core::ffi::c_void) -> *mut u8);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrOleAllocate(size : usize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrOleFree(nodetofree : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPartialIgnoreClientBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPartialIgnoreClientMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPartialIgnoreServerInitialize(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut ::core::ffi::c_void, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPartialIgnoreServerUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPointerBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPointerFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPointerMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPointerMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrPointerUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrRangeUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrRpcSmClientAllocate(size : usize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrRpcSmClientFree(nodetofree : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrRpcSmSetClientToOsf(pmessage : *mut MIDL_STUB_MESSAGE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NdrRpcSsDefaultAllocate(size : usize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrRpcSsDefaultFree(nodetofree : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrRpcSsDisableAllocate(pmessage : *mut MIDL_STUB_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrRpcSsEnableAllocate(pmessage : *mut MIDL_STUB_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSendReceive(pstubmsg : *mut MIDL_STUB_MESSAGE, pbufferend : *mut u8) -> *mut u8);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrServerCall2(prpcmsg : *mut RPC_MESSAGE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NdrServerCallAll(prpcmsg : *mut RPC_MESSAGE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NdrServerCallNdr64(prpcmsg : *mut RPC_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerContextMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, contexthandle : *mut NDR_SCONTEXT, rundownroutine : NDR_RUNDOWN) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerContextNewMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, contexthandle : *mut NDR_SCONTEXT, rundownroutine : NDR_RUNDOWN, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerContextNewUnmarshall(pstubmsg : *const MIDL_STUB_MESSAGE, pformat : *const u8) -> *mut NDR_SCONTEXT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerContextUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE) -> *mut NDR_SCONTEXT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerInitialize(prpcmsg : *mut RPC_MESSAGE, pstubmsg : *mut MIDL_STUB_MESSAGE, pstubdescriptor : *mut MIDL_STUB_DESC) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerInitializeMarshall(prpcmsg : *mut RPC_MESSAGE, pstubmsg : *mut MIDL_STUB_MESSAGE) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerInitializeNew(prpcmsg : *mut RPC_MESSAGE, pstubmsg : *mut MIDL_STUB_MESSAGE, pstubdescriptor : *mut MIDL_STUB_DESC) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerInitializePartial(prpcmsg : *mut RPC_MESSAGE, pstubmsg : *mut MIDL_STUB_MESSAGE, pstubdescriptor : *mut MIDL_STUB_DESC, requestedbuffersize : u32) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrServerInitializeUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pstubdescriptor : *mut MIDL_STUB_DESC, prpcmsg : *mut RPC_MESSAGE) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleStructBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleStructFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleStructMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleStructMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleStructUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleTypeMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, formatchar : u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrSimpleTypeUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, formatchar : u8) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn NdrStubCall2(pthis : *mut ::core::ffi::c_void, pchannel : *mut ::core::ffi::c_void, prpcmsg : *mut RPC_MESSAGE, pdwstubphase : *mut u32) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrStubCall3(pthis : *mut ::core::ffi::c_void, pchannel : *mut ::core::ffi::c_void, prpcmsg : *mut RPC_MESSAGE, pdwstubphase : *mut u32) -> i32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrUserMarshalBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrUserMarshalFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrUserMarshalMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrUserMarshalMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
::windows_targets::link!("rpcrt4.dll" "system" fn NdrUserMarshalSimpleTypeConvert(pflags : *mut u32, pbuffer : *mut u8, formatchar : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrUserMarshalUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrVaryingArrayBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrVaryingArrayFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrVaryingArrayMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrVaryingArrayMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrVaryingArrayUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrXmitOrRepAsBufferSize(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrXmitOrRepAsFree(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> ());
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrXmitOrRepAsMarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, pmemory : *mut u8, pformat : *mut u8) -> *mut u8);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrXmitOrRepAsMemorySize(pstubmsg : *mut MIDL_STUB_MESSAGE, pformat : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn NdrXmitOrRepAsUnmarshall(pstubmsg : *mut MIDL_STUB_MESSAGE, ppmemory : *mut *mut u8, pformat : *mut u8, fmustalloc : u8) -> *mut u8);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcAsyncAbortCall(pasync : *mut RPC_ASYNC_STATE, exceptioncode : u32) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcAsyncCancelCall(pasync : *mut RPC_ASYNC_STATE, fabort : super::super::Foundation:: BOOL) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcAsyncCompleteCall(pasync : *mut RPC_ASYNC_STATE, reply : *mut ::core::ffi::c_void) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcAsyncGetCallStatus(pasync : *const RPC_ASYNC_STATE) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcAsyncInitializeHandle(pasync : *mut RPC_ASYNC_STATE, size : u32) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcAsyncRegisterInfo(pasync : *const RPC_ASYNC_STATE) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcBindingBind(pasync : *const RPC_ASYNC_STATE, binding : *const ::core::ffi::c_void, ifspec : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingCopy(sourcebinding : *const ::core::ffi::c_void, destinationbinding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn RpcBindingCreateA(template : *const RPC_BINDING_HANDLE_TEMPLATE_V1_A, security : *const RPC_BINDING_HANDLE_SECURITY_V1_A, options : *const RPC_BINDING_HANDLE_OPTIONS_V1, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn RpcBindingCreateW(template : *const RPC_BINDING_HANDLE_TEMPLATE_V1_W, security : *const RPC_BINDING_HANDLE_SECURITY_V1_W, options : *const RPC_BINDING_HANDLE_OPTIONS_V1, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingFree(binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingFromStringBindingA(stringbinding : ::windows_sys::core::PCSTR, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingFromStringBindingW(stringbinding : ::windows_sys::core::PCWSTR, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqAuthClientA(clientbinding : *const ::core::ffi::c_void, privs : *mut *mut ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PSTR, authnlevel : *mut u32, authnsvc : *mut u32, authzsvc : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqAuthClientExA(clientbinding : *const ::core::ffi::c_void, privs : *mut *mut ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PSTR, authnlevel : *mut u32, authnsvc : *mut u32, authzsvc : *mut u32, flags : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqAuthClientExW(clientbinding : *const ::core::ffi::c_void, privs : *mut *mut ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PWSTR, authnlevel : *mut u32, authnsvc : *mut u32, authzsvc : *mut u32, flags : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqAuthClientW(clientbinding : *const ::core::ffi::c_void, privs : *mut *mut ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PWSTR, authnlevel : *mut u32, authnsvc : *mut u32, authzsvc : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqAuthInfoA(binding : *const ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PSTR, authnlevel : *mut u32, authnsvc : *mut u32, authidentity : *mut *mut ::core::ffi::c_void, authzsvc : *mut u32) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn RpcBindingInqAuthInfoExA(binding : *const ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PSTR, authnlevel : *mut u32, authnsvc : *mut u32, authidentity : *mut *mut ::core::ffi::c_void, authzsvc : *mut u32, rpcqosversion : u32, securityqos : *mut RPC_SECURITY_QOS) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn RpcBindingInqAuthInfoExW(binding : *const ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PWSTR, authnlevel : *mut u32, authnsvc : *mut u32, authidentity : *mut *mut ::core::ffi::c_void, authzsvc : *mut u32, rpcqosversion : u32, securityqos : *mut RPC_SECURITY_QOS) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqAuthInfoW(binding : *const ::core::ffi::c_void, serverprincname : *mut ::windows_sys::core::PWSTR, authnlevel : *mut u32, authnsvc : *mut u32, authidentity : *mut *mut ::core::ffi::c_void, authzsvc : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqMaxCalls(binding : *const ::core::ffi::c_void, maxcalls : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqObject(binding : *const ::core::ffi::c_void, objectuuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingInqOption(hbinding : *const ::core::ffi::c_void, option : u32, poptionvalue : *mut usize) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingReset(binding : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingServerFromClient(clientbinding : *const ::core::ffi::c_void, serverbinding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingSetAuthInfoA(binding : *const ::core::ffi::c_void, serverprincname : ::windows_sys::core::PCSTR, authnlevel : u32, authnsvc : u32, authidentity : *const ::core::ffi::c_void, authzsvc : u32) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn RpcBindingSetAuthInfoExA(binding : *const ::core::ffi::c_void, serverprincname : ::windows_sys::core::PCSTR, authnlevel : u32, authnsvc : u32, authidentity : *const ::core::ffi::c_void, authzsvc : u32, securityqos : *const RPC_SECURITY_QOS) -> RPC_STATUS);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn RpcBindingSetAuthInfoExW(binding : *const ::core::ffi::c_void, serverprincname : ::windows_sys::core::PCWSTR, authnlevel : u32, authnsvc : u32, authidentity : *const ::core::ffi::c_void, authzsvc : u32, securityqos : *const RPC_SECURITY_QOS) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingSetAuthInfoW(binding : *const ::core::ffi::c_void, serverprincname : ::windows_sys::core::PCWSTR, authnlevel : u32, authnsvc : u32, authidentity : *const ::core::ffi::c_void, authzsvc : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingSetObject(binding : *const ::core::ffi::c_void, objectuuid : *const ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingSetOption(hbinding : *const ::core::ffi::c_void, option : u32, optionvalue : usize) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingToStringBindingA(binding : *const ::core::ffi::c_void, stringbinding : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingToStringBindingW(binding : *const ::core::ffi::c_void, stringbinding : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingUnbind(binding : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcBindingVectorFree(bindingvector : *mut *mut RPC_BINDING_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcCancelThread(thread : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcCancelThreadEx(thread : *const ::core::ffi::c_void, timeout : i32) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn RpcCertGeneratePrincipalNameA(context : *const super::super::Security::Cryptography:: CERT_CONTEXT, flags : u32, pbuffer : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_Cryptography"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_Cryptography\"`"] fn RpcCertGeneratePrincipalNameW(context : *const super::super::Security::Cryptography:: CERT_CONTEXT, flags : u32, pbuffer : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcEpRegisterA(ifspec : *const ::core::ffi::c_void, bindingvector : *const RPC_BINDING_VECTOR, uuidvector : *const UUID_VECTOR, annotation : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcEpRegisterNoReplaceA(ifspec : *const ::core::ffi::c_void, bindingvector : *const RPC_BINDING_VECTOR, uuidvector : *const UUID_VECTOR, annotation : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcEpRegisterNoReplaceW(ifspec : *const ::core::ffi::c_void, bindingvector : *const RPC_BINDING_VECTOR, uuidvector : *const UUID_VECTOR, annotation : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcEpRegisterW(ifspec : *const ::core::ffi::c_void, bindingvector : *const RPC_BINDING_VECTOR, uuidvector : *const UUID_VECTOR, annotation : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcEpResolveBinding(binding : *const ::core::ffi::c_void, ifspec : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcEpUnregister(ifspec : *const ::core::ffi::c_void, bindingvector : *const RPC_BINDING_VECTOR, uuidvector : *const UUID_VECTOR) -> RPC_STATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RpcErrorAddRecord(errorinfo : *const RPC_EXTENDED_ERROR_INFO) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorClearInformation() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorEndEnumeration(enumhandle : *mut RPC_ERROR_ENUM_HANDLE) -> RPC_STATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RpcErrorGetNextRecord(enumhandle : *const RPC_ERROR_ENUM_HANDLE, copystrings : super::super::Foundation:: BOOL, errorinfo : *mut RPC_EXTENDED_ERROR_INFO) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorGetNumberOfRecords(enumhandle : *const RPC_ERROR_ENUM_HANDLE, records : *mut i32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorLoadErrorInfo(errorblob : *const ::core::ffi::c_void, blobsize : usize, enumhandle : *mut RPC_ERROR_ENUM_HANDLE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorResetEnumeration(enumhandle : *mut RPC_ERROR_ENUM_HANDLE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorSaveErrorInfo(enumhandle : *const RPC_ERROR_ENUM_HANDLE, errorblob : *mut *mut ::core::ffi::c_void, blobsize : *mut usize) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcErrorStartEnumeration(enumhandle : *mut RPC_ERROR_ENUM_HANDLE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcExceptionFilter(exceptioncode : u32) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcFreeAuthorizationContext(pauthzclientcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RpcGetAuthorizationContextForClient(clientbinding : *const ::core::ffi::c_void, impersonateonreturn : super::super::Foundation:: BOOL, reserved1 : *const ::core::ffi::c_void, pexpirationtime : *const i64, reserved2 : super::super::Foundation:: LUID, reserved3 : u32, reserved4 : *const ::core::ffi::c_void, pauthzclientcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcIfIdVectorFree(ifidvector : *mut *mut RPC_IF_ID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcIfInqId(rpcifhandle : *const ::core::ffi::c_void, rpcifid : *mut RPC_IF_ID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcImpersonateClient(bindinghandle : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcImpersonateClient2(bindinghandle : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcImpersonateClientContainer(bindinghandle : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtEnableIdleCleanup() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtEpEltInqBegin(epbinding : *const ::core::ffi::c_void, inquirytype : u32, ifid : *const RPC_IF_ID, versoption : u32, objectuuid : *const ::windows_sys::core::GUID, inquirycontext : *mut *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtEpEltInqDone(inquirycontext : *mut *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtEpEltInqNextA(inquirycontext : *const *const ::core::ffi::c_void, ifid : *mut RPC_IF_ID, binding : *mut *mut ::core::ffi::c_void, objectuuid : *mut ::windows_sys::core::GUID, annotation : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtEpEltInqNextW(inquirycontext : *const *const ::core::ffi::c_void, ifid : *mut RPC_IF_ID, binding : *mut *mut ::core::ffi::c_void, objectuuid : *mut ::windows_sys::core::GUID, annotation : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtEpUnregister(epbinding : *const ::core::ffi::c_void, ifid : *const RPC_IF_ID, binding : *const ::core::ffi::c_void, objectuuid : *const ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtInqComTimeout(binding : *const ::core::ffi::c_void, timeout : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtInqDefaultProtectLevel(authnsvc : u32, authnlevel : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtInqIfIds(binding : *const ::core::ffi::c_void, ifidvector : *mut *mut RPC_IF_ID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtInqServerPrincNameA(binding : *const ::core::ffi::c_void, authnsvc : u32, serverprincname : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtInqServerPrincNameW(binding : *const ::core::ffi::c_void, authnsvc : u32, serverprincname : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtInqStats(binding : *const ::core::ffi::c_void, statistics : *mut *mut RPC_STATS_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtIsServerListening(binding : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtSetAuthorizationFn(authorizationfn : RPC_MGMT_AUTHORIZATION_FN) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtSetCancelTimeout(timeout : i32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtSetComTimeout(binding : *const ::core::ffi::c_void, timeout : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtSetServerStackSize(threadstacksize : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtStatsVectorFree(statsvector : *mut *mut RPC_STATS_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtStopServerListening(binding : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcMgmtWaitServerListen() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcNetworkInqProtseqsA(protseqvector : *mut *mut RPC_PROTSEQ_VECTORA) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcNetworkInqProtseqsW(protseqvector : *mut *mut RPC_PROTSEQ_VECTORW) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcNetworkIsProtseqValidA(protseq : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcNetworkIsProtseqValidW(protseq : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingExportA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifspec : *const ::core::ffi::c_void, bindingvec : *const RPC_BINDING_VECTOR, objectuuidvec : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingExportPnPA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifspec : *const ::core::ffi::c_void, objectvector : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingExportPnPW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void, objectvector : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingExportW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void, bindingvec : *const RPC_BINDING_VECTOR, objectuuidvec : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingImportBeginA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifspec : *const ::core::ffi::c_void, objuuid : *const ::windows_sys::core::GUID, importcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingImportBeginW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void, objuuid : *const ::windows_sys::core::GUID, importcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingImportDone(importcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingImportNext(importcontext : *mut ::core::ffi::c_void, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcNsBindingInqEntryNameA(binding : *const ::core::ffi::c_void, entrynamesyntax : u32, entryname : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcNsBindingInqEntryNameW(binding : *const ::core::ffi::c_void, entrynamesyntax : u32, entryname : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingLookupBeginA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifspec : *const ::core::ffi::c_void, objuuid : *const ::windows_sys::core::GUID, bindingmaxcount : u32, lookupcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingLookupBeginW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void, objuuid : *const ::windows_sys::core::GUID, bindingmaxcount : u32, lookupcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingLookupDone(lookupcontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingLookupNext(lookupcontext : *mut ::core::ffi::c_void, bindingvec : *mut *mut RPC_BINDING_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingSelect(bindingvec : *mut RPC_BINDING_VECTOR, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingUnexportA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifspec : *const ::core::ffi::c_void, objectuuidvec : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingUnexportPnPA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifspec : *const ::core::ffi::c_void, objectvector : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingUnexportPnPW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void, objectvector : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsBindingUnexportW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifspec : *const ::core::ffi::c_void, objectuuidvec : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsEntryExpandNameA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, expandedname : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsEntryExpandNameW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, expandedname : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsEntryObjectInqBeginA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsEntryObjectInqBeginW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsEntryObjectInqDone(inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsEntryObjectInqNext(inquirycontext : *mut ::core::ffi::c_void, objuuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupDeleteA(groupnamesyntax : GROUP_NAME_SYNTAX, groupname : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupDeleteW(groupnamesyntax : GROUP_NAME_SYNTAX, groupname : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrAddA(groupnamesyntax : u32, groupname : ::windows_sys::core::PCSTR, membernamesyntax : u32, membername : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrAddW(groupnamesyntax : u32, groupname : ::windows_sys::core::PCWSTR, membernamesyntax : u32, membername : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrInqBeginA(groupnamesyntax : u32, groupname : ::windows_sys::core::PCSTR, membernamesyntax : u32, inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrInqBeginW(groupnamesyntax : u32, groupname : ::windows_sys::core::PCWSTR, membernamesyntax : u32, inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrInqDone(inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrInqNextA(inquirycontext : *mut ::core::ffi::c_void, membername : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrInqNextW(inquirycontext : *mut ::core::ffi::c_void, membername : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrRemoveA(groupnamesyntax : u32, groupname : ::windows_sys::core::PCSTR, membernamesyntax : u32, membername : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsGroupMbrRemoveW(groupnamesyntax : u32, groupname : ::windows_sys::core::PCWSTR, membernamesyntax : u32, membername : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtBindingUnexportA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifid : *const RPC_IF_ID, versoption : u32, objectuuidvec : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtBindingUnexportW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifid : *const RPC_IF_ID, versoption : u32, objectuuidvec : *const UUID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtEntryCreateA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtEntryCreateW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtEntryDeleteA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtEntryDeleteW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtEntryInqIfIdsA(entrynamesyntax : u32, entryname : ::windows_sys::core::PCSTR, ifidvec : *mut *mut RPC_IF_ID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtEntryInqIfIdsW(entrynamesyntax : u32, entryname : ::windows_sys::core::PCWSTR, ifidvec : *mut *mut RPC_IF_ID_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtHandleSetExpAge(nshandle : *mut ::core::ffi::c_void, expirationage : u32) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtInqExpAge(expirationage : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsMgmtSetExpAge(expirationage : u32) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileDeleteA(profilenamesyntax : u32, profilename : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileDeleteW(profilenamesyntax : u32, profilename : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltAddA(profilenamesyntax : u32, profilename : ::windows_sys::core::PCSTR, ifid : *const RPC_IF_ID, membernamesyntax : u32, membername : ::windows_sys::core::PCSTR, priority : u32, annotation : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltAddW(profilenamesyntax : u32, profilename : ::windows_sys::core::PCWSTR, ifid : *const RPC_IF_ID, membernamesyntax : u32, membername : ::windows_sys::core::PCWSTR, priority : u32, annotation : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltInqBeginA(profilenamesyntax : u32, profilename : ::windows_sys::core::PCSTR, inquirytype : u32, ifid : *const RPC_IF_ID, versoption : u32, membernamesyntax : u32, membername : ::windows_sys::core::PCSTR, inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltInqBeginW(profilenamesyntax : u32, profilename : ::windows_sys::core::PCWSTR, inquirytype : u32, ifid : *const RPC_IF_ID, versoption : u32, membernamesyntax : u32, membername : ::windows_sys::core::PCWSTR, inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltInqDone(inquirycontext : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltInqNextA(inquirycontext : *const ::core::ffi::c_void, ifid : *mut RPC_IF_ID, membername : *mut ::windows_sys::core::PSTR, priority : *mut u32, annotation : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltInqNextW(inquirycontext : *const ::core::ffi::c_void, ifid : *mut RPC_IF_ID, membername : *mut ::windows_sys::core::PWSTR, priority : *mut u32, annotation : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltRemoveA(profilenamesyntax : u32, profilename : ::windows_sys::core::PCSTR, ifid : *const RPC_IF_ID, membernamesyntax : u32, membername : ::windows_sys::core::PCSTR) -> RPC_STATUS);
::windows_targets::link!("rpcns4.dll" "system" fn RpcNsProfileEltRemoveW(profilenamesyntax : u32, profilename : ::windows_sys::core::PCWSTR, ifid : *const RPC_IF_ID, membernamesyntax : u32, membername : ::windows_sys::core::PCWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcObjectInqType(objuuid : *const ::windows_sys::core::GUID, typeuuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcObjectSetInqFn(inquiryfn : RPC_OBJECT_INQ_FN) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcObjectSetType(objuuid : *const ::windows_sys::core::GUID, typeuuid : *const ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcProtseqVectorFreeA(protseqvector : *mut *mut RPC_PROTSEQ_VECTORA) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcProtseqVectorFreeW(protseqvector : *mut *mut RPC_PROTSEQ_VECTORW) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcRaiseException(exception : RPC_STATUS) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcRevertContainerImpersonation() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcRevertToSelf() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcRevertToSelfEx(bindinghandle : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerCompleteSecurityCallback(bindinghandle : *const ::core::ffi::c_void, status : RPC_STATUS) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqBindingHandle(binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqBindings(bindingvector : *mut *mut RPC_BINDING_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqBindingsEx(securitydescriptor : *const ::core::ffi::c_void, bindingvector : *mut *mut RPC_BINDING_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqCallAttributesA(clientbinding : *const ::core::ffi::c_void, rpccallattributes : *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqCallAttributesW(clientbinding : *const ::core::ffi::c_void, rpccallattributes : *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqDefaultPrincNameA(authnsvc : u32, princname : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqDefaultPrincNameW(authnsvc : u32, princname : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInqIf(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, mgrepv : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInterfaceGroupActivate(ifgroup : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInterfaceGroupClose(ifgroup : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInterfaceGroupCreateA(interfaces : *const RPC_INTERFACE_TEMPLATEA, numifs : u32, endpoints : *const RPC_ENDPOINT_TEMPLATEA, numendpoints : u32, idleperiod : u32, idlecallbackfn : RPC_INTERFACE_GROUP_IDLE_CALLBACK_FN, idlecallbackcontext : *const ::core::ffi::c_void, ifgroup : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInterfaceGroupCreateW(interfaces : *const RPC_INTERFACE_TEMPLATEW, numifs : u32, endpoints : *const RPC_ENDPOINT_TEMPLATEW, numendpoints : u32, idleperiod : u32, idlecallbackfn : RPC_INTERFACE_GROUP_IDLE_CALLBACK_FN, idlecallbackcontext : *const ::core::ffi::c_void, ifgroup : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInterfaceGroupDeactivate(ifgroup : *const ::core::ffi::c_void, forcedeactivation : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerInterfaceGroupInqBindings(ifgroup : *const ::core::ffi::c_void, bindingvector : *mut *mut RPC_BINDING_VECTOR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerListen(minimumcallthreads : u32, maxcalls : u32, dontwait : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerRegisterAuthInfoA(serverprincname : ::windows_sys::core::PCSTR, authnsvc : u32, getkeyfn : RPC_AUTH_KEY_RETRIEVAL_FN, arg : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerRegisterAuthInfoW(serverprincname : ::windows_sys::core::PCWSTR, authnsvc : u32, getkeyfn : RPC_AUTH_KEY_RETRIEVAL_FN, arg : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerRegisterIf(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, mgrepv : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerRegisterIf2(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, mgrepv : *const ::core::ffi::c_void, flags : u32, maxcalls : u32, maxrpcsize : u32, ifcallbackfn : RPC_IF_CALLBACK_FN) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerRegisterIf3(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, mgrepv : *const ::core::ffi::c_void, flags : u32, maxcalls : u32, maxrpcsize : u32, ifcallback : RPC_IF_CALLBACK_FN, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerRegisterIfEx(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, mgrepv : *const ::core::ffi::c_void, flags : u32, maxcalls : u32, ifcallback : RPC_IF_CALLBACK_FN) -> RPC_STATUS);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
::windows_targets::link!("rpcrt4.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"] fn RpcServerSubscribeForNotification(binding : *const ::core::ffi::c_void, notification : RPC_NOTIFICATIONS, notificationtype : RPC_NOTIFICATION_TYPES, notificationinfo : *const RPC_ASYNC_NOTIFICATION_INFO) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerTestCancel(bindinghandle : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUnregisterIf(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, waitforcallstocomplete : u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUnregisterIfEx(ifspec : *const ::core::ffi::c_void, mgrtypeuuid : *const ::windows_sys::core::GUID, rundowncontexthandles : i32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUnsubscribeForNotification(binding : *const ::core::ffi::c_void, notification : RPC_NOTIFICATIONS, notificationsqueued : *mut u32) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseAllProtseqs(maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseAllProtseqsEx(maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseAllProtseqsIf(maxcalls : u32, ifspec : *const ::core::ffi::c_void, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseAllProtseqsIfEx(maxcalls : u32, ifspec : *const ::core::ffi::c_void, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqA(protseq : ::windows_sys::core::PCSTR, maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqEpA(protseq : ::windows_sys::core::PCSTR, maxcalls : u32, endpoint : ::windows_sys::core::PCSTR, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqEpExA(protseq : ::windows_sys::core::PCSTR, maxcalls : u32, endpoint : ::windows_sys::core::PCSTR, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqEpExW(protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, endpoint : ::windows_sys::core::PCWSTR, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqEpW(protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, endpoint : ::windows_sys::core::PCWSTR, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqExA(protseq : ::windows_sys::core::PCSTR, maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqExW(protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqIfA(protseq : ::windows_sys::core::PCSTR, maxcalls : u32, ifspec : *const ::core::ffi::c_void, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqIfExA(protseq : ::windows_sys::core::PCSTR, maxcalls : u32, ifspec : *const ::core::ffi::c_void, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqIfExW(protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, ifspec : *const ::core::ffi::c_void, securitydescriptor : *const ::core::ffi::c_void, policy : *const RPC_POLICY) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqIfW(protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, ifspec : *const ::core::ffi::c_void, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerUseProtseqW(protseq : ::windows_sys::core::PCWSTR, maxcalls : u32, securitydescriptor : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcServerYield() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmAllocate(size : usize, pstatus : *mut RPC_STATUS) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmClientFree(pnodetofree : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmDestroyClientContext(contexthandle : *const *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmDisableAllocate() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmEnableAllocate() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmFree(nodetofree : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmGetThreadHandle(pstatus : *mut RPC_STATUS) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmSetClientAllocFree(clientalloc : RPC_CLIENT_ALLOC, clientfree : RPC_CLIENT_FREE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmSetThreadHandle(id : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSmSwapClientAllocFree(clientalloc : RPC_CLIENT_ALLOC, clientfree : RPC_CLIENT_FREE, oldclientalloc : *mut RPC_CLIENT_ALLOC, oldclientfree : *mut RPC_CLIENT_FREE) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsAllocate(size : usize) -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsContextLockExclusive(serverbindinghandle : *const ::core::ffi::c_void, usercontext : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsContextLockShared(serverbindinghandle : *const ::core::ffi::c_void, usercontext : *const ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsDestroyClientContext(contexthandle : *const *const ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsDisableAllocate() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsDontSerializeContext() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsEnableAllocate() -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsFree(nodetofree : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsGetContextBinding(contexthandle : *const ::core::ffi::c_void, binding : *mut *mut ::core::ffi::c_void) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsGetThreadHandle() -> *mut ::core::ffi::c_void);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsSetClientAllocFree(clientalloc : RPC_CLIENT_ALLOC, clientfree : RPC_CLIENT_FREE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsSetThreadHandle(id : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcSsSwapClientAllocFree(clientalloc : RPC_CLIENT_ALLOC, clientfree : RPC_CLIENT_FREE, oldclientalloc : *mut RPC_CLIENT_ALLOC, oldclientfree : *mut RPC_CLIENT_FREE) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn RpcStringBindingComposeA(objuuid : ::windows_sys::core::PCSTR, protseq : ::windows_sys::core::PCSTR, networkaddr : ::windows_sys::core::PCSTR, endpoint : ::windows_sys::core::PCSTR, options : ::windows_sys::core::PCSTR, stringbinding : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcStringBindingComposeW(objuuid : ::windows_sys::core::PCWSTR, protseq : ::windows_sys::core::PCWSTR, networkaddr : ::windows_sys::core::PCWSTR, endpoint : ::windows_sys::core::PCWSTR, options : ::windows_sys::core::PCWSTR, stringbinding : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcStringBindingParseA(stringbinding : ::windows_sys::core::PCSTR, objuuid : *mut ::windows_sys::core::PSTR, protseq : *mut ::windows_sys::core::PSTR, networkaddr : *mut ::windows_sys::core::PSTR, endpoint : *mut ::windows_sys::core::PSTR, networkoptions : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcStringBindingParseW(stringbinding : ::windows_sys::core::PCWSTR, objuuid : *mut ::windows_sys::core::PWSTR, protseq : *mut ::windows_sys::core::PWSTR, networkaddr : *mut ::windows_sys::core::PWSTR, endpoint : *mut ::windows_sys::core::PWSTR, networkoptions : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcStringFreeA(string : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcStringFreeW(string : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcTestCancel() -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn RpcUserFree(asynchandle : *mut ::core::ffi::c_void, pbuffer : *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("rpcrt4.dll" "system" fn UuidCompare(uuid1 : *const ::windows_sys::core::GUID, uuid2 : *const ::windows_sys::core::GUID, status : *mut RPC_STATUS) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidCreate(uuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidCreateNil(niluuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidCreateSequential(uuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidEqual(uuid1 : *const ::windows_sys::core::GUID, uuid2 : *const ::windows_sys::core::GUID, status : *mut RPC_STATUS) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidFromStringA(stringuuid : ::windows_sys::core::PCSTR, uuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidFromStringW(stringuuid : ::windows_sys::core::PCWSTR, uuid : *mut ::windows_sys::core::GUID) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidHash(uuid : *const ::windows_sys::core::GUID, status : *mut RPC_STATUS) -> u16);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidIsNil(uuid : *const ::windows_sys::core::GUID, status : *mut RPC_STATUS) -> i32);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidToStringA(uuid : *const ::windows_sys::core::GUID, stringuuid : *mut ::windows_sys::core::PSTR) -> RPC_STATUS);
::windows_targets::link!("rpcrt4.dll" "system" fn UuidToStringW(uuid : *const ::windows_sys::core::GUID, stringuuid : *mut ::windows_sys::core::PWSTR) -> RPC_STATUS);
pub const DCE_C_ERROR_STRING_LEN: u32 = 256u32;
pub const EEInfoGCCOM: u32 = 11u32;
pub const EEInfoGCFRS: u32 = 12u32;
pub const EEInfoNextRecordsMissing: u32 = 2u32;
pub const EEInfoPreviousRecordsMissing: u32 = 1u32;
pub const EEInfoUseFileTime: u32 = 4u32;
pub const EPT_S_CANT_CREATE: RPC_STATUS = 1899i32;
pub const EPT_S_CANT_PERFORM_OP: RPC_STATUS = 1752i32;
pub const EPT_S_INVALID_ENTRY: RPC_STATUS = 1751i32;
pub const EPT_S_NOT_REGISTERED: RPC_STATUS = 1753i32;
pub const FC_EXPR_CONST32: EXPR_TOKEN = 1i32;
pub const FC_EXPR_CONST64: EXPR_TOKEN = 2i32;
pub const FC_EXPR_END: EXPR_TOKEN = 6i32;
pub const FC_EXPR_ILLEGAL: EXPR_TOKEN = 0i32;
pub const FC_EXPR_NOOP: EXPR_TOKEN = 5i32;
pub const FC_EXPR_OPER: EXPR_TOKEN = 4i32;
pub const FC_EXPR_START: EXPR_TOKEN = 0i32;
pub const FC_EXPR_VAR: EXPR_TOKEN = 3i32;
pub const IDL_CS_IN_PLACE_CONVERT: IDL_CS_CONVERT = 1i32;
pub const IDL_CS_NEW_BUFFER_CONVERT: IDL_CS_CONVERT = 2i32;
pub const IDL_CS_NO_CONVERT: IDL_CS_CONVERT = 0i32;
pub const INVALID_FRAGMENT_ID: u32 = 0u32;
pub const MES_DECODE: MIDL_ES_CODE = 1i32;
pub const MES_DYNAMIC_BUFFER_HANDLE: MIDL_ES_HANDLE_STYLE = 2i32;
pub const MES_ENCODE: MIDL_ES_CODE = 0i32;
pub const MES_ENCODE_NDR64: MIDL_ES_CODE = 2i32;
pub const MES_FIXED_BUFFER_HANDLE: MIDL_ES_HANDLE_STYLE = 1i32;
pub const MES_INCREMENTAL_HANDLE: MIDL_ES_HANDLE_STYLE = 0i32;
pub const MIDL_WINRT_TYPE_SERIALIZATION_INFO_CURRENT_VERSION: i32 = 1i32;
pub const MarshalDirectionMarshal: LRPC_SYSTEM_HANDLE_MARSHAL_DIRECTION = 0i32;
pub const MarshalDirectionUnmarshal: LRPC_SYSTEM_HANDLE_MARSHAL_DIRECTION = 1i32;
pub const MaxNumberOfEEInfoParams: u32 = 4u32;
pub const MidlInterceptionInfoVersionOne: i32 = 1i32;
pub const MidlWinrtTypeSerializationInfoVersionOne: i32 = 1i32;
pub const NDR64_FC_AUTO_HANDLE: u32 = 3u32;
pub const NDR64_FC_BIND_GENERIC: u32 = 1u32;
pub const NDR64_FC_BIND_PRIMITIVE: u32 = 2u32;
pub const NDR64_FC_CALLBACK_HANDLE: u32 = 4u32;
pub const NDR64_FC_EXPLICIT_HANDLE: u32 = 0u32;
pub const NDR64_FC_NO_HANDLE: u32 = 5u32;
pub const NDR_CUSTOM_OR_DEFAULT_ALLOCATOR: u32 = 268435456u32;
pub const NDR_DEFAULT_ALLOCATOR: u32 = 536870912u32;
pub const NT351_INTERFACE_SIZE: u32 = 64u32;
pub const PROTOCOL_ADDRESS_CHANGE: RPC_ADDRESS_CHANGE_TYPE = 3i32;
pub const PROTOCOL_LOADED: RPC_ADDRESS_CHANGE_TYPE = 2i32;
pub const PROTOCOL_NOT_LOADED: RPC_ADDRESS_CHANGE_TYPE = 1i32;
pub const PROXY_CALCSIZE: PROXY_PHASE = 0i32;
pub const PROXY_GETBUFFER: PROXY_PHASE = 1i32;
pub const PROXY_MARSHAL: PROXY_PHASE = 2i32;
pub const PROXY_SENDRECEIVE: PROXY_PHASE = 3i32;
pub const PROXY_UNMARSHAL: PROXY_PHASE = 4i32;
pub const RPCFLG_ACCESSIBILITY_BIT1: u32 = 1048576u32;
pub const RPCFLG_ACCESSIBILITY_BIT2: u32 = 2097152u32;
pub const RPCFLG_ACCESS_LOCAL: u32 = 4194304u32;
pub const RPCFLG_ASYNCHRONOUS: u32 = 1073741824u32;
pub const RPCFLG_AUTO_COMPLETE: u32 = 134217728u32;
pub const RPCFLG_HAS_CALLBACK: u32 = 67108864u32;
pub const RPCFLG_HAS_GUARANTEE: u32 = 16u32;
pub const RPCFLG_HAS_MULTI_SYNTAXES: u32 = 33554432u32;
pub const RPCFLG_INPUT_SYNCHRONOUS: u32 = 536870912u32;
pub const RPCFLG_LOCAL_CALL: u32 = 268435456u32;
pub const RPCFLG_MESSAGE: u32 = 16777216u32;
pub const RPCFLG_NDR64_CONTAINS_ARM_LAYOUT: u32 = 67108864u32;
pub const RPCFLG_NON_NDR: u32 = 2147483648u32;
pub const RPCFLG_SENDER_WAITING_FOR_REPLY: u32 = 8388608u32;
pub const RPCFLG_WINRT_REMOTE_ASYNC: u32 = 32u32;
pub const RPCHTTP_RS_ACCESS_1: RPC_HTTP_REDIRECTOR_STAGE = 2i32;
pub const RPCHTTP_RS_ACCESS_2: RPC_HTTP_REDIRECTOR_STAGE = 4i32;
pub const RPCHTTP_RS_INTERFACE: RPC_HTTP_REDIRECTOR_STAGE = 5i32;
pub const RPCHTTP_RS_REDIRECT: RPC_HTTP_REDIRECTOR_STAGE = 1i32;
pub const RPCHTTP_RS_SESSION: RPC_HTTP_REDIRECTOR_STAGE = 3i32;
pub const RPC_BHO_DONTLINGER: RPC_BINDING_HANDLE_OPTIONS_FLAGS = 2u32;
pub const RPC_BHO_EXCLUSIVE_AND_GUARANTEED: u32 = 4u32;
pub const RPC_BHO_NONCAUSAL: RPC_BINDING_HANDLE_OPTIONS_FLAGS = 1u32;
pub const RPC_BHT_OBJECT_UUID_VALID: u32 = 1u32;
pub const RPC_BUFFER_ASYNC: u32 = 32768u32;
pub const RPC_BUFFER_COMPLETE: u32 = 4096u32;
pub const RPC_BUFFER_EXTRA: u32 = 16384u32;
pub const RPC_BUFFER_NONOTIFY: u32 = 65536u32;
pub const RPC_BUFFER_PARTIAL: u32 = 8192u32;
pub const RPC_CALL_ATTRIBUTES_VERSION: u32 = 2u32;
pub const RPC_CALL_STATUS_CANCELLED: u32 = 1u32;
pub const RPC_CALL_STATUS_DISCONNECTED: u32 = 2u32;
pub const RPC_CONTEXT_HANDLE_DEFAULT_FLAGS: u32 = 0u32;
pub const RPC_CONTEXT_HANDLE_DONT_SERIALIZE: u32 = 536870912u32;
pub const RPC_CONTEXT_HANDLE_FLAGS: u32 = 805306368u32;
pub const RPC_CONTEXT_HANDLE_SERIALIZE: u32 = 268435456u32;
pub const RPC_C_AUTHN_CLOUD_AP: u32 = 36u32;
pub const RPC_C_AUTHN_DCE_PRIVATE: u32 = 1u32;
pub const RPC_C_AUTHN_DCE_PUBLIC: u32 = 2u32;
pub const RPC_C_AUTHN_DEC_PUBLIC: u32 = 4u32;
pub const RPC_C_AUTHN_DEFAULT: i32 = -1i32;
pub const RPC_C_AUTHN_DIGEST: u32 = 21u32;
pub const RPC_C_AUTHN_DPA: u32 = 17u32;
pub const RPC_C_AUTHN_GSS_KERBEROS: u32 = 16u32;
pub const RPC_C_AUTHN_GSS_NEGOTIATE: u32 = 9u32;
pub const RPC_C_AUTHN_GSS_SCHANNEL: u32 = 14u32;
pub const RPC_C_AUTHN_INFO_NONE: RPC_C_AUTHN_INFO_TYPE = 0u32;
pub const RPC_C_AUTHN_INFO_TYPE_HTTP: RPC_C_AUTHN_INFO_TYPE = 1u32;
pub const RPC_C_AUTHN_KERNEL: u32 = 20u32;
pub const RPC_C_AUTHN_LIVEXP_SSP: u32 = 35u32;
pub const RPC_C_AUTHN_LIVE_SSP: u32 = 32u32;
pub const RPC_C_AUTHN_MQ: u32 = 100u32;
pub const RPC_C_AUTHN_MSN: u32 = 18u32;
pub const RPC_C_AUTHN_MSONLINE: u32 = 82u32;
pub const RPC_C_AUTHN_NEGO_EXTENDER: u32 = 30u32;
pub const RPC_C_AUTHN_NONE: u32 = 0u32;
pub const RPC_C_AUTHN_PKU2U: u32 = 31u32;
pub const RPC_C_AUTHN_WINNT: u32 = 10u32;
pub const RPC_C_AUTHZ_DCE: u32 = 2u32;
pub const RPC_C_AUTHZ_DEFAULT: u32 = 4294967295u32;
pub const RPC_C_AUTHZ_NAME: u32 = 1u32;
pub const RPC_C_AUTHZ_NONE: u32 = 0u32;
pub const RPC_C_BINDING_DEFAULT_TIMEOUT: u32 = 5u32;
pub const RPC_C_BINDING_INFINITE_TIMEOUT: u32 = 10u32;
pub const RPC_C_BINDING_MAX_TIMEOUT: u32 = 9u32;
pub const RPC_C_BINDING_MIN_TIMEOUT: u32 = 0u32;
pub const RPC_C_BIND_TO_ALL_NICS: u32 = 1u32;
pub const RPC_C_CANCEL_INFINITE_TIMEOUT: i32 = -1i32;
pub const RPC_C_DONT_FAIL: u32 = 4u32;
pub const RPC_C_EP_ALL_ELTS: u32 = 0u32;
pub const RPC_C_EP_MATCH_BY_BOTH: u32 = 3u32;
pub const RPC_C_EP_MATCH_BY_IF: u32 = 1u32;
pub const RPC_C_EP_MATCH_BY_OBJ: u32 = 2u32;
pub const RPC_C_FULL_CERT_CHAIN: u32 = 1u32;
pub const RPC_C_HTTP_AUTHN_SCHEME_BASIC: u32 = 1u32;
pub const RPC_C_HTTP_AUTHN_SCHEME_CERT: u32 = 65536u32;
pub const RPC_C_HTTP_AUTHN_SCHEME_DIGEST: u32 = 8u32;
pub const RPC_C_HTTP_AUTHN_SCHEME_NEGOTIATE: u32 = 16u32;
pub const RPC_C_HTTP_AUTHN_SCHEME_NTLM: u32 = 2u32;
pub const RPC_C_HTTP_AUTHN_SCHEME_PASSPORT: u32 = 4u32;
pub const RPC_C_HTTP_AUTHN_TARGET_PROXY: RPC_C_HTTP_AUTHN_TARGET = 2u32;
pub const RPC_C_HTTP_AUTHN_TARGET_SERVER: RPC_C_HTTP_AUTHN_TARGET = 1u32;
pub const RPC_C_HTTP_FLAG_ENABLE_CERT_REVOCATION_CHECK: RPC_C_HTTP_FLAGS = 16u32;
pub const RPC_C_HTTP_FLAG_IGNORE_CERT_CN_INVALID: RPC_C_HTTP_FLAGS = 8u32;
pub const RPC_C_HTTP_FLAG_USE_FIRST_AUTH_SCHEME: RPC_C_HTTP_FLAGS = 2u32;
pub const RPC_C_HTTP_FLAG_USE_SSL: RPC_C_HTTP_FLAGS = 1u32;
pub const RPC_C_LISTEN_MAX_CALLS_DEFAULT: u32 = 1234u32;
pub const RPC_C_MGMT_INQ_IF_IDS: u32 = 0u32;
pub const RPC_C_MGMT_INQ_PRINC_NAME: u32 = 1u32;
pub const RPC_C_MGMT_INQ_STATS: u32 = 2u32;
pub const RPC_C_MGMT_IS_SERVER_LISTEN: u32 = 3u32;
pub const RPC_C_MGMT_STOP_SERVER_LISTEN: u32 = 4u32;
pub const RPC_C_MQ_AUTHN_LEVEL_NONE: u32 = 0u32;
pub const RPC_C_MQ_AUTHN_LEVEL_PKT_INTEGRITY: u32 = 8u32;
pub const RPC_C_MQ_AUTHN_LEVEL_PKT_PRIVACY: u32 = 16u32;
pub const RPC_C_MQ_CLEAR_ON_OPEN: u32 = 2u32;
pub const RPC_C_MQ_EXPRESS: u32 = 0u32;
pub const RPC_C_MQ_JOURNAL_ALWAYS: u32 = 2u32;
pub const RPC_C_MQ_JOURNAL_DEADLETTER: u32 = 1u32;
pub const RPC_C_MQ_JOURNAL_NONE: u32 = 0u32;
pub const RPC_C_MQ_PERMANENT: u32 = 1u32;
pub const RPC_C_MQ_RECOVERABLE: u32 = 1u32;
pub const RPC_C_MQ_TEMPORARY: u32 = 0u32;
pub const RPC_C_MQ_USE_EXISTING_SECURITY: u32 = 4u32;
pub const RPC_C_NOTIFY_ON_SEND_COMPLETE: u32 = 1u32;
pub const RPC_C_NS_DEFAULT_EXP_AGE: i32 = -1i32;
pub const RPC_C_NS_SYNTAX_DCE: GROUP_NAME_SYNTAX = 3u32;
pub const RPC_C_NS_SYNTAX_DEFAULT: GROUP_NAME_SYNTAX = 0u32;
pub const RPC_C_OPT_ASYNC_BLOCK: u32 = 15u32;
pub const RPC_C_OPT_BINDING_NONCAUSAL: u32 = 9u32;
pub const RPC_C_OPT_CALL_TIMEOUT: u32 = 12u32;
pub const RPC_C_OPT_COOKIE_AUTH: u32 = 7u32;
pub const RPC_C_OPT_DONT_LINGER: u32 = 13u32;
pub const RPC_C_OPT_MAX_OPTIONS: u32 = 12u32;
pub const RPC_C_OPT_MQ_ACKNOWLEDGE: u32 = 4u32;
pub const RPC_C_OPT_MQ_AUTHN_LEVEL: u32 = 6u32;
pub const RPC_C_OPT_MQ_AUTHN_SERVICE: u32 = 5u32;
pub const RPC_C_OPT_MQ_DELIVERY: u32 = 1u32;
pub const RPC_C_OPT_MQ_JOURNAL: u32 = 3u32;
pub const RPC_C_OPT_MQ_PRIORITY: u32 = 2u32;
pub const RPC_C_OPT_MQ_TIME_TO_BE_RECEIVED: u32 = 8u32;
pub const RPC_C_OPT_MQ_TIME_TO_REACH_QUEUE: u32 = 7u32;
pub const RPC_C_OPT_OPTIMIZE_TIME: u32 = 16u32;
pub const RPC_C_OPT_PRIVATE_BREAK_ON_SUSPEND: u32 = 3u32;
pub const RPC_C_OPT_PRIVATE_DO_NOT_DISTURB: u32 = 2u32;
pub const RPC_C_OPT_PRIVATE_SUPPRESS_WAKE: u32 = 1u32;
pub const RPC_C_OPT_RESOURCE_TYPE_UUID: u32 = 8u32;
pub const RPC_C_OPT_SECURITY_CALLBACK: u32 = 10u32;
pub const RPC_C_OPT_SESSION_ID: u32 = 6u32;
pub const RPC_C_OPT_TRANS_SEND_BUFFER_SIZE: u32 = 5u32;
pub const RPC_C_OPT_TRUST_PEER: u32 = 14u32;
pub const RPC_C_OPT_UNIQUE_BINDING: u32 = 11u32;
pub const RPC_C_PARM_BUFFER_LENGTH: u32 = 2u32;
pub const RPC_C_PARM_MAX_PACKET_LENGTH: u32 = 1u32;
pub const RPC_C_PROFILE_ALL_ELT: u32 = 1u32;
pub const RPC_C_PROFILE_ALL_ELTS: u32 = 1u32;
pub const RPC_C_PROFILE_DEFAULT_ELT: u32 = 0u32;
pub const RPC_C_PROFILE_MATCH_BY_BOTH: u32 = 4u32;
pub const RPC_C_PROFILE_MATCH_BY_IF: u32 = 2u32;
pub const RPC_C_PROFILE_MATCH_BY_MBR: u32 = 3u32;
pub const RPC_C_PROTSEQ_MAX_REQS_DEFAULT: u32 = 10u32;
pub const RPC_C_QOS_CAPABILITIES_ANY_AUTHORITY: RPC_C_QOS_CAPABILITIES = 4u32;
pub const RPC_C_QOS_CAPABILITIES_DEFAULT: RPC_C_QOS_CAPABILITIES = 0u32;
pub const RPC_C_QOS_CAPABILITIES_IGNORE_DELEGATE_FAILURE: RPC_C_QOS_CAPABILITIES = 8u32;
pub const RPC_C_QOS_CAPABILITIES_LOCAL_MA_HINT: RPC_C_QOS_CAPABILITIES = 16u32;
pub const RPC_C_QOS_CAPABILITIES_MAKE_FULLSIC: RPC_C_QOS_CAPABILITIES = 2u32;
pub const RPC_C_QOS_CAPABILITIES_MUTUAL_AUTH: RPC_C_QOS_CAPABILITIES = 1u32;
pub const RPC_C_QOS_CAPABILITIES_SCHANNEL_FULL_AUTH_IDENTITY: RPC_C_QOS_CAPABILITIES = 32u32;
pub const RPC_C_QOS_IDENTITY_DYNAMIC: RPC_C_QOS_IDENTITY = 1u32;
pub const RPC_C_QOS_IDENTITY_STATIC: RPC_C_QOS_IDENTITY = 0u32;
pub const RPC_C_RPCHTTP_USE_LOAD_BALANCE: u32 = 8u32;
pub const RPC_C_SECURITY_QOS_VERSION: i32 = 1i32;
pub const RPC_C_SECURITY_QOS_VERSION_1: i32 = 1i32;
pub const RPC_C_SECURITY_QOS_VERSION_2: i32 = 2i32;
pub const RPC_C_SECURITY_QOS_VERSION_3: i32 = 3i32;
pub const RPC_C_SECURITY_QOS_VERSION_4: i32 = 4i32;
pub const RPC_C_SECURITY_QOS_VERSION_5: i32 = 5i32;
pub const RPC_C_STATS_CALLS_IN: u32 = 0u32;
pub const RPC_C_STATS_CALLS_OUT: u32 = 1u32;
pub const RPC_C_STATS_PKTS_IN: u32 = 2u32;
pub const RPC_C_STATS_PKTS_OUT: u32 = 3u32;
pub const RPC_C_TRY_ENFORCE_MAX_CALLS: u32 = 16u32;
pub const RPC_C_USE_INTERNET_PORT: u32 = 1u32;
pub const RPC_C_USE_INTRANET_PORT: u32 = 2u32;
pub const RPC_C_VERS_ALL: u32 = 1u32;
pub const RPC_C_VERS_COMPATIBLE: u32 = 2u32;
pub const RPC_C_VERS_EXACT: u32 = 3u32;
pub const RPC_C_VERS_MAJOR_ONLY: u32 = 4u32;
pub const RPC_C_VERS_UPTO: u32 = 5u32;
pub const RPC_EEINFO_VERSION: u32 = 1u32;
pub const RPC_FLAGS_VALID_BIT: u32 = 32768u32;
pub const RPC_FW_IF_FLAG_DCOM: u32 = 1u32;
pub const RPC_IF_ALLOW_CALLBACKS_WITH_NO_AUTH: u32 = 16u32;
pub const RPC_IF_ALLOW_LOCAL_ONLY: u32 = 32u32;
pub const RPC_IF_ALLOW_SECURE_ONLY: u32 = 8u32;
pub const RPC_IF_ALLOW_UNKNOWN_AUTHORITY: u32 = 4u32;
pub const RPC_IF_ASYNC_CALLBACK: u32 = 256u32;
pub const RPC_IF_AUTOLISTEN: u32 = 1u32;
pub const RPC_IF_OLE: u32 = 2u32;
pub const RPC_IF_SEC_CACHE_PER_PROC: u32 = 128u32;
pub const RPC_IF_SEC_NO_CACHE: u32 = 64u32;
pub const RPC_INTERFACE_HAS_PIPES: u32 = 1u32;
pub const RPC_NCA_FLAGS_BROADCAST: u32 = 2u32;
pub const RPC_NCA_FLAGS_DEFAULT: u32 = 0u32;
pub const RPC_NCA_FLAGS_IDEMPOTENT: u32 = 1u32;
pub const RPC_NCA_FLAGS_MAYBE: u32 = 4u32;
pub const RPC_PROTSEQ_HTTP: u32 = 4u32;
pub const RPC_PROTSEQ_LRPC: u32 = 3u32;
pub const RPC_PROTSEQ_NMP: u32 = 2u32;
pub const RPC_PROTSEQ_TCP: u32 = 1u32;
pub const RPC_PROXY_CONNECTION_TYPE_IN_PROXY: u32 = 0u32;
pub const RPC_PROXY_CONNECTION_TYPE_OUT_PROXY: u32 = 1u32;
pub const RPC_P_ADDR_FORMAT_TCP_IPV4: u32 = 1u32;
pub const RPC_P_ADDR_FORMAT_TCP_IPV6: u32 = 2u32;
pub const RPC_QUERY_CALL_LOCAL_ADDRESS: u32 = 8u32;
pub const RPC_QUERY_CLIENT_ID: u32 = 128u32;
pub const RPC_QUERY_CLIENT_PID: u32 = 16u32;
pub const RPC_QUERY_CLIENT_PRINCIPAL_NAME: u32 = 4u32;
pub const RPC_QUERY_IS_CLIENT_LOCAL: u32 = 32u32;
pub const RPC_QUERY_NO_AUTH_REQUIRED: u32 = 64u32;
pub const RPC_QUERY_SERVER_PRINCIPAL_NAME: u32 = 2u32;
pub const RPC_SYSTEM_HANDLE_FREE_ALL: u32 = 3u32;
pub const RPC_SYSTEM_HANDLE_FREE_ERROR_ON_CLOSE: u32 = 4u32;
pub const RPC_SYSTEM_HANDLE_FREE_RETRIEVED: u32 = 2u32;
pub const RPC_SYSTEM_HANDLE_FREE_UNRETRIEVED: u32 = 1u32;
pub const RPC_S_ADDRESS_ERROR: RPC_STATUS = 1768i32;
pub const RPC_S_ALREADY_LISTENING: RPC_STATUS = 1713i32;
pub const RPC_S_ALREADY_REGISTERED: RPC_STATUS = 1711i32;
pub const RPC_S_BINDING_HAS_NO_AUTH: RPC_STATUS = 1746i32;
pub const RPC_S_BINDING_INCOMPLETE: RPC_STATUS = 1819i32;
pub const RPC_S_CALL_CANCELLED: RPC_STATUS = 1818i32;
pub const RPC_S_CALL_FAILED: RPC_STATUS = 1726i32;
pub const RPC_S_CALL_FAILED_DNE: RPC_STATUS = 1727i32;
pub const RPC_S_CALL_IN_PROGRESS: RPC_STATUS = 1791i32;
pub const RPC_S_CANNOT_SUPPORT: RPC_STATUS = 1764i32;
pub const RPC_S_CANT_CREATE_ENDPOINT: RPC_STATUS = 1720i32;
pub const RPC_S_COMM_FAILURE: RPC_STATUS = 1820i32;
pub const RPC_S_COOKIE_AUTH_FAILED: RPC_STATUS = 1833i32;
pub const RPC_S_DO_NOT_DISTURB: RPC_STATUS = 1834i32;
pub const RPC_S_DUPLICATE_ENDPOINT: RPC_STATUS = 1740i32;
pub const RPC_S_ENTRY_ALREADY_EXISTS: RPC_STATUS = 1760i32;
pub const RPC_S_ENTRY_NOT_FOUND: RPC_STATUS = 1761i32;
pub const RPC_S_ENTRY_TYPE_MISMATCH: RPC_STATUS = 1922i32;
pub const RPC_S_FP_DIV_ZERO: RPC_STATUS = 1769i32;
pub const RPC_S_FP_OVERFLOW: RPC_STATUS = 1771i32;
pub const RPC_S_FP_UNDERFLOW: RPC_STATUS = 1770i32;
pub const RPC_S_GROUP_MEMBER_NOT_FOUND: RPC_STATUS = 1898i32;
pub const RPC_S_GRP_ELT_NOT_ADDED: RPC_STATUS = 1928i32;
pub const RPC_S_GRP_ELT_NOT_REMOVED: RPC_STATUS = 1929i32;
pub const RPC_S_INCOMPLETE_NAME: RPC_STATUS = 1755i32;
pub const RPC_S_INTERFACE_NOT_EXPORTED: RPC_STATUS = 1924i32;
pub const RPC_S_INTERFACE_NOT_FOUND: RPC_STATUS = 1759i32;
pub const RPC_S_INTERNAL_ERROR: RPC_STATUS = 1766i32;
pub const RPC_S_INVALID_ASYNC_CALL: RPC_STATUS = 1915i32;
pub const RPC_S_INVALID_ASYNC_HANDLE: RPC_STATUS = 1914i32;
pub const RPC_S_INVALID_AUTH_IDENTITY: RPC_STATUS = 1749i32;
pub const RPC_S_INVALID_BINDING: RPC_STATUS = 1702i32;
pub const RPC_S_INVALID_BOUND: RPC_STATUS = 1734i32;
pub const RPC_S_INVALID_ENDPOINT_FORMAT: RPC_STATUS = 1706i32;
pub const RPC_S_INVALID_NAF_ID: RPC_STATUS = 1763i32;
pub const RPC_S_INVALID_NAME_SYNTAX: RPC_STATUS = 1736i32;
pub const RPC_S_INVALID_NETWORK_OPTIONS: RPC_STATUS = 1724i32;
pub const RPC_S_INVALID_NET_ADDR: RPC_STATUS = 1707i32;
pub const RPC_S_INVALID_OBJECT: RPC_STATUS = 1900i32;
pub const RPC_S_INVALID_RPC_PROTSEQ: RPC_STATUS = 1704i32;
pub const RPC_S_INVALID_STRING_BINDING: RPC_STATUS = 1700i32;
pub const RPC_S_INVALID_STRING_UUID: RPC_STATUS = 1705i32;
pub const RPC_S_INVALID_TAG: RPC_STATUS = 1733i32;
pub const RPC_S_INVALID_TIMEOUT: RPC_STATUS = 1709i32;
pub const RPC_S_INVALID_VERS_OPTION: RPC_STATUS = 1756i32;
pub const RPC_S_MAX_CALLS_TOO_SMALL: RPC_STATUS = 1742i32;
pub const RPC_S_NAME_SERVICE_UNAVAILABLE: RPC_STATUS = 1762i32;
pub const RPC_S_NOTHING_TO_EXPORT: RPC_STATUS = 1754i32;
pub const RPC_S_NOT_ALL_OBJS_EXPORTED: RPC_STATUS = 1923i32;
pub const RPC_S_NOT_ALL_OBJS_UNEXPORTED: RPC_STATUS = 1758i32;
pub const RPC_S_NOT_CANCELLED: RPC_STATUS = 1826i32;
pub const RPC_S_NOT_LISTENING: RPC_STATUS = 1715i32;
pub const RPC_S_NOT_RPC_ERROR: RPC_STATUS = 1823i32;
pub const RPC_S_NO_BINDINGS: RPC_STATUS = 1718i32;
pub const RPC_S_NO_CALL_ACTIVE: RPC_STATUS = 1725i32;
pub const RPC_S_NO_CONTEXT_AVAILABLE: RPC_STATUS = 1765i32;
pub const RPC_S_NO_ENDPOINT_FOUND: RPC_STATUS = 1708i32;
pub const RPC_S_NO_ENTRY_NAME: RPC_STATUS = 1735i32;
pub const RPC_S_NO_INTERFACES: RPC_STATUS = 1817i32;
pub const RPC_S_NO_MORE_BINDINGS: RPC_STATUS = 1806i32;
pub const RPC_S_NO_MORE_MEMBERS: RPC_STATUS = 1757i32;
pub const RPC_S_NO_PRINC_NAME: RPC_STATUS = 1822i32;
pub const RPC_S_NO_PROTSEQS: RPC_STATUS = 1719i32;
pub const RPC_S_NO_PROTSEQS_REGISTERED: RPC_STATUS = 1714i32;
pub const RPC_S_OBJECT_NOT_FOUND: RPC_STATUS = 1710i32;
pub const RPC_S_OUT_OF_RESOURCES: RPC_STATUS = 1721i32;
pub const RPC_S_PRF_ELT_NOT_ADDED: RPC_STATUS = 1926i32;
pub const RPC_S_PRF_ELT_NOT_REMOVED: RPC_STATUS = 1927i32;
pub const RPC_S_PROCNUM_OUT_OF_RANGE: RPC_STATUS = 1745i32;
pub const RPC_S_PROFILE_NOT_ADDED: RPC_STATUS = 1925i32;
pub const RPC_S_PROTOCOL_ERROR: RPC_STATUS = 1728i32;
pub const RPC_S_PROTSEQ_NOT_FOUND: RPC_STATUS = 1744i32;
pub const RPC_S_PROTSEQ_NOT_SUPPORTED: RPC_STATUS = 1703i32;
pub const RPC_S_PROXY_ACCESS_DENIED: RPC_STATUS = 1729i32;
pub const RPC_S_SEC_PKG_ERROR: RPC_STATUS = 1825i32;
pub const RPC_S_SEND_INCOMPLETE: RPC_STATUS = 1913i32;
pub const RPC_S_SERVER_TOO_BUSY: RPC_STATUS = 1723i32;
pub const RPC_S_SERVER_UNAVAILABLE: RPC_STATUS = 1722i32;
pub const RPC_S_STRING_TOO_LONG: RPC_STATUS = 1743i32;
pub const RPC_S_SYSTEM_HANDLE_COUNT_EXCEEDED: RPC_STATUS = 1835i32;
pub const RPC_S_SYSTEM_HANDLE_TYPE_MISMATCH: RPC_STATUS = 1836i32;
pub const RPC_S_TYPE_ALREADY_REGISTERED: RPC_STATUS = 1712i32;
pub const RPC_S_UNKNOWN_AUTHN_LEVEL: RPC_STATUS = 1748i32;
pub const RPC_S_UNKNOWN_AUTHN_SERVICE: RPC_STATUS = 1747i32;
pub const RPC_S_UNKNOWN_AUTHN_TYPE: RPC_STATUS = 1741i32;
pub const RPC_S_UNKNOWN_AUTHZ_SERVICE: RPC_STATUS = 1750i32;
pub const RPC_S_UNKNOWN_IF: RPC_STATUS = 1717i32;
pub const RPC_S_UNKNOWN_MGR_TYPE: RPC_STATUS = 1716i32;
pub const RPC_S_UNSUPPORTED_AUTHN_LEVEL: RPC_STATUS = 1821i32;
pub const RPC_S_UNSUPPORTED_NAME_SYNTAX: RPC_STATUS = 1737i32;
pub const RPC_S_UNSUPPORTED_TRANS_SYN: RPC_STATUS = 1730i32;
pub const RPC_S_UNSUPPORTED_TYPE: RPC_STATUS = 1732i32;
pub const RPC_S_UUID_LOCAL_ONLY: RPC_STATUS = 1824i32;
pub const RPC_S_UUID_NO_ADDRESS: RPC_STATUS = 1739i32;
pub const RPC_S_WRONG_KIND_OF_BINDING: RPC_STATUS = 1701i32;
pub const RPC_S_ZERO_DIVIDE: RPC_STATUS = 1767i32;
pub const RPC_TYPE_DISCONNECT_EVENT_CONTEXT_HANDLE: u32 = 2147483648u32;
pub const RPC_TYPE_STRICT_CONTEXT_HANDLE: u32 = 1073741824u32;
pub const RpcAttemptedLbsDecisions: RpcPerfCounters = 8i32;
pub const RpcAttemptedLbsMessages: RpcPerfCounters = 10i32;
pub const RpcBackEndConnectionAttempts: RpcPerfCounters = 2i32;
pub const RpcBackEndConnectionFailed: RpcPerfCounters = 3i32;
pub const RpcCallComplete: RPC_ASYNC_EVENT = 0i32;
pub const RpcClientCancel: RPC_ASYNC_EVENT = 4i32;
pub const RpcClientDisconnect: RPC_ASYNC_EVENT = 3i32;
pub const RpcCurrentUniqueUser: RpcPerfCounters = 1i32;
pub const RpcFailedLbsDecisions: RpcPerfCounters = 9i32;
pub const RpcFailedLbsMessages: RpcPerfCounters = 11i32;
pub const RpcIncomingBandwidth: RpcPerfCounters = 6i32;
pub const RpcIncomingConnections: RpcPerfCounters = 5i32;
pub const RpcLastCounter: RpcPerfCounters = 12i32;
pub const RpcNotificationCallCancel: RPC_NOTIFICATIONS = 2i32;
pub const RpcNotificationCallNone: RPC_NOTIFICATIONS = 0i32;
pub const RpcNotificationClientDisconnect: RPC_NOTIFICATIONS = 1i32;
pub const RpcNotificationTypeApc: RPC_NOTIFICATION_TYPES = 2i32;
pub const RpcNotificationTypeCallback: RPC_NOTIFICATION_TYPES = 5i32;
pub const RpcNotificationTypeEvent: RPC_NOTIFICATION_TYPES = 1i32;
pub const RpcNotificationTypeHwnd: RPC_NOTIFICATION_TYPES = 4i32;
pub const RpcNotificationTypeIoc: RPC_NOTIFICATION_TYPES = 3i32;
pub const RpcNotificationTypeNone: RPC_NOTIFICATION_TYPES = 0i32;
pub const RpcOutgoingBandwidth: RpcPerfCounters = 7i32;
pub const RpcReceiveComplete: RPC_ASYNC_EVENT = 2i32;
pub const RpcRequestsPerSecond: RpcPerfCounters = 4i32;
pub const RpcSendComplete: RPC_ASYNC_EVENT = 1i32;
pub const SEC_WINNT_AUTH_IDENTITY_ANSI: SEC_WINNT_AUTH_IDENTITY = 1u32;
pub const SEC_WINNT_AUTH_IDENTITY_UNICODE: SEC_WINNT_AUTH_IDENTITY = 2u32;
pub const STUB_CALL_SERVER: STUB_PHASE = 1i32;
pub const STUB_CALL_SERVER_NO_HRESULT: STUB_PHASE = 3i32;
pub const STUB_MARSHAL: STUB_PHASE = 2i32;
pub const STUB_UNMARSHAL: STUB_PHASE = 0i32;
pub const SYSTEM_HANDLE_COMPOSITION_OBJECT: system_handle_t = 9i32;
pub const SYSTEM_HANDLE_EVENT: system_handle_t = 2i32;
pub const SYSTEM_HANDLE_FILE: system_handle_t = 0i32;
pub const SYSTEM_HANDLE_INVALID: system_handle_t = 255i32;
pub const SYSTEM_HANDLE_JOB: system_handle_t = 11i32;
pub const SYSTEM_HANDLE_MAX: system_handle_t = 12i32;
pub const SYSTEM_HANDLE_MUTEX: system_handle_t = 3i32;
pub const SYSTEM_HANDLE_PIPE: system_handle_t = 12i32;
pub const SYSTEM_HANDLE_PROCESS: system_handle_t = 4i32;
pub const SYSTEM_HANDLE_REG_KEY: system_handle_t = 7i32;
pub const SYSTEM_HANDLE_SECTION: system_handle_t = 6i32;
pub const SYSTEM_HANDLE_SEMAPHORE: system_handle_t = 1i32;
pub const SYSTEM_HANDLE_SOCKET: system_handle_t = 10i32;
pub const SYSTEM_HANDLE_THREAD: system_handle_t = 8i32;
pub const SYSTEM_HANDLE_TOKEN: system_handle_t = 5i32;
pub const TARGET_IS_NT100_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT1012_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT102_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT351_OR_WIN95_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT40_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT50_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT51_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT60_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT61_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT62_OR_LATER: u32 = 1u32;
pub const TARGET_IS_NT63_OR_LATER: u32 = 1u32;
pub const TRANSPORT_TYPE_CN: u32 = 1u32;
pub const TRANSPORT_TYPE_DG: u32 = 2u32;
pub const TRANSPORT_TYPE_LPC: u32 = 4u32;
pub const TRANSPORT_TYPE_WMSG: u32 = 8u32;
pub const USER_CALL_IS_ASYNC: u32 = 256u32;
pub const USER_CALL_NEW_CORRELATION_DESC: u32 = 512u32;
pub const USER_MARSHAL_CB_BUFFER_SIZE: USER_MARSHAL_CB_TYPE = 0i32;
pub const USER_MARSHAL_CB_FREE: USER_MARSHAL_CB_TYPE = 3i32;
pub const USER_MARSHAL_CB_MARSHALL: USER_MARSHAL_CB_TYPE = 1i32;
pub const USER_MARSHAL_CB_UNMARSHALL: USER_MARSHAL_CB_TYPE = 2i32;
pub const USER_MARSHAL_FC_BYTE: u32 = 1u32;
pub const USER_MARSHAL_FC_CHAR: u32 = 2u32;
pub const USER_MARSHAL_FC_DOUBLE: u32 = 12u32;
pub const USER_MARSHAL_FC_FLOAT: u32 = 10u32;
pub const USER_MARSHAL_FC_HYPER: u32 = 11u32;
pub const USER_MARSHAL_FC_LONG: u32 = 8u32;
pub const USER_MARSHAL_FC_SHORT: u32 = 6u32;
pub const USER_MARSHAL_FC_SMALL: u32 = 3u32;
pub const USER_MARSHAL_FC_ULONG: u32 = 9u32;
pub const USER_MARSHAL_FC_USHORT: u32 = 7u32;
pub const USER_MARSHAL_FC_USMALL: u32 = 4u32;
pub const USER_MARSHAL_FC_WCHAR: u32 = 5u32;
pub const XLAT_CLIENT: XLAT_SIDE = 2i32;
pub const XLAT_SERVER: XLAT_SIDE = 1i32;
pub const __RPCPROXY_H_VERSION__: u32 = 477u32;
pub const cbNDRContext: u32 = 20u32;
pub const eeptAnsiString: ExtendedErrorParamTypes = 1i32;
pub const eeptBinary: ExtendedErrorParamTypes = 7i32;
pub const eeptLongVal: ExtendedErrorParamTypes = 3i32;
pub const eeptNone: ExtendedErrorParamTypes = 6i32;
pub const eeptPointerVal: ExtendedErrorParamTypes = 5i32;
pub const eeptShortVal: ExtendedErrorParamTypes = 4i32;
pub const eeptUnicodeString: ExtendedErrorParamTypes = 2i32;
pub const rcclClientUnknownLocality: RpcCallClientLocality = 3i32;
pub const rcclInvalid: RpcCallClientLocality = 0i32;
pub const rcclLocal: RpcCallClientLocality = 1i32;
pub const rcclRemote: RpcCallClientLocality = 2i32;
pub const rctGuaranteed: RpcCallType = 3i32;
pub const rctInvalid: RpcCallType = 0i32;
pub const rctNormal: RpcCallType = 1i32;
pub const rctTraining: RpcCallType = 2i32;
pub const rlafIPv4: RpcLocalAddressFormat = 1i32;
pub const rlafIPv6: RpcLocalAddressFormat = 2i32;
pub const rlafInvalid: RpcLocalAddressFormat = 0i32;
pub type EXPR_TOKEN = i32;
pub type ExtendedErrorParamTypes = i32;
pub type GROUP_NAME_SYNTAX = u32;
pub type IDL_CS_CONVERT = i32;
pub type LRPC_SYSTEM_HANDLE_MARSHAL_DIRECTION = i32;
pub type MIDL_ES_CODE = i32;
pub type MIDL_ES_HANDLE_STYLE = i32;
pub type PROXY_PHASE = i32;
pub type RPC_ADDRESS_CHANGE_TYPE = i32;
pub type RPC_ASYNC_EVENT = i32;
pub type RPC_BINDING_HANDLE_OPTIONS_FLAGS = u32;
pub type RPC_C_AUTHN_INFO_TYPE = u32;
pub type RPC_C_HTTP_AUTHN_TARGET = u32;
pub type RPC_C_HTTP_FLAGS = u32;
pub type RPC_C_QOS_CAPABILITIES = u32;
pub type RPC_C_QOS_IDENTITY = u32;
pub type RPC_HTTP_REDIRECTOR_STAGE = i32;
pub type RPC_NOTIFICATIONS = i32;
pub type RPC_NOTIFICATION_TYPES = i32;
pub type RPC_STATUS = i32;
pub type RpcCallClientLocality = i32;
pub type RpcCallType = i32;
pub type RpcLocalAddressFormat = i32;
pub type RpcPerfCounters = i32;
pub type SEC_WINNT_AUTH_IDENTITY = u32;
pub type STUB_PHASE = i32;
pub type USER_MARSHAL_CB_TYPE = i32;
pub type XLAT_SIDE = i32;
pub type system_handle_t = i32;
#[repr(C)]
pub struct ARRAY_INFO {
    pub Dimension: i32,
    pub BufferConformanceMark: *mut u32,
    pub BufferVarianceMark: *mut u32,
    pub MaxCountArray: *mut u32,
    pub OffsetArray: *mut u32,
    pub ActualCountArray: *mut u32,
}
impl ::core::marker::Copy for ARRAY_INFO {}
impl ::core::clone::Clone for ARRAY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct BinaryParam {
    pub Buffer: *mut ::core::ffi::c_void,
    pub Size: i16,
}
impl ::core::marker::Copy for BinaryParam {}
impl ::core::clone::Clone for BinaryParam {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLIENT_CALL_RETURN {
    pub Pointer: *mut ::core::ffi::c_void,
    pub Simple: isize,
}
impl ::core::marker::Copy for CLIENT_CALL_RETURN {}
impl ::core::clone::Clone for CLIENT_CALL_RETURN {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct COMM_FAULT_OFFSETS {
    pub CommOffset: i16,
    pub FaultOffset: i16,
}
impl ::core::marker::Copy for COMM_FAULT_OFFSETS {}
impl ::core::clone::Clone for COMM_FAULT_OFFSETS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FULL_PTR_XLAT_TABLES {
    pub RefIdToPointer: *mut ::core::ffi::c_void,
    pub PointerToRefId: *mut ::core::ffi::c_void,
    pub NextRefId: u32,
    pub XlatSide: XLAT_SIDE,
}
impl ::core::marker::Copy for FULL_PTR_XLAT_TABLES {}
impl ::core::clone::Clone for FULL_PTR_XLAT_TABLES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GENERIC_BINDING_INFO {
    pub pObj: *mut ::core::ffi::c_void,
    pub Size: u32,
    pub pfnBind: GENERIC_BINDING_ROUTINE,
    pub pfnUnbind: GENERIC_UNBIND_ROUTINE,
}
impl ::core::marker::Copy for GENERIC_BINDING_INFO {}
impl ::core::clone::Clone for GENERIC_BINDING_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GENERIC_BINDING_ROUTINE_PAIR {
    pub pfnBind: GENERIC_BINDING_ROUTINE,
    pub pfnUnbind: GENERIC_UNBIND_ROUTINE,
}
impl ::core::marker::Copy for GENERIC_BINDING_ROUTINE_PAIR {}
impl ::core::clone::Clone for GENERIC_BINDING_ROUTINE_PAIR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct I_RpcProxyCallbackInterface {
    pub IsValidMachineFn: I_RpcProxyIsValidMachineFn,
    pub GetClientAddressFn: I_RpcProxyGetClientAddressFn,
    pub GetConnectionTimeoutFn: I_RpcProxyGetConnectionTimeoutFn,
    pub PerformCalloutFn: I_RpcPerformCalloutFn,
    pub FreeCalloutStateFn: I_RpcFreeCalloutStateFn,
    pub GetClientSessionAndResourceUUIDFn: I_RpcProxyGetClientSessionAndResourceUUID,
    pub ProxyFilterIfFn: I_RpcProxyFilterIfFn,
    pub RpcProxyUpdatePerfCounterFn: I_RpcProxyUpdatePerfCounterFn,
    pub RpcProxyUpdatePerfCounterBackendServerFn: I_RpcProxyUpdatePerfCounterBackendServerFn,
}
impl ::core::marker::Copy for I_RpcProxyCallbackInterface {}
impl ::core::clone::Clone for I_RpcProxyCallbackInterface {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MALLOC_FREE_STRUCT {
    pub pfnAllocate: isize,
    pub pfnFree: isize,
}
impl ::core::marker::Copy for MALLOC_FREE_STRUCT {}
impl ::core::clone::Clone for MALLOC_FREE_STRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_FORMAT_STRING {
    pub Pad: i16,
    pub Format: [u8; 1],
}
impl ::core::marker::Copy for MIDL_FORMAT_STRING {}
impl ::core::clone::Clone for MIDL_FORMAT_STRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_INTERCEPTION_INFO {
    pub Version: u32,
    pub ProcString: *mut u8,
    pub ProcFormatOffsetTable: *const u16,
    pub ProcCount: u32,
    pub TypeString: *mut u8,
}
impl ::core::marker::Copy for MIDL_INTERCEPTION_INFO {}
impl ::core::clone::Clone for MIDL_INTERCEPTION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_INTERFACE_METHOD_PROPERTIES {
    pub MethodCount: u16,
    pub MethodProperties: *const *const MIDL_METHOD_PROPERTY_MAP,
}
impl ::core::marker::Copy for MIDL_INTERFACE_METHOD_PROPERTIES {}
impl ::core::clone::Clone for MIDL_INTERFACE_METHOD_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_METHOD_PROPERTY {
    pub Id: u32,
    pub Value: usize,
}
impl ::core::marker::Copy for MIDL_METHOD_PROPERTY {}
impl ::core::clone::Clone for MIDL_METHOD_PROPERTY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_METHOD_PROPERTY_MAP {
    pub Count: u32,
    pub Properties: *const MIDL_METHOD_PROPERTY,
}
impl ::core::marker::Copy for MIDL_METHOD_PROPERTY_MAP {}
impl ::core::clone::Clone for MIDL_METHOD_PROPERTY_MAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MIDL_SERVER_INFO {
    pub pStubDesc: *mut MIDL_STUB_DESC,
    pub DispatchTable: *const SERVER_ROUTINE,
    pub ProcString: *mut u8,
    pub FmtStringOffset: *const u16,
    pub ThunkTable: *const STUB_THUNK,
    pub pTransferSyntax: *mut RPC_SYNTAX_IDENTIFIER,
    pub nCount: usize,
    pub pSyntaxInfo: *mut MIDL_SYNTAX_INFO,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MIDL_SERVER_INFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MIDL_SERVER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MIDL_STUBLESS_PROXY_INFO {
    pub pStubDesc: *mut MIDL_STUB_DESC,
    pub ProcFormatString: *mut u8,
    pub FormatStringOffset: *const u16,
    pub pTransferSyntax: *mut RPC_SYNTAX_IDENTIFIER,
    pub nCount: usize,
    pub pSyntaxInfo: *mut MIDL_SYNTAX_INFO,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MIDL_STUBLESS_PROXY_INFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MIDL_STUBLESS_PROXY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MIDL_STUB_DESC {
    pub RpcInterfaceInformation: *mut ::core::ffi::c_void,
    pub pfnAllocate: PFN_RPC_ALLOCATE,
    pub pfnFree: PFN_RPC_FREE,
    pub IMPLICIT_HANDLE_INFO: MIDL_STUB_DESC_0,
    pub apfnNdrRundownRoutines: *const NDR_RUNDOWN,
    pub aGenericBindingRoutinePairs: *const GENERIC_BINDING_ROUTINE_PAIR,
    pub apfnExprEval: *const EXPR_EVAL,
    pub aXmitQuintuple: *const XMIT_ROUTINE_QUINTUPLE,
    pub pFormatTypes: *const u8,
    pub fCheckBounds: i32,
    pub Version: u32,
    pub pMallocFreeStruct: *mut MALLOC_FREE_STRUCT,
    pub MIDLVersion: i32,
    pub CommFaultOffsets: *const COMM_FAULT_OFFSETS,
    pub aUserMarshalQuadruple: *const USER_MARSHAL_ROUTINE_QUADRUPLE,
    pub NotifyRoutineTable: *const NDR_NOTIFY_ROUTINE,
    pub mFlags: usize,
    pub CsRoutineTables: *const NDR_CS_ROUTINES,
    pub ProxyServerInfo: *mut ::core::ffi::c_void,
    pub pExprInfo: *const NDR_EXPR_DESC,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MIDL_STUB_DESC {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MIDL_STUB_DESC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union MIDL_STUB_DESC_0 {
    pub pAutoHandle: *mut *mut ::core::ffi::c_void,
    pub pPrimitiveHandle: *mut *mut ::core::ffi::c_void,
    pub pGenericBindingInfo: *mut GENERIC_BINDING_INFO,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MIDL_STUB_DESC_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MIDL_STUB_DESC_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MIDL_STUB_MESSAGE {
    pub RpcMsg: *mut RPC_MESSAGE,
    pub Buffer: *mut u8,
    pub BufferStart: *mut u8,
    pub BufferEnd: *mut u8,
    pub BufferMark: *mut u8,
    pub BufferLength: u32,
    pub MemorySize: u32,
    pub Memory: *mut u8,
    pub IsClient: u8,
    pub Pad: u8,
    pub uFlags2: u16,
    pub ReuseBuffer: i32,
    pub pAllocAllNodesContext: *mut NDR_ALLOC_ALL_NODES_CONTEXT,
    pub pPointerQueueState: *mut NDR_POINTER_QUEUE_STATE,
    pub IgnoreEmbeddedPointers: i32,
    pub PointerBufferMark: *mut u8,
    pub CorrDespIncrement: u8,
    pub uFlags: u8,
    pub UniquePtrCount: u16,
    pub MaxCount: usize,
    pub Offset: u32,
    pub ActualCount: u32,
    pub pfnAllocate: PFN_RPC_ALLOCATE,
    pub pfnFree: PFN_RPC_FREE,
    pub StackTop: *mut u8,
    pub pPresentedType: *mut u8,
    pub pTransmitType: *mut u8,
    pub SavedHandle: *mut ::core::ffi::c_void,
    pub StubDesc: *const MIDL_STUB_DESC,
    pub FullPtrXlatTables: *mut FULL_PTR_XLAT_TABLES,
    pub FullPtrRefId: u32,
    pub PointerLength: u32,
    pub _bitfield: i32,
    pub dwDestContext: u32,
    pub pvDestContext: *mut ::core::ffi::c_void,
    pub SavedContextHandles: *mut *mut NDR_SCONTEXT,
    pub ParamNumber: i32,
    pub pRpcChannelBuffer: super::Com::IRpcChannelBuffer,
    pub pArrayInfo: *mut ARRAY_INFO,
    pub SizePtrCountArray: *mut u32,
    pub SizePtrOffsetArray: *mut u32,
    pub SizePtrLengthArray: *mut u32,
    pub pArgQueue: *mut ::core::ffi::c_void,
    pub dwStubPhase: u32,
    pub LowStackMark: *mut ::core::ffi::c_void,
    pub pAsyncMsg: PNDR_ASYNC_MESSAGE,
    pub pCorrInfo: PNDR_CORRELATION_INFO,
    pub pCorrMemory: *mut u8,
    pub pMemoryList: *mut ::core::ffi::c_void,
    pub pCSInfo: isize,
    pub ConformanceMark: *mut u8,
    pub VarianceMark: *mut u8,
    pub Unused: isize,
    pub pContext: *mut _NDR_PROC_CONTEXT,
    pub ContextHandleHash: *mut ::core::ffi::c_void,
    pub pUserMarshalList: *mut ::core::ffi::c_void,
    pub Reserved51_3: isize,
    pub Reserved51_4: isize,
    pub Reserved51_5: isize,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MIDL_STUB_MESSAGE {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MIDL_STUB_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_SYNTAX_INFO {
    pub TransferSyntax: RPC_SYNTAX_IDENTIFIER,
    pub DispatchTable: *mut RPC_DISPATCH_TABLE,
    pub ProcString: *mut u8,
    pub FmtStringOffset: *const u16,
    pub TypeString: *mut u8,
    pub aUserMarshalQuadruple: *const ::core::ffi::c_void,
    pub pMethodProperties: *const MIDL_INTERFACE_METHOD_PROPERTIES,
    pub pReserved2: usize,
}
impl ::core::marker::Copy for MIDL_SYNTAX_INFO {}
impl ::core::clone::Clone for MIDL_SYNTAX_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIDL_TYPE_PICKLING_INFO {
    pub Version: u32,
    pub Flags: u32,
    pub Reserved: [usize; 3],
}
impl ::core::marker::Copy for MIDL_TYPE_PICKLING_INFO {}
impl ::core::clone::Clone for MIDL_TYPE_PICKLING_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct MIDL_WINRT_TYPE_SERIALIZATION_INFO {
    pub Version: u32,
    pub TypeFormatString: *mut u8,
    pub FormatStringSize: u16,
    pub TypeOffset: u16,
    pub StubDesc: *mut MIDL_STUB_DESC,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for MIDL_WINRT_TYPE_SERIALIZATION_INFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for MIDL_WINRT_TYPE_SERIALIZATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_ARRAY_ELEMENT_INFO {
    pub ElementMemSize: u32,
    pub Element: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_ARRAY_ELEMENT_INFO {}
impl ::core::clone::Clone for NDR64_ARRAY_ELEMENT_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_ARRAY_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_ARRAY_FLAGS {}
impl ::core::clone::Clone for NDR64_ARRAY_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union NDR64_BINDINGS {
    pub Primitive: NDR64_BIND_PRIMITIVE,
    pub Generic: NDR64_BIND_GENERIC,
    pub Context: NDR64_BIND_CONTEXT,
}
impl ::core::marker::Copy for NDR64_BINDINGS {}
impl ::core::clone::Clone for NDR64_BINDINGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BIND_AND_NOTIFY_EXTENSION {
    pub Binding: NDR64_BIND_CONTEXT,
    pub NotifyIndex: u16,
}
impl ::core::marker::Copy for NDR64_BIND_AND_NOTIFY_EXTENSION {}
impl ::core::clone::Clone for NDR64_BIND_AND_NOTIFY_EXTENSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BIND_CONTEXT {
    pub HandleType: u8,
    pub Flags: u8,
    pub StackOffset: u16,
    pub RoutineIndex: u8,
    pub Ordinal: u8,
}
impl ::core::marker::Copy for NDR64_BIND_CONTEXT {}
impl ::core::clone::Clone for NDR64_BIND_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BIND_GENERIC {
    pub HandleType: u8,
    pub Flags: u8,
    pub StackOffset: u16,
    pub RoutineIndex: u8,
    pub Size: u8,
}
impl ::core::marker::Copy for NDR64_BIND_GENERIC {}
impl ::core::clone::Clone for NDR64_BIND_GENERIC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BIND_PRIMITIVE {
    pub HandleType: u8,
    pub Flags: u8,
    pub StackOffset: u16,
    pub Reserved: u16,
}
impl ::core::marker::Copy for NDR64_BIND_PRIMITIVE {}
impl ::core::clone::Clone for NDR64_BIND_PRIMITIVE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BOGUS_ARRAY_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_ARRAY_FLAGS,
    pub NumberDims: u8,
    pub NumberElements: u32,
    pub Element: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_BOGUS_ARRAY_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_BOGUS_ARRAY_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BOGUS_STRUCTURE_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_STRUCTURE_FLAGS,
    pub Reserve: u8,
    pub MemorySize: u32,
    pub OriginalMemberLayout: *mut ::core::ffi::c_void,
    pub OriginalPointerLayout: *mut ::core::ffi::c_void,
    pub PointerLayout: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_BOGUS_STRUCTURE_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_BOGUS_STRUCTURE_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_BUFFER_ALIGN_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Reserved: u16,
    pub Reserved2: u32,
}
impl ::core::marker::Copy for NDR64_BUFFER_ALIGN_FORMAT {}
impl ::core::clone::Clone for NDR64_BUFFER_ALIGN_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONFORMANT_STRING_FORMAT {
    pub Header: NDR64_STRING_HEADER_FORMAT,
}
impl ::core::marker::Copy for NDR64_CONFORMANT_STRING_FORMAT {}
impl ::core::clone::Clone for NDR64_CONFORMANT_STRING_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONF_ARRAY_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_ARRAY_FLAGS,
    pub Reserved: u8,
    pub ElementSize: u32,
    pub ConfDescriptor: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_CONF_ARRAY_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_CONF_ARRAY_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONF_BOGUS_STRUCTURE_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_STRUCTURE_FLAGS,
    pub Dimensions: u8,
    pub MemorySize: u32,
    pub OriginalMemberLayout: *mut ::core::ffi::c_void,
    pub OriginalPointerLayout: *mut ::core::ffi::c_void,
    pub PointerLayout: *mut ::core::ffi::c_void,
    pub ConfArrayDescription: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_CONF_BOGUS_STRUCTURE_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_CONF_BOGUS_STRUCTURE_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONF_STRUCTURE_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_STRUCTURE_FLAGS,
    pub Reserve: u8,
    pub MemorySize: u32,
    pub ArrayDescription: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_CONF_STRUCTURE_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_CONF_STRUCTURE_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONF_VAR_ARRAY_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_ARRAY_FLAGS,
    pub Reserved: u8,
    pub ElementSize: u32,
    pub ConfDescriptor: *mut ::core::ffi::c_void,
    pub VarDescriptor: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_CONF_VAR_ARRAY_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_CONF_VAR_ARRAY_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONF_VAR_BOGUS_ARRAY_HEADER_FORMAT {
    pub FixedArrayFormat: NDR64_BOGUS_ARRAY_HEADER_FORMAT,
    pub ConfDescription: *mut ::core::ffi::c_void,
    pub VarDescription: *mut ::core::ffi::c_void,
    pub OffsetDescription: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_CONF_VAR_BOGUS_ARRAY_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_CONF_VAR_BOGUS_ARRAY_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONSTANT_IID_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub Reserved: u16,
    pub Guid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for NDR64_CONSTANT_IID_FORMAT {}
impl ::core::clone::Clone for NDR64_CONSTANT_IID_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONTEXT_HANDLE_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_CONTEXT_HANDLE_FLAGS {}
impl ::core::clone::Clone for NDR64_CONTEXT_HANDLE_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_CONTEXT_HANDLE_FORMAT {
    pub FormatCode: u8,
    pub ContextFlags: u8,
    pub RundownRoutineIndex: u8,
    pub Ordinal: u8,
}
impl ::core::marker::Copy for NDR64_CONTEXT_HANDLE_FORMAT {}
impl ::core::clone::Clone for NDR64_CONTEXT_HANDLE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_EMBEDDED_COMPLEX_FORMAT {
    pub FormatCode: u8,
    pub Reserve1: u8,
    pub Reserve2: u16,
    pub Type: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_EMBEDDED_COMPLEX_FORMAT {}
impl ::core::clone::Clone for NDR64_EMBEDDED_COMPLEX_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_ENCAPSULATED_UNION {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: u8,
    pub SwitchType: u8,
    pub MemoryOffset: u32,
    pub MemorySize: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDR64_ENCAPSULATED_UNION {}
impl ::core::clone::Clone for NDR64_ENCAPSULATED_UNION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_EXPR_CONST32 {
    pub ExprType: u8,
    pub Reserved: u8,
    pub Reserved1: u16,
    pub ConstValue: u32,
}
impl ::core::marker::Copy for NDR64_EXPR_CONST32 {}
impl ::core::clone::Clone for NDR64_EXPR_CONST32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_EXPR_CONST64 {
    pub ExprType: u8,
    pub Reserved: u8,
    pub Reserved1: u16,
    pub ConstValue: i64,
}
impl ::core::marker::Copy for NDR64_EXPR_CONST64 {}
impl ::core::clone::Clone for NDR64_EXPR_CONST64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_EXPR_NOOP {
    pub ExprType: u8,
    pub Size: u8,
    pub Reserved: u16,
}
impl ::core::marker::Copy for NDR64_EXPR_NOOP {}
impl ::core::clone::Clone for NDR64_EXPR_NOOP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_EXPR_OPERATOR {
    pub ExprType: u8,
    pub Operator: u8,
    pub CastType: u8,
    pub Reserved: u8,
}
impl ::core::marker::Copy for NDR64_EXPR_OPERATOR {}
impl ::core::clone::Clone for NDR64_EXPR_OPERATOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_EXPR_VAR {
    pub ExprType: u8,
    pub VarType: u8,
    pub Reserved: u16,
    pub Offset: u32,
}
impl ::core::marker::Copy for NDR64_EXPR_VAR {}
impl ::core::clone::Clone for NDR64_EXPR_VAR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_FIXED_REPEAT_FORMAT {
    pub RepeatFormat: NDR64_REPEAT_FORMAT,
    pub Iterations: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDR64_FIXED_REPEAT_FORMAT {}
impl ::core::clone::Clone for NDR64_FIXED_REPEAT_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_FIX_ARRAY_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_ARRAY_FLAGS,
    pub Reserved: u8,
    pub TotalSize: u32,
}
impl ::core::marker::Copy for NDR64_FIX_ARRAY_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_FIX_ARRAY_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_IID_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_IID_FLAGS {}
impl ::core::clone::Clone for NDR64_IID_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_IID_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub Reserved: u16,
    pub IIDDescriptor: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_IID_FORMAT {}
impl ::core::clone::Clone for NDR64_IID_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_MEMPAD_FORMAT {
    pub FormatCode: u8,
    pub Reserve1: u8,
    pub MemPad: u16,
    pub Reserved2: u32,
}
impl ::core::marker::Copy for NDR64_MEMPAD_FORMAT {}
impl ::core::clone::Clone for NDR64_MEMPAD_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_NON_CONFORMANT_STRING_FORMAT {
    pub Header: NDR64_STRING_HEADER_FORMAT,
    pub TotalSize: u32,
}
impl ::core::marker::Copy for NDR64_NON_CONFORMANT_STRING_FORMAT {}
impl ::core::clone::Clone for NDR64_NON_CONFORMANT_STRING_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_NON_ENCAPSULATED_UNION {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: u8,
    pub SwitchType: u8,
    pub MemorySize: u32,
    pub Switch: *mut ::core::ffi::c_void,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDR64_NON_ENCAPSULATED_UNION {}
impl ::core::clone::Clone for NDR64_NON_ENCAPSULATED_UNION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_NO_REPEAT_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub Reserved1: u16,
    pub Reserved2: u32,
}
impl ::core::marker::Copy for NDR64_NO_REPEAT_FORMAT {}
impl ::core::clone::Clone for NDR64_NO_REPEAT_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_PARAM_FLAGS {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for NDR64_PARAM_FLAGS {}
impl ::core::clone::Clone for NDR64_PARAM_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_PARAM_FORMAT {
    pub Type: *mut ::core::ffi::c_void,
    pub Attributes: NDR64_PARAM_FLAGS,
    pub Reserved: u16,
    pub StackOffset: u32,
}
impl ::core::marker::Copy for NDR64_PARAM_FORMAT {}
impl ::core::clone::Clone for NDR64_PARAM_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_PIPE_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_PIPE_FLAGS {}
impl ::core::clone::Clone for NDR64_PIPE_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_PIPE_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub Alignment: u8,
    pub Reserved: u8,
    pub Type: *mut ::core::ffi::c_void,
    pub MemorySize: u32,
    pub BufferSize: u32,
}
impl ::core::marker::Copy for NDR64_PIPE_FORMAT {}
impl ::core::clone::Clone for NDR64_PIPE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_POINTER_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub Reserved: u16,
    pub Pointee: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_POINTER_FORMAT {}
impl ::core::clone::Clone for NDR64_POINTER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_POINTER_INSTANCE_HEADER_FORMAT {
    pub Offset: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDR64_POINTER_INSTANCE_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_POINTER_INSTANCE_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_POINTER_REPEAT_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_POINTER_REPEAT_FLAGS {}
impl ::core::clone::Clone for NDR64_POINTER_REPEAT_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_PROC_FLAGS {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for NDR64_PROC_FLAGS {}
impl ::core::clone::Clone for NDR64_PROC_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_PROC_FORMAT {
    pub Flags: u32,
    pub StackSize: u32,
    pub ConstantClientBufferSize: u32,
    pub ConstantServerBufferSize: u32,
    pub RpcFlags: u16,
    pub FloatDoubleMask: u16,
    pub NumberOfParams: u16,
    pub ExtensionSize: u16,
}
impl ::core::marker::Copy for NDR64_PROC_FORMAT {}
impl ::core::clone::Clone for NDR64_PROC_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_RANGED_STRING_FORMAT {
    pub Header: NDR64_STRING_HEADER_FORMAT,
    pub Reserved: u32,
    pub Min: u64,
    pub Max: u64,
}
impl ::core::marker::Copy for NDR64_RANGED_STRING_FORMAT {}
impl ::core::clone::Clone for NDR64_RANGED_STRING_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_RANGE_FORMAT {
    pub FormatCode: u8,
    pub RangeType: u8,
    pub Reserved: u16,
    pub MinValue: i64,
    pub MaxValue: i64,
}
impl ::core::marker::Copy for NDR64_RANGE_FORMAT {}
impl ::core::clone::Clone for NDR64_RANGE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_RANGE_PIPE_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub Alignment: u8,
    pub Reserved: u8,
    pub Type: *mut ::core::ffi::c_void,
    pub MemorySize: u32,
    pub BufferSize: u32,
    pub MinValue: u32,
    pub MaxValue: u32,
}
impl ::core::marker::Copy for NDR64_RANGE_PIPE_FORMAT {}
impl ::core::clone::Clone for NDR64_RANGE_PIPE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_REPEAT_FORMAT {
    pub FormatCode: u8,
    pub Flags: NDR64_POINTER_REPEAT_FLAGS,
    pub Reserved: u16,
    pub Increment: u32,
    pub OffsetToArray: u32,
    pub NumberOfPointers: u32,
}
impl ::core::marker::Copy for NDR64_REPEAT_FORMAT {}
impl ::core::clone::Clone for NDR64_REPEAT_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_RPC_FLAGS {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for NDR64_RPC_FLAGS {}
impl ::core::clone::Clone for NDR64_RPC_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_SIMPLE_MEMBER_FORMAT {
    pub FormatCode: u8,
    pub Reserved1: u8,
    pub Reserved2: u16,
    pub Reserved3: u32,
}
impl ::core::marker::Copy for NDR64_SIMPLE_MEMBER_FORMAT {}
impl ::core::clone::Clone for NDR64_SIMPLE_MEMBER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_SIMPLE_REGION_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub RegionSize: u16,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDR64_SIMPLE_REGION_FORMAT {}
impl ::core::clone::Clone for NDR64_SIMPLE_REGION_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_SIZED_CONFORMANT_STRING_FORMAT {
    pub Header: NDR64_STRING_HEADER_FORMAT,
    pub SizeDescription: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_SIZED_CONFORMANT_STRING_FORMAT {}
impl ::core::clone::Clone for NDR64_SIZED_CONFORMANT_STRING_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_STRING_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_STRING_FLAGS {}
impl ::core::clone::Clone for NDR64_STRING_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_STRING_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Flags: NDR64_STRING_FLAGS,
    pub ElementSize: u16,
}
impl ::core::marker::Copy for NDR64_STRING_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_STRING_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_STRUCTURE_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_STRUCTURE_FLAGS {}
impl ::core::clone::Clone for NDR64_STRUCTURE_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_STRUCTURE_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_STRUCTURE_FLAGS,
    pub Reserve: u8,
    pub MemorySize: u32,
}
impl ::core::marker::Copy for NDR64_STRUCTURE_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_STRUCTURE_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_SYSTEM_HANDLE_FORMAT {
    pub FormatCode: u8,
    pub HandleType: u8,
    pub DesiredAccess: u32,
}
impl ::core::marker::Copy for NDR64_SYSTEM_HANDLE_FORMAT {}
impl ::core::clone::Clone for NDR64_SYSTEM_HANDLE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_TRANSMIT_AS_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_TRANSMIT_AS_FLAGS {}
impl ::core::clone::Clone for NDR64_TRANSMIT_AS_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_TRANSMIT_AS_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub RoutineIndex: u16,
    pub TransmittedTypeWireAlignment: u16,
    pub MemoryAlignment: u16,
    pub PresentedTypeMemorySize: u32,
    pub TransmittedTypeBufferSize: u32,
    pub TransmittedType: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_TRANSMIT_AS_FORMAT {}
impl ::core::clone::Clone for NDR64_TRANSMIT_AS_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_TYPE_STRICT_CONTEXT_HANDLE {
    pub FormatCode: u8,
    pub RealFormatCode: u8,
    pub Reserved: u16,
    pub Type: *mut ::core::ffi::c_void,
    pub CtxtFlags: u32,
    pub CtxtID: u32,
}
impl ::core::marker::Copy for NDR64_TYPE_STRICT_CONTEXT_HANDLE {}
impl ::core::clone::Clone for NDR64_TYPE_STRICT_CONTEXT_HANDLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_UNION_ARM {
    pub CaseValue: i64,
    pub Type: *mut ::core::ffi::c_void,
    pub Reserved: u32,
}
impl ::core::marker::Copy for NDR64_UNION_ARM {}
impl ::core::clone::Clone for NDR64_UNION_ARM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_UNION_ARM_SELECTOR {
    pub Reserved1: u8,
    pub Alignment: u8,
    pub Reserved2: u16,
    pub Arms: u32,
}
impl ::core::marker::Copy for NDR64_UNION_ARM_SELECTOR {}
impl ::core::clone::Clone for NDR64_UNION_ARM_SELECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_USER_MARSHAL_FLAGS {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for NDR64_USER_MARSHAL_FLAGS {}
impl ::core::clone::Clone for NDR64_USER_MARSHAL_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_USER_MARSHAL_FORMAT {
    pub FormatCode: u8,
    pub Flags: u8,
    pub RoutineIndex: u16,
    pub TransmittedTypeWireAlignment: u16,
    pub MemoryAlignment: u16,
    pub UserTypeMemorySize: u32,
    pub TransmittedTypeBufferSize: u32,
    pub TransmittedType: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_USER_MARSHAL_FORMAT {}
impl ::core::clone::Clone for NDR64_USER_MARSHAL_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR64_VAR_ARRAY_HEADER_FORMAT {
    pub FormatCode: u8,
    pub Alignment: u8,
    pub Flags: NDR64_ARRAY_FLAGS,
    pub Reserved: u8,
    pub TotalSize: u32,
    pub ElementSize: u32,
    pub VarDescriptor: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR64_VAR_ARRAY_HEADER_FORMAT {}
impl ::core::clone::Clone for NDR64_VAR_ARRAY_HEADER_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type NDR_ALLOC_ALL_NODES_CONTEXT = isize;
#[repr(C)]
pub struct NDR_CS_ROUTINES {
    pub pSizeConvertRoutines: *mut NDR_CS_SIZE_CONVERT_ROUTINES,
    pub pTagGettingRoutines: *mut CS_TAG_GETTING_ROUTINE,
}
impl ::core::marker::Copy for NDR_CS_ROUTINES {}
impl ::core::clone::Clone for NDR_CS_ROUTINES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR_CS_SIZE_CONVERT_ROUTINES {
    pub pfnNetSize: CS_TYPE_NET_SIZE_ROUTINE,
    pub pfnToNetCs: CS_TYPE_TO_NETCS_ROUTINE,
    pub pfnLocalSize: CS_TYPE_LOCAL_SIZE_ROUTINE,
    pub pfnFromNetCs: CS_TYPE_FROM_NETCS_ROUTINE,
}
impl ::core::marker::Copy for NDR_CS_SIZE_CONVERT_ROUTINES {}
impl ::core::clone::Clone for NDR_CS_SIZE_CONVERT_ROUTINES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NDR_EXPR_DESC {
    pub pOffset: *const u16,
    pub pFormatExpr: *mut u8,
}
impl ::core::marker::Copy for NDR_EXPR_DESC {}
impl ::core::clone::Clone for NDR_EXPR_DESC {
    fn clone(&self) -> Self {
        *self
    }
}
pub type NDR_POINTER_QUEUE_STATE = isize;
#[repr(C)]
pub struct NDR_SCONTEXT {
    pub pad: [*mut ::core::ffi::c_void; 2],
    pub userContext: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for NDR_SCONTEXT {}
impl ::core::clone::Clone for NDR_SCONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct NDR_USER_MARSHAL_INFO {
    pub InformationLevel: u32,
    pub Anonymous: NDR_USER_MARSHAL_INFO_0,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for NDR_USER_MARSHAL_INFO {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for NDR_USER_MARSHAL_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union NDR_USER_MARSHAL_INFO_0 {
    pub Level1: NDR_USER_MARSHAL_INFO_LEVEL1,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for NDR_USER_MARSHAL_INFO_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for NDR_USER_MARSHAL_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct NDR_USER_MARSHAL_INFO_LEVEL1 {
    pub Buffer: *mut ::core::ffi::c_void,
    pub BufferSize: u32,
    pub pfnAllocate: isize,
    pub pfnFree: isize,
    pub pRpcChannelBuffer: super::Com::IRpcChannelBuffer,
    pub Reserved: [usize; 5],
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for NDR_USER_MARSHAL_INFO_LEVEL1 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for NDR_USER_MARSHAL_INFO_LEVEL1 {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PNDR_ASYNC_MESSAGE = isize;
pub type PNDR_CORRELATION_INFO = isize;
#[repr(C)]
pub struct RDR_CALLOUT_STATE {
    pub LastError: RPC_STATUS,
    pub LastEEInfo: *mut ::core::ffi::c_void,
    pub LastCalledStage: RPC_HTTP_REDIRECTOR_STAGE,
    pub ServerName: *mut u16,
    pub ServerPort: *mut u16,
    pub RemoteUser: *mut u16,
    pub AuthType: *mut u16,
    pub ResourceTypePresent: u8,
    pub SessionIdPresent: u8,
    pub InterfacePresent: u8,
    pub ResourceType: ::windows_sys::core::GUID,
    pub SessionId: ::windows_sys::core::GUID,
    pub Interface: RPC_SYNTAX_IDENTIFIER,
    pub CertContext: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for RDR_CALLOUT_STATE {}
impl ::core::clone::Clone for RDR_CALLOUT_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
pub union RPC_ASYNC_NOTIFICATION_INFO {
    pub APC: RPC_ASYNC_NOTIFICATION_INFO_0,
    pub IOC: RPC_ASYNC_NOTIFICATION_INFO_1,
    pub IntPtr: RPC_ASYNC_NOTIFICATION_INFO_2,
    pub hEvent: super::super::Foundation::HANDLE,
    pub NotificationRoutine: PFN_RPCNOTIFICATION_ROUTINE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::marker::Copy for RPC_ASYNC_NOTIFICATION_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::clone::Clone for RPC_ASYNC_NOTIFICATION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
pub struct RPC_ASYNC_NOTIFICATION_INFO_0 {
    pub NotificationRoutine: PFN_RPCNOTIFICATION_ROUTINE,
    pub hThread: super::super::Foundation::HANDLE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::marker::Copy for RPC_ASYNC_NOTIFICATION_INFO_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::clone::Clone for RPC_ASYNC_NOTIFICATION_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
pub struct RPC_ASYNC_NOTIFICATION_INFO_1 {
    pub hIOPort: super::super::Foundation::HANDLE,
    pub dwNumberOfBytesTransferred: u32,
    pub dwCompletionKey: usize,
    pub lpOverlapped: *mut super::IO::OVERLAPPED,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::marker::Copy for RPC_ASYNC_NOTIFICATION_INFO_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::clone::Clone for RPC_ASYNC_NOTIFICATION_INFO_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
pub struct RPC_ASYNC_NOTIFICATION_INFO_2 {
    pub hWnd: super::super::Foundation::HWND,
    pub Msg: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::marker::Copy for RPC_ASYNC_NOTIFICATION_INFO_2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::clone::Clone for RPC_ASYNC_NOTIFICATION_INFO_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
pub struct RPC_ASYNC_STATE {
    pub Size: u32,
    pub Signature: u32,
    pub Lock: i32,
    pub Flags: u32,
    pub StubInfo: *mut ::core::ffi::c_void,
    pub UserInfo: *mut ::core::ffi::c_void,
    pub RuntimeInfo: *mut ::core::ffi::c_void,
    pub Event: RPC_ASYNC_EVENT,
    pub NotificationType: RPC_NOTIFICATION_TYPES,
    pub u: RPC_ASYNC_NOTIFICATION_INFO,
    pub Reserved: [isize; 4],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::marker::Copy for RPC_ASYNC_STATE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
impl ::core::clone::Clone for RPC_ASYNC_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_BINDING_HANDLE_OPTIONS_V1 {
    pub Version: u32,
    pub Flags: RPC_BINDING_HANDLE_OPTIONS_FLAGS,
    pub ComTimeout: u32,
    pub CallTimeout: u32,
}
impl ::core::marker::Copy for RPC_BINDING_HANDLE_OPTIONS_V1 {}
impl ::core::clone::Clone for RPC_BINDING_HANDLE_OPTIONS_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_BINDING_HANDLE_SECURITY_V1_A {
    pub Version: u32,
    pub ServerPrincName: *mut u8,
    pub AuthnLevel: u32,
    pub AuthnSvc: u32,
    pub AuthIdentity: *mut SEC_WINNT_AUTH_IDENTITY_A,
    pub SecurityQos: *mut RPC_SECURITY_QOS,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_BINDING_HANDLE_SECURITY_V1_A {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_BINDING_HANDLE_SECURITY_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_BINDING_HANDLE_SECURITY_V1_W {
    pub Version: u32,
    pub ServerPrincName: *mut u16,
    pub AuthnLevel: u32,
    pub AuthnSvc: u32,
    pub AuthIdentity: *mut SEC_WINNT_AUTH_IDENTITY_W,
    pub SecurityQos: *mut RPC_SECURITY_QOS,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_BINDING_HANDLE_SECURITY_V1_W {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_BINDING_HANDLE_SECURITY_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_BINDING_HANDLE_TEMPLATE_V1_A {
    pub Version: u32,
    pub Flags: u32,
    pub ProtocolSequence: u32,
    pub NetworkAddress: *mut u8,
    pub StringEndpoint: *mut u8,
    pub u1: RPC_BINDING_HANDLE_TEMPLATE_V1_A_0,
    pub ObjectUuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for RPC_BINDING_HANDLE_TEMPLATE_V1_A {}
impl ::core::clone::Clone for RPC_BINDING_HANDLE_TEMPLATE_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RPC_BINDING_HANDLE_TEMPLATE_V1_A_0 {
    pub Reserved: *mut u8,
}
impl ::core::marker::Copy for RPC_BINDING_HANDLE_TEMPLATE_V1_A_0 {}
impl ::core::clone::Clone for RPC_BINDING_HANDLE_TEMPLATE_V1_A_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_BINDING_HANDLE_TEMPLATE_V1_W {
    pub Version: u32,
    pub Flags: u32,
    pub ProtocolSequence: u32,
    pub NetworkAddress: *mut u16,
    pub StringEndpoint: *mut u16,
    pub u1: RPC_BINDING_HANDLE_TEMPLATE_V1_W_0,
    pub ObjectUuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for RPC_BINDING_HANDLE_TEMPLATE_V1_W {}
impl ::core::clone::Clone for RPC_BINDING_HANDLE_TEMPLATE_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RPC_BINDING_HANDLE_TEMPLATE_V1_W_0 {
    pub Reserved: *mut u16,
}
impl ::core::marker::Copy for RPC_BINDING_HANDLE_TEMPLATE_V1_W_0 {}
impl ::core::clone::Clone for RPC_BINDING_HANDLE_TEMPLATE_V1_W_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_BINDING_VECTOR {
    pub Count: u32,
    pub BindingH: [*mut ::core::ffi::c_void; 1],
}
impl ::core::marker::Copy for RPC_BINDING_VECTOR {}
impl ::core::clone::Clone for RPC_BINDING_VECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_CALL_ATTRIBUTES_V1_A {
    pub Version: u32,
    pub Flags: u32,
    pub ServerPrincipalNameBufferLength: u32,
    pub ServerPrincipalName: *mut u8,
    pub ClientPrincipalNameBufferLength: u32,
    pub ClientPrincipalName: *mut u8,
    pub AuthenticationLevel: u32,
    pub AuthenticationService: u32,
    pub NullSession: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_CALL_ATTRIBUTES_V1_A {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_CALL_ATTRIBUTES_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_CALL_ATTRIBUTES_V1_W {
    pub Version: u32,
    pub Flags: u32,
    pub ServerPrincipalNameBufferLength: u32,
    pub ServerPrincipalName: *mut u16,
    pub ClientPrincipalNameBufferLength: u32,
    pub ClientPrincipalName: *mut u16,
    pub AuthenticationLevel: u32,
    pub AuthenticationService: u32,
    pub NullSession: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_CALL_ATTRIBUTES_V1_W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_CALL_ATTRIBUTES_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_CALL_ATTRIBUTES_V2_A {
    pub Version: u32,
    pub Flags: u32,
    pub ServerPrincipalNameBufferLength: u32,
    pub ServerPrincipalName: *mut u8,
    pub ClientPrincipalNameBufferLength: u32,
    pub ClientPrincipalName: *mut u8,
    pub AuthenticationLevel: u32,
    pub AuthenticationService: u32,
    pub NullSession: super::super::Foundation::BOOL,
    pub KernelModeCaller: super::super::Foundation::BOOL,
    pub ProtocolSequence: u32,
    pub IsClientLocal: u32,
    pub ClientPID: super::super::Foundation::HANDLE,
    pub CallStatus: u32,
    pub CallType: RpcCallType,
    pub CallLocalAddress: *mut RPC_CALL_LOCAL_ADDRESS_V1,
    pub OpNum: u16,
    pub InterfaceUuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_CALL_ATTRIBUTES_V2_A {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_CALL_ATTRIBUTES_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_CALL_ATTRIBUTES_V2_W {
    pub Version: u32,
    pub Flags: u32,
    pub ServerPrincipalNameBufferLength: u32,
    pub ServerPrincipalName: *mut u16,
    pub ClientPrincipalNameBufferLength: u32,
    pub ClientPrincipalName: *mut u16,
    pub AuthenticationLevel: u32,
    pub AuthenticationService: u32,
    pub NullSession: super::super::Foundation::BOOL,
    pub KernelModeCaller: super::super::Foundation::BOOL,
    pub ProtocolSequence: u32,
    pub IsClientLocal: RpcCallClientLocality,
    pub ClientPID: super::super::Foundation::HANDLE,
    pub CallStatus: u32,
    pub CallType: RpcCallType,
    pub CallLocalAddress: *mut RPC_CALL_LOCAL_ADDRESS_V1,
    pub OpNum: u16,
    pub InterfaceUuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_CALL_ATTRIBUTES_V2_W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_CALL_ATTRIBUTES_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_CALL_ATTRIBUTES_V3_A {
    pub Version: u32,
    pub Flags: u32,
    pub ServerPrincipalNameBufferLength: u32,
    pub ServerPrincipalName: *mut u8,
    pub ClientPrincipalNameBufferLength: u32,
    pub ClientPrincipalName: *mut u8,
    pub AuthenticationLevel: u32,
    pub AuthenticationService: u32,
    pub NullSession: super::super::Foundation::BOOL,
    pub KernelModeCaller: super::super::Foundation::BOOL,
    pub ProtocolSequence: u32,
    pub IsClientLocal: u32,
    pub ClientPID: super::super::Foundation::HANDLE,
    pub CallStatus: u32,
    pub CallType: RpcCallType,
    pub CallLocalAddress: *mut RPC_CALL_LOCAL_ADDRESS_V1,
    pub OpNum: u16,
    pub InterfaceUuid: ::windows_sys::core::GUID,
    pub ClientIdentifierBufferLength: u32,
    pub ClientIdentifier: *mut u8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_CALL_ATTRIBUTES_V3_A {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_CALL_ATTRIBUTES_V3_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_CALL_ATTRIBUTES_V3_W {
    pub Version: u32,
    pub Flags: u32,
    pub ServerPrincipalNameBufferLength: u32,
    pub ServerPrincipalName: *mut u16,
    pub ClientPrincipalNameBufferLength: u32,
    pub ClientPrincipalName: *mut u16,
    pub AuthenticationLevel: u32,
    pub AuthenticationService: u32,
    pub NullSession: super::super::Foundation::BOOL,
    pub KernelModeCaller: super::super::Foundation::BOOL,
    pub ProtocolSequence: u32,
    pub IsClientLocal: RpcCallClientLocality,
    pub ClientPID: super::super::Foundation::HANDLE,
    pub CallStatus: u32,
    pub CallType: RpcCallType,
    pub CallLocalAddress: *mut RPC_CALL_LOCAL_ADDRESS_V1,
    pub OpNum: u16,
    pub InterfaceUuid: ::windows_sys::core::GUID,
    pub ClientIdentifierBufferLength: u32,
    pub ClientIdentifier: *mut u8,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_CALL_ATTRIBUTES_V3_W {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_CALL_ATTRIBUTES_V3_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_CALL_LOCAL_ADDRESS_V1 {
    pub Version: u32,
    pub Buffer: *mut ::core::ffi::c_void,
    pub BufferSize: u32,
    pub AddressFormat: RpcLocalAddressFormat,
}
impl ::core::marker::Copy for RPC_CALL_LOCAL_ADDRESS_V1 {}
impl ::core::clone::Clone for RPC_CALL_LOCAL_ADDRESS_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_CLIENT_INFORMATION1 {
    pub UserName: *mut u8,
    pub ComputerName: *mut u8,
    pub Privilege: u16,
    pub AuthFlags: u32,
}
impl ::core::marker::Copy for RPC_CLIENT_INFORMATION1 {}
impl ::core::clone::Clone for RPC_CLIENT_INFORMATION1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_CLIENT_INTERFACE {
    pub Length: u32,
    pub InterfaceId: RPC_SYNTAX_IDENTIFIER,
    pub TransferSyntax: RPC_SYNTAX_IDENTIFIER,
    pub DispatchTable: *mut RPC_DISPATCH_TABLE,
    pub RpcProtseqEndpointCount: u32,
    pub RpcProtseqEndpoint: *mut RPC_PROTSEQ_ENDPOINT,
    pub Reserved: usize,
    pub InterpreterInfo: *const ::core::ffi::c_void,
    pub Flags: u32,
}
impl ::core::marker::Copy for RPC_CLIENT_INTERFACE {}
impl ::core::clone::Clone for RPC_CLIENT_INTERFACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_C_OPT_COOKIE_AUTH_DESCRIPTOR {
    pub BufferSize: u32,
    pub Buffer: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for RPC_C_OPT_COOKIE_AUTH_DESCRIPTOR {}
impl ::core::clone::Clone for RPC_C_OPT_COOKIE_AUTH_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_DISPATCH_TABLE {
    pub DispatchTableCount: u32,
    pub DispatchTable: RPC_DISPATCH_FUNCTION,
    pub Reserved: isize,
}
impl ::core::marker::Copy for RPC_DISPATCH_TABLE {}
impl ::core::clone::Clone for RPC_DISPATCH_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_EE_INFO_PARAM {
    pub ParameterType: ExtendedErrorParamTypes,
    pub u: RPC_EE_INFO_PARAM_0,
}
impl ::core::marker::Copy for RPC_EE_INFO_PARAM {}
impl ::core::clone::Clone for RPC_EE_INFO_PARAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union RPC_EE_INFO_PARAM_0 {
    pub AnsiString: ::windows_sys::core::PSTR,
    pub UnicodeString: ::windows_sys::core::PWSTR,
    pub LVal: i32,
    pub SVal: i16,
    pub PVal: u64,
    pub BVal: BinaryParam,
}
impl ::core::marker::Copy for RPC_EE_INFO_PARAM_0 {}
impl ::core::clone::Clone for RPC_EE_INFO_PARAM_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_ENDPOINT_TEMPLATEA {
    pub Version: u32,
    pub ProtSeq: ::windows_sys::core::PSTR,
    pub Endpoint: ::windows_sys::core::PSTR,
    pub SecurityDescriptor: *mut ::core::ffi::c_void,
    pub Backlog: u32,
}
impl ::core::marker::Copy for RPC_ENDPOINT_TEMPLATEA {}
impl ::core::clone::Clone for RPC_ENDPOINT_TEMPLATEA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_ENDPOINT_TEMPLATEW {
    pub Version: u32,
    pub ProtSeq: ::windows_sys::core::PWSTR,
    pub Endpoint: ::windows_sys::core::PWSTR,
    pub SecurityDescriptor: *mut ::core::ffi::c_void,
    pub Backlog: u32,
}
impl ::core::marker::Copy for RPC_ENDPOINT_TEMPLATEW {}
impl ::core::clone::Clone for RPC_ENDPOINT_TEMPLATEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_ERROR_ENUM_HANDLE {
    pub Signature: u32,
    pub CurrentPos: *mut ::core::ffi::c_void,
    pub Head: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for RPC_ERROR_ENUM_HANDLE {}
impl ::core::clone::Clone for RPC_ERROR_ENUM_HANDLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RPC_EXTENDED_ERROR_INFO {
    pub Version: u32,
    pub ComputerName: ::windows_sys::core::PWSTR,
    pub ProcessID: u32,
    pub u: RPC_EXTENDED_ERROR_INFO_0,
    pub GeneratingComponent: u32,
    pub Status: u32,
    pub DetectionLocation: u16,
    pub Flags: u16,
    pub NumberOfParameters: i32,
    pub Parameters: [RPC_EE_INFO_PARAM; 4],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_EXTENDED_ERROR_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_EXTENDED_ERROR_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union RPC_EXTENDED_ERROR_INFO_0 {
    pub SystemTime: super::super::Foundation::SYSTEMTIME,
    pub FileTime: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RPC_EXTENDED_ERROR_INFO_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RPC_EXTENDED_ERROR_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_HTTP_TRANSPORT_CREDENTIALS_A {
    pub TransportCredentials: *mut SEC_WINNT_AUTH_IDENTITY_A,
    pub Flags: RPC_C_HTTP_FLAGS,
    pub AuthenticationTarget: RPC_C_HTTP_AUTHN_TARGET,
    pub NumberOfAuthnSchemes: u32,
    pub AuthnSchemes: *mut u32,
    pub ServerCertificateSubject: *mut u8,
}
impl ::core::marker::Copy for RPC_HTTP_TRANSPORT_CREDENTIALS_A {}
impl ::core::clone::Clone for RPC_HTTP_TRANSPORT_CREDENTIALS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_HTTP_TRANSPORT_CREDENTIALS_V2_A {
    pub TransportCredentials: *mut SEC_WINNT_AUTH_IDENTITY_A,
    pub Flags: RPC_C_HTTP_FLAGS,
    pub AuthenticationTarget: RPC_C_HTTP_AUTHN_TARGET,
    pub NumberOfAuthnSchemes: u32,
    pub AuthnSchemes: *mut u32,
    pub ServerCertificateSubject: *mut u8,
    pub ProxyCredentials: *mut SEC_WINNT_AUTH_IDENTITY_A,
    pub NumberOfProxyAuthnSchemes: u32,
    pub ProxyAuthnSchemes: *mut u32,
}
impl ::core::marker::Copy for RPC_HTTP_TRANSPORT_CREDENTIALS_V2_A {}
impl ::core::clone::Clone for RPC_HTTP_TRANSPORT_CREDENTIALS_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_HTTP_TRANSPORT_CREDENTIALS_V2_W {
    pub TransportCredentials: *mut SEC_WINNT_AUTH_IDENTITY_W,
    pub Flags: RPC_C_HTTP_FLAGS,
    pub AuthenticationTarget: RPC_C_HTTP_AUTHN_TARGET,
    pub NumberOfAuthnSchemes: u32,
    pub AuthnSchemes: *mut u32,
    pub ServerCertificateSubject: *mut u16,
    pub ProxyCredentials: *mut SEC_WINNT_AUTH_IDENTITY_W,
    pub NumberOfProxyAuthnSchemes: u32,
    pub ProxyAuthnSchemes: *mut u32,
}
impl ::core::marker::Copy for RPC_HTTP_TRANSPORT_CREDENTIALS_V2_W {}
impl ::core::clone::Clone for RPC_HTTP_TRANSPORT_CREDENTIALS_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_HTTP_TRANSPORT_CREDENTIALS_V3_A {
    pub TransportCredentials: *mut ::core::ffi::c_void,
    pub Flags: RPC_C_HTTP_FLAGS,
    pub AuthenticationTarget: RPC_C_HTTP_AUTHN_TARGET,
    pub NumberOfAuthnSchemes: u32,
    pub AuthnSchemes: *mut u32,
    pub ServerCertificateSubject: *mut u8,
    pub ProxyCredentials: *mut ::core::ffi::c_void,
    pub NumberOfProxyAuthnSchemes: u32,
    pub ProxyAuthnSchemes: *mut u32,
}
impl ::core::marker::Copy for RPC_HTTP_TRANSPORT_CREDENTIALS_V3_A {}
impl ::core::clone::Clone for RPC_HTTP_TRANSPORT_CREDENTIALS_V3_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_HTTP_TRANSPORT_CREDENTIALS_V3_W {
    pub TransportCredentials: *mut ::core::ffi::c_void,
    pub Flags: RPC_C_HTTP_FLAGS,
    pub AuthenticationTarget: RPC_C_HTTP_AUTHN_TARGET,
    pub NumberOfAuthnSchemes: u32,
    pub AuthnSchemes: *mut u32,
    pub ServerCertificateSubject: *mut u16,
    pub ProxyCredentials: *mut ::core::ffi::c_void,
    pub NumberOfProxyAuthnSchemes: u32,
    pub ProxyAuthnSchemes: *mut u32,
}
impl ::core::marker::Copy for RPC_HTTP_TRANSPORT_CREDENTIALS_V3_W {}
impl ::core::clone::Clone for RPC_HTTP_TRANSPORT_CREDENTIALS_V3_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_HTTP_TRANSPORT_CREDENTIALS_W {
    pub TransportCredentials: *mut SEC_WINNT_AUTH_IDENTITY_W,
    pub Flags: RPC_C_HTTP_FLAGS,
    pub AuthenticationTarget: RPC_C_HTTP_AUTHN_TARGET,
    pub NumberOfAuthnSchemes: u32,
    pub AuthnSchemes: *mut u32,
    pub ServerCertificateSubject: *mut u16,
}
impl ::core::marker::Copy for RPC_HTTP_TRANSPORT_CREDENTIALS_W {}
impl ::core::clone::Clone for RPC_HTTP_TRANSPORT_CREDENTIALS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_IF_ID {
    pub Uuid: ::windows_sys::core::GUID,
    pub VersMajor: u16,
    pub VersMinor: u16,
}
impl ::core::marker::Copy for RPC_IF_ID {}
impl ::core::clone::Clone for RPC_IF_ID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_IF_ID_VECTOR {
    pub Count: u32,
    pub IfId: [*mut RPC_IF_ID; 1],
}
impl ::core::marker::Copy for RPC_IF_ID_VECTOR {}
impl ::core::clone::Clone for RPC_IF_ID_VECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_IMPORT_CONTEXT_P {
    pub LookupContext: *mut ::core::ffi::c_void,
    pub ProposedHandle: *mut ::core::ffi::c_void,
    pub Bindings: *mut RPC_BINDING_VECTOR,
}
impl ::core::marker::Copy for RPC_IMPORT_CONTEXT_P {}
impl ::core::clone::Clone for RPC_IMPORT_CONTEXT_P {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_INTERFACE_TEMPLATEA {
    pub Version: u32,
    pub IfSpec: *mut ::core::ffi::c_void,
    pub MgrTypeUuid: *mut ::windows_sys::core::GUID,
    pub MgrEpv: *mut ::core::ffi::c_void,
    pub Flags: u32,
    pub MaxCalls: u32,
    pub MaxRpcSize: u32,
    pub IfCallback: RPC_IF_CALLBACK_FN,
    pub UuidVector: *mut UUID_VECTOR,
    pub Annotation: ::windows_sys::core::PSTR,
    pub SecurityDescriptor: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for RPC_INTERFACE_TEMPLATEA {}
impl ::core::clone::Clone for RPC_INTERFACE_TEMPLATEA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_INTERFACE_TEMPLATEW {
    pub Version: u32,
    pub IfSpec: *mut ::core::ffi::c_void,
    pub MgrTypeUuid: *mut ::windows_sys::core::GUID,
    pub MgrEpv: *mut ::core::ffi::c_void,
    pub Flags: u32,
    pub MaxCalls: u32,
    pub MaxRpcSize: u32,
    pub IfCallback: RPC_IF_CALLBACK_FN,
    pub UuidVector: *mut UUID_VECTOR,
    pub Annotation: ::windows_sys::core::PWSTR,
    pub SecurityDescriptor: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for RPC_INTERFACE_TEMPLATEW {}
impl ::core::clone::Clone for RPC_INTERFACE_TEMPLATEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_MESSAGE {
    pub Handle: *mut ::core::ffi::c_void,
    pub DataRepresentation: u32,
    pub Buffer: *mut ::core::ffi::c_void,
    pub BufferLength: u32,
    pub ProcNum: u32,
    pub TransferSyntax: *mut RPC_SYNTAX_IDENTIFIER,
    pub RpcInterfaceInformation: *mut ::core::ffi::c_void,
    pub ReservedForRuntime: *mut ::core::ffi::c_void,
    pub ManagerEpv: *mut ::core::ffi::c_void,
    pub ImportContext: *mut ::core::ffi::c_void,
    pub RpcFlags: u32,
}
impl ::core::marker::Copy for RPC_MESSAGE {}
impl ::core::clone::Clone for RPC_MESSAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_POLICY {
    pub Length: u32,
    pub EndpointFlags: u32,
    pub NICFlags: u32,
}
impl ::core::marker::Copy for RPC_POLICY {}
impl ::core::clone::Clone for RPC_POLICY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_PROTSEQ_ENDPOINT {
    pub RpcProtocolSequence: *mut u8,
    pub Endpoint: *mut u8,
}
impl ::core::marker::Copy for RPC_PROTSEQ_ENDPOINT {}
impl ::core::clone::Clone for RPC_PROTSEQ_ENDPOINT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_PROTSEQ_VECTORA {
    pub Count: u32,
    pub Protseq: [*mut u8; 1],
}
impl ::core::marker::Copy for RPC_PROTSEQ_VECTORA {}
impl ::core::clone::Clone for RPC_PROTSEQ_VECTORA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_PROTSEQ_VECTORW {
    pub Count: u32,
    pub Protseq: [*mut u16; 1],
}
impl ::core::marker::Copy for RPC_PROTSEQ_VECTORW {}
impl ::core::clone::Clone for RPC_PROTSEQ_VECTORW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V2_A {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V2_A_0,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V2_A {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V2_A_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_A,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V2_A_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V2_A_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V2_W {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V2_W_0,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V2_W {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V2_W_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_W,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V2_W_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V2_W_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V3_A {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V3_A_0,
    pub Sid: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V3_A {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V3_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V3_A_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_A,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V3_A_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V3_A_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V3_W {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V3_W_0,
    pub Sid: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V3_W {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V3_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V3_W_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_W,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V3_W_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V3_W_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V4_A {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V4_A_0,
    pub Sid: *mut ::core::ffi::c_void,
    pub EffectiveOnly: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V4_A {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V4_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V4_A_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_A,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V4_A_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V4_A_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V4_W {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V4_W_0,
    pub Sid: *mut ::core::ffi::c_void,
    pub EffectiveOnly: u32,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V4_W {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V4_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V4_W_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_W,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V4_W_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V4_W_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V5_A {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V5_A_0,
    pub Sid: *mut ::core::ffi::c_void,
    pub EffectiveOnly: u32,
    pub ServerSecurityDescriptor: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V5_A {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V5_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V5_A_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_A,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V5_A_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V5_A_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct RPC_SECURITY_QOS_V5_W {
    pub Version: u32,
    pub Capabilities: RPC_C_QOS_CAPABILITIES,
    pub IdentityTracking: RPC_C_QOS_IDENTITY,
    pub ImpersonationType: super::Com::RPC_C_IMP_LEVEL,
    pub AdditionalSecurityInfoType: RPC_C_AUTHN_INFO_TYPE,
    pub u: RPC_SECURITY_QOS_V5_W_0,
    pub Sid: *mut ::core::ffi::c_void,
    pub EffectiveOnly: u32,
    pub ServerSecurityDescriptor: *mut ::core::ffi::c_void,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V5_W {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V5_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub union RPC_SECURITY_QOS_V5_W_0 {
    pub HttpCredentials: *mut RPC_HTTP_TRANSPORT_CREDENTIALS_W,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for RPC_SECURITY_QOS_V5_W_0 {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for RPC_SECURITY_QOS_V5_W_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_SEC_CONTEXT_KEY_INFO {
    pub EncryptAlgorithm: u32,
    pub KeySize: u32,
    pub SignatureAlgorithm: u32,
}
impl ::core::marker::Copy for RPC_SEC_CONTEXT_KEY_INFO {}
impl ::core::clone::Clone for RPC_SEC_CONTEXT_KEY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_SERVER_INTERFACE {
    pub Length: u32,
    pub InterfaceId: RPC_SYNTAX_IDENTIFIER,
    pub TransferSyntax: RPC_SYNTAX_IDENTIFIER,
    pub DispatchTable: *mut RPC_DISPATCH_TABLE,
    pub RpcProtseqEndpointCount: u32,
    pub RpcProtseqEndpoint: *mut RPC_PROTSEQ_ENDPOINT,
    pub DefaultManagerEpv: *mut ::core::ffi::c_void,
    pub InterpreterInfo: *const ::core::ffi::c_void,
    pub Flags: u32,
}
impl ::core::marker::Copy for RPC_SERVER_INTERFACE {}
impl ::core::clone::Clone for RPC_SERVER_INTERFACE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_STATS_VECTOR {
    pub Count: u32,
    pub Stats: [u32; 1],
}
impl ::core::marker::Copy for RPC_STATS_VECTOR {}
impl ::core::clone::Clone for RPC_STATS_VECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_SYNTAX_IDENTIFIER {
    pub SyntaxGUID: ::windows_sys::core::GUID,
    pub SyntaxVersion: RPC_VERSION,
}
impl ::core::marker::Copy for RPC_SYNTAX_IDENTIFIER {}
impl ::core::clone::Clone for RPC_SYNTAX_IDENTIFIER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_TRANSFER_SYNTAX {
    pub Uuid: ::windows_sys::core::GUID,
    pub VersMajor: u16,
    pub VersMinor: u16,
}
impl ::core::marker::Copy for RPC_TRANSFER_SYNTAX {}
impl ::core::clone::Clone for RPC_TRANSFER_SYNTAX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RPC_VERSION {
    pub MajorVersion: u16,
    pub MinorVersion: u16,
}
impl ::core::marker::Copy for RPC_VERSION {}
impl ::core::clone::Clone for RPC_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCONTEXT_QUEUE {
    pub NumberOfObjects: u32,
    pub ArrayOfObjects: *mut *mut NDR_SCONTEXT,
}
impl ::core::marker::Copy for SCONTEXT_QUEUE {}
impl ::core::clone::Clone for SCONTEXT_QUEUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_WINNT_AUTH_IDENTITY_A {
    pub User: *mut u8,
    pub UserLength: u32,
    pub Domain: *mut u8,
    pub DomainLength: u32,
    pub Password: *mut u8,
    pub PasswordLength: u32,
    pub Flags: SEC_WINNT_AUTH_IDENTITY,
}
impl ::core::marker::Copy for SEC_WINNT_AUTH_IDENTITY_A {}
impl ::core::clone::Clone for SEC_WINNT_AUTH_IDENTITY_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SEC_WINNT_AUTH_IDENTITY_W {
    pub User: *mut u16,
    pub UserLength: u32,
    pub Domain: *mut u16,
    pub DomainLength: u32,
    pub Password: *mut u16,
    pub PasswordLength: u32,
    pub Flags: SEC_WINNT_AUTH_IDENTITY,
}
impl ::core::marker::Copy for SEC_WINNT_AUTH_IDENTITY_W {}
impl ::core::clone::Clone for SEC_WINNT_AUTH_IDENTITY_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct USER_MARSHAL_CB {
    pub Flags: u32,
    pub pStubMsg: *mut MIDL_STUB_MESSAGE,
    pub pReserve: *mut u8,
    pub Signature: u32,
    pub CBType: USER_MARSHAL_CB_TYPE,
    pub pFormat: *mut u8,
    pub pTypeFormat: *mut u8,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for USER_MARSHAL_CB {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for USER_MARSHAL_CB {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct USER_MARSHAL_ROUTINE_QUADRUPLE {
    pub pfnBufferSize: USER_MARSHAL_SIZING_ROUTINE,
    pub pfnMarshall: USER_MARSHAL_MARSHALLING_ROUTINE,
    pub pfnUnmarshall: USER_MARSHAL_UNMARSHALLING_ROUTINE,
    pub pfnFree: USER_MARSHAL_FREEING_ROUTINE,
}
impl ::core::marker::Copy for USER_MARSHAL_ROUTINE_QUADRUPLE {}
impl ::core::clone::Clone for USER_MARSHAL_ROUTINE_QUADRUPLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UUID_VECTOR {
    pub Count: u32,
    pub Uuid: [*mut ::windows_sys::core::GUID; 1],
}
impl ::core::marker::Copy for UUID_VECTOR {}
impl ::core::clone::Clone for UUID_VECTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub struct XMIT_ROUTINE_QUINTUPLE {
    pub pfnTranslateToXmit: XMIT_HELPER_ROUTINE,
    pub pfnTranslateFromXmit: XMIT_HELPER_ROUTINE,
    pub pfnFreeXmit: XMIT_HELPER_ROUTINE,
    pub pfnFreeInst: XMIT_HELPER_ROUTINE,
}
#[cfg(feature = "Win32_System_Com")]
impl ::core::marker::Copy for XMIT_ROUTINE_QUINTUPLE {}
#[cfg(feature = "Win32_System_Com")]
impl ::core::clone::Clone for XMIT_ROUTINE_QUINTUPLE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type _NDR_PROC_CONTEXT = isize;
pub type CS_TAG_GETTING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hbinding: *mut ::core::ffi::c_void, fserverside: i32, pulsendingtag: *mut u32, puldesiredreceivingtag: *mut u32, pulreceivingtag: *mut u32, pstatus: *mut u32) -> ()>;
pub type CS_TYPE_FROM_NETCS_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hbinding: *mut ::core::ffi::c_void, ulnetworkcodeset: u32, pnetworkdata: *mut u8, ulnetworkdatalength: u32, ullocalbuffersize: u32, plocaldata: *mut ::core::ffi::c_void, pullocaldatalength: *mut u32, pstatus: *mut u32) -> ()>;
pub type CS_TYPE_LOCAL_SIZE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hbinding: *mut ::core::ffi::c_void, ulnetworkcodeset: u32, ulnetworkbuffersize: u32, conversiontype: *mut IDL_CS_CONVERT, pullocalbuffersize: *mut u32, pstatus: *mut u32) -> ()>;
pub type CS_TYPE_NET_SIZE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hbinding: *mut ::core::ffi::c_void, ulnetworkcodeset: u32, ullocalbuffersize: u32, conversiontype: *mut IDL_CS_CONVERT, pulnetworkbuffersize: *mut u32, pstatus: *mut u32) -> ()>;
pub type CS_TYPE_TO_NETCS_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hbinding: *mut ::core::ffi::c_void, ulnetworkcodeset: u32, plocaldata: *mut ::core::ffi::c_void, ullocaldatalength: u32, pnetworkdata: *mut u8, pulnetworkdatalength: *mut u32, pstatus: *mut u32) -> ()>;
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub type EXPR_EVAL = ::core::option::Option<unsafe extern "system" fn(param0: *mut MIDL_STUB_MESSAGE) -> ()>;
pub type GENERIC_BINDING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void) -> *mut ::core::ffi::c_void>;
pub type GENERIC_UNBIND_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void, param1: *mut u8) -> ()>;
pub type I_RpcFreeCalloutStateFn = ::core::option::Option<unsafe extern "system" fn(calloutstate: *mut RDR_CALLOUT_STATE) -> ()>;
pub type I_RpcPerformCalloutFn = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void, calloutstate: *mut RDR_CALLOUT_STATE, stage: RPC_HTTP_REDIRECTOR_STAGE) -> RPC_STATUS>;
pub type I_RpcProxyFilterIfFn = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, ifuuid: *const ::windows_sys::core::GUID, ifmajorversion: u16, fallow: *mut i32) -> RPC_STATUS>;
pub type I_RpcProxyGetClientAddressFn = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void, buffer: ::windows_sys::core::PCSTR, bufferlength: *mut u32) -> RPC_STATUS>;
pub type I_RpcProxyGetClientSessionAndResourceUUID = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, sessionidpresent: *mut i32, sessionid: *mut ::windows_sys::core::GUID, resourceidpresent: *mut i32, resourceid: *mut ::windows_sys::core::GUID) -> RPC_STATUS>;
pub type I_RpcProxyGetConnectionTimeoutFn = ::core::option::Option<unsafe extern "system" fn(connectiontimeout: *mut u32) -> RPC_STATUS>;
pub type I_RpcProxyIsValidMachineFn = ::core::option::Option<unsafe extern "system" fn(machine: ::windows_sys::core::PCWSTR, dotmachine: ::windows_sys::core::PCWSTR, portnumber: u32) -> RPC_STATUS>;
pub type I_RpcProxyUpdatePerfCounterBackendServerFn = ::core::option::Option<unsafe extern "system" fn(machinename: *const u16, isconnectevent: i32) -> ()>;
pub type I_RpcProxyUpdatePerfCounterFn = ::core::option::Option<unsafe extern "system" fn(counter: RpcPerfCounters, modifytrend: i32, size: u32) -> ()>;
pub type MIDL_ES_ALLOC = ::core::option::Option<unsafe extern "system" fn(state: *mut ::core::ffi::c_void, pbuffer: *mut *mut i8, psize: *mut u32) -> ()>;
pub type MIDL_ES_READ = ::core::option::Option<unsafe extern "system" fn(state: *mut ::core::ffi::c_void, pbuffer: *mut *mut i8, psize: *mut u32) -> ()>;
pub type MIDL_ES_WRITE = ::core::option::Option<unsafe extern "system" fn(state: *mut ::core::ffi::c_void, buffer: ::windows_sys::core::PCSTR, size: u32) -> ()>;
pub type NDR_NOTIFY2_ROUTINE = ::core::option::Option<unsafe extern "system" fn(flag: u8) -> ()>;
pub type NDR_NOTIFY_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> ()>;
pub type NDR_RUNDOWN = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_IO\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_IO"))]
pub type PFN_RPCNOTIFICATION_ROUTINE = ::core::option::Option<unsafe extern "system" fn(pasync: *mut RPC_ASYNC_STATE, context: *mut ::core::ffi::c_void, event: RPC_ASYNC_EVENT) -> ()>;
pub type PFN_RPC_ALLOCATE = ::core::option::Option<unsafe extern "system" fn(param0: usize) -> *mut ::core::ffi::c_void>;
pub type PFN_RPC_FREE = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void) -> ()>;
pub type PRPC_RUNDOWN = ::core::option::Option<unsafe extern "system" fn(associationcontext: *mut ::core::ffi::c_void) -> ()>;
pub type RPCLT_PDU_FILTER_FUNC = ::core::option::Option<unsafe extern "system" fn(buffer: *mut ::core::ffi::c_void, bufferlength: u32, fdatagram: i32) -> ()>;
pub type RPC_ADDRESS_CHANGE_FN = ::core::option::Option<unsafe extern "system" fn(arg: *mut ::core::ffi::c_void) -> ()>;
pub type RPC_AUTH_KEY_RETRIEVAL_FN = ::core::option::Option<unsafe extern "system" fn(arg: *const ::core::ffi::c_void, serverprincname: ::windows_sys::core::PCWSTR, keyver: u32, key: *mut *mut ::core::ffi::c_void, status: *mut RPC_STATUS) -> ()>;
pub type RPC_BLOCKING_FN = ::core::option::Option<unsafe extern "system" fn(hwnd: *mut ::core::ffi::c_void, context: *mut ::core::ffi::c_void, hsyncevent: *mut ::core::ffi::c_void) -> RPC_STATUS>;
pub type RPC_CLIENT_ALLOC = ::core::option::Option<unsafe extern "system" fn(size: usize) -> *mut ::core::ffi::c_void>;
pub type RPC_CLIENT_FREE = ::core::option::Option<unsafe extern "system" fn(ptr: *const ::core::ffi::c_void) -> ()>;
pub type RPC_DISPATCH_FUNCTION = ::core::option::Option<unsafe extern "system" fn(message: *mut RPC_MESSAGE) -> ()>;
pub type RPC_FORWARD_FUNCTION = ::core::option::Option<unsafe extern "system" fn(interfaceid: *mut ::windows_sys::core::GUID, interfaceversion: *mut RPC_VERSION, objectid: *mut ::windows_sys::core::GUID, rpcpro: *mut u8, ppdestendpoint: *mut *mut ::core::ffi::c_void) -> RPC_STATUS>;
pub type RPC_HTTP_PROXY_FREE_STRING = ::core::option::Option<unsafe extern "system" fn(string: ::windows_sys::core::PCWSTR) -> ()>;
pub type RPC_IF_CALLBACK_FN = ::core::option::Option<unsafe extern "system" fn(interfaceuuid: *const ::core::ffi::c_void, context: *const ::core::ffi::c_void) -> RPC_STATUS>;
pub type RPC_INTERFACE_GROUP_IDLE_CALLBACK_FN = ::core::option::Option<unsafe extern "system" fn(ifgroup: *const ::core::ffi::c_void, idlecallbackcontext: *const ::core::ffi::c_void, isgroupidle: u32) -> ()>;
pub type RPC_MGMT_AUTHORIZATION_FN = ::core::option::Option<unsafe extern "system" fn(clientbinding: *const ::core::ffi::c_void, requestedmgmtoperation: u32, status: *mut RPC_STATUS) -> i32>;
pub type RPC_NEW_HTTP_PROXY_CHANNEL = ::core::option::Option<unsafe extern "system" fn(redirectorstage: RPC_HTTP_REDIRECTOR_STAGE, servername: ::windows_sys::core::PCWSTR, serverport: ::windows_sys::core::PCWSTR, remoteuser: ::windows_sys::core::PCWSTR, authtype: ::windows_sys::core::PCWSTR, resourceuuid: *mut ::core::ffi::c_void, sessionid: *mut ::core::ffi::c_void, interface: *const ::core::ffi::c_void, reserved: *const ::core::ffi::c_void, flags: u32, newservername: *mut ::windows_sys::core::PWSTR, newserverport: *mut ::windows_sys::core::PWSTR) -> RPC_STATUS>;
pub type RPC_OBJECT_INQ_FN = ::core::option::Option<unsafe extern "system" fn(objectuuid: *const ::windows_sys::core::GUID, typeuuid: *mut ::windows_sys::core::GUID, status: *mut RPC_STATUS) -> ()>;
pub type RPC_SECURITY_CALLBACK_FN = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type RPC_SETFILTER_FUNC = ::core::option::Option<unsafe extern "system" fn(pfnfilter: RPCLT_PDU_FILTER_FUNC) -> ()>;
pub type SERVER_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> i32>;
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub type STUB_THUNK = ::core::option::Option<unsafe extern "system" fn(param0: *mut MIDL_STUB_MESSAGE) -> ()>;
pub type USER_MARSHAL_FREEING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut u32, param1: *mut ::core::ffi::c_void) -> ()>;
pub type USER_MARSHAL_MARSHALLING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut u32, param1: *mut u8, param2: *mut ::core::ffi::c_void) -> *mut u8>;
pub type USER_MARSHAL_SIZING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut u32, param1: u32, param2: *mut ::core::ffi::c_void) -> u32>;
pub type USER_MARSHAL_UNMARSHALLING_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut u32, param1: *mut u8, param2: *mut ::core::ffi::c_void) -> *mut u8>;
#[doc = "Required features: `\"Win32_System_Com\"`"]
#[cfg(feature = "Win32_System_Com")]
pub type XMIT_HELPER_ROUTINE = ::core::option::Option<unsafe extern "system" fn(param0: *mut MIDL_STUB_MESSAGE) -> ()>;
