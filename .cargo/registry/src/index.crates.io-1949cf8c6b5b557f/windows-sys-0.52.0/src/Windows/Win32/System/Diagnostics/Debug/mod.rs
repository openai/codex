#[cfg(feature = "Win32_System_Diagnostics_Debug_Extensions")]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug_Extensions\"`"]
pub mod Extensions;
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn AddVectoredContinueHandler(first : u32, handler : PVECTORED_EXCEPTION_HANDLER) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn AddVectoredExceptionHandler(first : u32, handler : PVECTORED_EXCEPTION_HANDLER) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Beep(dwfreq : u32, dwduration : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BindImage(imagename : ::windows_sys::core::PCSTR, dllpath : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn BindImageEx(flags : u32, imagename : ::windows_sys::core::PCSTR, dllpath : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, statusroutine : PIMAGEHLP_STATUS_ROUTINE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CheckRemoteDebuggerPresent(hprocess : super::super::super::Foundation:: HANDLE, pbdebuggerpresent : *mut super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn CheckSumMappedFile(baseaddress : *const ::core::ffi::c_void, filelength : u32, headersum : *mut u32, checksum : *mut u32) -> *mut IMAGE_NT_HEADERS64);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn CheckSumMappedFile(baseaddress : *const ::core::ffi::c_void, filelength : u32, headersum : *mut u32, checksum : *mut u32) -> *mut IMAGE_NT_HEADERS32);
::windows_targets::link!("advapi32.dll" "system" fn CloseThreadWaitChainSession(wcthandle : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ContinueDebugEvent(dwprocessid : u32, dwthreadid : u32, dwcontinuestatus : super::super::super::Foundation:: NTSTATUS) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn CopyContext(destination : *mut CONTEXT, contextflags : CONTEXT_FLAGS, source : *const CONTEXT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DbgHelpCreateUserDump(filename : ::windows_sys::core::PCSTR, callback : PDBGHELP_CREATE_USER_DUMP_CALLBACK, userdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DbgHelpCreateUserDumpW(filename : ::windows_sys::core::PCWSTR, callback : PDBGHELP_CREATE_USER_DUMP_CALLBACK, userdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DebugActiveProcess(dwprocessid : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DebugActiveProcessStop(dwprocessid : u32) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn DebugBreak() -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DebugBreakProcess(process : super::super::super::Foundation:: HANDLE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DebugSetProcessKillOnExit(killonexit : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn DecodePointer(ptr : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-core-util-l1-1-1.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DecodeRemotePointer(processhandle : super::super::super::Foundation:: HANDLE, ptr : *const ::core::ffi::c_void, decodedptr : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn DecodeSystemPointer(ptr : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
::windows_targets::link!("kernel32.dll" "system" fn EncodePointer(ptr : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("api-ms-win-core-util-l1-1-1.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EncodeRemotePointer(processhandle : super::super::super::Foundation:: HANDLE, ptr : *const ::core::ffi::c_void, encodedptr : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn EncodeSystemPointer(ptr : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDirTree(hprocess : super::super::super::Foundation:: HANDLE, rootpath : ::windows_sys::core::PCSTR, inputpathname : ::windows_sys::core::PCSTR, outputpathbuffer : ::windows_sys::core::PSTR, cb : PENUMDIRTREE_CALLBACK, data : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDirTreeW(hprocess : super::super::super::Foundation:: HANDLE, rootpath : ::windows_sys::core::PCWSTR, inputpathname : ::windows_sys::core::PCWSTR, outputpathbuffer : ::windows_sys::core::PWSTR, cb : PENUMDIRTREE_CALLBACKW, data : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumerateLoadedModules(hprocess : super::super::super::Foundation:: HANDLE, enumloadedmodulescallback : PENUMLOADED_MODULES_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumerateLoadedModules64(hprocess : super::super::super::Foundation:: HANDLE, enumloadedmodulescallback : PENUMLOADED_MODULES_CALLBACK64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumerateLoadedModulesEx(hprocess : super::super::super::Foundation:: HANDLE, enumloadedmodulescallback : PENUMLOADED_MODULES_CALLBACK64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumerateLoadedModulesExW(hprocess : super::super::super::Foundation:: HANDLE, enumloadedmodulescallback : PENUMLOADED_MODULES_CALLBACKW64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumerateLoadedModulesW64(hprocess : super::super::super::Foundation:: HANDLE, enumloadedmodulescallback : PENUMLOADED_MODULES_CALLBACKW64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn FatalAppExitA(uaction : u32, lpmessagetext : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("kernel32.dll" "system" fn FatalAppExitW(uaction : u32, lpmessagetext : ::windows_sys::core::PCWSTR) -> ());
::windows_targets::link!("kernel32.dll" "system" fn FatalExit(exitcode : i32) -> !);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindDebugInfoFile(filename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, debugfilepath : ::windows_sys::core::PSTR) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindDebugInfoFileEx(filename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, debugfilepath : ::windows_sys::core::PSTR, callback : PFIND_DEBUG_FILE_CALLBACK, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindDebugInfoFileExW(filename : ::windows_sys::core::PCWSTR, symbolpath : ::windows_sys::core::PCWSTR, debugfilepath : ::windows_sys::core::PWSTR, callback : PFIND_DEBUG_FILE_CALLBACKW, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindExecutableImage(filename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, imagefilepath : ::windows_sys::core::PSTR) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindExecutableImageEx(filename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, imagefilepath : ::windows_sys::core::PSTR, callback : PFIND_EXE_FILE_CALLBACK, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindExecutableImageExW(filename : ::windows_sys::core::PCWSTR, symbolpath : ::windows_sys::core::PCWSTR, imagefilepath : ::windows_sys::core::PWSTR, callback : PFIND_EXE_FILE_CALLBACKW, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindFileInPath(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, id : *const ::core::ffi::c_void, two : u32, three : u32, flags : u32, filepath : ::windows_sys::core::PSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindFileInSearchPath(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, one : u32, two : u32, three : u32, filepath : ::windows_sys::core::PSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FlushInstructionCache(hprocess : super::super::super::Foundation:: HANDLE, lpbaseaddress : *const ::core::ffi::c_void, dwsize : usize) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn FormatMessageA(dwflags : FORMAT_MESSAGE_OPTIONS, lpsource : *const ::core::ffi::c_void, dwmessageid : u32, dwlanguageid : u32, lpbuffer : ::windows_sys::core::PSTR, nsize : u32, arguments : *const *const i8) -> u32);
::windows_targets::link!("kernel32.dll" "system" fn FormatMessageW(dwflags : FORMAT_MESSAGE_OPTIONS, lpsource : *const ::core::ffi::c_void, dwmessageid : u32, dwlanguageid : u32, lpbuffer : ::windows_sys::core::PWSTR, nsize : u32, arguments : *const *const i8) -> u32);
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
::windows_targets::link!("kernel32.dll" "system" fn GetEnabledXStateFeatures() -> u64);
::windows_targets::link!("kernel32.dll" "system" fn GetErrorMode() -> u32);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn GetImageConfigInformation(loadedimage : *const LOADED_IMAGE, imageconfiginformation : *mut IMAGE_LOAD_CONFIG_DIRECTORY64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn GetImageConfigInformation(loadedimage : *const LOADED_IMAGE, imageconfiginformation : *mut IMAGE_LOAD_CONFIG_DIRECTORY32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn GetImageUnusedHeaderBytes(loadedimage : *const LOADED_IMAGE, sizeunusedheaderbytes : *mut u32) -> u32);
::windows_targets::link!("dbghelp.dll" "system" fn GetSymLoadError() -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn GetThreadContext(hthread : super::super::super::Foundation:: HANDLE, lpcontext : *mut CONTEXT) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetThreadErrorMode() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetThreadSelectorEntry(hthread : super::super::super::Foundation:: HANDLE, dwselector : u32, lpselectorentry : *mut LDT_ENTRY) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetThreadWaitChain(wcthandle : *const ::core::ffi::c_void, context : usize, flags : WAIT_CHAIN_THREAD_OPTIONS, threadid : u32, nodecount : *mut u32, nodeinfoarray : *mut WAITCHAIN_NODE_INFO, iscycle : *mut super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTimestampForLoadedLibrary(module : super::super::super::Foundation:: HMODULE) -> u32);
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn GetXStateFeaturesMask(context : *const CONTEXT, featuremask : *mut u64) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_WinTrust"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_WinTrust\"`"] fn ImageAddCertificate(filehandle : super::super::super::Foundation:: HANDLE, certificate : *const super::super::super::Security::WinTrust:: WIN_CERTIFICATE, index : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImageDirectoryEntryToData(base : *const ::core::ffi::c_void, mappedasimage : super::super::super::Foundation:: BOOLEAN, directoryentry : IMAGE_DIRECTORY_ENTRY, size : *mut u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImageDirectoryEntryToDataEx(base : *const ::core::ffi::c_void, mappedasimage : super::super::super::Foundation:: BOOLEAN, directoryentry : IMAGE_DIRECTORY_ENTRY, size : *mut u32, foundheader : *mut *mut IMAGE_SECTION_HEADER) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImageEnumerateCertificates(filehandle : super::super::super::Foundation:: HANDLE, typefilter : u16, certificatecount : *mut u32, indices : *mut u32, indexcount : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_WinTrust"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_WinTrust\"`"] fn ImageGetCertificateData(filehandle : super::super::super::Foundation:: HANDLE, certificateindex : u32, certificate : *mut super::super::super::Security::WinTrust:: WIN_CERTIFICATE, requiredlength : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security_WinTrust"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security_WinTrust\"`"] fn ImageGetCertificateHeader(filehandle : super::super::super::Foundation:: HANDLE, certificateindex : u32, certificateheader : *mut super::super::super::Security::WinTrust:: WIN_CERTIFICATE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImageGetDigestStream(filehandle : super::super::super::Foundation:: HANDLE, digestlevel : u32, digestfunction : DIGEST_FUNCTION, digesthandle : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn ImageLoad(dllname : ::windows_sys::core::PCSTR, dllpath : ::windows_sys::core::PCSTR) -> *mut LOADED_IMAGE);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn ImageNtHeader(base : *const ::core::ffi::c_void) -> *mut IMAGE_NT_HEADERS64);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn ImageNtHeader(base : *const ::core::ffi::c_void) -> *mut IMAGE_NT_HEADERS32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ImageRemoveCertificate(filehandle : super::super::super::Foundation:: HANDLE, index : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn ImageRvaToSection(ntheaders : *const IMAGE_NT_HEADERS64, base : *const ::core::ffi::c_void, rva : u32) -> *mut IMAGE_SECTION_HEADER);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn ImageRvaToSection(ntheaders : *const IMAGE_NT_HEADERS32, base : *const ::core::ffi::c_void, rva : u32) -> *mut IMAGE_SECTION_HEADER);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn ImageRvaToVa(ntheaders : *const IMAGE_NT_HEADERS64, base : *const ::core::ffi::c_void, rva : u32, lastrvasection : *const *const IMAGE_SECTION_HEADER) -> *mut ::core::ffi::c_void);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_SystemInformation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_System_SystemInformation\"`"] fn ImageRvaToVa(ntheaders : *const IMAGE_NT_HEADERS32, base : *const ::core::ffi::c_void, rva : u32, lastrvasection : *const *const IMAGE_SECTION_HEADER) -> *mut ::core::ffi::c_void);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn ImageUnload(loadedimage : *mut LOADED_IMAGE) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("dbghelp.dll" "system" fn ImagehlpApiVersion() -> *mut API_VERSION);
::windows_targets::link!("dbghelp.dll" "system" fn ImagehlpApiVersionEx(appversion : *const API_VERSION) -> *mut API_VERSION);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn InitializeContext(buffer : *mut ::core::ffi::c_void, contextflags : CONTEXT_FLAGS, context : *mut *mut CONTEXT, contextlength : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn InitializeContext2(buffer : *mut ::core::ffi::c_void, contextflags : CONTEXT_FLAGS, context : *mut *mut CONTEXT, contextlength : *mut u32, xstatecompactionmask : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDebuggerPresent() -> super::super::super::Foundation:: BOOL);
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Kernel")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_System_Kernel\"`"] fn LocateXStateFeature(context : *const CONTEXT, featureid : u32, length : *mut u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MakeSureDirectoryPathExists(dirpath : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn MapAndLoad(imagename : ::windows_sys::core::PCSTR, dllpath : ::windows_sys::core::PCSTR, loadedimage : *mut LOADED_IMAGE, dotdll : super::super::super::Foundation:: BOOL, readonly : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("imagehlp.dll" "system" fn MapFileAndCheckSumA(filename : ::windows_sys::core::PCSTR, headersum : *mut u32, checksum : *mut u32) -> u32);
::windows_targets::link!("imagehlp.dll" "system" fn MapFileAndCheckSumW(filename : ::windows_sys::core::PCWSTR, headersum : *mut u32, checksum : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn MessageBeep(utype : super::super::super::UI::WindowsAndMessaging:: MESSAGEBOX_STYLE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn MiniDumpReadDumpStream(baseofdump : *const ::core::ffi::c_void, streamnumber : u32, dir : *mut *mut MINIDUMP_DIRECTORY, streampointer : *mut *mut ::core::ffi::c_void, streamsize : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Memory\"`"] fn MiniDumpWriteDump(hprocess : super::super::super::Foundation:: HANDLE, processid : u32, hfile : super::super::super::Foundation:: HANDLE, dumptype : MINIDUMP_TYPE, exceptionparam : *const MINIDUMP_EXCEPTION_INFORMATION, userstreamparam : *const MINIDUMP_USER_STREAM_INFORMATION, callbackparam : *const MINIDUMP_CALLBACK_INFORMATION) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn OpenThreadWaitChainSession(flags : OPEN_THREAD_WAIT_CHAIN_SESSION_FLAGS, callback : PWAITCHAINCALLBACK) -> *mut ::core::ffi::c_void);
::windows_targets::link!("kernel32.dll" "system" fn OutputDebugStringA(lpoutputstring : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("kernel32.dll" "system" fn OutputDebugStringW(lpoutputstring : ::windows_sys::core::PCWSTR) -> ());
::windows_targets::link!("kernel32.dll" "system" fn RaiseException(dwexceptioncode : u32, dwexceptionflags : u32, nnumberofarguments : u32, lparguments : *const usize) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RaiseFailFastException(pexceptionrecord : *const EXCEPTION_RECORD, pcontextrecord : *const CONTEXT, dwflags : u32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RangeMapAddPeImageSections(rmaphandle : *const ::core::ffi::c_void, imagename : ::windows_sys::core::PCWSTR, mappedimage : *const ::core::ffi::c_void, mappingbytes : u32, imagebase : u64, usertag : u64, mappingflags : u32) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("dbghelp.dll" "system" fn RangeMapCreate() -> *mut ::core::ffi::c_void);
::windows_targets::link!("dbghelp.dll" "system" fn RangeMapFree(rmaphandle : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RangeMapRead(rmaphandle : *const ::core::ffi::c_void, offset : u64, buffer : *mut ::core::ffi::c_void, requestbytes : u32, flags : u32, donebytes : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RangeMapRemove(rmaphandle : *const ::core::ffi::c_void, usertag : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RangeMapWrite(rmaphandle : *const ::core::ffi::c_void, offset : u64, buffer : *const ::core::ffi::c_void, requestbytes : u32, flags : u32, donebytes : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReBaseImage(currentimagename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, frebase : super::super::super::Foundation:: BOOL, frebasesysfileok : super::super::super::Foundation:: BOOL, fgoingdown : super::super::super::Foundation:: BOOL, checkimagesize : u32, oldimagesize : *mut u32, oldimagebase : *mut usize, newimagesize : *mut u32, newimagebase : *mut usize, timestamp : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReBaseImage64(currentimagename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, frebase : super::super::super::Foundation:: BOOL, frebasesysfileok : super::super::super::Foundation:: BOOL, fgoingdown : super::super::super::Foundation:: BOOL, checkimagesize : u32, oldimagesize : *mut u32, oldimagebase : *mut u64, newimagesize : *mut u32, newimagebase : *mut u64, timestamp : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReadProcessMemory(hprocess : super::super::super::Foundation:: HANDLE, lpbaseaddress : *const ::core::ffi::c_void, lpbuffer : *mut ::core::ffi::c_void, nsize : usize, lpnumberofbytesread : *mut usize) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("advapi32.dll" "system" fn RegisterWaitChainCOMCallback(callstatecallback : PCOGETCALLSTATE, activationstatecallback : PCOGETACTIVATIONSTATE) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemoveInvalidModuleList(hprocess : super::super::super::Foundation:: HANDLE) -> ());
::windows_targets::link!("kernel32.dll" "system" fn RemoveVectoredContinueHandler(handle : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("kernel32.dll" "system" fn RemoveVectoredExceptionHandler(handle : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ReportSymbolLoadSummary(hprocess : super::super::super::Foundation:: HANDLE, ploadmodule : ::windows_sys::core::PCWSTR, psymboldata : *const DBGHELP_DATA_REPORT_STRUCT) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlAddFunctionTable(functiontable : *const IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY, entrycount : u32, baseaddress : usize) -> super::super::super::Foundation:: BOOLEAN);
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlAddFunctionTable(functiontable : *const IMAGE_RUNTIME_FUNCTION_ENTRY, entrycount : u32, baseaddress : u64) -> super::super::super::Foundation:: BOOLEAN);
#[cfg(target_arch = "aarch64")]
::windows_targets::link!("ntdll.dll" "system" fn RtlAddGrowableFunctionTable(dynamictable : *mut *mut ::core::ffi::c_void, functiontable : *const IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY, entrycount : u32, maximumentrycount : u32, rangebase : usize, rangeend : usize) -> u32);
#[cfg(target_arch = "x86_64")]
::windows_targets::link!("ntdll.dll" "system" fn RtlAddGrowableFunctionTable(dynamictable : *mut *mut ::core::ffi::c_void, functiontable : *const IMAGE_RUNTIME_FUNCTION_ENTRY, entrycount : u32, maximumentrycount : u32, rangebase : usize, rangeend : usize) -> u32);
#[cfg(feature = "Win32_System_Kernel")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_System_Kernel\"`"] fn RtlCaptureContext(contextrecord : *mut CONTEXT) -> ());
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_System_Kernel\"`"] fn RtlCaptureContext2(contextrecord : *mut CONTEXT) -> ());
::windows_targets::link!("kernel32.dll" "system" fn RtlCaptureStackBackTrace(framestoskip : u32, framestocapture : u32, backtrace : *mut *mut ::core::ffi::c_void, backtracehash : *mut u32) -> u16);
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDeleteFunctionTable(functiontable : *const IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY) -> super::super::super::Foundation:: BOOLEAN);
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlDeleteFunctionTable(functiontable : *const IMAGE_RUNTIME_FUNCTION_ENTRY) -> super::super::super::Foundation:: BOOLEAN);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
::windows_targets::link!("ntdll.dll" "system" fn RtlDeleteGrowableFunctionTable(dynamictable : *const ::core::ffi::c_void) -> ());
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
::windows_targets::link!("ntdll.dll" "system" fn RtlGrowFunctionTable(dynamictable : *mut ::core::ffi::c_void, newentrycount : u32) -> ());
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlInstallFunctionTableCallback(tableidentifier : u64, baseaddress : u64, length : u32, callback : PGET_RUNTIME_FUNCTION_CALLBACK, context : *const ::core::ffi::c_void, outofprocesscallbackdll : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOLEAN);
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlInstallFunctionTableCallback(tableidentifier : u64, baseaddress : u64, length : u32, callback : PGET_RUNTIME_FUNCTION_CALLBACK, context : *const ::core::ffi::c_void, outofprocesscallbackdll : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOLEAN);
#[cfg(target_arch = "aarch64")]
::windows_targets::link!("kernel32.dll" "system" fn RtlLookupFunctionEntry(controlpc : usize, imagebase : *mut usize, historytable : *mut UNWIND_HISTORY_TABLE) -> *mut IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY);
#[cfg(target_arch = "x86_64")]
::windows_targets::link!("kernel32.dll" "system" fn RtlLookupFunctionEntry(controlpc : u64, imagebase : *mut u64, historytable : *mut UNWIND_HISTORY_TABLE) -> *mut IMAGE_RUNTIME_FUNCTION_ENTRY);
::windows_targets::link!("kernel32.dll" "system" fn RtlPcToFileHeader(pcvalue : *const ::core::ffi::c_void, baseofimage : *mut *mut ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlRaiseException(exceptionrecord : *const EXCEPTION_RECORD) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "cdecl" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlRestoreContext(contextrecord : *const CONTEXT, exceptionrecord : *const EXCEPTION_RECORD) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RtlUnwind(targetframe : *const ::core::ffi::c_void, targetip : *const ::core::ffi::c_void, exceptionrecord : *const EXCEPTION_RECORD, returnvalue : *const ::core::ffi::c_void) -> ());
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlUnwindEx(targetframe : *const ::core::ffi::c_void, targetip : *const ::core::ffi::c_void, exceptionrecord : *const EXCEPTION_RECORD, returnvalue : *const ::core::ffi::c_void, contextrecord : *const CONTEXT, historytable : *const UNWIND_HISTORY_TABLE) -> ());
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlVirtualUnwind(handlertype : RTL_VIRTUAL_UNWIND_HANDLER_TYPE, imagebase : usize, controlpc : usize, functionentry : *const IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY, contextrecord : *mut CONTEXT, handlerdata : *mut *mut ::core::ffi::c_void, establisherframe : *mut usize, contextpointers : *mut KNONVOLATILE_CONTEXT_POINTERS_ARM64) -> super::super::Kernel:: EXCEPTION_ROUTINE);
#[cfg(target_arch = "x86_64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn RtlVirtualUnwind(handlertype : RTL_VIRTUAL_UNWIND_HANDLER_TYPE, imagebase : u64, controlpc : u64, functionentry : *const IMAGE_RUNTIME_FUNCTION_ENTRY, contextrecord : *mut CONTEXT, handlerdata : *mut *mut ::core::ffi::c_void, establisherframe : *mut u64, contextpointers : *mut KNONVOLATILE_CONTEXT_POINTERS) -> super::super::Kernel:: EXCEPTION_ROUTINE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SearchTreeForFile(rootpath : ::windows_sys::core::PCSTR, inputpathname : ::windows_sys::core::PCSTR, outputpathbuffer : ::windows_sys::core::PSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SearchTreeForFileW(rootpath : ::windows_sys::core::PCWSTR, inputpathname : ::windows_sys::core::PCWSTR, outputpathbuffer : ::windows_sys::core::PWSTR) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("dbghelp.dll" "system" fn SetCheckUserInterruptShared(lpstartaddress : LPCALL_BACK_USER_INTERRUPT_ROUTINE) -> ());
::windows_targets::link!("kernel32.dll" "system" fn SetErrorMode(umode : THREAD_ERROR_MODE) -> u32);
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn SetImageConfigInformation(loadedimage : *mut LOADED_IMAGE, imageconfiginformation : *const IMAGE_LOAD_CONFIG_DIRECTORY64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn SetImageConfigInformation(loadedimage : *mut LOADED_IMAGE, imageconfiginformation : *const IMAGE_LOAD_CONFIG_DIRECTORY32) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("dbghelp.dll" "system" fn SetSymLoadError(error : u32) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn SetThreadContext(hthread : super::super::super::Foundation:: HANDLE, lpcontext : *const CONTEXT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetThreadErrorMode(dwnewmode : THREAD_ERROR_MODE, lpoldmode : *mut THREAD_ERROR_MODE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn SetUnhandledExceptionFilter(lptoplevelexceptionfilter : LPTOP_LEVEL_EXCEPTION_FILTER) -> LPTOP_LEVEL_EXCEPTION_FILTER);
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn SetXStateFeaturesMask(context : *mut CONTEXT, featuremask : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn StackWalk(machinetype : u32, hprocess : super::super::super::Foundation:: HANDLE, hthread : super::super::super::Foundation:: HANDLE, stackframe : *mut STACKFRAME, contextrecord : *mut ::core::ffi::c_void, readmemoryroutine : PREAD_PROCESS_MEMORY_ROUTINE, functiontableaccessroutine : PFUNCTION_TABLE_ACCESS_ROUTINE, getmodulebaseroutine : PGET_MODULE_BASE_ROUTINE, translateaddress : PTRANSLATE_ADDRESS_ROUTINE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn StackWalk2(machinetype : u32, hprocess : super::super::super::Foundation:: HANDLE, hthread : super::super::super::Foundation:: HANDLE, stackframe : *mut STACKFRAME_EX, contextrecord : *mut ::core::ffi::c_void, readmemoryroutine : PREAD_PROCESS_MEMORY_ROUTINE64, functiontableaccessroutine : PFUNCTION_TABLE_ACCESS_ROUTINE64, getmodulebaseroutine : PGET_MODULE_BASE_ROUTINE64, translateaddress : PTRANSLATE_ADDRESS_ROUTINE64, gettargetattributevalue : PGET_TARGET_ATTRIBUTE_VALUE64, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn StackWalk64(machinetype : u32, hprocess : super::super::super::Foundation:: HANDLE, hthread : super::super::super::Foundation:: HANDLE, stackframe : *mut STACKFRAME64, contextrecord : *mut ::core::ffi::c_void, readmemoryroutine : PREAD_PROCESS_MEMORY_ROUTINE64, functiontableaccessroutine : PFUNCTION_TABLE_ACCESS_ROUTINE64, getmodulebaseroutine : PGET_MODULE_BASE_ROUTINE64, translateaddress : PTRANSLATE_ADDRESS_ROUTINE64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn StackWalkEx(machinetype : u32, hprocess : super::super::super::Foundation:: HANDLE, hthread : super::super::super::Foundation:: HANDLE, stackframe : *mut STACKFRAME_EX, contextrecord : *mut ::core::ffi::c_void, readmemoryroutine : PREAD_PROCESS_MEMORY_ROUTINE64, functiontableaccessroutine : PFUNCTION_TABLE_ACCESS_ROUTINE64, getmodulebaseroutine : PGET_MODULE_BASE_ROUTINE64, translateaddress : PTRANSLATE_ADDRESS_ROUTINE64, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymAddSourceStream(hprocess : super::super::super::Foundation:: HANDLE, base : u64, streamfile : ::windows_sys::core::PCSTR, buffer : *const u8, size : usize) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymAddSourceStreamA(hprocess : super::super::super::Foundation:: HANDLE, base : u64, streamfile : ::windows_sys::core::PCSTR, buffer : *const u8, size : usize) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymAddSourceStreamW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCWSTR, buffer : *const u8, size : usize) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymAddSymbol(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, name : ::windows_sys::core::PCSTR, address : u64, size : u32, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymAddSymbolW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, name : ::windows_sys::core::PCWSTR, address : u64, size : u32, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymAddrIncludeInlineTrace(hprocess : super::super::super::Foundation:: HANDLE, address : u64) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymCleanup(hprocess : super::super::super::Foundation:: HANDLE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymCompareInlineTrace(hprocess : super::super::super::Foundation:: HANDLE, address1 : u64, inlinecontext1 : u32, retaddress1 : u64, address2 : u64, retaddress2 : u64) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymDeleteSymbol(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, name : ::windows_sys::core::PCSTR, address : u64, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymDeleteSymbolW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, name : ::windows_sys::core::PCWSTR, address : u64, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumLines(hprocess : super::super::super::Foundation:: HANDLE, base : u64, obj : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR, enumlinescallback : PSYM_ENUMLINES_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumLinesW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, obj : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR, enumlinescallback : PSYM_ENUMLINES_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumProcesses(enumprocessescallback : PSYM_ENUMPROCESSES_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSourceFileTokens(hprocess : super::super::super::Foundation:: HANDLE, base : u64, callback : PENUMSOURCEFILETOKENSCALLBACK) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSourceFiles(hprocess : super::super::super::Foundation:: HANDLE, modbase : u64, mask : ::windows_sys::core::PCSTR, cbsrcfiles : PSYM_ENUMSOURCEFILES_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSourceFilesW(hprocess : super::super::super::Foundation:: HANDLE, modbase : u64, mask : ::windows_sys::core::PCWSTR, cbsrcfiles : PSYM_ENUMSOURCEFILES_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSourceLines(hprocess : super::super::super::Foundation:: HANDLE, base : u64, obj : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR, line : u32, flags : u32, enumlinescallback : PSYM_ENUMLINES_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSourceLinesW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, obj : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR, line : u32, flags : u32, enumlinescallback : PSYM_ENUMLINES_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSym(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSymbols(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, mask : ::windows_sys::core::PCSTR, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSymbolsEx(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, mask : ::windows_sys::core::PCSTR, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void, options : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSymbolsExW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, mask : ::windows_sys::core::PCWSTR, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void, options : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSymbolsForAddr(hprocess : super::super::super::Foundation:: HANDLE, address : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSymbolsForAddrW(hprocess : super::super::super::Foundation:: HANDLE, address : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumSymbolsW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, mask : ::windows_sys::core::PCWSTR, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumTypes(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumTypesByName(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, mask : ::windows_sys::core::PCSTR, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumTypesByNameW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, mask : ::windows_sys::core::PCWSTR, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumTypesW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateModules(hprocess : super::super::super::Foundation:: HANDLE, enummodulescallback : PSYM_ENUMMODULES_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateModules64(hprocess : super::super::super::Foundation:: HANDLE, enummodulescallback : PSYM_ENUMMODULES_CALLBACK64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateModulesW64(hprocess : super::super::super::Foundation:: HANDLE, enummodulescallback : PSYM_ENUMMODULES_CALLBACKW64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateSymbols(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u32, enumsymbolscallback : PSYM_ENUMSYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateSymbols64(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, enumsymbolscallback : PSYM_ENUMSYMBOLS_CALLBACK64, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateSymbolsW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u32, enumsymbolscallback : PSYM_ENUMSYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymEnumerateSymbolsW64(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, enumsymbolscallback : PSYM_ENUMSYMBOLS_CALLBACK64W, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFindDebugInfoFile(hprocess : super::super::super::Foundation:: HANDLE, filename : ::windows_sys::core::PCSTR, debugfilepath : ::windows_sys::core::PSTR, callback : PFIND_DEBUG_FILE_CALLBACK, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFindDebugInfoFileW(hprocess : super::super::super::Foundation:: HANDLE, filename : ::windows_sys::core::PCWSTR, debugfilepath : ::windows_sys::core::PWSTR, callback : PFIND_DEBUG_FILE_CALLBACKW, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFindExecutableImage(hprocess : super::super::super::Foundation:: HANDLE, filename : ::windows_sys::core::PCSTR, imagefilepath : ::windows_sys::core::PSTR, callback : PFIND_EXE_FILE_CALLBACK, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFindExecutableImageW(hprocess : super::super::super::Foundation:: HANDLE, filename : ::windows_sys::core::PCWSTR, imagefilepath : ::windows_sys::core::PWSTR, callback : PFIND_EXE_FILE_CALLBACKW, callerdata : *const ::core::ffi::c_void) -> super::super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFindFileInPath(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, id : *const ::core::ffi::c_void, two : u32, three : u32, flags : SYM_FIND_ID_OPTION, foundfile : ::windows_sys::core::PSTR, callback : PFINDFILEINPATHCALLBACK, context : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFindFileInPathW(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PCWSTR, filename : ::windows_sys::core::PCWSTR, id : *const ::core::ffi::c_void, two : u32, three : u32, flags : SYM_FIND_ID_OPTION, foundfile : ::windows_sys::core::PWSTR, callback : PFINDFILEINPATHCALLBACKW, context : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromAddr(hprocess : super::super::super::Foundation:: HANDLE, address : u64, displacement : *mut u64, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromAddrW(hprocess : super::super::super::Foundation:: HANDLE, address : u64, displacement : *mut u64, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromIndex(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromIndexW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromInlineContext(hprocess : super::super::super::Foundation:: HANDLE, address : u64, inlinecontext : u32, displacement : *mut u64, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromInlineContextW(hprocess : super::super::super::Foundation:: HANDLE, address : u64, inlinecontext : u32, displacement : *mut u64, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromName(hprocess : super::super::super::Foundation:: HANDLE, name : ::windows_sys::core::PCSTR, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromNameW(hprocess : super::super::super::Foundation:: HANDLE, name : ::windows_sys::core::PCWSTR, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromToken(hprocess : super::super::super::Foundation:: HANDLE, base : u64, token : u32, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFromTokenW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, token : u32, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFunctionTableAccess(hprocess : super::super::super::Foundation:: HANDLE, addrbase : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFunctionTableAccess64(hprocess : super::super::super::Foundation:: HANDLE, addrbase : u64) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymFunctionTableAccess64AccessRoutines(hprocess : super::super::super::Foundation:: HANDLE, addrbase : u64, readmemoryroutine : PREAD_PROCESS_MEMORY_ROUTINE64, getmodulebaseroutine : PGET_MODULE_BASE_ROUTINE64) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetExtendedOption(option : IMAGEHLP_EXTENDED_OPTIONS) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetFileLineOffsets64(hprocess : super::super::super::Foundation:: HANDLE, modulename : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, buffer : *mut u64, bufferlines : u32) -> u32);
::windows_targets::link!("dbghelp.dll" "system" fn SymGetHomeDirectory(r#type : u32, dir : ::windows_sys::core::PSTR, size : usize) -> ::windows_sys::core::PSTR);
::windows_targets::link!("dbghelp.dll" "system" fn SymGetHomeDirectoryW(r#type : u32, dir : ::windows_sys::core::PWSTR, size : usize) -> ::windows_sys::core::PWSTR);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromAddr(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u32, pdwdisplacement : *mut u32, line : *mut IMAGEHLP_LINE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromAddr64(hprocess : super::super::super::Foundation:: HANDLE, qwaddr : u64, pdwdisplacement : *mut u32, line64 : *mut IMAGEHLP_LINE64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromAddrW64(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u64, pdwdisplacement : *mut u32, line : *mut IMAGEHLP_LINEW64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromInlineContext(hprocess : super::super::super::Foundation:: HANDLE, qwaddr : u64, inlinecontext : u32, qwmodulebaseaddress : u64, pdwdisplacement : *mut u32, line64 : *mut IMAGEHLP_LINE64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromInlineContextW(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u64, inlinecontext : u32, qwmodulebaseaddress : u64, pdwdisplacement : *mut u32, line : *mut IMAGEHLP_LINEW64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromName(hprocess : super::super::super::Foundation:: HANDLE, modulename : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, dwlinenumber : u32, pldisplacement : *mut i32, line : *mut IMAGEHLP_LINE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromName64(hprocess : super::super::super::Foundation:: HANDLE, modulename : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, dwlinenumber : u32, pldisplacement : *mut i32, line : *mut IMAGEHLP_LINE64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineFromNameW64(hprocess : super::super::super::Foundation:: HANDLE, modulename : ::windows_sys::core::PCWSTR, filename : ::windows_sys::core::PCWSTR, dwlinenumber : u32, pldisplacement : *mut i32, line : *mut IMAGEHLP_LINEW64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineNext(hprocess : super::super::super::Foundation:: HANDLE, line : *mut IMAGEHLP_LINE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineNext64(hprocess : super::super::super::Foundation:: HANDLE, line : *mut IMAGEHLP_LINE64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLineNextW64(hprocess : super::super::super::Foundation:: HANDLE, line : *mut IMAGEHLP_LINEW64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLinePrev(hprocess : super::super::super::Foundation:: HANDLE, line : *mut IMAGEHLP_LINE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLinePrev64(hprocess : super::super::super::Foundation:: HANDLE, line : *mut IMAGEHLP_LINE64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetLinePrevW64(hprocess : super::super::super::Foundation:: HANDLE, line : *mut IMAGEHLP_LINEW64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetModuleBase(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetModuleBase64(hprocess : super::super::super::Foundation:: HANDLE, qwaddr : u64) -> u64);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetModuleInfo(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u32, moduleinfo : *mut IMAGEHLP_MODULE) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetModuleInfo64(hprocess : super::super::super::Foundation:: HANDLE, qwaddr : u64, moduleinfo : *mut IMAGEHLP_MODULE64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetModuleInfoW(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u32, moduleinfo : *mut IMAGEHLP_MODULEW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetModuleInfoW64(hprocess : super::super::super::Foundation:: HANDLE, qwaddr : u64, moduleinfo : *mut IMAGEHLP_MODULEW64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetOmaps(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, omapto : *mut *mut OMAP, comapto : *mut u64, omapfrom : *mut *mut OMAP, comapfrom : *mut u64) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("dbghelp.dll" "system" fn SymGetOptions() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetScope(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetScopeW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSearchPath(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PSTR, searchpathlength : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSearchPathW(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PWSTR, searchpathlength : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFile(hprocess : super::super::super::Foundation:: HANDLE, base : u64, params : ::windows_sys::core::PCSTR, filespec : ::windows_sys::core::PCSTR, filepath : ::windows_sys::core::PSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileChecksum(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCSTR, pchecksumtype : *mut u32, pchecksum : *mut u8, checksumsize : u32, pactualbyteswritten : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileChecksumW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCWSTR, pchecksumtype : *mut u32, pchecksum : *mut u8, checksumsize : u32, pactualbyteswritten : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileFromToken(hprocess : super::super::super::Foundation:: HANDLE, token : *const ::core::ffi::c_void, params : ::windows_sys::core::PCSTR, filepath : ::windows_sys::core::PSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileFromTokenByTokenName(hprocess : super::super::super::Foundation:: HANDLE, token : *const ::core::ffi::c_void, tokenname : ::windows_sys::core::PCSTR, params : ::windows_sys::core::PCSTR, filepath : ::windows_sys::core::PSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileFromTokenByTokenNameW(hprocess : super::super::super::Foundation:: HANDLE, token : *const ::core::ffi::c_void, tokenname : ::windows_sys::core::PCWSTR, params : ::windows_sys::core::PCWSTR, filepath : ::windows_sys::core::PWSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileFromTokenW(hprocess : super::super::super::Foundation:: HANDLE, token : *const ::core::ffi::c_void, params : ::windows_sys::core::PCWSTR, filepath : ::windows_sys::core::PWSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileToken(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCSTR, token : *mut *mut ::core::ffi::c_void, size : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileTokenByTokenName(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCSTR, tokenname : ::windows_sys::core::PCSTR, tokenparameters : ::windows_sys::core::PCSTR, token : *mut *mut ::core::ffi::c_void, size : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileTokenByTokenNameW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCWSTR, tokenname : ::windows_sys::core::PCWSTR, tokenparameters : ::windows_sys::core::PCWSTR, token : *mut *mut ::core::ffi::c_void, size : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileTokenW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, filespec : ::windows_sys::core::PCWSTR, token : *mut *mut ::core::ffi::c_void, size : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceFileW(hprocess : super::super::super::Foundation:: HANDLE, base : u64, params : ::windows_sys::core::PCWSTR, filespec : ::windows_sys::core::PCWSTR, filepath : ::windows_sys::core::PWSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceVarFromToken(hprocess : super::super::super::Foundation:: HANDLE, token : *const ::core::ffi::c_void, params : ::windows_sys::core::PCSTR, varname : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSourceVarFromTokenW(hprocess : super::super::super::Foundation:: HANDLE, token : *const ::core::ffi::c_void, params : ::windows_sys::core::PCWSTR, varname : ::windows_sys::core::PCWSTR, value : ::windows_sys::core::PWSTR, size : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymFromAddr(hprocess : super::super::super::Foundation:: HANDLE, dwaddr : u32, pdwdisplacement : *mut u32, symbol : *mut IMAGEHLP_SYMBOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymFromAddr64(hprocess : super::super::super::Foundation:: HANDLE, qwaddr : u64, pdwdisplacement : *mut u64, symbol : *mut IMAGEHLP_SYMBOL64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymFromName(hprocess : super::super::super::Foundation:: HANDLE, name : ::windows_sys::core::PCSTR, symbol : *mut IMAGEHLP_SYMBOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymFromName64(hprocess : super::super::super::Foundation:: HANDLE, name : ::windows_sys::core::PCSTR, symbol : *mut IMAGEHLP_SYMBOL64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymNext(hprocess : super::super::super::Foundation:: HANDLE, symbol : *mut IMAGEHLP_SYMBOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymNext64(hprocess : super::super::super::Foundation:: HANDLE, symbol : *mut IMAGEHLP_SYMBOL64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymPrev(hprocess : super::super::super::Foundation:: HANDLE, symbol : *mut IMAGEHLP_SYMBOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymPrev64(hprocess : super::super::super::Foundation:: HANDLE, symbol : *mut IMAGEHLP_SYMBOL64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymbolFile(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCSTR, imagefile : ::windows_sys::core::PCSTR, r#type : u32, symbolfile : ::windows_sys::core::PSTR, csymbolfile : usize, dbgfile : ::windows_sys::core::PSTR, cdbgfile : usize) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetSymbolFileW(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCWSTR, imagefile : ::windows_sys::core::PCWSTR, r#type : u32, symbolfile : ::windows_sys::core::PWSTR, csymbolfile : usize, dbgfile : ::windows_sys::core::PWSTR, cdbgfile : usize) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetTypeFromName(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, name : ::windows_sys::core::PCSTR, symbol : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetTypeFromNameW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, name : ::windows_sys::core::PCWSTR, symbol : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetTypeInfo(hprocess : super::super::super::Foundation:: HANDLE, modbase : u64, typeid : u32, gettype : IMAGEHLP_SYMBOL_TYPE_INFO, pinfo : *mut ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetTypeInfoEx(hprocess : super::super::super::Foundation:: HANDLE, modbase : u64, params : *mut IMAGEHLP_GET_TYPE_INFO_PARAMS) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymGetUnwindInfo(hprocess : super::super::super::Foundation:: HANDLE, address : u64, buffer : *mut ::core::ffi::c_void, size : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymInitialize(hprocess : super::super::super::Foundation:: HANDLE, usersearchpath : ::windows_sys::core::PCSTR, finvadeprocess : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymInitializeW(hprocess : super::super::super::Foundation:: HANDLE, usersearchpath : ::windows_sys::core::PCWSTR, finvadeprocess : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymLoadModule(hprocess : super::super::super::Foundation:: HANDLE, hfile : super::super::super::Foundation:: HANDLE, imagename : ::windows_sys::core::PCSTR, modulename : ::windows_sys::core::PCSTR, baseofdll : u32, sizeofdll : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymLoadModule64(hprocess : super::super::super::Foundation:: HANDLE, hfile : super::super::super::Foundation:: HANDLE, imagename : ::windows_sys::core::PCSTR, modulename : ::windows_sys::core::PCSTR, baseofdll : u64, sizeofdll : u32) -> u64);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymLoadModuleEx(hprocess : super::super::super::Foundation:: HANDLE, hfile : super::super::super::Foundation:: HANDLE, imagename : ::windows_sys::core::PCSTR, modulename : ::windows_sys::core::PCSTR, baseofdll : u64, dllsize : u32, data : *const MODLOAD_DATA, flags : SYM_LOAD_FLAGS) -> u64);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymLoadModuleExW(hprocess : super::super::super::Foundation:: HANDLE, hfile : super::super::super::Foundation:: HANDLE, imagename : ::windows_sys::core::PCWSTR, modulename : ::windows_sys::core::PCWSTR, baseofdll : u64, dllsize : u32, data : *const MODLOAD_DATA, flags : SYM_LOAD_FLAGS) -> u64);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymMatchFileName(filename : ::windows_sys::core::PCSTR, r#match : ::windows_sys::core::PCSTR, filenamestop : *mut ::windows_sys::core::PSTR, matchstop : *mut ::windows_sys::core::PSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymMatchFileNameW(filename : ::windows_sys::core::PCWSTR, r#match : ::windows_sys::core::PCWSTR, filenamestop : *mut ::windows_sys::core::PWSTR, matchstop : *mut ::windows_sys::core::PWSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymMatchString(string : ::windows_sys::core::PCSTR, expression : ::windows_sys::core::PCSTR, fcase : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymMatchStringA(string : ::windows_sys::core::PCSTR, expression : ::windows_sys::core::PCSTR, fcase : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymMatchStringW(string : ::windows_sys::core::PCWSTR, expression : ::windows_sys::core::PCWSTR, fcase : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymNext(hprocess : super::super::super::Foundation:: HANDLE, si : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymNextW(hprocess : super::super::super::Foundation:: HANDLE, siw : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymPrev(hprocess : super::super::super::Foundation:: HANDLE, si : *mut SYMBOL_INFO) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymPrevW(hprocess : super::super::super::Foundation:: HANDLE, siw : *mut SYMBOL_INFOW) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymQueryInlineTrace(hprocess : super::super::super::Foundation:: HANDLE, startaddress : u64, startcontext : u32, startretaddress : u64, curaddress : u64, curcontext : *mut u32, curframeindex : *mut u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymRefreshModuleList(hprocess : super::super::super::Foundation:: HANDLE) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymRegisterCallback(hprocess : super::super::super::Foundation:: HANDLE, callbackfunction : PSYMBOL_REGISTERED_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymRegisterCallback64(hprocess : super::super::super::Foundation:: HANDLE, callbackfunction : PSYMBOL_REGISTERED_CALLBACK64, usercontext : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymRegisterCallbackW64(hprocess : super::super::super::Foundation:: HANDLE, callbackfunction : PSYMBOL_REGISTERED_CALLBACK64, usercontext : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymRegisterFunctionEntryCallback(hprocess : super::super::super::Foundation:: HANDLE, callbackfunction : PSYMBOL_FUNCENTRY_CALLBACK, usercontext : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymRegisterFunctionEntryCallback64(hprocess : super::super::super::Foundation:: HANDLE, callbackfunction : PSYMBOL_FUNCENTRY_CALLBACK64, usercontext : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSearch(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32, symtag : u32, mask : ::windows_sys::core::PCSTR, address : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACK, usercontext : *const ::core::ffi::c_void, options : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSearchW(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32, symtag : u32, mask : ::windows_sys::core::PCWSTR, address : u64, enumsymbolscallback : PSYM_ENUMERATESYMBOLS_CALLBACKW, usercontext : *const ::core::ffi::c_void, options : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetContext(hprocess : super::super::super::Foundation:: HANDLE, stackframe : *const IMAGEHLP_STACK_FRAME, context : *const ::core::ffi::c_void) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetExtendedOption(option : IMAGEHLP_EXTENDED_OPTIONS, value : super::super::super::Foundation:: BOOL) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetHomeDirectory(hprocess : super::super::super::Foundation:: HANDLE, dir : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetHomeDirectoryW(hprocess : super::super::super::Foundation:: HANDLE, dir : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("dbghelp.dll" "system" fn SymSetOptions(symoptions : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetParentWindow(hwnd : super::super::super::Foundation:: HWND) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetScopeFromAddr(hprocess : super::super::super::Foundation:: HANDLE, address : u64) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetScopeFromIndex(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64, index : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetScopeFromInlineContext(hprocess : super::super::super::Foundation:: HANDLE, address : u64, inlinecontext : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetSearchPath(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSetSearchPathW(hprocess : super::super::super::Foundation:: HANDLE, searchpatha : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvDeltaName(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCSTR, r#type : ::windows_sys::core::PCSTR, file1 : ::windows_sys::core::PCSTR, file2 : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvDeltaNameW(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCWSTR, r#type : ::windows_sys::core::PCWSTR, file1 : ::windows_sys::core::PCWSTR, file2 : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PCWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetFileIndexInfo(file : ::windows_sys::core::PCSTR, info : *mut SYMSRV_INDEX_INFO, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetFileIndexInfoW(file : ::windows_sys::core::PCWSTR, info : *mut SYMSRV_INDEX_INFOW, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetFileIndexString(hprocess : super::super::super::Foundation:: HANDLE, srvpath : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR, index : ::windows_sys::core::PSTR, size : usize, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetFileIndexStringW(hprocess : super::super::super::Foundation:: HANDLE, srvpath : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR, index : ::windows_sys::core::PWSTR, size : usize, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetFileIndexes(file : ::windows_sys::core::PCSTR, id : *mut ::windows_sys::core::GUID, val1 : *mut u32, val2 : *mut u32, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetFileIndexesW(file : ::windows_sys::core::PCWSTR, id : *mut ::windows_sys::core::GUID, val1 : *mut u32, val2 : *mut u32, flags : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetSupplement(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCSTR, node : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvGetSupplementW(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCWSTR, node : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PCWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvIsStore(hprocess : super::super::super::Foundation:: HANDLE, path : ::windows_sys::core::PCSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvIsStoreW(hprocess : super::super::super::Foundation:: HANDLE, path : ::windows_sys::core::PCWSTR) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvStoreFile(hprocess : super::super::super::Foundation:: HANDLE, srvpath : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR, flags : SYM_SRV_STORE_FILE_FLAGS) -> ::windows_sys::core::PCSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvStoreFileW(hprocess : super::super::super::Foundation:: HANDLE, srvpath : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR, flags : SYM_SRV_STORE_FILE_FLAGS) -> ::windows_sys::core::PCWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvStoreSupplement(hprocess : super::super::super::Foundation:: HANDLE, srvpath : ::windows_sys::core::PCSTR, node : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR, flags : u32) -> ::windows_sys::core::PCSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymSrvStoreSupplementW(hprocess : super::super::super::Foundation:: HANDLE, sympath : ::windows_sys::core::PCWSTR, node : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR, flags : u32) -> ::windows_sys::core::PCWSTR);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymUnDName(sym : *const IMAGEHLP_SYMBOL, undecname : ::windows_sys::core::PSTR, undecnamelength : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymUnDName64(sym : *const IMAGEHLP_SYMBOL64, undecname : ::windows_sys::core::PSTR, undecnamelength : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymUnloadModule(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("dbghelp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SymUnloadModule64(hprocess : super::super::super::Foundation:: HANDLE, baseofdll : u64) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("api-ms-win-core-errorhandling-l1-1-3.dll" "system" fn TerminateProcessOnMemoryExhaustion(failedallocationsize : usize) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TouchFileTimes(filehandle : super::super::super::Foundation:: HANDLE, psystemtime : *const super::super::super::Foundation:: SYSTEMTIME) -> super::super::super::Foundation:: BOOL);
::windows_targets::link!("dbghelp.dll" "system" fn UnDecorateSymbolName(name : ::windows_sys::core::PCSTR, outputstring : ::windows_sys::core::PSTR, maxstringlength : u32, flags : u32) -> u32);
::windows_targets::link!("dbghelp.dll" "system" fn UnDecorateSymbolNameW(name : ::windows_sys::core::PCWSTR, outputstring : ::windows_sys::core::PWSTR, maxstringlength : u32, flags : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"] fn UnMapAndLoad(loadedimage : *mut LOADED_IMAGE) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"] fn UnhandledExceptionFilter(exceptioninfo : *const EXCEPTION_POINTERS) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_SystemInformation\"`"] fn UpdateDebugInfoFile(imagefilename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, debugfilepath : ::windows_sys::core::PSTR, ntheaders : *const IMAGE_NT_HEADERS32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_SystemInformation"))]
::windows_targets::link!("imagehlp.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_SystemInformation\"`"] fn UpdateDebugInfoFileEx(imagefilename : ::windows_sys::core::PCSTR, symbolpath : ::windows_sys::core::PCSTR, debugfilepath : ::windows_sys::core::PSTR, ntheaders : *const IMAGE_NT_HEADERS32, oldchecksum : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"] fn WaitForDebugEvent(lpdebugevent : *mut DEBUG_EVENT, dwmilliseconds : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"] fn WaitForDebugEventEx(lpdebugevent : *mut DEBUG_EVENT, dwmilliseconds : u32) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Wow64GetThreadContext(hthread : super::super::super::Foundation:: HANDLE, lpcontext : *mut WOW64_CONTEXT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Wow64GetThreadSelectorEntry(hthread : super::super::super::Foundation:: HANDLE, dwselector : u32, lpselectorentry : *mut WOW64_LDT_ENTRY) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn Wow64SetThreadContext(hthread : super::super::super::Foundation:: HANDLE, lpcontext : *const WOW64_CONTEXT) -> super::super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WriteProcessMemory(hprocess : super::super::super::Foundation:: HANDLE, lpbaseaddress : *const ::core::ffi::c_void, lpbuffer : *const ::core::ffi::c_void, nsize : usize, lpnumberofbyteswritten : *mut usize) -> super::super::super::Foundation:: BOOL);
pub type IDebugExtendedProperty = *mut ::core::ffi::c_void;
pub type IDebugProperty = *mut ::core::ffi::c_void;
pub type IDebugPropertyEnumType_All = *mut ::core::ffi::c_void;
pub type IDebugPropertyEnumType_Arguments = *mut ::core::ffi::c_void;
pub type IDebugPropertyEnumType_Locals = *mut ::core::ffi::c_void;
pub type IDebugPropertyEnumType_LocalsPlusArgs = *mut ::core::ffi::c_void;
pub type IDebugPropertyEnumType_Registers = *mut ::core::ffi::c_void;
pub type IEnumDebugExtendedPropertyInfo = *mut ::core::ffi::c_void;
pub type IEnumDebugPropertyInfo = *mut ::core::ffi::c_void;
pub type IObjectSafety = *mut ::core::ffi::c_void;
pub type IPerPropertyBrowsing2 = *mut ::core::ffi::c_void;
pub const ABNORMAL_RESET_DETECTED: BUGCHECK_ERROR = 327u32;
pub const ACPI_BIOS_ERROR: BUGCHECK_ERROR = 165u32;
pub const ACPI_BIOS_FATAL_ERROR: BUGCHECK_ERROR = 224u32;
pub const ACPI_DRIVER_INTERNAL: BUGCHECK_ERROR = 163u32;
pub const ACPI_FIRMWARE_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 464u32;
pub const ACTIVE_EX_WORKER_THREAD_TERMINATION: BUGCHECK_ERROR = 233u32;
pub const AGP_GART_CORRUPTION: BUGCHECK_ERROR = 261u32;
pub const AGP_ILLEGALLY_REPROGRAMMED: BUGCHECK_ERROR = 262u32;
pub const AGP_INTERNAL: BUGCHECK_ERROR = 277u32;
pub const AGP_INVALID_ACCESS: BUGCHECK_ERROR = 260u32;
pub const APC_INDEX_MISMATCH: BUGCHECK_ERROR = 1u32;
pub const API_VERSION_NUMBER: u32 = 12u32;
pub const APP_TAGGING_INITIALIZATION_FAILED: BUGCHECK_ERROR = 266u32;
pub const ASSIGN_DRIVE_LETTERS_FAILED: BUGCHECK_ERROR = 114u32;
pub const ATDISK_DRIVER_INTERNAL: BUGCHECK_ERROR = 66u32;
pub const ATTEMPTED_EXECUTE_OF_NOEXECUTE_MEMORY: BUGCHECK_ERROR = 252u32;
pub const ATTEMPTED_SWITCH_FROM_DPC: BUGCHECK_ERROR = 184u32;
pub const ATTEMPTED_WRITE_TO_CM_PROTECTED_STORAGE: BUGCHECK_ERROR = 284u32;
pub const ATTEMPTED_WRITE_TO_READONLY_MEMORY: BUGCHECK_ERROR = 190u32;
pub const AUDIT_FAILURE: BUGCHECK_ERROR = 134u32;
pub const AZURE_DEVICE_FW_DUMP: BUGCHECK_ERROR = 482u32;
pub const AddrMode1616: ADDRESS_MODE = 0i32;
pub const AddrMode1632: ADDRESS_MODE = 1i32;
pub const AddrModeFlat: ADDRESS_MODE = 3i32;
pub const AddrModeReal: ADDRESS_MODE = 2i32;
pub const BAD_EXHANDLE: BUGCHECK_ERROR = 167u32;
pub const BAD_OBJECT_HEADER: BUGCHECK_ERROR = 393u32;
pub const BAD_POOL_CALLER: BUGCHECK_ERROR = 194u32;
pub const BAD_POOL_HEADER: BUGCHECK_ERROR = 25u32;
pub const BAD_SYSTEM_CONFIG_INFO: BUGCHECK_ERROR = 116u32;
pub const BC_BLUETOOTH_VERIFIER_FAULT: BUGCHECK_ERROR = 3070u32;
pub const BC_BTHMINI_VERIFIER_FAULT: BUGCHECK_ERROR = 3071u32;
pub const BGI_DETECTED_VIOLATION: BUGCHECK_ERROR = 177u32;
pub const BIND_ALL_IMAGES: u32 = 4u32;
pub const BIND_CACHE_IMPORT_DLLS: u32 = 8u32;
pub const BIND_NO_BOUND_IMPORTS: u32 = 1u32;
pub const BIND_NO_UPDATE: u32 = 2u32;
pub const BIND_REPORT_64BIT_VA: u32 = 16u32;
pub const BITLOCKER_FATAL_ERROR: BUGCHECK_ERROR = 288u32;
pub const BLUETOOTH_ERROR_RECOVERY_LIVEDUMP: BUGCHECK_ERROR = 422u32;
pub const BOOTING_IN_SAFEMODE_DSREPAIR: BUGCHECK_ERROR = 170u32;
pub const BOOTING_IN_SAFEMODE_MINIMAL: BUGCHECK_ERROR = 168u32;
pub const BOOTING_IN_SAFEMODE_NETWORK: BUGCHECK_ERROR = 169u32;
pub const BOOTLOG_ENABLED: BUGCHECK_ERROR = 183u32;
pub const BOOTLOG_LOADED: BUGCHECK_ERROR = 181u32;
pub const BOOTLOG_NOT_LOADED: BUGCHECK_ERROR = 182u32;
pub const BOOTPROC_INITIALIZATION_FAILED: BUGCHECK_ERROR = 110u32;
pub const BOUND_IMAGE_UNSUPPORTED: BUGCHECK_ERROR = 151u32;
pub const BREAKAWAY_CABLE_TRANSITION: BUGCHECK_ERROR = 483u32;
pub const BUGCHECK_CONTEXT_MODIFIER: BUGCHECK_ERROR = 2147483648u32;
pub const BUGCODE_ID_DRIVER: BUGCHECK_ERROR = 210u32;
pub const BUGCODE_MBBADAPTER_DRIVER: BUGCHECK_ERROR = 477u32;
pub const BUGCODE_NDIS_DRIVER: BUGCHECK_ERROR = 124u32;
pub const BUGCODE_NDIS_DRIVER_LIVE_DUMP: BUGCHECK_ERROR = 350u32;
pub const BUGCODE_NETADAPTER_DRIVER: BUGCHECK_ERROR = 414u32;
pub const BUGCODE_USB3_DRIVER: BUGCHECK_ERROR = 324u32;
pub const BUGCODE_USB_DRIVER: BUGCHECK_ERROR = 254u32;
pub const BUGCODE_WIFIADAPTER_DRIVER: BUGCHECK_ERROR = 478u32;
pub const BindExpandFileHeaders: IMAGEHLP_STATUS_REASON = 10i32;
pub const BindForwarder: IMAGEHLP_STATUS_REASON = 7i32;
pub const BindForwarder32: IMAGEHLP_STATUS_REASON = 16i32;
pub const BindForwarder64: IMAGEHLP_STATUS_REASON = 17i32;
pub const BindForwarderNOT: IMAGEHLP_STATUS_REASON = 8i32;
pub const BindForwarderNOT32: IMAGEHLP_STATUS_REASON = 18i32;
pub const BindForwarderNOT64: IMAGEHLP_STATUS_REASON = 19i32;
pub const BindImageComplete: IMAGEHLP_STATUS_REASON = 11i32;
pub const BindImageModified: IMAGEHLP_STATUS_REASON = 9i32;
pub const BindImportModule: IMAGEHLP_STATUS_REASON = 5i32;
pub const BindImportModuleFailed: IMAGEHLP_STATUS_REASON = 3i32;
pub const BindImportProcedure: IMAGEHLP_STATUS_REASON = 6i32;
pub const BindImportProcedure32: IMAGEHLP_STATUS_REASON = 14i32;
pub const BindImportProcedure64: IMAGEHLP_STATUS_REASON = 15i32;
pub const BindImportProcedureFailed: IMAGEHLP_STATUS_REASON = 4i32;
pub const BindMismatchedSymbols: IMAGEHLP_STATUS_REASON = 12i32;
pub const BindNoRoomInImage: IMAGEHLP_STATUS_REASON = 2i32;
pub const BindOutOfMemory: IMAGEHLP_STATUS_REASON = 0i32;
pub const BindRvaToVaFailed: IMAGEHLP_STATUS_REASON = 1i32;
pub const BindSymbolsNotUpdated: IMAGEHLP_STATUS_REASON = 13i32;
pub const CACHE_INITIALIZATION_FAILED: BUGCHECK_ERROR = 102u32;
pub const CACHE_MANAGER: BUGCHECK_ERROR = 52u32;
pub const CALL_HAS_NOT_RETURNED_WATCHDOG_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 419u32;
pub const CANCEL_STATE_IN_COMPLETED_IRP: BUGCHECK_ERROR = 72u32;
pub const CANNOT_WRITE_CONFIGURATION: BUGCHECK_ERROR = 117u32;
pub const CBA_CHECK_ARM_MACHINE_THUMB_TYPE_OVERRIDE: u32 = 2147483648u32;
pub const CBA_CHECK_ENGOPT_DISALLOW_NETWORK_PATHS: u32 = 1879048192u32;
pub const CBA_DEBUG_INFO: u32 = 268435456u32;
pub const CBA_DEFERRED_SYMBOL_LOAD_CANCEL: u32 = 7u32;
pub const CBA_DEFERRED_SYMBOL_LOAD_COMPLETE: u32 = 2u32;
pub const CBA_DEFERRED_SYMBOL_LOAD_FAILURE: u32 = 3u32;
pub const CBA_DEFERRED_SYMBOL_LOAD_PARTIAL: u32 = 32u32;
pub const CBA_DEFERRED_SYMBOL_LOAD_START: u32 = 1u32;
pub const CBA_DUPLICATE_SYMBOL: u32 = 5u32;
pub const CBA_ENGINE_PRESENT: u32 = 1610612736u32;
pub const CBA_EVENT: u32 = 16u32;
pub const CBA_MAP_JIT_SYMBOL: u32 = 2684354560u32;
pub const CBA_READ_MEMORY: u32 = 6u32;
pub const CBA_SET_OPTIONS: u32 = 8u32;
pub const CBA_SRCSRV_EVENT: u32 = 1073741824u32;
pub const CBA_SRCSRV_INFO: u32 = 536870912u32;
pub const CBA_SYMBOLS_UNLOADED: u32 = 4u32;
pub const CBA_UPDATE_STATUS_BAR: u32 = 1342177280u32;
pub const CBA_XML_LOG: u32 = 2415919104u32;
pub const CDFS_FILE_SYSTEM: BUGCHECK_ERROR = 38u32;
pub const CERT_PE_IMAGE_DIGEST_ALL_IMPORT_INFO: u32 = 4u32;
pub const CERT_PE_IMAGE_DIGEST_DEBUG_INFO: u32 = 1u32;
pub const CERT_PE_IMAGE_DIGEST_NON_PE_INFO: u32 = 8u32;
pub const CERT_PE_IMAGE_DIGEST_RESOURCES: u32 = 2u32;
pub const CERT_SECTION_TYPE_ANY: u32 = 255u32;
pub const CHECKSUM_MAPVIEW_FAILURE: u32 = 3u32;
pub const CHECKSUM_MAP_FAILURE: u32 = 2u32;
pub const CHECKSUM_OPEN_FAILURE: u32 = 1u32;
pub const CHECKSUM_SUCCESS: u32 = 0u32;
pub const CHECKSUM_UNICODE_FAILURE: u32 = 4u32;
pub const CHIPSET_DETECTED_ERROR: BUGCHECK_ERROR = 185u32;
pub const CID_HANDLE_CREATION: BUGCHECK_ERROR = 22u32;
pub const CID_HANDLE_DELETION: BUGCHECK_ERROR = 23u32;
pub const CLOCK_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 257u32;
pub const CLUSTER_CLUSPORT_STATUS_IO_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 377u32;
pub const CLUSTER_CSVFS_LIVEDUMP: BUGCHECK_ERROR = 392u32;
pub const CLUSTER_CSV_CLUSSVC_DISCONNECT_WATCHDOG: BUGCHECK_ERROR = 368u32;
pub const CLUSTER_CSV_CLUSTER_WATCHDOG_LIVEDUMP: BUGCHECK_ERROR = 363u32;
pub const CLUSTER_CSV_SNAPSHOT_DEVICE_INFO_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 359u32;
pub const CLUSTER_CSV_STATE_TRANSITION_INTERVAL_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 367u32;
pub const CLUSTER_CSV_STATE_TRANSITION_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 360u32;
pub const CLUSTER_CSV_STATUS_IO_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 357u32;
pub const CLUSTER_CSV_VOLUME_ARRIVAL_LIVEDUMP: BUGCHECK_ERROR = 361u32;
pub const CLUSTER_CSV_VOLUME_REMOVAL_LIVEDUMP: BUGCHECK_ERROR = 362u32;
pub const CLUSTER_RESOURCE_CALL_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 358u32;
pub const CLUSTER_SVHDX_LIVEDUMP: BUGCHECK_ERROR = 413u32;
pub const CNSS_FILE_SYSTEM_FILTER: BUGCHECK_ERROR = 164u32;
pub const CONFIG_INITIALIZATION_FAILED: BUGCHECK_ERROR = 103u32;
pub const CONFIG_LIST_FAILED: BUGCHECK_ERROR = 115u32;
pub const CONNECTED_STANDBY_WATCHDOG_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 351u32;
pub const CONTEXT_ALL_AMD64: CONTEXT_FLAGS = 1048607u32;
pub const CONTEXT_ALL_ARM: CONTEXT_FLAGS = 2097167u32;
pub const CONTEXT_ALL_ARM64: CONTEXT_FLAGS = 4194335u32;
pub const CONTEXT_ALL_X86: CONTEXT_FLAGS = 65599u32;
pub const CONTEXT_AMD64: CONTEXT_FLAGS = 1048576u32;
pub const CONTEXT_ARM: CONTEXT_FLAGS = 2097152u32;
pub const CONTEXT_ARM64: CONTEXT_FLAGS = 4194304u32;
pub const CONTEXT_CONTROL_AMD64: CONTEXT_FLAGS = 1048577u32;
pub const CONTEXT_CONTROL_ARM: CONTEXT_FLAGS = 2097153u32;
pub const CONTEXT_CONTROL_ARM64: CONTEXT_FLAGS = 4194305u32;
pub const CONTEXT_CONTROL_X86: CONTEXT_FLAGS = 65537u32;
pub const CONTEXT_DEBUG_REGISTERS_AMD64: CONTEXT_FLAGS = 1048592u32;
pub const CONTEXT_DEBUG_REGISTERS_ARM: CONTEXT_FLAGS = 2097160u32;
pub const CONTEXT_DEBUG_REGISTERS_ARM64: CONTEXT_FLAGS = 4194312u32;
pub const CONTEXT_DEBUG_REGISTERS_X86: CONTEXT_FLAGS = 65552u32;
pub const CONTEXT_EXCEPTION_ACTIVE_AMD64: CONTEXT_FLAGS = 134217728u32;
pub const CONTEXT_EXCEPTION_ACTIVE_ARM: CONTEXT_FLAGS = 134217728u32;
pub const CONTEXT_EXCEPTION_ACTIVE_ARM64: CONTEXT_FLAGS = 134217728u32;
pub const CONTEXT_EXCEPTION_ACTIVE_X86: CONTEXT_FLAGS = 134217728u32;
pub const CONTEXT_EXCEPTION_REPORTING_AMD64: CONTEXT_FLAGS = 2147483648u32;
pub const CONTEXT_EXCEPTION_REPORTING_ARM: CONTEXT_FLAGS = 2147483648u32;
pub const CONTEXT_EXCEPTION_REPORTING_ARM64: CONTEXT_FLAGS = 2147483648u32;
pub const CONTEXT_EXCEPTION_REPORTING_X86: CONTEXT_FLAGS = 2147483648u32;
pub const CONTEXT_EXCEPTION_REQUEST_AMD64: CONTEXT_FLAGS = 1073741824u32;
pub const CONTEXT_EXCEPTION_REQUEST_ARM: CONTEXT_FLAGS = 1073741824u32;
pub const CONTEXT_EXCEPTION_REQUEST_ARM64: CONTEXT_FLAGS = 1073741824u32;
pub const CONTEXT_EXCEPTION_REQUEST_X86: CONTEXT_FLAGS = 1073741824u32;
pub const CONTEXT_EXTENDED_REGISTERS_X86: CONTEXT_FLAGS = 65568u32;
pub const CONTEXT_FLOATING_POINT_AMD64: CONTEXT_FLAGS = 1048584u32;
pub const CONTEXT_FLOATING_POINT_ARM: CONTEXT_FLAGS = 2097156u32;
pub const CONTEXT_FLOATING_POINT_ARM64: CONTEXT_FLAGS = 4194308u32;
pub const CONTEXT_FLOATING_POINT_X86: CONTEXT_FLAGS = 65544u32;
pub const CONTEXT_FULL_AMD64: CONTEXT_FLAGS = 1048587u32;
pub const CONTEXT_FULL_ARM: CONTEXT_FLAGS = 2097159u32;
pub const CONTEXT_FULL_ARM64: CONTEXT_FLAGS = 4194311u32;
pub const CONTEXT_FULL_X86: CONTEXT_FLAGS = 65543u32;
pub const CONTEXT_INTEGER_AMD64: CONTEXT_FLAGS = 1048578u32;
pub const CONTEXT_INTEGER_ARM: CONTEXT_FLAGS = 2097154u32;
pub const CONTEXT_INTEGER_ARM64: CONTEXT_FLAGS = 4194306u32;
pub const CONTEXT_INTEGER_X86: CONTEXT_FLAGS = 65538u32;
pub const CONTEXT_KERNEL_CET_AMD64: CONTEXT_FLAGS = 1048704u32;
pub const CONTEXT_KERNEL_DEBUGGER_AMD64: CONTEXT_FLAGS = 67108864u32;
pub const CONTEXT_RET_TO_GUEST_ARM64: CONTEXT_FLAGS = 1073741824u32;
pub const CONTEXT_SEGMENTS_AMD64: CONTEXT_FLAGS = 1048580u32;
pub const CONTEXT_SEGMENTS_X86: CONTEXT_FLAGS = 65540u32;
pub const CONTEXT_SERVICE_ACTIVE_AMD64: CONTEXT_FLAGS = 268435456u32;
pub const CONTEXT_SERVICE_ACTIVE_ARM: CONTEXT_FLAGS = 268435456u32;
pub const CONTEXT_SERVICE_ACTIVE_ARM64: CONTEXT_FLAGS = 268435456u32;
pub const CONTEXT_SERVICE_ACTIVE_X86: CONTEXT_FLAGS = 268435456u32;
pub const CONTEXT_UNWOUND_TO_CALL_AMD64: CONTEXT_FLAGS = 536870912u32;
pub const CONTEXT_UNWOUND_TO_CALL_ARM: CONTEXT_FLAGS = 536870912u32;
pub const CONTEXT_UNWOUND_TO_CALL_ARM64: CONTEXT_FLAGS = 536870912u32;
pub const CONTEXT_X18_ARM64: CONTEXT_FLAGS = 4194320u32;
pub const CONTEXT_X86: CONTEXT_FLAGS = 65536u32;
pub const CONTEXT_XSTATE_AMD64: CONTEXT_FLAGS = 1048640u32;
pub const CONTEXT_XSTATE_X86: CONTEXT_FLAGS = 65600u32;
pub const COREMSGCALL_INTERNAL_ERROR: BUGCHECK_ERROR = 371u32;
pub const COREMSG_INTERNAL_ERROR: BUGCHECK_ERROR = 372u32;
pub const CORRUPT_ACCESS_TOKEN: BUGCHECK_ERROR = 40u32;
pub const CRASHDUMP_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 486u32;
pub const CREATE_DELETE_LOCK_NOT_LOCKED: BUGCHECK_ERROR = 20u32;
pub const CREATE_PROCESS_DEBUG_EVENT: DEBUG_EVENT_CODE = 3u32;
pub const CREATE_THREAD_DEBUG_EVENT: DEBUG_EVENT_CODE = 2u32;
pub const CRITICAL_INITIALIZATION_FAILURE: BUGCHECK_ERROR = 317u32;
pub const CRITICAL_OBJECT_TERMINATION: BUGCHECK_ERROR = 244u32;
pub const CRITICAL_PROCESS_DIED: BUGCHECK_ERROR = 239u32;
pub const CRITICAL_SERVICE_FAILED: BUGCHECK_ERROR = 90u32;
pub const CRITICAL_STRUCTURE_CORRUPTION: BUGCHECK_ERROR = 265u32;
pub const CRYPTO_LIBRARY_INTERNAL_ERROR: BUGCHECK_ERROR = 369u32;
pub const CRYPTO_SELF_TEST_FAILURE: BUGCHECK_ERROR = 291u32;
pub const CancelCallback: MINIDUMP_CALLBACK_TYPE = 6i32;
pub const CommentStreamA: MINIDUMP_STREAM_TYPE = 10i32;
pub const CommentStreamW: MINIDUMP_STREAM_TYPE = 11i32;
pub const DAM_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 491u32;
pub const DATA_BUS_ERROR: BUGCHECK_ERROR = 46u32;
pub const DATA_COHERENCY_EXCEPTION: BUGCHECK_ERROR = 85u32;
pub const DBGPROP_ATTRIB_ACCESS_FINAL: DBGPROP_ATTRIB_FLAGS = 32768i32;
pub const DBGPROP_ATTRIB_ACCESS_PRIVATE: DBGPROP_ATTRIB_FLAGS = 8192i32;
pub const DBGPROP_ATTRIB_ACCESS_PROTECTED: DBGPROP_ATTRIB_FLAGS = 16384i32;
pub const DBGPROP_ATTRIB_ACCESS_PUBLIC: DBGPROP_ATTRIB_FLAGS = 4096i32;
pub const DBGPROP_ATTRIB_FRAME_INCATCHBLOCK: DBGPROP_ATTRIB_FLAGS = 33554432i32;
pub const DBGPROP_ATTRIB_FRAME_INFINALLYBLOCK: DBGPROP_ATTRIB_FLAGS = 67108864i32;
pub const DBGPROP_ATTRIB_FRAME_INTRYBLOCK: DBGPROP_ATTRIB_FLAGS = 16777216i32;
pub const DBGPROP_ATTRIB_HAS_EXTENDED_ATTRIBS: DBGPROP_ATTRIB_FLAGS = 8388608i32;
pub const DBGPROP_ATTRIB_NO_ATTRIB: DBGPROP_ATTRIB_FLAGS = 0i32;
pub const DBGPROP_ATTRIB_STORAGE_FIELD: DBGPROP_ATTRIB_FLAGS = 262144i32;
pub const DBGPROP_ATTRIB_STORAGE_GLOBAL: DBGPROP_ATTRIB_FLAGS = 65536i32;
pub const DBGPROP_ATTRIB_STORAGE_STATIC: DBGPROP_ATTRIB_FLAGS = 131072i32;
pub const DBGPROP_ATTRIB_STORAGE_VIRTUAL: DBGPROP_ATTRIB_FLAGS = 524288i32;
pub const DBGPROP_ATTRIB_TYPE_IS_CONSTANT: DBGPROP_ATTRIB_FLAGS = 1048576i32;
pub const DBGPROP_ATTRIB_TYPE_IS_SYNCHRONIZED: DBGPROP_ATTRIB_FLAGS = 2097152i32;
pub const DBGPROP_ATTRIB_TYPE_IS_VOLATILE: DBGPROP_ATTRIB_FLAGS = 4194304i32;
pub const DBGPROP_ATTRIB_VALUE_IS_EVENT: DBGPROP_ATTRIB_FLAGS = 512i32;
pub const DBGPROP_ATTRIB_VALUE_IS_EXPANDABLE: DBGPROP_ATTRIB_FLAGS = 16i32;
pub const DBGPROP_ATTRIB_VALUE_IS_FAKE: DBGPROP_ATTRIB_FLAGS = 32i32;
pub const DBGPROP_ATTRIB_VALUE_IS_INVALID: DBGPROP_ATTRIB_FLAGS = 8i32;
pub const DBGPROP_ATTRIB_VALUE_IS_METHOD: DBGPROP_ATTRIB_FLAGS = 256i32;
pub const DBGPROP_ATTRIB_VALUE_IS_RAW_STRING: DBGPROP_ATTRIB_FLAGS = 1024i32;
pub const DBGPROP_ATTRIB_VALUE_IS_RETURN_VALUE: DBGPROP_ATTRIB_FLAGS = 134217728i32;
pub const DBGPROP_ATTRIB_VALUE_PENDING_MUTATION: DBGPROP_ATTRIB_FLAGS = 268435456i32;
pub const DBGPROP_ATTRIB_VALUE_READONLY: DBGPROP_ATTRIB_FLAGS = 2048i32;
pub const DBGPROP_INFO_ATTRIBUTES: DBGPROP_INFO = 8i32;
pub const DBGPROP_INFO_AUTOEXPAND: DBGPROP_INFO = 134217728i32;
pub const DBGPROP_INFO_BEAUTIFY: DBGPROP_INFO = 33554432i32;
pub const DBGPROP_INFO_CALLTOSTRING: DBGPROP_INFO = 67108864i32;
pub const DBGPROP_INFO_DEBUGPROP: DBGPROP_INFO = 16i32;
pub const DBGPROP_INFO_FULLNAME: DBGPROP_INFO = 32i32;
pub const DBGPROP_INFO_NAME: DBGPROP_INFO = 1i32;
pub const DBGPROP_INFO_TYPE: DBGPROP_INFO = 2i32;
pub const DBGPROP_INFO_VALUE: DBGPROP_INFO = 4i32;
pub const DBHHEADER_CVMISC: MODLOAD_DATA_TYPE = 2u32;
pub const DBHHEADER_DEBUGDIRS: MODLOAD_DATA_TYPE = 1u32;
pub const DBHHEADER_PDBGUID: u32 = 3u32;
pub const DEREF_UNKNOWN_LOGON_SESSION: BUGCHECK_ERROR = 70u32;
pub const DEVICE_DIAGNOSTIC_LOG_LIVEDUMP: BUGCHECK_ERROR = 481u32;
pub const DEVICE_QUEUE_NOT_BUSY: BUGCHECK_ERROR = 2u32;
pub const DEVICE_REFERENCE_COUNT_NOT_ZERO: BUGCHECK_ERROR = 54u32;
pub const DFSC_FILE_SYSTEM: BUGCHECK_ERROR = 267u32;
pub const DFS_FILE_SYSTEM: BUGCHECK_ERROR = 130u32;
pub const DIRECTED_FX_TRANSITION_LIVEDUMP: BUGCHECK_ERROR = 425u32;
pub const DIRTY_MAPPED_PAGES_CONGESTION: BUGCHECK_ERROR = 235u32;
pub const DIRTY_NOWRITE_PAGES_CONGESTION: BUGCHECK_ERROR = 253u32;
pub const DISORDERLY_SHUTDOWN: BUGCHECK_ERROR = 243u32;
pub const DMA_COMMON_BUFFER_VECTOR_ERROR: BUGCHECK_ERROR = 476u32;
pub const DMP_CONTEXT_RECORD_SIZE_32: u32 = 1200u32;
pub const DMP_CONTEXT_RECORD_SIZE_64: u32 = 3000u32;
pub const DMP_HEADER_COMMENT_SIZE: u32 = 128u32;
pub const DMP_PHYSICAL_MEMORY_BLOCK_SIZE_32: u32 = 700u32;
pub const DMP_PHYSICAL_MEMORY_BLOCK_SIZE_64: u32 = 700u32;
pub const DMP_RESERVED_0_SIZE_32: u32 = 1760u32;
pub const DMP_RESERVED_0_SIZE_64: u32 = 4008u32;
pub const DMP_RESERVED_2_SIZE_32: u32 = 16u32;
pub const DMP_RESERVED_3_SIZE_32: u32 = 56u32;
pub const DPC_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 258u32;
pub const DPC_WATCHDOG_VIOLATION: BUGCHECK_ERROR = 307u32;
pub const DRIPS_SW_HW_DIVERGENCE_LIVEDUMP: BUGCHECK_ERROR = 420u32;
pub const DRIVER_CAUGHT_MODIFYING_FREED_POOL: BUGCHECK_ERROR = 198u32;
pub const DRIVER_CORRUPTED_EXPOOL: BUGCHECK_ERROR = 197u32;
pub const DRIVER_CORRUPTED_MMPOOL: BUGCHECK_ERROR = 208u32;
pub const DRIVER_CORRUPTED_SYSPTES: BUGCHECK_ERROR = 219u32;
pub const DRIVER_INVALID_CRUNTIME_PARAMETER: BUGCHECK_ERROR = 272u32;
pub const DRIVER_INVALID_STACK_ACCESS: BUGCHECK_ERROR = 220u32;
pub const DRIVER_IRQL_NOT_LESS_OR_EQUAL: BUGCHECK_ERROR = 209u32;
pub const DRIVER_LEFT_LOCKED_PAGES_IN_PROCESS: BUGCHECK_ERROR = 203u32;
pub const DRIVER_OVERRAN_STACK_BUFFER: BUGCHECK_ERROR = 247u32;
pub const DRIVER_PAGE_FAULT_BEYOND_END_OF_ALLOCATION: BUGCHECK_ERROR = 214u32;
pub const DRIVER_PAGE_FAULT_BEYOND_END_OF_ALLOCATION_M: BUGCHECK_ERROR = 268435670u32;
pub const DRIVER_PAGE_FAULT_IN_FREED_SPECIAL_POOL: BUGCHECK_ERROR = 213u32;
pub const DRIVER_PNP_WATCHDOG: BUGCHECK_ERROR = 469u32;
pub const DRIVER_PORTION_MUST_BE_NONPAGED: BUGCHECK_ERROR = 211u32;
pub const DRIVER_POWER_STATE_FAILURE: BUGCHECK_ERROR = 159u32;
pub const DRIVER_RETURNED_HOLDING_CANCEL_LOCK: BUGCHECK_ERROR = 283u32;
pub const DRIVER_RETURNED_STATUS_REPARSE_FOR_VOLUME_OPEN: BUGCHECK_ERROR = 249u32;
pub const DRIVER_UNLOADED_WITHOUT_CANCELLING_PENDING_OPERATIONS: BUGCHECK_ERROR = 206u32;
pub const DRIVER_UNMAPPING_INVALID_VIEW: BUGCHECK_ERROR = 215u32;
pub const DRIVER_USED_EXCESSIVE_PTES: BUGCHECK_ERROR = 216u32;
pub const DRIVER_VERIFIER_DETECTED_VIOLATION: BUGCHECK_ERROR = 196u32;
pub const DRIVER_VERIFIER_DETECTED_VIOLATION_LIVEDUMP: BUGCHECK_ERROR = 452u32;
pub const DRIVER_VERIFIER_DMA_VIOLATION: BUGCHECK_ERROR = 230u32;
pub const DRIVER_VERIFIER_IOMANAGER_VIOLATION: BUGCHECK_ERROR = 201u32;
pub const DRIVER_VERIFIER_TRACKING_LIVE_DUMP: BUGCHECK_ERROR = 485u32;
pub const DRIVER_VIOLATION: BUGCHECK_ERROR = 289u32;
pub const DRIVE_EXTENDER: BUGCHECK_ERROR = 308u32;
pub const DSLFLAG_MISMATCHED_DBG: u32 = 2u32;
pub const DSLFLAG_MISMATCHED_PDB: u32 = 1u32;
pub const DUMP_SUMMARY_VALID_CURRENT_USER_VA: u32 = 2u32;
pub const DUMP_SUMMARY_VALID_KERNEL_VA: u32 = 1u32;
pub const DUMP_TYPE_AUTOMATIC: DUMP_TYPE = 7i32;
pub const DUMP_TYPE_BITMAP_FULL: DUMP_TYPE = 5i32;
pub const DUMP_TYPE_BITMAP_KERNEL: DUMP_TYPE = 6i32;
pub const DUMP_TYPE_FULL: DUMP_TYPE = 1i32;
pub const DUMP_TYPE_HEADER: DUMP_TYPE = 3i32;
pub const DUMP_TYPE_INVALID: DUMP_TYPE = -1i32;
pub const DUMP_TYPE_SUMMARY: DUMP_TYPE = 2i32;
pub const DUMP_TYPE_TRIAGE: DUMP_TYPE = 4i32;
pub const DUMP_TYPE_UNKNOWN: DUMP_TYPE = 0i32;
pub const DYNAMIC_ADD_PROCESSOR_MISMATCH: BUGCHECK_ERROR = 304u32;
pub const EFS_FATAL_ERROR: BUGCHECK_ERROR = 471u32;
pub const ELAM_DRIVER_DETECTED_FATAL_ERROR: BUGCHECK_ERROR = 376u32;
pub const EMPTY_THREAD_REAPER_LIST: BUGCHECK_ERROR = 19u32;
pub const EM_INITIALIZATION_ERROR: BUGCHECK_ERROR = 282u32;
pub const END_OF_NT_EVALUATION_PERIOD: BUGCHECK_ERROR = 152u32;
pub const ERESOURCE_INVALID_RELEASE: BUGCHECK_ERROR = 366u32;
pub const ERRATA_WORKAROUND_UNSUCCESSFUL: BUGCHECK_ERROR = 318u32;
pub const ERROR_IMAGE_NOT_STRIPPED: u32 = 34816u32;
pub const ERROR_NO_DBG_POINTER: u32 = 34817u32;
pub const ERROR_NO_PDB_POINTER: u32 = 34818u32;
pub const ESLFLAG_FULLPATH: u32 = 1u32;
pub const ESLFLAG_INLINE_SITE: u32 = 16u32;
pub const ESLFLAG_NEAREST: u32 = 2u32;
pub const ESLFLAG_NEXT: u32 = 8u32;
pub const ESLFLAG_PREV: u32 = 4u32;
pub const EVENT_SRCSPEW: u32 = 100u32;
pub const EVENT_SRCSPEW_END: u32 = 199u32;
pub const EVENT_SRCSPEW_START: u32 = 100u32;
pub const EVENT_TRACING_FATAL_ERROR: BUGCHECK_ERROR = 285u32;
pub const EXCEPTION_DEBUG_EVENT: DEBUG_EVENT_CODE = 1u32;
pub const EXCEPTION_ON_INVALID_STACK: BUGCHECK_ERROR = 426u32;
pub const EXCEPTION_SCOPE_INVALID: BUGCHECK_ERROR = 333u32;
pub const EXFAT_FILE_SYSTEM: BUGCHECK_ERROR = 300u32;
pub const EXIT_PROCESS_DEBUG_EVENT: DEBUG_EVENT_CODE = 5u32;
pub const EXIT_THREAD_DEBUG_EVENT: DEBUG_EVENT_CODE = 4u32;
pub const EXRESOURCE_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 460u32;
pub const EXT_OUTPUT_VER: u32 = 1u32;
pub const EX_PROP_INFO_DEBUGEXTPROP: EX_PROP_INFO_FLAGS = 4096i32;
pub const EX_PROP_INFO_ID: EX_PROP_INFO_FLAGS = 256i32;
pub const EX_PROP_INFO_LOCKBYTES: EX_PROP_INFO_FLAGS = 2048i32;
pub const EX_PROP_INFO_NTYPE: EX_PROP_INFO_FLAGS = 512i32;
pub const EX_PROP_INFO_NVALUE: EX_PROP_INFO_FLAGS = 1024i32;
pub const ExceptionStream: MINIDUMP_STREAM_TYPE = 6i32;
pub const FACILITY_AAF: FACILITY_CODE = 18u32;
pub const FACILITY_ACCELERATOR: FACILITY_CODE = 1536u32;
pub const FACILITY_ACS: FACILITY_CODE = 20u32;
pub const FACILITY_ACTION_QUEUE: FACILITY_CODE = 44u32;
pub const FACILITY_AUDCLNT: FACILITY_CODE = 2185u32;
pub const FACILITY_AUDIO: FACILITY_CODE = 102u32;
pub const FACILITY_AUDIOSTREAMING: FACILITY_CODE = 1094u32;
pub const FACILITY_BACKGROUNDCOPY: FACILITY_CODE = 32u32;
pub const FACILITY_BCD: FACILITY_CODE = 57u32;
pub const FACILITY_BLB: FACILITY_CODE = 120u32;
pub const FACILITY_BLBUI: FACILITY_CODE = 128u32;
pub const FACILITY_BLB_CLI: FACILITY_CODE = 121u32;
pub const FACILITY_BLUETOOTH_ATT: FACILITY_CODE = 101u32;
pub const FACILITY_CERT: FACILITY_CODE = 11u32;
pub const FACILITY_CMI: FACILITY_CODE = 54u32;
pub const FACILITY_COMPLUS: FACILITY_CODE = 17u32;
pub const FACILITY_CONFIGURATION: FACILITY_CODE = 33u32;
pub const FACILITY_CONTROL: FACILITY_CODE = 10u32;
pub const FACILITY_DAF: FACILITY_CODE = 100u32;
pub const FACILITY_DEBUGGERS: FACILITY_CODE = 176u32;
pub const FACILITY_DEFRAG: FACILITY_CODE = 2304u32;
pub const FACILITY_DELIVERY_OPTIMIZATION: FACILITY_CODE = 208u32;
pub const FACILITY_DEPLOYMENT_SERVICES_BINLSVC: FACILITY_CODE = 261u32;
pub const FACILITY_DEPLOYMENT_SERVICES_CONTENT_PROVIDER: FACILITY_CODE = 293u32;
pub const FACILITY_DEPLOYMENT_SERVICES_DRIVER_PROVISIONING: FACILITY_CODE = 278u32;
pub const FACILITY_DEPLOYMENT_SERVICES_IMAGING: FACILITY_CODE = 258u32;
pub const FACILITY_DEPLOYMENT_SERVICES_MANAGEMENT: FACILITY_CODE = 259u32;
pub const FACILITY_DEPLOYMENT_SERVICES_MULTICAST_CLIENT: FACILITY_CODE = 290u32;
pub const FACILITY_DEPLOYMENT_SERVICES_MULTICAST_SERVER: FACILITY_CODE = 289u32;
pub const FACILITY_DEPLOYMENT_SERVICES_PXE: FACILITY_CODE = 263u32;
pub const FACILITY_DEPLOYMENT_SERVICES_SERVER: FACILITY_CODE = 257u32;
pub const FACILITY_DEPLOYMENT_SERVICES_TFTP: FACILITY_CODE = 264u32;
pub const FACILITY_DEPLOYMENT_SERVICES_TRANSPORT_MANAGEMENT: FACILITY_CODE = 272u32;
pub const FACILITY_DEPLOYMENT_SERVICES_UTIL: FACILITY_CODE = 260u32;
pub const FACILITY_DEVICE_UPDATE_AGENT: FACILITY_CODE = 135u32;
pub const FACILITY_DIRECT2D: FACILITY_CODE = 2201u32;
pub const FACILITY_DIRECT3D10: FACILITY_CODE = 2169u32;
pub const FACILITY_DIRECT3D11: FACILITY_CODE = 2172u32;
pub const FACILITY_DIRECT3D11_DEBUG: FACILITY_CODE = 2173u32;
pub const FACILITY_DIRECT3D12: FACILITY_CODE = 2174u32;
pub const FACILITY_DIRECT3D12_DEBUG: FACILITY_CODE = 2175u32;
pub const FACILITY_DIRECTMUSIC: FACILITY_CODE = 2168u32;
pub const FACILITY_DIRECTORYSERVICE: FACILITY_CODE = 37u32;
pub const FACILITY_DISPATCH: FACILITY_CODE = 2u32;
pub const FACILITY_DLS: FACILITY_CODE = 153u32;
pub const FACILITY_DMSERVER: FACILITY_CODE = 256u32;
pub const FACILITY_DPLAY: FACILITY_CODE = 21u32;
pub const FACILITY_DRVSERVICING: FACILITY_CODE = 136u32;
pub const FACILITY_DXCORE: FACILITY_CODE = 2176u32;
pub const FACILITY_DXGI: FACILITY_CODE = 2170u32;
pub const FACILITY_DXGI_DDI: FACILITY_CODE = 2171u32;
pub const FACILITY_EAP: FACILITY_CODE = 66u32;
pub const FACILITY_EAS: FACILITY_CODE = 85u32;
pub const FACILITY_FVE: FACILITY_CODE = 49u32;
pub const FACILITY_FWP: FACILITY_CODE = 50u32;
pub const FACILITY_GAME: FACILITY_CODE = 2340u32;
pub const FACILITY_GRAPHICS: FACILITY_CODE = 38u32;
pub const FACILITY_HSP_SERVICES: FACILITY_CODE = 296u32;
pub const FACILITY_HSP_SOFTWARE: FACILITY_CODE = 297u32;
pub const FACILITY_HTTP: FACILITY_CODE = 25u32;
pub const FACILITY_INPUT: FACILITY_CODE = 64u32;
pub const FACILITY_INTERNET: FACILITY_CODE = 12u32;
pub const FACILITY_IORING: FACILITY_CODE = 70u32;
pub const FACILITY_ITF: FACILITY_CODE = 4u32;
pub const FACILITY_JSCRIPT: FACILITY_CODE = 2306u32;
pub const FACILITY_LEAP: FACILITY_CODE = 2184u32;
pub const FACILITY_LINGUISTIC_SERVICES: FACILITY_CODE = 305u32;
pub const FACILITY_MBN: FACILITY_CODE = 84u32;
pub const FACILITY_MEDIASERVER: FACILITY_CODE = 13u32;
pub const FACILITY_METADIRECTORY: FACILITY_CODE = 35u32;
pub const FACILITY_MOBILE: FACILITY_CODE = 1793u32;
pub const FACILITY_MSMQ: FACILITY_CODE = 14u32;
pub const FACILITY_NAP: FACILITY_CODE = 39u32;
pub const FACILITY_NDIS: FACILITY_CODE = 52u32;
pub const FACILITY_NT_BIT: FACILITY_CODE = 268435456u32;
pub const FACILITY_NULL: FACILITY_CODE = 0u32;
pub const FACILITY_OCP_UPDATE_AGENT: FACILITY_CODE = 173u32;
pub const FACILITY_ONLINE_ID: FACILITY_CODE = 134u32;
pub const FACILITY_OPC: FACILITY_CODE = 81u32;
pub const FACILITY_P2P: FACILITY_CODE = 99u32;
pub const FACILITY_P2P_INT: FACILITY_CODE = 98u32;
pub const FACILITY_PARSE: FACILITY_CODE = 113u32;
pub const FACILITY_PIDGENX: FACILITY_CODE = 2561u32;
pub const FACILITY_PIX: FACILITY_CODE = 2748u32;
pub const FACILITY_PLA: FACILITY_CODE = 48u32;
pub const FACILITY_POWERSHELL: FACILITY_CODE = 84u32;
pub const FACILITY_PRESENTATION: FACILITY_CODE = 2177u32;
pub const FACILITY_QUIC: FACILITY_CODE = 65u32;
pub const FACILITY_RAS: FACILITY_CODE = 83u32;
pub const FACILITY_RESTORE: FACILITY_CODE = 256u32;
pub const FACILITY_RPC: FACILITY_CODE = 1u32;
pub const FACILITY_SCARD: FACILITY_CODE = 16u32;
pub const FACILITY_SCRIPT: FACILITY_CODE = 112u32;
pub const FACILITY_SDIAG: FACILITY_CODE = 60u32;
pub const FACILITY_SECURITY: FACILITY_CODE = 9u32;
pub const FACILITY_SERVICE_FABRIC: FACILITY_CODE = 1968u32;
pub const FACILITY_SETUPAPI: FACILITY_CODE = 15u32;
pub const FACILITY_SHELL: FACILITY_CODE = 39u32;
pub const FACILITY_SOS: FACILITY_CODE = 160u32;
pub const FACILITY_SPP: FACILITY_CODE = 256u32;
pub const FACILITY_SQLITE: FACILITY_CODE = 1967u32;
pub const FACILITY_SSPI: FACILITY_CODE = 9u32;
pub const FACILITY_STATEREPOSITORY: FACILITY_CODE = 103u32;
pub const FACILITY_STATE_MANAGEMENT: FACILITY_CODE = 34u32;
pub const FACILITY_STORAGE: FACILITY_CODE = 3u32;
pub const FACILITY_SXS: FACILITY_CODE = 23u32;
pub const FACILITY_SYNCENGINE: FACILITY_CODE = 2050u32;
pub const FACILITY_TIERING: FACILITY_CODE = 131u32;
pub const FACILITY_TPM_SERVICES: FACILITY_CODE = 40u32;
pub const FACILITY_TPM_SOFTWARE: FACILITY_CODE = 41u32;
pub const FACILITY_TTD: FACILITY_CODE = 1490u32;
pub const FACILITY_UI: FACILITY_CODE = 42u32;
pub const FACILITY_UMI: FACILITY_CODE = 22u32;
pub const FACILITY_URT: FACILITY_CODE = 19u32;
pub const FACILITY_USERMODE_COMMONLOG: FACILITY_CODE = 26u32;
pub const FACILITY_USERMODE_FILTER_MANAGER: FACILITY_CODE = 31u32;
pub const FACILITY_USERMODE_HNS: FACILITY_CODE = 59u32;
pub const FACILITY_USERMODE_HYPERVISOR: FACILITY_CODE = 53u32;
pub const FACILITY_USERMODE_LICENSING: FACILITY_CODE = 234u32;
pub const FACILITY_USERMODE_SDBUS: FACILITY_CODE = 2305u32;
pub const FACILITY_USERMODE_SPACES: FACILITY_CODE = 231u32;
pub const FACILITY_USERMODE_VHD: FACILITY_CODE = 58u32;
pub const FACILITY_USERMODE_VIRTUALIZATION: FACILITY_CODE = 55u32;
pub const FACILITY_USERMODE_VOLMGR: FACILITY_CODE = 56u32;
pub const FACILITY_USERMODE_VOLSNAP: FACILITY_CODE = 130u32;
pub const FACILITY_USER_MODE_SECURITY_CORE: FACILITY_CODE = 232u32;
pub const FACILITY_USN: FACILITY_CODE = 129u32;
pub const FACILITY_UTC: FACILITY_CODE = 1989u32;
pub const FACILITY_VISUALCPP: FACILITY_CODE = 109u32;
pub const FACILITY_WEB: FACILITY_CODE = 885u32;
pub const FACILITY_WEBSERVICES: FACILITY_CODE = 61u32;
pub const FACILITY_WEB_SOCKET: FACILITY_CODE = 886u32;
pub const FACILITY_WEP: FACILITY_CODE = 2049u32;
pub const FACILITY_WER: FACILITY_CODE = 27u32;
pub const FACILITY_WIA: FACILITY_CODE = 33u32;
pub const FACILITY_WIN32: FACILITY_CODE = 7u32;
pub const FACILITY_WINCODEC_DWRITE_DWM: FACILITY_CODE = 2200u32;
pub const FACILITY_WINDOWS: FACILITY_CODE = 8u32;
pub const FACILITY_WINDOWSUPDATE: FACILITY_CODE = 36u32;
pub const FACILITY_WINDOWS_CE: FACILITY_CODE = 24u32;
pub const FACILITY_WINDOWS_DEFENDER: FACILITY_CODE = 80u32;
pub const FACILITY_WINDOWS_SETUP: FACILITY_CODE = 48u32;
pub const FACILITY_WINDOWS_STORE: FACILITY_CODE = 63u32;
pub const FACILITY_WINML: FACILITY_CODE = 2192u32;
pub const FACILITY_WINPE: FACILITY_CODE = 61u32;
pub const FACILITY_WINRM: FACILITY_CODE = 51u32;
pub const FACILITY_WMAAECMA: FACILITY_CODE = 1996u32;
pub const FACILITY_WPN: FACILITY_CODE = 62u32;
pub const FACILITY_WSBAPP: FACILITY_CODE = 122u32;
pub const FACILITY_WSB_ONLINE: FACILITY_CODE = 133u32;
pub const FACILITY_XAML: FACILITY_CODE = 43u32;
pub const FACILITY_XBOX: FACILITY_CODE = 2339u32;
pub const FACILITY_XPS: FACILITY_CODE = 82u32;
pub const FAST_ERESOURCE_PRECONDITION_VIOLATION: BUGCHECK_ERROR = 454u32;
pub const FATAL_ABNORMAL_RESET_ERROR: BUGCHECK_ERROR = 332u32;
pub const FATAL_UNHANDLED_HARD_ERROR: BUGCHECK_ERROR = 76u32;
pub const FAT_FILE_SYSTEM: BUGCHECK_ERROR = 35u32;
pub const FAULTY_HARDWARE_CORRUPTED_PAGE: BUGCHECK_ERROR = 299u32;
pub const FILE_INITIALIZATION_FAILED: BUGCHECK_ERROR = 104u32;
pub const FILE_SYSTEM: BUGCHECK_ERROR = 34u32;
pub const FLAG_ENGINE_PRESENT: u32 = 4u32;
pub const FLAG_ENGOPT_DISALLOW_NETWORK_PATHS: u32 = 8u32;
pub const FLAG_OVERRIDE_ARM_MACHINE_TYPE: u32 = 16u32;
pub const FLOPPY_INTERNAL_ERROR: BUGCHECK_ERROR = 55u32;
pub const FLTMGR_FILE_SYSTEM: BUGCHECK_ERROR = 245u32;
pub const FORMAT_MESSAGE_ALLOCATE_BUFFER: FORMAT_MESSAGE_OPTIONS = 256u32;
pub const FORMAT_MESSAGE_ARGUMENT_ARRAY: FORMAT_MESSAGE_OPTIONS = 8192u32;
pub const FORMAT_MESSAGE_FROM_HMODULE: FORMAT_MESSAGE_OPTIONS = 2048u32;
pub const FORMAT_MESSAGE_FROM_STRING: FORMAT_MESSAGE_OPTIONS = 1024u32;
pub const FORMAT_MESSAGE_FROM_SYSTEM: FORMAT_MESSAGE_OPTIONS = 4096u32;
pub const FORMAT_MESSAGE_IGNORE_INSERTS: FORMAT_MESSAGE_OPTIONS = 512u32;
pub const FP_EMULATION_ERROR: BUGCHECK_ERROR = 166u32;
pub const FSRTL_EXTRA_CREATE_PARAMETER_VIOLATION: BUGCHECK_ERROR = 268u32;
pub const FunctionTableStream: MINIDUMP_STREAM_TYPE = 13i32;
pub const GPIO_CONTROLLER_DRIVER_ERROR: BUGCHECK_ERROR = 312u32;
pub const HAL1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 97u32;
pub const HAL_BLOCKED_PROCESSOR_INTERNAL_ERROR: BUGCHECK_ERROR = 474u32;
pub const HAL_ILLEGAL_IOMMU_PAGE_FAULT: BUGCHECK_ERROR = 345u32;
pub const HAL_INITIALIZATION_FAILED: BUGCHECK_ERROR = 92u32;
pub const HAL_IOMMU_INTERNAL_ERROR: BUGCHECK_ERROR = 473u32;
pub const HAL_MEMORY_ALLOCATION: BUGCHECK_ERROR = 172u32;
pub const HANDLE_ERROR_ON_CRITICAL_THREAD: BUGCHECK_ERROR = 493u32;
pub const HANDLE_LIVE_DUMP: BUGCHECK_ERROR = 492u32;
pub const HARDWARE_INTERRUPT_STORM: BUGCHECK_ERROR = 242u32;
pub const HARDWARE_PROFILE_DOCKED_STRING: BUGCHECK_ERROR = 1073807362u32;
pub const HARDWARE_PROFILE_UNDOCKED_STRING: BUGCHECK_ERROR = 1073807361u32;
pub const HARDWARE_PROFILE_UNKNOWN_STRING: BUGCHECK_ERROR = 1073807363u32;
pub const HARDWARE_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 463u32;
pub const HTTP_DRIVER_CORRUPTED: BUGCHECK_ERROR = 250u32;
pub const HYPERGUARD_INITIALIZATION_FAILURE: BUGCHECK_ERROR = 383u32;
pub const HYPERGUARD_VIOLATION: BUGCHECK_ERROR = 396u32;
pub const HYPERVISOR_ERROR: BUGCHECK_ERROR = 131073u32;
pub const HandleDataStream: MINIDUMP_STREAM_TYPE = 12i32;
pub const HandleOperationListStream: MINIDUMP_STREAM_TYPE = 18i32;
pub const ILLEGAL_ATS_INITIALIZATION: BUGCHECK_ERROR = 489u32;
pub const ILLEGAL_IOMMU_PAGE_FAULT: BUGCHECK_ERROR = 344u32;
pub const IMAGEHLP_GET_TYPE_INFO_CHILDREN: IMAGEHLP_GET_TYPE_INFO_FLAGS = 2u32;
pub const IMAGEHLP_GET_TYPE_INFO_UNCACHED: IMAGEHLP_GET_TYPE_INFO_FLAGS = 1u32;
pub const IMAGEHLP_MODULE_REGION_ADDITIONAL: u32 = 4u32;
pub const IMAGEHLP_MODULE_REGION_ALL: u32 = 255u32;
pub const IMAGEHLP_MODULE_REGION_DLLBASE: u32 = 1u32;
pub const IMAGEHLP_MODULE_REGION_DLLRANGE: u32 = 2u32;
pub const IMAGEHLP_MODULE_REGION_JIT: u32 = 8u32;
pub const IMAGEHLP_RMAP_BIG_ENDIAN: u32 = 2u32;
pub const IMAGEHLP_RMAP_FIXUP_ARM64X: u32 = 268435456u32;
pub const IMAGEHLP_RMAP_FIXUP_IMAGEBASE: u32 = 2147483648u32;
pub const IMAGEHLP_RMAP_IGNORE_MISCOMPARE: u32 = 4u32;
pub const IMAGEHLP_RMAP_LOAD_RW_DATA_SECTIONS: u32 = 536870912u32;
pub const IMAGEHLP_RMAP_MAPPED_FLAT: u32 = 1u32;
pub const IMAGEHLP_RMAP_OMIT_SHARED_RW_DATA_SECTIONS: u32 = 1073741824u32;
pub const IMAGEHLP_SYMBOL_FUNCTION: u32 = 2048u32;
pub const IMAGEHLP_SYMBOL_INFO_CONSTANT: u32 = 256u32;
pub const IMAGEHLP_SYMBOL_INFO_FRAMERELATIVE: u32 = 32u32;
pub const IMAGEHLP_SYMBOL_INFO_LOCAL: u32 = 128u32;
pub const IMAGEHLP_SYMBOL_INFO_PARAMETER: u32 = 64u32;
pub const IMAGEHLP_SYMBOL_INFO_REGISTER: u32 = 8u32;
pub const IMAGEHLP_SYMBOL_INFO_REGRELATIVE: u32 = 16u32;
pub const IMAGEHLP_SYMBOL_INFO_TLSRELATIVE: u32 = 16384u32;
pub const IMAGEHLP_SYMBOL_INFO_VALUEPRESENT: u32 = 1u32;
pub const IMAGEHLP_SYMBOL_THUNK: u32 = 8192u32;
pub const IMAGEHLP_SYMBOL_TYPE_INFO_MAX: IMAGEHLP_SYMBOL_TYPE_INFO = 35i32;
pub const IMAGEHLP_SYMBOL_VIRTUAL: u32 = 4096u32;
pub const IMAGE_DEBUG_TYPE_BORLAND: IMAGE_DEBUG_TYPE = 9u32;
pub const IMAGE_DEBUG_TYPE_CODEVIEW: IMAGE_DEBUG_TYPE = 2u32;
pub const IMAGE_DEBUG_TYPE_COFF: IMAGE_DEBUG_TYPE = 1u32;
pub const IMAGE_DEBUG_TYPE_EXCEPTION: IMAGE_DEBUG_TYPE = 5u32;
pub const IMAGE_DEBUG_TYPE_FIXUP: IMAGE_DEBUG_TYPE = 6u32;
pub const IMAGE_DEBUG_TYPE_FPO: IMAGE_DEBUG_TYPE = 3u32;
pub const IMAGE_DEBUG_TYPE_MISC: IMAGE_DEBUG_TYPE = 4u32;
pub const IMAGE_DEBUG_TYPE_UNKNOWN: IMAGE_DEBUG_TYPE = 0u32;
pub const IMAGE_DIRECTORY_ENTRY_ARCHITECTURE: IMAGE_DIRECTORY_ENTRY = 7u16;
pub const IMAGE_DIRECTORY_ENTRY_BASERELOC: IMAGE_DIRECTORY_ENTRY = 5u16;
pub const IMAGE_DIRECTORY_ENTRY_BOUND_IMPORT: IMAGE_DIRECTORY_ENTRY = 11u16;
pub const IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR: IMAGE_DIRECTORY_ENTRY = 14u16;
pub const IMAGE_DIRECTORY_ENTRY_DEBUG: IMAGE_DIRECTORY_ENTRY = 6u16;
pub const IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT: IMAGE_DIRECTORY_ENTRY = 13u16;
pub const IMAGE_DIRECTORY_ENTRY_EXCEPTION: IMAGE_DIRECTORY_ENTRY = 3u16;
pub const IMAGE_DIRECTORY_ENTRY_EXPORT: IMAGE_DIRECTORY_ENTRY = 0u16;
pub const IMAGE_DIRECTORY_ENTRY_GLOBALPTR: IMAGE_DIRECTORY_ENTRY = 8u16;
pub const IMAGE_DIRECTORY_ENTRY_IAT: IMAGE_DIRECTORY_ENTRY = 12u16;
pub const IMAGE_DIRECTORY_ENTRY_IMPORT: IMAGE_DIRECTORY_ENTRY = 1u16;
pub const IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG: IMAGE_DIRECTORY_ENTRY = 10u16;
pub const IMAGE_DIRECTORY_ENTRY_RESOURCE: IMAGE_DIRECTORY_ENTRY = 2u16;
pub const IMAGE_DIRECTORY_ENTRY_SECURITY: IMAGE_DIRECTORY_ENTRY = 4u16;
pub const IMAGE_DIRECTORY_ENTRY_TLS: IMAGE_DIRECTORY_ENTRY = 9u16;
pub const IMAGE_DLLCHARACTERISTICS_APPCONTAINER: IMAGE_DLL_CHARACTERISTICS = 4096u16;
pub const IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE: IMAGE_DLL_CHARACTERISTICS = 64u16;
pub const IMAGE_DLLCHARACTERISTICS_EX_CET_COMPAT: IMAGE_DLL_CHARACTERISTICS = 1u16;
pub const IMAGE_DLLCHARACTERISTICS_EX_CET_COMPAT_STRICT_MODE: IMAGE_DLL_CHARACTERISTICS = 2u16;
pub const IMAGE_DLLCHARACTERISTICS_EX_CET_DYNAMIC_APIS_ALLOW_IN_PROC: IMAGE_DLL_CHARACTERISTICS = 8u16;
pub const IMAGE_DLLCHARACTERISTICS_EX_CET_RESERVED_1: IMAGE_DLL_CHARACTERISTICS = 16u16;
pub const IMAGE_DLLCHARACTERISTICS_EX_CET_RESERVED_2: IMAGE_DLL_CHARACTERISTICS = 32u16;
pub const IMAGE_DLLCHARACTERISTICS_EX_CET_SET_CONTEXT_IP_VALIDATION_RELAXED_MODE: IMAGE_DLL_CHARACTERISTICS = 4u16;
pub const IMAGE_DLLCHARACTERISTICS_FORCE_INTEGRITY: IMAGE_DLL_CHARACTERISTICS = 128u16;
pub const IMAGE_DLLCHARACTERISTICS_GUARD_CF: IMAGE_DLL_CHARACTERISTICS = 16384u16;
pub const IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA: IMAGE_DLL_CHARACTERISTICS = 32u16;
pub const IMAGE_DLLCHARACTERISTICS_NO_BIND: IMAGE_DLL_CHARACTERISTICS = 2048u16;
pub const IMAGE_DLLCHARACTERISTICS_NO_ISOLATION: IMAGE_DLL_CHARACTERISTICS = 512u16;
pub const IMAGE_DLLCHARACTERISTICS_NO_SEH: IMAGE_DLL_CHARACTERISTICS = 1024u16;
pub const IMAGE_DLLCHARACTERISTICS_NX_COMPAT: IMAGE_DLL_CHARACTERISTICS = 256u16;
pub const IMAGE_DLLCHARACTERISTICS_TERMINAL_SERVER_AWARE: IMAGE_DLL_CHARACTERISTICS = 32768u16;
pub const IMAGE_DLLCHARACTERISTICS_WDM_DRIVER: IMAGE_DLL_CHARACTERISTICS = 8192u16;
pub const IMAGE_FILE_32BIT_MACHINE: IMAGE_FILE_CHARACTERISTICS = 256u16;
pub const IMAGE_FILE_32BIT_MACHINE2: IMAGE_FILE_CHARACTERISTICS2 = 256u32;
pub const IMAGE_FILE_AGGRESIVE_WS_TRIM: IMAGE_FILE_CHARACTERISTICS = 16u16;
pub const IMAGE_FILE_AGGRESIVE_WS_TRIM2: IMAGE_FILE_CHARACTERISTICS2 = 16u32;
pub const IMAGE_FILE_BYTES_REVERSED_HI: IMAGE_FILE_CHARACTERISTICS = 32768u16;
pub const IMAGE_FILE_BYTES_REVERSED_HI_2: IMAGE_FILE_CHARACTERISTICS2 = 32768u32;
pub const IMAGE_FILE_BYTES_REVERSED_LO: IMAGE_FILE_CHARACTERISTICS = 128u16;
pub const IMAGE_FILE_BYTES_REVERSED_LO2: IMAGE_FILE_CHARACTERISTICS2 = 128u32;
pub const IMAGE_FILE_DEBUG_STRIPPED: IMAGE_FILE_CHARACTERISTICS = 512u16;
pub const IMAGE_FILE_DEBUG_STRIPPED2: IMAGE_FILE_CHARACTERISTICS2 = 512u32;
pub const IMAGE_FILE_DLL: IMAGE_FILE_CHARACTERISTICS = 8192u16;
pub const IMAGE_FILE_DLL_2: IMAGE_FILE_CHARACTERISTICS2 = 8192u32;
pub const IMAGE_FILE_EXECUTABLE_IMAGE: IMAGE_FILE_CHARACTERISTICS = 2u16;
pub const IMAGE_FILE_EXECUTABLE_IMAGE2: IMAGE_FILE_CHARACTERISTICS2 = 2u32;
pub const IMAGE_FILE_LARGE_ADDRESS_AWARE: IMAGE_FILE_CHARACTERISTICS = 32u16;
pub const IMAGE_FILE_LARGE_ADDRESS_AWARE2: IMAGE_FILE_CHARACTERISTICS2 = 32u32;
pub const IMAGE_FILE_LINE_NUMS_STRIPPED: IMAGE_FILE_CHARACTERISTICS = 4u16;
pub const IMAGE_FILE_LINE_NUMS_STRIPPED2: IMAGE_FILE_CHARACTERISTICS2 = 4u32;
pub const IMAGE_FILE_LOCAL_SYMS_STRIPPED: IMAGE_FILE_CHARACTERISTICS = 8u16;
pub const IMAGE_FILE_LOCAL_SYMS_STRIPPED2: IMAGE_FILE_CHARACTERISTICS2 = 8u32;
pub const IMAGE_FILE_NET_RUN_FROM_SWAP: IMAGE_FILE_CHARACTERISTICS = 2048u16;
pub const IMAGE_FILE_NET_RUN_FROM_SWAP2: IMAGE_FILE_CHARACTERISTICS2 = 2048u32;
pub const IMAGE_FILE_RELOCS_STRIPPED: IMAGE_FILE_CHARACTERISTICS = 1u16;
pub const IMAGE_FILE_RELOCS_STRIPPED2: IMAGE_FILE_CHARACTERISTICS2 = 1u32;
pub const IMAGE_FILE_REMOVABLE_RUN_FROM_SWAP: IMAGE_FILE_CHARACTERISTICS = 1024u16;
pub const IMAGE_FILE_REMOVABLE_RUN_FROM_SWAP2: IMAGE_FILE_CHARACTERISTICS2 = 1024u32;
pub const IMAGE_FILE_SYSTEM: IMAGE_FILE_CHARACTERISTICS = 4096u16;
pub const IMAGE_FILE_SYSTEM_2: IMAGE_FILE_CHARACTERISTICS2 = 4096u32;
pub const IMAGE_FILE_UP_SYSTEM_ONLY: IMAGE_FILE_CHARACTERISTICS = 16384u16;
pub const IMAGE_FILE_UP_SYSTEM_ONLY_2: IMAGE_FILE_CHARACTERISTICS2 = 16384u32;
pub const IMAGE_NT_OPTIONAL_HDR32_MAGIC: IMAGE_OPTIONAL_HEADER_MAGIC = 267u16;
pub const IMAGE_NT_OPTIONAL_HDR64_MAGIC: IMAGE_OPTIONAL_HEADER_MAGIC = 523u16;
pub const IMAGE_NT_OPTIONAL_HDR_MAGIC: IMAGE_OPTIONAL_HEADER_MAGIC = 523u16;
pub const IMAGE_ROM_OPTIONAL_HDR_MAGIC: IMAGE_OPTIONAL_HEADER_MAGIC = 263u16;
pub const IMAGE_SCN_ALIGN_1024BYTES: IMAGE_SECTION_CHARACTERISTICS = 11534336u32;
pub const IMAGE_SCN_ALIGN_128BYTES: IMAGE_SECTION_CHARACTERISTICS = 8388608u32;
pub const IMAGE_SCN_ALIGN_16BYTES: IMAGE_SECTION_CHARACTERISTICS = 5242880u32;
pub const IMAGE_SCN_ALIGN_1BYTES: IMAGE_SECTION_CHARACTERISTICS = 1048576u32;
pub const IMAGE_SCN_ALIGN_2048BYTES: IMAGE_SECTION_CHARACTERISTICS = 12582912u32;
pub const IMAGE_SCN_ALIGN_256BYTES: IMAGE_SECTION_CHARACTERISTICS = 9437184u32;
pub const IMAGE_SCN_ALIGN_2BYTES: IMAGE_SECTION_CHARACTERISTICS = 2097152u32;
pub const IMAGE_SCN_ALIGN_32BYTES: IMAGE_SECTION_CHARACTERISTICS = 6291456u32;
pub const IMAGE_SCN_ALIGN_4096BYTES: IMAGE_SECTION_CHARACTERISTICS = 13631488u32;
pub const IMAGE_SCN_ALIGN_4BYTES: IMAGE_SECTION_CHARACTERISTICS = 3145728u32;
pub const IMAGE_SCN_ALIGN_512BYTES: IMAGE_SECTION_CHARACTERISTICS = 10485760u32;
pub const IMAGE_SCN_ALIGN_64BYTES: IMAGE_SECTION_CHARACTERISTICS = 7340032u32;
pub const IMAGE_SCN_ALIGN_8192BYTES: IMAGE_SECTION_CHARACTERISTICS = 14680064u32;
pub const IMAGE_SCN_ALIGN_8BYTES: IMAGE_SECTION_CHARACTERISTICS = 4194304u32;
pub const IMAGE_SCN_ALIGN_MASK: IMAGE_SECTION_CHARACTERISTICS = 15728640u32;
pub const IMAGE_SCN_CNT_CODE: IMAGE_SECTION_CHARACTERISTICS = 32u32;
pub const IMAGE_SCN_CNT_INITIALIZED_DATA: IMAGE_SECTION_CHARACTERISTICS = 64u32;
pub const IMAGE_SCN_CNT_UNINITIALIZED_DATA: IMAGE_SECTION_CHARACTERISTICS = 128u32;
pub const IMAGE_SCN_GPREL: IMAGE_SECTION_CHARACTERISTICS = 32768u32;
pub const IMAGE_SCN_LNK_COMDAT: IMAGE_SECTION_CHARACTERISTICS = 4096u32;
pub const IMAGE_SCN_LNK_INFO: IMAGE_SECTION_CHARACTERISTICS = 512u32;
pub const IMAGE_SCN_LNK_NRELOC_OVFL: IMAGE_SECTION_CHARACTERISTICS = 16777216u32;
pub const IMAGE_SCN_LNK_OTHER: IMAGE_SECTION_CHARACTERISTICS = 256u32;
pub const IMAGE_SCN_LNK_REMOVE: IMAGE_SECTION_CHARACTERISTICS = 2048u32;
pub const IMAGE_SCN_MEM_16BIT: IMAGE_SECTION_CHARACTERISTICS = 131072u32;
pub const IMAGE_SCN_MEM_DISCARDABLE: IMAGE_SECTION_CHARACTERISTICS = 33554432u32;
pub const IMAGE_SCN_MEM_EXECUTE: IMAGE_SECTION_CHARACTERISTICS = 536870912u32;
pub const IMAGE_SCN_MEM_FARDATA: IMAGE_SECTION_CHARACTERISTICS = 32768u32;
pub const IMAGE_SCN_MEM_LOCKED: IMAGE_SECTION_CHARACTERISTICS = 262144u32;
pub const IMAGE_SCN_MEM_NOT_CACHED: IMAGE_SECTION_CHARACTERISTICS = 67108864u32;
pub const IMAGE_SCN_MEM_NOT_PAGED: IMAGE_SECTION_CHARACTERISTICS = 134217728u32;
pub const IMAGE_SCN_MEM_PRELOAD: IMAGE_SECTION_CHARACTERISTICS = 524288u32;
pub const IMAGE_SCN_MEM_PURGEABLE: IMAGE_SECTION_CHARACTERISTICS = 131072u32;
pub const IMAGE_SCN_MEM_READ: IMAGE_SECTION_CHARACTERISTICS = 1073741824u32;
pub const IMAGE_SCN_MEM_SHARED: IMAGE_SECTION_CHARACTERISTICS = 268435456u32;
pub const IMAGE_SCN_MEM_WRITE: IMAGE_SECTION_CHARACTERISTICS = 2147483648u32;
pub const IMAGE_SCN_NO_DEFER_SPEC_EXC: IMAGE_SECTION_CHARACTERISTICS = 16384u32;
pub const IMAGE_SCN_SCALE_INDEX: IMAGE_SECTION_CHARACTERISTICS = 1u32;
pub const IMAGE_SCN_TYPE_NO_PAD: IMAGE_SECTION_CHARACTERISTICS = 8u32;
pub const IMAGE_SUBSYSTEM_EFI_APPLICATION: IMAGE_SUBSYSTEM = 10u16;
pub const IMAGE_SUBSYSTEM_EFI_BOOT_SERVICE_DRIVER: IMAGE_SUBSYSTEM = 11u16;
pub const IMAGE_SUBSYSTEM_EFI_ROM: IMAGE_SUBSYSTEM = 13u16;
pub const IMAGE_SUBSYSTEM_EFI_RUNTIME_DRIVER: IMAGE_SUBSYSTEM = 12u16;
pub const IMAGE_SUBSYSTEM_NATIVE: IMAGE_SUBSYSTEM = 1u16;
pub const IMAGE_SUBSYSTEM_NATIVE_WINDOWS: IMAGE_SUBSYSTEM = 8u16;
pub const IMAGE_SUBSYSTEM_OS2_CUI: IMAGE_SUBSYSTEM = 5u16;
pub const IMAGE_SUBSYSTEM_POSIX_CUI: IMAGE_SUBSYSTEM = 7u16;
pub const IMAGE_SUBSYSTEM_UNKNOWN: IMAGE_SUBSYSTEM = 0u16;
pub const IMAGE_SUBSYSTEM_WINDOWS_BOOT_APPLICATION: IMAGE_SUBSYSTEM = 16u16;
pub const IMAGE_SUBSYSTEM_WINDOWS_CE_GUI: IMAGE_SUBSYSTEM = 9u16;
pub const IMAGE_SUBSYSTEM_WINDOWS_CUI: IMAGE_SUBSYSTEM = 3u16;
pub const IMAGE_SUBSYSTEM_WINDOWS_GUI: IMAGE_SUBSYSTEM = 2u16;
pub const IMAGE_SUBSYSTEM_XBOX: IMAGE_SUBSYSTEM = 14u16;
pub const IMAGE_SUBSYSTEM_XBOX_CODE_CATALOG: IMAGE_SUBSYSTEM = 17u16;
pub const IMPERSONATING_WORKER_THREAD: BUGCHECK_ERROR = 223u32;
pub const INACCESSIBLE_BOOT_DEVICE: BUGCHECK_ERROR = 123u32;
pub const INCONSISTENT_IRP: BUGCHECK_ERROR = 42u32;
pub const INLINE_FRAME_CONTEXT_IGNORE: u32 = 4294967295u32;
pub const INLINE_FRAME_CONTEXT_INIT: u32 = 0u32;
pub const INSTALL_MORE_MEMORY: BUGCHECK_ERROR = 125u32;
pub const INSTRUCTION_BUS_ERROR: BUGCHECK_ERROR = 47u32;
pub const INSTRUCTION_COHERENCY_EXCEPTION: BUGCHECK_ERROR = 86u32;
pub const INSUFFICIENT_SYSTEM_MAP_REGS: BUGCHECK_ERROR = 69u32;
pub const INTERFACESAFE_FOR_UNTRUSTED_CALLER: u32 = 1u32;
pub const INTERFACESAFE_FOR_UNTRUSTED_DATA: u32 = 2u32;
pub const INTERFACE_USES_DISPEX: u32 = 4u32;
pub const INTERFACE_USES_SECURITY_MANAGER: u32 = 8u32;
pub const INTERNAL_POWER_ERROR: BUGCHECK_ERROR = 160u32;
pub const INTERRUPT_EXCEPTION_NOT_HANDLED: BUGCHECK_ERROR = 61u32;
pub const INTERRUPT_UNWIND_ATTEMPTED: BUGCHECK_ERROR = 60u32;
pub const INVALID_AFFINITY_SET: BUGCHECK_ERROR = 3u32;
pub const INVALID_ALTERNATE_SYSTEM_CALL_HANDLER_REGISTRATION: BUGCHECK_ERROR = 480u32;
pub const INVALID_CALLBACK_STACK_ADDRESS: BUGCHECK_ERROR = 461u32;
pub const INVALID_CANCEL_OF_FILE_OPEN: BUGCHECK_ERROR = 232u32;
pub const INVALID_DATA_ACCESS_TRAP: BUGCHECK_ERROR = 4u32;
pub const INVALID_DRIVER_HANDLE: BUGCHECK_ERROR = 287u32;
pub const INVALID_EXTENDED_PROCESSOR_STATE: BUGCHECK_ERROR = 305u32;
pub const INVALID_FLOATING_POINT_STATE: BUGCHECK_ERROR = 231u32;
pub const INVALID_HIBERNATED_STATE: BUGCHECK_ERROR = 189u32;
pub const INVALID_IO_BOOST_STATE: BUGCHECK_ERROR = 316u32;
pub const INVALID_KERNEL_HANDLE: BUGCHECK_ERROR = 147u32;
pub const INVALID_KERNEL_STACK_ADDRESS: BUGCHECK_ERROR = 462u32;
pub const INVALID_MDL_RANGE: BUGCHECK_ERROR = 302u32;
pub const INVALID_PROCESS_ATTACH_ATTEMPT: BUGCHECK_ERROR = 5u32;
pub const INVALID_PROCESS_DETACH_ATTEMPT: BUGCHECK_ERROR = 6u32;
pub const INVALID_PUSH_LOCK_FLAGS: BUGCHECK_ERROR = 338u32;
pub const INVALID_REGION_OR_SEGMENT: BUGCHECK_ERROR = 153u32;
pub const INVALID_RUNDOWN_PROTECTION_FLAGS: BUGCHECK_ERROR = 364u32;
pub const INVALID_SILO_DETACH: BUGCHECK_ERROR = 459u32;
pub const INVALID_SLOT_ALLOCATOR_FLAGS: BUGCHECK_ERROR = 365u32;
pub const INVALID_SOFTWARE_INTERRUPT: BUGCHECK_ERROR = 7u32;
pub const INVALID_THREAD_AFFINITY_STATE: BUGCHECK_ERROR = 488u32;
pub const INVALID_WORK_QUEUE_ITEM: BUGCHECK_ERROR = 150u32;
pub const IO1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 105u32;
pub const IOCTL_IPMI_INTERNAL_RECORD_SEL_EVENT: u32 = 2232320u32;
pub const IORING: BUGCHECK_ERROR = 499u32;
pub const IO_OBJECT_INVALID: BUGCHECK_ERROR = 328u32;
pub const IO_THREADPOOL_DEADLOCK_LIVEDUMP: BUGCHECK_ERROR = 453u32;
pub const IPI_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 475u32;
pub const IPMI_IOCTL_INDEX: u32 = 1024u32;
pub const IPMI_OS_SEL_RECORD_MASK: u32 = 65535u32;
pub const IPMI_OS_SEL_RECORD_VERSION: u32 = 1u32;
pub const IPMI_OS_SEL_RECORD_VERSION_1: u32 = 1u32;
pub const IRQL_GT_ZERO_AT_SYSTEM_SERVICE: BUGCHECK_ERROR = 74u32;
pub const IRQL_NOT_DISPATCH_LEVEL: BUGCHECK_ERROR = 8u32;
pub const IRQL_NOT_GREATER_OR_EQUAL: BUGCHECK_ERROR = 9u32;
pub const IRQL_NOT_LESS_OR_EQUAL: BUGCHECK_ERROR = 10u32;
pub const IRQL_UNEXPECTED_VALUE: BUGCHECK_ERROR = 200u32;
pub const IncludeModuleCallback: MINIDUMP_CALLBACK_TYPE = 4i32;
pub const IncludeThreadCallback: MINIDUMP_CALLBACK_TYPE = 3i32;
pub const IncludeVmRegionCallback: MINIDUMP_CALLBACK_TYPE = 10i32;
pub const IoFinishCallback: MINIDUMP_CALLBACK_TYPE = 13i32;
pub const IoStartCallback: MINIDUMP_CALLBACK_TYPE = 11i32;
pub const IoWriteAllCallback: MINIDUMP_CALLBACK_TYPE = 12i32;
pub const IpmiOsSelRecordTypeBugcheckData: IPMI_OS_SEL_RECORD_TYPE = 9i32;
pub const IpmiOsSelRecordTypeBugcheckRecovery: IPMI_OS_SEL_RECORD_TYPE = 8i32;
pub const IpmiOsSelRecordTypeDriver: IPMI_OS_SEL_RECORD_TYPE = 7i32;
pub const IpmiOsSelRecordTypeMax: IPMI_OS_SEL_RECORD_TYPE = 10i32;
pub const IpmiOsSelRecordTypeOther: IPMI_OS_SEL_RECORD_TYPE = 1i32;
pub const IpmiOsSelRecordTypeRaw: IPMI_OS_SEL_RECORD_TYPE = 6i32;
pub const IpmiOsSelRecordTypeWhea: IPMI_OS_SEL_RECORD_TYPE = 0i32;
pub const IpmiOsSelRecordTypeWheaErrorNmi: IPMI_OS_SEL_RECORD_TYPE = 4i32;
pub const IpmiOsSelRecordTypeWheaErrorOther: IPMI_OS_SEL_RECORD_TYPE = 5i32;
pub const IpmiOsSelRecordTypeWheaErrorPci: IPMI_OS_SEL_RECORD_TYPE = 3i32;
pub const IpmiOsSelRecordTypeWheaErrorXpfMca: IPMI_OS_SEL_RECORD_TYPE = 2i32;
pub const IptTraceStream: MINIDUMP_STREAM_TYPE = 23i32;
pub const IsProcessSnapshotCallback: MINIDUMP_CALLBACK_TYPE = 16i32;
pub const JavaScriptDataStream: MINIDUMP_STREAM_TYPE = 20i32;
pub const KASAN_ENLIGHTENMENT_VIOLATION: BUGCHECK_ERROR = 497u32;
pub const KASAN_ILLEGAL_ACCESS: BUGCHECK_ERROR = 498u32;
pub const KERNEL_APC_PENDING_DURING_EXIT: BUGCHECK_ERROR = 32u32;
pub const KERNEL_AUTO_BOOST_INVALID_LOCK_RELEASE: BUGCHECK_ERROR = 354u32;
pub const KERNEL_AUTO_BOOST_LOCK_ACQUISITION_WITH_RAISED_IRQL: BUGCHECK_ERROR = 402u32;
pub const KERNEL_CFG_INIT_FAILURE: BUGCHECK_ERROR = 1058u32;
pub const KERNEL_DATA_INPAGE_ERROR: BUGCHECK_ERROR = 122u32;
pub const KERNEL_EXPAND_STACK_ACTIVE: BUGCHECK_ERROR = 263u32;
pub const KERNEL_LOCK_ENTRY_LEAKED_ON_THREAD_TERMINATION: BUGCHECK_ERROR = 339u32;
pub const KERNEL_MODE_EXCEPTION_NOT_HANDLED: BUGCHECK_ERROR = 142u32;
pub const KERNEL_MODE_EXCEPTION_NOT_HANDLED_M: BUGCHECK_ERROR = 268435598u32;
pub const KERNEL_MODE_HEAP_CORRUPTION: BUGCHECK_ERROR = 314u32;
pub const KERNEL_PARTITION_REFERENCE_VIOLATION: BUGCHECK_ERROR = 398u32;
pub const KERNEL_SECURITY_CHECK_FAILURE: BUGCHECK_ERROR = 313u32;
pub const KERNEL_STACK_INPAGE_ERROR: BUGCHECK_ERROR = 119u32;
pub const KERNEL_STACK_LOCKED_AT_EXIT: BUGCHECK_ERROR = 148u32;
pub const KERNEL_STORAGE_SLOT_IN_USE: BUGCHECK_ERROR = 409u32;
pub const KERNEL_THREAD_PRIORITY_FLOOR_VIOLATION: BUGCHECK_ERROR = 343u32;
pub const KERNEL_WMI_INTERNAL: BUGCHECK_ERROR = 330u32;
pub const KMODE_EXCEPTION_NOT_HANDLED: BUGCHECK_ERROR = 30u32;
pub const KernelMinidumpStatusCallback: MINIDUMP_CALLBACK_TYPE = 8i32;
pub const LAST_CHANCE_CALLED_FROM_KMODE: BUGCHECK_ERROR = 21u32;
pub const LIVE_SYSTEM_DUMP: BUGCHECK_ERROR = 353u32;
pub const LM_SERVER_INTERNAL_ERROR: BUGCHECK_ERROR = 84u32;
pub const LOADER_BLOCK_MISMATCH: BUGCHECK_ERROR = 256u32;
pub const LOADER_ROLLBACK_DETECTED: BUGCHECK_ERROR = 406u32;
pub const LOAD_DLL_DEBUG_EVENT: DEBUG_EVENT_CODE = 6u32;
pub const LOCKED_PAGES_TRACKER_CORRUPTION: BUGCHECK_ERROR = 217u32;
pub const LPC_INITIALIZATION_FAILED: BUGCHECK_ERROR = 106u32;
pub const LastReservedStream: MINIDUMP_STREAM_TYPE = 65535i32;
pub const MACHINE_CHECK_EXCEPTION: BUGCHECK_ERROR = 156u32;
pub const MAILSLOT_FILE_SYSTEM: BUGCHECK_ERROR = 82u32;
pub const MANUALLY_INITIATED_BLACKSCREEN_HOTKEY_LIVE_DUMP: BUGCHECK_ERROR = 8648u32;
pub const MANUALLY_INITIATED_CRASH: BUGCHECK_ERROR = 226u32;
pub const MANUALLY_INITIATED_CRASH1: BUGCHECK_ERROR = 3735936685u32;
pub const MANUALLY_INITIATED_POWER_BUTTON_HOLD: BUGCHECK_ERROR = 456u32;
pub const MANUALLY_INITIATED_POWER_BUTTON_HOLD_LIVE_DUMP: BUGCHECK_ERROR = 4552u32;
pub const MAXIMUM_WAIT_OBJECTS_EXCEEDED: BUGCHECK_ERROR = 12u32;
pub const MAX_SYM_NAME: u32 = 2000u32;
pub const MBR_CHECKSUM_MISMATCH: BUGCHECK_ERROR = 139u32;
pub const MDL_CACHE: BUGCHECK_ERROR = 500u32;
pub const MEMORY1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 101u32;
pub const MEMORY_IMAGE_CORRUPT: BUGCHECK_ERROR = 162u32;
pub const MEMORY_MANAGEMENT: BUGCHECK_ERROR = 26u32;
pub const MICROCODE_REVISION_MISMATCH: BUGCHECK_ERROR = 382u32;
pub const MINIDUMP_MISC1_PROCESSOR_POWER_INFO: u32 = 4u32;
pub const MINIDUMP_MISC1_PROCESS_ID: MINIDUMP_MISC_INFO_FLAGS = 1u32;
pub const MINIDUMP_MISC1_PROCESS_TIMES: MINIDUMP_MISC_INFO_FLAGS = 2u32;
pub const MINIDUMP_MISC3_PROCESS_EXECUTE_FLAGS: u32 = 32u32;
pub const MINIDUMP_MISC3_PROCESS_INTEGRITY: u32 = 16u32;
pub const MINIDUMP_MISC3_PROTECTED_PROCESS: u32 = 128u32;
pub const MINIDUMP_MISC3_TIMEZONE: u32 = 64u32;
pub const MINIDUMP_MISC4_BUILDSTRING: u32 = 256u32;
pub const MINIDUMP_MISC5_PROCESS_COOKIE: u32 = 512u32;
pub const MINIDUMP_PROCESS_VM_COUNTERS: u32 = 1u32;
pub const MINIDUMP_PROCESS_VM_COUNTERS_EX: u32 = 4u32;
pub const MINIDUMP_PROCESS_VM_COUNTERS_EX2: u32 = 8u32;
pub const MINIDUMP_PROCESS_VM_COUNTERS_JOB: u32 = 16u32;
pub const MINIDUMP_PROCESS_VM_COUNTERS_VIRTUALSIZE: u32 = 2u32;
pub const MINIDUMP_SYSMEMINFO1_BASICPERF: u32 = 2u32;
pub const MINIDUMP_SYSMEMINFO1_FILECACHE_TRANSITIONREPURPOSECOUNT_FLAGS: u32 = 1u32;
pub const MINIDUMP_SYSMEMINFO1_PERF_CCTOTALDIRTYPAGES_CCDIRTYPAGETHRESHOLD: u32 = 4u32;
pub const MINIDUMP_SYSMEMINFO1_PERF_RESIDENTAVAILABLEPAGES_SHAREDCOMMITPAGES: u32 = 8u32;
pub const MINIDUMP_THREAD_INFO_ERROR_THREAD: MINIDUMP_THREAD_INFO_DUMP_FLAGS = 1u32;
pub const MINIDUMP_THREAD_INFO_EXITED_THREAD: MINIDUMP_THREAD_INFO_DUMP_FLAGS = 4u32;
pub const MINIDUMP_THREAD_INFO_INVALID_CONTEXT: MINIDUMP_THREAD_INFO_DUMP_FLAGS = 16u32;
pub const MINIDUMP_THREAD_INFO_INVALID_INFO: MINIDUMP_THREAD_INFO_DUMP_FLAGS = 8u32;
pub const MINIDUMP_THREAD_INFO_INVALID_TEB: MINIDUMP_THREAD_INFO_DUMP_FLAGS = 32u32;
pub const MINIDUMP_THREAD_INFO_WRITING_THREAD: MINIDUMP_THREAD_INFO_DUMP_FLAGS = 2u32;
pub const MINIDUMP_VERSION: u32 = 42899u32;
pub const MISALIGNED_POINTER_PARAMETER: BUGCHECK_ERROR = 502u32;
pub const MISMATCHED_HAL: BUGCHECK_ERROR = 121u32;
pub const MPSDRV_QUERY_USER: BUGCHECK_ERROR = 1073742318u32;
pub const MSRPC_STATE_VIOLATION: BUGCHECK_ERROR = 274u32;
pub const MUI_NO_VALID_SYSTEM_LANGUAGE: BUGCHECK_ERROR = 298u32;
pub const MULTIPLE_IRP_COMPLETE_REQUESTS: BUGCHECK_ERROR = 68u32;
pub const MULTIPROCESSOR_CONFIGURATION_NOT_SUPPORTED: BUGCHECK_ERROR = 62u32;
pub const MUP_FILE_SYSTEM: BUGCHECK_ERROR = 259u32;
pub const MUST_SUCCEED_POOL_EMPTY: BUGCHECK_ERROR = 65u32;
pub const MUTEX_ALREADY_OWNED: BUGCHECK_ERROR = 191u32;
pub const MUTEX_LEVEL_NUMBER_VIOLATION: BUGCHECK_ERROR = 13u32;
pub const Memory64ListStream: MINIDUMP_STREAM_TYPE = 9i32;
pub const MemoryCallback: MINIDUMP_CALLBACK_TYPE = 5i32;
pub const MemoryInfoListStream: MINIDUMP_STREAM_TYPE = 16i32;
pub const MemoryListStream: MINIDUMP_STREAM_TYPE = 5i32;
pub const MiniDumpFilterMemory: MINIDUMP_TYPE = 8i32;
pub const MiniDumpFilterModulePaths: MINIDUMP_TYPE = 128i32;
pub const MiniDumpFilterTriage: MINIDUMP_TYPE = 1048576i32;
pub const MiniDumpFilterWriteCombinedMemory: MINIDUMP_TYPE = 16777216i32;
pub const MiniDumpIgnoreInaccessibleMemory: MINIDUMP_TYPE = 131072i32;
pub const MiniDumpNormal: MINIDUMP_TYPE = 0i32;
pub const MiniDumpScanInaccessiblePartialPages: MINIDUMP_TYPE = 8388608i32;
pub const MiniDumpScanMemory: MINIDUMP_TYPE = 16i32;
pub const MiniDumpValidTypeFlags: MINIDUMP_TYPE = 33554431i32;
pub const MiniDumpWithAvxXStateContext: MINIDUMP_TYPE = 2097152i32;
pub const MiniDumpWithCodeSegs: MINIDUMP_TYPE = 8192i32;
pub const MiniDumpWithDataSegs: MINIDUMP_TYPE = 1i32;
pub const MiniDumpWithFullAuxiliaryState: MINIDUMP_TYPE = 32768i32;
pub const MiniDumpWithFullMemory: MINIDUMP_TYPE = 2i32;
pub const MiniDumpWithFullMemoryInfo: MINIDUMP_TYPE = 2048i32;
pub const MiniDumpWithHandleData: MINIDUMP_TYPE = 4i32;
pub const MiniDumpWithIndirectlyReferencedMemory: MINIDUMP_TYPE = 64i32;
pub const MiniDumpWithIptTrace: MINIDUMP_TYPE = 4194304i32;
pub const MiniDumpWithModuleHeaders: MINIDUMP_TYPE = 524288i32;
pub const MiniDumpWithPrivateReadWriteMemory: MINIDUMP_TYPE = 512i32;
pub const MiniDumpWithPrivateWriteCopyMemory: MINIDUMP_TYPE = 65536i32;
pub const MiniDumpWithProcessThreadData: MINIDUMP_TYPE = 256i32;
pub const MiniDumpWithThreadInfo: MINIDUMP_TYPE = 4096i32;
pub const MiniDumpWithTokenInformation: MINIDUMP_TYPE = 262144i32;
pub const MiniDumpWithUnloadedModules: MINIDUMP_TYPE = 32i32;
pub const MiniDumpWithoutAuxiliaryState: MINIDUMP_TYPE = 16384i32;
pub const MiniDumpWithoutOptionalData: MINIDUMP_TYPE = 1024i32;
pub const MiniEventInformation1: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 6i32;
pub const MiniHandleObjectInformationNone: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 0i32;
pub const MiniHandleObjectInformationTypeMax: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 9i32;
pub const MiniMutantInformation1: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 2i32;
pub const MiniMutantInformation2: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 3i32;
pub const MiniProcessInformation1: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 4i32;
pub const MiniProcessInformation2: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 5i32;
pub const MiniSecondaryValidFlags: MINIDUMP_SECONDARY_FLAGS = 1i32;
pub const MiniSecondaryWithoutPowerInfo: MINIDUMP_SECONDARY_FLAGS = 1i32;
pub const MiniSectionInformation1: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 7i32;
pub const MiniSemaphoreInformation1: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 8i32;
pub const MiniThreadInformation1: MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = 1i32;
pub const MiscInfoStream: MINIDUMP_STREAM_TYPE = 15i32;
pub const ModuleCallback: MINIDUMP_CALLBACK_TYPE = 0i32;
pub const ModuleListStream: MINIDUMP_STREAM_TYPE = 4i32;
pub const ModuleReferencedByMemory: MODULE_WRITE_FLAGS = 16i32;
pub const ModuleWriteCodeSegs: MODULE_WRITE_FLAGS = 64i32;
pub const ModuleWriteCvRecord: MODULE_WRITE_FLAGS = 8i32;
pub const ModuleWriteDataSeg: MODULE_WRITE_FLAGS = 2i32;
pub const ModuleWriteMiscRecord: MODULE_WRITE_FLAGS = 4i32;
pub const ModuleWriteModule: MODULE_WRITE_FLAGS = 1i32;
pub const ModuleWriteTlsData: MODULE_WRITE_FLAGS = 32i32;
pub const NDIS_INTERNAL_ERROR: BUGCHECK_ERROR = 79u32;
pub const NDIS_NET_BUFFER_LIST_INFO_ILLEGALLY_TRANSFERRED: BUGCHECK_ERROR = 326u32;
pub const NETIO_INVALID_POOL_CALLER: BUGCHECK_ERROR = 294u32;
pub const NETWORK_BOOT_DUPLICATE_ADDRESS: BUGCHECK_ERROR = 188u32;
pub const NETWORK_BOOT_INITIALIZATION_FAILED: BUGCHECK_ERROR = 187u32;
pub const NMI_HARDWARE_FAILURE: BUGCHECK_ERROR = 128u32;
pub const NMR_INVALID_STATE: BUGCHECK_ERROR = 293u32;
pub const NO_BOOT_DEVICE: BUGCHECK_ERROR = 83u32;
pub const NO_EXCEPTION_HANDLING_SUPPORT: BUGCHECK_ERROR = 11u32;
pub const NO_MORE_IRP_STACK_LOCATIONS: BUGCHECK_ERROR = 53u32;
pub const NO_MORE_SYSTEM_PTES: BUGCHECK_ERROR = 63u32;
pub const NO_PAGES_AVAILABLE: BUGCHECK_ERROR = 77u32;
pub const NO_SPIN_LOCK_AVAILABLE: BUGCHECK_ERROR = 29u32;
pub const NO_SUCH_PARTITION: BUGCHECK_ERROR = 67u32;
pub const NO_USER_MODE_CONTEXT: BUGCHECK_ERROR = 14u32;
pub const NPFS_FILE_SYSTEM: BUGCHECK_ERROR = 37u32;
pub const NTFS_FILE_SYSTEM: BUGCHECK_ERROR = 36u32;
pub const NTHV_GUEST_ERROR: BUGCHECK_ERROR = 280u32;
pub const NUM_SSRVOPTS: u32 = 32u32;
pub const NumSymTypes: SYM_TYPE = 9i32;
pub const OBJECT1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 98u32;
pub const OBJECT_ATTRIB_ACCESS_FINAL: OBJECT_ATTRIB_FLAGS = 32768i32;
pub const OBJECT_ATTRIB_ACCESS_PRIVATE: OBJECT_ATTRIB_FLAGS = 8192i32;
pub const OBJECT_ATTRIB_ACCESS_PROTECTED: OBJECT_ATTRIB_FLAGS = 16384i32;
pub const OBJECT_ATTRIB_ACCESS_PUBLIC: OBJECT_ATTRIB_FLAGS = 4096i32;
pub const OBJECT_ATTRIB_HAS_EXTENDED_ATTRIBS: OBJECT_ATTRIB_FLAGS = 8388608i32;
pub const OBJECT_ATTRIB_IS_CLASS: OBJECT_ATTRIB_FLAGS = 16777216i32;
pub const OBJECT_ATTRIB_IS_FUNCTION: OBJECT_ATTRIB_FLAGS = 33554432i32;
pub const OBJECT_ATTRIB_IS_INHERITED: OBJECT_ATTRIB_FLAGS = 1073741824i32;
pub const OBJECT_ATTRIB_IS_INTERFACE: OBJECT_ATTRIB_FLAGS = -2147483648i32;
pub const OBJECT_ATTRIB_IS_MACRO: OBJECT_ATTRIB_FLAGS = 268435456i32;
pub const OBJECT_ATTRIB_IS_PROPERTY: OBJECT_ATTRIB_FLAGS = 134217728i32;
pub const OBJECT_ATTRIB_IS_TYPE: OBJECT_ATTRIB_FLAGS = 536870912i32;
pub const OBJECT_ATTRIB_IS_VARIABLE: OBJECT_ATTRIB_FLAGS = 67108864i32;
pub const OBJECT_ATTRIB_NO_ATTRIB: OBJECT_ATTRIB_FLAGS = 0i32;
pub const OBJECT_ATTRIB_NO_NAME: OBJECT_ATTRIB_FLAGS = 1i32;
pub const OBJECT_ATTRIB_NO_TYPE: OBJECT_ATTRIB_FLAGS = 2i32;
pub const OBJECT_ATTRIB_NO_VALUE: OBJECT_ATTRIB_FLAGS = 4i32;
pub const OBJECT_ATTRIB_OBJECT_IS_EXPANDABLE: OBJECT_ATTRIB_FLAGS = 112i32;
pub const OBJECT_ATTRIB_SLOT_IS_CATEGORY: OBJECT_ATTRIB_FLAGS = 1024i32;
pub const OBJECT_ATTRIB_STORAGE_FIELD: OBJECT_ATTRIB_FLAGS = 262144i32;
pub const OBJECT_ATTRIB_STORAGE_GLOBAL: OBJECT_ATTRIB_FLAGS = 65536i32;
pub const OBJECT_ATTRIB_STORAGE_STATIC: OBJECT_ATTRIB_FLAGS = 131072i32;
pub const OBJECT_ATTRIB_STORAGE_VIRTUAL: OBJECT_ATTRIB_FLAGS = 524288i32;
pub const OBJECT_ATTRIB_TYPE_HAS_CODE: OBJECT_ATTRIB_FLAGS = 512i32;
pub const OBJECT_ATTRIB_TYPE_IS_CONSTANT: OBJECT_ATTRIB_FLAGS = 1048576i32;
pub const OBJECT_ATTRIB_TYPE_IS_EXPANDABLE: OBJECT_ATTRIB_FLAGS = 256i32;
pub const OBJECT_ATTRIB_TYPE_IS_OBJECT: OBJECT_ATTRIB_FLAGS = 256i32;
pub const OBJECT_ATTRIB_TYPE_IS_SYNCHRONIZED: OBJECT_ATTRIB_FLAGS = 2097152i32;
pub const OBJECT_ATTRIB_TYPE_IS_VOLATILE: OBJECT_ATTRIB_FLAGS = 4194304i32;
pub const OBJECT_ATTRIB_VALUE_HAS_CODE: OBJECT_ATTRIB_FLAGS = 128i32;
pub const OBJECT_ATTRIB_VALUE_IS_CUSTOM: OBJECT_ATTRIB_FLAGS = 64i32;
pub const OBJECT_ATTRIB_VALUE_IS_ENUM: OBJECT_ATTRIB_FLAGS = 32i32;
pub const OBJECT_ATTRIB_VALUE_IS_INVALID: OBJECT_ATTRIB_FLAGS = 8i32;
pub const OBJECT_ATTRIB_VALUE_IS_OBJECT: OBJECT_ATTRIB_FLAGS = 16i32;
pub const OBJECT_ATTRIB_VALUE_READONLY: OBJECT_ATTRIB_FLAGS = 2048i32;
pub const OBJECT_INITIALIZATION_FAILED: BUGCHECK_ERROR = 94u32;
pub const OFS_FILE_SYSTEM: BUGCHECK_ERROR = 131u32;
pub const OS_DATA_TAMPERING: BUGCHECK_ERROR = 341u32;
pub const OUTPUT_DEBUG_STRING_EVENT: DEBUG_EVENT_CODE = 8u32;
pub const PAGE_FAULT_BEYOND_END_OF_ALLOCATION: BUGCHECK_ERROR = 205u32;
pub const PAGE_FAULT_IN_FREED_SPECIAL_POOL: BUGCHECK_ERROR = 204u32;
pub const PAGE_FAULT_IN_NONPAGED_AREA: BUGCHECK_ERROR = 80u32;
pub const PAGE_FAULT_IN_NONPAGED_AREA_M: BUGCHECK_ERROR = 268435536u32;
pub const PAGE_FAULT_WITH_INTERRUPTS_OFF: BUGCHECK_ERROR = 73u32;
pub const PAGE_NOT_ZERO: BUGCHECK_ERROR = 295u32;
pub const PANIC_STACK_SWITCH: BUGCHECK_ERROR = 43u32;
pub const PASSIVE_INTERRUPT_ERROR: BUGCHECK_ERROR = 315u32;
pub const PCI_BUS_DRIVER_INTERNAL: BUGCHECK_ERROR = 161u32;
pub const PCI_CONFIG_SPACE_ACCESS_FAILURE: BUGCHECK_ERROR = 192u32;
pub const PCI_VERIFIER_DETECTED_VIOLATION: BUGCHECK_ERROR = 246u32;
pub const PDC_LOCK_WATCHDOG_LIVEDUMP: BUGCHECK_ERROR = 380u32;
pub const PDC_PRIVILEGE_CHECK_LIVEDUMP: BUGCHECK_ERROR = 415u32;
pub const PDC_UNEXPECTED_REVOCATION_LIVEDUMP: BUGCHECK_ERROR = 381u32;
pub const PDC_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 335u32;
pub const PDC_WATCHDOG_TIMEOUT_LIVEDUMP: BUGCHECK_ERROR = 348u32;
pub const PFN_LIST_CORRUPT: BUGCHECK_ERROR = 78u32;
pub const PFN_REFERENCE_COUNT: BUGCHECK_ERROR = 28u32;
pub const PFN_SHARE_COUNT: BUGCHECK_ERROR = 27u32;
pub const PF_DETECTED_CORRUPTION: BUGCHECK_ERROR = 401u32;
pub const PHASE0_EXCEPTION: BUGCHECK_ERROR = 120u32;
pub const PHASE0_INITIALIZATION_FAILED: BUGCHECK_ERROR = 49u32;
pub const PHASE1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 50u32;
pub const PINBALL_FILE_SYSTEM: BUGCHECK_ERROR = 89u32;
pub const PNP_DETECTED_FATAL_ERROR: BUGCHECK_ERROR = 202u32;
pub const PNP_INTERNAL_ERROR: BUGCHECK_ERROR = 149u32;
pub const POOL_CORRUPTION_IN_FILE_AREA: BUGCHECK_ERROR = 222u32;
pub const PORT_DRIVER_INTERNAL: BUGCHECK_ERROR = 44u32;
pub const POWER_FAILURE_SIMULATE: BUGCHECK_ERROR = 229u32;
pub const PP0_INITIALIZATION_FAILED: BUGCHECK_ERROR = 143u32;
pub const PP1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 144u32;
pub const PREVIOUS_FATAL_ABNORMAL_RESET_ERROR: BUGCHECK_ERROR = 373u32;
pub const PROCESS1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 107u32;
pub const PROCESSOR_DRIVER_INTERNAL: BUGCHECK_ERROR = 323u32;
pub const PROCESSOR_START_TIMEOUT: BUGCHECK_ERROR = 479u32;
pub const PROCESS_HAS_LOCKED_PAGES: BUGCHECK_ERROR = 118u32;
pub const PROCESS_INITIALIZATION_FAILED: BUGCHECK_ERROR = 96u32;
pub const PROFILER_CONFIGURATION_ILLEGAL: BUGCHECK_ERROR = 379u32;
pub const PROP_INFO_ATTRIBUTES: PROP_INFO_FLAGS = 8i32;
pub const PROP_INFO_AUTOEXPAND: PROP_INFO_FLAGS = 134217728i32;
pub const PROP_INFO_DEBUGPROP: PROP_INFO_FLAGS = 16i32;
pub const PROP_INFO_FULLNAME: PROP_INFO_FLAGS = 32i32;
pub const PROP_INFO_NAME: PROP_INFO_FLAGS = 1i32;
pub const PROP_INFO_TYPE: PROP_INFO_FLAGS = 2i32;
pub const PROP_INFO_VALUE: PROP_INFO_FLAGS = 4i32;
pub const ProcessVmCountersStream: MINIDUMP_STREAM_TYPE = 22i32;
pub const QUOTA_UNDERFLOW: BUGCHECK_ERROR = 33u32;
pub const RAMDISK_BOOT_INITIALIZATION_FAILED: BUGCHECK_ERROR = 248u32;
pub const RDR_FILE_SYSTEM: BUGCHECK_ERROR = 39u32;
pub const RECOM_DRIVER: BUGCHECK_ERROR = 132u32;
pub const RECURSIVE_MACHINE_CHECK: BUGCHECK_ERROR = 251u32;
pub const RECURSIVE_NMI: BUGCHECK_ERROR = 273u32;
pub const REFERENCE_BY_POINTER: BUGCHECK_ERROR = 24u32;
pub const REFMON_INITIALIZATION_FAILED: BUGCHECK_ERROR = 108u32;
pub const REFS_FILE_SYSTEM: BUGCHECK_ERROR = 329u32;
pub const REF_UNKNOWN_LOGON_SESSION: BUGCHECK_ERROR = 71u32;
pub const REGISTRY_CALLBACK_DRIVER_EXCEPTION: BUGCHECK_ERROR = 319u32;
pub const REGISTRY_ERROR: BUGCHECK_ERROR = 81u32;
pub const REGISTRY_FILTER_DRIVER_EXCEPTION: BUGCHECK_ERROR = 309u32;
pub const REGISTRY_LIVE_DUMP: BUGCHECK_ERROR = 487u32;
pub const RESERVE_QUEUE_OVERFLOW: BUGCHECK_ERROR = 255u32;
pub const RESOURCE_NOT_OWNED: BUGCHECK_ERROR = 227u32;
pub const RESOURCE_OWNER_POINTER_INVALID: BUGCHECK_ERROR = 306u32;
pub const RESTORE_LAST_ERROR_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RestoreLastError");
pub const RESTORE_LAST_ERROR_NAME_A: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("RestoreLastError");
pub const RESTORE_LAST_ERROR_NAME_W: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RestoreLastError");
pub const RIP_EVENT: DEBUG_EVENT_CODE = 9u32;
pub const ReadMemoryFailureCallback: MINIDUMP_CALLBACK_TYPE = 14i32;
pub const RemoveMemoryCallback: MINIDUMP_CALLBACK_TYPE = 9i32;
pub const ReservedStream0: MINIDUMP_STREAM_TYPE = 1i32;
pub const ReservedStream1: MINIDUMP_STREAM_TYPE = 2i32;
pub const SAVER_ACCOUNTPROVSVCINITFAILURE: BUGCHECK_ERROR = 61461u32;
pub const SAVER_APPBARDISMISSAL: BUGCHECK_ERROR = 61454u32;
pub const SAVER_APPLISTUNREACHABLE: BUGCHECK_ERROR = 61456u32;
pub const SAVER_AUDIODRIVERHANG: BUGCHECK_ERROR = 62464u32;
pub const SAVER_AUXILIARYFULLDUMP: BUGCHECK_ERROR = 61460u32;
pub const SAVER_BATTERYPULLOUT: BUGCHECK_ERROR = 62720u32;
pub const SAVER_BLANKSCREEN: BUGCHECK_ERROR = 61442u32;
pub const SAVER_CALLDISMISSAL: BUGCHECK_ERROR = 61453u32;
pub const SAVER_CAPTURESERVICE: BUGCHECK_ERROR = 63488u32;
pub const SAVER_CHROMEPROCESSCRASH: BUGCHECK_ERROR = 61450u32;
pub const SAVER_DEVICEUPDATEUNSPECIFIED: BUGCHECK_ERROR = 62208u32;
pub const SAVER_GRAPHICS: BUGCHECK_ERROR = 61448u32;
pub const SAVER_INPUT: BUGCHECK_ERROR = 61443u32;
pub const SAVER_MEDIACORETESTHANG: BUGCHECK_ERROR = 62976u32;
pub const SAVER_MTBFCOMMANDHANG: BUGCHECK_ERROR = 61697u32;
pub const SAVER_MTBFCOMMANDTIMEOUT: BUGCHECK_ERROR = 789u32;
pub const SAVER_MTBFIOERROR: BUGCHECK_ERROR = 61699u32;
pub const SAVER_MTBFPASSBUGCHECK: BUGCHECK_ERROR = 61698u32;
pub const SAVER_NAVIGATIONMODEL: BUGCHECK_ERROR = 61446u32;
pub const SAVER_NAVSERVERTIMEOUT: BUGCHECK_ERROR = 61449u32;
pub const SAVER_NONRESPONSIVEPROCESS: BUGCHECK_ERROR = 404u32;
pub const SAVER_NOTIFICATIONDISMISSAL: BUGCHECK_ERROR = 61451u32;
pub const SAVER_OUTOFMEMORY: BUGCHECK_ERROR = 61447u32;
pub const SAVER_RENDERMOBILEUIOOM: BUGCHECK_ERROR = 61953u32;
pub const SAVER_RENDERTHREADHANG: BUGCHECK_ERROR = 61952u32;
pub const SAVER_REPORTNOTIFICATIONFAILURE: BUGCHECK_ERROR = 61457u32;
pub const SAVER_RESOURCEMANAGEMENT: BUGCHECK_ERROR = 63232u32;
pub const SAVER_RILADAPTATIONCRASH: BUGCHECK_ERROR = 61455u32;
pub const SAVER_RPCFAILURE: BUGCHECK_ERROR = 61459u32;
pub const SAVER_SICKAPPLICATION: BUGCHECK_ERROR = 34918u32;
pub const SAVER_SPEECHDISMISSAL: BUGCHECK_ERROR = 61452u32;
pub const SAVER_STARTNOTVISIBLE: BUGCHECK_ERROR = 61445u32;
pub const SAVER_UNEXPECTEDSHUTDOWN: BUGCHECK_ERROR = 61458u32;
pub const SAVER_UNSPECIFIED: BUGCHECK_ERROR = 61440u32;
pub const SAVER_WAITFORSHELLREADY: BUGCHECK_ERROR = 63744u32;
pub const SAVER_WATCHDOG: BUGCHECK_ERROR = 61444u32;
pub const SCSI_DISK_DRIVER_INTERNAL: BUGCHECK_ERROR = 45u32;
pub const SCSI_VERIFIER_DETECTED_VIOLATION: BUGCHECK_ERROR = 241u32;
pub const SDBUS_INTERNAL_ERROR: BUGCHECK_ERROR = 346u32;
pub const SECURE_BOOT_VIOLATION: BUGCHECK_ERROR = 325u32;
pub const SECURE_FAULT_UNHANDLED: BUGCHECK_ERROR = 397u32;
pub const SECURE_KERNEL_ERROR: BUGCHECK_ERROR = 395u32;
pub const SECURE_PCI_CONFIG_SPACE_ACCESS_VIOLATION: BUGCHECK_ERROR = 490u32;
pub const SECURITY1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 99u32;
pub const SECURITY_INITIALIZATION_FAILED: BUGCHECK_ERROR = 95u32;
pub const SECURITY_SYSTEM: BUGCHECK_ERROR = 41u32;
pub const SEM_ALL_ERRORS: THREAD_ERROR_MODE = 0u32;
pub const SEM_FAILCRITICALERRORS: THREAD_ERROR_MODE = 1u32;
pub const SEM_NOALIGNMENTFAULTEXCEPT: THREAD_ERROR_MODE = 4u32;
pub const SEM_NOGPFAULTERRORBOX: THREAD_ERROR_MODE = 2u32;
pub const SEM_NOOPENFILEERRORBOX: THREAD_ERROR_MODE = 32768u32;
pub const SERIAL_DRIVER_INTERNAL: BUGCHECK_ERROR = 56u32;
pub const SESSION1_INITIALIZATION_FAILED: BUGCHECK_ERROR = 109u32;
pub const SESSION_HAS_VALID_POOL_ON_EXIT: BUGCHECK_ERROR = 171u32;
pub const SESSION_HAS_VALID_SPECIAL_POOL_ON_EXIT: BUGCHECK_ERROR = 236u32;
pub const SESSION_HAS_VALID_VIEWS_ON_EXIT: BUGCHECK_ERROR = 186u32;
pub const SETUP_FAILURE: BUGCHECK_ERROR = 133u32;
pub const SET_ENV_VAR_FAILED: BUGCHECK_ERROR = 91u32;
pub const SET_OF_INVALID_CONTEXT: BUGCHECK_ERROR = 48u32;
pub const SHARED_RESOURCE_CONV_ERROR: BUGCHECK_ERROR = 31u32;
pub const SILO_CORRUPT: BUGCHECK_ERROR = 394u32;
pub const SLE_ERROR: RIP_INFO_TYPE = 1u32;
pub const SLE_MINORERROR: RIP_INFO_TYPE = 2u32;
pub const SLE_WARNING: RIP_INFO_TYPE = 3u32;
pub const SLMFLAG_ALT_INDEX: SYM_LOAD_FLAGS = 2u32;
pub const SLMFLAG_NONE: SYM_LOAD_FLAGS = 0u32;
pub const SLMFLAG_NO_SYMBOLS: SYM_LOAD_FLAGS = 4u32;
pub const SLMFLAG_VIRTUAL: SYM_LOAD_FLAGS = 1u32;
pub const SMB_REDIRECTOR_LIVEDUMP: BUGCHECK_ERROR = 423u32;
pub const SMB_SERVER_LIVEDUMP: BUGCHECK_ERROR = 405u32;
pub const SOC_CRITICAL_DEVICE_REMOVED: BUGCHECK_ERROR = 334u32;
pub const SOC_SUBSYSTEM_FAILURE: BUGCHECK_ERROR = 331u32;
pub const SOC_SUBSYSTEM_FAILURE_LIVEDUMP: BUGCHECK_ERROR = 349u32;
pub const SOFT_RESTART_FATAL_ERROR: BUGCHECK_ERROR = 112u32;
pub const SPECIAL_POOL_DETECTED_MEMORY_CORRUPTION: BUGCHECK_ERROR = 193u32;
pub const SPIN_LOCK_ALREADY_OWNED: BUGCHECK_ERROR = 15u32;
pub const SPIN_LOCK_INIT_FAILURE: BUGCHECK_ERROR = 129u32;
pub const SPIN_LOCK_NOT_OWNED: BUGCHECK_ERROR = 16u32;
pub const SPLITSYM_EXTRACT_ALL: u32 = 2u32;
pub const SPLITSYM_REMOVE_PRIVATE: u32 = 1u32;
pub const SPLITSYM_SYMBOLPATH_IS_SRC: u32 = 4u32;
pub const SSRVACTION_CHECKSUMSTATUS: u32 = 8u32;
pub const SSRVACTION_EVENT: u32 = 3u32;
pub const SSRVACTION_EVENTW: u32 = 4u32;
pub const SSRVACTION_HTTPSTATUS: u32 = 6u32;
pub const SSRVACTION_QUERYCANCEL: u32 = 2u32;
pub const SSRVACTION_SIZE: u32 = 5u32;
pub const SSRVACTION_TRACE: u32 = 1u32;
pub const SSRVACTION_XMLOUTPUT: u32 = 7u32;
pub const SSRVOPT_CALLBACK: u32 = 1u32;
pub const SSRVOPT_CALLBACKW: u32 = 65536u32;
pub const SSRVOPT_DISABLE_PING_HOST: u32 = 67108864u32;
pub const SSRVOPT_DISABLE_TIMEOUT: u32 = 134217728u32;
pub const SSRVOPT_DONT_UNCOMPRESS: u32 = 33554432u32;
pub const SSRVOPT_DOWNSTREAM_STORE: u32 = 8192u32;
pub const SSRVOPT_DWORD: SYM_FIND_ID_OPTION = 2u32;
pub const SSRVOPT_DWORDPTR: SYM_FIND_ID_OPTION = 4u32;
pub const SSRVOPT_ENABLE_COMM_MSG: u32 = 268435456u32;
pub const SSRVOPT_FAVOR_COMPRESSED: u32 = 2097152u32;
pub const SSRVOPT_FLAT_DEFAULT_STORE: u32 = 131072u32;
pub const SSRVOPT_GETPATH: u32 = 64u32;
pub const SSRVOPT_GUIDPTR: SYM_FIND_ID_OPTION = 8u32;
pub const SSRVOPT_MAX: u32 = 2147483648u32;
pub const SSRVOPT_MESSAGE: u32 = 524288u32;
pub const SSRVOPT_NOCOPY: u32 = 64u32;
pub const SSRVOPT_OLDGUIDPTR: u32 = 16u32;
pub const SSRVOPT_OVERWRITE: u32 = 16384u32;
pub const SSRVOPT_PARAMTYPE: u32 = 256u32;
pub const SSRVOPT_PARENTWIN: u32 = 128u32;
pub const SSRVOPT_PROXY: u32 = 4096u32;
pub const SSRVOPT_PROXYW: u32 = 262144u32;
pub const SSRVOPT_RESETTOU: u32 = 32768u32;
pub const SSRVOPT_RETRY_APP_HANG: u32 = 2147483648u32;
pub const SSRVOPT_SECURE: u32 = 512u32;
pub const SSRVOPT_SERVICE: u32 = 1048576u32;
pub const SSRVOPT_SETCONTEXT: u32 = 2048u32;
pub const SSRVOPT_STRING: u32 = 4194304u32;
pub const SSRVOPT_TRACE: u32 = 1024u32;
pub const SSRVOPT_UNATTENDED: u32 = 32u32;
pub const SSRVOPT_URI_FILTER: u32 = 536870912u32;
pub const SSRVOPT_URI_TIERS: u32 = 1073741824u32;
pub const SSRVOPT_WINHTTP: u32 = 8388608u32;
pub const SSRVOPT_WININET: u32 = 16777216u32;
pub const SSRVURI_ALL: u32 = 255u32;
pub const SSRVURI_COMPRESSED: u32 = 2u32;
pub const SSRVURI_FILEPTR: u32 = 4u32;
pub const SSRVURI_HTTP_COMPRESSED: u32 = 2u32;
pub const SSRVURI_HTTP_FILEPTR: u32 = 4u32;
pub const SSRVURI_HTTP_MASK: u32 = 15u32;
pub const SSRVURI_HTTP_NORMAL: u32 = 1u32;
pub const SSRVURI_NORMAL: u32 = 1u32;
pub const SSRVURI_UNC_COMPRESSED: u32 = 32u32;
pub const SSRVURI_UNC_FILEPTR: u32 = 64u32;
pub const SSRVURI_UNC_MASK: u32 = 240u32;
pub const SSRVURI_UNC_NORMAL: u32 = 16u32;
pub const STORAGE_DEVICE_ABNORMALITY_DETECTED: BUGCHECK_ERROR = 320u32;
pub const STORAGE_MINIPORT_ERROR: BUGCHECK_ERROR = 240u32;
pub const STORE_DATA_STRUCTURE_CORRUPTION: BUGCHECK_ERROR = 455u32;
pub const STREAMS_INTERNAL_ERROR: BUGCHECK_ERROR = 75u32;
pub const SYMBOLIC_INITIALIZATION_FAILED: BUGCHECK_ERROR = 100u32;
pub const SYMENUM_OPTIONS_DEFAULT: u32 = 1u32;
pub const SYMENUM_OPTIONS_INLINE: u32 = 2u32;
pub const SYMFLAG_CLR_TOKEN: SYMBOL_INFO_FLAGS = 262144u32;
pub const SYMFLAG_CONSTANT: SYMBOL_INFO_FLAGS = 256u32;
pub const SYMFLAG_EXPORT: SYMBOL_INFO_FLAGS = 512u32;
pub const SYMFLAG_FIXUP_ARM64X: u32 = 16777216u32;
pub const SYMFLAG_FORWARDER: SYMBOL_INFO_FLAGS = 1024u32;
pub const SYMFLAG_FRAMEREL: SYMBOL_INFO_FLAGS = 32u32;
pub const SYMFLAG_FUNCTION: SYMBOL_INFO_FLAGS = 2048u32;
pub const SYMFLAG_FUNC_NO_RETURN: u32 = 1048576u32;
pub const SYMFLAG_GLOBAL: u32 = 33554432u32;
pub const SYMFLAG_ILREL: SYMBOL_INFO_FLAGS = 65536u32;
pub const SYMFLAG_LOCAL: SYMBOL_INFO_FLAGS = 128u32;
pub const SYMFLAG_METADATA: SYMBOL_INFO_FLAGS = 131072u32;
pub const SYMFLAG_NULL: u32 = 524288u32;
pub const SYMFLAG_PARAMETER: SYMBOL_INFO_FLAGS = 64u32;
pub const SYMFLAG_PUBLIC_CODE: u32 = 4194304u32;
pub const SYMFLAG_REGISTER: SYMBOL_INFO_FLAGS = 8u32;
pub const SYMFLAG_REGREL: SYMBOL_INFO_FLAGS = 16u32;
pub const SYMFLAG_REGREL_ALIASINDIR: u32 = 8388608u32;
pub const SYMFLAG_RESET: u32 = 2147483648u32;
pub const SYMFLAG_SLOT: SYMBOL_INFO_FLAGS = 32768u32;
pub const SYMFLAG_SYNTHETIC_ZEROBASE: u32 = 2097152u32;
pub const SYMFLAG_THUNK: SYMBOL_INFO_FLAGS = 8192u32;
pub const SYMFLAG_TLSREL: SYMBOL_INFO_FLAGS = 16384u32;
pub const SYMFLAG_VALUEPRESENT: SYMBOL_INFO_FLAGS = 1u32;
pub const SYMFLAG_VIRTUAL: SYMBOL_INFO_FLAGS = 4096u32;
pub const SYMF_CONSTANT: u32 = 256u32;
pub const SYMF_EXPORT: u32 = 512u32;
pub const SYMF_FORWARDER: u32 = 1024u32;
pub const SYMF_FRAMEREL: u32 = 32u32;
pub const SYMF_FUNCTION: u32 = 2048u32;
pub const SYMF_LOCAL: u32 = 128u32;
pub const SYMF_OMAP_GENERATED: u32 = 1u32;
pub const SYMF_OMAP_MODIFIED: u32 = 2u32;
pub const SYMF_PARAMETER: u32 = 64u32;
pub const SYMF_REGISTER: u32 = 8u32;
pub const SYMF_REGREL: u32 = 16u32;
pub const SYMF_THUNK: u32 = 8192u32;
pub const SYMF_TLSREL: u32 = 16384u32;
pub const SYMF_VIRTUAL: u32 = 4096u32;
pub const SYMOPT_ALLOW_ABSOLUTE_SYMBOLS: u32 = 2048u32;
pub const SYMOPT_ALLOW_ZERO_ADDRESS: u32 = 16777216u32;
pub const SYMOPT_AUTO_PUBLICS: u32 = 65536u32;
pub const SYMOPT_CASE_INSENSITIVE: u32 = 1u32;
pub const SYMOPT_DEBUG: u32 = 2147483648u32;
pub const SYMOPT_DEFERRED_LOADS: u32 = 4u32;
pub const SYMOPT_DISABLE_FAST_SYMBOLS: u32 = 268435456u32;
pub const SYMOPT_DISABLE_SRVSTAR_ON_STARTUP: u32 = 1073741824u32;
pub const SYMOPT_DISABLE_SYMSRV_AUTODETECT: u32 = 33554432u32;
pub const SYMOPT_DISABLE_SYMSRV_TIMEOUT: u32 = 536870912u32;
pub const SYMOPT_EXACT_SYMBOLS: u32 = 1024u32;
pub const SYMOPT_EX_DISABLEACCESSTIMEUPDATE: IMAGEHLP_EXTENDED_OPTIONS = 0i32;
pub const SYMOPT_EX_LASTVALIDDEBUGDIRECTORY: IMAGEHLP_EXTENDED_OPTIONS = 1i32;
pub const SYMOPT_EX_MAX: IMAGEHLP_EXTENDED_OPTIONS = 4i32;
pub const SYMOPT_EX_NEVERLOADSYMBOLS: IMAGEHLP_EXTENDED_OPTIONS = 3i32;
pub const SYMOPT_EX_NOIMPLICITPATTERNSEARCH: IMAGEHLP_EXTENDED_OPTIONS = 2i32;
pub const SYMOPT_FAIL_CRITICAL_ERRORS: u32 = 512u32;
pub const SYMOPT_FAVOR_COMPRESSED: u32 = 8388608u32;
pub const SYMOPT_FLAT_DIRECTORY: u32 = 4194304u32;
pub const SYMOPT_IGNORE_CVREC: u32 = 128u32;
pub const SYMOPT_IGNORE_IMAGEDIR: u32 = 2097152u32;
pub const SYMOPT_IGNORE_NT_SYMPATH: u32 = 4096u32;
pub const SYMOPT_INCLUDE_32BIT_MODULES: u32 = 8192u32;
pub const SYMOPT_LOAD_ANYTHING: u32 = 64u32;
pub const SYMOPT_LOAD_LINES: u32 = 16u32;
pub const SYMOPT_NO_CPP: u32 = 8u32;
pub const SYMOPT_NO_IMAGE_SEARCH: u32 = 131072u32;
pub const SYMOPT_NO_PROMPTS: u32 = 524288u32;
pub const SYMOPT_NO_PUBLICS: u32 = 32768u32;
pub const SYMOPT_NO_UNQUALIFIED_LOADS: u32 = 256u32;
pub const SYMOPT_OMAP_FIND_NEAREST: u32 = 32u32;
pub const SYMOPT_OVERWRITE: u32 = 1048576u32;
pub const SYMOPT_PUBLICS_ONLY: u32 = 16384u32;
pub const SYMOPT_READONLY_CACHE: u32 = 67108864u32;
pub const SYMOPT_SECURE: u32 = 262144u32;
pub const SYMOPT_SYMPATH_LAST: u32 = 134217728u32;
pub const SYMOPT_UNDNAME: u32 = 2u32;
pub const SYMSEARCH_ALLITEMS: u32 = 8u32;
pub const SYMSEARCH_GLOBALSONLY: u32 = 4u32;
pub const SYMSEARCH_MASKOBJS: u32 = 1u32;
pub const SYMSEARCH_RECURSE: u32 = 2u32;
pub const SYMSRV_VERSION: u32 = 2u32;
pub const SYMSTOREOPT_ALT_INDEX: u32 = 16u32;
pub const SYMSTOREOPT_COMPRESS: SYM_SRV_STORE_FILE_FLAGS = 1u32;
pub const SYMSTOREOPT_OVERWRITE: SYM_SRV_STORE_FILE_FLAGS = 2u32;
pub const SYMSTOREOPT_PASS_IF_EXISTS: SYM_SRV_STORE_FILE_FLAGS = 64u32;
pub const SYMSTOREOPT_POINTER: SYM_SRV_STORE_FILE_FLAGS = 8u32;
pub const SYMSTOREOPT_RETURNINDEX: SYM_SRV_STORE_FILE_FLAGS = 4u32;
pub const SYMSTOREOPT_UNICODE: u32 = 32u32;
pub const SYM_INLINE_COMP_DIFFERENT: u32 = 5u32;
pub const SYM_INLINE_COMP_ERROR: u32 = 0u32;
pub const SYM_INLINE_COMP_IDENTICAL: u32 = 1u32;
pub const SYM_INLINE_COMP_STEPIN: u32 = 2u32;
pub const SYM_INLINE_COMP_STEPOUT: u32 = 3u32;
pub const SYM_INLINE_COMP_STEPOVER: u32 = 4u32;
pub const SYM_STKWALK_DEFAULT: u32 = 0u32;
pub const SYM_STKWALK_FORCE_FRAMEPTR: u32 = 1u32;
pub const SYM_STKWALK_ZEROEXTEND_PTRS: u32 = 2u32;
pub const SYNTHETIC_EXCEPTION_UNHANDLED: BUGCHECK_ERROR = 399u32;
pub const SYNTHETIC_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 458u32;
pub const SYSTEM_EXIT_OWNED_MUTEX: BUGCHECK_ERROR = 57u32;
pub const SYSTEM_IMAGE_BAD_SIGNATURE: BUGCHECK_ERROR = 195u32;
pub const SYSTEM_LICENSE_VIOLATION: BUGCHECK_ERROR = 154u32;
pub const SYSTEM_PTE_MISUSE: BUGCHECK_ERROR = 218u32;
pub const SYSTEM_SCAN_AT_RAISED_IRQL_CAUGHT_IMPROPER_DRIVER_UNLOAD: BUGCHECK_ERROR = 212u32;
pub const SYSTEM_SERVICE_EXCEPTION: BUGCHECK_ERROR = 59u32;
pub const SYSTEM_THREAD_EXCEPTION_NOT_HANDLED: BUGCHECK_ERROR = 126u32;
pub const SYSTEM_THREAD_EXCEPTION_NOT_HANDLED_M: BUGCHECK_ERROR = 268435582u32;
pub const SYSTEM_UNWIND_PREVIOUS_USER: BUGCHECK_ERROR = 58u32;
pub const SecondaryFlagsCallback: MINIDUMP_CALLBACK_TYPE = 15i32;
pub const SymCoff: SYM_TYPE = 1i32;
pub const SymCv: SYM_TYPE = 2i32;
pub const SymDeferred: SYM_TYPE = 5i32;
pub const SymDia: SYM_TYPE = 7i32;
pub const SymExport: SYM_TYPE = 4i32;
pub const SymNone: SYM_TYPE = 0i32;
pub const SymPdb: SYM_TYPE = 3i32;
pub const SymSym: SYM_TYPE = 6i32;
pub const SymVirtual: SYM_TYPE = 8i32;
pub const SystemInfoStream: MINIDUMP_STREAM_TYPE = 7i32;
pub const SystemMemoryInfoStream: MINIDUMP_STREAM_TYPE = 21i32;
pub const TARGET_ATTRIBUTE_PACMASK: u32 = 1u32;
pub const TARGET_MDL_TOO_SMALL: BUGCHECK_ERROR = 64u32;
pub const TCPIP_AOAC_NIC_ACTIVE_REFERENCE_LEAK: BUGCHECK_ERROR = 336u32;
pub const TELEMETRY_ASSERTS_LIVEDUMP: BUGCHECK_ERROR = 465u32;
pub const TERMINAL_SERVER_DRIVER_MADE_INCORRECT_MEMORY_REFERENCE: BUGCHECK_ERROR = 207u32;
pub const THIRD_PARTY_FILE_SYSTEM_FAILURE: BUGCHECK_ERROR = 264u32;
pub const THREAD_NOT_MUTEX_OWNER: BUGCHECK_ERROR = 17u32;
pub const THREAD_STUCK_IN_DEVICE_DRIVER: BUGCHECK_ERROR = 234u32;
pub const THREAD_STUCK_IN_DEVICE_DRIVER_M: BUGCHECK_ERROR = 268435690u32;
pub const THREAD_TERMINATE_HELD_MUTEX: BUGCHECK_ERROR = 1073741962u32;
pub const TIMER_OR_DPC_INVALID: BUGCHECK_ERROR = 199u32;
pub const TI_FINDCHILDREN: IMAGEHLP_SYMBOL_TYPE_INFO = 7i32;
pub const TI_GET_ADDRESS: IMAGEHLP_SYMBOL_TYPE_INFO = 22i32;
pub const TI_GET_ADDRESSOFFSET: IMAGEHLP_SYMBOL_TYPE_INFO = 9i32;
pub const TI_GET_ARRAYINDEXTYPEID: IMAGEHLP_SYMBOL_TYPE_INFO = 6i32;
pub const TI_GET_BASETYPE: IMAGEHLP_SYMBOL_TYPE_INFO = 5i32;
pub const TI_GET_BITPOSITION: IMAGEHLP_SYMBOL_TYPE_INFO = 14i32;
pub const TI_GET_CALLING_CONVENTION: IMAGEHLP_SYMBOL_TYPE_INFO = 26i32;
pub const TI_GET_CHILDRENCOUNT: IMAGEHLP_SYMBOL_TYPE_INFO = 13i32;
pub const TI_GET_CLASSPARENTID: IMAGEHLP_SYMBOL_TYPE_INFO = 18i32;
pub const TI_GET_COUNT: IMAGEHLP_SYMBOL_TYPE_INFO = 12i32;
pub const TI_GET_DATAKIND: IMAGEHLP_SYMBOL_TYPE_INFO = 8i32;
pub const TI_GET_INDIRECTVIRTUALBASECLASS: IMAGEHLP_SYMBOL_TYPE_INFO = 32i32;
pub const TI_GET_IS_REFERENCE: IMAGEHLP_SYMBOL_TYPE_INFO = 31i32;
pub const TI_GET_LENGTH: IMAGEHLP_SYMBOL_TYPE_INFO = 2i32;
pub const TI_GET_LEXICALPARENT: IMAGEHLP_SYMBOL_TYPE_INFO = 21i32;
pub const TI_GET_NESTED: IMAGEHLP_SYMBOL_TYPE_INFO = 19i32;
pub const TI_GET_OBJECTPOINTERTYPE: IMAGEHLP_SYMBOL_TYPE_INFO = 34i32;
pub const TI_GET_OFFSET: IMAGEHLP_SYMBOL_TYPE_INFO = 10i32;
pub const TI_GET_SYMINDEX: IMAGEHLP_SYMBOL_TYPE_INFO = 20i32;
pub const TI_GET_SYMNAME: IMAGEHLP_SYMBOL_TYPE_INFO = 1i32;
pub const TI_GET_SYMTAG: IMAGEHLP_SYMBOL_TYPE_INFO = 0i32;
pub const TI_GET_THISADJUST: IMAGEHLP_SYMBOL_TYPE_INFO = 23i32;
pub const TI_GET_TYPE: IMAGEHLP_SYMBOL_TYPE_INFO = 3i32;
pub const TI_GET_TYPEID: IMAGEHLP_SYMBOL_TYPE_INFO = 4i32;
pub const TI_GET_UDTKIND: IMAGEHLP_SYMBOL_TYPE_INFO = 24i32;
pub const TI_GET_VALUE: IMAGEHLP_SYMBOL_TYPE_INFO = 11i32;
pub const TI_GET_VIRTUALBASECLASS: IMAGEHLP_SYMBOL_TYPE_INFO = 15i32;
pub const TI_GET_VIRTUALBASEDISPINDEX: IMAGEHLP_SYMBOL_TYPE_INFO = 30i32;
pub const TI_GET_VIRTUALBASEOFFSET: IMAGEHLP_SYMBOL_TYPE_INFO = 29i32;
pub const TI_GET_VIRTUALBASEPOINTEROFFSET: IMAGEHLP_SYMBOL_TYPE_INFO = 17i32;
pub const TI_GET_VIRTUALBASETABLETYPE: IMAGEHLP_SYMBOL_TYPE_INFO = 33i32;
pub const TI_GET_VIRTUALTABLESHAPEID: IMAGEHLP_SYMBOL_TYPE_INFO = 16i32;
pub const TI_GTIEX_REQS_VALID: IMAGEHLP_SYMBOL_TYPE_INFO = 28i32;
pub const TI_IS_CLOSE_EQUIV_TO: IMAGEHLP_SYMBOL_TYPE_INFO = 27i32;
pub const TI_IS_EQUIV_TO: IMAGEHLP_SYMBOL_TYPE_INFO = 25i32;
pub const TOO_MANY_RECURSIVE_FAULTS: BUGCHECK_ERROR = 286u32;
pub const TRAP_CAUSE_UNKNOWN: BUGCHECK_ERROR = 18u32;
pub const TTM_FATAL_ERROR: BUGCHECK_ERROR = 411u32;
pub const TTM_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 416u32;
pub const ThreadCallback: MINIDUMP_CALLBACK_TYPE = 1i32;
pub const ThreadExCallback: MINIDUMP_CALLBACK_TYPE = 2i32;
pub const ThreadExListStream: MINIDUMP_STREAM_TYPE = 8i32;
pub const ThreadInfoListStream: MINIDUMP_STREAM_TYPE = 17i32;
pub const ThreadListStream: MINIDUMP_STREAM_TYPE = 3i32;
pub const ThreadNamesStream: MINIDUMP_STREAM_TYPE = 24i32;
pub const ThreadWriteBackingStore: THREAD_WRITE_FLAGS = 8i32;
pub const ThreadWriteContext: THREAD_WRITE_FLAGS = 4i32;
pub const ThreadWriteInstructionWindow: THREAD_WRITE_FLAGS = 16i32;
pub const ThreadWriteStack: THREAD_WRITE_FLAGS = 2i32;
pub const ThreadWriteThread: THREAD_WRITE_FLAGS = 1i32;
pub const ThreadWriteThreadData: THREAD_WRITE_FLAGS = 32i32;
pub const ThreadWriteThreadInfo: THREAD_WRITE_FLAGS = 64i32;
pub const TokenStream: MINIDUMP_STREAM_TYPE = 19i32;
pub const UCMUCSI_FAILURE: BUGCHECK_ERROR = 472u32;
pub const UCMUCSI_LIVEDUMP: BUGCHECK_ERROR = 468u32;
pub const UDFS_FILE_SYSTEM: BUGCHECK_ERROR = 155u32;
pub const UFX_LIVEDUMP: BUGCHECK_ERROR = 408u32;
pub const UNDNAME_32_BIT_DECODE: u32 = 2048u32;
pub const UNDNAME_COMPLETE: u32 = 0u32;
pub const UNDNAME_NAME_ONLY: u32 = 4096u32;
pub const UNDNAME_NO_ACCESS_SPECIFIERS: u32 = 128u32;
pub const UNDNAME_NO_ALLOCATION_LANGUAGE: u32 = 16u32;
pub const UNDNAME_NO_ALLOCATION_MODEL: u32 = 8u32;
pub const UNDNAME_NO_ARGUMENTS: u32 = 8192u32;
pub const UNDNAME_NO_CV_THISTYPE: u32 = 64u32;
pub const UNDNAME_NO_FUNCTION_RETURNS: u32 = 4u32;
pub const UNDNAME_NO_LEADING_UNDERSCORES: u32 = 1u32;
pub const UNDNAME_NO_MEMBER_TYPE: u32 = 512u32;
pub const UNDNAME_NO_MS_KEYWORDS: u32 = 2u32;
pub const UNDNAME_NO_MS_THISTYPE: u32 = 32u32;
pub const UNDNAME_NO_RETURN_UDT_MODEL: u32 = 1024u32;
pub const UNDNAME_NO_SPECIAL_SYMS: u32 = 16384u32;
pub const UNDNAME_NO_THISTYPE: u32 = 96u32;
pub const UNDNAME_NO_THROW_SIGNATURES: u32 = 256u32;
pub const UNEXPECTED_INITIALIZATION_CALL: BUGCHECK_ERROR = 51u32;
pub const UNEXPECTED_KERNEL_MODE_TRAP: BUGCHECK_ERROR = 127u32;
pub const UNEXPECTED_KERNEL_MODE_TRAP_M: BUGCHECK_ERROR = 268435583u32;
pub const UNEXPECTED_STORE_EXCEPTION: BUGCHECK_ERROR = 340u32;
pub const UNLOAD_DLL_DEBUG_EVENT: DEBUG_EVENT_CODE = 7u32;
pub const UNMOUNTABLE_BOOT_VOLUME: BUGCHECK_ERROR = 237u32;
pub const UNSUPPORTED_INSTRUCTION_MODE: BUGCHECK_ERROR = 337u32;
pub const UNSUPPORTED_PROCESSOR: BUGCHECK_ERROR = 93u32;
pub const UNWIND_ON_INVALID_STACK: BUGCHECK_ERROR = 427u32;
pub const UNW_FLAG_CHAININFO: RTL_VIRTUAL_UNWIND_HANDLER_TYPE = 4u32;
pub const UNW_FLAG_EHANDLER: RTL_VIRTUAL_UNWIND_HANDLER_TYPE = 1u32;
pub const UNW_FLAG_NHANDLER: RTL_VIRTUAL_UNWIND_HANDLER_TYPE = 0u32;
pub const UNW_FLAG_UHANDLER: RTL_VIRTUAL_UNWIND_HANDLER_TYPE = 2u32;
pub const UP_DRIVER_ON_MP_SYSTEM: BUGCHECK_ERROR = 146u32;
pub const USB4_HARDWARE_VIOLATION: BUGCHECK_ERROR = 496u32;
pub const USB_DRIPS_BLOCKER_SURPRISE_REMOVAL_LIVEDUMP: BUGCHECK_ERROR = 421u32;
pub const USER_MODE_HEALTH_MONITOR: BUGCHECK_ERROR = 158u32;
pub const USER_MODE_HEALTH_MONITOR_LIVEDUMP: BUGCHECK_ERROR = 457u32;
pub const UnloadedModuleListStream: MINIDUMP_STREAM_TYPE = 14i32;
pub const UnusedStream: MINIDUMP_STREAM_TYPE = 0i32;
pub const VER_PLATFORM_WIN32_NT: VER_PLATFORM = 2u32;
pub const VER_PLATFORM_WIN32_WINDOWS: VER_PLATFORM = 1u32;
pub const VER_PLATFORM_WIN32s: VER_PLATFORM = 0u32;
pub const VHD_BOOT_HOST_VOLUME_NOT_ENOUGH_SPACE: BUGCHECK_ERROR = 310u32;
pub const VHD_BOOT_INITIALIZATION_FAILED: BUGCHECK_ERROR = 303u32;
pub const VIDEO_DRIVER_DEBUG_REPORT_REQUEST: BUGCHECK_ERROR = 1073741997u32;
pub const VIDEO_DRIVER_INIT_FAILURE: BUGCHECK_ERROR = 180u32;
pub const VIDEO_DWMINIT_TIMEOUT_FALLBACK_BDD: BUGCHECK_ERROR = 391u32;
pub const VIDEO_DXGKRNL_BLACK_SCREEN_LIVEDUMP: BUGCHECK_ERROR = 424u32;
pub const VIDEO_DXGKRNL_FATAL_ERROR: BUGCHECK_ERROR = 275u32;
pub const VIDEO_DXGKRNL_LIVEDUMP: BUGCHECK_ERROR = 403u32;
pub const VIDEO_DXGKRNL_SYSMM_FATAL_ERROR: BUGCHECK_ERROR = 484u32;
pub const VIDEO_ENGINE_TIMEOUT_DETECTED: BUGCHECK_ERROR = 321u32;
pub const VIDEO_MEMORY_MANAGEMENT_INTERNAL: BUGCHECK_ERROR = 270u32;
pub const VIDEO_MINIPORT_BLACK_SCREEN_LIVEDUMP: BUGCHECK_ERROR = 440u32;
pub const VIDEO_MINIPORT_FAILED_LIVEDUMP: BUGCHECK_ERROR = 432u32;
pub const VIDEO_SCHEDULER_INTERNAL_ERROR: BUGCHECK_ERROR = 281u32;
pub const VIDEO_SHADOW_DRIVER_FATAL_ERROR: BUGCHECK_ERROR = 276u32;
pub const VIDEO_TDR_APPLICATION_BLOCKED: BUGCHECK_ERROR = 322u32;
pub const VIDEO_TDR_FAILURE: BUGCHECK_ERROR = 278u32;
pub const VIDEO_TDR_TIMEOUT_DETECTED: BUGCHECK_ERROR = 279u32;
pub const VMBUS_LIVEDUMP: BUGCHECK_ERROR = 1073742319u32;
pub const VOLMGRX_INTERNAL_ERROR: BUGCHECK_ERROR = 88u32;
pub const VOLSNAP_OVERLAPPED_TABLE_ACCESS: BUGCHECK_ERROR = 301u32;
pub const VSL_INITIALIZATION_FAILED: BUGCHECK_ERROR = 111u32;
pub const VmPostReadCallback: MINIDUMP_CALLBACK_TYPE = 20i32;
pub const VmPreReadCallback: MINIDUMP_CALLBACK_TYPE = 19i32;
pub const VmQueryCallback: MINIDUMP_CALLBACK_TYPE = 18i32;
pub const VmStartCallback: MINIDUMP_CALLBACK_TYPE = 17i32;
pub const WCT_ASYNC_OPEN_FLAG: OPEN_THREAD_WAIT_CHAIN_SESSION_FLAGS = 1u32;
pub const WCT_MAX_NODE_COUNT: u32 = 16u32;
pub const WCT_NETWORK_IO_FLAG: u32 = 8u32;
pub const WCT_OBJNAME_LENGTH: u32 = 128u32;
pub const WCT_OUT_OF_PROC_COM_FLAG: WAIT_CHAIN_THREAD_OPTIONS = 2u32;
pub const WCT_OUT_OF_PROC_CS_FLAG: WAIT_CHAIN_THREAD_OPTIONS = 4u32;
pub const WCT_OUT_OF_PROC_FLAG: WAIT_CHAIN_THREAD_OPTIONS = 1u32;
pub const WDF_VIOLATION: BUGCHECK_ERROR = 269u32;
pub const WFP_INVALID_OPERATION: BUGCHECK_ERROR = 467u32;
pub const WHEA_BAD_PAGE_LIST_LOCATION: u32 = 15u32;
pub const WHEA_BAD_PAGE_LIST_MAX_SIZE: u32 = 14u32;
pub const WHEA_CMCI_THRESHOLD_COUNT: u32 = 10u32;
pub const WHEA_CMCI_THRESHOLD_POLL_COUNT: u32 = 12u32;
pub const WHEA_CMCI_THRESHOLD_TIME: u32 = 11u32;
pub const WHEA_DEVICE_DRIVER_BUFFER_SET_MAX: u32 = 1u32;
pub const WHEA_DEVICE_DRIVER_BUFFER_SET_MIN: u32 = 1u32;
pub const WHEA_DEVICE_DRIVER_BUFFER_SET_V1: u32 = 1u32;
pub const WHEA_DEVICE_DRIVER_CONFIG_MAX: u32 = 2u32;
pub const WHEA_DEVICE_DRIVER_CONFIG_MIN: u32 = 1u32;
pub const WHEA_DEVICE_DRIVER_CONFIG_V1: u32 = 1u32;
pub const WHEA_DEVICE_DRIVER_CONFIG_V2: u32 = 2u32;
pub const WHEA_DISABLE_DUMMY_WRITE: u32 = 6u32;
pub const WHEA_DISABLE_OFFLINE: u32 = 0u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_AERBRIDGE: u32 = 8u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_AERENDPOINT: u32 = 7u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_AERROOTPORT: u32 = 6u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_GENERIC: u32 = 9u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_GENERIC_V2: u32 = 10u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_IPFCMC: u32 = 4u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_IPFCPE: u32 = 5u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_IPFMCA: u32 = 3u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_XPFCMC: u32 = 1u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_XPFMCE: u32 = 0u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_TYPE_XPFNMI: u32 = 2u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_VERSION_10: u32 = 10u32;
pub const WHEA_ERROR_SOURCE_DESCRIPTOR_VERSION_11: u32 = 11u32;
pub const WHEA_ERROR_SOURCE_FLAG_DEFAULTSOURCE: u32 = 2147483648u32;
pub const WHEA_ERROR_SOURCE_FLAG_FIRMWAREFIRST: u32 = 1u32;
pub const WHEA_ERROR_SOURCE_FLAG_GHES_ASSIST: u32 = 4u32;
pub const WHEA_ERROR_SOURCE_FLAG_GLOBAL: u32 = 2u32;
pub const WHEA_ERROR_SOURCE_INVALID_RELATED_SOURCE: u32 = 65535u32;
pub const WHEA_INTERNAL_ERROR: BUGCHECK_ERROR = 290u32;
pub const WHEA_MAX_MC_BANKS: u32 = 32u32;
pub const WHEA_MEM_PERSISTOFFLINE: u32 = 1u32;
pub const WHEA_MEM_PFA_DISABLE: u32 = 2u32;
pub const WHEA_MEM_PFA_PAGECOUNT: u32 = 3u32;
pub const WHEA_MEM_PFA_THRESHOLD: u32 = 4u32;
pub const WHEA_MEM_PFA_TIMEOUT: u32 = 5u32;
pub const WHEA_NOTIFICATION_TYPE_ARMV8_SEA: u32 = 8u32;
pub const WHEA_NOTIFICATION_TYPE_ARMV8_SEI: u32 = 9u32;
pub const WHEA_NOTIFICATION_TYPE_CMCI: u32 = 5u32;
pub const WHEA_NOTIFICATION_TYPE_EXTERNALINTERRUPT: u32 = 1u32;
pub const WHEA_NOTIFICATION_TYPE_EXTERNALINTERRUPT_GSIV: u32 = 10u32;
pub const WHEA_NOTIFICATION_TYPE_GPIO_SIGNAL: u32 = 7u32;
pub const WHEA_NOTIFICATION_TYPE_LOCALINTERRUPT: u32 = 2u32;
pub const WHEA_NOTIFICATION_TYPE_MCE: u32 = 6u32;
pub const WHEA_NOTIFICATION_TYPE_NMI: u32 = 4u32;
pub const WHEA_NOTIFICATION_TYPE_POLLED: u32 = 0u32;
pub const WHEA_NOTIFICATION_TYPE_SCI: u32 = 3u32;
pub const WHEA_NOTIFICATION_TYPE_SDEI: u32 = 11u32;
pub const WHEA_NOTIFY_ALL_OFFLINES: u32 = 16u32;
pub const WHEA_PENDING_PAGE_LIST_SZ: u32 = 13u32;
pub const WHEA_RESTORE_CMCI_ATTEMPTS: u32 = 8u32;
pub const WHEA_RESTORE_CMCI_ENABLED: u32 = 7u32;
pub const WHEA_RESTORE_CMCI_ERR_LIMIT: u32 = 9u32;
pub const WHEA_ROW_FAIL_CHECK_ENABLE: u32 = 18u32;
pub const WHEA_ROW_FAIL_CHECK_EXTENT: u32 = 17u32;
pub const WHEA_ROW_FAIL_CHECK_THRESHOLD: u32 = 19u32;
pub const WHEA_UNCORRECTABLE_ERROR: BUGCHECK_ERROR = 292u32;
pub const WHEA_XPF_MC_BANK_STATUSFORMAT_AMD64MCA: u32 = 2u32;
pub const WHEA_XPF_MC_BANK_STATUSFORMAT_IA32MCA: u32 = 0u32;
pub const WHEA_XPF_MC_BANK_STATUSFORMAT_Intel64MCA: u32 = 1u32;
pub const WIN32K_ATOMIC_CHECK_FAILURE: BUGCHECK_ERROR = 352u32;
pub const WIN32K_CALLOUT_WATCHDOG_BUGCHECK: BUGCHECK_ERROR = 418u32;
pub const WIN32K_CALLOUT_WATCHDOG_LIVEDUMP: BUGCHECK_ERROR = 417u32;
pub const WIN32K_CRITICAL_FAILURE: BUGCHECK_ERROR = 356u32;
pub const WIN32K_CRITICAL_FAILURE_LIVEDUMP: BUGCHECK_ERROR = 400u32;
pub const WIN32K_HANDLE_MANAGER: BUGCHECK_ERROR = 311u32;
pub const WIN32K_INIT_OR_RIT_FAILURE: BUGCHECK_ERROR = 145u32;
pub const WIN32K_POWER_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 412u32;
pub const WIN32K_SECURITY_FAILURE: BUGCHECK_ERROR = 407u32;
pub const WINDOWS_NT_BANNER: BUGCHECK_ERROR = 1073741950u32;
pub const WINDOWS_NT_CSD_STRING: BUGCHECK_ERROR = 1073741959u32;
pub const WINDOWS_NT_INFO_STRING: BUGCHECK_ERROR = 1073741960u32;
pub const WINDOWS_NT_INFO_STRING_PLURAL: BUGCHECK_ERROR = 1073741981u32;
pub const WINDOWS_NT_MP_STRING: BUGCHECK_ERROR = 1073741961u32;
pub const WINDOWS_NT_RC_STRING: BUGCHECK_ERROR = 1073741982u32;
pub const WINLOGON_FATAL_ERROR: BUGCHECK_ERROR = 3221226010u32;
pub const WINSOCK_DETECTED_HUNG_CLOSESOCKET_LIVEDUMP: BUGCHECK_ERROR = 342u32;
pub const WORKER_INVALID: BUGCHECK_ERROR = 228u32;
pub const WORKER_THREAD_INVALID_STATE: BUGCHECK_ERROR = 466u32;
pub const WORKER_THREAD_RETURNED_AT_BAD_IRQL: BUGCHECK_ERROR = 225u32;
pub const WORKER_THREAD_RETURNED_WHILE_ATTACHED_TO_SILO: BUGCHECK_ERROR = 410u32;
pub const WORKER_THREAD_RETURNED_WITH_BAD_IO_PRIORITY: BUGCHECK_ERROR = 296u32;
pub const WORKER_THREAD_RETURNED_WITH_BAD_PAGING_IO_PRIORITY: BUGCHECK_ERROR = 297u32;
pub const WORKER_THREAD_RETURNED_WITH_NON_DEFAULT_WORKLOAD_CLASS: BUGCHECK_ERROR = 470u32;
pub const WORKER_THREAD_RETURNED_WITH_SYSTEM_PAGE_PRIORITY_ACTIVE: BUGCHECK_ERROR = 347u32;
pub const WORKER_THREAD_TEST_CONDITION: BUGCHECK_ERROR = 355u32;
pub const WOW64_CONTEXT_ALL: WOW64_CONTEXT_FLAGS = 65599u32;
pub const WOW64_CONTEXT_CONTROL: WOW64_CONTEXT_FLAGS = 65537u32;
pub const WOW64_CONTEXT_DEBUG_REGISTERS: WOW64_CONTEXT_FLAGS = 65552u32;
pub const WOW64_CONTEXT_EXCEPTION_ACTIVE: WOW64_CONTEXT_FLAGS = 134217728u32;
pub const WOW64_CONTEXT_EXCEPTION_REPORTING: WOW64_CONTEXT_FLAGS = 2147483648u32;
pub const WOW64_CONTEXT_EXCEPTION_REQUEST: WOW64_CONTEXT_FLAGS = 1073741824u32;
pub const WOW64_CONTEXT_EXTENDED_REGISTERS: WOW64_CONTEXT_FLAGS = 65568u32;
pub const WOW64_CONTEXT_FLOATING_POINT: WOW64_CONTEXT_FLAGS = 65544u32;
pub const WOW64_CONTEXT_FULL: WOW64_CONTEXT_FLAGS = 65543u32;
pub const WOW64_CONTEXT_INTEGER: WOW64_CONTEXT_FLAGS = 65538u32;
pub const WOW64_CONTEXT_SEGMENTS: WOW64_CONTEXT_FLAGS = 65540u32;
pub const WOW64_CONTEXT_SERVICE_ACTIVE: WOW64_CONTEXT_FLAGS = 268435456u32;
pub const WOW64_CONTEXT_X86: WOW64_CONTEXT_FLAGS = 65536u32;
pub const WOW64_CONTEXT_XSTATE: WOW64_CONTEXT_FLAGS = 65600u32;
pub const WOW64_MAXIMUM_SUPPORTED_EXTENSION: u32 = 512u32;
pub const WOW64_SIZE_OF_80387_REGISTERS: u32 = 80u32;
pub const WVR_LIVEDUMP_APP_IO_TIMEOUT: BUGCHECK_ERROR = 387u32;
pub const WVR_LIVEDUMP_CRITICAL_ERROR: BUGCHECK_ERROR = 390u32;
pub const WVR_LIVEDUMP_MANUALLY_INITIATED: BUGCHECK_ERROR = 388u32;
pub const WVR_LIVEDUMP_RECOVERY_IOCONTEXT_TIMEOUT: BUGCHECK_ERROR = 386u32;
pub const WVR_LIVEDUMP_REPLICATION_IOCONTEXT_TIMEOUT: BUGCHECK_ERROR = 384u32;
pub const WVR_LIVEDUMP_STATE_FAILURE: BUGCHECK_ERROR = 389u32;
pub const WVR_LIVEDUMP_STATE_TRANSITION_TIMEOUT: BUGCHECK_ERROR = 385u32;
pub const WctAlpcType: WCT_OBJECT_TYPE = 4i32;
pub const WctComActivationType: WCT_OBJECT_TYPE = 9i32;
pub const WctComType: WCT_OBJECT_TYPE = 5i32;
pub const WctCriticalSectionType: WCT_OBJECT_TYPE = 1i32;
pub const WctMaxType: WCT_OBJECT_TYPE = 13i32;
pub const WctMutexType: WCT_OBJECT_TYPE = 3i32;
pub const WctProcessWaitType: WCT_OBJECT_TYPE = 7i32;
pub const WctSendMessageType: WCT_OBJECT_TYPE = 2i32;
pub const WctSmbIoType: WCT_OBJECT_TYPE = 12i32;
pub const WctSocketIoType: WCT_OBJECT_TYPE = 11i32;
pub const WctStatusAbandoned: WCT_OBJECT_STATUS = 8i32;
pub const WctStatusBlocked: WCT_OBJECT_STATUS = 3i32;
pub const WctStatusError: WCT_OBJECT_STATUS = 10i32;
pub const WctStatusMax: WCT_OBJECT_STATUS = 11i32;
pub const WctStatusNoAccess: WCT_OBJECT_STATUS = 1i32;
pub const WctStatusNotOwned: WCT_OBJECT_STATUS = 7i32;
pub const WctStatusOwned: WCT_OBJECT_STATUS = 6i32;
pub const WctStatusPidOnly: WCT_OBJECT_STATUS = 4i32;
pub const WctStatusPidOnlyRpcss: WCT_OBJECT_STATUS = 5i32;
pub const WctStatusRunning: WCT_OBJECT_STATUS = 2i32;
pub const WctStatusUnknown: WCT_OBJECT_STATUS = 9i32;
pub const WctThreadType: WCT_OBJECT_TYPE = 8i32;
pub const WctThreadWaitType: WCT_OBJECT_TYPE = 6i32;
pub const WctUnknownType: WCT_OBJECT_TYPE = 10i32;
pub const WheaErrSrcStateRemovePending: WHEA_ERROR_SOURCE_STATE = 4i32;
pub const WheaErrSrcStateRemoved: WHEA_ERROR_SOURCE_STATE = 3i32;
pub const WheaErrSrcStateStarted: WHEA_ERROR_SOURCE_STATE = 2i32;
pub const WheaErrSrcStateStopped: WHEA_ERROR_SOURCE_STATE = 1i32;
pub const WheaErrSrcTypeBMC: WHEA_ERROR_SOURCE_TYPE = 14i32;
pub const WheaErrSrcTypeBOOT: WHEA_ERROR_SOURCE_TYPE = 7i32;
pub const WheaErrSrcTypeCMC: WHEA_ERROR_SOURCE_TYPE = 1i32;
pub const WheaErrSrcTypeCPE: WHEA_ERROR_SOURCE_TYPE = 2i32;
pub const WheaErrSrcTypeDeviceDriver: WHEA_ERROR_SOURCE_TYPE = 16i32;
pub const WheaErrSrcTypeGeneric: WHEA_ERROR_SOURCE_TYPE = 5i32;
pub const WheaErrSrcTypeGenericV2: WHEA_ERROR_SOURCE_TYPE = 12i32;
pub const WheaErrSrcTypeINIT: WHEA_ERROR_SOURCE_TYPE = 6i32;
pub const WheaErrSrcTypeIPFCMC: WHEA_ERROR_SOURCE_TYPE = 10i32;
pub const WheaErrSrcTypeIPFCPE: WHEA_ERROR_SOURCE_TYPE = 11i32;
pub const WheaErrSrcTypeIPFMCA: WHEA_ERROR_SOURCE_TYPE = 9i32;
pub const WheaErrSrcTypeMCE: WHEA_ERROR_SOURCE_TYPE = 0i32;
pub const WheaErrSrcTypeMax: WHEA_ERROR_SOURCE_TYPE = 19i32;
pub const WheaErrSrcTypeNMI: WHEA_ERROR_SOURCE_TYPE = 3i32;
pub const WheaErrSrcTypePCIe: WHEA_ERROR_SOURCE_TYPE = 4i32;
pub const WheaErrSrcTypePMEM: WHEA_ERROR_SOURCE_TYPE = 15i32;
pub const WheaErrSrcTypeSCIGeneric: WHEA_ERROR_SOURCE_TYPE = 8i32;
pub const WheaErrSrcTypeSCIGenericV2: WHEA_ERROR_SOURCE_TYPE = 13i32;
pub const WheaErrSrcTypeSea: WHEA_ERROR_SOURCE_TYPE = 17i32;
pub const WheaErrSrcTypeSei: WHEA_ERROR_SOURCE_TYPE = 18i32;
pub const WriteKernelMinidumpCallback: MINIDUMP_CALLBACK_TYPE = 7i32;
pub const XBOX_360_SYSTEM_CRASH: BUGCHECK_ERROR = 864u32;
pub const XBOX_360_SYSTEM_CRASH_RESERVED: BUGCHECK_ERROR = 1056u32;
pub const XBOX_CORRUPTED_IMAGE: BUGCHECK_ERROR = 855u32;
pub const XBOX_CORRUPTED_IMAGE_BASE: BUGCHECK_ERROR = 857u32;
pub const XBOX_INVERTED_FUNCTION_TABLE_OVERFLOW: BUGCHECK_ERROR = 856u32;
pub const XBOX_MANUALLY_INITIATED_CRASH: BUGCHECK_ERROR = 196614u32;
pub const XBOX_SECURITY_FAILUE: BUGCHECK_ERROR = 1057u32;
pub const XBOX_SHUTDOWN_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 859u32;
pub const XBOX_VMCTRL_CS_TIMEOUT: BUGCHECK_ERROR = 854u32;
pub const XBOX_XDS_WATCHDOG_TIMEOUT: BUGCHECK_ERROR = 858u32;
pub const XNS_INTERNAL_ERROR: BUGCHECK_ERROR = 87u32;
pub const ceStreamBucketParameters: MINIDUMP_STREAM_TYPE = 32778i32;
pub const ceStreamDiagnosisList: MINIDUMP_STREAM_TYPE = 32780i32;
pub const ceStreamException: MINIDUMP_STREAM_TYPE = 32770i32;
pub const ceStreamMemoryPhysicalList: MINIDUMP_STREAM_TYPE = 32777i32;
pub const ceStreamMemoryVirtualList: MINIDUMP_STREAM_TYPE = 32776i32;
pub const ceStreamModuleList: MINIDUMP_STREAM_TYPE = 32771i32;
pub const ceStreamNull: MINIDUMP_STREAM_TYPE = 32768i32;
pub const ceStreamProcessList: MINIDUMP_STREAM_TYPE = 32772i32;
pub const ceStreamProcessModuleMap: MINIDUMP_STREAM_TYPE = 32779i32;
pub const ceStreamSystemInfo: MINIDUMP_STREAM_TYPE = 32769i32;
pub const ceStreamThreadCallStackList: MINIDUMP_STREAM_TYPE = 32775i32;
pub const ceStreamThreadContextList: MINIDUMP_STREAM_TYPE = 32774i32;
pub const ceStreamThreadList: MINIDUMP_STREAM_TYPE = 32773i32;
pub const hdBase: IMAGEHLP_HD_TYPE = 0i32;
pub const hdMax: IMAGEHLP_HD_TYPE = 3i32;
pub const hdSrc: IMAGEHLP_HD_TYPE = 2i32;
pub const hdSym: IMAGEHLP_HD_TYPE = 1i32;
pub const sevAttn: IMAGEHLP_CBA_EVENT_SEVERITY = 2u32;
pub const sevFatal: IMAGEHLP_CBA_EVENT_SEVERITY = 3u32;
pub const sevInfo: IMAGEHLP_CBA_EVENT_SEVERITY = 0u32;
pub const sevMax: i32 = 4i32;
pub const sevProblem: IMAGEHLP_CBA_EVENT_SEVERITY = 1u32;
pub const sfDbg: IMAGEHLP_SF_TYPE = 1i32;
pub const sfImage: IMAGEHLP_SF_TYPE = 0i32;
pub const sfMax: IMAGEHLP_SF_TYPE = 4i32;
pub const sfMpd: IMAGEHLP_SF_TYPE = 3i32;
pub const sfPdb: IMAGEHLP_SF_TYPE = 2i32;
pub type ADDRESS_MODE = i32;
pub type BUGCHECK_ERROR = u32;
pub type CONTEXT_FLAGS = u32;
pub type DBGPROP_ATTRIB_FLAGS = i32;
pub type DBGPROP_INFO = i32;
pub type DEBUG_EVENT_CODE = u32;
pub type DUMP_TYPE = i32;
pub type EX_PROP_INFO_FLAGS = i32;
pub type FACILITY_CODE = u32;
pub type FORMAT_MESSAGE_OPTIONS = u32;
pub type IMAGEHLP_CBA_EVENT_SEVERITY = u32;
pub type IMAGEHLP_EXTENDED_OPTIONS = i32;
pub type IMAGEHLP_GET_TYPE_INFO_FLAGS = u32;
pub type IMAGEHLP_HD_TYPE = i32;
pub type IMAGEHLP_SF_TYPE = i32;
pub type IMAGEHLP_STATUS_REASON = i32;
pub type IMAGEHLP_SYMBOL_TYPE_INFO = i32;
pub type IMAGE_DEBUG_TYPE = u32;
pub type IMAGE_DIRECTORY_ENTRY = u16;
pub type IMAGE_DLL_CHARACTERISTICS = u16;
pub type IMAGE_FILE_CHARACTERISTICS = u16;
pub type IMAGE_FILE_CHARACTERISTICS2 = u32;
pub type IMAGE_OPTIONAL_HEADER_MAGIC = u16;
pub type IMAGE_SECTION_CHARACTERISTICS = u32;
pub type IMAGE_SUBSYSTEM = u16;
pub type IPMI_OS_SEL_RECORD_TYPE = i32;
pub type MINIDUMP_CALLBACK_TYPE = i32;
pub type MINIDUMP_HANDLE_OBJECT_INFORMATION_TYPE = i32;
pub type MINIDUMP_MISC_INFO_FLAGS = u32;
pub type MINIDUMP_SECONDARY_FLAGS = i32;
pub type MINIDUMP_STREAM_TYPE = i32;
pub type MINIDUMP_THREAD_INFO_DUMP_FLAGS = u32;
pub type MINIDUMP_TYPE = i32;
pub type MODLOAD_DATA_TYPE = u32;
pub type MODULE_WRITE_FLAGS = i32;
pub type OBJECT_ATTRIB_FLAGS = i32;
pub type OPEN_THREAD_WAIT_CHAIN_SESSION_FLAGS = u32;
pub type PROP_INFO_FLAGS = i32;
pub type RIP_INFO_TYPE = u32;
pub type RTL_VIRTUAL_UNWIND_HANDLER_TYPE = u32;
pub type SYMBOL_INFO_FLAGS = u32;
pub type SYM_FIND_ID_OPTION = u32;
pub type SYM_LOAD_FLAGS = u32;
pub type SYM_SRV_STORE_FILE_FLAGS = u32;
pub type SYM_TYPE = i32;
pub type THREAD_ERROR_MODE = u32;
pub type THREAD_WRITE_FLAGS = i32;
pub type VER_PLATFORM = u32;
pub type WAIT_CHAIN_THREAD_OPTIONS = u32;
pub type WCT_OBJECT_STATUS = i32;
pub type WCT_OBJECT_TYPE = i32;
pub type WHEA_ERROR_SOURCE_STATE = i32;
pub type WHEA_ERROR_SOURCE_TYPE = i32;
pub type WOW64_CONTEXT_FLAGS = u32;
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct ADDRESS {
    pub Offset: u32,
    pub Segment: u16,
    pub Mode: ADDRESS_MODE,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for ADDRESS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ADDRESS64 {
    pub Offset: u64,
    pub Segment: u16,
    pub Mode: ADDRESS_MODE,
}
impl ::core::marker::Copy for ADDRESS64 {}
impl ::core::clone::Clone for ADDRESS64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union AER_BRIDGE_DESCRIPTOR_FLAGS {
    pub Anonymous: AER_BRIDGE_DESCRIPTOR_FLAGS_0,
    pub AsUSHORT: u16,
}
impl ::core::marker::Copy for AER_BRIDGE_DESCRIPTOR_FLAGS {}
impl ::core::clone::Clone for AER_BRIDGE_DESCRIPTOR_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct AER_BRIDGE_DESCRIPTOR_FLAGS_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for AER_BRIDGE_DESCRIPTOR_FLAGS_0 {}
impl ::core::clone::Clone for AER_BRIDGE_DESCRIPTOR_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union AER_ENDPOINT_DESCRIPTOR_FLAGS {
    pub Anonymous: AER_ENDPOINT_DESCRIPTOR_FLAGS_0,
    pub AsUSHORT: u16,
}
impl ::core::marker::Copy for AER_ENDPOINT_DESCRIPTOR_FLAGS {}
impl ::core::clone::Clone for AER_ENDPOINT_DESCRIPTOR_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct AER_ENDPOINT_DESCRIPTOR_FLAGS_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for AER_ENDPOINT_DESCRIPTOR_FLAGS_0 {}
impl ::core::clone::Clone for AER_ENDPOINT_DESCRIPTOR_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union AER_ROOTPORT_DESCRIPTOR_FLAGS {
    pub Anonymous: AER_ROOTPORT_DESCRIPTOR_FLAGS_0,
    pub AsUSHORT: u16,
}
impl ::core::marker::Copy for AER_ROOTPORT_DESCRIPTOR_FLAGS {}
impl ::core::clone::Clone for AER_ROOTPORT_DESCRIPTOR_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct AER_ROOTPORT_DESCRIPTOR_FLAGS_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for AER_ROOTPORT_DESCRIPTOR_FLAGS_0 {}
impl ::core::clone::Clone for AER_ROOTPORT_DESCRIPTOR_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(feature = "Win32_System_Kernel")]
pub struct APC_CALLBACK_DATA {
    pub Parameter: usize,
    pub ContextRecord: *mut CONTEXT,
    pub Reserved0: usize,
    pub Reserved1: usize,
}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for APC_CALLBACK_DATA {}
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for APC_CALLBACK_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct API_VERSION {
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub Revision: u16,
    pub Reserved: u16,
}
impl ::core::marker::Copy for API_VERSION {}
impl ::core::clone::Clone for API_VERSION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub struct ARM64_NT_CONTEXT {
    pub ContextFlags: u32,
    pub Cpsr: u32,
    pub Anonymous: ARM64_NT_CONTEXT_0,
    pub Sp: u64,
    pub Pc: u64,
    pub V: [ARM64_NT_NEON128; 32],
    pub Fpcr: u32,
    pub Fpsr: u32,
    pub Bcr: [u32; 8],
    pub Bvr: [u64; 8],
    pub Wcr: [u32; 2],
    pub Wvr: [u64; 2],
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ::core::marker::Copy for ARM64_NT_CONTEXT {}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ::core::clone::Clone for ARM64_NT_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub union ARM64_NT_CONTEXT_0 {
    pub Anonymous: ARM64_NT_CONTEXT_0_0,
    pub X: [u64; 31],
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ::core::marker::Copy for ARM64_NT_CONTEXT_0 {}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ::core::clone::Clone for ARM64_NT_CONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub struct ARM64_NT_CONTEXT_0_0 {
    pub X0: u64,
    pub X1: u64,
    pub X2: u64,
    pub X3: u64,
    pub X4: u64,
    pub X5: u64,
    pub X6: u64,
    pub X7: u64,
    pub X8: u64,
    pub X9: u64,
    pub X10: u64,
    pub X11: u64,
    pub X12: u64,
    pub X13: u64,
    pub X14: u64,
    pub X15: u64,
    pub X16: u64,
    pub X17: u64,
    pub X18: u64,
    pub X19: u64,
    pub X20: u64,
    pub X21: u64,
    pub X22: u64,
    pub X23: u64,
    pub X24: u64,
    pub X25: u64,
    pub X26: u64,
    pub X27: u64,
    pub X28: u64,
    pub Fp: u64,
    pub Lr: u64,
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ::core::marker::Copy for ARM64_NT_CONTEXT_0_0 {}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ::core::clone::Clone for ARM64_NT_CONTEXT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union ARM64_NT_NEON128 {
    pub Anonymous: ARM64_NT_NEON128_0,
    pub D: [f64; 2],
    pub S: [f32; 4],
    pub H: [u16; 8],
    pub B: [u8; 16],
}
impl ::core::marker::Copy for ARM64_NT_NEON128 {}
impl ::core::clone::Clone for ARM64_NT_NEON128 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ARM64_NT_NEON128_0 {
    pub Low: u64,
    pub High: i64,
}
impl ::core::marker::Copy for ARM64_NT_NEON128_0 {}
impl ::core::clone::Clone for ARM64_NT_NEON128_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
pub struct CONTEXT {
    pub ContextFlags: CONTEXT_FLAGS,
    pub Cpsr: u32,
    pub Anonymous: CONTEXT_0,
    pub Sp: u64,
    pub Pc: u64,
    pub V: [ARM64_NT_NEON128; 32],
    pub Fpcr: u32,
    pub Fpsr: u32,
    pub Bcr: [u32; 8],
    pub Bvr: [u64; 8],
    pub Wcr: [u32; 2],
    pub Wvr: [u64; 2],
}
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT {}
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
pub union CONTEXT_0 {
    pub Anonymous: CONTEXT_0_0,
    pub X: [u64; 31],
}
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT_0 {}
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
pub struct CONTEXT_0_0 {
    pub X0: u64,
    pub X1: u64,
    pub X2: u64,
    pub X3: u64,
    pub X4: u64,
    pub X5: u64,
    pub X6: u64,
    pub X7: u64,
    pub X8: u64,
    pub X9: u64,
    pub X10: u64,
    pub X11: u64,
    pub X12: u64,
    pub X13: u64,
    pub X14: u64,
    pub X15: u64,
    pub X16: u64,
    pub X17: u64,
    pub X18: u64,
    pub X19: u64,
    pub X20: u64,
    pub X21: u64,
    pub X22: u64,
    pub X23: u64,
    pub X24: u64,
    pub X25: u64,
    pub X26: u64,
    pub X27: u64,
    pub X28: u64,
    pub Fp: u64,
    pub Lr: u64,
}
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT_0_0 {}
#[cfg(target_arch = "aarch64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
pub struct CONTEXT {
    pub P1Home: u64,
    pub P2Home: u64,
    pub P3Home: u64,
    pub P4Home: u64,
    pub P5Home: u64,
    pub P6Home: u64,
    pub ContextFlags: CONTEXT_FLAGS,
    pub MxCsr: u32,
    pub SegCs: u16,
    pub SegDs: u16,
    pub SegEs: u16,
    pub SegFs: u16,
    pub SegGs: u16,
    pub SegSs: u16,
    pub EFlags: u32,
    pub Dr0: u64,
    pub Dr1: u64,
    pub Dr2: u64,
    pub Dr3: u64,
    pub Dr6: u64,
    pub Dr7: u64,
    pub Rax: u64,
    pub Rcx: u64,
    pub Rdx: u64,
    pub Rbx: u64,
    pub Rsp: u64,
    pub Rbp: u64,
    pub Rsi: u64,
    pub Rdi: u64,
    pub R8: u64,
    pub R9: u64,
    pub R10: u64,
    pub R11: u64,
    pub R12: u64,
    pub R13: u64,
    pub R14: u64,
    pub R15: u64,
    pub Rip: u64,
    pub Anonymous: CONTEXT_0,
    pub VectorRegister: [M128A; 26],
    pub VectorControl: u64,
    pub DebugControl: u64,
    pub LastBranchToRip: u64,
    pub LastBranchFromRip: u64,
    pub LastExceptionToRip: u64,
    pub LastExceptionFromRip: u64,
}
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT {}
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
pub union CONTEXT_0 {
    pub FltSave: XSAVE_FORMAT,
    pub Anonymous: CONTEXT_0_0,
}
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT_0 {}
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
pub struct CONTEXT_0_0 {
    pub Header: [M128A; 2],
    pub Legacy: [M128A; 8],
    pub Xmm0: M128A,
    pub Xmm1: M128A,
    pub Xmm2: M128A,
    pub Xmm3: M128A,
    pub Xmm4: M128A,
    pub Xmm5: M128A,
    pub Xmm6: M128A,
    pub Xmm7: M128A,
    pub Xmm8: M128A,
    pub Xmm9: M128A,
    pub Xmm10: M128A,
    pub Xmm11: M128A,
    pub Xmm12: M128A,
    pub Xmm13: M128A,
    pub Xmm14: M128A,
    pub Xmm15: M128A,
}
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT_0_0 {}
#[cfg(target_arch = "x86_64")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Kernel")]
pub struct CONTEXT {
    pub ContextFlags: CONTEXT_FLAGS,
    pub Dr0: u32,
    pub Dr1: u32,
    pub Dr2: u32,
    pub Dr3: u32,
    pub Dr6: u32,
    pub Dr7: u32,
    pub FloatSave: super::super::Kernel::FLOATING_SAVE_AREA,
    pub SegGs: u32,
    pub SegFs: u32,
    pub SegEs: u32,
    pub SegDs: u32,
    pub Edi: u32,
    pub Esi: u32,
    pub Ebx: u32,
    pub Edx: u32,
    pub Ecx: u32,
    pub Eax: u32,
    pub Ebp: u32,
    pub Eip: u32,
    pub SegCs: u32,
    pub EFlags: u32,
    pub Esp: u32,
    pub SegSs: u32,
    pub ExtendedRegisters: [u8; 512],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::marker::Copy for CONTEXT {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Kernel")]
impl ::core::clone::Clone for CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CPU_INFORMATION {
    pub X86CpuInfo: CPU_INFORMATION_1,
    pub OtherCpuInfo: CPU_INFORMATION_0,
}
impl ::core::marker::Copy for CPU_INFORMATION {}
impl ::core::clone::Clone for CPU_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct CPU_INFORMATION_0 {
    pub ProcessorFeatures: [u64; 2],
}
impl ::core::marker::Copy for CPU_INFORMATION_0 {}
impl ::core::clone::Clone for CPU_INFORMATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPU_INFORMATION_1 {
    pub VendorId: [u32; 3],
    pub VersionInformation: u32,
    pub FeatureInformation: u32,
    pub AMDExtendedCpuFeatures: u32,
}
impl ::core::marker::Copy for CPU_INFORMATION_1 {}
impl ::core::clone::Clone for CPU_INFORMATION_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
pub struct CREATE_PROCESS_DEBUG_INFO {
    pub hFile: super::super::super::Foundation::HANDLE,
    pub hProcess: super::super::super::Foundation::HANDLE,
    pub hThread: super::super::super::Foundation::HANDLE,
    pub lpBaseOfImage: *mut ::core::ffi::c_void,
    pub dwDebugInfoFileOffset: u32,
    pub nDebugInfoSize: u32,
    pub lpThreadLocalBase: *mut ::core::ffi::c_void,
    pub lpStartAddress: super::super::Threading::LPTHREAD_START_ROUTINE,
    pub lpImageName: *mut ::core::ffi::c_void,
    pub fUnicode: u16,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::marker::Copy for CREATE_PROCESS_DEBUG_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::clone::Clone for CREATE_PROCESS_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
pub struct CREATE_THREAD_DEBUG_INFO {
    pub hThread: super::super::super::Foundation::HANDLE,
    pub lpThreadLocalBase: *mut ::core::ffi::c_void,
    pub lpStartAddress: super::super::Threading::LPTHREAD_START_ROUTINE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::marker::Copy for CREATE_THREAD_DEBUG_INFO {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::clone::Clone for CREATE_THREAD_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DBGHELP_DATA_REPORT_STRUCT {
    pub pBinPathNonExist: ::windows_sys::core::PCWSTR,
    pub pSymbolPathNonExist: ::windows_sys::core::PCWSTR,
}
impl ::core::marker::Copy for DBGHELP_DATA_REPORT_STRUCT {}
impl ::core::clone::Clone for DBGHELP_DATA_REPORT_STRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
pub struct DEBUG_EVENT {
    pub dwDebugEventCode: DEBUG_EVENT_CODE,
    pub dwProcessId: u32,
    pub dwThreadId: u32,
    pub u: DEBUG_EVENT_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::marker::Copy for DEBUG_EVENT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::clone::Clone for DEBUG_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Threading\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
pub union DEBUG_EVENT_0 {
    pub Exception: EXCEPTION_DEBUG_INFO,
    pub CreateThread: CREATE_THREAD_DEBUG_INFO,
    pub CreateProcessInfo: CREATE_PROCESS_DEBUG_INFO,
    pub ExitThread: EXIT_THREAD_DEBUG_INFO,
    pub ExitProcess: EXIT_PROCESS_DEBUG_INFO,
    pub LoadDll: LOAD_DLL_DEBUG_INFO,
    pub UnloadDll: UNLOAD_DLL_DEBUG_INFO,
    pub DebugString: OUTPUT_DEBUG_STRING_INFO,
    pub RipInfo: RIP_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::marker::Copy for DEBUG_EVENT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Threading"))]
impl ::core::clone::Clone for DEBUG_EVENT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_CONTEXT {
    pub ControlPc: usize,
    pub ImageBase: usize,
    pub FunctionEntry: *mut IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY,
    pub EstablisherFrame: usize,
    pub TargetPc: usize,
    pub ContextRecord: *mut CONTEXT,
    pub LanguageHandler: super::super::Kernel::EXCEPTION_ROUTINE,
    pub HandlerData: *mut ::core::ffi::c_void,
    pub HistoryTable: *mut UNWIND_HISTORY_TABLE,
    pub ScopeIndex: u32,
    pub ControlPcIsUnwound: super::super::super::Foundation::BOOLEAN,
    pub NonVolatileRegisters: *mut u8,
}
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_CONTEXT {}
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86_64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct DISPATCHER_CONTEXT {
    pub ControlPc: u64,
    pub ImageBase: u64,
    pub FunctionEntry: *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
    pub EstablisherFrame: u64,
    pub TargetIp: u64,
    pub ContextRecord: *mut CONTEXT,
    pub LanguageHandler: super::super::Kernel::EXCEPTION_ROUTINE,
    pub HandlerData: *mut ::core::ffi::c_void,
    pub HistoryTable: *mut UNWIND_HISTORY_TABLE,
    pub ScopeIndex: u32,
    pub Fill0: u32,
}
#[cfg(target_arch = "x86_64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for DISPATCHER_CONTEXT {}
#[cfg(target_arch = "x86_64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for DISPATCHER_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union DUMP_FILE_ATTRIBUTES {
    pub Anonymous: DUMP_FILE_ATTRIBUTES_0,
    pub Attributes: u32,
}
impl ::core::marker::Copy for DUMP_FILE_ATTRIBUTES {}
impl ::core::clone::Clone for DUMP_FILE_ATTRIBUTES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DUMP_FILE_ATTRIBUTES_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for DUMP_FILE_ATTRIBUTES_0 {}
impl ::core::clone::Clone for DUMP_FILE_ATTRIBUTES_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DUMP_HEADER32 {
    pub Signature: u32,
    pub ValidDump: u32,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub DirectoryTableBase: u32,
    pub PfnDataBase: u32,
    pub PsLoadedModuleList: u32,
    pub PsActiveProcessHead: u32,
    pub MachineImageType: u32,
    pub NumberProcessors: u32,
    pub BugCheckCode: u32,
    pub BugCheckParameter1: u32,
    pub BugCheckParameter2: u32,
    pub BugCheckParameter3: u32,
    pub BugCheckParameter4: u32,
    pub VersionUser: [u8; 32],
    pub PaeEnabled: u8,
    pub KdSecondaryVersion: u8,
    pub Spare3: [u8; 2],
    pub KdDebuggerDataBlock: u32,
    pub Anonymous: DUMP_HEADER32_0,
    pub ContextRecord: [u8; 1200],
    pub Exception: EXCEPTION_RECORD32,
    pub Comment: [u8; 128],
    pub Attributes: DUMP_FILE_ATTRIBUTES,
    pub BootId: u32,
    pub _reserved0: [u8; 1760],
    pub DumpType: u32,
    pub MiniDumpFields: u32,
    pub SecondaryDataState: u32,
    pub ProductType: u32,
    pub SuiteMask: u32,
    pub WriterStatus: u32,
    pub RequiredDumpSpace: i64,
    pub _reserved2: [u8; 16],
    pub SystemUpTime: i64,
    pub SystemTime: i64,
    pub _reserved3: [u8; 56],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DUMP_HEADER32 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DUMP_HEADER32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union DUMP_HEADER32_0 {
    pub PhysicalMemoryBlock: PHYSICAL_MEMORY_DESCRIPTOR32,
    pub PhysicalMemoryBlockBuffer: [u8; 700],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DUMP_HEADER32_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DUMP_HEADER32_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DUMP_HEADER64 {
    pub Signature: u32,
    pub ValidDump: u32,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub DirectoryTableBase: u64,
    pub PfnDataBase: u64,
    pub PsLoadedModuleList: u64,
    pub PsActiveProcessHead: u64,
    pub MachineImageType: u32,
    pub NumberProcessors: u32,
    pub BugCheckCode: u32,
    pub BugCheckParameter1: u64,
    pub BugCheckParameter2: u64,
    pub BugCheckParameter3: u64,
    pub BugCheckParameter4: u64,
    pub VersionUser: [u8; 32],
    pub KdDebuggerDataBlock: u64,
    pub Anonymous: DUMP_HEADER64_0,
    pub ContextRecord: [u8; 3000],
    pub Exception: EXCEPTION_RECORD64,
    pub DumpType: u32,
    pub RequiredDumpSpace: i64,
    pub SystemTime: i64,
    pub Comment: [u8; 128],
    pub SystemUpTime: i64,
    pub MiniDumpFields: u32,
    pub SecondaryDataState: u32,
    pub ProductType: u32,
    pub SuiteMask: u32,
    pub WriterStatus: u32,
    pub Unused1: u8,
    pub KdSecondaryVersion: u8,
    pub Unused: [u8; 2],
    pub Attributes: DUMP_FILE_ATTRIBUTES,
    pub BootId: u32,
    pub _reserved0: [u8; 4008],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DUMP_HEADER64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DUMP_HEADER64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union DUMP_HEADER64_0 {
    pub PhysicalMemoryBlock: PHYSICAL_MEMORY_DESCRIPTOR64,
    pub PhysicalMemoryBlockBuffer: [u8; 700],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DUMP_HEADER64_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DUMP_HEADER64_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DebugPropertyInfo {
    pub m_dwValidFields: u32,
    pub m_bstrName: ::windows_sys::core::BSTR,
    pub m_bstrType: ::windows_sys::core::BSTR,
    pub m_bstrValue: ::windows_sys::core::BSTR,
    pub m_bstrFullName: ::windows_sys::core::BSTR,
    pub m_dwAttrib: u32,
    pub m_pDebugProp: IDebugProperty,
}
impl ::core::marker::Copy for DebugPropertyInfo {}
impl ::core::clone::Clone for DebugPropertyInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EXCEPTION_DEBUG_INFO {
    pub ExceptionRecord: EXCEPTION_RECORD,
    pub dwFirstChance: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EXCEPTION_DEBUG_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EXCEPTION_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct EXCEPTION_POINTERS {
    pub ExceptionRecord: *mut EXCEPTION_RECORD,
    pub ContextRecord: *mut CONTEXT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for EXCEPTION_POINTERS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for EXCEPTION_POINTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EXCEPTION_RECORD {
    pub ExceptionCode: super::super::super::Foundation::NTSTATUS,
    pub ExceptionFlags: u32,
    pub ExceptionRecord: *mut EXCEPTION_RECORD,
    pub ExceptionAddress: *mut ::core::ffi::c_void,
    pub NumberParameters: u32,
    pub ExceptionInformation: [usize; 15],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EXCEPTION_RECORD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EXCEPTION_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EXCEPTION_RECORD32 {
    pub ExceptionCode: super::super::super::Foundation::NTSTATUS,
    pub ExceptionFlags: u32,
    pub ExceptionRecord: u32,
    pub ExceptionAddress: u32,
    pub NumberParameters: u32,
    pub ExceptionInformation: [u32; 15],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EXCEPTION_RECORD32 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EXCEPTION_RECORD32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct EXCEPTION_RECORD64 {
    pub ExceptionCode: super::super::super::Foundation::NTSTATUS,
    pub ExceptionFlags: u32,
    pub ExceptionRecord: u64,
    pub ExceptionAddress: u64,
    pub NumberParameters: u32,
    pub __unusedAlignment: u32,
    pub ExceptionInformation: [u64; 15],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for EXCEPTION_RECORD64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for EXCEPTION_RECORD64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXIT_PROCESS_DEBUG_INFO {
    pub dwExitCode: u32,
}
impl ::core::marker::Copy for EXIT_PROCESS_DEBUG_INFO {}
impl ::core::clone::Clone for EXIT_PROCESS_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct EXIT_THREAD_DEBUG_INFO {
    pub dwExitCode: u32,
}
impl ::core::marker::Copy for EXIT_THREAD_DEBUG_INFO {}
impl ::core::clone::Clone for EXIT_THREAD_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Com_StructuredStorage\"`, `\"Win32_System_Ole\"`, `\"Win32_System_Variant\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
pub struct ExtendedDebugPropertyInfo {
    pub dwValidFields: u32,
    pub pszName: ::windows_sys::core::PWSTR,
    pub pszType: ::windows_sys::core::PWSTR,
    pub pszValue: ::windows_sys::core::PWSTR,
    pub pszFullName: ::windows_sys::core::PWSTR,
    pub dwAttrib: u32,
    pub pDebugProp: IDebugProperty,
    pub nDISPID: u32,
    pub nType: u32,
    pub varValue: super::super::Variant::VARIANT,
    pub plbValue: super::super::Com::StructuredStorage::ILockBytes,
    pub pDebugExtProp: IDebugExtendedProperty,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::marker::Copy for ExtendedDebugPropertyInfo {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Com_StructuredStorage", feature = "Win32_System_Ole", feature = "Win32_System_Variant"))]
impl ::core::clone::Clone for ExtendedDebugPropertyInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FPO_DATA {
    pub ulOffStart: u32,
    pub cbProcSize: u32,
    pub cdwLocals: u32,
    pub cdwParams: u16,
    pub _bitfield: u16,
}
impl ::core::marker::Copy for FPO_DATA {}
impl ::core::clone::Clone for FPO_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_CBA_EVENT {
    pub severity: IMAGEHLP_CBA_EVENT_SEVERITY,
    pub code: u32,
    pub desc: ::windows_sys::core::PSTR,
    pub object: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for IMAGEHLP_CBA_EVENT {}
impl ::core::clone::Clone for IMAGEHLP_CBA_EVENT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_CBA_EVENTW {
    pub severity: IMAGEHLP_CBA_EVENT_SEVERITY,
    pub code: u32,
    pub desc: ::windows_sys::core::PCWSTR,
    pub object: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for IMAGEHLP_CBA_EVENTW {}
impl ::core::clone::Clone for IMAGEHLP_CBA_EVENTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_CBA_READ_MEMORY {
    pub addr: u64,
    pub buf: *mut ::core::ffi::c_void,
    pub bytes: u32,
    pub bytesread: *mut u32,
}
impl ::core::marker::Copy for IMAGEHLP_CBA_READ_MEMORY {}
impl ::core::clone::Clone for IMAGEHLP_CBA_READ_MEMORY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_DEFERRED_SYMBOL_LOAD {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u32,
    pub CheckSum: u32,
    pub TimeDateStamp: u32,
    pub FileName: [u8; 260],
    pub Reparse: super::super::super::Foundation::BOOLEAN,
    pub hFile: super::super::super::Foundation::HANDLE,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_DEFERRED_SYMBOL_LOAD {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_DEFERRED_SYMBOL_LOAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_DEFERRED_SYMBOL_LOAD64 {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u64,
    pub CheckSum: u32,
    pub TimeDateStamp: u32,
    pub FileName: [u8; 260],
    pub Reparse: super::super::super::Foundation::BOOLEAN,
    pub hFile: super::super::super::Foundation::HANDLE,
    pub Flags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_DEFERRED_SYMBOL_LOAD64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_DEFERRED_SYMBOL_LOAD64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_DEFERRED_SYMBOL_LOADW64 {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u64,
    pub CheckSum: u32,
    pub TimeDateStamp: u32,
    pub FileName: [u16; 261],
    pub Reparse: super::super::super::Foundation::BOOLEAN,
    pub hFile: super::super::super::Foundation::HANDLE,
    pub Flags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_DEFERRED_SYMBOL_LOADW64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_DEFERRED_SYMBOL_LOADW64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_DUPLICATE_SYMBOL {
    pub SizeOfStruct: u32,
    pub NumberOfDups: u32,
    pub Symbol: *mut IMAGEHLP_SYMBOL,
    pub SelectedSymbol: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_DUPLICATE_SYMBOL {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_DUPLICATE_SYMBOL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_DUPLICATE_SYMBOL64 {
    pub SizeOfStruct: u32,
    pub NumberOfDups: u32,
    pub Symbol: *mut IMAGEHLP_SYMBOL64,
    pub SelectedSymbol: u32,
}
impl ::core::marker::Copy for IMAGEHLP_DUPLICATE_SYMBOL64 {}
impl ::core::clone::Clone for IMAGEHLP_DUPLICATE_SYMBOL64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_GET_TYPE_INFO_PARAMS {
    pub SizeOfStruct: u32,
    pub Flags: IMAGEHLP_GET_TYPE_INFO_FLAGS,
    pub NumIds: u32,
    pub TypeIds: *mut u32,
    pub TagFilter: u64,
    pub NumReqs: u32,
    pub ReqKinds: *mut IMAGEHLP_SYMBOL_TYPE_INFO,
    pub ReqOffsets: *mut usize,
    pub ReqSizes: *mut u32,
    pub ReqStride: usize,
    pub BufferSize: usize,
    pub Buffer: *mut ::core::ffi::c_void,
    pub EntriesMatched: u32,
    pub EntriesFilled: u32,
    pub TagsFound: u64,
    pub AllReqsValid: u64,
    pub NumReqsValid: u32,
    pub ReqsValid: *mut u64,
}
impl ::core::marker::Copy for IMAGEHLP_GET_TYPE_INFO_PARAMS {}
impl ::core::clone::Clone for IMAGEHLP_GET_TYPE_INFO_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_JIT_SYMBOLMAP {
    pub SizeOfStruct: u32,
    pub Address: u64,
    pub BaseOfImage: u64,
}
impl ::core::marker::Copy for IMAGEHLP_JIT_SYMBOLMAP {}
impl ::core::clone::Clone for IMAGEHLP_JIT_SYMBOLMAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_LINE {
    pub SizeOfStruct: u32,
    pub Key: *mut ::core::ffi::c_void,
    pub LineNumber: u32,
    pub FileName: ::windows_sys::core::PSTR,
    pub Address: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_LINE {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_LINE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_LINE64 {
    pub SizeOfStruct: u32,
    pub Key: *mut ::core::ffi::c_void,
    pub LineNumber: u32,
    pub FileName: ::windows_sys::core::PSTR,
    pub Address: u64,
}
impl ::core::marker::Copy for IMAGEHLP_LINE64 {}
impl ::core::clone::Clone for IMAGEHLP_LINE64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_LINEW {
    pub SizeOfStruct: u32,
    pub Key: *mut ::core::ffi::c_void,
    pub LineNumber: u32,
    pub FileName: ::windows_sys::core::PSTR,
    pub Address: u64,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_LINEW {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_LINEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_LINEW64 {
    pub SizeOfStruct: u32,
    pub Key: *mut ::core::ffi::c_void,
    pub LineNumber: u32,
    pub FileName: ::windows_sys::core::PWSTR,
    pub Address: u64,
}
impl ::core::marker::Copy for IMAGEHLP_LINEW64 {}
impl ::core::clone::Clone for IMAGEHLP_LINEW64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_MODULE {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u32,
    pub ImageSize: u32,
    pub TimeDateStamp: u32,
    pub CheckSum: u32,
    pub NumSyms: u32,
    pub SymType: SYM_TYPE,
    pub ModuleName: [u8; 32],
    pub ImageName: [u8; 256],
    pub LoadedImageName: [u8; 256],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_MODULE {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_MODULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_MODULE64 {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u64,
    pub ImageSize: u32,
    pub TimeDateStamp: u32,
    pub CheckSum: u32,
    pub NumSyms: u32,
    pub SymType: SYM_TYPE,
    pub ModuleName: [u8; 32],
    pub ImageName: [u8; 256],
    pub LoadedImageName: [u8; 256],
    pub LoadedPdbName: [u8; 256],
    pub CVSig: u32,
    pub CVData: [u8; 780],
    pub PdbSig: u32,
    pub PdbSig70: ::windows_sys::core::GUID,
    pub PdbAge: u32,
    pub PdbUnmatched: super::super::super::Foundation::BOOL,
    pub DbgUnmatched: super::super::super::Foundation::BOOL,
    pub LineNumbers: super::super::super::Foundation::BOOL,
    pub GlobalSymbols: super::super::super::Foundation::BOOL,
    pub TypeInfo: super::super::super::Foundation::BOOL,
    pub SourceIndexed: super::super::super::Foundation::BOOL,
    pub Publics: super::super::super::Foundation::BOOL,
    pub MachineType: u32,
    pub Reserved: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_MODULE64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_MODULE64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_MODULE64_EX {
    pub Module: IMAGEHLP_MODULE64,
    pub RegionFlags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_MODULE64_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_MODULE64_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_MODULEW {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u32,
    pub ImageSize: u32,
    pub TimeDateStamp: u32,
    pub CheckSum: u32,
    pub NumSyms: u32,
    pub SymType: SYM_TYPE,
    pub ModuleName: [u16; 32],
    pub ImageName: [u16; 256],
    pub LoadedImageName: [u16; 256],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_MODULEW {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_MODULEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_MODULEW64 {
    pub SizeOfStruct: u32,
    pub BaseOfImage: u64,
    pub ImageSize: u32,
    pub TimeDateStamp: u32,
    pub CheckSum: u32,
    pub NumSyms: u32,
    pub SymType: SYM_TYPE,
    pub ModuleName: [u16; 32],
    pub ImageName: [u16; 256],
    pub LoadedImageName: [u16; 256],
    pub LoadedPdbName: [u16; 256],
    pub CVSig: u32,
    pub CVData: [u16; 780],
    pub PdbSig: u32,
    pub PdbSig70: ::windows_sys::core::GUID,
    pub PdbAge: u32,
    pub PdbUnmatched: super::super::super::Foundation::BOOL,
    pub DbgUnmatched: super::super::super::Foundation::BOOL,
    pub LineNumbers: super::super::super::Foundation::BOOL,
    pub GlobalSymbols: super::super::super::Foundation::BOOL,
    pub TypeInfo: super::super::super::Foundation::BOOL,
    pub SourceIndexed: super::super::super::Foundation::BOOL,
    pub Publics: super::super::super::Foundation::BOOL,
    pub MachineType: u32,
    pub Reserved: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_MODULEW64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_MODULEW64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_MODULEW64_EX {
    pub Module: IMAGEHLP_MODULEW64,
    pub RegionFlags: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_MODULEW64_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_MODULEW64_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct IMAGEHLP_STACK_FRAME {
    pub InstructionOffset: u64,
    pub ReturnOffset: u64,
    pub FrameOffset: u64,
    pub StackOffset: u64,
    pub BackingStoreOffset: u64,
    pub FuncTableEntry: u64,
    pub Params: [u64; 4],
    pub Reserved: [u64; 5],
    pub Virtual: super::super::super::Foundation::BOOL,
    pub Reserved2: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for IMAGEHLP_STACK_FRAME {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for IMAGEHLP_STACK_FRAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_SYMBOL {
    pub SizeOfStruct: u32,
    pub Address: u32,
    pub Size: u32,
    pub Flags: u32,
    pub MaxNameLength: u32,
    pub Name: [u8; 1],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_SYMBOL {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_SYMBOL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_SYMBOL64 {
    pub SizeOfStruct: u32,
    pub Address: u64,
    pub Size: u32,
    pub Flags: u32,
    pub MaxNameLength: u32,
    pub Name: [u8; 1],
}
impl ::core::marker::Copy for IMAGEHLP_SYMBOL64 {}
impl ::core::clone::Clone for IMAGEHLP_SYMBOL64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_SYMBOL64_PACKAGE {
    pub sym: IMAGEHLP_SYMBOL64,
    pub name: [u8; 2001],
}
impl ::core::marker::Copy for IMAGEHLP_SYMBOL64_PACKAGE {}
impl ::core::clone::Clone for IMAGEHLP_SYMBOL64_PACKAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_SYMBOLW {
    pub SizeOfStruct: u32,
    pub Address: u32,
    pub Size: u32,
    pub Flags: u32,
    pub MaxNameLength: u32,
    pub Name: [u16; 1],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_SYMBOLW {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_SYMBOLW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_SYMBOLW64 {
    pub SizeOfStruct: u32,
    pub Address: u64,
    pub Size: u32,
    pub Flags: u32,
    pub MaxNameLength: u32,
    pub Name: [u16; 1],
}
impl ::core::marker::Copy for IMAGEHLP_SYMBOLW64 {}
impl ::core::clone::Clone for IMAGEHLP_SYMBOLW64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_SYMBOLW64_PACKAGE {
    pub sym: IMAGEHLP_SYMBOLW64,
    pub name: [u16; 2001],
}
impl ::core::marker::Copy for IMAGEHLP_SYMBOLW64_PACKAGE {}
impl ::core::clone::Clone for IMAGEHLP_SYMBOLW64_PACKAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_SYMBOLW_PACKAGE {
    pub sym: IMAGEHLP_SYMBOLW,
    pub name: [u16; 2001],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_SYMBOLW_PACKAGE {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_SYMBOLW_PACKAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct IMAGEHLP_SYMBOL_PACKAGE {
    pub sym: IMAGEHLP_SYMBOL,
    pub name: [u8; 2001],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for IMAGEHLP_SYMBOL_PACKAGE {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for IMAGEHLP_SYMBOL_PACKAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGEHLP_SYMBOL_SRC {
    pub sizeofstruct: u32,
    pub r#type: u32,
    pub file: [u8; 260],
}
impl ::core::marker::Copy for IMAGEHLP_SYMBOL_SRC {}
impl ::core::clone::Clone for IMAGEHLP_SYMBOL_SRC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY {
    pub BeginAddress: u32,
    pub Anonymous: IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0,
}
impl ::core::marker::Copy for IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY {}
impl ::core::clone::Clone for IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0 {
    pub UnwindData: u32,
    pub Anonymous: IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0_0,
}
impl ::core::marker::Copy for IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0 {}
impl ::core::clone::Clone for IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0_0 {}
impl ::core::clone::Clone for IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_COFF_SYMBOLS_HEADER {
    pub NumberOfSymbols: u32,
    pub LvaToFirstSymbol: u32,
    pub NumberOfLinenumbers: u32,
    pub LvaToFirstLinenumber: u32,
    pub RvaToFirstByteOfCode: u32,
    pub RvaToLastByteOfCode: u32,
    pub RvaToFirstByteOfData: u32,
    pub RvaToLastByteOfData: u32,
}
impl ::core::marker::Copy for IMAGE_COFF_SYMBOLS_HEADER {}
impl ::core::clone::Clone for IMAGE_COFF_SYMBOLS_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_COR20_HEADER {
    pub cb: u32,
    pub MajorRuntimeVersion: u16,
    pub MinorRuntimeVersion: u16,
    pub MetaData: IMAGE_DATA_DIRECTORY,
    pub Flags: u32,
    pub Anonymous: IMAGE_COR20_HEADER_0,
    pub Resources: IMAGE_DATA_DIRECTORY,
    pub StrongNameSignature: IMAGE_DATA_DIRECTORY,
    pub CodeManagerTable: IMAGE_DATA_DIRECTORY,
    pub VTableFixups: IMAGE_DATA_DIRECTORY,
    pub ExportAddressTableJumps: IMAGE_DATA_DIRECTORY,
    pub ManagedNativeHeader: IMAGE_DATA_DIRECTORY,
}
impl ::core::marker::Copy for IMAGE_COR20_HEADER {}
impl ::core::clone::Clone for IMAGE_COR20_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IMAGE_COR20_HEADER_0 {
    pub EntryPointToken: u32,
    pub EntryPointRVA: u32,
}
impl ::core::marker::Copy for IMAGE_COR20_HEADER_0 {}
impl ::core::clone::Clone for IMAGE_COR20_HEADER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_DATA_DIRECTORY {
    pub VirtualAddress: u32,
    pub Size: u32,
}
impl ::core::marker::Copy for IMAGE_DATA_DIRECTORY {}
impl ::core::clone::Clone for IMAGE_DATA_DIRECTORY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_DEBUG_DIRECTORY {
    pub Characteristics: u32,
    pub TimeDateStamp: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub Type: IMAGE_DEBUG_TYPE,
    pub SizeOfData: u32,
    pub AddressOfRawData: u32,
    pub PointerToRawData: u32,
}
impl ::core::marker::Copy for IMAGE_DEBUG_DIRECTORY {}
impl ::core::clone::Clone for IMAGE_DEBUG_DIRECTORY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct IMAGE_DEBUG_INFORMATION {
    pub List: super::super::Kernel::LIST_ENTRY,
    pub ReservedSize: u32,
    pub ReservedMappedBase: *mut ::core::ffi::c_void,
    pub ReservedMachine: u16,
    pub ReservedCharacteristics: u16,
    pub ReservedCheckSum: u32,
    pub ImageBase: u32,
    pub SizeOfImage: u32,
    pub ReservedNumberOfSections: u32,
    pub ReservedSections: *mut IMAGE_SECTION_HEADER,
    pub ReservedExportedNamesSize: u32,
    pub ReservedExportedNames: ::windows_sys::core::PSTR,
    pub ReservedNumberOfFunctionTableEntries: u32,
    pub ReservedFunctionTableEntries: *mut IMAGE_FUNCTION_ENTRY,
    pub ReservedLowestFunctionStartingAddress: u32,
    pub ReservedHighestFunctionEndingAddress: u32,
    pub ReservedNumberOfFpoTableEntries: u32,
    pub ReservedFpoTableEntries: *mut FPO_DATA,
    pub SizeOfCoffSymbols: u32,
    pub CoffSymbols: *mut IMAGE_COFF_SYMBOLS_HEADER,
    pub ReservedSizeOfCodeViewSymbols: u32,
    pub ReservedCodeViewSymbols: *mut ::core::ffi::c_void,
    pub ImageFilePath: ::windows_sys::core::PSTR,
    pub ImageFileName: ::windows_sys::core::PSTR,
    pub ReservedDebugFilePath: ::windows_sys::core::PSTR,
    pub ReservedTimeDateStamp: u32,
    pub ReservedRomImage: super::super::super::Foundation::BOOL,
    pub ReservedDebugDirectory: *mut IMAGE_DEBUG_DIRECTORY,
    pub ReservedNumberOfDebugDirectories: u32,
    pub ReservedOriginalFunctionTableBaseAddress: u32,
    pub Reserved: [u32; 2],
}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for IMAGE_DEBUG_INFORMATION {}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for IMAGE_DEBUG_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct IMAGE_FILE_HEADER {
    pub Machine: super::super::SystemInformation::IMAGE_FILE_MACHINE,
    pub NumberOfSections: u16,
    pub TimeDateStamp: u32,
    pub PointerToSymbolTable: u32,
    pub NumberOfSymbols: u32,
    pub SizeOfOptionalHeader: u16,
    pub Characteristics: IMAGE_FILE_CHARACTERISTICS,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for IMAGE_FILE_HEADER {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for IMAGE_FILE_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_FUNCTION_ENTRY {
    pub StartingAddress: u32,
    pub EndingAddress: u32,
    pub EndOfPrologue: u32,
}
impl ::core::marker::Copy for IMAGE_FUNCTION_ENTRY {}
impl ::core::clone::Clone for IMAGE_FUNCTION_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct IMAGE_FUNCTION_ENTRY64 {
    pub StartingAddress: u64,
    pub EndingAddress: u64,
    pub Anonymous: IMAGE_FUNCTION_ENTRY64_0,
}
impl ::core::marker::Copy for IMAGE_FUNCTION_ENTRY64 {}
impl ::core::clone::Clone for IMAGE_FUNCTION_ENTRY64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub union IMAGE_FUNCTION_ENTRY64_0 {
    pub EndOfPrologue: u64,
    pub UnwindInfoAddress: u64,
}
impl ::core::marker::Copy for IMAGE_FUNCTION_ENTRY64_0 {}
impl ::core::clone::Clone for IMAGE_FUNCTION_ENTRY64_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_LOAD_CONFIG_CODE_INTEGRITY {
    pub Flags: u16,
    pub Catalog: u16,
    pub CatalogOffset: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for IMAGE_LOAD_CONFIG_CODE_INTEGRITY {}
impl ::core::clone::Clone for IMAGE_LOAD_CONFIG_CODE_INTEGRITY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_LOAD_CONFIG_DIRECTORY32 {
    pub Size: u32,
    pub TimeDateStamp: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub GlobalFlagsClear: u32,
    pub GlobalFlagsSet: u32,
    pub CriticalSectionDefaultTimeout: u32,
    pub DeCommitFreeBlockThreshold: u32,
    pub DeCommitTotalFreeThreshold: u32,
    pub LockPrefixTable: u32,
    pub MaximumAllocationSize: u32,
    pub VirtualMemoryThreshold: u32,
    pub ProcessHeapFlags: u32,
    pub ProcessAffinityMask: u32,
    pub CSDVersion: u16,
    pub DependentLoadFlags: u16,
    pub EditList: u32,
    pub SecurityCookie: u32,
    pub SEHandlerTable: u32,
    pub SEHandlerCount: u32,
    pub GuardCFCheckFunctionPointer: u32,
    pub GuardCFDispatchFunctionPointer: u32,
    pub GuardCFFunctionTable: u32,
    pub GuardCFFunctionCount: u32,
    pub GuardFlags: u32,
    pub CodeIntegrity: IMAGE_LOAD_CONFIG_CODE_INTEGRITY,
    pub GuardAddressTakenIatEntryTable: u32,
    pub GuardAddressTakenIatEntryCount: u32,
    pub GuardLongJumpTargetTable: u32,
    pub GuardLongJumpTargetCount: u32,
    pub DynamicValueRelocTable: u32,
    pub CHPEMetadataPointer: u32,
    pub GuardRFFailureRoutine: u32,
    pub GuardRFFailureRoutineFunctionPointer: u32,
    pub DynamicValueRelocTableOffset: u32,
    pub DynamicValueRelocTableSection: u16,
    pub Reserved2: u16,
    pub GuardRFVerifyStackPointerFunctionPointer: u32,
    pub HotPatchTableOffset: u32,
    pub Reserved3: u32,
    pub EnclaveConfigurationPointer: u32,
    pub VolatileMetadataPointer: u32,
    pub GuardEHContinuationTable: u32,
    pub GuardEHContinuationCount: u32,
    pub GuardXFGCheckFunctionPointer: u32,
    pub GuardXFGDispatchFunctionPointer: u32,
    pub GuardXFGTableDispatchFunctionPointer: u32,
    pub CastGuardOsDeterminedFailureMode: u32,
    pub GuardMemcpyFunctionPointer: u32,
}
impl ::core::marker::Copy for IMAGE_LOAD_CONFIG_DIRECTORY32 {}
impl ::core::clone::Clone for IMAGE_LOAD_CONFIG_DIRECTORY32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct IMAGE_LOAD_CONFIG_DIRECTORY64 {
    pub Size: u32,
    pub TimeDateStamp: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub GlobalFlagsClear: u32,
    pub GlobalFlagsSet: u32,
    pub CriticalSectionDefaultTimeout: u32,
    pub DeCommitFreeBlockThreshold: u64,
    pub DeCommitTotalFreeThreshold: u64,
    pub LockPrefixTable: u64,
    pub MaximumAllocationSize: u64,
    pub VirtualMemoryThreshold: u64,
    pub ProcessAffinityMask: u64,
    pub ProcessHeapFlags: u32,
    pub CSDVersion: u16,
    pub DependentLoadFlags: u16,
    pub EditList: u64,
    pub SecurityCookie: u64,
    pub SEHandlerTable: u64,
    pub SEHandlerCount: u64,
    pub GuardCFCheckFunctionPointer: u64,
    pub GuardCFDispatchFunctionPointer: u64,
    pub GuardCFFunctionTable: u64,
    pub GuardCFFunctionCount: u64,
    pub GuardFlags: u32,
    pub CodeIntegrity: IMAGE_LOAD_CONFIG_CODE_INTEGRITY,
    pub GuardAddressTakenIatEntryTable: u64,
    pub GuardAddressTakenIatEntryCount: u64,
    pub GuardLongJumpTargetTable: u64,
    pub GuardLongJumpTargetCount: u64,
    pub DynamicValueRelocTable: u64,
    pub CHPEMetadataPointer: u64,
    pub GuardRFFailureRoutine: u64,
    pub GuardRFFailureRoutineFunctionPointer: u64,
    pub DynamicValueRelocTableOffset: u32,
    pub DynamicValueRelocTableSection: u16,
    pub Reserved2: u16,
    pub GuardRFVerifyStackPointerFunctionPointer: u64,
    pub HotPatchTableOffset: u32,
    pub Reserved3: u32,
    pub EnclaveConfigurationPointer: u64,
    pub VolatileMetadataPointer: u64,
    pub GuardEHContinuationTable: u64,
    pub GuardEHContinuationCount: u64,
    pub GuardXFGCheckFunctionPointer: u64,
    pub GuardXFGDispatchFunctionPointer: u64,
    pub GuardXFGTableDispatchFunctionPointer: u64,
    pub CastGuardOsDeterminedFailureMode: u64,
    pub GuardMemcpyFunctionPointer: u64,
}
impl ::core::marker::Copy for IMAGE_LOAD_CONFIG_DIRECTORY64 {}
impl ::core::clone::Clone for IMAGE_LOAD_CONFIG_DIRECTORY64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct IMAGE_NT_HEADERS32 {
    pub Signature: u32,
    pub FileHeader: IMAGE_FILE_HEADER,
    pub OptionalHeader: IMAGE_OPTIONAL_HEADER32,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for IMAGE_NT_HEADERS32 {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for IMAGE_NT_HEADERS32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct IMAGE_NT_HEADERS64 {
    pub Signature: u32,
    pub FileHeader: IMAGE_FILE_HEADER,
    pub OptionalHeader: IMAGE_OPTIONAL_HEADER64,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for IMAGE_NT_HEADERS64 {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for IMAGE_NT_HEADERS64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_OPTIONAL_HEADER32 {
    pub Magic: IMAGE_OPTIONAL_HEADER_MAGIC,
    pub MajorLinkerVersion: u8,
    pub MinorLinkerVersion: u8,
    pub SizeOfCode: u32,
    pub SizeOfInitializedData: u32,
    pub SizeOfUninitializedData: u32,
    pub AddressOfEntryPoint: u32,
    pub BaseOfCode: u32,
    pub BaseOfData: u32,
    pub ImageBase: u32,
    pub SectionAlignment: u32,
    pub FileAlignment: u32,
    pub MajorOperatingSystemVersion: u16,
    pub MinorOperatingSystemVersion: u16,
    pub MajorImageVersion: u16,
    pub MinorImageVersion: u16,
    pub MajorSubsystemVersion: u16,
    pub MinorSubsystemVersion: u16,
    pub Win32VersionValue: u32,
    pub SizeOfImage: u32,
    pub SizeOfHeaders: u32,
    pub CheckSum: u32,
    pub Subsystem: IMAGE_SUBSYSTEM,
    pub DllCharacteristics: IMAGE_DLL_CHARACTERISTICS,
    pub SizeOfStackReserve: u32,
    pub SizeOfStackCommit: u32,
    pub SizeOfHeapReserve: u32,
    pub SizeOfHeapCommit: u32,
    pub LoaderFlags: u32,
    pub NumberOfRvaAndSizes: u32,
    pub DataDirectory: [IMAGE_DATA_DIRECTORY; 16],
}
impl ::core::marker::Copy for IMAGE_OPTIONAL_HEADER32 {}
impl ::core::clone::Clone for IMAGE_OPTIONAL_HEADER32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct IMAGE_OPTIONAL_HEADER64 {
    pub Magic: IMAGE_OPTIONAL_HEADER_MAGIC,
    pub MajorLinkerVersion: u8,
    pub MinorLinkerVersion: u8,
    pub SizeOfCode: u32,
    pub SizeOfInitializedData: u32,
    pub SizeOfUninitializedData: u32,
    pub AddressOfEntryPoint: u32,
    pub BaseOfCode: u32,
    pub ImageBase: u64,
    pub SectionAlignment: u32,
    pub FileAlignment: u32,
    pub MajorOperatingSystemVersion: u16,
    pub MinorOperatingSystemVersion: u16,
    pub MajorImageVersion: u16,
    pub MinorImageVersion: u16,
    pub MajorSubsystemVersion: u16,
    pub MinorSubsystemVersion: u16,
    pub Win32VersionValue: u32,
    pub SizeOfImage: u32,
    pub SizeOfHeaders: u32,
    pub CheckSum: u32,
    pub Subsystem: IMAGE_SUBSYSTEM,
    pub DllCharacteristics: IMAGE_DLL_CHARACTERISTICS,
    pub SizeOfStackReserve: u64,
    pub SizeOfStackCommit: u64,
    pub SizeOfHeapReserve: u64,
    pub SizeOfHeapCommit: u64,
    pub LoaderFlags: u32,
    pub NumberOfRvaAndSizes: u32,
    pub DataDirectory: [IMAGE_DATA_DIRECTORY; 16],
}
impl ::core::marker::Copy for IMAGE_OPTIONAL_HEADER64 {}
impl ::core::clone::Clone for IMAGE_OPTIONAL_HEADER64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct IMAGE_ROM_HEADERS {
    pub FileHeader: IMAGE_FILE_HEADER,
    pub OptionalHeader: IMAGE_ROM_OPTIONAL_HEADER,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for IMAGE_ROM_HEADERS {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for IMAGE_ROM_HEADERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_ROM_OPTIONAL_HEADER {
    pub Magic: u16,
    pub MajorLinkerVersion: u8,
    pub MinorLinkerVersion: u8,
    pub SizeOfCode: u32,
    pub SizeOfInitializedData: u32,
    pub SizeOfUninitializedData: u32,
    pub AddressOfEntryPoint: u32,
    pub BaseOfCode: u32,
    pub BaseOfData: u32,
    pub BaseOfBss: u32,
    pub GprMask: u32,
    pub CprMask: [u32; 4],
    pub GpValue: u32,
}
impl ::core::marker::Copy for IMAGE_ROM_OPTIONAL_HEADER {}
impl ::core::clone::Clone for IMAGE_ROM_OPTIONAL_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_RUNTIME_FUNCTION_ENTRY {
    pub BeginAddress: u32,
    pub EndAddress: u32,
    pub Anonymous: IMAGE_RUNTIME_FUNCTION_ENTRY_0,
}
impl ::core::marker::Copy for IMAGE_RUNTIME_FUNCTION_ENTRY {}
impl ::core::clone::Clone for IMAGE_RUNTIME_FUNCTION_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IMAGE_RUNTIME_FUNCTION_ENTRY_0 {
    pub UnwindInfoAddress: u32,
    pub UnwindData: u32,
}
impl ::core::marker::Copy for IMAGE_RUNTIME_FUNCTION_ENTRY_0 {}
impl ::core::clone::Clone for IMAGE_RUNTIME_FUNCTION_ENTRY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IMAGE_SECTION_HEADER {
    pub Name: [u8; 8],
    pub Misc: IMAGE_SECTION_HEADER_0,
    pub VirtualAddress: u32,
    pub SizeOfRawData: u32,
    pub PointerToRawData: u32,
    pub PointerToRelocations: u32,
    pub PointerToLinenumbers: u32,
    pub NumberOfRelocations: u16,
    pub NumberOfLinenumbers: u16,
    pub Characteristics: IMAGE_SECTION_CHARACTERISTICS,
}
impl ::core::marker::Copy for IMAGE_SECTION_HEADER {}
impl ::core::clone::Clone for IMAGE_SECTION_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union IMAGE_SECTION_HEADER_0 {
    pub PhysicalAddress: u32,
    pub VirtualSize: u32,
}
impl ::core::marker::Copy for IMAGE_SECTION_HEADER_0 {}
impl ::core::clone::Clone for IMAGE_SECTION_HEADER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IPMI_OS_SEL_RECORD {
    pub Signature: u32,
    pub Version: u32,
    pub Length: u32,
    pub RecordType: IPMI_OS_SEL_RECORD_TYPE,
    pub DataLength: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for IPMI_OS_SEL_RECORD {}
impl ::core::clone::Clone for IPMI_OS_SEL_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct KDHELP {
    pub Thread: u32,
    pub ThCallbackStack: u32,
    pub NextCallback: u32,
    pub FramePointer: u32,
    pub KiCallUserMode: u32,
    pub KeUserCallbackDispatcher: u32,
    pub SystemRangeStart: u32,
    pub ThCallbackBStore: u32,
    pub KiUserExceptionDispatcher: u32,
    pub StackBase: u32,
    pub StackLimit: u32,
    pub Reserved: [u32; 5],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for KDHELP {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for KDHELP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct KDHELP64 {
    pub Thread: u64,
    pub ThCallbackStack: u32,
    pub ThCallbackBStore: u32,
    pub NextCallback: u32,
    pub FramePointer: u32,
    pub KiCallUserMode: u64,
    pub KeUserCallbackDispatcher: u64,
    pub SystemRangeStart: u64,
    pub KiUserExceptionDispatcher: u64,
    pub StackBase: u64,
    pub StackLimit: u64,
    pub BuildVersion: u32,
    pub RetpolineStubFunctionTableSize: u32,
    pub RetpolineStubFunctionTable: u64,
    pub RetpolineStubOffset: u32,
    pub RetpolineStubSize: u32,
    pub Reserved0: [u64; 2],
}
impl ::core::marker::Copy for KDHELP64 {}
impl ::core::clone::Clone for KDHELP64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct KNONVOLATILE_CONTEXT_POINTERS {
    pub Anonymous1: KNONVOLATILE_CONTEXT_POINTERS_0,
    pub Anonymous2: KNONVOLATILE_CONTEXT_POINTERS_1,
}
#[cfg(target_arch = "x86_64")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS {}
#[cfg(target_arch = "x86_64")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub union KNONVOLATILE_CONTEXT_POINTERS_0 {
    pub FloatingContext: [*mut M128A; 16],
    pub Anonymous: KNONVOLATILE_CONTEXT_POINTERS_0_0,
}
#[cfg(target_arch = "x86_64")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS_0 {}
#[cfg(target_arch = "x86_64")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct KNONVOLATILE_CONTEXT_POINTERS_0_0 {
    pub Xmm0: *mut M128A,
    pub Xmm1: *mut M128A,
    pub Xmm2: *mut M128A,
    pub Xmm3: *mut M128A,
    pub Xmm4: *mut M128A,
    pub Xmm5: *mut M128A,
    pub Xmm6: *mut M128A,
    pub Xmm7: *mut M128A,
    pub Xmm8: *mut M128A,
    pub Xmm9: *mut M128A,
    pub Xmm10: *mut M128A,
    pub Xmm11: *mut M128A,
    pub Xmm12: *mut M128A,
    pub Xmm13: *mut M128A,
    pub Xmm14: *mut M128A,
    pub Xmm15: *mut M128A,
}
#[cfg(target_arch = "x86_64")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS_0_0 {}
#[cfg(target_arch = "x86_64")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub union KNONVOLATILE_CONTEXT_POINTERS_1 {
    pub IntegerContext: [*mut u64; 16],
    pub Anonymous: KNONVOLATILE_CONTEXT_POINTERS_1_0,
}
#[cfg(target_arch = "x86_64")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS_1 {}
#[cfg(target_arch = "x86_64")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct KNONVOLATILE_CONTEXT_POINTERS_1_0 {
    pub Rax: *mut u64,
    pub Rcx: *mut u64,
    pub Rdx: *mut u64,
    pub Rbx: *mut u64,
    pub Rsp: *mut u64,
    pub Rbp: *mut u64,
    pub Rsi: *mut u64,
    pub Rdi: *mut u64,
    pub R8: *mut u64,
    pub R9: *mut u64,
    pub R10: *mut u64,
    pub R11: *mut u64,
    pub R12: *mut u64,
    pub R13: *mut u64,
    pub R14: *mut u64,
    pub R15: *mut u64,
}
#[cfg(target_arch = "x86_64")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS_1_0 {}
#[cfg(target_arch = "x86_64")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct KNONVOLATILE_CONTEXT_POINTERS {
    pub Dummy: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "aarch64")]
pub struct KNONVOLATILE_CONTEXT_POINTERS_ARM64 {
    pub X19: *mut u64,
    pub X20: *mut u64,
    pub X21: *mut u64,
    pub X22: *mut u64,
    pub X23: *mut u64,
    pub X24: *mut u64,
    pub X25: *mut u64,
    pub X26: *mut u64,
    pub X27: *mut u64,
    pub X28: *mut u64,
    pub Fp: *mut u64,
    pub Lr: *mut u64,
    pub D8: *mut u64,
    pub D9: *mut u64,
    pub D10: *mut u64,
    pub D11: *mut u64,
    pub D12: *mut u64,
    pub D13: *mut u64,
    pub D14: *mut u64,
    pub D15: *mut u64,
}
#[cfg(target_arch = "aarch64")]
impl ::core::marker::Copy for KNONVOLATILE_CONTEXT_POINTERS_ARM64 {}
#[cfg(target_arch = "aarch64")]
impl ::core::clone::Clone for KNONVOLATILE_CONTEXT_POINTERS_ARM64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDT_ENTRY {
    pub LimitLow: u16,
    pub BaseLow: u16,
    pub HighWord: LDT_ENTRY_0,
}
impl ::core::marker::Copy for LDT_ENTRY {}
impl ::core::clone::Clone for LDT_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union LDT_ENTRY_0 {
    pub Bytes: LDT_ENTRY_0_1,
    pub Bits: LDT_ENTRY_0_0,
}
impl ::core::marker::Copy for LDT_ENTRY_0 {}
impl ::core::clone::Clone for LDT_ENTRY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDT_ENTRY_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for LDT_ENTRY_0_0 {}
impl ::core::clone::Clone for LDT_ENTRY_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct LDT_ENTRY_0_1 {
    pub BaseMid: u8,
    pub Flags1: u8,
    pub Flags2: u8,
    pub BaseHi: u8,
}
impl ::core::marker::Copy for LDT_ENTRY_0_1 {}
impl ::core::clone::Clone for LDT_ENTRY_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
pub struct LOADED_IMAGE {
    pub ModuleName: ::windows_sys::core::PSTR,
    pub hFile: super::super::super::Foundation::HANDLE,
    pub MappedAddress: *mut u8,
    pub FileHeader: *mut IMAGE_NT_HEADERS64,
    pub LastRvaSection: *mut IMAGE_SECTION_HEADER,
    pub NumberOfSections: u32,
    pub Sections: *mut IMAGE_SECTION_HEADER,
    pub Characteristics: IMAGE_FILE_CHARACTERISTICS2,
    pub fSystemImage: super::super::super::Foundation::BOOLEAN,
    pub fDOSImage: super::super::super::Foundation::BOOLEAN,
    pub fReadOnly: super::super::super::Foundation::BOOLEAN,
    pub Version: u8,
    pub Links: super::super::Kernel::LIST_ENTRY,
    pub SizeOfImage: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
impl ::core::marker::Copy for LOADED_IMAGE {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
impl ::core::clone::Clone for LOADED_IMAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_SystemInformation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
pub struct LOADED_IMAGE {
    pub ModuleName: ::windows_sys::core::PSTR,
    pub hFile: super::super::super::Foundation::HANDLE,
    pub MappedAddress: *mut u8,
    pub FileHeader: *mut IMAGE_NT_HEADERS32,
    pub LastRvaSection: *mut IMAGE_SECTION_HEADER,
    pub NumberOfSections: u32,
    pub Sections: *mut IMAGE_SECTION_HEADER,
    pub Characteristics: IMAGE_FILE_CHARACTERISTICS2,
    pub fSystemImage: super::super::super::Foundation::BOOLEAN,
    pub fDOSImage: super::super::super::Foundation::BOOLEAN,
    pub fReadOnly: super::super::super::Foundation::BOOLEAN,
    pub Version: u8,
    pub Links: super::super::Kernel::LIST_ENTRY,
    pub SizeOfImage: u32,
}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
impl ::core::marker::Copy for LOADED_IMAGE {}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel", feature = "Win32_System_SystemInformation"))]
impl ::core::clone::Clone for LOADED_IMAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct LOAD_DLL_DEBUG_INFO {
    pub hFile: super::super::super::Foundation::HANDLE,
    pub lpBaseOfDll: *mut ::core::ffi::c_void,
    pub dwDebugInfoFileOffset: u32,
    pub nDebugInfoSize: u32,
    pub lpImageName: *mut ::core::ffi::c_void,
    pub fUnicode: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for LOAD_DLL_DEBUG_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for LOAD_DLL_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct M128A {
    pub Low: u64,
    pub High: i64,
}
impl ::core::marker::Copy for M128A {}
impl ::core::clone::Clone for M128A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Memory\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_INFORMATION {
    pub CallbackRoutine: MINIDUMP_CALLBACK_ROUTINE,
    pub CallbackParam: *mut ::core::ffi::c_void,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_INFORMATION {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Memory\"`"]
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_INFORMATION {
    pub CallbackRoutine: MINIDUMP_CALLBACK_ROUTINE,
    pub CallbackParam: *mut ::core::ffi::c_void,
}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_INFORMATION {}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_CALLBACK_INPUT {
    pub ProcessId: u32,
    pub ProcessHandle: super::super::super::Foundation::HANDLE,
    pub CallbackType: u32,
    pub Anonymous: MINIDUMP_CALLBACK_INPUT_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_INPUT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel"))]
pub union MINIDUMP_CALLBACK_INPUT_0 {
    pub Status: ::windows_sys::core::HRESULT,
    pub Thread: MINIDUMP_THREAD_CALLBACK,
    pub ThreadEx: MINIDUMP_THREAD_EX_CALLBACK,
    pub Module: MINIDUMP_MODULE_CALLBACK,
    pub IncludeThread: MINIDUMP_INCLUDE_THREAD_CALLBACK,
    pub IncludeModule: MINIDUMP_INCLUDE_MODULE_CALLBACK,
    pub Io: MINIDUMP_IO_CALLBACK,
    pub ReadMemoryFailure: MINIDUMP_READ_MEMORY_FAILURE_CALLBACK,
    pub SecondaryFlags: u32,
    pub VmQuery: MINIDUMP_VM_QUERY_CALLBACK,
    pub VmPreRead: MINIDUMP_VM_PRE_READ_CALLBACK,
    pub VmPostRead: MINIDUMP_VM_POST_READ_CALLBACK,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_INPUT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_INPUT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_OUTPUT {
    pub Anonymous: MINIDUMP_CALLBACK_OUTPUT_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub union MINIDUMP_CALLBACK_OUTPUT_0 {
    pub ModuleWriteFlags: u32,
    pub ThreadWriteFlags: u32,
    pub SecondaryFlags: u32,
    pub Anonymous1: MINIDUMP_CALLBACK_OUTPUT_0_0,
    pub Anonymous2: MINIDUMP_CALLBACK_OUTPUT_0_1,
    pub Handle: super::super::super::Foundation::HANDLE,
    pub Anonymous3: MINIDUMP_CALLBACK_OUTPUT_0_2,
    pub Anonymous4: MINIDUMP_CALLBACK_OUTPUT_0_3,
    pub Anonymous5: MINIDUMP_CALLBACK_OUTPUT_0_4,
    pub Status: ::windows_sys::core::HRESULT,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_OUTPUT_0_0 {
    pub MemoryBase: u64,
    pub MemorySize: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT_0_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_OUTPUT_0_1 {
    pub CheckCancel: super::super::super::Foundation::BOOL,
    pub Cancel: super::super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT_0_1 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_OUTPUT_0_2 {
    pub VmRegion: MINIDUMP_MEMORY_INFO,
    pub Continue: super::super::super::Foundation::BOOL,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT_0_2 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_OUTPUT_0_3 {
    pub VmQueryStatus: ::windows_sys::core::HRESULT,
    pub VmQueryResult: MINIDUMP_MEMORY_INFO,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT_0_3 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
pub struct MINIDUMP_CALLBACK_OUTPUT_0_4 {
    pub VmReadStatus: ::windows_sys::core::HRESULT,
    pub VmReadBytesCompleted: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::marker::Copy for MINIDUMP_CALLBACK_OUTPUT_0_4 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Memory"))]
impl ::core::clone::Clone for MINIDUMP_CALLBACK_OUTPUT_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_DIRECTORY {
    pub StreamType: u32,
    pub Location: MINIDUMP_LOCATION_DESCRIPTOR,
}
impl ::core::marker::Copy for MINIDUMP_DIRECTORY {}
impl ::core::clone::Clone for MINIDUMP_DIRECTORY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_EXCEPTION {
    pub ExceptionCode: u32,
    pub ExceptionFlags: u32,
    pub ExceptionRecord: u64,
    pub ExceptionAddress: u64,
    pub NumberParameters: u32,
    pub __unusedAlignment: u32,
    pub ExceptionInformation: [u64; 15],
}
impl ::core::marker::Copy for MINIDUMP_EXCEPTION {}
impl ::core::clone::Clone for MINIDUMP_EXCEPTION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_EXCEPTION_INFORMATION {
    pub ThreadId: u32,
    pub ExceptionPointers: *mut EXCEPTION_POINTERS,
    pub ClientPointers: super::super::super::Foundation::BOOL,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_EXCEPTION_INFORMATION {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_EXCEPTION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_EXCEPTION_INFORMATION {
    pub ThreadId: u32,
    pub ExceptionPointers: *mut EXCEPTION_POINTERS,
    pub ClientPointers: super::super::super::Foundation::BOOL,
}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_EXCEPTION_INFORMATION {}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_EXCEPTION_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MINIDUMP_EXCEPTION_INFORMATION64 {
    pub ThreadId: u32,
    pub ExceptionRecord: u64,
    pub ContextRecord: u64,
    pub ClientPointers: super::super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MINIDUMP_EXCEPTION_INFORMATION64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MINIDUMP_EXCEPTION_INFORMATION64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_EXCEPTION_STREAM {
    pub ThreadId: u32,
    pub __alignment: u32,
    pub ExceptionRecord: MINIDUMP_EXCEPTION,
    pub ThreadContext: MINIDUMP_LOCATION_DESCRIPTOR,
}
impl ::core::marker::Copy for MINIDUMP_EXCEPTION_STREAM {}
impl ::core::clone::Clone for MINIDUMP_EXCEPTION_STREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_FUNCTION_TABLE_DESCRIPTOR {
    pub MinimumAddress: u64,
    pub MaximumAddress: u64,
    pub BaseAddress: u64,
    pub EntryCount: u32,
    pub SizeOfAlignPad: u32,
}
impl ::core::marker::Copy for MINIDUMP_FUNCTION_TABLE_DESCRIPTOR {}
impl ::core::clone::Clone for MINIDUMP_FUNCTION_TABLE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_FUNCTION_TABLE_STREAM {
    pub SizeOfHeader: u32,
    pub SizeOfDescriptor: u32,
    pub SizeOfNativeDescriptor: u32,
    pub SizeOfFunctionEntry: u32,
    pub NumberOfDescriptors: u32,
    pub SizeOfAlignPad: u32,
}
impl ::core::marker::Copy for MINIDUMP_FUNCTION_TABLE_STREAM {}
impl ::core::clone::Clone for MINIDUMP_FUNCTION_TABLE_STREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_HANDLE_DATA_STREAM {
    pub SizeOfHeader: u32,
    pub SizeOfDescriptor: u32,
    pub NumberOfDescriptors: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for MINIDUMP_HANDLE_DATA_STREAM {}
impl ::core::clone::Clone for MINIDUMP_HANDLE_DATA_STREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_HANDLE_DESCRIPTOR {
    pub Handle: u64,
    pub TypeNameRva: u32,
    pub ObjectNameRva: u32,
    pub Attributes: u32,
    pub GrantedAccess: u32,
    pub HandleCount: u32,
    pub PointerCount: u32,
}
impl ::core::marker::Copy for MINIDUMP_HANDLE_DESCRIPTOR {}
impl ::core::clone::Clone for MINIDUMP_HANDLE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_HANDLE_DESCRIPTOR_2 {
    pub Handle: u64,
    pub TypeNameRva: u32,
    pub ObjectNameRva: u32,
    pub Attributes: u32,
    pub GrantedAccess: u32,
    pub HandleCount: u32,
    pub PointerCount: u32,
    pub ObjectInfoRva: u32,
    pub Reserved0: u32,
}
impl ::core::marker::Copy for MINIDUMP_HANDLE_DESCRIPTOR_2 {}
impl ::core::clone::Clone for MINIDUMP_HANDLE_DESCRIPTOR_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_HANDLE_OBJECT_INFORMATION {
    pub NextInfoRva: u32,
    pub InfoType: u32,
    pub SizeOfInfo: u32,
}
impl ::core::marker::Copy for MINIDUMP_HANDLE_OBJECT_INFORMATION {}
impl ::core::clone::Clone for MINIDUMP_HANDLE_OBJECT_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_HANDLE_OPERATION_LIST {
    pub SizeOfHeader: u32,
    pub SizeOfEntry: u32,
    pub NumberOfEntries: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for MINIDUMP_HANDLE_OPERATION_LIST {}
impl ::core::clone::Clone for MINIDUMP_HANDLE_OPERATION_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_HEADER {
    pub Signature: u32,
    pub Version: u32,
    pub NumberOfStreams: u32,
    pub StreamDirectoryRva: u32,
    pub CheckSum: u32,
    pub Anonymous: MINIDUMP_HEADER_0,
    pub Flags: u64,
}
impl ::core::marker::Copy for MINIDUMP_HEADER {}
impl ::core::clone::Clone for MINIDUMP_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union MINIDUMP_HEADER_0 {
    pub Reserved: u32,
    pub TimeDateStamp: u32,
}
impl ::core::marker::Copy for MINIDUMP_HEADER_0 {}
impl ::core::clone::Clone for MINIDUMP_HEADER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_INCLUDE_MODULE_CALLBACK {
    pub BaseOfImage: u64,
}
impl ::core::marker::Copy for MINIDUMP_INCLUDE_MODULE_CALLBACK {}
impl ::core::clone::Clone for MINIDUMP_INCLUDE_MODULE_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_INCLUDE_THREAD_CALLBACK {
    pub ThreadId: u32,
}
impl ::core::marker::Copy for MINIDUMP_INCLUDE_THREAD_CALLBACK {}
impl ::core::clone::Clone for MINIDUMP_INCLUDE_THREAD_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MINIDUMP_IO_CALLBACK {
    pub Handle: super::super::super::Foundation::HANDLE,
    pub Offset: u64,
    pub Buffer: *mut ::core::ffi::c_void,
    pub BufferBytes: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MINIDUMP_IO_CALLBACK {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MINIDUMP_IO_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_LOCATION_DESCRIPTOR {
    pub DataSize: u32,
    pub Rva: u32,
}
impl ::core::marker::Copy for MINIDUMP_LOCATION_DESCRIPTOR {}
impl ::core::clone::Clone for MINIDUMP_LOCATION_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_LOCATION_DESCRIPTOR64 {
    pub DataSize: u64,
    pub Rva: u64,
}
impl ::core::marker::Copy for MINIDUMP_LOCATION_DESCRIPTOR64 {}
impl ::core::clone::Clone for MINIDUMP_LOCATION_DESCRIPTOR64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MEMORY64_LIST {
    pub NumberOfMemoryRanges: u64,
    pub BaseRva: u64,
    pub MemoryRanges: [MINIDUMP_MEMORY_DESCRIPTOR64; 1],
}
impl ::core::marker::Copy for MINIDUMP_MEMORY64_LIST {}
impl ::core::clone::Clone for MINIDUMP_MEMORY64_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MEMORY_DESCRIPTOR {
    pub StartOfMemoryRange: u64,
    pub Memory: MINIDUMP_LOCATION_DESCRIPTOR,
}
impl ::core::marker::Copy for MINIDUMP_MEMORY_DESCRIPTOR {}
impl ::core::clone::Clone for MINIDUMP_MEMORY_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MEMORY_DESCRIPTOR64 {
    pub StartOfMemoryRange: u64,
    pub DataSize: u64,
}
impl ::core::marker::Copy for MINIDUMP_MEMORY_DESCRIPTOR64 {}
impl ::core::clone::Clone for MINIDUMP_MEMORY_DESCRIPTOR64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_System_Memory\"`"]
#[cfg(feature = "Win32_System_Memory")]
pub struct MINIDUMP_MEMORY_INFO {
    pub BaseAddress: u64,
    pub AllocationBase: u64,
    pub AllocationProtect: u32,
    pub __alignment1: u32,
    pub RegionSize: u64,
    pub State: super::super::Memory::VIRTUAL_ALLOCATION_TYPE,
    pub Protect: u32,
    pub Type: u32,
    pub __alignment2: u32,
}
#[cfg(feature = "Win32_System_Memory")]
impl ::core::marker::Copy for MINIDUMP_MEMORY_INFO {}
#[cfg(feature = "Win32_System_Memory")]
impl ::core::clone::Clone for MINIDUMP_MEMORY_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MEMORY_INFO_LIST {
    pub SizeOfHeader: u32,
    pub SizeOfEntry: u32,
    pub NumberOfEntries: u64,
}
impl ::core::marker::Copy for MINIDUMP_MEMORY_INFO_LIST {}
impl ::core::clone::Clone for MINIDUMP_MEMORY_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MEMORY_LIST {
    pub NumberOfMemoryRanges: u32,
    pub MemoryRanges: [MINIDUMP_MEMORY_DESCRIPTOR; 1],
}
impl ::core::marker::Copy for MINIDUMP_MEMORY_LIST {}
impl ::core::clone::Clone for MINIDUMP_MEMORY_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MISC_INFO {
    pub SizeOfInfo: u32,
    pub Flags1: MINIDUMP_MISC_INFO_FLAGS,
    pub ProcessId: u32,
    pub ProcessCreateTime: u32,
    pub ProcessUserTime: u32,
    pub ProcessKernelTime: u32,
}
impl ::core::marker::Copy for MINIDUMP_MISC_INFO {}
impl ::core::clone::Clone for MINIDUMP_MISC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_MISC_INFO_2 {
    pub SizeOfInfo: u32,
    pub Flags1: u32,
    pub ProcessId: u32,
    pub ProcessCreateTime: u32,
    pub ProcessUserTime: u32,
    pub ProcessKernelTime: u32,
    pub ProcessorMaxMhz: u32,
    pub ProcessorCurrentMhz: u32,
    pub ProcessorMhzLimit: u32,
    pub ProcessorMaxIdleState: u32,
    pub ProcessorCurrentIdleState: u32,
}
impl ::core::marker::Copy for MINIDUMP_MISC_INFO_2 {}
impl ::core::clone::Clone for MINIDUMP_MISC_INFO_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Time\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
pub struct MINIDUMP_MISC_INFO_3 {
    pub SizeOfInfo: u32,
    pub Flags1: u32,
    pub ProcessId: u32,
    pub ProcessCreateTime: u32,
    pub ProcessUserTime: u32,
    pub ProcessKernelTime: u32,
    pub ProcessorMaxMhz: u32,
    pub ProcessorCurrentMhz: u32,
    pub ProcessorMhzLimit: u32,
    pub ProcessorMaxIdleState: u32,
    pub ProcessorCurrentIdleState: u32,
    pub ProcessIntegrityLevel: u32,
    pub ProcessExecuteFlags: u32,
    pub ProtectedProcess: u32,
    pub TimeZoneId: u32,
    pub TimeZone: super::super::Time::TIME_ZONE_INFORMATION,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
impl ::core::marker::Copy for MINIDUMP_MISC_INFO_3 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
impl ::core::clone::Clone for MINIDUMP_MISC_INFO_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Time\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
pub struct MINIDUMP_MISC_INFO_4 {
    pub SizeOfInfo: u32,
    pub Flags1: u32,
    pub ProcessId: u32,
    pub ProcessCreateTime: u32,
    pub ProcessUserTime: u32,
    pub ProcessKernelTime: u32,
    pub ProcessorMaxMhz: u32,
    pub ProcessorCurrentMhz: u32,
    pub ProcessorMhzLimit: u32,
    pub ProcessorMaxIdleState: u32,
    pub ProcessorCurrentIdleState: u32,
    pub ProcessIntegrityLevel: u32,
    pub ProcessExecuteFlags: u32,
    pub ProtectedProcess: u32,
    pub TimeZoneId: u32,
    pub TimeZone: super::super::Time::TIME_ZONE_INFORMATION,
    pub BuildString: [u16; 260],
    pub DbgBldStr: [u16; 40],
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
impl ::core::marker::Copy for MINIDUMP_MISC_INFO_4 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
impl ::core::clone::Clone for MINIDUMP_MISC_INFO_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Time\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
pub struct MINIDUMP_MISC_INFO_5 {
    pub SizeOfInfo: u32,
    pub Flags1: u32,
    pub ProcessId: u32,
    pub ProcessCreateTime: u32,
    pub ProcessUserTime: u32,
    pub ProcessKernelTime: u32,
    pub ProcessorMaxMhz: u32,
    pub ProcessorCurrentMhz: u32,
    pub ProcessorMhzLimit: u32,
    pub ProcessorMaxIdleState: u32,
    pub ProcessorCurrentIdleState: u32,
    pub ProcessIntegrityLevel: u32,
    pub ProcessExecuteFlags: u32,
    pub ProtectedProcess: u32,
    pub TimeZoneId: u32,
    pub TimeZone: super::super::Time::TIME_ZONE_INFORMATION,
    pub BuildString: [u16; 260],
    pub DbgBldStr: [u16; 40],
    pub XStateData: XSTATE_CONFIG_FEATURE_MSC_INFO,
    pub ProcessCookie: u32,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
impl ::core::marker::Copy for MINIDUMP_MISC_INFO_5 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Time"))]
impl ::core::clone::Clone for MINIDUMP_MISC_INFO_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct MINIDUMP_MODULE {
    pub BaseOfImage: u64,
    pub SizeOfImage: u32,
    pub CheckSum: u32,
    pub TimeDateStamp: u32,
    pub ModuleNameRva: u32,
    pub VersionInfo: super::super::super::Storage::FileSystem::VS_FIXEDFILEINFO,
    pub CvRecord: MINIDUMP_LOCATION_DESCRIPTOR,
    pub MiscRecord: MINIDUMP_LOCATION_DESCRIPTOR,
    pub Reserved0: u64,
    pub Reserved1: u64,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for MINIDUMP_MODULE {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for MINIDUMP_MODULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct MINIDUMP_MODULE_CALLBACK {
    pub FullPath: ::windows_sys::core::PWSTR,
    pub BaseOfImage: u64,
    pub SizeOfImage: u32,
    pub CheckSum: u32,
    pub TimeDateStamp: u32,
    pub VersionInfo: super::super::super::Storage::FileSystem::VS_FIXEDFILEINFO,
    pub CvRecord: *mut ::core::ffi::c_void,
    pub SizeOfCvRecord: u32,
    pub MiscRecord: *mut ::core::ffi::c_void,
    pub SizeOfMiscRecord: u32,
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for MINIDUMP_MODULE_CALLBACK {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for MINIDUMP_MODULE_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Storage_FileSystem\"`"]
#[cfg(feature = "Win32_Storage_FileSystem")]
pub struct MINIDUMP_MODULE_LIST {
    pub NumberOfModules: u32,
    pub Modules: [MINIDUMP_MODULE; 1],
}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::marker::Copy for MINIDUMP_MODULE_LIST {}
#[cfg(feature = "Win32_Storage_FileSystem")]
impl ::core::clone::Clone for MINIDUMP_MODULE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_PROCESS_VM_COUNTERS_1 {
    pub Revision: u16,
    pub PageFaultCount: u32,
    pub PeakWorkingSetSize: u64,
    pub WorkingSetSize: u64,
    pub QuotaPeakPagedPoolUsage: u64,
    pub QuotaPagedPoolUsage: u64,
    pub QuotaPeakNonPagedPoolUsage: u64,
    pub QuotaNonPagedPoolUsage: u64,
    pub PagefileUsage: u64,
    pub PeakPagefileUsage: u64,
    pub PrivateUsage: u64,
}
impl ::core::marker::Copy for MINIDUMP_PROCESS_VM_COUNTERS_1 {}
impl ::core::clone::Clone for MINIDUMP_PROCESS_VM_COUNTERS_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_PROCESS_VM_COUNTERS_2 {
    pub Revision: u16,
    pub Flags: u16,
    pub PageFaultCount: u32,
    pub PeakWorkingSetSize: u64,
    pub WorkingSetSize: u64,
    pub QuotaPeakPagedPoolUsage: u64,
    pub QuotaPagedPoolUsage: u64,
    pub QuotaPeakNonPagedPoolUsage: u64,
    pub QuotaNonPagedPoolUsage: u64,
    pub PagefileUsage: u64,
    pub PeakPagefileUsage: u64,
    pub PeakVirtualSize: u64,
    pub VirtualSize: u64,
    pub PrivateUsage: u64,
    pub PrivateWorkingSetSize: u64,
    pub SharedCommitUsage: u64,
    pub JobSharedCommitUsage: u64,
    pub JobPrivateCommitUsage: u64,
    pub JobPeakPrivateCommitUsage: u64,
    pub JobPrivateCommitLimit: u64,
    pub JobTotalCommitLimit: u64,
}
impl ::core::marker::Copy for MINIDUMP_PROCESS_VM_COUNTERS_2 {}
impl ::core::clone::Clone for MINIDUMP_PROCESS_VM_COUNTERS_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_READ_MEMORY_FAILURE_CALLBACK {
    pub Offset: u64,
    pub Bytes: u32,
    pub FailureStatus: ::windows_sys::core::HRESULT,
}
impl ::core::marker::Copy for MINIDUMP_READ_MEMORY_FAILURE_CALLBACK {}
impl ::core::clone::Clone for MINIDUMP_READ_MEMORY_FAILURE_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_STRING {
    pub Length: u32,
    pub Buffer: [u16; 1],
}
impl ::core::marker::Copy for MINIDUMP_STRING {}
impl ::core::clone::Clone for MINIDUMP_STRING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_SYSTEM_BASIC_INFORMATION {
    pub TimerResolution: u32,
    pub PageSize: u32,
    pub NumberOfPhysicalPages: u32,
    pub LowestPhysicalPageNumber: u32,
    pub HighestPhysicalPageNumber: u32,
    pub AllocationGranularity: u32,
    pub MinimumUserModeAddress: u64,
    pub MaximumUserModeAddress: u64,
    pub ActiveProcessorsAffinityMask: u64,
    pub NumberOfProcessors: u32,
}
impl ::core::marker::Copy for MINIDUMP_SYSTEM_BASIC_INFORMATION {}
impl ::core::clone::Clone for MINIDUMP_SYSTEM_BASIC_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_SYSTEM_BASIC_PERFORMANCE_INFORMATION {
    pub AvailablePages: u64,
    pub CommittedPages: u64,
    pub CommitLimit: u64,
    pub PeakCommitment: u64,
}
impl ::core::marker::Copy for MINIDUMP_SYSTEM_BASIC_PERFORMANCE_INFORMATION {}
impl ::core::clone::Clone for MINIDUMP_SYSTEM_BASIC_PERFORMANCE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_SYSTEM_FILECACHE_INFORMATION {
    pub CurrentSize: u64,
    pub PeakSize: u64,
    pub PageFaultCount: u32,
    pub MinimumWorkingSet: u64,
    pub MaximumWorkingSet: u64,
    pub CurrentSizeIncludingTransitionInPages: u64,
    pub PeakSizeIncludingTransitionInPages: u64,
    pub TransitionRePurposeCount: u32,
    pub Flags: u32,
}
impl ::core::marker::Copy for MINIDUMP_SYSTEM_FILECACHE_INFORMATION {}
impl ::core::clone::Clone for MINIDUMP_SYSTEM_FILECACHE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct MINIDUMP_SYSTEM_INFO {
    pub ProcessorArchitecture: super::super::SystemInformation::PROCESSOR_ARCHITECTURE,
    pub ProcessorLevel: u16,
    pub ProcessorRevision: u16,
    pub Anonymous1: MINIDUMP_SYSTEM_INFO_0,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub BuildNumber: u32,
    pub PlatformId: VER_PLATFORM,
    pub CSDVersionRva: u32,
    pub Anonymous2: MINIDUMP_SYSTEM_INFO_1,
    pub Cpu: CPU_INFORMATION,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for MINIDUMP_SYSTEM_INFO {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for MINIDUMP_SYSTEM_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub union MINIDUMP_SYSTEM_INFO_0 {
    pub Reserved0: u16,
    pub Anonymous: MINIDUMP_SYSTEM_INFO_0_0,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for MINIDUMP_SYSTEM_INFO_0 {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for MINIDUMP_SYSTEM_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct MINIDUMP_SYSTEM_INFO_0_0 {
    pub NumberOfProcessors: u8,
    pub ProductType: u8,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for MINIDUMP_SYSTEM_INFO_0_0 {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for MINIDUMP_SYSTEM_INFO_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub union MINIDUMP_SYSTEM_INFO_1 {
    pub Reserved1: u32,
    pub Anonymous: MINIDUMP_SYSTEM_INFO_1_0,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for MINIDUMP_SYSTEM_INFO_1 {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for MINIDUMP_SYSTEM_INFO_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_SystemInformation\"`"]
#[cfg(feature = "Win32_System_SystemInformation")]
pub struct MINIDUMP_SYSTEM_INFO_1_0 {
    pub SuiteMask: u16,
    pub Reserved2: u16,
}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::marker::Copy for MINIDUMP_SYSTEM_INFO_1_0 {}
#[cfg(feature = "Win32_System_SystemInformation")]
impl ::core::clone::Clone for MINIDUMP_SYSTEM_INFO_1_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_SYSTEM_MEMORY_INFO_1 {
    pub Revision: u16,
    pub Flags: u16,
    pub BasicInfo: MINIDUMP_SYSTEM_BASIC_INFORMATION,
    pub FileCacheInfo: MINIDUMP_SYSTEM_FILECACHE_INFORMATION,
    pub BasicPerfInfo: MINIDUMP_SYSTEM_BASIC_PERFORMANCE_INFORMATION,
    pub PerfInfo: MINIDUMP_SYSTEM_PERFORMANCE_INFORMATION,
}
impl ::core::marker::Copy for MINIDUMP_SYSTEM_MEMORY_INFO_1 {}
impl ::core::clone::Clone for MINIDUMP_SYSTEM_MEMORY_INFO_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_SYSTEM_PERFORMANCE_INFORMATION {
    pub IdleProcessTime: u64,
    pub IoReadTransferCount: u64,
    pub IoWriteTransferCount: u64,
    pub IoOtherTransferCount: u64,
    pub IoReadOperationCount: u32,
    pub IoWriteOperationCount: u32,
    pub IoOtherOperationCount: u32,
    pub AvailablePages: u32,
    pub CommittedPages: u32,
    pub CommitLimit: u32,
    pub PeakCommitment: u32,
    pub PageFaultCount: u32,
    pub CopyOnWriteCount: u32,
    pub TransitionCount: u32,
    pub CacheTransitionCount: u32,
    pub DemandZeroCount: u32,
    pub PageReadCount: u32,
    pub PageReadIoCount: u32,
    pub CacheReadCount: u32,
    pub CacheIoCount: u32,
    pub DirtyPagesWriteCount: u32,
    pub DirtyWriteIoCount: u32,
    pub MappedPagesWriteCount: u32,
    pub MappedWriteIoCount: u32,
    pub PagedPoolPages: u32,
    pub NonPagedPoolPages: u32,
    pub PagedPoolAllocs: u32,
    pub PagedPoolFrees: u32,
    pub NonPagedPoolAllocs: u32,
    pub NonPagedPoolFrees: u32,
    pub FreeSystemPtes: u32,
    pub ResidentSystemCodePage: u32,
    pub TotalSystemDriverPages: u32,
    pub TotalSystemCodePages: u32,
    pub NonPagedPoolLookasideHits: u32,
    pub PagedPoolLookasideHits: u32,
    pub AvailablePagedPoolPages: u32,
    pub ResidentSystemCachePage: u32,
    pub ResidentPagedPoolPage: u32,
    pub ResidentSystemDriverPage: u32,
    pub CcFastReadNoWait: u32,
    pub CcFastReadWait: u32,
    pub CcFastReadResourceMiss: u32,
    pub CcFastReadNotPossible: u32,
    pub CcFastMdlReadNoWait: u32,
    pub CcFastMdlReadWait: u32,
    pub CcFastMdlReadResourceMiss: u32,
    pub CcFastMdlReadNotPossible: u32,
    pub CcMapDataNoWait: u32,
    pub CcMapDataWait: u32,
    pub CcMapDataNoWaitMiss: u32,
    pub CcMapDataWaitMiss: u32,
    pub CcPinMappedDataCount: u32,
    pub CcPinReadNoWait: u32,
    pub CcPinReadWait: u32,
    pub CcPinReadNoWaitMiss: u32,
    pub CcPinReadWaitMiss: u32,
    pub CcCopyReadNoWait: u32,
    pub CcCopyReadWait: u32,
    pub CcCopyReadNoWaitMiss: u32,
    pub CcCopyReadWaitMiss: u32,
    pub CcMdlReadNoWait: u32,
    pub CcMdlReadWait: u32,
    pub CcMdlReadNoWaitMiss: u32,
    pub CcMdlReadWaitMiss: u32,
    pub CcReadAheadIos: u32,
    pub CcLazyWriteIos: u32,
    pub CcLazyWritePages: u32,
    pub CcDataFlushes: u32,
    pub CcDataPages: u32,
    pub ContextSwitches: u32,
    pub FirstLevelTbFills: u32,
    pub SecondLevelTbFills: u32,
    pub SystemCalls: u32,
    pub CcTotalDirtyPages: u64,
    pub CcDirtyPageThreshold: u64,
    pub ResidentAvailablePages: i64,
    pub SharedCommittedPages: u64,
}
impl ::core::marker::Copy for MINIDUMP_SYSTEM_PERFORMANCE_INFORMATION {}
impl ::core::clone::Clone for MINIDUMP_SYSTEM_PERFORMANCE_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD {
    pub ThreadId: u32,
    pub SuspendCount: u32,
    pub PriorityClass: u32,
    pub Priority: u32,
    pub Teb: u64,
    pub Stack: MINIDUMP_MEMORY_DESCRIPTOR,
    pub ThreadContext: MINIDUMP_LOCATION_DESCRIPTOR,
}
impl ::core::marker::Copy for MINIDUMP_THREAD {}
impl ::core::clone::Clone for MINIDUMP_THREAD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_THREAD_CALLBACK {
    pub ThreadId: u32,
    pub ThreadHandle: super::super::super::Foundation::HANDLE,
    pub Pad: u32,
    pub Context: CONTEXT,
    pub SizeOfContext: u32,
    pub StackBase: u64,
    pub StackEnd: u64,
}
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_THREAD_CALLBACK {}
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_THREAD_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_THREAD_CALLBACK {
    pub ThreadId: u32,
    pub ThreadHandle: super::super::super::Foundation::HANDLE,
    pub Context: CONTEXT,
    pub SizeOfContext: u32,
    pub StackBase: u64,
    pub StackEnd: u64,
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_THREAD_CALLBACK {}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_THREAD_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_EX {
    pub ThreadId: u32,
    pub SuspendCount: u32,
    pub PriorityClass: u32,
    pub Priority: u32,
    pub Teb: u64,
    pub Stack: MINIDUMP_MEMORY_DESCRIPTOR,
    pub ThreadContext: MINIDUMP_LOCATION_DESCRIPTOR,
    pub BackingStore: MINIDUMP_MEMORY_DESCRIPTOR,
}
impl ::core::marker::Copy for MINIDUMP_THREAD_EX {}
impl ::core::clone::Clone for MINIDUMP_THREAD_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_THREAD_EX_CALLBACK {
    pub ThreadId: u32,
    pub ThreadHandle: super::super::super::Foundation::HANDLE,
    pub Pad: u32,
    pub Context: CONTEXT,
    pub SizeOfContext: u32,
    pub StackBase: u64,
    pub StackEnd: u64,
    pub BackingStoreBase: u64,
    pub BackingStoreEnd: u64,
}
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_THREAD_EX_CALLBACK {}
#[cfg(target_arch = "aarch64")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_THREAD_EX_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub struct MINIDUMP_THREAD_EX_CALLBACK {
    pub ThreadId: u32,
    pub ThreadHandle: super::super::super::Foundation::HANDLE,
    pub Context: CONTEXT,
    pub SizeOfContext: u32,
    pub StackBase: u64,
    pub StackEnd: u64,
    pub BackingStoreBase: u64,
    pub BackingStoreEnd: u64,
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::marker::Copy for MINIDUMP_THREAD_EX_CALLBACK {}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
impl ::core::clone::Clone for MINIDUMP_THREAD_EX_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_EX_LIST {
    pub NumberOfThreads: u32,
    pub Threads: [MINIDUMP_THREAD_EX; 1],
}
impl ::core::marker::Copy for MINIDUMP_THREAD_EX_LIST {}
impl ::core::clone::Clone for MINIDUMP_THREAD_EX_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_INFO {
    pub ThreadId: u32,
    pub DumpFlags: MINIDUMP_THREAD_INFO_DUMP_FLAGS,
    pub DumpError: u32,
    pub ExitStatus: u32,
    pub CreateTime: u64,
    pub ExitTime: u64,
    pub KernelTime: u64,
    pub UserTime: u64,
    pub StartAddress: u64,
    pub Affinity: u64,
}
impl ::core::marker::Copy for MINIDUMP_THREAD_INFO {}
impl ::core::clone::Clone for MINIDUMP_THREAD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_INFO_LIST {
    pub SizeOfHeader: u32,
    pub SizeOfEntry: u32,
    pub NumberOfEntries: u32,
}
impl ::core::marker::Copy for MINIDUMP_THREAD_INFO_LIST {}
impl ::core::clone::Clone for MINIDUMP_THREAD_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_LIST {
    pub NumberOfThreads: u32,
    pub Threads: [MINIDUMP_THREAD; 1],
}
impl ::core::marker::Copy for MINIDUMP_THREAD_LIST {}
impl ::core::clone::Clone for MINIDUMP_THREAD_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_NAME {
    pub ThreadId: u32,
    pub RvaOfThreadName: u64,
}
impl ::core::marker::Copy for MINIDUMP_THREAD_NAME {}
impl ::core::clone::Clone for MINIDUMP_THREAD_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_THREAD_NAME_LIST {
    pub NumberOfThreadNames: u32,
    pub ThreadNames: [MINIDUMP_THREAD_NAME; 1],
}
impl ::core::marker::Copy for MINIDUMP_THREAD_NAME_LIST {}
impl ::core::clone::Clone for MINIDUMP_THREAD_NAME_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_TOKEN_INFO_HEADER {
    pub TokenSize: u32,
    pub TokenId: u32,
    pub TokenHandle: u64,
}
impl ::core::marker::Copy for MINIDUMP_TOKEN_INFO_HEADER {}
impl ::core::clone::Clone for MINIDUMP_TOKEN_INFO_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_TOKEN_INFO_LIST {
    pub TokenListSize: u32,
    pub TokenListEntries: u32,
    pub ListHeaderSize: u32,
    pub ElementHeaderSize: u32,
}
impl ::core::marker::Copy for MINIDUMP_TOKEN_INFO_LIST {}
impl ::core::clone::Clone for MINIDUMP_TOKEN_INFO_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_UNLOADED_MODULE {
    pub BaseOfImage: u64,
    pub SizeOfImage: u32,
    pub CheckSum: u32,
    pub TimeDateStamp: u32,
    pub ModuleNameRva: u32,
}
impl ::core::marker::Copy for MINIDUMP_UNLOADED_MODULE {}
impl ::core::clone::Clone for MINIDUMP_UNLOADED_MODULE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_UNLOADED_MODULE_LIST {
    pub SizeOfHeader: u32,
    pub SizeOfEntry: u32,
    pub NumberOfEntries: u32,
}
impl ::core::marker::Copy for MINIDUMP_UNLOADED_MODULE_LIST {}
impl ::core::clone::Clone for MINIDUMP_UNLOADED_MODULE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_USER_RECORD {
    pub Type: u32,
    pub Memory: MINIDUMP_LOCATION_DESCRIPTOR,
}
impl ::core::marker::Copy for MINIDUMP_USER_RECORD {}
impl ::core::clone::Clone for MINIDUMP_USER_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct MINIDUMP_USER_STREAM {
    pub Type: u32,
    pub BufferSize: u32,
    pub Buffer: *mut ::core::ffi::c_void,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for MINIDUMP_USER_STREAM {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for MINIDUMP_USER_STREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct MINIDUMP_USER_STREAM {
    pub Type: u32,
    pub BufferSize: u32,
    pub Buffer: *mut ::core::ffi::c_void,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for MINIDUMP_USER_STREAM {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for MINIDUMP_USER_STREAM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct MINIDUMP_USER_STREAM_INFORMATION {
    pub UserStreamCount: u32,
    pub UserStreamArray: *mut MINIDUMP_USER_STREAM,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for MINIDUMP_USER_STREAM_INFORMATION {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for MINIDUMP_USER_STREAM_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct MINIDUMP_USER_STREAM_INFORMATION {
    pub UserStreamCount: u32,
    pub UserStreamArray: *mut MINIDUMP_USER_STREAM,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for MINIDUMP_USER_STREAM_INFORMATION {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for MINIDUMP_USER_STREAM_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_VM_POST_READ_CALLBACK {
    pub Offset: u64,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Size: u32,
    pub Completed: u32,
    pub Status: ::windows_sys::core::HRESULT,
}
impl ::core::marker::Copy for MINIDUMP_VM_POST_READ_CALLBACK {}
impl ::core::clone::Clone for MINIDUMP_VM_POST_READ_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_VM_PRE_READ_CALLBACK {
    pub Offset: u64,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Size: u32,
}
impl ::core::marker::Copy for MINIDUMP_VM_PRE_READ_CALLBACK {}
impl ::core::clone::Clone for MINIDUMP_VM_PRE_READ_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct MINIDUMP_VM_QUERY_CALLBACK {
    pub Offset: u64,
}
impl ::core::marker::Copy for MINIDUMP_VM_QUERY_CALLBACK {}
impl ::core::clone::Clone for MINIDUMP_VM_QUERY_CALLBACK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODLOAD_CVMISC {
    pub oCV: u32,
    pub cCV: usize,
    pub oMisc: u32,
    pub cMisc: usize,
    pub dtImage: u32,
    pub cImage: u32,
}
impl ::core::marker::Copy for MODLOAD_CVMISC {}
impl ::core::clone::Clone for MODLOAD_CVMISC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODLOAD_DATA {
    pub ssize: u32,
    pub ssig: MODLOAD_DATA_TYPE,
    pub data: *mut ::core::ffi::c_void,
    pub size: u32,
    pub flags: u32,
}
impl ::core::marker::Copy for MODLOAD_DATA {}
impl ::core::clone::Clone for MODLOAD_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODLOAD_PDBGUID_PDBAGE {
    pub PdbGuid: ::windows_sys::core::GUID,
    pub PdbAge: u32,
}
impl ::core::marker::Copy for MODLOAD_PDBGUID_PDBAGE {}
impl ::core::clone::Clone for MODLOAD_PDBGUID_PDBAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MODULE_TYPE_INFO {
    pub dataLength: u16,
    pub leaf: u16,
    pub data: [u8; 1],
}
impl ::core::marker::Copy for MODULE_TYPE_INFO {}
impl ::core::clone::Clone for MODULE_TYPE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OMAP {
    pub rva: u32,
    pub rvaTo: u32,
}
impl ::core::marker::Copy for OMAP {}
impl ::core::clone::Clone for OMAP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OUTPUT_DEBUG_STRING_INFO {
    pub lpDebugStringData: ::windows_sys::core::PSTR,
    pub fUnicode: u16,
    pub nDebugStringLength: u16,
}
impl ::core::marker::Copy for OUTPUT_DEBUG_STRING_INFO {}
impl ::core::clone::Clone for OUTPUT_DEBUG_STRING_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_MEMORY_DESCRIPTOR32 {
    pub NumberOfRuns: u32,
    pub NumberOfPages: u32,
    pub Run: [PHYSICAL_MEMORY_RUN32; 1],
}
impl ::core::marker::Copy for PHYSICAL_MEMORY_DESCRIPTOR32 {}
impl ::core::clone::Clone for PHYSICAL_MEMORY_DESCRIPTOR32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_MEMORY_DESCRIPTOR64 {
    pub NumberOfRuns: u32,
    pub NumberOfPages: u64,
    pub Run: [PHYSICAL_MEMORY_RUN64; 1],
}
impl ::core::marker::Copy for PHYSICAL_MEMORY_DESCRIPTOR64 {}
impl ::core::clone::Clone for PHYSICAL_MEMORY_DESCRIPTOR64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_MEMORY_RUN32 {
    pub BasePage: u32,
    pub PageCount: u32,
}
impl ::core::marker::Copy for PHYSICAL_MEMORY_RUN32 {}
impl ::core::clone::Clone for PHYSICAL_MEMORY_RUN32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PHYSICAL_MEMORY_RUN64 {
    pub BasePage: u64,
    pub PageCount: u64,
}
impl ::core::marker::Copy for PHYSICAL_MEMORY_RUN64 {}
impl ::core::clone::Clone for PHYSICAL_MEMORY_RUN64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RIP_INFO {
    pub dwError: u32,
    pub dwType: RIP_INFO_TYPE,
}
impl ::core::marker::Copy for RIP_INFO {}
impl ::core::clone::Clone for RIP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOURCEFILE {
    pub ModBase: u64,
    pub FileName: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for SOURCEFILE {}
impl ::core::clone::Clone for SOURCEFILE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SOURCEFILEW {
    pub ModBase: u64,
    pub FileName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for SOURCEFILEW {}
impl ::core::clone::Clone for SOURCEFILEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SRCCODEINFO {
    pub SizeOfStruct: u32,
    pub Key: *mut ::core::ffi::c_void,
    pub ModBase: u64,
    pub Obj: [u8; 261],
    pub FileName: [u8; 261],
    pub LineNumber: u32,
    pub Address: u64,
}
impl ::core::marker::Copy for SRCCODEINFO {}
impl ::core::clone::Clone for SRCCODEINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SRCCODEINFOW {
    pub SizeOfStruct: u32,
    pub Key: *mut ::core::ffi::c_void,
    pub ModBase: u64,
    pub Obj: [u16; 261],
    pub FileName: [u16; 261],
    pub LineNumber: u32,
    pub Address: u64,
}
impl ::core::marker::Copy for SRCCODEINFOW {}
impl ::core::clone::Clone for SRCCODEINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct STACKFRAME {
    pub AddrPC: ADDRESS,
    pub AddrReturn: ADDRESS,
    pub AddrFrame: ADDRESS,
    pub AddrStack: ADDRESS,
    pub FuncTableEntry: *mut ::core::ffi::c_void,
    pub Params: [u32; 4],
    pub Far: super::super::super::Foundation::BOOL,
    pub Virtual: super::super::super::Foundation::BOOL,
    pub Reserved: [u32; 3],
    pub KdHelp: KDHELP,
    pub AddrBStore: ADDRESS,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for STACKFRAME {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for STACKFRAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct STACKFRAME64 {
    pub AddrPC: ADDRESS64,
    pub AddrReturn: ADDRESS64,
    pub AddrFrame: ADDRESS64,
    pub AddrStack: ADDRESS64,
    pub AddrBStore: ADDRESS64,
    pub FuncTableEntry: *mut ::core::ffi::c_void,
    pub Params: [u64; 4],
    pub Far: super::super::super::Foundation::BOOL,
    pub Virtual: super::super::super::Foundation::BOOL,
    pub Reserved: [u64; 3],
    pub KdHelp: KDHELP64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for STACKFRAME64 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for STACKFRAME64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct STACKFRAME_EX {
    pub AddrPC: ADDRESS64,
    pub AddrReturn: ADDRESS64,
    pub AddrFrame: ADDRESS64,
    pub AddrStack: ADDRESS64,
    pub AddrBStore: ADDRESS64,
    pub FuncTableEntry: *mut ::core::ffi::c_void,
    pub Params: [u64; 4],
    pub Far: super::super::super::Foundation::BOOL,
    pub Virtual: super::super::super::Foundation::BOOL,
    pub Reserved: [u64; 3],
    pub KdHelp: KDHELP64,
    pub StackFrameSize: u32,
    pub InlineFrameContext: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for STACKFRAME_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for STACKFRAME_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYMBOL_INFO {
    pub SizeOfStruct: u32,
    pub TypeIndex: u32,
    pub Reserved: [u64; 2],
    pub Index: u32,
    pub Size: u32,
    pub ModBase: u64,
    pub Flags: SYMBOL_INFO_FLAGS,
    pub Value: u64,
    pub Address: u64,
    pub Register: u32,
    pub Scope: u32,
    pub Tag: u32,
    pub NameLen: u32,
    pub MaxNameLen: u32,
    pub Name: [u8; 1],
}
impl ::core::marker::Copy for SYMBOL_INFO {}
impl ::core::clone::Clone for SYMBOL_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYMBOL_INFOW {
    pub SizeOfStruct: u32,
    pub TypeIndex: u32,
    pub Reserved: [u64; 2],
    pub Index: u32,
    pub Size: u32,
    pub ModBase: u64,
    pub Flags: SYMBOL_INFO_FLAGS,
    pub Value: u64,
    pub Address: u64,
    pub Register: u32,
    pub Scope: u32,
    pub Tag: u32,
    pub NameLen: u32,
    pub MaxNameLen: u32,
    pub Name: [u16; 1],
}
impl ::core::marker::Copy for SYMBOL_INFOW {}
impl ::core::clone::Clone for SYMBOL_INFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYMBOL_INFO_PACKAGE {
    pub si: SYMBOL_INFO,
    pub name: [u8; 2001],
}
impl ::core::marker::Copy for SYMBOL_INFO_PACKAGE {}
impl ::core::clone::Clone for SYMBOL_INFO_PACKAGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYMBOL_INFO_PACKAGEW {
    pub si: SYMBOL_INFOW,
    pub name: [u16; 2001],
}
impl ::core::marker::Copy for SYMBOL_INFO_PACKAGEW {}
impl ::core::clone::Clone for SYMBOL_INFO_PACKAGEW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SYMSRV_EXTENDED_OUTPUT_DATA {
    pub sizeOfStruct: u32,
    pub version: u32,
    pub filePtrMsg: [u16; 261],
}
impl ::core::marker::Copy for SYMSRV_EXTENDED_OUTPUT_DATA {}
impl ::core::clone::Clone for SYMSRV_EXTENDED_OUTPUT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SYMSRV_INDEX_INFO {
    pub sizeofstruct: u32,
    pub file: [u8; 261],
    pub stripped: super::super::super::Foundation::BOOL,
    pub timestamp: u32,
    pub size: u32,
    pub dbgfile: [u8; 261],
    pub pdbfile: [u8; 261],
    pub guid: ::windows_sys::core::GUID,
    pub sig: u32,
    pub age: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SYMSRV_INDEX_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SYMSRV_INDEX_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SYMSRV_INDEX_INFOW {
    pub sizeofstruct: u32,
    pub file: [u16; 261],
    pub stripped: super::super::super::Foundation::BOOL,
    pub timestamp: u32,
    pub size: u32,
    pub dbgfile: [u16; 261],
    pub pdbfile: [u16; 261],
    pub guid: ::windows_sys::core::GUID,
    pub sig: u32,
    pub age: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SYMSRV_INDEX_INFOW {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SYMSRV_INDEX_INFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TI_FINDCHILDREN_PARAMS {
    pub Count: u32,
    pub Start: u32,
    pub ChildId: [u32; 1],
}
impl ::core::marker::Copy for TI_FINDCHILDREN_PARAMS {}
impl ::core::clone::Clone for TI_FINDCHILDREN_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UNLOAD_DLL_DEBUG_INFO {
    pub lpBaseOfDll: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for UNLOAD_DLL_DEBUG_INFO {}
impl ::core::clone::Clone for UNLOAD_DLL_DEBUG_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct UNWIND_HISTORY_TABLE {
    pub Count: u32,
    pub LocalHint: u8,
    pub GlobalHint: u8,
    pub Search: u8,
    pub Once: u8,
    pub LowAddress: usize,
    pub HighAddress: usize,
    pub Entry: [UNWIND_HISTORY_TABLE_ENTRY; 12],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for UNWIND_HISTORY_TABLE {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for UNWIND_HISTORY_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "aarch64")]
pub struct UNWIND_HISTORY_TABLE_ENTRY {
    pub ImageBase: usize,
    pub FunctionEntry: *mut IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY,
}
#[cfg(target_arch = "aarch64")]
impl ::core::marker::Copy for UNWIND_HISTORY_TABLE_ENTRY {}
#[cfg(target_arch = "aarch64")]
impl ::core::clone::Clone for UNWIND_HISTORY_TABLE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86_64")]
pub struct UNWIND_HISTORY_TABLE_ENTRY {
    pub ImageBase: usize,
    pub FunctionEntry: *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
}
#[cfg(target_arch = "x86_64")]
impl ::core::marker::Copy for UNWIND_HISTORY_TABLE_ENTRY {}
#[cfg(target_arch = "x86_64")]
impl ::core::clone::Clone for UNWIND_HISTORY_TABLE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WAITCHAIN_NODE_INFO {
    pub ObjectType: WCT_OBJECT_TYPE,
    pub ObjectStatus: WCT_OBJECT_STATUS,
    pub Anonymous: WAITCHAIN_NODE_INFO_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WAITCHAIN_NODE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WAITCHAIN_NODE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union WAITCHAIN_NODE_INFO_0 {
    pub LockObject: WAITCHAIN_NODE_INFO_0_0,
    pub ThreadObject: WAITCHAIN_NODE_INFO_0_1,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WAITCHAIN_NODE_INFO_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WAITCHAIN_NODE_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WAITCHAIN_NODE_INFO_0_0 {
    pub ObjectName: [u16; 128],
    pub Timeout: i64,
    pub Alertable: super::super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WAITCHAIN_NODE_INFO_0_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WAITCHAIN_NODE_INFO_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WAITCHAIN_NODE_INFO_0_1 {
    pub ProcessId: u32,
    pub ThreadId: u32,
    pub WaitTime: u32,
    pub ContextSwitches: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WAITCHAIN_NODE_INFO_0_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WAITCHAIN_NODE_INFO_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_AER_BRIDGE_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: super::super::super::Foundation::BOOLEAN,
    pub Reserved: u8,
    pub BusNumber: u32,
    pub Slot: WHEA_PCI_SLOT_NUMBER,
    pub DeviceControl: u16,
    pub Flags: AER_BRIDGE_DESCRIPTOR_FLAGS,
    pub UncorrectableErrorMask: u32,
    pub UncorrectableErrorSeverity: u32,
    pub CorrectableErrorMask: u32,
    pub AdvancedCapsAndControl: u32,
    pub SecondaryUncorrectableErrorMask: u32,
    pub SecondaryUncorrectableErrorSev: u32,
    pub SecondaryCapsAndControl: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_AER_BRIDGE_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_AER_BRIDGE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_AER_ENDPOINT_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: super::super::super::Foundation::BOOLEAN,
    pub Reserved: u8,
    pub BusNumber: u32,
    pub Slot: WHEA_PCI_SLOT_NUMBER,
    pub DeviceControl: u16,
    pub Flags: AER_ENDPOINT_DESCRIPTOR_FLAGS,
    pub UncorrectableErrorMask: u32,
    pub UncorrectableErrorSeverity: u32,
    pub CorrectableErrorMask: u32,
    pub AdvancedCapsAndControl: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_AER_ENDPOINT_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_AER_ENDPOINT_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_AER_ROOTPORT_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: super::super::super::Foundation::BOOLEAN,
    pub Reserved: u8,
    pub BusNumber: u32,
    pub Slot: WHEA_PCI_SLOT_NUMBER,
    pub DeviceControl: u16,
    pub Flags: AER_ROOTPORT_DESCRIPTOR_FLAGS,
    pub UncorrectableErrorMask: u32,
    pub UncorrectableErrorSeverity: u32,
    pub CorrectableErrorMask: u32,
    pub AdvancedCapsAndControl: u32,
    pub RootErrorCommand: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_AER_ROOTPORT_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_AER_ROOTPORT_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_DEVICE_DRIVER_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: super::super::super::Foundation::BOOLEAN,
    pub Reserved: u8,
    pub SourceGuid: ::windows_sys::core::GUID,
    pub LogTag: u16,
    pub Reserved2: u16,
    pub PacketLength: u32,
    pub PacketCount: u32,
    pub PacketBuffer: *mut u8,
    pub Config: WHEA_ERROR_SOURCE_CONFIGURATION_DD,
    pub CreatorId: ::windows_sys::core::GUID,
    pub PartitionId: ::windows_sys::core::GUID,
    pub MaxSectionDataLength: u32,
    pub MaxSectionsPerRecord: u32,
    pub PacketStateBuffer: *mut u8,
    pub OpenHandles: i32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_DEVICE_DRIVER_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_DEVICE_DRIVER_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_DRIVER_BUFFER_SET {
    pub Version: u32,
    pub Data: *mut u8,
    pub DataSize: u32,
    pub SectionTypeGuid: *mut ::windows_sys::core::GUID,
    pub SectionFriendlyName: *mut u8,
    pub Flags: *mut u8,
}
impl ::core::marker::Copy for WHEA_DRIVER_BUFFER_SET {}
impl ::core::clone::Clone for WHEA_DRIVER_BUFFER_SET {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_ERROR_SOURCE_CONFIGURATION_DD {
    pub Initialize: WHEA_ERROR_SOURCE_INITIALIZE_DEVICE_DRIVER,
    pub Uninitialize: WHEA_ERROR_SOURCE_UNINITIALIZE_DEVICE_DRIVER,
    pub Correct: WHEA_ERROR_SOURCE_CORRECT_DEVICE_DRIVER,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_ERROR_SOURCE_CONFIGURATION_DD {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_ERROR_SOURCE_CONFIGURATION_DD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_ERROR_SOURCE_CONFIGURATION_DEVICE_DRIVER {
    pub Version: u32,
    pub SourceGuid: ::windows_sys::core::GUID,
    pub LogTag: u16,
    pub Reserved: [u8; 6],
    pub Initialize: WHEA_ERROR_SOURCE_INITIALIZE_DEVICE_DRIVER,
    pub Uninitialize: WHEA_ERROR_SOURCE_UNINITIALIZE_DEVICE_DRIVER,
    pub MaxSectionDataLength: u32,
    pub MaxSectionsPerReport: u32,
    pub CreatorId: ::windows_sys::core::GUID,
    pub PartitionId: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_ERROR_SOURCE_CONFIGURATION_DEVICE_DRIVER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_ERROR_SOURCE_CONFIGURATION_DEVICE_DRIVER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_ERROR_SOURCE_CONFIGURATION_DEVICE_DRIVER_V1 {
    pub Version: u32,
    pub SourceGuid: ::windows_sys::core::GUID,
    pub LogTag: u16,
    pub Reserved: [u8; 6],
    pub Initialize: WHEA_ERROR_SOURCE_INITIALIZE_DEVICE_DRIVER,
    pub Uninitialize: WHEA_ERROR_SOURCE_UNINITIALIZE_DEVICE_DRIVER,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_ERROR_SOURCE_CONFIGURATION_DEVICE_DRIVER_V1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_ERROR_SOURCE_CONFIGURATION_DEVICE_DRIVER_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_ERROR_SOURCE_DESCRIPTOR {
    pub Length: u32,
    pub Version: u32,
    pub Type: WHEA_ERROR_SOURCE_TYPE,
    pub State: WHEA_ERROR_SOURCE_STATE,
    pub MaxRawDataLength: u32,
    pub NumRecordsToPreallocate: u32,
    pub MaxSectionsPerRecord: u32,
    pub ErrorSourceId: u32,
    pub PlatformErrorSourceId: u32,
    pub Flags: u32,
    pub Info: WHEA_ERROR_SOURCE_DESCRIPTOR_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_ERROR_SOURCE_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_ERROR_SOURCE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union WHEA_ERROR_SOURCE_DESCRIPTOR_0 {
    pub XpfMceDescriptor: WHEA_XPF_MCE_DESCRIPTOR,
    pub XpfCmcDescriptor: WHEA_XPF_CMC_DESCRIPTOR,
    pub XpfNmiDescriptor: WHEA_XPF_NMI_DESCRIPTOR,
    pub IpfMcaDescriptor: WHEA_IPF_MCA_DESCRIPTOR,
    pub IpfCmcDescriptor: WHEA_IPF_CMC_DESCRIPTOR,
    pub IpfCpeDescriptor: WHEA_IPF_CPE_DESCRIPTOR,
    pub AerRootportDescriptor: WHEA_AER_ROOTPORT_DESCRIPTOR,
    pub AerEndpointDescriptor: WHEA_AER_ENDPOINT_DESCRIPTOR,
    pub AerBridgeDescriptor: WHEA_AER_BRIDGE_DESCRIPTOR,
    pub GenErrDescriptor: WHEA_GENERIC_ERROR_DESCRIPTOR,
    pub GenErrDescriptorV2: WHEA_GENERIC_ERROR_DESCRIPTOR_V2,
    pub DeviceDriverDescriptor: WHEA_DEVICE_DRIVER_DESCRIPTOR,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_ERROR_SOURCE_DESCRIPTOR_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_ERROR_SOURCE_DESCRIPTOR_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_GENERIC_ERROR_DESCRIPTOR {
    pub Type: u16,
    pub Reserved: u8,
    pub Enabled: u8,
    pub ErrStatusBlockLength: u32,
    pub RelatedErrorSourceId: u32,
    pub ErrStatusAddressSpaceID: u8,
    pub ErrStatusAddressBitWidth: u8,
    pub ErrStatusAddressBitOffset: u8,
    pub ErrStatusAddressAccessSize: u8,
    pub ErrStatusAddress: i64,
    pub Notify: WHEA_NOTIFICATION_DESCRIPTOR,
}
impl ::core::marker::Copy for WHEA_GENERIC_ERROR_DESCRIPTOR {}
impl ::core::clone::Clone for WHEA_GENERIC_ERROR_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_GENERIC_ERROR_DESCRIPTOR_V2 {
    pub Type: u16,
    pub Reserved: u8,
    pub Enabled: u8,
    pub ErrStatusBlockLength: u32,
    pub RelatedErrorSourceId: u32,
    pub ErrStatusAddressSpaceID: u8,
    pub ErrStatusAddressBitWidth: u8,
    pub ErrStatusAddressBitOffset: u8,
    pub ErrStatusAddressAccessSize: u8,
    pub ErrStatusAddress: i64,
    pub Notify: WHEA_NOTIFICATION_DESCRIPTOR,
    pub ReadAckAddressSpaceID: u8,
    pub ReadAckAddressBitWidth: u8,
    pub ReadAckAddressBitOffset: u8,
    pub ReadAckAddressAccessSize: u8,
    pub ReadAckAddress: i64,
    pub ReadAckPreserveMask: u64,
    pub ReadAckWriteMask: u64,
}
impl ::core::marker::Copy for WHEA_GENERIC_ERROR_DESCRIPTOR_V2 {}
impl ::core::clone::Clone for WHEA_GENERIC_ERROR_DESCRIPTOR_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_IPF_CMC_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: u8,
    pub Reserved: u8,
}
impl ::core::marker::Copy for WHEA_IPF_CMC_DESCRIPTOR {}
impl ::core::clone::Clone for WHEA_IPF_CMC_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_IPF_CPE_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: u8,
    pub Reserved: u8,
}
impl ::core::marker::Copy for WHEA_IPF_CPE_DESCRIPTOR {}
impl ::core::clone::Clone for WHEA_IPF_CPE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_IPF_MCA_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: u8,
    pub Reserved: u8,
}
impl ::core::marker::Copy for WHEA_IPF_MCA_DESCRIPTOR {}
impl ::core::clone::Clone for WHEA_IPF_MCA_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHEA_NOTIFICATION_DESCRIPTOR {
    pub Type: u8,
    pub Length: u8,
    pub Flags: WHEA_NOTIFICATION_FLAGS,
    pub u: WHEA_NOTIFICATION_DESCRIPTOR_0,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WHEA_NOTIFICATION_DESCRIPTOR_0 {
    pub Polled: WHEA_NOTIFICATION_DESCRIPTOR_0_4,
    pub Interrupt: WHEA_NOTIFICATION_DESCRIPTOR_0_1,
    pub LocalInterrupt: WHEA_NOTIFICATION_DESCRIPTOR_0_2,
    pub Sci: WHEA_NOTIFICATION_DESCRIPTOR_0_5,
    pub Nmi: WHEA_NOTIFICATION_DESCRIPTOR_0_3,
    pub Sea: WHEA_NOTIFICATION_DESCRIPTOR_0_6,
    pub Sei: WHEA_NOTIFICATION_DESCRIPTOR_0_7,
    pub Gsiv: WHEA_NOTIFICATION_DESCRIPTOR_0_0,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_0 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_0 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_1 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_1 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_2 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_2 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_3 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_3 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_4 {
    pub PollInterval: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_4 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_5 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_5 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_5 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_6 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_6 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_6 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_DESCRIPTOR_0_7 {
    pub PollInterval: u32,
    pub Vector: u32,
    pub SwitchToPollingThreshold: u32,
    pub SwitchToPollingWindow: u32,
    pub ErrorThreshold: u32,
    pub ErrorThresholdWindow: u32,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_DESCRIPTOR_0_7 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_DESCRIPTOR_0_7 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union WHEA_NOTIFICATION_FLAGS {
    pub Anonymous: WHEA_NOTIFICATION_FLAGS_0,
    pub AsUSHORT: u16,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_FLAGS {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_NOTIFICATION_FLAGS_0 {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for WHEA_NOTIFICATION_FLAGS_0 {}
impl ::core::clone::Clone for WHEA_NOTIFICATION_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WHEA_PCI_SLOT_NUMBER {
    pub u: WHEA_PCI_SLOT_NUMBER_0,
}
impl ::core::marker::Copy for WHEA_PCI_SLOT_NUMBER {}
impl ::core::clone::Clone for WHEA_PCI_SLOT_NUMBER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union WHEA_PCI_SLOT_NUMBER_0 {
    pub bits: WHEA_PCI_SLOT_NUMBER_0_0,
    pub AsULONG: u32,
}
impl ::core::marker::Copy for WHEA_PCI_SLOT_NUMBER_0 {}
impl ::core::clone::Clone for WHEA_PCI_SLOT_NUMBER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct WHEA_PCI_SLOT_NUMBER_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WHEA_PCI_SLOT_NUMBER_0_0 {}
impl ::core::clone::Clone for WHEA_PCI_SLOT_NUMBER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_XPF_CMC_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: super::super::super::Foundation::BOOLEAN,
    pub NumberOfBanks: u8,
    pub Reserved: u32,
    pub Notify: WHEA_NOTIFICATION_DESCRIPTOR,
    pub Banks: [WHEA_XPF_MC_BANK_DESCRIPTOR; 32],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_XPF_CMC_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_XPF_CMC_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_XPF_MCE_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: u8,
    pub NumberOfBanks: u8,
    pub Flags: XPF_MCE_FLAGS,
    pub MCG_Capability: u64,
    pub MCG_GlobalControl: u64,
    pub Banks: [WHEA_XPF_MC_BANK_DESCRIPTOR; 32],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_XPF_MCE_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_XPF_MCE_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_XPF_MC_BANK_DESCRIPTOR {
    pub BankNumber: u8,
    pub ClearOnInitialization: super::super::super::Foundation::BOOLEAN,
    pub StatusDataFormat: u8,
    pub Flags: XPF_MC_BANK_FLAGS,
    pub ControlMsr: u32,
    pub StatusMsr: u32,
    pub AddressMsr: u32,
    pub MiscMsr: u32,
    pub ControlData: u64,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_XPF_MC_BANK_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_XPF_MC_BANK_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct WHEA_XPF_NMI_DESCRIPTOR {
    pub Type: u16,
    pub Enabled: super::super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for WHEA_XPF_NMI_DESCRIPTOR {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for WHEA_XPF_NMI_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WOW64_CONTEXT {
    pub ContextFlags: WOW64_CONTEXT_FLAGS,
    pub Dr0: u32,
    pub Dr1: u32,
    pub Dr2: u32,
    pub Dr3: u32,
    pub Dr6: u32,
    pub Dr7: u32,
    pub FloatSave: WOW64_FLOATING_SAVE_AREA,
    pub SegGs: u32,
    pub SegFs: u32,
    pub SegEs: u32,
    pub SegDs: u32,
    pub Edi: u32,
    pub Esi: u32,
    pub Ebx: u32,
    pub Edx: u32,
    pub Ecx: u32,
    pub Eax: u32,
    pub Ebp: u32,
    pub Eip: u32,
    pub SegCs: u32,
    pub EFlags: u32,
    pub Esp: u32,
    pub SegSs: u32,
    pub ExtendedRegisters: [u8; 512],
}
impl ::core::marker::Copy for WOW64_CONTEXT {}
impl ::core::clone::Clone for WOW64_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WOW64_DESCRIPTOR_TABLE_ENTRY {
    pub Selector: u32,
    pub Descriptor: WOW64_LDT_ENTRY,
}
impl ::core::marker::Copy for WOW64_DESCRIPTOR_TABLE_ENTRY {}
impl ::core::clone::Clone for WOW64_DESCRIPTOR_TABLE_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WOW64_FLOATING_SAVE_AREA {
    pub ControlWord: u32,
    pub StatusWord: u32,
    pub TagWord: u32,
    pub ErrorOffset: u32,
    pub ErrorSelector: u32,
    pub DataOffset: u32,
    pub DataSelector: u32,
    pub RegisterArea: [u8; 80],
    pub Cr0NpxState: u32,
}
impl ::core::marker::Copy for WOW64_FLOATING_SAVE_AREA {}
impl ::core::clone::Clone for WOW64_FLOATING_SAVE_AREA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WOW64_LDT_ENTRY {
    pub LimitLow: u16,
    pub BaseLow: u16,
    pub HighWord: WOW64_LDT_ENTRY_0,
}
impl ::core::marker::Copy for WOW64_LDT_ENTRY {}
impl ::core::clone::Clone for WOW64_LDT_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union WOW64_LDT_ENTRY_0 {
    pub Bytes: WOW64_LDT_ENTRY_0_1,
    pub Bits: WOW64_LDT_ENTRY_0_0,
}
impl ::core::marker::Copy for WOW64_LDT_ENTRY_0 {}
impl ::core::clone::Clone for WOW64_LDT_ENTRY_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WOW64_LDT_ENTRY_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for WOW64_LDT_ENTRY_0_0 {}
impl ::core::clone::Clone for WOW64_LDT_ENTRY_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WOW64_LDT_ENTRY_0_1 {
    pub BaseMid: u8,
    pub Flags1: u8,
    pub Flags2: u8,
    pub BaseHi: u8,
}
impl ::core::marker::Copy for WOW64_LDT_ENTRY_0_1 {}
impl ::core::clone::Clone for WOW64_LDT_ENTRY_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub union XPF_MCE_FLAGS {
    pub Anonymous: XPF_MCE_FLAGS_0,
    pub AsULONG: u32,
}
impl ::core::marker::Copy for XPF_MCE_FLAGS {}
impl ::core::clone::Clone for XPF_MCE_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct XPF_MCE_FLAGS_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for XPF_MCE_FLAGS_0 {}
impl ::core::clone::Clone for XPF_MCE_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union XPF_MC_BANK_FLAGS {
    pub Anonymous: XPF_MC_BANK_FLAGS_0,
    pub AsUCHAR: u8,
}
impl ::core::marker::Copy for XPF_MC_BANK_FLAGS {}
impl ::core::clone::Clone for XPF_MC_BANK_FLAGS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPF_MC_BANK_FLAGS_0 {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for XPF_MC_BANK_FLAGS_0 {}
impl ::core::clone::Clone for XPF_MC_BANK_FLAGS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XSAVE_AREA {
    pub LegacyState: XSAVE_FORMAT,
    pub Header: XSAVE_AREA_HEADER,
}
impl ::core::marker::Copy for XSAVE_AREA {}
impl ::core::clone::Clone for XSAVE_AREA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XSAVE_AREA_HEADER {
    pub Mask: u64,
    pub CompactionMask: u64,
    pub Reserved2: [u64; 6],
}
impl ::core::marker::Copy for XSAVE_AREA_HEADER {}
impl ::core::clone::Clone for XSAVE_AREA_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct XSAVE_FORMAT {
    pub ControlWord: u16,
    pub StatusWord: u16,
    pub TagWord: u8,
    pub Reserved1: u8,
    pub ErrorOpcode: u16,
    pub ErrorOffset: u32,
    pub ErrorSelector: u16,
    pub Reserved2: u16,
    pub DataOffset: u32,
    pub DataSelector: u16,
    pub Reserved3: u16,
    pub MxCsr: u32,
    pub MxCsr_Mask: u32,
    pub FloatRegisters: [M128A; 8],
    pub XmmRegisters: [M128A; 16],
    pub Reserved4: [u8; 96],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for XSAVE_FORMAT {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for XSAVE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct XSAVE_FORMAT {
    pub ControlWord: u16,
    pub StatusWord: u16,
    pub TagWord: u8,
    pub Reserved1: u8,
    pub ErrorOpcode: u16,
    pub ErrorOffset: u32,
    pub ErrorSelector: u16,
    pub Reserved2: u16,
    pub DataOffset: u32,
    pub DataSelector: u16,
    pub Reserved3: u16,
    pub MxCsr: u32,
    pub MxCsr_Mask: u32,
    pub FloatRegisters: [M128A; 8],
    pub XmmRegisters: [M128A; 8],
    pub Reserved4: [u8; 224],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for XSAVE_FORMAT {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for XSAVE_FORMAT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XSTATE_CONFIGURATION {
    pub EnabledFeatures: u64,
    pub EnabledVolatileFeatures: u64,
    pub Size: u32,
    pub Anonymous: XSTATE_CONFIGURATION_0,
    pub Features: [XSTATE_FEATURE; 64],
    pub EnabledSupervisorFeatures: u64,
    pub AlignedFeatures: u64,
    pub AllFeatureSize: u32,
    pub AllFeatures: [u32; 64],
    pub EnabledUserVisibleSupervisorFeatures: u64,
    pub ExtendedFeatureDisableFeatures: u64,
    pub AllNonLargeFeatureSize: u32,
    pub Spare: u32,
}
impl ::core::marker::Copy for XSTATE_CONFIGURATION {}
impl ::core::clone::Clone for XSTATE_CONFIGURATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union XSTATE_CONFIGURATION_0 {
    pub ControlFlags: u32,
    pub Anonymous: XSTATE_CONFIGURATION_0_0,
}
impl ::core::marker::Copy for XSTATE_CONFIGURATION_0 {}
impl ::core::clone::Clone for XSTATE_CONFIGURATION_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XSTATE_CONFIGURATION_0_0 {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for XSTATE_CONFIGURATION_0_0 {}
impl ::core::clone::Clone for XSTATE_CONFIGURATION_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(4))]
pub struct XSTATE_CONFIG_FEATURE_MSC_INFO {
    pub SizeOfInfo: u32,
    pub ContextSize: u32,
    pub EnabledFeatures: u64,
    pub Features: [XSTATE_FEATURE; 64],
}
impl ::core::marker::Copy for XSTATE_CONFIG_FEATURE_MSC_INFO {}
impl ::core::clone::Clone for XSTATE_CONFIG_FEATURE_MSC_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct XSTATE_CONTEXT {
    pub Mask: u64,
    pub Length: u32,
    pub Reserved1: u32,
    pub Area: *mut XSAVE_AREA,
    pub Buffer: *mut ::core::ffi::c_void,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for XSTATE_CONTEXT {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for XSTATE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(target_arch = "x86")]
pub struct XSTATE_CONTEXT {
    pub Mask: u64,
    pub Length: u32,
    pub Reserved1: u32,
    pub Area: *mut XSAVE_AREA,
    pub Reserved2: u32,
    pub Buffer: *mut ::core::ffi::c_void,
    pub Reserved3: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for XSTATE_CONTEXT {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for XSTATE_CONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XSTATE_FEATURE {
    pub Offset: u32,
    pub Size: u32,
}
impl ::core::marker::Copy for XSTATE_FEATURE {}
impl ::core::clone::Clone for XSTATE_FEATURE {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DIGEST_FUNCTION = ::core::option::Option<unsafe extern "system" fn(refdata: *mut ::core::ffi::c_void, pdata: *mut u8, dwlength: u32) -> super::super::super::Foundation::BOOL>;
pub type LPCALL_BACK_USER_INTERRUPT_ROUTINE = ::core::option::Option<unsafe extern "system" fn() -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub type LPTOP_LEVEL_EXCEPTION_FILTER = ::core::option::Option<unsafe extern "system" fn(exceptioninfo: *const EXCEPTION_POINTERS) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Storage_FileSystem\"`, `\"Win32_System_Kernel\"`, `\"Win32_System_Memory\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Storage_FileSystem", feature = "Win32_System_Kernel", feature = "Win32_System_Memory"))]
pub type MINIDUMP_CALLBACK_ROUTINE = ::core::option::Option<unsafe extern "system" fn(callbackparam: *mut ::core::ffi::c_void, callbackinput: *const MINIDUMP_CALLBACK_INPUT, callbackoutput: *mut MINIDUMP_CALLBACK_OUTPUT) -> super::super::super::Foundation::BOOL>;
pub type PCOGETACTIVATIONSTATE = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::GUID, param1: u32, param2: *mut u32) -> ::windows_sys::core::HRESULT>;
pub type PCOGETCALLSTATE = ::core::option::Option<unsafe extern "system" fn(param0: i32, param1: *mut u32) -> ::windows_sys::core::HRESULT>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDBGHELP_CREATE_USER_DUMP_CALLBACK = ::core::option::Option<unsafe extern "system" fn(datatype: u32, data: *const *const ::core::ffi::c_void, datalength: *mut u32, userdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMDIRTREE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(filepath: ::windows_sys::core::PCSTR, callerdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMDIRTREE_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(filepath: ::windows_sys::core::PCWSTR, callerdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMLOADED_MODULES_CALLBACK = ::core::option::Option<unsafe extern "system" fn(modulename: ::windows_sys::core::PCSTR, modulebase: u32, modulesize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMLOADED_MODULES_CALLBACK64 = ::core::option::Option<unsafe extern "system" fn(modulename: ::windows_sys::core::PCSTR, modulebase: u64, modulesize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMLOADED_MODULES_CALLBACKW64 = ::core::option::Option<unsafe extern "system" fn(modulename: ::windows_sys::core::PCWSTR, modulebase: u64, modulesize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PENUMSOURCEFILETOKENSCALLBACK = ::core::option::Option<unsafe extern "system" fn(token: *const ::core::ffi::c_void, size: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFINDFILEINPATHCALLBACK = ::core::option::Option<unsafe extern "system" fn(filename: ::windows_sys::core::PCSTR, context: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFINDFILEINPATHCALLBACKW = ::core::option::Option<unsafe extern "system" fn(filename: ::windows_sys::core::PCWSTR, context: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFIND_DEBUG_FILE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(filehandle: super::super::super::Foundation::HANDLE, filename: ::windows_sys::core::PCSTR, callerdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFIND_DEBUG_FILE_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(filehandle: super::super::super::Foundation::HANDLE, filename: ::windows_sys::core::PCWSTR, callerdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFIND_EXE_FILE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(filehandle: super::super::super::Foundation::HANDLE, filename: ::windows_sys::core::PCSTR, callerdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFIND_EXE_FILE_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(filehandle: super::super::super::Foundation::HANDLE, filename: ::windows_sys::core::PCWSTR, callerdata: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PFUNCTION_TABLE_ACCESS_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, addrbase: u32) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PFUNCTION_TABLE_ACCESS_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(ahprocess: super::super::super::Foundation::HANDLE, addrbase: u64) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PGET_MODULE_BASE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, address: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PGET_MODULE_BASE_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, address: u64) -> u64>;
#[cfg(target_arch = "aarch64")]
pub type PGET_RUNTIME_FUNCTION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(controlpc: u64, context: *const ::core::ffi::c_void) -> *mut IMAGE_ARM64_RUNTIME_FUNCTION_ENTRY>;
#[cfg(target_arch = "x86_64")]
pub type PGET_RUNTIME_FUNCTION_CALLBACK = ::core::option::Option<unsafe extern "system" fn(controlpc: u64, context: *const ::core::ffi::c_void) -> *mut IMAGE_RUNTIME_FUNCTION_ENTRY>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PGET_TARGET_ATTRIBUTE_VALUE64 = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, attribute: u32, attributedata: u64, attributevalue: *mut u64) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PIMAGEHLP_STATUS_ROUTINE = ::core::option::Option<unsafe extern "system" fn(reason: IMAGEHLP_STATUS_REASON, imagename: ::windows_sys::core::PCSTR, dllname: ::windows_sys::core::PCSTR, va: usize, parameter: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PIMAGEHLP_STATUS_ROUTINE32 = ::core::option::Option<unsafe extern "system" fn(reason: IMAGEHLP_STATUS_REASON, imagename: ::windows_sys::core::PCSTR, dllname: ::windows_sys::core::PCSTR, va: u32, parameter: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PIMAGEHLP_STATUS_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(reason: IMAGEHLP_STATUS_REASON, imagename: ::windows_sys::core::PCSTR, dllname: ::windows_sys::core::PCSTR, va: u64, parameter: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PREAD_PROCESS_MEMORY_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, lpbaseaddress: u32, lpbuffer: *mut ::core::ffi::c_void, nsize: u32, lpnumberofbytesread: *mut u32) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PREAD_PROCESS_MEMORY_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, qwbaseaddress: u64, lpbuffer: *mut ::core::ffi::c_void, nsize: u32, lpnumberofbytesread: *mut u32) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERBYINDEXPROC = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: ::windows_sys::core::PCSTR, param3: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERBYINDEXPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: ::windows_sys::core::PCSTR, param3: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERBYINDEXPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: ::windows_sys::core::PCWSTR, param3: ::windows_sys::core::PCWSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERCALLBACKPROC = ::core::option::Option<unsafe extern "system" fn(action: usize, data: u64, context: u64) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERCLOSEPROC = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERDELTANAME = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: *mut ::core::ffi::c_void, param2: u32, param3: u32, param4: *mut ::core::ffi::c_void, param5: u32, param6: u32, param7: ::windows_sys::core::PCSTR, param8: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERDELTANAMEW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: *mut ::core::ffi::c_void, param2: u32, param3: u32, param4: *mut ::core::ffi::c_void, param5: u32, param6: u32, param7: ::windows_sys::core::PCWSTR, param8: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERGETINDEXSTRING = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void, param1: u32, param2: u32, param3: ::windows_sys::core::PCSTR, param4: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERGETINDEXSTRINGW = ::core::option::Option<unsafe extern "system" fn(param0: *mut ::core::ffi::c_void, param1: u32, param2: u32, param3: ::windows_sys::core::PCWSTR, param4: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERGETOPTIONDATAPROC = ::core::option::Option<unsafe extern "system" fn(param0: usize, param1: *mut u64) -> super::super::super::Foundation::BOOL>;
pub type PSYMBOLSERVERGETOPTIONSPROC = ::core::option::Option<unsafe extern "system" fn() -> usize>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERGETSUPPLEMENT = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: ::windows_sys::core::PCSTR, param3: ::windows_sys::core::PCSTR, param4: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERGETSUPPLEMENTW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: ::windows_sys::core::PCWSTR, param3: ::windows_sys::core::PCWSTR, param4: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERGETVERSION = ::core::option::Option<unsafe extern "system" fn(param0: *mut API_VERSION) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERISSTORE = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERISSTOREW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERMESSAGEPROC = ::core::option::Option<unsafe extern "system" fn(action: usize, data: u64, context: u64) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVEROPENPROC = ::core::option::Option<unsafe extern "system" fn() -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPINGPROC = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPINGPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPINGPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPINGPROCWEX = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPROC = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: *mut ::core::ffi::c_void, param3: u32, param4: u32, param5: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: *mut ::core::ffi::c_void, param3: u32, param4: u32, param5: ::windows_sys::core::PCSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut ::core::ffi::c_void, param3: u32, param4: u32, param5: ::windows_sys::core::PCWSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSETHTTPAUTHHEADER = ::core::option::Option<unsafe extern "system" fn(pszauthheader: ::windows_sys::core::PCWSTR) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSETOPTIONSPROC = ::core::option::Option<unsafe extern "system" fn(param0: usize, param1: u64) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSETOPTIONSWPROC = ::core::option::Option<unsafe extern "system" fn(param0: usize, param1: u64) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSTOREFILE = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: *mut ::core::ffi::c_void, param3: u32, param4: u32, param5: ::windows_sys::core::PCSTR, param6: usize, param7: u32) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSTOREFILEW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut ::core::ffi::c_void, param3: u32, param4: u32, param5: ::windows_sys::core::PCWSTR, param6: usize, param7: u32) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSTORESUPPLEMENT = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: ::windows_sys::core::PCSTR, param2: ::windows_sys::core::PCSTR, param3: ::windows_sys::core::PCSTR, param4: usize, param5: u32) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERSTORESUPPLEMENTW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: ::windows_sys::core::PCWSTR, param3: ::windows_sys::core::PCWSTR, param4: usize, param5: u32) -> super::super::super::Foundation::BOOL>;
pub type PSYMBOLSERVERVERSION = ::core::option::Option<unsafe extern "system" fn() -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOLSERVERWEXPROC = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: ::windows_sys::core::PCWSTR, param2: *mut ::core::ffi::c_void, param3: u32, param4: u32, param5: ::windows_sys::core::PCWSTR, param6: *mut SYMSRV_EXTENDED_OUTPUT_DATA) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOL_FUNCENTRY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, addrbase: u32, usercontext: *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOL_FUNCENTRY_CALLBACK64 = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, addrbase: u64, usercontext: u64) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOL_REGISTERED_CALLBACK = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, actioncode: u32, callbackdata: *const ::core::ffi::c_void, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYMBOL_REGISTERED_CALLBACK64 = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, actioncode: u32, callbackdata: u64, usercontext: u64) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMERATESYMBOLS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(psyminfo: *const SYMBOL_INFO, symbolsize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMERATESYMBOLS_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(psyminfo: *const SYMBOL_INFOW, symbolsize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMLINES_CALLBACK = ::core::option::Option<unsafe extern "system" fn(lineinfo: *const SRCCODEINFO, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMLINES_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(lineinfo: *const SRCCODEINFOW, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMMODULES_CALLBACK = ::core::option::Option<unsafe extern "system" fn(modulename: ::windows_sys::core::PCSTR, baseofdll: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMMODULES_CALLBACK64 = ::core::option::Option<unsafe extern "system" fn(modulename: ::windows_sys::core::PCSTR, baseofdll: u64, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMMODULES_CALLBACKW64 = ::core::option::Option<unsafe extern "system" fn(modulename: ::windows_sys::core::PCWSTR, baseofdll: u64, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMPROCESSES_CALLBACK = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMSOURCEFILES_CALLBACK = ::core::option::Option<unsafe extern "system" fn(psourcefile: *const SOURCEFILE, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMSOURCEFILES_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(psourcefile: *const SOURCEFILEW, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMSYMBOLS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(symbolname: ::windows_sys::core::PCSTR, symboladdress: u32, symbolsize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMSYMBOLS_CALLBACK64 = ::core::option::Option<unsafe extern "system" fn(symbolname: ::windows_sys::core::PCSTR, symboladdress: u64, symbolsize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMSYMBOLS_CALLBACK64W = ::core::option::Option<unsafe extern "system" fn(symbolname: ::windows_sys::core::PCWSTR, symboladdress: u64, symbolsize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PSYM_ENUMSYMBOLS_CALLBACKW = ::core::option::Option<unsafe extern "system" fn(symbolname: ::windows_sys::core::PCWSTR, symboladdress: u32, symbolsize: u32, usercontext: *const ::core::ffi::c_void) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub type PTRANSLATE_ADDRESS_ROUTINE = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, hthread: super::super::super::Foundation::HANDLE, lpaddr: *mut ADDRESS) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PTRANSLATE_ADDRESS_ROUTINE64 = ::core::option::Option<unsafe extern "system" fn(hprocess: super::super::super::Foundation::HANDLE, hthread: super::super::super::Foundation::HANDLE, lpaddr: *const ADDRESS64) -> u64>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Kernel\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Kernel"))]
pub type PVECTORED_EXCEPTION_HANDLER = ::core::option::Option<unsafe extern "system" fn(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWAITCHAINCALLBACK = ::core::option::Option<unsafe extern "system" fn(wcthandle: *mut ::core::ffi::c_void, context: usize, callbackstatus: u32, nodecount: *mut u32, nodeinfoarray: *mut WAITCHAIN_NODE_INFO, iscycle: *mut super::super::super::Foundation::BOOL) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type SYMADDSOURCESTREAM = ::core::option::Option<unsafe extern "system" fn(param0: super::super::super::Foundation::HANDLE, param1: u64, param2: ::windows_sys::core::PCSTR, param3: *mut u8, param4: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type SYMADDSOURCESTREAMA = ::core::option::Option<unsafe extern "system" fn(param0: super::super::super::Foundation::HANDLE, param1: u64, param2: ::windows_sys::core::PCSTR, param3: *mut u8, param4: usize) -> super::super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WHEA_ERROR_SOURCE_CORRECT_DEVICE_DRIVER = ::core::option::Option<unsafe extern "system" fn(errorsourcedesc: *mut ::core::ffi::c_void, maximumsectionlength: *mut u32) -> super::super::super::Foundation::NTSTATUS>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type WHEA_ERROR_SOURCE_INITIALIZE_DEVICE_DRIVER = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void, errorsourceid: u32) -> super::super::super::Foundation::NTSTATUS>;
pub type WHEA_ERROR_SOURCE_UNINITIALIZE_DEVICE_DRIVER = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void) -> ()>;
