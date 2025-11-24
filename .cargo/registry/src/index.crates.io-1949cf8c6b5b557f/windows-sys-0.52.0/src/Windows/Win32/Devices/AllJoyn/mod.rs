#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynAcceptBusConnection(serverbushandle : super::super::Foundation:: HANDLE, abortevent : super::super::Foundation:: HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynCloseBusHandle(bushandle : super::super::Foundation:: HANDLE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynConnectToBus(connectionspec : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: HANDLE);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"] fn AllJoynCreateBus(outbuffersize : u32, inbuffersize : u32, lpsecurityattributes : *const super::super::Security:: SECURITY_ATTRIBUTES) -> super::super::Foundation:: HANDLE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynEnumEvents(connectedbushandle : super::super::Foundation:: HANDLE, eventtoreset : super::super::Foundation:: HANDLE, eventtypes : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynEventSelect(connectedbushandle : super::super::Foundation:: HANDLE, eventhandle : super::super::Foundation:: HANDLE, eventtypes : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynReceiveFromBus(connectedbushandle : super::super::Foundation:: HANDLE, buffer : *mut ::core::ffi::c_void, bytestoread : u32, bytestransferred : *mut u32, reserved : *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("msajapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AllJoynSendToBus(connectedbushandle : super::super::Foundation:: HANDLE, buffer : *const ::core::ffi::c_void, bytestowrite : u32, bytestransferred : *mut u32, reserved : *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
::windows_targets::link!("msajapi.dll" "system" fn QCC_StatusText(status : QStatus) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_create(defaultlanguage : ::windows_sys::core::PCSTR) -> alljoyn_aboutdata);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_create_empty() -> alljoyn_aboutdata);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_create_full(arg : alljoyn_msgarg, language : ::windows_sys::core::PCSTR) -> alljoyn_aboutdata);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_createfrommsgarg(data : alljoyn_aboutdata, arg : alljoyn_msgarg, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_createfromxml(data : alljoyn_aboutdata, aboutdataxml : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_destroy(data : alljoyn_aboutdata) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getaboutdata(data : alljoyn_aboutdata, msgarg : alljoyn_msgarg, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getajsoftwareversion(data : alljoyn_aboutdata, ajsoftwareversion : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getannouncedaboutdata(data : alljoyn_aboutdata, msgarg : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getappid(data : alljoyn_aboutdata, appid : *mut *mut u8, num : *mut usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getappname(data : alljoyn_aboutdata, appname : *mut *mut i8, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getdateofmanufacture(data : alljoyn_aboutdata, dateofmanufacture : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getdefaultlanguage(data : alljoyn_aboutdata, defaultlanguage : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getdescription(data : alljoyn_aboutdata, description : *mut *mut i8, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getdeviceid(data : alljoyn_aboutdata, deviceid : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getdevicename(data : alljoyn_aboutdata, devicename : *mut *mut i8, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getfield(data : alljoyn_aboutdata, name : ::windows_sys::core::PCSTR, value : *mut alljoyn_msgarg, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getfields(data : alljoyn_aboutdata, fields : *const *const i8, num_fields : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getfieldsignature(data : alljoyn_aboutdata, fieldname : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_gethardwareversion(data : alljoyn_aboutdata, hardwareversion : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getmanufacturer(data : alljoyn_aboutdata, manufacturer : *mut *mut i8, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getmodelnumber(data : alljoyn_aboutdata, modelnumber : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getsoftwareversion(data : alljoyn_aboutdata, softwareversion : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getsupportedlanguages(data : alljoyn_aboutdata, languagetags : *const *const i8, num : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_getsupporturl(data : alljoyn_aboutdata, supporturl : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_isfieldannounced(data : alljoyn_aboutdata, fieldname : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_isfieldlocalized(data : alljoyn_aboutdata, fieldname : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_isfieldrequired(data : alljoyn_aboutdata, fieldname : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_isvalid(data : alljoyn_aboutdata, language : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setappid(data : alljoyn_aboutdata, appid : *const u8, num : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setappid_fromstring(data : alljoyn_aboutdata, appid : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setappname(data : alljoyn_aboutdata, appname : ::windows_sys::core::PCSTR, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setdateofmanufacture(data : alljoyn_aboutdata, dateofmanufacture : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setdefaultlanguage(data : alljoyn_aboutdata, defaultlanguage : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setdescription(data : alljoyn_aboutdata, description : ::windows_sys::core::PCSTR, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setdeviceid(data : alljoyn_aboutdata, deviceid : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setdevicename(data : alljoyn_aboutdata, devicename : ::windows_sys::core::PCSTR, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setfield(data : alljoyn_aboutdata, name : ::windows_sys::core::PCSTR, value : alljoyn_msgarg, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_sethardwareversion(data : alljoyn_aboutdata, hardwareversion : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setmanufacturer(data : alljoyn_aboutdata, manufacturer : ::windows_sys::core::PCSTR, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setmodelnumber(data : alljoyn_aboutdata, modelnumber : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setsoftwareversion(data : alljoyn_aboutdata, softwareversion : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setsupportedlanguage(data : alljoyn_aboutdata, language : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdata_setsupporturl(data : alljoyn_aboutdata, supporturl : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdatalistener_create(callbacks : *const alljoyn_aboutdatalistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_aboutdatalistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutdatalistener_destroy(listener : alljoyn_aboutdatalistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_clear(icon : alljoyn_abouticon) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_create() -> alljoyn_abouticon);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_destroy(icon : alljoyn_abouticon) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_getcontent(icon : alljoyn_abouticon, data : *const *const u8, size : *mut usize) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_geturl(icon : alljoyn_abouticon, r#type : *const *const i8, url : *const *const i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_setcontent(icon : alljoyn_abouticon, r#type : ::windows_sys::core::PCSTR, data : *mut u8, csize : usize, ownsdata : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_setcontent_frommsgarg(icon : alljoyn_abouticon, arg : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticon_seturl(icon : alljoyn_abouticon, r#type : ::windows_sys::core::PCSTR, url : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticonobj_create(bus : alljoyn_busattachment, icon : alljoyn_abouticon) -> alljoyn_abouticonobj);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticonobj_destroy(icon : alljoyn_abouticonobj) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticonproxy_create(bus : alljoyn_busattachment, busname : ::windows_sys::core::PCSTR, sessionid : u32) -> alljoyn_abouticonproxy);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticonproxy_destroy(proxy : alljoyn_abouticonproxy) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticonproxy_geticon(proxy : alljoyn_abouticonproxy, icon : alljoyn_abouticon) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_abouticonproxy_getversion(proxy : alljoyn_abouticonproxy, version : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutlistener_create(callback : *const alljoyn_aboutlistener_callback, context : *const ::core::ffi::c_void) -> alljoyn_aboutlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutlistener_destroy(listener : alljoyn_aboutlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobj_announce(obj : alljoyn_aboutobj, sessionport : u16, aboutdata : alljoyn_aboutdata) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobj_announce_using_datalistener(obj : alljoyn_aboutobj, sessionport : u16, aboutlistener : alljoyn_aboutdatalistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobj_create(bus : alljoyn_busattachment, isannounced : alljoyn_about_announceflag) -> alljoyn_aboutobj);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobj_destroy(obj : alljoyn_aboutobj) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobj_unannounce(obj : alljoyn_aboutobj) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_clear(description : alljoyn_aboutobjectdescription) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_create() -> alljoyn_aboutobjectdescription);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_create_full(arg : alljoyn_msgarg) -> alljoyn_aboutobjectdescription);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_createfrommsgarg(description : alljoyn_aboutobjectdescription, arg : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_destroy(description : alljoyn_aboutobjectdescription) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_getinterfacepaths(description : alljoyn_aboutobjectdescription, interfacename : ::windows_sys::core::PCSTR, paths : *const *const i8, numpaths : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_getinterfaces(description : alljoyn_aboutobjectdescription, path : ::windows_sys::core::PCSTR, interfaces : *const *const i8, numinterfaces : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_getmsgarg(description : alljoyn_aboutobjectdescription, msgarg : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_getpaths(description : alljoyn_aboutobjectdescription, paths : *const *const i8, numpaths : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_hasinterface(description : alljoyn_aboutobjectdescription, interfacename : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_hasinterfaceatpath(description : alljoyn_aboutobjectdescription, path : ::windows_sys::core::PCSTR, interfacename : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutobjectdescription_haspath(description : alljoyn_aboutobjectdescription, path : ::windows_sys::core::PCSTR) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutproxy_create(bus : alljoyn_busattachment, busname : ::windows_sys::core::PCSTR, sessionid : u32) -> alljoyn_aboutproxy);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutproxy_destroy(proxy : alljoyn_aboutproxy) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutproxy_getaboutdata(proxy : alljoyn_aboutproxy, language : ::windows_sys::core::PCSTR, data : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutproxy_getobjectdescription(proxy : alljoyn_aboutproxy, objectdesc : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_aboutproxy_getversion(proxy : alljoyn_aboutproxy, version : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_applicationstatelistener_create(callbacks : *const alljoyn_applicationstatelistener_callbacks, context : *mut ::core::ffi::c_void) -> alljoyn_applicationstatelistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_applicationstatelistener_destroy(listener : alljoyn_applicationstatelistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistener_create(callbacks : *const alljoyn_authlistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_authlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistener_destroy(listener : alljoyn_authlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistener_requestcredentialsresponse(listener : alljoyn_authlistener, authcontext : *mut ::core::ffi::c_void, accept : i32, credentials : alljoyn_credentials) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistener_setsharedsecret(listener : alljoyn_authlistener, sharedsecret : *const u8, sharedsecretsize : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistener_verifycredentialsresponse(listener : alljoyn_authlistener, authcontext : *mut ::core::ffi::c_void, accept : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistenerasync_create(callbacks : *const alljoyn_authlistenerasync_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_authlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_authlistenerasync_destroy(listener : alljoyn_authlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_adddestination(autopinger : alljoyn_autopinger, group : ::windows_sys::core::PCSTR, destination : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_addpinggroup(autopinger : alljoyn_autopinger, group : ::windows_sys::core::PCSTR, listener : alljoyn_pinglistener, pinginterval : u32) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_create(bus : alljoyn_busattachment) -> alljoyn_autopinger);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_destroy(autopinger : alljoyn_autopinger) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_pause(autopinger : alljoyn_autopinger) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_removedestination(autopinger : alljoyn_autopinger, group : ::windows_sys::core::PCSTR, destination : ::windows_sys::core::PCSTR, removeall : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_removepinggroup(autopinger : alljoyn_autopinger, group : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_resume(autopinger : alljoyn_autopinger) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_autopinger_setpinginterval(autopinger : alljoyn_autopinger, group : ::windows_sys::core::PCSTR, pinginterval : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_addlogonentry(bus : alljoyn_busattachment, authmechanism : ::windows_sys::core::PCSTR, username : ::windows_sys::core::PCSTR, password : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_addmatch(bus : alljoyn_busattachment, rule : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_advertisename(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, transports : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_bindsessionport(bus : alljoyn_busattachment, sessionport : *mut u16, opts : alljoyn_sessionopts, listener : alljoyn_sessionportlistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_canceladvertisename(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, transports : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_cancelfindadvertisedname(bus : alljoyn_busattachment, nameprefix : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_cancelfindadvertisednamebytransport(bus : alljoyn_busattachment, nameprefix : ::windows_sys::core::PCSTR, transports : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_cancelwhoimplements_interface(bus : alljoyn_busattachment, implementsinterface : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_cancelwhoimplements_interfaces(bus : alljoyn_busattachment, implementsinterfaces : *const *const i8, numberinterfaces : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_clearkeys(bus : alljoyn_busattachment, guid : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_clearkeystore(bus : alljoyn_busattachment) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_connect(bus : alljoyn_busattachment, connectspec : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_create(applicationname : ::windows_sys::core::PCSTR, allowremotemessages : i32) -> alljoyn_busattachment);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_create_concurrency(applicationname : ::windows_sys::core::PCSTR, allowremotemessages : i32, concurrency : u32) -> alljoyn_busattachment);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_createinterface(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, iface : *mut alljoyn_interfacedescription) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_createinterface_secure(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, iface : *mut alljoyn_interfacedescription, secpolicy : alljoyn_interfacedescription_securitypolicy) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_createinterfacesfromxml(bus : alljoyn_busattachment, xml : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_deletedefaultkeystore(applicationname : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_deleteinterface(bus : alljoyn_busattachment, iface : alljoyn_interfacedescription) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_destroy(bus : alljoyn_busattachment) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_disconnect(bus : alljoyn_busattachment, unused : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_enableconcurrentcallbacks(bus : alljoyn_busattachment) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_enablepeersecurity(bus : alljoyn_busattachment, authmechanisms : ::windows_sys::core::PCSTR, listener : alljoyn_authlistener, keystorefilename : ::windows_sys::core::PCSTR, isshared : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_enablepeersecuritywithpermissionconfigurationlistener(bus : alljoyn_busattachment, authmechanisms : ::windows_sys::core::PCSTR, authlistener : alljoyn_authlistener, keystorefilename : ::windows_sys::core::PCSTR, isshared : i32, permissionconfigurationlistener : alljoyn_permissionconfigurationlistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_findadvertisedname(bus : alljoyn_busattachment, nameprefix : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_findadvertisednamebytransport(bus : alljoyn_busattachment, nameprefix : ::windows_sys::core::PCSTR, transports : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getalljoyndebugobj(bus : alljoyn_busattachment) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getalljoynproxyobj(bus : alljoyn_busattachment) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getconcurrency(bus : alljoyn_busattachment) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getconnectspec(bus : alljoyn_busattachment) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getdbusproxyobj(bus : alljoyn_busattachment) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getglobalguidstring(bus : alljoyn_busattachment) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getinterface(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR) -> alljoyn_interfacedescription);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getinterfaces(bus : alljoyn_busattachment, ifaces : *const alljoyn_interfacedescription, numifaces : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getkeyexpiration(bus : alljoyn_busattachment, guid : ::windows_sys::core::PCSTR, timeout : *mut u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getpeerguid(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, guid : ::windows_sys::core::PCSTR, guidsz : *mut usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getpermissionconfigurator(bus : alljoyn_busattachment) -> alljoyn_permissionconfigurator);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_gettimestamp() -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_getuniquename(bus : alljoyn_busattachment) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_isconnected(bus : alljoyn_busattachment) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_ispeersecurityenabled(bus : alljoyn_busattachment) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_isstarted(bus : alljoyn_busattachment) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_isstopping(bus : alljoyn_busattachment) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_join(bus : alljoyn_busattachment) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_joinsession(bus : alljoyn_busattachment, sessionhost : ::windows_sys::core::PCSTR, sessionport : u16, listener : alljoyn_sessionlistener, sessionid : *mut u32, opts : alljoyn_sessionopts) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_joinsessionasync(bus : alljoyn_busattachment, sessionhost : ::windows_sys::core::PCSTR, sessionport : u16, listener : alljoyn_sessionlistener, opts : alljoyn_sessionopts, callback : alljoyn_busattachment_joinsessioncb_ptr, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_leavesession(bus : alljoyn_busattachment, sessionid : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_namehasowner(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, hasowner : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_ping(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, timeout : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registeraboutlistener(bus : alljoyn_busattachment, aboutlistener : alljoyn_aboutlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registerapplicationstatelistener(bus : alljoyn_busattachment, listener : alljoyn_applicationstatelistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registerbuslistener(bus : alljoyn_busattachment, listener : alljoyn_buslistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registerbusobject(bus : alljoyn_busattachment, obj : alljoyn_busobject) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registerbusobject_secure(bus : alljoyn_busattachment, obj : alljoyn_busobject) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registerkeystorelistener(bus : alljoyn_busattachment, listener : alljoyn_keystorelistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registersignalhandler(bus : alljoyn_busattachment, signal_handler : alljoyn_messagereceiver_signalhandler_ptr, member : alljoyn_interfacedescription_member, srcpath : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_registersignalhandlerwithrule(bus : alljoyn_busattachment, signal_handler : alljoyn_messagereceiver_signalhandler_ptr, member : alljoyn_interfacedescription_member, matchrule : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_releasename(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_reloadkeystore(bus : alljoyn_busattachment) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_removematch(bus : alljoyn_busattachment, rule : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_removesessionmember(bus : alljoyn_busattachment, sessionid : u32, membername : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_requestname(bus : alljoyn_busattachment, requestedname : ::windows_sys::core::PCSTR, flags : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_secureconnection(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, forceauth : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_secureconnectionasync(bus : alljoyn_busattachment, name : ::windows_sys::core::PCSTR, forceauth : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_setdaemondebug(bus : alljoyn_busattachment, module : ::windows_sys::core::PCSTR, level : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_setkeyexpiration(bus : alljoyn_busattachment, guid : ::windows_sys::core::PCSTR, timeout : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_setlinktimeout(bus : alljoyn_busattachment, sessionid : u32, linktimeout : *mut u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_setlinktimeoutasync(bus : alljoyn_busattachment, sessionid : u32, linktimeout : u32, callback : alljoyn_busattachment_setlinktimeoutcb_ptr, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_setsessionlistener(bus : alljoyn_busattachment, sessionid : u32, listener : alljoyn_sessionlistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_start(bus : alljoyn_busattachment) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_stop(bus : alljoyn_busattachment) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unbindsessionport(bus : alljoyn_busattachment, sessionport : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregisteraboutlistener(bus : alljoyn_busattachment, aboutlistener : alljoyn_aboutlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregisterallaboutlisteners(bus : alljoyn_busattachment) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregisterallhandlers(bus : alljoyn_busattachment) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregisterapplicationstatelistener(bus : alljoyn_busattachment, listener : alljoyn_applicationstatelistener) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregisterbuslistener(bus : alljoyn_busattachment, listener : alljoyn_buslistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregisterbusobject(bus : alljoyn_busattachment, object : alljoyn_busobject) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregistersignalhandler(bus : alljoyn_busattachment, signal_handler : alljoyn_messagereceiver_signalhandler_ptr, member : alljoyn_interfacedescription_member, srcpath : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_unregistersignalhandlerwithrule(bus : alljoyn_busattachment, signal_handler : alljoyn_messagereceiver_signalhandler_ptr, member : alljoyn_interfacedescription_member, matchrule : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_whoimplements_interface(bus : alljoyn_busattachment, implementsinterface : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busattachment_whoimplements_interfaces(bus : alljoyn_busattachment, implementsinterfaces : *const *const i8, numberinterfaces : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_buslistener_create(callbacks : *const alljoyn_buslistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_buslistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_buslistener_destroy(listener : alljoyn_buslistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_addinterface(bus : alljoyn_busobject, iface : alljoyn_interfacedescription) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_addinterface_announced(bus : alljoyn_busobject, iface : alljoyn_interfacedescription) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_addmethodhandler(bus : alljoyn_busobject, member : alljoyn_interfacedescription_member, handler : alljoyn_messagereceiver_methodhandler_ptr, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_addmethodhandlers(bus : alljoyn_busobject, entries : *const alljoyn_busobject_methodentry, numentries : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_cancelsessionlessmessage(bus : alljoyn_busobject, msg : alljoyn_message) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_cancelsessionlessmessage_serial(bus : alljoyn_busobject, serialnumber : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_create(path : ::windows_sys::core::PCSTR, isplaceholder : i32, callbacks_in : *const alljoyn_busobject_callbacks, context_in : *const ::core::ffi::c_void) -> alljoyn_busobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_destroy(bus : alljoyn_busobject) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_emitpropertieschanged(bus : alljoyn_busobject, ifcname : ::windows_sys::core::PCSTR, propnames : *const *const i8, numprops : usize, id : u32) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_emitpropertychanged(bus : alljoyn_busobject, ifcname : ::windows_sys::core::PCSTR, propname : ::windows_sys::core::PCSTR, val : alljoyn_msgarg, id : u32) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_getannouncedinterfacenames(bus : alljoyn_busobject, interfaces : *const *const i8, numinterfaces : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_getbusattachment(bus : alljoyn_busobject) -> alljoyn_busattachment);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_getname(bus : alljoyn_busobject, buffer : ::windows_sys::core::PCSTR, buffersz : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_getpath(bus : alljoyn_busobject) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_issecure(bus : alljoyn_busobject) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_methodreply_args(bus : alljoyn_busobject, msg : alljoyn_message, args : alljoyn_msgarg, numargs : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_methodreply_err(bus : alljoyn_busobject, msg : alljoyn_message, error : ::windows_sys::core::PCSTR, errormessage : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_methodreply_status(bus : alljoyn_busobject, msg : alljoyn_message, status : QStatus) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_setannounceflag(bus : alljoyn_busobject, iface : alljoyn_interfacedescription, isannounced : alljoyn_about_announceflag) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_busobject_signal(bus : alljoyn_busobject, destination : ::windows_sys::core::PCSTR, sessionid : u32, signal : alljoyn_interfacedescription_member, args : alljoyn_msgarg, numargs : usize, timetolive : u16, flags : u8, msg : alljoyn_message) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_clear(cred : alljoyn_credentials) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_create() -> alljoyn_credentials);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_destroy(cred : alljoyn_credentials) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_getcertchain(cred : alljoyn_credentials) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_getexpiration(cred : alljoyn_credentials) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_getlogonentry(cred : alljoyn_credentials) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_getpassword(cred : alljoyn_credentials) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_getprivateKey(cred : alljoyn_credentials) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_getusername(cred : alljoyn_credentials) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_isset(cred : alljoyn_credentials, creds : u16) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_setcertchain(cred : alljoyn_credentials, certchain : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_setexpiration(cred : alljoyn_credentials, expiration : u32) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_setlogonentry(cred : alljoyn_credentials, logonentry : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_setpassword(cred : alljoyn_credentials, pwd : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_setprivatekey(cred : alljoyn_credentials, pk : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_credentials_setusername(cred : alljoyn_credentials, username : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_getbuildinfo() -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_getnumericversion() -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_getversion() -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_init() -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_activate(iface : alljoyn_interfacedescription) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addannotation(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addargannotation(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, argname : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addmember(iface : alljoyn_interfacedescription, r#type : alljoyn_messagetype, name : ::windows_sys::core::PCSTR, inputsig : ::windows_sys::core::PCSTR, outsig : ::windows_sys::core::PCSTR, argnames : ::windows_sys::core::PCSTR, annotation : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addmemberannotation(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addmethod(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, inputsig : ::windows_sys::core::PCSTR, outsig : ::windows_sys::core::PCSTR, argnames : ::windows_sys::core::PCSTR, annotation : u8, accessperms : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addproperty(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, signature : ::windows_sys::core::PCSTR, access : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addpropertyannotation(iface : alljoyn_interfacedescription, property : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_addsignal(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, sig : ::windows_sys::core::PCSTR, argnames : ::windows_sys::core::PCSTR, annotation : u8, accessperms : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_eql(one : alljoyn_interfacedescription, other : alljoyn_interfacedescription) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getannotation(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getannotationatindex(iface : alljoyn_interfacedescription, index : usize, name : ::windows_sys::core::PCSTR, name_size : *mut usize, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getannotationscount(iface : alljoyn_interfacedescription) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getargdescriptionforlanguage(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, arg : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR, maxlanguagelength : usize, languagetag : ::windows_sys::core::PCSTR) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getdescriptionforlanguage(iface : alljoyn_interfacedescription, description : ::windows_sys::core::PCSTR, maxlanguagelength : usize, languagetag : ::windows_sys::core::PCSTR) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getdescriptionlanguages(iface : alljoyn_interfacedescription, languages : *const *const i8, size : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getdescriptionlanguages2(iface : alljoyn_interfacedescription, languages : ::windows_sys::core::PCSTR, languagessize : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getdescriptiontranslationcallback(iface : alljoyn_interfacedescription) -> alljoyn_interfacedescription_translation_callback_ptr);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getmember(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, member : *mut alljoyn_interfacedescription_member) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getmemberannotation(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getmemberargannotation(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, argname : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getmemberdescriptionforlanguage(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR, maxlanguagelength : usize, languagetag : ::windows_sys::core::PCSTR) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getmembers(iface : alljoyn_interfacedescription, members : *mut alljoyn_interfacedescription_member, nummembers : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getmethod(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, member : *mut alljoyn_interfacedescription_member) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getname(iface : alljoyn_interfacedescription) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getproperties(iface : alljoyn_interfacedescription, props : *mut alljoyn_interfacedescription_property, numprops : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getproperty(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, property : *mut alljoyn_interfacedescription_property) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getpropertyannotation(iface : alljoyn_interfacedescription, property : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, str_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getpropertydescriptionforlanguage(iface : alljoyn_interfacedescription, property : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR, maxlanguagelength : usize, languagetag : ::windows_sys::core::PCSTR) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getsecuritypolicy(iface : alljoyn_interfacedescription) -> alljoyn_interfacedescription_securitypolicy);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_getsignal(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, member : *mut alljoyn_interfacedescription_member) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_hasdescription(iface : alljoyn_interfacedescription) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_hasmember(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, insig : ::windows_sys::core::PCSTR, outsig : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_hasproperties(iface : alljoyn_interfacedescription) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_hasproperty(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_introspect(iface : alljoyn_interfacedescription, str : ::windows_sys::core::PCSTR, buf : usize, indent : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_issecure(iface : alljoyn_interfacedescription) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_eql(one : alljoyn_interfacedescription_member, other : alljoyn_interfacedescription_member) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_getannotation(member : alljoyn_interfacedescription_member, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_getannotationatindex(member : alljoyn_interfacedescription_member, index : usize, name : ::windows_sys::core::PCSTR, name_size : *mut usize, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_getannotationscount(member : alljoyn_interfacedescription_member) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_getargannotation(member : alljoyn_interfacedescription_member, argname : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_getargannotationatindex(member : alljoyn_interfacedescription_member, argname : ::windows_sys::core::PCSTR, index : usize, name : ::windows_sys::core::PCSTR, name_size : *mut usize, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_member_getargannotationscount(member : alljoyn_interfacedescription_member, argname : ::windows_sys::core::PCSTR) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_property_eql(one : alljoyn_interfacedescription_property, other : alljoyn_interfacedescription_property) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_property_getannotation(property : alljoyn_interfacedescription_property, name : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_property_getannotationatindex(property : alljoyn_interfacedescription_property, index : usize, name : ::windows_sys::core::PCSTR, name_size : *mut usize, value : ::windows_sys::core::PCSTR, value_size : *mut usize) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_property_getannotationscount(property : alljoyn_interfacedescription_property) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setargdescription(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, argname : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setargdescriptionforlanguage(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, arg : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR, languagetag : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setdescription(iface : alljoyn_interfacedescription, description : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setdescriptionforlanguage(iface : alljoyn_interfacedescription, description : ::windows_sys::core::PCSTR, languagetag : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setdescriptionlanguage(iface : alljoyn_interfacedescription, language : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setdescriptiontranslationcallback(iface : alljoyn_interfacedescription, translationcallback : alljoyn_interfacedescription_translation_callback_ptr) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setmemberdescription(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setmemberdescriptionforlanguage(iface : alljoyn_interfacedescription, member : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR, languagetag : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setpropertydescription(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_interfacedescription_setpropertydescriptionforlanguage(iface : alljoyn_interfacedescription, name : ::windows_sys::core::PCSTR, description : ::windows_sys::core::PCSTR, languagetag : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_keystorelistener_create(callbacks : *const alljoyn_keystorelistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_keystorelistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_keystorelistener_destroy(listener : alljoyn_keystorelistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_keystorelistener_getkeys(listener : alljoyn_keystorelistener, keystore : alljoyn_keystore, sink : ::windows_sys::core::PCSTR, sink_sz : *mut usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_keystorelistener_putkeys(listener : alljoyn_keystorelistener, keystore : alljoyn_keystore, source : ::windows_sys::core::PCSTR, password : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_keystorelistener_with_synchronization_create(callbacks : *const alljoyn_keystorelistener_with_synchronization_callbacks, context : *mut ::core::ffi::c_void) -> alljoyn_keystorelistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_create(bus : alljoyn_busattachment) -> alljoyn_message);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_description(msg : alljoyn_message, str : ::windows_sys::core::PCSTR, buf : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_destroy(msg : alljoyn_message) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_eql(one : alljoyn_message, other : alljoyn_message) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getarg(msg : alljoyn_message, argn : usize) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getargs(msg : alljoyn_message, numargs : *mut usize, args : *mut alljoyn_msgarg) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getauthmechanism(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getcallserial(msg : alljoyn_message) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getcompressiontoken(msg : alljoyn_message) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getdestination(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_geterrorname(msg : alljoyn_message, errormessage : ::windows_sys::core::PCSTR, errormessage_size : *mut usize) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getflags(msg : alljoyn_message) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getinterface(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getmembername(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getobjectpath(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getreceiveendpointname(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getreplyserial(msg : alljoyn_message) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getsender(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getsessionid(msg : alljoyn_message) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_getsignature(msg : alljoyn_message) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_gettimestamp(msg : alljoyn_message) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_gettype(msg : alljoyn_message) -> alljoyn_messagetype);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_isbroadcastsignal(msg : alljoyn_message) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_isencrypted(msg : alljoyn_message) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_isexpired(msg : alljoyn_message, tillexpirems : *mut u32) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_isglobalbroadcast(msg : alljoyn_message) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_issessionless(msg : alljoyn_message) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_isunreliable(msg : alljoyn_message) -> i32);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_message_parseargs(msg : alljoyn_message, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_setendianess(endian : i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_message_tostring(msg : alljoyn_message, str : ::windows_sys::core::PCSTR, buf : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_array_create(size : usize) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_array_element(arg : alljoyn_msgarg, index : usize) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_array_get(args : alljoyn_msgarg, numargs : usize, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_array_set(args : alljoyn_msgarg, numargs : *mut usize, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_array_set_offset(args : alljoyn_msgarg, argoffset : usize, numargs : *mut usize, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_array_signature(values : alljoyn_msgarg, numvalues : usize, str : ::windows_sys::core::PCSTR, buf : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_array_tostring(args : alljoyn_msgarg, numargs : usize, str : ::windows_sys::core::PCSTR, buf : usize, indent : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_clear(arg : alljoyn_msgarg) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_clone(destination : alljoyn_msgarg, source : alljoyn_msgarg) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_copy(source : alljoyn_msgarg) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_create() -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_create_and_set(signature : ::windows_sys::core::PCSTR, ...) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_destroy(arg : alljoyn_msgarg) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_equal(lhv : alljoyn_msgarg, rhv : alljoyn_msgarg) -> i32);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_get(arg : alljoyn_msgarg, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_array_element(arg : alljoyn_msgarg, index : usize, element : *mut alljoyn_msgarg) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_array_elementsignature(arg : alljoyn_msgarg, index : usize) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_array_numberofelements(arg : alljoyn_msgarg) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_bool(arg : alljoyn_msgarg, b : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_bool_array(arg : alljoyn_msgarg, length : *mut usize, ab : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_double(arg : alljoyn_msgarg, d : *mut f64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_double_array(arg : alljoyn_msgarg, length : *mut usize, ad : *mut f64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_int16(arg : alljoyn_msgarg, n : *mut i16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_int16_array(arg : alljoyn_msgarg, length : *mut usize, an : *mut i16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_int32(arg : alljoyn_msgarg, i : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_int32_array(arg : alljoyn_msgarg, length : *mut usize, ai : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_int64(arg : alljoyn_msgarg, x : *mut i64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_int64_array(arg : alljoyn_msgarg, length : *mut usize, ax : *mut i64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_objectpath(arg : alljoyn_msgarg, o : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_signature(arg : alljoyn_msgarg, g : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_string(arg : alljoyn_msgarg, s : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint16(arg : alljoyn_msgarg, q : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint16_array(arg : alljoyn_msgarg, length : *mut usize, aq : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint32(arg : alljoyn_msgarg, u : *mut u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint32_array(arg : alljoyn_msgarg, length : *mut usize, au : *mut u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint64(arg : alljoyn_msgarg, t : *mut u64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint64_array(arg : alljoyn_msgarg, length : *mut usize, at : *mut u64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint8(arg : alljoyn_msgarg, y : *mut u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_uint8_array(arg : alljoyn_msgarg, length : *mut usize, ay : *mut u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_variant(arg : alljoyn_msgarg, v : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_get_variant_array(arg : alljoyn_msgarg, signature : ::windows_sys::core::PCSTR, length : *mut usize, av : *mut alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_getdictelement(arg : alljoyn_msgarg, elemsig : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_getkey(arg : alljoyn_msgarg) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_getmember(arg : alljoyn_msgarg, index : usize) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_getnummembers(arg : alljoyn_msgarg) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_gettype(arg : alljoyn_msgarg) -> alljoyn_typeid);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_getvalue(arg : alljoyn_msgarg) -> alljoyn_msgarg);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_hassignature(arg : alljoyn_msgarg, signature : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_set(arg : alljoyn_msgarg, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "cdecl" fn alljoyn_msgarg_set_and_stabilize(arg : alljoyn_msgarg, signature : ::windows_sys::core::PCSTR, ...) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_bool(arg : alljoyn_msgarg, b : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_bool_array(arg : alljoyn_msgarg, length : usize, ab : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_double(arg : alljoyn_msgarg, d : f64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_double_array(arg : alljoyn_msgarg, length : usize, ad : *mut f64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_int16(arg : alljoyn_msgarg, n : i16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_int16_array(arg : alljoyn_msgarg, length : usize, an : *mut i16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_int32(arg : alljoyn_msgarg, i : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_int32_array(arg : alljoyn_msgarg, length : usize, ai : *mut i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_int64(arg : alljoyn_msgarg, x : i64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_int64_array(arg : alljoyn_msgarg, length : usize, ax : *mut i64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_objectpath(arg : alljoyn_msgarg, o : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_objectpath_array(arg : alljoyn_msgarg, length : usize, ao : *const *const i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_signature(arg : alljoyn_msgarg, g : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_signature_array(arg : alljoyn_msgarg, length : usize, ag : *const *const i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_string(arg : alljoyn_msgarg, s : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_string_array(arg : alljoyn_msgarg, length : usize, r#as : *const *const i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint16(arg : alljoyn_msgarg, q : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint16_array(arg : alljoyn_msgarg, length : usize, aq : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint32(arg : alljoyn_msgarg, u : u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint32_array(arg : alljoyn_msgarg, length : usize, au : *mut u32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint64(arg : alljoyn_msgarg, t : u64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint64_array(arg : alljoyn_msgarg, length : usize, at : *mut u64) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint8(arg : alljoyn_msgarg, y : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_set_uint8_array(arg : alljoyn_msgarg, length : usize, ay : *mut u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_setdictentry(arg : alljoyn_msgarg, key : alljoyn_msgarg, value : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_setstruct(arg : alljoyn_msgarg, struct_members : alljoyn_msgarg, num_members : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_signature(arg : alljoyn_msgarg, str : ::windows_sys::core::PCSTR, buf : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_stabilize(arg : alljoyn_msgarg) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_msgarg_tostring(arg : alljoyn_msgarg, str : ::windows_sys::core::PCSTR, buf : usize, indent : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_create(bus : alljoyn_busattachment, mandatoryinterfaces : *const *const i8, nummandatoryinterfaces : usize) -> alljoyn_observer);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_destroy(observer : alljoyn_observer) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_get(observer : alljoyn_observer, uniquebusname : ::windows_sys::core::PCSTR, objectpath : ::windows_sys::core::PCSTR) -> alljoyn_proxybusobject_ref);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_getfirst(observer : alljoyn_observer) -> alljoyn_proxybusobject_ref);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_getnext(observer : alljoyn_observer, proxyref : alljoyn_proxybusobject_ref) -> alljoyn_proxybusobject_ref);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_registerlistener(observer : alljoyn_observer, listener : alljoyn_observerlistener, triggeronexisting : i32) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_unregisteralllisteners(observer : alljoyn_observer) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observer_unregisterlistener(observer : alljoyn_observer, listener : alljoyn_observerlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observerlistener_create(callback : *const alljoyn_observerlistener_callback, context : *const ::core::ffi::c_void) -> alljoyn_observerlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_observerlistener_destroy(listener : alljoyn_observerlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_passwordmanager_setcredentials(authmechanism : ::windows_sys::core::PCSTR, password : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurationlistener_create(callbacks : *const alljoyn_permissionconfigurationlistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_permissionconfigurationlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurationlistener_destroy(listener : alljoyn_permissionconfigurationlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_certificatechain_destroy(certificatechain : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_certificateid_cleanup(certificateid : *mut alljoyn_certificateid) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_certificateidarray_cleanup(certificateidarray : *mut alljoyn_certificateidarray) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_claim(configurator : alljoyn_permissionconfigurator, cakey : *mut i8, identitycertificatechain : *mut i8, groupid : *const u8, groupsize : usize, groupauthority : *mut i8, manifestsxmls : *mut *mut i8, manifestscount : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_endmanagement(configurator : alljoyn_permissionconfigurator) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getapplicationstate(configurator : alljoyn_permissionconfigurator, state : *mut alljoyn_applicationstate) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getclaimcapabilities(configurator : alljoyn_permissionconfigurator, claimcapabilities : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getclaimcapabilitiesadditionalinfo(configurator : alljoyn_permissionconfigurator, additionalinfo : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getdefaultclaimcapabilities() -> u16);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getdefaultpolicy(configurator : alljoyn_permissionconfigurator, policyxml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getidentity(configurator : alljoyn_permissionconfigurator, identitycertificatechain : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getidentitycertificateid(configurator : alljoyn_permissionconfigurator, certificateid : *mut alljoyn_certificateid) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getmanifests(configurator : alljoyn_permissionconfigurator, manifestarray : *mut alljoyn_manifestarray) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getmanifesttemplate(configurator : alljoyn_permissionconfigurator, manifesttemplatexml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getmembershipsummaries(configurator : alljoyn_permissionconfigurator, certificateids : *mut alljoyn_certificateidarray) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getpolicy(configurator : alljoyn_permissionconfigurator, policyxml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_getpublickey(configurator : alljoyn_permissionconfigurator, publickey : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_installmanifests(configurator : alljoyn_permissionconfigurator, manifestsxmls : *mut *mut i8, manifestscount : usize, append : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_installmembership(configurator : alljoyn_permissionconfigurator, membershipcertificatechain : *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_manifestarray_cleanup(manifestarray : *mut alljoyn_manifestarray) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_manifesttemplate_destroy(manifesttemplatexml : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_policy_destroy(policyxml : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_publickey_destroy(publickey : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_removemembership(configurator : alljoyn_permissionconfigurator, serial : *const u8, seriallen : usize, issuerpublickey : *mut i8, issueraki : *const u8, issuerakilen : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_reset(configurator : alljoyn_permissionconfigurator) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_resetpolicy(configurator : alljoyn_permissionconfigurator) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_setapplicationstate(configurator : alljoyn_permissionconfigurator, state : alljoyn_applicationstate) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_setclaimcapabilities(configurator : alljoyn_permissionconfigurator, claimcapabilities : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_setclaimcapabilitiesadditionalinfo(configurator : alljoyn_permissionconfigurator, additionalinfo : u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_setmanifesttemplatefromxml(configurator : alljoyn_permissionconfigurator, manifesttemplatexml : *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_startmanagement(configurator : alljoyn_permissionconfigurator) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_updateidentity(configurator : alljoyn_permissionconfigurator, identitycertificatechain : *mut i8, manifestsxmls : *mut *mut i8, manifestscount : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_permissionconfigurator_updatepolicy(configurator : alljoyn_permissionconfigurator, policyxml : *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_pinglistener_create(callback : *const alljoyn_pinglistener_callback, context : *const ::core::ffi::c_void) -> alljoyn_pinglistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_pinglistener_destroy(listener : alljoyn_pinglistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_addchild(proxyobj : alljoyn_proxybusobject, child : alljoyn_proxybusobject) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_addinterface(proxyobj : alljoyn_proxybusobject, iface : alljoyn_interfacedescription) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_addinterface_by_name(proxyobj : alljoyn_proxybusobject, name : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_copy(source : alljoyn_proxybusobject) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_create(bus : alljoyn_busattachment, service : ::windows_sys::core::PCSTR, path : ::windows_sys::core::PCSTR, sessionid : u32) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_create_secure(bus : alljoyn_busattachment, service : ::windows_sys::core::PCSTR, path : ::windows_sys::core::PCSTR, sessionid : u32) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_destroy(proxyobj : alljoyn_proxybusobject) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_enablepropertycaching(proxyobj : alljoyn_proxybusobject) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getallproperties(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, values : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getallpropertiesasync(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, callback : alljoyn_proxybusobject_listener_getallpropertiescb_ptr, timeout : u32, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getchild(proxyobj : alljoyn_proxybusobject, path : ::windows_sys::core::PCSTR) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getchildren(proxyobj : alljoyn_proxybusobject, children : *mut alljoyn_proxybusobject, numchildren : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getinterface(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR) -> alljoyn_interfacedescription);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getinterfaces(proxyobj : alljoyn_proxybusobject, ifaces : *const alljoyn_interfacedescription, numifaces : usize) -> usize);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getpath(proxyobj : alljoyn_proxybusobject) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getproperty(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, property : ::windows_sys::core::PCSTR, value : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getpropertyasync(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, property : ::windows_sys::core::PCSTR, callback : alljoyn_proxybusobject_listener_getpropertycb_ptr, timeout : u32, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getservicename(proxyobj : alljoyn_proxybusobject) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getsessionid(proxyobj : alljoyn_proxybusobject) -> u32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_getuniquename(proxyobj : alljoyn_proxybusobject) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_implementsinterface(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_introspectremoteobject(proxyobj : alljoyn_proxybusobject) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_introspectremoteobjectasync(proxyobj : alljoyn_proxybusobject, callback : alljoyn_proxybusobject_listener_introspectcb_ptr, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_issecure(proxyobj : alljoyn_proxybusobject) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_isvalid(proxyobj : alljoyn_proxybusobject) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_methodcall(proxyobj : alljoyn_proxybusobject, ifacename : ::windows_sys::core::PCSTR, methodname : ::windows_sys::core::PCSTR, args : alljoyn_msgarg, numargs : usize, replymsg : alljoyn_message, timeout : u32, flags : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_methodcall_member(proxyobj : alljoyn_proxybusobject, method : alljoyn_interfacedescription_member, args : alljoyn_msgarg, numargs : usize, replymsg : alljoyn_message, timeout : u32, flags : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_methodcall_member_noreply(proxyobj : alljoyn_proxybusobject, method : alljoyn_interfacedescription_member, args : alljoyn_msgarg, numargs : usize, flags : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_methodcall_noreply(proxyobj : alljoyn_proxybusobject, ifacename : ::windows_sys::core::PCSTR, methodname : ::windows_sys::core::PCSTR, args : alljoyn_msgarg, numargs : usize, flags : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_methodcallasync(proxyobj : alljoyn_proxybusobject, ifacename : ::windows_sys::core::PCSTR, methodname : ::windows_sys::core::PCSTR, replyfunc : alljoyn_messagereceiver_replyhandler_ptr, args : alljoyn_msgarg, numargs : usize, context : *mut ::core::ffi::c_void, timeout : u32, flags : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_methodcallasync_member(proxyobj : alljoyn_proxybusobject, method : alljoyn_interfacedescription_member, replyfunc : alljoyn_messagereceiver_replyhandler_ptr, args : alljoyn_msgarg, numargs : usize, context : *mut ::core::ffi::c_void, timeout : u32, flags : u8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_parsexml(proxyobj : alljoyn_proxybusobject, xml : ::windows_sys::core::PCSTR, identifier : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_ref_create(proxy : alljoyn_proxybusobject) -> alljoyn_proxybusobject_ref);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_ref_decref(r#ref : alljoyn_proxybusobject_ref) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_ref_get(r#ref : alljoyn_proxybusobject_ref) -> alljoyn_proxybusobject);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_ref_incref(r#ref : alljoyn_proxybusobject_ref) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_registerpropertieschangedlistener(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, properties : *const *const i8, numproperties : usize, callback : alljoyn_proxybusobject_listener_propertieschanged_ptr, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_removechild(proxyobj : alljoyn_proxybusobject, path : ::windows_sys::core::PCSTR) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_secureconnection(proxyobj : alljoyn_proxybusobject, forceauth : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_secureconnectionasync(proxyobj : alljoyn_proxybusobject, forceauth : i32) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_setproperty(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, property : ::windows_sys::core::PCSTR, value : alljoyn_msgarg) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_setpropertyasync(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, property : ::windows_sys::core::PCSTR, value : alljoyn_msgarg, callback : alljoyn_proxybusobject_listener_setpropertycb_ptr, timeout : u32, context : *mut ::core::ffi::c_void) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_proxybusobject_unregisterpropertieschangedlistener(proxyobj : alljoyn_proxybusobject, iface : ::windows_sys::core::PCSTR, callback : alljoyn_proxybusobject_listener_propertieschanged_ptr) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_routerinit() -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_routerinitwithconfig(configxml : *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_routershutdown() -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_claim(proxy : alljoyn_securityapplicationproxy, cakey : *mut i8, identitycertificatechain : *mut i8, groupid : *const u8, groupsize : usize, groupauthority : *mut i8, manifestsxmls : *mut *mut i8, manifestscount : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_computemanifestdigest(unsignedmanifestxml : *mut i8, identitycertificatepem : *mut i8, digest : *mut *mut u8, digestsize : *mut usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_create(bus : alljoyn_busattachment, appbusname : *mut i8, sessionid : u32) -> alljoyn_securityapplicationproxy);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_destroy(proxy : alljoyn_securityapplicationproxy) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_digest_destroy(digest : *mut u8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_eccpublickey_destroy(eccpublickey : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_endmanagement(proxy : alljoyn_securityapplicationproxy) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getapplicationstate(proxy : alljoyn_securityapplicationproxy, applicationstate : *mut alljoyn_applicationstate) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getclaimcapabilities(proxy : alljoyn_securityapplicationproxy, capabilities : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getclaimcapabilitiesadditionalinfo(proxy : alljoyn_securityapplicationproxy, additionalinfo : *mut u16) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getdefaultpolicy(proxy : alljoyn_securityapplicationproxy, policyxml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_geteccpublickey(proxy : alljoyn_securityapplicationproxy, eccpublickey : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getmanifesttemplate(proxy : alljoyn_securityapplicationproxy, manifesttemplatexml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getpermissionmanagementsessionport() -> u16);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_getpolicy(proxy : alljoyn_securityapplicationproxy, policyxml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_installmembership(proxy : alljoyn_securityapplicationproxy, membershipcertificatechain : *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_manifest_destroy(signedmanifestxml : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_manifesttemplate_destroy(manifesttemplatexml : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_policy_destroy(policyxml : *mut i8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_reset(proxy : alljoyn_securityapplicationproxy) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_resetpolicy(proxy : alljoyn_securityapplicationproxy) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_setmanifestsignature(unsignedmanifestxml : *mut i8, identitycertificatepem : *mut i8, signature : *const u8, signaturesize : usize, signedmanifestxml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_signmanifest(unsignedmanifestxml : *mut i8, identitycertificatepem : *mut i8, signingprivatekeypem : *mut i8, signedmanifestxml : *mut *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_startmanagement(proxy : alljoyn_securityapplicationproxy) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_updateidentity(proxy : alljoyn_securityapplicationproxy, identitycertificatechain : *mut i8, manifestsxmls : *mut *mut i8, manifestscount : usize) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_securityapplicationproxy_updatepolicy(proxy : alljoyn_securityapplicationproxy, policyxml : *mut i8) -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionlistener_create(callbacks : *const alljoyn_sessionlistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_sessionlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionlistener_destroy(listener : alljoyn_sessionlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_cmp(one : alljoyn_sessionopts, other : alljoyn_sessionopts) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_create(traffic : u8, ismultipoint : i32, proximity : u8, transports : u16) -> alljoyn_sessionopts);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_destroy(opts : alljoyn_sessionopts) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_get_multipoint(opts : alljoyn_sessionopts) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_get_proximity(opts : alljoyn_sessionopts) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_get_traffic(opts : alljoyn_sessionopts) -> u8);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_get_transports(opts : alljoyn_sessionopts) -> u16);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_iscompatible(one : alljoyn_sessionopts, other : alljoyn_sessionopts) -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_set_multipoint(opts : alljoyn_sessionopts, ismultipoint : i32) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_set_proximity(opts : alljoyn_sessionopts, proximity : u8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_set_traffic(opts : alljoyn_sessionopts, traffic : u8) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionopts_set_transports(opts : alljoyn_sessionopts, transports : u16) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionportlistener_create(callbacks : *const alljoyn_sessionportlistener_callbacks, context : *const ::core::ffi::c_void) -> alljoyn_sessionportlistener);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_sessionportlistener_destroy(listener : alljoyn_sessionportlistener) -> ());
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_shutdown() -> QStatus);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_unity_deferred_callbacks_process() -> i32);
::windows_targets::link!("msajapi.dll" "system" fn alljoyn_unity_set_deferred_callback_mainthread_only(mainthread_only : i32) -> ());
pub const AJ_IFC_SECURITY_INHERIT: alljoyn_interfacedescription_securitypolicy = 0i32;
pub const AJ_IFC_SECURITY_OFF: alljoyn_interfacedescription_securitypolicy = 2i32;
pub const AJ_IFC_SECURITY_REQUIRED: alljoyn_interfacedescription_securitypolicy = 1i32;
pub const ALLJOYN_ARRAY: alljoyn_typeid = 97i32;
pub const ALLJOYN_BIG_ENDIAN: u8 = 66u8;
pub const ALLJOYN_BOOLEAN: alljoyn_typeid = 98i32;
pub const ALLJOYN_BOOLEAN_ARRAY: alljoyn_typeid = 25185i32;
pub const ALLJOYN_BYTE: alljoyn_typeid = 121i32;
pub const ALLJOYN_BYTE_ARRAY: alljoyn_typeid = 31073i32;
pub const ALLJOYN_CRED_CERT_CHAIN: u16 = 4u16;
pub const ALLJOYN_CRED_EXPIRATION: u16 = 32u16;
pub const ALLJOYN_CRED_LOGON_ENTRY: u16 = 16u16;
pub const ALLJOYN_CRED_NEW_PASSWORD: u16 = 4097u16;
pub const ALLJOYN_CRED_ONE_TIME_PWD: u16 = 8193u16;
pub const ALLJOYN_CRED_PASSWORD: u16 = 1u16;
pub const ALLJOYN_CRED_PRIVATE_KEY: u16 = 8u16;
pub const ALLJOYN_CRED_USER_NAME: u16 = 2u16;
pub const ALLJOYN_DICT_ENTRY: alljoyn_typeid = 101i32;
pub const ALLJOYN_DICT_ENTRY_CLOSE: alljoyn_typeid = 125i32;
pub const ALLJOYN_DICT_ENTRY_OPEN: alljoyn_typeid = 123i32;
pub const ALLJOYN_DISCONNECTED: u32 = 4u32;
pub const ALLJOYN_DOUBLE: alljoyn_typeid = 100i32;
pub const ALLJOYN_DOUBLE_ARRAY: alljoyn_typeid = 25697i32;
pub const ALLJOYN_HANDLE: alljoyn_typeid = 104i32;
pub const ALLJOYN_INT16: alljoyn_typeid = 110i32;
pub const ALLJOYN_INT16_ARRAY: alljoyn_typeid = 28257i32;
pub const ALLJOYN_INT32: alljoyn_typeid = 105i32;
pub const ALLJOYN_INT32_ARRAY: alljoyn_typeid = 26977i32;
pub const ALLJOYN_INT64: alljoyn_typeid = 120i32;
pub const ALLJOYN_INT64_ARRAY: alljoyn_typeid = 30817i32;
pub const ALLJOYN_INVALID: alljoyn_typeid = 0i32;
pub const ALLJOYN_LITTLE_ENDIAN: u8 = 108u8;
pub const ALLJOYN_MEMBER_ANNOTATE_DEPRECATED: u8 = 2u8;
pub const ALLJOYN_MEMBER_ANNOTATE_GLOBAL_BROADCAST: u8 = 32u8;
pub const ALLJOYN_MEMBER_ANNOTATE_NO_REPLY: u8 = 1u8;
pub const ALLJOYN_MEMBER_ANNOTATE_SESSIONCAST: u8 = 4u8;
pub const ALLJOYN_MEMBER_ANNOTATE_SESSIONLESS: u8 = 8u8;
pub const ALLJOYN_MEMBER_ANNOTATE_UNICAST: u8 = 16u8;
pub const ALLJOYN_MESSAGE_DEFAULT_TIMEOUT: u32 = 25000u32;
pub const ALLJOYN_MESSAGE_ERROR: alljoyn_messagetype = 3i32;
pub const ALLJOYN_MESSAGE_FLAG_ALLOW_REMOTE_MSG: u32 = 4u32;
pub const ALLJOYN_MESSAGE_FLAG_AUTO_START: u32 = 2u32;
pub const ALLJOYN_MESSAGE_FLAG_ENCRYPTED: u32 = 128u32;
pub const ALLJOYN_MESSAGE_FLAG_GLOBAL_BROADCAST: u32 = 32u32;
pub const ALLJOYN_MESSAGE_FLAG_NO_REPLY_EXPECTED: u32 = 1u32;
pub const ALLJOYN_MESSAGE_FLAG_SESSIONLESS: u32 = 16u32;
pub const ALLJOYN_MESSAGE_INVALID: alljoyn_messagetype = 0i32;
pub const ALLJOYN_MESSAGE_METHOD_CALL: alljoyn_messagetype = 1i32;
pub const ALLJOYN_MESSAGE_METHOD_RET: alljoyn_messagetype = 2i32;
pub const ALLJOYN_MESSAGE_SIGNAL: alljoyn_messagetype = 4i32;
pub const ALLJOYN_NAMED_PIPE_CONNECT_SPEC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("npipe:");
pub const ALLJOYN_OBJECT_PATH: alljoyn_typeid = 111i32;
pub const ALLJOYN_PROP_ACCESS_READ: u8 = 1u8;
pub const ALLJOYN_PROP_ACCESS_RW: u8 = 3u8;
pub const ALLJOYN_PROP_ACCESS_WRITE: u8 = 2u8;
pub const ALLJOYN_PROXIMITY_ANY: u32 = 255u32;
pub const ALLJOYN_PROXIMITY_NETWORK: u32 = 2u32;
pub const ALLJOYN_PROXIMITY_PHYSICAL: u32 = 1u32;
pub const ALLJOYN_READ_READY: u32 = 1u32;
pub const ALLJOYN_SESSIONLOST_INVALID: alljoyn_sessionlostreason = 0i32;
pub const ALLJOYN_SESSIONLOST_LINK_TIMEOUT: alljoyn_sessionlostreason = 4i32;
pub const ALLJOYN_SESSIONLOST_REASON_OTHER: alljoyn_sessionlostreason = 5i32;
pub const ALLJOYN_SESSIONLOST_REMOTE_END_CLOSED_ABRUPTLY: alljoyn_sessionlostreason = 2i32;
pub const ALLJOYN_SESSIONLOST_REMOTE_END_LEFT_SESSION: alljoyn_sessionlostreason = 1i32;
pub const ALLJOYN_SESSIONLOST_REMOVED_BY_BINDER: alljoyn_sessionlostreason = 3i32;
pub const ALLJOYN_SIGNATURE: alljoyn_typeid = 103i32;
pub const ALLJOYN_STRING: alljoyn_typeid = 115i32;
pub const ALLJOYN_STRUCT: alljoyn_typeid = 114i32;
pub const ALLJOYN_STRUCT_CLOSE: alljoyn_typeid = 41i32;
pub const ALLJOYN_STRUCT_OPEN: alljoyn_typeid = 40i32;
pub const ALLJOYN_TRAFFIC_TYPE_MESSAGES: u32 = 1u32;
pub const ALLJOYN_TRAFFIC_TYPE_RAW_RELIABLE: u32 = 4u32;
pub const ALLJOYN_TRAFFIC_TYPE_RAW_UNRELIABLE: u32 = 2u32;
pub const ALLJOYN_UINT16: alljoyn_typeid = 113i32;
pub const ALLJOYN_UINT16_ARRAY: alljoyn_typeid = 29025i32;
pub const ALLJOYN_UINT32: alljoyn_typeid = 117i32;
pub const ALLJOYN_UINT32_ARRAY: alljoyn_typeid = 30049i32;
pub const ALLJOYN_UINT64: alljoyn_typeid = 116i32;
pub const ALLJOYN_UINT64_ARRAY: alljoyn_typeid = 29793i32;
pub const ALLJOYN_VARIANT: alljoyn_typeid = 118i32;
pub const ALLJOYN_WILDCARD: alljoyn_typeid = 42i32;
pub const ALLJOYN_WRITE_READY: u32 = 2u32;
pub const ANNOUNCED: alljoyn_about_announceflag = 1i32;
pub const CAPABLE_ECDHE_ECDSA: alljoyn_claimcapability_masks = 4i32;
pub const CAPABLE_ECDHE_NULL: alljoyn_claimcapability_masks = 1i32;
pub const CAPABLE_ECDHE_SPEKE: alljoyn_claimcapability_masks = 8i32;
pub const CLAIMABLE: alljoyn_applicationstate = 1i32;
pub const CLAIMED: alljoyn_applicationstate = 2i32;
pub const ER_ABOUT_ABOUTDATA_MISSING_REQUIRED_FIELD: QStatus = 37157i32;
pub const ER_ABOUT_DEFAULT_LANGUAGE_NOT_SPECIFIED: QStatus = 37155i32;
pub const ER_ABOUT_FIELD_ALREADY_SPECIFIED: QStatus = 37147i32;
pub const ER_ABOUT_INVALID_ABOUTDATA_FIELD_APPID_SIZE: QStatus = 37163i32;
pub const ER_ABOUT_INVALID_ABOUTDATA_FIELD_VALUE: QStatus = 37162i32;
pub const ER_ABOUT_INVALID_ABOUTDATA_LISTENER: QStatus = 37158i32;
pub const ER_ABOUT_SESSIONPORT_NOT_BOUND: QStatus = 37156i32;
pub const ER_ALERTED_THREAD: QStatus = 4098i32;
pub const ER_ALLJOYN_ACCESS_PERMISSION_ERROR: QStatus = 37028i32;
pub const ER_ALLJOYN_ACCESS_PERMISSION_WARNING: QStatus = 37027i32;
pub const ER_ALLJOYN_ADVERTISENAME_REPLY_ALREADY_ADVERTISING: QStatus = 37005i32;
pub const ER_ALLJOYN_ADVERTISENAME_REPLY_FAILED: QStatus = 37006i32;
pub const ER_ALLJOYN_ADVERTISENAME_REPLY_TRANSPORT_NOT_AVAILABLE: QStatus = 37004i32;
pub const ER_ALLJOYN_BINDSESSIONPORT_REPLY_ALREADY_EXISTS: QStatus = 36992i32;
pub const ER_ALLJOYN_BINDSESSIONPORT_REPLY_FAILED: QStatus = 36993i32;
pub const ER_ALLJOYN_BINDSESSIONPORT_REPLY_INVALID_OPTS: QStatus = 37018i32;
pub const ER_ALLJOYN_CANCELADVERTISENAME_REPLY_FAILED: QStatus = 37008i32;
pub const ER_ALLJOYN_CANCELFINDADVERTISEDNAME_REPLY_FAILED: QStatus = 37013i32;
pub const ER_ALLJOYN_FINDADVERTISEDNAME_REPLY_ALREADY_DISCOVERING: QStatus = 37010i32;
pub const ER_ALLJOYN_FINDADVERTISEDNAME_REPLY_FAILED: QStatus = 37011i32;
pub const ER_ALLJOYN_FINDADVERTISEDNAME_REPLY_TRANSPORT_NOT_AVAILABLE: QStatus = 37009i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_ALREADY_JOINED: QStatus = 37019i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_BAD_SESSION_OPTS: QStatus = 36999i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_CONNECT_FAILED: QStatus = 36997i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_FAILED: QStatus = 37000i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_NO_SESSION: QStatus = 36995i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_REJECTED: QStatus = 36998i32;
pub const ER_ALLJOYN_JOINSESSION_REPLY_UNREACHABLE: QStatus = 36996i32;
pub const ER_ALLJOYN_LEAVESESSION_REPLY_FAILED: QStatus = 37003i32;
pub const ER_ALLJOYN_LEAVESESSION_REPLY_NO_SESSION: QStatus = 37002i32;
pub const ER_ALLJOYN_ONAPPRESUME_REPLY_FAILED: QStatus = 37100i32;
pub const ER_ALLJOYN_ONAPPRESUME_REPLY_UNSUPPORTED: QStatus = 37101i32;
pub const ER_ALLJOYN_ONAPPSUSPEND_REPLY_FAILED: QStatus = 37098i32;
pub const ER_ALLJOYN_ONAPPSUSPEND_REPLY_UNSUPPORTED: QStatus = 37099i32;
pub const ER_ALLJOYN_PING_FAILED: QStatus = 37111i32;
pub const ER_ALLJOYN_PING_REPLY_FAILED: QStatus = 37143i32;
pub const ER_ALLJOYN_PING_REPLY_INCOMPATIBLE_REMOTE_ROUTING_NODE: QStatus = 37140i32;
pub const ER_ALLJOYN_PING_REPLY_IN_PROGRESS: QStatus = 37145i32;
pub const ER_ALLJOYN_PING_REPLY_TIMEOUT: QStatus = 37141i32;
pub const ER_ALLJOYN_PING_REPLY_UNKNOWN_NAME: QStatus = 37142i32;
pub const ER_ALLJOYN_PING_REPLY_UNREACHABLE: QStatus = 37112i32;
pub const ER_ALLJOYN_REMOVESESSIONMEMBER_INCOMPATIBLE_REMOTE_DAEMON: QStatus = 37107i32;
pub const ER_ALLJOYN_REMOVESESSIONMEMBER_NOT_BINDER: QStatus = 37104i32;
pub const ER_ALLJOYN_REMOVESESSIONMEMBER_NOT_FOUND: QStatus = 37106i32;
pub const ER_ALLJOYN_REMOVESESSIONMEMBER_NOT_MULTIPOINT: QStatus = 37105i32;
pub const ER_ALLJOYN_REMOVESESSIONMEMBER_REPLY_FAILED: QStatus = 37108i32;
pub const ER_ALLJOYN_REMOVESESSIONMEMBER_REPLY_NO_SESSION: QStatus = 37103i32;
pub const ER_ALLJOYN_SETLINKTIMEOUT_REPLY_FAILED: QStatus = 37026i32;
pub const ER_ALLJOYN_SETLINKTIMEOUT_REPLY_NOT_SUPPORTED: QStatus = 37024i32;
pub const ER_ALLJOYN_SETLINKTIMEOUT_REPLY_NO_DEST_SUPPORT: QStatus = 37025i32;
pub const ER_ALLJOYN_UNBINDSESSIONPORT_REPLY_BAD_PORT: QStatus = 37016i32;
pub const ER_ALLJOYN_UNBINDSESSIONPORT_REPLY_FAILED: QStatus = 37017i32;
pub const ER_APPLICATION_STATE_LISTENER_ALREADY_EXISTS: QStatus = 37184i32;
pub const ER_APPLICATION_STATE_LISTENER_NO_SUCH_LISTENER: QStatus = 37185i32;
pub const ER_ARDP_BACKPRESSURE: QStatus = 37122i32;
pub const ER_ARDP_DISCONNECTING: QStatus = 37139i32;
pub const ER_ARDP_INVALID_CONNECTION: QStatus = 37135i32;
pub const ER_ARDP_INVALID_RESPONSE: QStatus = 37134i32;
pub const ER_ARDP_INVALID_STATE: QStatus = 37124i32;
pub const ER_ARDP_PERSIST_TIMEOUT: QStatus = 37126i32;
pub const ER_ARDP_PROBE_TIMEOUT: QStatus = 37127i32;
pub const ER_ARDP_REMOTE_CONNECTION_RESET: QStatus = 37128i32;
pub const ER_ARDP_TTL_EXPIRED: QStatus = 37125i32;
pub const ER_ARDP_VERSION_NOT_SUPPORTED: QStatus = 37151i32;
pub const ER_ARDP_WRITE_BLOCKED: QStatus = 37153i32;
pub const ER_AUTH_FAIL: QStatus = 4100i32;
pub const ER_AUTH_USER_REJECT: QStatus = 4101i32;
pub const ER_BAD_ARG_1: QStatus = 12i32;
pub const ER_BAD_ARG_2: QStatus = 13i32;
pub const ER_BAD_ARG_3: QStatus = 14i32;
pub const ER_BAD_ARG_4: QStatus = 15i32;
pub const ER_BAD_ARG_5: QStatus = 16i32;
pub const ER_BAD_ARG_6: QStatus = 17i32;
pub const ER_BAD_ARG_7: QStatus = 18i32;
pub const ER_BAD_ARG_8: QStatus = 19i32;
pub const ER_BAD_ARG_COUNT: QStatus = 28i32;
pub const ER_BAD_HOSTNAME: QStatus = 4112i32;
pub const ER_BAD_STRING_ENCODING: QStatus = 4120i32;
pub const ER_BAD_TRANSPORT_MASK: QStatus = 37088i32;
pub const ER_BUFFER_TOO_SMALL: QStatus = 3i32;
pub const ER_BUS_ALREADY_CONNECTED: QStatus = 36932i32;
pub const ER_BUS_ALREADY_LISTENING: QStatus = 36934i32;
pub const ER_BUS_ANNOTATION_ALREADY_EXISTS: QStatus = 37082i32;
pub const ER_BUS_AUTHENTICATION_PENDING: QStatus = 37031i32;
pub const ER_BUS_BAD_BODY_LEN: QStatus = 36879i32;
pub const ER_BUS_BAD_BUS_NAME: QStatus = 36874i32;
pub const ER_BUS_BAD_CHILD_PATH: QStatus = 36927i32;
pub const ER_BUS_BAD_ERROR_NAME: QStatus = 36873i32;
pub const ER_BUS_BAD_HDR_FLAGS: QStatus = 36878i32;
pub const ER_BUS_BAD_HEADER_FIELD: QStatus = 36868i32;
pub const ER_BUS_BAD_HEADER_LEN: QStatus = 36880i32;
pub const ER_BUS_BAD_INTERFACE_NAME: QStatus = 36872i32;
pub const ER_BUS_BAD_LENGTH: QStatus = 36876i32;
pub const ER_BUS_BAD_MEMBER_NAME: QStatus = 36871i32;
pub const ER_BUS_BAD_OBJ_PATH: QStatus = 36870i32;
pub const ER_BUS_BAD_SENDER_ID: QStatus = 36908i32;
pub const ER_BUS_BAD_SEND_PARAMETER: QStatus = 36906i32;
pub const ER_BUS_BAD_SESSION_OPTS: QStatus = 36980i32;
pub const ER_BUS_BAD_SIGNATURE: QStatus = 36869i32;
pub const ER_BUS_BAD_TRANSPORT_ARGS: QStatus = 36903i32;
pub const ER_BUS_BAD_VALUE: QStatus = 36877i32;
pub const ER_BUS_BAD_VALUE_TYPE: QStatus = 36867i32;
pub const ER_BUS_BAD_XML: QStatus = 36926i32;
pub const ER_BUS_BLOCKING_CALL_NOT_ALLOWED: QStatus = 36960i32;
pub const ER_BUS_BUS_ALREADY_STARTED: QStatus = 36939i32;
pub const ER_BUS_BUS_NOT_STARTED: QStatus = 36940i32;
pub const ER_BUS_CANNOT_ADD_HANDLER: QStatus = 36965i32;
pub const ER_BUS_CANNOT_ADD_INTERFACE: QStatus = 36964i32;
pub const ER_BUS_CANNOT_EXPAND_MESSAGE: QStatus = 36930i32;
pub const ER_BUS_CONNECTION_REJECTED: QStatus = 36981i32;
pub const ER_BUS_CONNECT_FAILED: QStatus = 36913i32;
pub const ER_BUS_CORRUPT_KEYSTORE: QStatus = 36952i32;
pub const ER_BUS_DESCRIPTION_ALREADY_EXISTS: QStatus = 37188i32;
pub const ER_BUS_DESTINATION_NOT_AUTHENTICATED: QStatus = 37029i32;
pub const ER_BUS_ELEMENT_NOT_FOUND: QStatus = 36976i32;
pub const ER_BUS_EMPTY_MESSAGE: QStatus = 36910i32;
pub const ER_BUS_ENDPOINT_CLOSING: QStatus = 36920i32;
pub const ER_BUS_ENDPOINT_REDIRECTED: QStatus = 37030i32;
pub const ER_BUS_ERRORS: QStatus = 36864i32;
pub const ER_BUS_ERROR_NAME_MISSING: QStatus = 36890i32;
pub const ER_BUS_ERROR_RESPONSE: QStatus = 36925i32;
pub const ER_BUS_ESTABLISH_FAILED: QStatus = 36884i32;
pub const ER_BUS_HANDLES_MISMATCH: QStatus = 36973i32;
pub const ER_BUS_HANDLES_NOT_ENABLED: QStatus = 36972i32;
pub const ER_BUS_HDR_EXPANSION_INVALID: QStatus = 36946i32;
pub const ER_BUS_IFACE_ALREADY_EXISTS: QStatus = 36924i32;
pub const ER_BUS_INCOMPATIBLE_DAEMON: QStatus = 37094i32;
pub const ER_BUS_INTERFACE_ACTIVATED: QStatus = 37015i32;
pub const ER_BUS_INTERFACE_MISMATCH: QStatus = 36921i32;
pub const ER_BUS_INTERFACE_MISSING: QStatus = 36886i32;
pub const ER_BUS_INTERFACE_NO_SUCH_MEMBER: QStatus = 36891i32;
pub const ER_BUS_INVALID_AUTH_MECHANISM: QStatus = 36958i32;
pub const ER_BUS_INVALID_HEADER_CHECKSUM: QStatus = 36942i32;
pub const ER_BUS_INVALID_HEADER_SERIAL: QStatus = 36944i32;
pub const ER_BUS_KEYBLOB_OP_INVALID: QStatus = 36941i32;
pub const ER_BUS_KEYSTORE_NOT_LOADED: QStatus = 36966i32;
pub const ER_BUS_KEYSTORE_VERSION_MISMATCH: QStatus = 36959i32;
pub const ER_BUS_KEY_EXPIRED: QStatus = 36951i32;
pub const ER_BUS_KEY_STORE_NOT_LOADED: QStatus = 36937i32;
pub const ER_BUS_KEY_UNAVAILABLE: QStatus = 36935i32;
pub const ER_BUS_LISTENER_ALREADY_SET: QStatus = 37022i32;
pub const ER_BUS_MATCH_RULE_NOT_FOUND: QStatus = 37110i32;
pub const ER_BUS_MEMBER_ALREADY_EXISTS: QStatus = 36922i32;
pub const ER_BUS_MEMBER_MISSING: QStatus = 36888i32;
pub const ER_BUS_MEMBER_NO_SUCH_SIGNATURE: QStatus = 36896i32;
pub const ER_BUS_MESSAGE_DECRYPTION_FAILED: QStatus = 36949i32;
pub const ER_BUS_MESSAGE_NOT_ENCRYPTED: QStatus = 36943i32;
pub const ER_BUS_METHOD_CALL_ABORTED: QStatus = 36963i32;
pub const ER_BUS_MISSING_COMPRESSION_TOKEN: QStatus = 36947i32;
pub const ER_BUS_NAME_TOO_LONG: QStatus = 36875i32;
pub const ER_BUS_NOT_ALLOWED: QStatus = 36918i32;
pub const ER_BUS_NOT_AUTHENTICATING: QStatus = 36915i32;
pub const ER_BUS_NOT_AUTHORIZED: QStatus = 37032i32;
pub const ER_BUS_NOT_A_COMPLETE_TYPE: QStatus = 36954i32;
pub const ER_BUS_NOT_A_DICTIONARY: QStatus = 36977i32;
pub const ER_BUS_NOT_COMPRESSED: QStatus = 36931i32;
pub const ER_BUS_NOT_CONNECTED: QStatus = 36933i32;
pub const ER_BUS_NOT_NUL_TERMINATED: QStatus = 36897i32;
pub const ER_BUS_NOT_OWNER: QStatus = 36911i32;
pub const ER_BUS_NO_AUTHENTICATION_MECHANISM: QStatus = 36938i32;
pub const ER_BUS_NO_CALL_FOR_REPLY: QStatus = 36953i32;
pub const ER_BUS_NO_ENDPOINT: QStatus = 36905i32;
pub const ER_BUS_NO_LISTENER: QStatus = 36916i32;
pub const ER_BUS_NO_PEER_GUID: QStatus = 36948i32;
pub const ER_BUS_NO_ROUTE: QStatus = 36904i32;
pub const ER_BUS_NO_SESSION: QStatus = 36975i32;
pub const ER_BUS_NO_SUCH_ANNOTATION: QStatus = 37081i32;
pub const ER_BUS_NO_SUCH_HANDLE: QStatus = 36971i32;
pub const ER_BUS_NO_SUCH_INTERFACE: QStatus = 36895i32;
pub const ER_BUS_NO_SUCH_MESSAGE: QStatus = 37102i32;
pub const ER_BUS_NO_SUCH_OBJECT: QStatus = 36892i32;
pub const ER_BUS_NO_SUCH_PROPERTY: QStatus = 36898i32;
pub const ER_BUS_NO_SUCH_SERVICE: QStatus = 36956i32;
pub const ER_BUS_NO_TRANSPORTS: QStatus = 36902i32;
pub const ER_BUS_OBJECT_NOT_REGISTERED: QStatus = 37091i32;
pub const ER_BUS_OBJECT_NO_SUCH_INTERFACE: QStatus = 36894i32;
pub const ER_BUS_OBJECT_NO_SUCH_MEMBER: QStatus = 36893i32;
pub const ER_BUS_OBJ_ALREADY_EXISTS: QStatus = 36928i32;
pub const ER_BUS_OBJ_NOT_FOUND: QStatus = 36929i32;
pub const ER_BUS_PATH_MISSING: QStatus = 36887i32;
pub const ER_BUS_PEER_AUTH_VERSION_MISMATCH: QStatus = 37023i32;
pub const ER_BUS_PING_GROUP_NOT_FOUND: QStatus = 37159i32;
pub const ER_BUS_POLICY_VIOLATION: QStatus = 36955i32;
pub const ER_BUS_PROPERTY_ACCESS_DENIED: QStatus = 36901i32;
pub const ER_BUS_PROPERTY_ALREADY_EXISTS: QStatus = 36923i32;
pub const ER_BUS_PROPERTY_VALUE_NOT_SET: QStatus = 36900i32;
pub const ER_BUS_READ_ERROR: QStatus = 36865i32;
pub const ER_BUS_REMOVED_BY_BINDER: QStatus = 37109i32;
pub const ER_BUS_REMOVED_BY_BINDER_SELF: QStatus = 37160i32;
pub const ER_BUS_REPLY_IS_ERROR_MESSAGE: QStatus = 36914i32;
pub const ER_BUS_REPLY_SERIAL_MISSING: QStatus = 36889i32;
pub const ER_BUS_SECURITY_FATAL: QStatus = 36950i32;
pub const ER_BUS_SECURITY_NOT_ENABLED: QStatus = 37021i32;
pub const ER_BUS_SELF_CONNECT: QStatus = 37020i32;
pub const ER_BUS_SET_PROPERTY_REJECTED: QStatus = 36912i32;
pub const ER_BUS_SET_WRONG_SIGNATURE: QStatus = 36899i32;
pub const ER_BUS_SIGNATURE_MISMATCH: QStatus = 36961i32;
pub const ER_BUS_STOPPING: QStatus = 36962i32;
pub const ER_BUS_TIME_TO_LIVE_EXPIRED: QStatus = 36945i32;
pub const ER_BUS_TRANSPORT_ACCESS_DENIED: QStatus = 37164i32;
pub const ER_BUS_TRANSPORT_NOT_AVAILABLE: QStatus = 36957i32;
pub const ER_BUS_TRANSPORT_NOT_STARTED: QStatus = 36909i32;
pub const ER_BUS_TRUNCATED: QStatus = 36936i32;
pub const ER_BUS_UNEXPECTED_DISPOSITION: QStatus = 37014i32;
pub const ER_BUS_UNEXPECTED_SIGNATURE: QStatus = 36885i32;
pub const ER_BUS_UNKNOWN_INTERFACE: QStatus = 36883i32;
pub const ER_BUS_UNKNOWN_PATH: QStatus = 36882i32;
pub const ER_BUS_UNKNOWN_SERIAL: QStatus = 36881i32;
pub const ER_BUS_UNMATCHED_REPLY_SERIAL: QStatus = 36907i32;
pub const ER_BUS_WAIT_FAILED: QStatus = 36978i32;
pub const ER_BUS_WRITE_ERROR: QStatus = 36866i32;
pub const ER_BUS_WRITE_QUEUE_FULL: QStatus = 36919i32;
pub const ER_CERTIFICATE_NOT_FOUND: QStatus = 37166i32;
pub const ER_COMMON_ERRORS: QStatus = 4096i32;
pub const ER_CONNECTION_LIMIT_EXCEEDED: QStatus = 37152i32;
pub const ER_CONN_REFUSED: QStatus = 27i32;
pub const ER_CORRUPT_KEYBLOB: QStatus = 4115i32;
pub const ER_CRYPTO_ERROR: QStatus = 4109i32;
pub const ER_CRYPTO_HASH_UNINITIALIZED: QStatus = 4123i32;
pub const ER_CRYPTO_ILLEGAL_PARAMETERS: QStatus = 4122i32;
pub const ER_CRYPTO_INSUFFICIENT_SECURITY: QStatus = 4121i32;
pub const ER_CRYPTO_KEY_UNAVAILABLE: QStatus = 4111i32;
pub const ER_CRYPTO_KEY_UNUSABLE: QStatus = 4113i32;
pub const ER_CRYPTO_TRUNCATED: QStatus = 4110i32;
pub const ER_DBUS_RELEASE_NAME_REPLY_NON_EXISTENT: QStatus = 36987i32;
pub const ER_DBUS_RELEASE_NAME_REPLY_NOT_OWNER: QStatus = 36988i32;
pub const ER_DBUS_RELEASE_NAME_REPLY_RELEASED: QStatus = 36986i32;
pub const ER_DBUS_REQUEST_NAME_REPLY_ALREADY_OWNER: QStatus = 36985i32;
pub const ER_DBUS_REQUEST_NAME_REPLY_EXISTS: QStatus = 36984i32;
pub const ER_DBUS_REQUEST_NAME_REPLY_IN_QUEUE: QStatus = 36983i32;
pub const ER_DBUS_REQUEST_NAME_REPLY_PRIMARY_OWNER: QStatus = 36982i32;
pub const ER_DBUS_START_REPLY_ALREADY_RUNNING: QStatus = 36990i32;
pub const ER_DEADLOCK: QStatus = 31i32;
pub const ER_DEAD_THREAD: QStatus = 4117i32;
pub const ER_DIGEST_MISMATCH: QStatus = 37170i32;
pub const ER_DUPLICATE_CERTIFICATE: QStatus = 37167i32;
pub const ER_DUPLICATE_KEY: QStatus = 37171i32;
pub const ER_EMPTY_KEY_BLOB: QStatus = 4114i32;
pub const ER_END_OF_DATA: QStatus = 26i32;
pub const ER_EOF: QStatus = 30i32;
pub const ER_EXTERNAL_THREAD: QStatus = 4108i32;
pub const ER_FAIL: QStatus = 1i32;
pub const ER_FEATURE_NOT_AVAILABLE: QStatus = 37177i32;
pub const ER_INIT_FAILED: QStatus = 7i32;
pub const ER_INVALID_ADDRESS: QStatus = 20i32;
pub const ER_INVALID_APPLICATION_STATE: QStatus = 37176i32;
pub const ER_INVALID_CERTIFICATE: QStatus = 37165i32;
pub const ER_INVALID_CERTIFICATE_USAGE: QStatus = 37182i32;
pub const ER_INVALID_CERT_CHAIN: QStatus = 37174i32;
pub const ER_INVALID_CONFIG: QStatus = 37161i32;
pub const ER_INVALID_DATA: QStatus = 21i32;
pub const ER_INVALID_GUID: QStatus = 4126i32;
pub const ER_INVALID_HTTP_METHOD_USED_FOR_RENDEZVOUS_SERVER_INTERFACE_MESSAGE: QStatus = 37075i32;
pub const ER_INVALID_KEY_ENCODING: QStatus = 4116i32;
pub const ER_INVALID_ON_DEMAND_CONNECTION_MESSAGE_RESPONSE: QStatus = 37074i32;
pub const ER_INVALID_PERSISTENT_CONNECTION_MESSAGE_RESPONSE: QStatus = 37073i32;
pub const ER_INVALID_RENDEZVOUS_SERVER_INTERFACE_MESSAGE: QStatus = 37072i32;
pub const ER_INVALID_SIGNAL_EMISSION_TYPE: QStatus = 37183i32;
pub const ER_INVALID_STREAM: QStatus = 4129i32;
pub const ER_IODISPATCH_STOPPING: QStatus = 4131i32;
pub const ER_KEY_STORE_ALREADY_INITIALIZED: QStatus = 37178i32;
pub const ER_KEY_STORE_ID_NOT_YET_SET: QStatus = 37179i32;
pub const ER_LANGUAGE_NOT_SUPPORTED: QStatus = 37146i32;
pub const ER_MANAGEMENT_ALREADY_STARTED: QStatus = 37186i32;
pub const ER_MANAGEMENT_NOT_STARTED: QStatus = 37187i32;
pub const ER_MANIFEST_NOT_FOUND: QStatus = 37173i32;
pub const ER_MANIFEST_REJECTED: QStatus = 37181i32;
pub const ER_MISSING_DIGEST_IN_CERTIFICATE: QStatus = 37169i32;
pub const ER_NONE: QStatus = 65535i32;
pub const ER_NOT_CONN: QStatus = 4141i32;
pub const ER_NOT_CONNECTED_TO_RENDEZVOUS_SERVER: QStatus = 37070i32;
pub const ER_NOT_IMPLEMENTED: QStatus = 9i32;
pub const ER_NO_COMMON_TRUST: QStatus = 37172i32;
pub const ER_NO_SUCH_ALARM: QStatus = 4102i32;
pub const ER_NO_SUCH_DEVICE: QStatus = 37084i32;
pub const ER_NO_TRUST_ANCHOR: QStatus = 37175i32;
pub const ER_OK: QStatus = 0i32;
pub const ER_OPEN_FAILED: QStatus = 24i32;
pub const ER_OS_ERROR: QStatus = 4i32;
pub const ER_OUT_OF_MEMORY: QStatus = 5i32;
pub const ER_P2P: QStatus = 37085i32;
pub const ER_P2P_BUSY: QStatus = 37093i32;
pub const ER_P2P_DISABLED: QStatus = 37092i32;
pub const ER_P2P_FORBIDDEN: QStatus = 37097i32;
pub const ER_P2P_NOT_CONNECTED: QStatus = 37087i32;
pub const ER_P2P_NO_GO: QStatus = 37095i32;
pub const ER_P2P_NO_STA: QStatus = 37096i32;
pub const ER_P2P_TIMEOUT: QStatus = 37086i32;
pub const ER_PACKET_BAD_CRC: QStatus = 37039i32;
pub const ER_PACKET_BAD_FORMAT: QStatus = 37034i32;
pub const ER_PACKET_BAD_PARAMETER: QStatus = 37038i32;
pub const ER_PACKET_BUS_NO_SUCH_CHANNEL: QStatus = 37033i32;
pub const ER_PACKET_CHANNEL_FAIL: QStatus = 37036i32;
pub const ER_PACKET_CONNECT_TIMEOUT: QStatus = 37035i32;
pub const ER_PACKET_TOO_LARGE: QStatus = 37037i32;
pub const ER_PARSE_ERROR: QStatus = 25i32;
pub const ER_PERMISSION_DENIED: QStatus = 37154i32;
pub const ER_POLICY_NOT_NEWER: QStatus = 37180i32;
pub const ER_PROXIMITY_CONNECTION_ESTABLISH_FAIL: QStatus = 37089i32;
pub const ER_PROXIMITY_NO_PEERS_FOUND: QStatus = 37090i32;
pub const ER_READ_ERROR: QStatus = 22i32;
pub const ER_RENDEZVOUS_SERVER_DEACTIVATED_USER: QStatus = 37067i32;
pub const ER_RENDEZVOUS_SERVER_ERR401_UNAUTHORIZED_REQUEST: QStatus = 37078i32;
pub const ER_RENDEZVOUS_SERVER_ERR500_INTERNAL_ERROR: QStatus = 37076i32;
pub const ER_RENDEZVOUS_SERVER_ERR503_STATUS_UNAVAILABLE: QStatus = 37077i32;
pub const ER_RENDEZVOUS_SERVER_ROOT_CERTIFICATE_UNINITIALIZED: QStatus = 37080i32;
pub const ER_RENDEZVOUS_SERVER_UNKNOWN_USER: QStatus = 37068i32;
pub const ER_RENDEZVOUS_SERVER_UNRECOVERABLE_ERROR: QStatus = 37079i32;
pub const ER_SLAP_CRC_ERROR: QStatus = 4137i32;
pub const ER_SLAP_ERROR: QStatus = 4138i32;
pub const ER_SLAP_HDR_CHECKSUM_ERROR: QStatus = 4133i32;
pub const ER_SLAP_INVALID_PACKET_LEN: QStatus = 4132i32;
pub const ER_SLAP_INVALID_PACKET_TYPE: QStatus = 4134i32;
pub const ER_SLAP_LEN_MISMATCH: QStatus = 4135i32;
pub const ER_SLAP_OTHER_END_CLOSED: QStatus = 4139i32;
pub const ER_SLAP_PACKET_TYPE_MISMATCH: QStatus = 4136i32;
pub const ER_SOCKET_BIND_ERROR: QStatus = 6i32;
pub const ER_SOCK_CLOSING: QStatus = 37083i32;
pub const ER_SOCK_OTHER_END_CLOSED: QStatus = 11i32;
pub const ER_SSL_CONNECT: QStatus = 4106i32;
pub const ER_SSL_ERRORS: QStatus = 4104i32;
pub const ER_SSL_INIT: QStatus = 4105i32;
pub const ER_SSL_VERIFY: QStatus = 4107i32;
pub const ER_STOPPING_THREAD: QStatus = 4097i32;
pub const ER_TCP_MAX_UNTRUSTED: QStatus = 37144i32;
pub const ER_THREADPOOL_EXHAUSTED: QStatus = 4127i32;
pub const ER_THREADPOOL_STOPPING: QStatus = 4128i32;
pub const ER_THREAD_NO_WAIT: QStatus = 4124i32;
pub const ER_THREAD_RUNNING: QStatus = 4118i32;
pub const ER_THREAD_STOPPING: QStatus = 4119i32;
pub const ER_TIMEOUT: QStatus = 10i32;
pub const ER_TIMER_EXITING: QStatus = 4125i32;
pub const ER_TIMER_FALLBEHIND: QStatus = 4103i32;
pub const ER_TIMER_FULL: QStatus = 4130i32;
pub const ER_TIMER_NOT_ALLOWED: QStatus = 4140i32;
pub const ER_UDP_BACKPRESSURE: QStatus = 37123i32;
pub const ER_UDP_BUSHELLO: QStatus = 37129i32;
pub const ER_UDP_DEMUX_NO_ENDPOINT: QStatus = 37114i32;
pub const ER_UDP_DISCONNECT: QStatus = 37118i32;
pub const ER_UDP_EARLY_EXIT: QStatus = 37137i32;
pub const ER_UDP_ENDPOINT_NOT_STARTED: QStatus = 37149i32;
pub const ER_UDP_ENDPOINT_REMOVED: QStatus = 37150i32;
pub const ER_UDP_ENDPOINT_STALLED: QStatus = 37133i32;
pub const ER_UDP_INVALID: QStatus = 37131i32;
pub const ER_UDP_LOCAL_DISCONNECT: QStatus = 37136i32;
pub const ER_UDP_LOCAL_DISCONNECT_FAIL: QStatus = 37138i32;
pub const ER_UDP_MESSAGE: QStatus = 37130i32;
pub const ER_UDP_MSG_TOO_LONG: QStatus = 37113i32;
pub const ER_UDP_NOT_DISCONNECTED: QStatus = 37148i32;
pub const ER_UDP_NOT_IMPLEMENTED: QStatus = 37119i32;
pub const ER_UDP_NO_LISTENER: QStatus = 37120i32;
pub const ER_UDP_NO_NETWORK: QStatus = 37115i32;
pub const ER_UDP_STOPPING: QStatus = 37121i32;
pub const ER_UDP_UNEXPECTED_FLOW: QStatus = 37117i32;
pub const ER_UDP_UNEXPECTED_LENGTH: QStatus = 37116i32;
pub const ER_UDP_UNSUPPORTED: QStatus = 37132i32;
pub const ER_UNABLE_TO_CONNECT_TO_RENDEZVOUS_SERVER: QStatus = 37069i32;
pub const ER_UNABLE_TO_SEND_MESSAGE_TO_RENDEZVOUS_SERVER: QStatus = 37071i32;
pub const ER_UNKNOWN_CERTIFICATE: QStatus = 37168i32;
pub const ER_UTF_CONVERSION_FAILED: QStatus = 2i32;
pub const ER_WARNING: QStatus = 29i32;
pub const ER_WOULDBLOCK: QStatus = 8i32;
pub const ER_WRITE_ERROR: QStatus = 23i32;
pub const ER_XML_ACLS_MISSING: QStatus = 8211i32;
pub const ER_XML_ACL_ALL_TYPE_PEER_WITH_OTHERS: QStatus = 8207i32;
pub const ER_XML_ACL_PEERS_MISSING: QStatus = 8212i32;
pub const ER_XML_ACL_PEER_NOT_UNIQUE: QStatus = 8209i32;
pub const ER_XML_ACL_PEER_PUBLIC_KEY_SET: QStatus = 8210i32;
pub const ER_XML_ANNOTATION_NOT_UNIQUE: QStatus = 8222i32;
pub const ER_XML_CONVERTER_ERROR: QStatus = 8192i32;
pub const ER_XML_INTERFACE_MEMBERS_MISSING: QStatus = 8194i32;
pub const ER_XML_INTERFACE_NAME_NOT_UNIQUE: QStatus = 8219i32;
pub const ER_XML_INVALID_ACL_PEER_CHILDREN_COUNT: QStatus = 8206i32;
pub const ER_XML_INVALID_ACL_PEER_PUBLIC_KEY: QStatus = 8208i32;
pub const ER_XML_INVALID_ACL_PEER_TYPE: QStatus = 8205i32;
pub const ER_XML_INVALID_ANNOTATIONS_COUNT: QStatus = 8198i32;
pub const ER_XML_INVALID_ATTRIBUTE_VALUE: QStatus = 8200i32;
pub const ER_XML_INVALID_BASE64: QStatus = 8218i32;
pub const ER_XML_INVALID_ELEMENT_CHILDREN_COUNT: QStatus = 8202i32;
pub const ER_XML_INVALID_ELEMENT_NAME: QStatus = 8199i32;
pub const ER_XML_INVALID_INTERFACE_NAME: QStatus = 8214i32;
pub const ER_XML_INVALID_MANIFEST_VERSION: QStatus = 8216i32;
pub const ER_XML_INVALID_MEMBER_ACTION: QStatus = 8196i32;
pub const ER_XML_INVALID_MEMBER_NAME: QStatus = 8215i32;
pub const ER_XML_INVALID_MEMBER_TYPE: QStatus = 8195i32;
pub const ER_XML_INVALID_OBJECT_PATH: QStatus = 8213i32;
pub const ER_XML_INVALID_OID: QStatus = 8217i32;
pub const ER_XML_INVALID_POLICY_SERIAL_NUMBER: QStatus = 8204i32;
pub const ER_XML_INVALID_POLICY_VERSION: QStatus = 8203i32;
pub const ER_XML_INVALID_RULES_COUNT: QStatus = 8193i32;
pub const ER_XML_INVALID_SECURITY_LEVEL_ANNOTATION_VALUE: QStatus = 8201i32;
pub const ER_XML_MALFORMED: QStatus = 4099i32;
pub const ER_XML_MEMBER_DENY_ACTION_WITH_OTHER: QStatus = 8197i32;
pub const ER_XML_MEMBER_NAME_NOT_UNIQUE: QStatus = 8220i32;
pub const ER_XML_OBJECT_PATH_NOT_UNIQUE: QStatus = 8221i32;
pub const NEED_UPDATE: alljoyn_applicationstate = 3i32;
pub const NOT_CLAIMABLE: alljoyn_applicationstate = 0i32;
pub const PASSWORD_GENERATED_BY_APPLICATION: alljoyn_claimcapabilityadditionalinfo_masks = 2i32;
pub const PASSWORD_GENERATED_BY_SECURITY_MANAGER: alljoyn_claimcapabilityadditionalinfo_masks = 1i32;
pub const QCC_FALSE: u32 = 0u32;
pub const QCC_TRUE: u32 = 1u32;
pub const UNANNOUNCED: alljoyn_about_announceflag = 0i32;
pub type QStatus = i32;
pub type alljoyn_about_announceflag = i32;
pub type alljoyn_applicationstate = i32;
pub type alljoyn_claimcapability_masks = i32;
pub type alljoyn_claimcapabilityadditionalinfo_masks = i32;
pub type alljoyn_interfacedescription_securitypolicy = i32;
pub type alljoyn_messagetype = i32;
pub type alljoyn_sessionlostreason = i32;
pub type alljoyn_typeid = i32;
pub type alljoyn_aboutdata = isize;
pub type alljoyn_aboutdatalistener = isize;
#[repr(C)]
pub struct alljoyn_aboutdatalistener_callbacks {
    pub about_datalistener_getaboutdata: alljoyn_aboutdatalistener_getaboutdata_ptr,
    pub about_datalistener_getannouncedaboutdata: alljoyn_aboutdatalistener_getannouncedaboutdata_ptr,
}
impl ::core::marker::Copy for alljoyn_aboutdatalistener_callbacks {}
impl ::core::clone::Clone for alljoyn_aboutdatalistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_abouticon = isize;
pub type alljoyn_abouticonobj = isize;
pub type alljoyn_abouticonproxy = isize;
pub type alljoyn_aboutlistener = isize;
#[repr(C)]
pub struct alljoyn_aboutlistener_callback {
    pub about_listener_announced: alljoyn_about_announced_ptr,
}
impl ::core::marker::Copy for alljoyn_aboutlistener_callback {}
impl ::core::clone::Clone for alljoyn_aboutlistener_callback {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_aboutobj = isize;
pub type alljoyn_aboutobjectdescription = isize;
pub type alljoyn_aboutproxy = isize;
pub type alljoyn_applicationstatelistener = isize;
#[repr(C)]
pub struct alljoyn_applicationstatelistener_callbacks {
    pub state: alljoyn_applicationstatelistener_state_ptr,
}
impl ::core::marker::Copy for alljoyn_applicationstatelistener_callbacks {}
impl ::core::clone::Clone for alljoyn_applicationstatelistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_authlistener = isize;
#[repr(C)]
pub struct alljoyn_authlistener_callbacks {
    pub request_credentials: alljoyn_authlistener_requestcredentials_ptr,
    pub verify_credentials: alljoyn_authlistener_verifycredentials_ptr,
    pub security_violation: alljoyn_authlistener_securityviolation_ptr,
    pub authentication_complete: alljoyn_authlistener_authenticationcomplete_ptr,
}
impl ::core::marker::Copy for alljoyn_authlistener_callbacks {}
impl ::core::clone::Clone for alljoyn_authlistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_authlistenerasync_callbacks {
    pub request_credentials: alljoyn_authlistener_requestcredentialsasync_ptr,
    pub verify_credentials: alljoyn_authlistener_verifycredentialsasync_ptr,
    pub security_violation: alljoyn_authlistener_securityviolation_ptr,
    pub authentication_complete: alljoyn_authlistener_authenticationcomplete_ptr,
}
impl ::core::marker::Copy for alljoyn_authlistenerasync_callbacks {}
impl ::core::clone::Clone for alljoyn_authlistenerasync_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_autopinger = isize;
pub type alljoyn_busattachment = isize;
pub type alljoyn_buslistener = isize;
#[repr(C)]
pub struct alljoyn_buslistener_callbacks {
    pub listener_registered: alljoyn_buslistener_listener_registered_ptr,
    pub listener_unregistered: alljoyn_buslistener_listener_unregistered_ptr,
    pub found_advertised_name: alljoyn_buslistener_found_advertised_name_ptr,
    pub lost_advertised_name: alljoyn_buslistener_lost_advertised_name_ptr,
    pub name_owner_changed: alljoyn_buslistener_name_owner_changed_ptr,
    pub bus_stopping: alljoyn_buslistener_bus_stopping_ptr,
    pub bus_disconnected: alljoyn_buslistener_bus_disconnected_ptr,
    pub property_changed: alljoyn_buslistener_bus_prop_changed_ptr,
}
impl ::core::marker::Copy for alljoyn_buslistener_callbacks {}
impl ::core::clone::Clone for alljoyn_buslistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_busobject = isize;
#[repr(C)]
pub struct alljoyn_busobject_callbacks {
    pub property_get: alljoyn_busobject_prop_get_ptr,
    pub property_set: alljoyn_busobject_prop_set_ptr,
    pub object_registered: alljoyn_busobject_object_registration_ptr,
    pub object_unregistered: alljoyn_busobject_object_registration_ptr,
}
impl ::core::marker::Copy for alljoyn_busobject_callbacks {}
impl ::core::clone::Clone for alljoyn_busobject_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_busobject_methodentry {
    pub member: *const alljoyn_interfacedescription_member,
    pub method_handler: alljoyn_messagereceiver_methodhandler_ptr,
}
impl ::core::marker::Copy for alljoyn_busobject_methodentry {}
impl ::core::clone::Clone for alljoyn_busobject_methodentry {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_certificateid {
    pub serial: *mut u8,
    pub serialLen: usize,
    pub issuerPublicKey: *mut i8,
    pub issuerAki: *mut u8,
    pub issuerAkiLen: usize,
}
impl ::core::marker::Copy for alljoyn_certificateid {}
impl ::core::clone::Clone for alljoyn_certificateid {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_certificateidarray {
    pub count: usize,
    pub ids: *mut alljoyn_certificateid,
}
impl ::core::marker::Copy for alljoyn_certificateidarray {}
impl ::core::clone::Clone for alljoyn_certificateidarray {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_credentials = isize;
pub type alljoyn_interfacedescription = isize;
#[repr(C)]
pub struct alljoyn_interfacedescription_member {
    pub iface: alljoyn_interfacedescription,
    pub memberType: alljoyn_messagetype,
    pub name: ::windows_sys::core::PCSTR,
    pub signature: ::windows_sys::core::PCSTR,
    pub returnSignature: ::windows_sys::core::PCSTR,
    pub argNames: ::windows_sys::core::PCSTR,
    pub internal_member: *const ::core::ffi::c_void,
}
impl ::core::marker::Copy for alljoyn_interfacedescription_member {}
impl ::core::clone::Clone for alljoyn_interfacedescription_member {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_interfacedescription_property {
    pub name: ::windows_sys::core::PCSTR,
    pub signature: ::windows_sys::core::PCSTR,
    pub access: u8,
    pub internal_property: *const ::core::ffi::c_void,
}
impl ::core::marker::Copy for alljoyn_interfacedescription_property {}
impl ::core::clone::Clone for alljoyn_interfacedescription_property {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_keystore = isize;
pub type alljoyn_keystorelistener = isize;
#[repr(C)]
pub struct alljoyn_keystorelistener_callbacks {
    pub load_request: alljoyn_keystorelistener_loadrequest_ptr,
    pub store_request: alljoyn_keystorelistener_storerequest_ptr,
}
impl ::core::marker::Copy for alljoyn_keystorelistener_callbacks {}
impl ::core::clone::Clone for alljoyn_keystorelistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_keystorelistener_with_synchronization_callbacks {
    pub load_request: alljoyn_keystorelistener_loadrequest_ptr,
    pub store_request: alljoyn_keystorelistener_storerequest_ptr,
    pub acquire_exclusive_lock: alljoyn_keystorelistener_acquireexclusivelock_ptr,
    pub release_exclusive_lock: alljoyn_keystorelistener_releaseexclusivelock_ptr,
}
impl ::core::marker::Copy for alljoyn_keystorelistener_with_synchronization_callbacks {}
impl ::core::clone::Clone for alljoyn_keystorelistener_with_synchronization_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct alljoyn_manifestarray {
    pub count: usize,
    pub xmls: *mut *mut i8,
}
impl ::core::marker::Copy for alljoyn_manifestarray {}
impl ::core::clone::Clone for alljoyn_manifestarray {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_message = isize;
pub type alljoyn_msgarg = isize;
pub type alljoyn_observer = isize;
pub type alljoyn_observerlistener = isize;
#[repr(C)]
pub struct alljoyn_observerlistener_callback {
    pub object_discovered: alljoyn_observer_object_discovered_ptr,
    pub object_lost: alljoyn_observer_object_lost_ptr,
}
impl ::core::marker::Copy for alljoyn_observerlistener_callback {}
impl ::core::clone::Clone for alljoyn_observerlistener_callback {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_permissionconfigurationlistener = isize;
#[repr(C)]
pub struct alljoyn_permissionconfigurationlistener_callbacks {
    pub factory_reset: alljoyn_permissionconfigurationlistener_factoryreset_ptr,
    pub policy_changed: alljoyn_permissionconfigurationlistener_policychanged_ptr,
    pub start_management: alljoyn_permissionconfigurationlistener_startmanagement_ptr,
    pub end_management: alljoyn_permissionconfigurationlistener_endmanagement_ptr,
}
impl ::core::marker::Copy for alljoyn_permissionconfigurationlistener_callbacks {}
impl ::core::clone::Clone for alljoyn_permissionconfigurationlistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_permissionconfigurator = isize;
pub type alljoyn_pinglistener = isize;
#[repr(C)]
pub struct alljoyn_pinglistener_callback {
    pub destination_found: alljoyn_autopinger_destination_found_ptr,
    pub destination_lost: alljoyn_autopinger_destination_lost_ptr,
}
impl ::core::marker::Copy for alljoyn_pinglistener_callback {}
impl ::core::clone::Clone for alljoyn_pinglistener_callback {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_proxybusobject = isize;
pub type alljoyn_proxybusobject_ref = isize;
pub type alljoyn_securityapplicationproxy = isize;
pub type alljoyn_sessionlistener = isize;
#[repr(C)]
pub struct alljoyn_sessionlistener_callbacks {
    pub session_lost: alljoyn_sessionlistener_sessionlost_ptr,
    pub session_member_added: alljoyn_sessionlistener_sessionmemberadded_ptr,
    pub session_member_removed: alljoyn_sessionlistener_sessionmemberremoved_ptr,
}
impl ::core::marker::Copy for alljoyn_sessionlistener_callbacks {}
impl ::core::clone::Clone for alljoyn_sessionlistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_sessionopts = isize;
pub type alljoyn_sessionportlistener = isize;
#[repr(C)]
pub struct alljoyn_sessionportlistener_callbacks {
    pub accept_session_joiner: alljoyn_sessionportlistener_acceptsessionjoiner_ptr,
    pub session_joined: alljoyn_sessionportlistener_sessionjoined_ptr,
}
impl ::core::marker::Copy for alljoyn_sessionportlistener_callbacks {}
impl ::core::clone::Clone for alljoyn_sessionportlistener_callbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type alljoyn_about_announced_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, busname: ::windows_sys::core::PCSTR, version: u16, port: u16, objectdescriptionarg: alljoyn_msgarg, aboutdataarg: alljoyn_msgarg) -> ()>;
pub type alljoyn_aboutdatalistener_getaboutdata_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, msgarg: alljoyn_msgarg, language: ::windows_sys::core::PCSTR) -> QStatus>;
pub type alljoyn_aboutdatalistener_getannouncedaboutdata_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, msgarg: alljoyn_msgarg) -> QStatus>;
pub type alljoyn_applicationstatelistener_state_ptr = ::core::option::Option<unsafe extern "system" fn(busname: *mut i8, publickey: *mut i8, applicationstate: alljoyn_applicationstate, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_authlistener_authenticationcomplete_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, authmechanism: ::windows_sys::core::PCSTR, peername: ::windows_sys::core::PCSTR, success: i32) -> ()>;
pub type alljoyn_authlistener_requestcredentials_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, authmechanism: ::windows_sys::core::PCSTR, peername: ::windows_sys::core::PCSTR, authcount: u16, username: ::windows_sys::core::PCSTR, credmask: u16, credentials: alljoyn_credentials) -> i32>;
pub type alljoyn_authlistener_requestcredentialsasync_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, listener: alljoyn_authlistener, authmechanism: ::windows_sys::core::PCSTR, peername: ::windows_sys::core::PCSTR, authcount: u16, username: ::windows_sys::core::PCSTR, credmask: u16, authcontext: *mut ::core::ffi::c_void) -> QStatus>;
pub type alljoyn_authlistener_securityviolation_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, status: QStatus, msg: alljoyn_message) -> ()>;
pub type alljoyn_authlistener_verifycredentials_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, authmechanism: ::windows_sys::core::PCSTR, peername: ::windows_sys::core::PCSTR, credentials: alljoyn_credentials) -> i32>;
pub type alljoyn_authlistener_verifycredentialsasync_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, listener: alljoyn_authlistener, authmechanism: ::windows_sys::core::PCSTR, peername: ::windows_sys::core::PCSTR, credentials: alljoyn_credentials, authcontext: *mut ::core::ffi::c_void) -> QStatus>;
pub type alljoyn_autopinger_destination_found_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, group: ::windows_sys::core::PCSTR, destination: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_autopinger_destination_lost_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, group: ::windows_sys::core::PCSTR, destination: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_busattachment_joinsessioncb_ptr = ::core::option::Option<unsafe extern "system" fn(status: QStatus, sessionid: u32, opts: alljoyn_sessionopts, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_busattachment_setlinktimeoutcb_ptr = ::core::option::Option<unsafe extern "system" fn(status: QStatus, timeout: u32, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_buslistener_bus_disconnected_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_buslistener_bus_prop_changed_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, prop_name: ::windows_sys::core::PCSTR, prop_value: alljoyn_msgarg) -> ()>;
pub type alljoyn_buslistener_bus_stopping_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_buslistener_found_advertised_name_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, name: ::windows_sys::core::PCSTR, transport: u16, nameprefix: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_buslistener_listener_registered_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, bus: alljoyn_busattachment) -> ()>;
pub type alljoyn_buslistener_listener_unregistered_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_buslistener_lost_advertised_name_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, name: ::windows_sys::core::PCSTR, transport: u16, nameprefix: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_buslistener_name_owner_changed_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, busname: ::windows_sys::core::PCSTR, previousowner: ::windows_sys::core::PCSTR, newowner: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_busobject_object_registration_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_busobject_prop_get_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, ifcname: ::windows_sys::core::PCSTR, propname: ::windows_sys::core::PCSTR, val: alljoyn_msgarg) -> QStatus>;
pub type alljoyn_busobject_prop_set_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, ifcname: ::windows_sys::core::PCSTR, propname: ::windows_sys::core::PCSTR, val: alljoyn_msgarg) -> QStatus>;
pub type alljoyn_interfacedescription_translation_callback_ptr = ::core::option::Option<unsafe extern "system" fn(sourcelanguage: ::windows_sys::core::PCSTR, targetlanguage: ::windows_sys::core::PCSTR, sourcetext: ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR>;
pub type alljoyn_keystorelistener_acquireexclusivelock_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, listener: alljoyn_keystorelistener) -> QStatus>;
pub type alljoyn_keystorelistener_loadrequest_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, listener: alljoyn_keystorelistener, keystore: alljoyn_keystore) -> QStatus>;
pub type alljoyn_keystorelistener_releaseexclusivelock_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, listener: alljoyn_keystorelistener) -> ()>;
pub type alljoyn_keystorelistener_storerequest_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, listener: alljoyn_keystorelistener, keystore: alljoyn_keystore) -> QStatus>;
pub type alljoyn_messagereceiver_methodhandler_ptr = ::core::option::Option<unsafe extern "system" fn(bus: alljoyn_busobject, member: *const alljoyn_interfacedescription_member, message: alljoyn_message) -> ()>;
pub type alljoyn_messagereceiver_replyhandler_ptr = ::core::option::Option<unsafe extern "system" fn(message: alljoyn_message, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_messagereceiver_signalhandler_ptr = ::core::option::Option<unsafe extern "system" fn(member: *const alljoyn_interfacedescription_member, srcpath: ::windows_sys::core::PCSTR, message: alljoyn_message) -> ()>;
pub type alljoyn_observer_object_discovered_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, proxyref: alljoyn_proxybusobject_ref) -> ()>;
pub type alljoyn_observer_object_lost_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, proxyref: alljoyn_proxybusobject_ref) -> ()>;
pub type alljoyn_permissionconfigurationlistener_endmanagement_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_permissionconfigurationlistener_factoryreset_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> QStatus>;
pub type alljoyn_permissionconfigurationlistener_policychanged_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_permissionconfigurationlistener_startmanagement_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void) -> ()>;
pub type alljoyn_proxybusobject_listener_getallpropertiescb_ptr = ::core::option::Option<unsafe extern "system" fn(status: QStatus, obj: alljoyn_proxybusobject, values: alljoyn_msgarg, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_proxybusobject_listener_getpropertycb_ptr = ::core::option::Option<unsafe extern "system" fn(status: QStatus, obj: alljoyn_proxybusobject, value: alljoyn_msgarg, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_proxybusobject_listener_introspectcb_ptr = ::core::option::Option<unsafe extern "system" fn(status: QStatus, obj: alljoyn_proxybusobject, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_proxybusobject_listener_propertieschanged_ptr = ::core::option::Option<unsafe extern "system" fn(obj: alljoyn_proxybusobject, ifacename: ::windows_sys::core::PCSTR, changed: alljoyn_msgarg, invalidated: alljoyn_msgarg, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_proxybusobject_listener_setpropertycb_ptr = ::core::option::Option<unsafe extern "system" fn(status: QStatus, obj: alljoyn_proxybusobject, context: *mut ::core::ffi::c_void) -> ()>;
pub type alljoyn_sessionlistener_sessionlost_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, sessionid: u32, reason: alljoyn_sessionlostreason) -> ()>;
pub type alljoyn_sessionlistener_sessionmemberadded_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, sessionid: u32, uniquename: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_sessionlistener_sessionmemberremoved_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, sessionid: u32, uniquename: ::windows_sys::core::PCSTR) -> ()>;
pub type alljoyn_sessionportlistener_acceptsessionjoiner_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, sessionport: u16, joiner: ::windows_sys::core::PCSTR, opts: alljoyn_sessionopts) -> i32>;
pub type alljoyn_sessionportlistener_sessionjoined_ptr = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, sessionport: u16, id: u32, joiner: ::windows_sys::core::PCSTR) -> ()>;
