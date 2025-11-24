::windows_targets::link!("cfgmgr32.dll" "system" fn CMP_WaitNoPendingInstallEvents(dwtimeout : u32) -> u32);
#[cfg(feature = "Win32_Data_HtmlHelp")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Data_HtmlHelp\"`"] fn CM_Add_Empty_Log_Conf(plclogconf : *mut usize, dndevinst : u32, priority : super::super::Data::HtmlHelp:: PRIORITY, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Data_HtmlHelp")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Data_HtmlHelp\"`"] fn CM_Add_Empty_Log_Conf_Ex(plclogconf : *mut usize, dndevinst : u32, priority : super::super::Data::HtmlHelp:: PRIORITY, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_IDA(dndevinst : u32, pszid : ::windows_sys::core::PCSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_IDW(dndevinst : u32, pszid : ::windows_sys::core::PCWSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_ID_ExA(dndevinst : u32, pszid : ::windows_sys::core::PCSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_ID_ExW(dndevinst : u32, pszid : ::windows_sys::core::PCWSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_Range(ullstartvalue : u64, ullendvalue : u64, rlh : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_Res_Des(prdresdes : *mut usize, lclogconf : usize, resourceid : u32, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Add_Res_Des_Ex(prdresdes : *mut usize, lclogconf : usize, resourceid : u32, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Connect_MachineA(uncservername : ::windows_sys::core::PCSTR, phmachine : *mut isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Connect_MachineW(uncservername : ::windows_sys::core::PCWSTR, phmachine : *mut isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Create_DevNodeA(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCSTR, dnparent : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Create_DevNodeW(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCWSTR, dnparent : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Create_DevNode_ExA(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCSTR, dnparent : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Create_DevNode_ExW(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCWSTR, dnparent : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Create_Range_List(prlh : *mut usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Class_Key(classguid : *const ::windows_sys::core::GUID, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Class_Key_Ex(classguid : *const ::windows_sys::core::GUID, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_DevNode_Key(dndevnode : u32, ulhardwareprofile : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_DevNode_Key_Ex(dndevnode : u32, ulhardwareprofile : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Device_Interface_KeyA(pszdeviceinterface : ::windows_sys::core::PCSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Device_Interface_KeyW(pszdeviceinterface : ::windows_sys::core::PCWSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Device_Interface_Key_ExA(pszdeviceinterface : ::windows_sys::core::PCSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Device_Interface_Key_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Delete_Range(ullstartvalue : u64, ullendvalue : u64, rlh : usize, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Detect_Resource_Conflict(dndevinst : u32, resourceid : u32, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, pbconflictdetected : *mut super::super::Foundation:: BOOL, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Detect_Resource_Conflict_Ex(dndevinst : u32, resourceid : u32, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, pbconflictdetected : *mut super::super::Foundation:: BOOL, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Disable_DevNode(dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Disable_DevNode_Ex(dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Disconnect_Machine(hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Dup_Range_List(rlhold : usize, rlhnew : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enable_DevNode(dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enable_DevNode_Ex(dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enumerate_Classes(ulclassindex : u32, classguid : *mut ::windows_sys::core::GUID, ulflags : CM_ENUMERATE_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enumerate_Classes_Ex(ulclassindex : u32, classguid : *mut ::windows_sys::core::GUID, ulflags : CM_ENUMERATE_FLAGS, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enumerate_EnumeratorsA(ulenumindex : u32, buffer : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enumerate_EnumeratorsW(ulenumindex : u32, buffer : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enumerate_Enumerators_ExA(ulenumindex : u32, buffer : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Enumerate_Enumerators_ExW(ulenumindex : u32, buffer : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Find_Range(pullstart : *mut u64, ullstart : u64, ullength : u32, ullalignment : u64, ullend : u64, rlh : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_First_Range(rlh : usize, pullstart : *mut u64, pullend : *mut u64, preelement : *mut usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Log_Conf(lclogconftobefreed : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Log_Conf_Ex(lclogconftobefreed : usize, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Log_Conf_Handle(lclogconf : usize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Range_List(rlh : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Res_Des(prdresdes : *mut usize, rdresdes : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Res_Des_Ex(prdresdes : *mut usize, rdresdes : usize, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Res_Des_Handle(rdresdes : usize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Free_Resource_Conflict_Handle(clconflictlist : usize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Child(pdndevinst : *mut u32, dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Child_Ex(pdndevinst : *mut u32, dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Key_NameA(classguid : *const ::windows_sys::core::GUID, pszkeyname : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Key_NameW(classguid : *const ::windows_sys::core::GUID, pszkeyname : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Key_Name_ExA(classguid : *const ::windows_sys::core::GUID, pszkeyname : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Key_Name_ExW(classguid : *const ::windows_sys::core::GUID, pszkeyname : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_NameA(classguid : *const ::windows_sys::core::GUID, buffer : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_NameW(classguid : *const ::windows_sys::core::GUID, buffer : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Name_ExA(classguid : *const ::windows_sys::core::GUID, buffer : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Name_ExW(classguid : *const ::windows_sys::core::GUID, buffer : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Class_PropertyW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : *mut u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Class_Property_ExW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Class_Property_Keys(classguid : *const ::windows_sys::core::GUID, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : *mut u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Class_Property_Keys_Ex(classguid : *const ::windows_sys::core::GUID, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Registry_PropertyA(classguid : *const ::windows_sys::core::GUID, ulproperty : u32, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Class_Registry_PropertyW(classguid : *const ::windows_sys::core::GUID, ulproperty : u32, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Depth(puldepth : *mut u32, dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Depth_Ex(puldepth : *mut u32, dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Custom_PropertyA(dndevinst : u32, pszcustompropertyname : ::windows_sys::core::PCSTR, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Custom_PropertyW(dndevinst : u32, pszcustompropertyname : ::windows_sys::core::PCWSTR, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Custom_Property_ExA(dndevinst : u32, pszcustompropertyname : ::windows_sys::core::PCSTR, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Custom_Property_ExW(dndevinst : u32, pszcustompropertyname : ::windows_sys::core::PCWSTR, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_DevNode_PropertyW(dndevinst : u32, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : *mut u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_DevNode_Property_ExW(dndevinst : u32, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_DevNode_Property_Keys(dndevinst : u32, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : *mut u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_DevNode_Property_Keys_Ex(dndevinst : u32, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Registry_PropertyA(dndevinst : u32, ulproperty : u32, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Registry_PropertyW(dndevinst : u32, ulproperty : u32, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Registry_Property_ExA(dndevinst : u32, ulproperty : u32, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Registry_Property_ExW(dndevinst : u32, ulproperty : u32, pulregdatatype : *mut u32, buffer : *mut ::core::ffi::c_void, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Status(pulstatus : *mut CM_DEVNODE_STATUS_FLAGS, pulproblemnumber : *mut CM_PROB, dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_DevNode_Status_Ex(pulstatus : *mut CM_DEVNODE_STATUS_FLAGS, pulproblemnumber : *mut CM_PROB, dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_IDA(dndevinst : u32, buffer : ::windows_sys::core::PSTR, bufferlen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_IDW(dndevinst : u32, buffer : ::windows_sys::core::PWSTR, bufferlen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_ExA(dndevinst : u32, buffer : ::windows_sys::core::PSTR, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_ExW(dndevinst : u32, buffer : ::windows_sys::core::PWSTR, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_ListA(pszfilter : ::windows_sys::core::PCSTR, buffer : ::windows_sys::core::PSTR, bufferlen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_ListW(pszfilter : ::windows_sys::core::PCWSTR, buffer : ::windows_sys::core::PWSTR, bufferlen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_List_ExA(pszfilter : ::windows_sys::core::PCSTR, buffer : ::windows_sys::core::PSTR, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_List_ExW(pszfilter : ::windows_sys::core::PCWSTR, buffer : ::windows_sys::core::PWSTR, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_List_SizeA(pullen : *mut u32, pszfilter : ::windows_sys::core::PCSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_List_SizeW(pullen : *mut u32, pszfilter : ::windows_sys::core::PCWSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_List_Size_ExA(pullen : *mut u32, pszfilter : ::windows_sys::core::PCSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_List_Size_ExW(pullen : *mut u32, pszfilter : ::windows_sys::core::PCWSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_Size(pullen : *mut u32, dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_ID_Size_Ex(pullen : *mut u32, dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_AliasA(pszdeviceinterface : ::windows_sys::core::PCSTR, aliasinterfaceguid : *const ::windows_sys::core::GUID, pszaliasdeviceinterface : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_AliasW(pszdeviceinterface : ::windows_sys::core::PCWSTR, aliasinterfaceguid : *const ::windows_sys::core::GUID, pszaliasdeviceinterface : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_Alias_ExA(pszdeviceinterface : ::windows_sys::core::PCSTR, aliasinterfaceguid : *const ::windows_sys::core::GUID, pszaliasdeviceinterface : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_Alias_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, aliasinterfaceguid : *const ::windows_sys::core::GUID, pszaliasdeviceinterface : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_ListA(interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCSTR, buffer : ::windows_sys::core::PSTR, bufferlen : u32, ulflags : CM_GET_DEVICE_INTERFACE_LIST_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_ListW(interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCWSTR, buffer : ::windows_sys::core::PWSTR, bufferlen : u32, ulflags : CM_GET_DEVICE_INTERFACE_LIST_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_List_ExA(interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCSTR, buffer : ::windows_sys::core::PSTR, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_List_ExW(interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCWSTR, buffer : ::windows_sys::core::PWSTR, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_List_SizeA(pullen : *mut u32, interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCSTR, ulflags : CM_GET_DEVICE_INTERFACE_LIST_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_List_SizeW(pullen : *mut u32, interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCWSTR, ulflags : CM_GET_DEVICE_INTERFACE_LIST_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_List_Size_ExA(pullen : *mut u32, interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCSTR, ulflags : CM_GET_DEVICE_INTERFACE_LIST_FLAGS, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Device_Interface_List_Size_ExW(pullen : *mut u32, interfaceclassguid : *const ::windows_sys::core::GUID, pdeviceid : ::windows_sys::core::PCWSTR, ulflags : CM_GET_DEVICE_INTERFACE_LIST_FLAGS, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Device_Interface_PropertyW(pszdeviceinterface : ::windows_sys::core::PCWSTR, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : *mut u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Device_Interface_Property_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Device_Interface_Property_KeysW(pszdeviceinterface : ::windows_sys::core::PCWSTR, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : *mut u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Get_Device_Interface_Property_Keys_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_First_Log_Conf(plclogconf : *mut usize, dndevinst : u32, ulflags : CM_LOG_CONF) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_First_Log_Conf_Ex(plclogconf : *mut usize, dndevinst : u32, ulflags : CM_LOG_CONF, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Global_State(pulstate : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Global_State_Ex(pulstate : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_HW_Prof_FlagsA(pdeviceid : ::windows_sys::core::PCSTR, ulhardwareprofile : u32, pulvalue : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_HW_Prof_FlagsW(pdeviceid : ::windows_sys::core::PCWSTR, ulhardwareprofile : u32, pulvalue : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_HW_Prof_Flags_ExA(pdeviceid : ::windows_sys::core::PCSTR, ulhardwareprofile : u32, pulvalue : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_HW_Prof_Flags_ExW(pdeviceid : ::windows_sys::core::PCWSTR, ulhardwareprofile : u32, pulvalue : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Hardware_Profile_InfoA(ulindex : u32, phwprofileinfo : *mut HWPROFILEINFO_A, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Hardware_Profile_InfoW(ulindex : u32, phwprofileinfo : *mut HWPROFILEINFO_W, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Hardware_Profile_Info_ExA(ulindex : u32, phwprofileinfo : *mut HWPROFILEINFO_A, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Hardware_Profile_Info_ExW(ulindex : u32, phwprofileinfo : *mut HWPROFILEINFO_W, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Log_Conf_Priority(lclogconf : usize, ppriority : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Log_Conf_Priority_Ex(lclogconf : usize, ppriority : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Next_Log_Conf(plclogconf : *mut usize, lclogconf : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Next_Log_Conf_Ex(plclogconf : *mut usize, lclogconf : usize, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Next_Res_Des(prdresdes : *mut usize, rdresdes : usize, forresource : CM_RESTYPE, presourceid : *mut CM_RESTYPE, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Next_Res_Des_Ex(prdresdes : *mut usize, rdresdes : usize, forresource : CM_RESTYPE, presourceid : *mut CM_RESTYPE, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Parent(pdndevinst : *mut u32, dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Parent_Ex(pdndevinst : *mut u32, dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Res_Des_Data(rdresdes : usize, buffer : *mut ::core::ffi::c_void, bufferlen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Res_Des_Data_Ex(rdresdes : usize, buffer : *mut ::core::ffi::c_void, bufferlen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Res_Des_Data_Size(pulsize : *mut u32, rdresdes : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Res_Des_Data_Size_Ex(pulsize : *mut u32, rdresdes : usize, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Resource_Conflict_Count(clconflictlist : usize, pulcount : *mut u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Resource_Conflict_DetailsA(clconflictlist : usize, ulindex : u32, pconflictdetails : *mut CONFLICT_DETAILS_A) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Resource_Conflict_DetailsW(clconflictlist : usize, ulindex : u32, pconflictdetails : *mut CONFLICT_DETAILS_W) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Sibling(pdndevinst : *mut u32, dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Sibling_Ex(pdndevinst : *mut u32, dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Version() -> u16);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Get_Version_Ex(hmachine : isize) -> u16);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Intersect_Range_List(rlhold1 : usize, rlhold2 : usize, rlhnew : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Invert_Range_List(rlhold : usize, rlhnew : usize, ullmaxvalue : u64, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Is_Dock_Station_Present(pbpresent : *mut super::super::Foundation:: BOOL) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Is_Dock_Station_Present_Ex(pbpresent : *mut super::super::Foundation:: BOOL, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Is_Version_Available(wversion : u16) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Is_Version_Available_Ex(wversion : u16, hmachine : isize) -> super::super::Foundation:: BOOL);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Locate_DevNodeA(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCSTR, ulflags : CM_LOCATE_DEVNODE_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Locate_DevNodeW(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCWSTR, ulflags : CM_LOCATE_DEVNODE_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Locate_DevNode_ExA(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Locate_DevNode_ExW(pdndevinst : *mut u32, pdeviceid : ::windows_sys::core::PCWSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_MapCrToWin32Err(cmreturncode : CONFIGRET, defaulterr : u32) -> u32);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Merge_Range_List(rlhold1 : usize, rlhold2 : usize, rlhnew : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Modify_Res_Des(prdresdes : *mut usize, rdresdes : usize, resourceid : u32, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Modify_Res_Des_Ex(prdresdes : *mut usize, rdresdes : usize, resourceid : u32, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Move_DevNode(dnfromdevinst : u32, dntodevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Move_DevNode_Ex(dnfromdevinst : u32, dntodevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Next_Range(preelement : *mut usize, pullstart : *mut u64, pullend : *mut u64, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Class_KeyA(classguid : *const ::windows_sys::core::GUID, pszclassname : ::windows_sys::core::PCSTR, samdesired : u32, disposition : u32, phkclass : *mut super::super::System::Registry:: HKEY, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Class_KeyW(classguid : *const ::windows_sys::core::GUID, pszclassname : ::windows_sys::core::PCWSTR, samdesired : u32, disposition : u32, phkclass : *mut super::super::System::Registry:: HKEY, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Class_Key_ExA(classguid : *const ::windows_sys::core::GUID, pszclassname : ::windows_sys::core::PCSTR, samdesired : u32, disposition : u32, phkclass : *mut super::super::System::Registry:: HKEY, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Class_Key_ExW(classguid : *const ::windows_sys::core::GUID, pszclassname : ::windows_sys::core::PCWSTR, samdesired : u32, disposition : u32, phkclass : *mut super::super::System::Registry:: HKEY, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_DevNode_Key(dndevnode : u32, samdesired : u32, ulhardwareprofile : u32, disposition : u32, phkdevice : *mut super::super::System::Registry:: HKEY, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_DevNode_Key_Ex(dndevnode : u32, samdesired : u32, ulhardwareprofile : u32, disposition : u32, phkdevice : *mut super::super::System::Registry:: HKEY, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Device_Interface_KeyA(pszdeviceinterface : ::windows_sys::core::PCSTR, samdesired : u32, disposition : u32, phkdeviceinterface : *mut super::super::System::Registry:: HKEY, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Device_Interface_KeyW(pszdeviceinterface : ::windows_sys::core::PCWSTR, samdesired : u32, disposition : u32, phkdeviceinterface : *mut super::super::System::Registry:: HKEY, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Device_Interface_Key_ExA(pszdeviceinterface : ::windows_sys::core::PCSTR, samdesired : u32, disposition : u32, phkdeviceinterface : *mut super::super::System::Registry:: HKEY, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn CM_Open_Device_Interface_Key_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, samdesired : u32, disposition : u32, phkdeviceinterface : *mut super::super::System::Registry:: HKEY, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_And_Remove_SubTreeA(dnancestor : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PSTR, ulnamelength : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_And_Remove_SubTreeW(dnancestor : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PWSTR, ulnamelength : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_And_Remove_SubTree_ExA(dnancestor : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PSTR, ulnamelength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_And_Remove_SubTree_ExW(dnancestor : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PWSTR, ulnamelength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Arbitrator_Free_Data(pdata : *mut ::core::ffi::c_void, datalen : u32, dndevinst : u32, resourceid : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Arbitrator_Free_Data_Ex(pdata : *mut ::core::ffi::c_void, datalen : u32, dndevinst : u32, resourceid : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Arbitrator_Free_Size(pulsize : *mut u32, dndevinst : u32, resourceid : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Arbitrator_Free_Size_Ex(pulsize : *mut u32, dndevinst : u32, resourceid : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Remove_SubTree(dnancestor : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Remove_SubTree_Ex(dnancestor : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Query_Resource_Conflict_List(pclconflictlist : *mut usize, dndevinst : u32, resourceid : CM_RESTYPE, resourcedata : *const ::core::ffi::c_void, resourcelen : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Reenumerate_DevNode(dndevinst : u32, ulflags : CM_REENUMERATE_FLAGS) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Reenumerate_DevNode_Ex(dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Register_Device_Driver(dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Register_Device_Driver_Ex(dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Register_Device_InterfaceA(dndevinst : u32, interfaceclassguid : *const ::windows_sys::core::GUID, pszreference : ::windows_sys::core::PCSTR, pszdeviceinterface : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Register_Device_InterfaceW(dndevinst : u32, interfaceclassguid : *const ::windows_sys::core::GUID, pszreference : ::windows_sys::core::PCWSTR, pszdeviceinterface : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Register_Device_Interface_ExA(dndevinst : u32, interfaceclassguid : *const ::windows_sys::core::GUID, pszreference : ::windows_sys::core::PCSTR, pszdeviceinterface : ::windows_sys::core::PSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Register_Device_Interface_ExW(dndevinst : u32, interfaceclassguid : *const ::windows_sys::core::GUID, pszreference : ::windows_sys::core::PCWSTR, pszdeviceinterface : ::windows_sys::core::PWSTR, pullength : *mut u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CM_Register_Notification(pfilter : *const CM_NOTIFY_FILTER, pcontext : *const ::core::ffi::c_void, pcallback : PCM_NOTIFY_CALLBACK, pnotifycontext : *mut HCMNOTIFICATION) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Remove_SubTree(dnancestor : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Remove_SubTree_Ex(dnancestor : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Request_Device_EjectA(dndevinst : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PSTR, ulnamelength : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Request_Device_EjectW(dndevinst : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PWSTR, ulnamelength : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Request_Device_Eject_ExA(dndevinst : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PSTR, ulnamelength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Request_Device_Eject_ExW(dndevinst : u32, pvetotype : *mut PNP_VETO_TYPE, pszvetoname : ::windows_sys::core::PWSTR, ulnamelength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Request_Eject_PC() -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Request_Eject_PC_Ex(hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Run_Detection(ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Run_Detection_Ex(ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Set_Class_PropertyW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Set_Class_Property_ExW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_Class_Registry_PropertyA(classguid : *const ::windows_sys::core::GUID, ulproperty : u32, buffer : *const ::core::ffi::c_void, ullength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_Class_Registry_PropertyW(classguid : *const ::windows_sys::core::GUID, ulproperty : u32, buffer : *const ::core::ffi::c_void, ullength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_DevNode_Problem(dndevinst : u32, ulproblem : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_DevNode_Problem_Ex(dndevinst : u32, ulproblem : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Set_DevNode_PropertyW(dndevinst : u32, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Set_DevNode_Property_ExW(dndevinst : u32, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_DevNode_Registry_PropertyA(dndevinst : u32, ulproperty : u32, buffer : *const ::core::ffi::c_void, ullength : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_DevNode_Registry_PropertyW(dndevinst : u32, ulproperty : u32, buffer : *const ::core::ffi::c_void, ullength : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_DevNode_Registry_Property_ExA(dndevinst : u32, ulproperty : u32, buffer : *const ::core::ffi::c_void, ullength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_DevNode_Registry_Property_ExW(dndevinst : u32, ulproperty : u32, buffer : *const ::core::ffi::c_void, ullength : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Set_Device_Interface_PropertyW(pszdeviceinterface : ::windows_sys::core::PCWSTR, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, ulflags : u32) -> CONFIGRET);
#[cfg(feature = "Win32_Devices_Properties")]
::windows_targets::link!("cfgmgr32.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`"] fn CM_Set_Device_Interface_Property_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_HW_Prof(ulhardwareprofile : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_HW_Prof_Ex(ulhardwareprofile : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_HW_Prof_FlagsA(pdeviceid : ::windows_sys::core::PCSTR, ulconfig : u32, ulvalue : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_HW_Prof_FlagsW(pdeviceid : ::windows_sys::core::PCWSTR, ulconfig : u32, ulvalue : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_HW_Prof_Flags_ExA(pdeviceid : ::windows_sys::core::PCSTR, ulconfig : u32, ulvalue : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Set_HW_Prof_Flags_ExW(pdeviceid : ::windows_sys::core::PCWSTR, ulconfig : u32, ulvalue : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Setup_DevNode(dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Setup_DevNode_Ex(dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Test_Range_Available(ullstartvalue : u64, ullendvalue : u64, rlh : usize, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Uninstall_DevNode(dndevinst : u32, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Uninstall_DevNode_Ex(dndevinst : u32, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Unregister_Device_InterfaceA(pszdeviceinterface : ::windows_sys::core::PCSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Unregister_Device_InterfaceW(pszdeviceinterface : ::windows_sys::core::PCWSTR, ulflags : u32) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Unregister_Device_Interface_ExA(pszdeviceinterface : ::windows_sys::core::PCSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Unregister_Device_Interface_ExW(pszdeviceinterface : ::windows_sys::core::PCWSTR, ulflags : u32, hmachine : isize) -> CONFIGRET);
::windows_targets::link!("cfgmgr32.dll" "system" fn CM_Unregister_Notification(notifycontext : HCMNOTIFICATION) -> CONFIGRET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiInstallDevice(hwndparent : super::super::Foundation:: HWND, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_W, flags : DIINSTALLDEVICE_FLAGS, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiInstallDriverA(hwndparent : super::super::Foundation:: HWND, infpath : ::windows_sys::core::PCSTR, flags : DIINSTALLDRIVER_FLAGS, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiInstallDriverW(hwndparent : super::super::Foundation:: HWND, infpath : ::windows_sys::core::PCWSTR, flags : DIINSTALLDRIVER_FLAGS, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiRollbackDriver(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, hwndparent : super::super::Foundation:: HWND, flags : DIROLLBACKDRIVER_FLAGS, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiShowUpdateDevice(hwndparent : super::super::Foundation:: HWND, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, flags : u32, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiShowUpdateDriver(hwndparent : super::super::Foundation:: HWND, filepath : ::windows_sys::core::PCWSTR, flags : u32, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiUninstallDevice(hwndparent : super::super::Foundation:: HWND, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, flags : u32, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiUninstallDriverA(hwndparent : super::super::Foundation:: HWND, infpath : ::windows_sys::core::PCSTR, flags : DIUNINSTALLDRIVER_FLAGS, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DiUninstallDriverW(hwndparent : super::super::Foundation:: HWND, infpath : ::windows_sys::core::PCWSTR, flags : DIUNINSTALLDRIVER_FLAGS, needreboot : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InstallHinfSectionA(window : super::super::Foundation:: HWND, modulehandle : super::super::Foundation:: HINSTANCE, commandline : ::windows_sys::core::PCSTR, showcommand : i32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn InstallHinfSectionW(window : super::super::Foundation:: HWND, modulehandle : super::super::Foundation:: HINSTANCE, commandline : ::windows_sys::core::PCWSTR, showcommand : i32) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddInstallSectionToDiskSpaceListA(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, layoutinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddInstallSectionToDiskSpaceListW(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, layoutinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddSectionToDiskSpaceListA(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddSectionToDiskSpaceListW(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddToDiskSpaceListA(diskspace : *const ::core::ffi::c_void, targetfilespec : ::windows_sys::core::PCSTR, filesize : i64, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddToDiskSpaceListW(diskspace : *const ::core::ffi::c_void, targetfilespec : ::windows_sys::core::PCWSTR, filesize : i64, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddToSourceListA(flags : u32, source : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAddToSourceListW(flags : u32, source : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAdjustDiskSpaceListA(diskspace : *const ::core::ffi::c_void, driveroot : ::windows_sys::core::PCSTR, amount : i64, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupAdjustDiskSpaceListW(diskspace : *const ::core::ffi::c_void, driveroot : ::windows_sys::core::PCWSTR, amount : i64, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupBackupErrorA(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCSTR, sourcefile : ::windows_sys::core::PCSTR, targetfile : ::windows_sys::core::PCSTR, win32errorcode : u32, style : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupBackupErrorW(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCWSTR, sourcefile : ::windows_sys::core::PCWSTR, targetfile : ::windows_sys::core::PCWSTR, win32errorcode : u32, style : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCancelTemporarySourceList() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCloseFileQueue(queuehandle : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupCloseInfFile(infhandle : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("setupapi.dll" "system" fn SetupCloseLog() -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCommitFileQueueA(owner : super::super::Foundation:: HWND, queuehandle : *const ::core::ffi::c_void, msghandler : PSP_FILE_CALLBACK_A, context : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCommitFileQueueW(owner : super::super::Foundation:: HWND, queuehandle : *const ::core::ffi::c_void, msghandler : PSP_FILE_CALLBACK_W, context : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupConfigureWmiFromInfSectionA(infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupConfigureWmiFromInfSectionW(infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCopyErrorA(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCSTR, diskname : ::windows_sys::core::PCSTR, pathtosource : ::windows_sys::core::PCSTR, sourcefile : ::windows_sys::core::PCSTR, targetpathfile : ::windows_sys::core::PCSTR, win32errorcode : u32, style : u32, pathbuffer : ::windows_sys::core::PSTR, pathbuffersize : u32, pathrequiredsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCopyErrorW(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCWSTR, diskname : ::windows_sys::core::PCWSTR, pathtosource : ::windows_sys::core::PCWSTR, sourcefile : ::windows_sys::core::PCWSTR, targetpathfile : ::windows_sys::core::PCWSTR, win32errorcode : u32, style : u32, pathbuffer : ::windows_sys::core::PWSTR, pathbuffersize : u32, pathrequiredsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCopyOEMInfA(sourceinffilename : ::windows_sys::core::PCSTR, oemsourcemedialocation : ::windows_sys::core::PCSTR, oemsourcemediatype : OEM_SOURCE_MEDIA_TYPE, copystyle : SP_COPY_STYLE, destinationinffilename : ::windows_sys::core::PSTR, destinationinffilenamesize : u32, requiredsize : *mut u32, destinationinffilenamecomponent : *mut ::windows_sys::core::PSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupCopyOEMInfW(sourceinffilename : ::windows_sys::core::PCWSTR, oemsourcemedialocation : ::windows_sys::core::PCWSTR, oemsourcemediatype : OEM_SOURCE_MEDIA_TYPE, copystyle : SP_COPY_STYLE, destinationinffilename : ::windows_sys::core::PWSTR, destinationinffilenamesize : u32, requiredsize : *mut u32, destinationinffilenamecomponent : *mut ::windows_sys::core::PWSTR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupCreateDiskSpaceListA(reserved1 : *const ::core::ffi::c_void, reserved2 : u32, flags : u32) -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupCreateDiskSpaceListW(reserved1 : *const ::core::ffi::c_void, reserved2 : u32, flags : u32) -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupDecompressOrCopyFileA(sourcefilename : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR, compressiontype : *const u32) -> u32);
::windows_targets::link!("setupapi.dll" "system" fn SetupDecompressOrCopyFileW(sourcefilename : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR, compressiontype : *const u32) -> u32);
::windows_targets::link!("setupapi.dll" "system" fn SetupDefaultQueueCallbackA(context : *const ::core::ffi::c_void, notification : u32, param1 : usize, param2 : usize) -> u32);
::windows_targets::link!("setupapi.dll" "system" fn SetupDefaultQueueCallbackW(context : *const ::core::ffi::c_void, notification : u32, param1 : usize, param2 : usize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDeleteErrorA(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCSTR, file : ::windows_sys::core::PCSTR, win32errorcode : u32, style : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDeleteErrorW(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCWSTR, file : ::windows_sys::core::PCWSTR, win32errorcode : u32, style : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDestroyDiskSpaceList(diskspace : *mut ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiAskForOEMDisk(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiBuildClassInfoList(flags : u32, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiBuildClassInfoListExA(flags : u32, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiBuildClassInfoListExW(flags : u32, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiBuildDriverInfoList(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA, drivertype : SETUP_DI_BUILD_DRIVER_DRIVER_TYPE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCallClassInstaller(installfunction : u32, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCancelDriverInfoSearch(deviceinfoset : HDEVINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiChangeState(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassGuidsFromNameA(classname : ::windows_sys::core::PCSTR, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassGuidsFromNameExA(classname : ::windows_sys::core::PCSTR, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassGuidsFromNameExW(classname : ::windows_sys::core::PCWSTR, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassGuidsFromNameW(classname : ::windows_sys::core::PCWSTR, classguidlist : *mut ::windows_sys::core::GUID, classguidlistsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassNameFromGuidA(classguid : *const ::windows_sys::core::GUID, classname : ::windows_sys::core::PSTR, classnamesize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassNameFromGuidExA(classguid : *const ::windows_sys::core::GUID, classname : ::windows_sys::core::PSTR, classnamesize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassNameFromGuidExW(classguid : *const ::windows_sys::core::GUID, classname : ::windows_sys::core::PWSTR, classnamesize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiClassNameFromGuidW(classguid : *const ::windows_sys::core::GUID, classname : ::windows_sys::core::PWSTR, classnamesize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiCreateDevRegKeyA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, scope : u32, hwprofile : u32, keytype : u32, infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCSTR) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiCreateDevRegKeyW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, scope : u32, hwprofile : u32, keytype : u32, infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCWSTR) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInfoA(deviceinfoset : HDEVINFO, devicename : ::windows_sys::core::PCSTR, classguid : *const ::windows_sys::core::GUID, devicedescription : ::windows_sys::core::PCSTR, hwndparent : super::super::Foundation:: HWND, creationflags : u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInfoList(classguid : *const ::windows_sys::core::GUID, hwndparent : super::super::Foundation:: HWND) -> HDEVINFO);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInfoListExA(classguid : *const ::windows_sys::core::GUID, hwndparent : super::super::Foundation:: HWND, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> HDEVINFO);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInfoListExW(classguid : *const ::windows_sys::core::GUID, hwndparent : super::super::Foundation:: HWND, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> HDEVINFO);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInfoW(deviceinfoset : HDEVINFO, devicename : ::windows_sys::core::PCWSTR, classguid : *const ::windows_sys::core::GUID, devicedescription : ::windows_sys::core::PCWSTR, hwndparent : super::super::Foundation:: HWND, creationflags : u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInterfaceA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, interfaceclassguid : *const ::windows_sys::core::GUID, referencestring : ::windows_sys::core::PCSTR, creationflags : u32, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiCreateDeviceInterfaceRegKeyA(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, reserved : u32, samdesired : u32, infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCSTR) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiCreateDeviceInterfaceRegKeyW(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, reserved : u32, samdesired : u32, infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCWSTR) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiCreateDeviceInterfaceW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, interfaceclassguid : *const ::windows_sys::core::GUID, referencestring : ::windows_sys::core::PCWSTR, creationflags : u32, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiDeleteDevRegKey(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, scope : u32, hwprofile : u32, keytype : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiDeleteDeviceInfo(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiDeleteDeviceInterfaceData(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiDeleteDeviceInterfaceRegKey(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, reserved : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn SetupDiDestroyClassImageList(classimagelistdata : *const SP_CLASSIMAGELIST_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiDestroyDeviceInfoList(deviceinfoset : HDEVINFO) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiDestroyDriverInfoList(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, drivertype : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn SetupDiDrawMiniIcon(hdc : super::super::Graphics::Gdi:: HDC, rc : super::super::Foundation:: RECT, miniiconindex : i32, flags : u32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiEnumDeviceInfo(deviceinfoset : HDEVINFO, memberindex : u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiEnumDeviceInterfaces(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, interfaceclassguid : *const ::windows_sys::core::GUID, memberindex : u32, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiEnumDriverInfoA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, drivertype : u32, memberindex : u32, driverinfodata : *mut SP_DRVINFO_DATA_V2_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiEnumDriverInfoW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, drivertype : u32, memberindex : u32, driverinfodata : *mut SP_DRVINFO_DATA_V2_W) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupDiGetActualModelsSectionA(context : *const INFCONTEXT, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, infsectionwithext : ::windows_sys::core::PSTR, infsectionwithextsize : u32, requiredsize : *mut u32, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupDiGetActualModelsSectionW(context : *const INFCONTEXT, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, infsectionwithext : ::windows_sys::core::PWSTR, infsectionwithextsize : u32, requiredsize : *mut u32, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetActualSectionToInstallA(infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCSTR, infsectionwithext : ::windows_sys::core::PSTR, infsectionwithextsize : u32, requiredsize : *mut u32, extension : *mut ::windows_sys::core::PSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupDiGetActualSectionToInstallExA(infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCSTR, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, infsectionwithext : ::windows_sys::core::PSTR, infsectionwithextsize : u32, requiredsize : *mut u32, extension : *mut ::windows_sys::core::PSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupDiGetActualSectionToInstallExW(infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCWSTR, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, infsectionwithext : ::windows_sys::core::PWSTR, infsectionwithextsize : u32, requiredsize : *mut u32, extension : *mut ::windows_sys::core::PWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetActualSectionToInstallW(infhandle : *const ::core::ffi::c_void, infsectionname : ::windows_sys::core::PCWSTR, infsectionwithext : ::windows_sys::core::PWSTR, infsectionwithextsize : u32, requiredsize : *mut u32, extension : *mut ::windows_sys::core::PWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassBitmapIndex(classguid : *const ::windows_sys::core::GUID, miniiconindex : *mut i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDescriptionA(classguid : *const ::windows_sys::core::GUID, classdescription : ::windows_sys::core::PSTR, classdescriptionsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDescriptionExA(classguid : *const ::windows_sys::core::GUID, classdescription : ::windows_sys::core::PSTR, classdescriptionsize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDescriptionExW(classguid : *const ::windows_sys::core::GUID, classdescription : ::windows_sys::core::PWSTR, classdescriptionsize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDescriptionW(classguid : *const ::windows_sys::core::GUID, classdescription : ::windows_sys::core::PWSTR, classdescriptionsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_UI_Controls", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_UI_Controls\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn SetupDiGetClassDevPropertySheetsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, propertysheetheader : *const super::super::UI::Controls:: PROPSHEETHEADERA_V2, propertysheetheaderpagelistsize : u32, requiredsize : *mut u32, propertysheettype : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi", feature = "Win32_UI_Controls", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`, `\"Win32_UI_Controls\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn SetupDiGetClassDevPropertySheetsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, propertysheetheader : *const super::super::UI::Controls:: PROPSHEETHEADERW_V2, propertysheetheaderpagelistsize : u32, requiredsize : *mut u32, propertysheettype : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDevsA(classguid : *const ::windows_sys::core::GUID, enumerator : ::windows_sys::core::PCSTR, hwndparent : super::super::Foundation:: HWND, flags : u32) -> HDEVINFO);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDevsExA(classguid : *const ::windows_sys::core::GUID, enumerator : ::windows_sys::core::PCSTR, hwndparent : super::super::Foundation:: HWND, flags : u32, deviceinfoset : HDEVINFO, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> HDEVINFO);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDevsExW(classguid : *const ::windows_sys::core::GUID, enumerator : ::windows_sys::core::PCWSTR, hwndparent : super::super::Foundation:: HWND, flags : u32, deviceinfoset : HDEVINFO, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> HDEVINFO);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassDevsW(classguid : *const ::windows_sys::core::GUID, enumerator : ::windows_sys::core::PCWSTR, hwndparent : super::super::Foundation:: HWND, flags : u32) -> HDEVINFO);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn SetupDiGetClassImageIndex(classimagelistdata : *const SP_CLASSIMAGELIST_DATA, classguid : *const ::windows_sys::core::GUID, imageindex : *mut i32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn SetupDiGetClassImageList(classimagelistdata : *mut SP_CLASSIMAGELIST_DATA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn SetupDiGetClassImageListExA(classimagelistdata : *mut SP_CLASSIMAGELIST_DATA, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn SetupDiGetClassImageListExW(classimagelistdata : *mut SP_CLASSIMAGELIST_DATA, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassInstallParamsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, classinstallparams : *mut SP_CLASSINSTALL_HEADER, classinstallparamssize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassInstallParamsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, classinstallparams : *mut SP_CLASSINSTALL_HEADER, classinstallparamssize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetClassPropertyExW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32, flags : u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetClassPropertyKeys(classguid : *const ::windows_sys::core::GUID, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : u32, requiredpropertykeycount : *mut u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetClassPropertyKeysExW(classguid : *const ::windows_sys::core::GUID, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : u32, requiredpropertykeycount : *mut u32, flags : u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetClassPropertyW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassRegistryPropertyA(classguid : *const ::windows_sys::core::GUID, property : u32, propertyregdatatype : *mut u32, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetClassRegistryPropertyW(classguid : *const ::windows_sys::core::GUID, property : u32, propertyregdatatype : *mut u32, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetCustomDevicePropertyA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, custompropertyname : ::windows_sys::core::PCSTR, flags : u32, propertyregdatatype : *mut u32, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetCustomDevicePropertyW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, custompropertyname : ::windows_sys::core::PCWSTR, flags : u32, propertyregdatatype : *mut u32, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInfoListClass(deviceinfoset : HDEVINFO, classguid : *mut ::windows_sys::core::GUID) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInfoListDetailA(deviceinfoset : HDEVINFO, deviceinfosetdetaildata : *mut SP_DEVINFO_LIST_DETAIL_DATA_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInfoListDetailW(deviceinfoset : HDEVINFO, deviceinfosetdetaildata : *mut SP_DEVINFO_LIST_DETAIL_DATA_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInstallParamsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, deviceinstallparams : *mut SP_DEVINSTALL_PARAMS_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInstallParamsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, deviceinstallparams : *mut SP_DEVINSTALL_PARAMS_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInstanceIdA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, deviceinstanceid : ::windows_sys::core::PSTR, deviceinstanceidsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInstanceIdW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, deviceinstanceid : ::windows_sys::core::PWSTR, deviceinstanceidsize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInterfaceAlias(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, aliasinterfaceclassguid : *const ::windows_sys::core::GUID, aliasdeviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInterfaceDetailA(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, deviceinterfacedetaildata : *mut SP_DEVICE_INTERFACE_DETAIL_DATA_A, deviceinterfacedetaildatasize : u32, requiredsize : *mut u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInterfaceDetailW(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, deviceinterfacedetaildata : *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W, deviceinterfacedetaildatasize : u32, requiredsize : *mut u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInterfacePropertyKeys(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : u32, requiredpropertykeycount : *mut u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetDeviceInterfacePropertyW(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetDevicePropertyKeys(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, propertykeyarray : *mut super::Properties:: DEVPROPKEY, propertykeycount : u32, requiredpropertykeycount : *mut u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiGetDevicePropertyW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : *mut super::Properties:: DEVPROPTYPE, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceRegistryPropertyA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, property : u32, propertyregdatatype : *mut u32, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDeviceRegistryPropertyW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, property : u32, propertyregdatatype : *mut u32, propertybuffer : *mut u8, propertybuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDriverInfoDetailA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_A, driverinfodetaildata : *mut SP_DRVINFO_DETAIL_DATA_A, driverinfodetaildatasize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDriverInfoDetailW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_W, driverinfodetaildata : *mut SP_DRVINFO_DETAIL_DATA_W, driverinfodetaildatasize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDriverInstallParamsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_A, driverinstallparams : *mut SP_DRVINSTALL_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetDriverInstallParamsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_W, driverinstallparams : *mut SP_DRVINSTALL_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileFriendlyNameA(hwprofile : u32, friendlyname : ::windows_sys::core::PSTR, friendlynamesize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileFriendlyNameExA(hwprofile : u32, friendlyname : ::windows_sys::core::PSTR, friendlynamesize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileFriendlyNameExW(hwprofile : u32, friendlyname : ::windows_sys::core::PWSTR, friendlynamesize : u32, requiredsize : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileFriendlyNameW(hwprofile : u32, friendlyname : ::windows_sys::core::PWSTR, friendlynamesize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileList(hwprofilelist : *mut u32, hwprofilelistsize : u32, requiredsize : *mut u32, currentlyactiveindex : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileListExA(hwprofilelist : *mut u32, hwprofilelistsize : u32, requiredsize : *mut u32, currentlyactiveindex : *mut u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetHwProfileListExW(hwprofilelist : *mut u32, hwprofilelistsize : u32, requiredsize : *mut u32, currentlyactiveindex : *mut u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetINFClassA(infname : ::windows_sys::core::PCSTR, classguid : *mut ::windows_sys::core::GUID, classname : ::windows_sys::core::PSTR, classnamesize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetINFClassW(infname : ::windows_sys::core::PCWSTR, classguid : *mut ::windows_sys::core::GUID, classname : ::windows_sys::core::PWSTR, classnamesize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetSelectedDevice(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetSelectedDriverA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *mut SP_DRVINFO_DATA_V2_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiGetSelectedDriverW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *mut SP_DRVINFO_DATA_V2_W) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"] fn SetupDiGetWizardPage(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, installwizarddata : *const SP_INSTALLWIZARD_DATA, pagetype : u32, flags : u32) -> super::super::UI::Controls:: HPROPSHEETPAGE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallClassA(hwndparent : super::super::Foundation:: HWND, inffilename : ::windows_sys::core::PCSTR, flags : u32, filequeue : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallClassExA(hwndparent : super::super::Foundation:: HWND, inffilename : ::windows_sys::core::PCSTR, flags : u32, filequeue : *const ::core::ffi::c_void, interfaceclassguid : *const ::windows_sys::core::GUID, reserved1 : *const ::core::ffi::c_void, reserved2 : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallClassExW(hwndparent : super::super::Foundation:: HWND, inffilename : ::windows_sys::core::PCWSTR, flags : u32, filequeue : *const ::core::ffi::c_void, interfaceclassguid : *const ::windows_sys::core::GUID, reserved1 : *const ::core::ffi::c_void, reserved2 : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallClassW(hwndparent : super::super::Foundation:: HWND, inffilename : ::windows_sys::core::PCWSTR, flags : u32, filequeue : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallDevice(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallDeviceInterfaces(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiInstallDriverFiles(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn SetupDiLoadClassIcon(classguid : *const ::windows_sys::core::GUID, largeicon : *mut super::super::UI::WindowsAndMessaging:: HICON, miniiconindex : *mut i32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_WindowsAndMessaging"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_WindowsAndMessaging\"`"] fn SetupDiLoadDeviceIcon(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, cxicon : u32, cyicon : u32, flags : u32, hicon : *mut super::super::UI::WindowsAndMessaging:: HICON) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiOpenClassRegKey(classguid : *const ::windows_sys::core::GUID, samdesired : u32) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiOpenClassRegKeyExA(classguid : *const ::windows_sys::core::GUID, samdesired : u32, flags : u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiOpenClassRegKeyExW(classguid : *const ::windows_sys::core::GUID, samdesired : u32, flags : u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiOpenDevRegKey(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, scope : u32, hwprofile : u32, keytype : u32, samdesired : u32) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiOpenDeviceInfoA(deviceinfoset : HDEVINFO, deviceinstanceid : ::windows_sys::core::PCSTR, hwndparent : super::super::Foundation:: HWND, openflags : u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiOpenDeviceInfoW(deviceinfoset : HDEVINFO, deviceinstanceid : ::windows_sys::core::PCWSTR, hwndparent : super::super::Foundation:: HWND, openflags : u32, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiOpenDeviceInterfaceA(deviceinfoset : HDEVINFO, devicepath : ::windows_sys::core::PCSTR, openflags : u32, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn SetupDiOpenDeviceInterfaceRegKey(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, reserved : u32, samdesired : u32) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiOpenDeviceInterfaceW(deviceinfoset : HDEVINFO, devicepath : ::windows_sys::core::PCWSTR, openflags : u32, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiRegisterCoDeviceInstallers(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiRegisterDeviceInfo(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA, flags : u32, compareproc : PSP_DETSIG_CMPPROC, comparecontext : *const ::core::ffi::c_void, dupdeviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiRemoveDevice(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiRemoveDeviceInterface(deviceinfoset : HDEVINFO, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiRestartDevices(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSelectBestCompatDrv(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSelectDevice(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSelectOEMDrv(hwndparent : super::super::Foundation:: HWND, deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetClassInstallParamsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, classinstallparams : *const SP_CLASSINSTALL_HEADER, classinstallparamssize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetClassInstallParamsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, classinstallparams : *const SP_CLASSINSTALL_HEADER, classinstallparamssize : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiSetClassPropertyExW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, flags : u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiSetClassPropertyW(classguid : *const ::windows_sys::core::GUID, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetClassRegistryPropertyA(classguid : *const ::windows_sys::core::GUID, property : u32, propertybuffer : *const u8, propertybuffersize : u32, machinename : ::windows_sys::core::PCSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetClassRegistryPropertyW(classguid : *const ::windows_sys::core::GUID, property : u32, propertybuffer : *const u8, propertybuffersize : u32, machinename : ::windows_sys::core::PCWSTR, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDeviceInstallParamsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, deviceinstallparams : *const SP_DEVINSTALL_PARAMS_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDeviceInstallParamsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, deviceinstallparams : *const SP_DEVINSTALL_PARAMS_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDeviceInterfaceDefault(deviceinfoset : HDEVINFO, deviceinterfacedata : *mut SP_DEVICE_INTERFACE_DATA, flags : u32, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiSetDeviceInterfacePropertyW(deviceinfoset : HDEVINFO, deviceinterfacedata : *const SP_DEVICE_INTERFACE_DATA, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Devices_Properties", feature = "Win32_Foundation"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Devices_Properties\"`, `\"Win32_Foundation\"`"] fn SetupDiSetDevicePropertyW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, propertykey : *const super::Properties:: DEVPROPKEY, propertytype : super::Properties:: DEVPROPTYPE, propertybuffer : *const u8, propertybuffersize : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDeviceRegistryPropertyA(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA, property : u32, propertybuffer : *const u8, propertybuffersize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDeviceRegistryPropertyW(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA, property : u32, propertybuffer : *const u8, propertybuffersize : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDriverInstallParamsA(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_A, driverinstallparams : *const SP_DRVINSTALL_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetDriverInstallParamsW(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, driverinfodata : *const SP_DRVINFO_DATA_V2_W, driverinstallparams : *const SP_DRVINSTALL_PARAMS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetSelectedDevice(deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetSelectedDriverA(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA, driverinfodata : *mut SP_DRVINFO_DATA_V2_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiSetSelectedDriverW(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA, driverinfodata : *mut SP_DRVINFO_DATA_V2_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupDiUnremoveDevice(deviceinfoset : HDEVINFO, deviceinfodata : *mut SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupDuplicateDiskSpaceListA(diskspace : *const ::core::ffi::c_void, reserved1 : *const ::core::ffi::c_void, reserved2 : u32, flags : u32) -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupDuplicateDiskSpaceListW(diskspace : *const ::core::ffi::c_void, reserved1 : *const ::core::ffi::c_void, reserved2 : u32, flags : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupEnumInfSectionsA(infhandle : *const ::core::ffi::c_void, index : u32, buffer : ::windows_sys::core::PSTR, size : u32, sizeneeded : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupEnumInfSectionsW(infhandle : *const ::core::ffi::c_void, index : u32, buffer : ::windows_sys::core::PWSTR, size : u32, sizeneeded : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFindFirstLineA(infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR, key : ::windows_sys::core::PCSTR, context : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFindFirstLineW(infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR, key : ::windows_sys::core::PCWSTR, context : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFindNextLine(contextin : *const INFCONTEXT, contextout : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFindNextMatchLineA(contextin : *const INFCONTEXT, key : ::windows_sys::core::PCSTR, contextout : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFindNextMatchLineW(contextin : *const INFCONTEXT, key : ::windows_sys::core::PCWSTR, contextout : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFreeSourceListA(list : *mut *mut ::windows_sys::core::PCSTR, count : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupFreeSourceListW(list : *mut *mut ::windows_sys::core::PCWSTR, count : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetBackupInformationA(queuehandle : *const ::core::ffi::c_void, backupparams : *mut SP_BACKUP_QUEUE_PARAMS_V2_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetBackupInformationW(queuehandle : *const ::core::ffi::c_void, backupparams : *mut SP_BACKUP_QUEUE_PARAMS_V2_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetBinaryField(context : *const INFCONTEXT, fieldindex : u32, returnbuffer : *mut u8, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupGetFieldCount(context : *const INFCONTEXT) -> u32);
::windows_targets::link!("setupapi.dll" "system" fn SetupGetFileCompressionInfoA(sourcefilename : ::windows_sys::core::PCSTR, actualsourcefilename : *mut ::windows_sys::core::PSTR, sourcefilesize : *mut u32, targetfilesize : *mut u32, compressiontype : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetFileCompressionInfoExA(sourcefilename : ::windows_sys::core::PCSTR, actualsourcefilenamebuffer : ::windows_sys::core::PCSTR, actualsourcefilenamebufferlen : u32, requiredbufferlen : *mut u32, sourcefilesize : *mut u32, targetfilesize : *mut u32, compressiontype : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetFileCompressionInfoExW(sourcefilename : ::windows_sys::core::PCWSTR, actualsourcefilenamebuffer : ::windows_sys::core::PCWSTR, actualsourcefilenamebufferlen : u32, requiredbufferlen : *mut u32, sourcefilesize : *mut u32, targetfilesize : *mut u32, compressiontype : *mut u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupGetFileCompressionInfoW(sourcefilename : ::windows_sys::core::PCWSTR, actualsourcefilename : *mut ::windows_sys::core::PWSTR, sourcefilesize : *mut u32, targetfilesize : *mut u32, compressiontype : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetFileQueueCount(filequeue : *const ::core::ffi::c_void, subqueuefileop : u32, numoperations : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetFileQueueFlags(filequeue : *const ::core::ffi::c_void, flags : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupGetInfDriverStoreLocationA(filename : ::windows_sys::core::PCSTR, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, localename : ::windows_sys::core::PCSTR, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupGetInfDriverStoreLocationW(filename : ::windows_sys::core::PCWSTR, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, localename : ::windows_sys::core::PCWSTR, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetInfFileListA(directorypath : ::windows_sys::core::PCSTR, infstyle : INF_STYLE, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetInfFileListW(directorypath : ::windows_sys::core::PCWSTR, infstyle : INF_STYLE, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetInfInformationA(infspec : *const ::core::ffi::c_void, searchcontrol : u32, returnbuffer : *mut SP_INF_INFORMATION, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetInfInformationW(infspec : *const ::core::ffi::c_void, searchcontrol : u32, returnbuffer : *mut SP_INF_INFORMATION, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetInfPublishedNameA(driverstorelocation : ::windows_sys::core::PCSTR, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetInfPublishedNameW(driverstorelocation : ::windows_sys::core::PCWSTR, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetIntField(context : *const INFCONTEXT, fieldindex : u32, integervalue : *mut i32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetLineByIndexA(infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR, index : u32, context : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetLineByIndexW(infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR, index : u32, context : *mut INFCONTEXT) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupGetLineCountA(infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("setupapi.dll" "system" fn SetupGetLineCountW(infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetLineTextA(context : *const INFCONTEXT, infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR, key : ::windows_sys::core::PCSTR, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetLineTextW(context : *const INFCONTEXT, infhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR, key : ::windows_sys::core::PCWSTR, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetMultiSzFieldA(context : *const INFCONTEXT, fieldindex : u32, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetMultiSzFieldW(context : *const INFCONTEXT, fieldindex : u32, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetNonInteractiveMode() -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetSourceFileLocationA(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, filename : ::windows_sys::core::PCSTR, sourceid : *mut u32, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetSourceFileLocationW(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, filename : ::windows_sys::core::PCWSTR, sourceid : *mut u32, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetSourceFileSizeA(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, filename : ::windows_sys::core::PCSTR, section : ::windows_sys::core::PCSTR, filesize : *mut u32, roundingfactor : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetSourceFileSizeW(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, filename : ::windows_sys::core::PCWSTR, section : ::windows_sys::core::PCWSTR, filesize : *mut u32, roundingfactor : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetSourceInfoA(infhandle : *const ::core::ffi::c_void, sourceid : u32, infodesired : u32, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetSourceInfoW(infhandle : *const ::core::ffi::c_void, sourceid : u32, infodesired : u32, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetStringFieldA(context : *const INFCONTEXT, fieldindex : u32, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetStringFieldW(context : *const INFCONTEXT, fieldindex : u32, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetTargetPathA(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, section : ::windows_sys::core::PCSTR, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupGetTargetPathW(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, section : ::windows_sys::core::PCWSTR, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupGetThreadLogToken() -> u64);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInitDefaultQueueCallback(ownerwindow : super::super::Foundation:: HWND) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInitDefaultQueueCallbackEx(ownerwindow : super::super::Foundation:: HWND, alternateprogresswindow : super::super::Foundation:: HWND, progressmessage : u32, reserved1 : u32, reserved2 : *const ::core::ffi::c_void) -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupInitializeFileLogA(logfilename : ::windows_sys::core::PCSTR, flags : u32) -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupInitializeFileLogW(logfilename : ::windows_sys::core::PCWSTR, flags : u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallFileA(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, sourcefile : ::windows_sys::core::PCSTR, sourcepathroot : ::windows_sys::core::PCSTR, destinationname : ::windows_sys::core::PCSTR, copystyle : SP_COPY_STYLE, copymsghandler : PSP_FILE_CALLBACK_A, context : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallFileExA(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, sourcefile : ::windows_sys::core::PCSTR, sourcepathroot : ::windows_sys::core::PCSTR, destinationname : ::windows_sys::core::PCSTR, copystyle : SP_COPY_STYLE, copymsghandler : PSP_FILE_CALLBACK_A, context : *const ::core::ffi::c_void, filewasinuse : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallFileExW(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, sourcefile : ::windows_sys::core::PCWSTR, sourcepathroot : ::windows_sys::core::PCWSTR, destinationname : ::windows_sys::core::PCWSTR, copystyle : SP_COPY_STYLE, copymsghandler : PSP_FILE_CALLBACK_W, context : *const ::core::ffi::c_void, filewasinuse : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallFileW(infhandle : *const ::core::ffi::c_void, infcontext : *const INFCONTEXT, sourcefile : ::windows_sys::core::PCWSTR, sourcepathroot : ::windows_sys::core::PCWSTR, destinationname : ::windows_sys::core::PCWSTR, copystyle : SP_COPY_STYLE, copymsghandler : PSP_FILE_CALLBACK_W, context : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallFilesFromInfSectionA(infhandle : *const ::core::ffi::c_void, layoutinfhandle : *const ::core::ffi::c_void, filequeue : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, sourcerootpath : ::windows_sys::core::PCSTR, copyflags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallFilesFromInfSectionW(infhandle : *const ::core::ffi::c_void, layoutinfhandle : *const ::core::ffi::c_void, filequeue : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, sourcerootpath : ::windows_sys::core::PCWSTR, copyflags : u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn SetupInstallFromInfSectionA(owner : super::super::Foundation:: HWND, infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, flags : u32, relativekeyroot : super::super::System::Registry:: HKEY, sourcerootpath : ::windows_sys::core::PCSTR, copyflags : u32, msghandler : PSP_FILE_CALLBACK_A, context : *const ::core::ffi::c_void, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn SetupInstallFromInfSectionW(owner : super::super::Foundation:: HWND, infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, flags : u32, relativekeyroot : super::super::System::Registry:: HKEY, sourcerootpath : ::windows_sys::core::PCWSTR, copyflags : u32, msghandler : PSP_FILE_CALLBACK_W, context : *const ::core::ffi::c_void, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallServicesFromInfSectionA(infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, flags : SPSVCINST_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallServicesFromInfSectionExA(infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, flags : SPSVCINST_FLAGS, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, reserved1 : *const ::core::ffi::c_void, reserved2 : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallServicesFromInfSectionExW(infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, flags : SPSVCINST_FLAGS, deviceinfoset : HDEVINFO, deviceinfodata : *const SP_DEVINFO_DATA, reserved1 : *const ::core::ffi::c_void, reserved2 : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupInstallServicesFromInfSectionW(infhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, flags : SPSVCINST_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupIterateCabinetA(cabinetfile : ::windows_sys::core::PCSTR, reserved : u32, msghandler : PSP_FILE_CALLBACK_A, context : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupIterateCabinetW(cabinetfile : ::windows_sys::core::PCWSTR, reserved : u32, msghandler : PSP_FILE_CALLBACK_W, context : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupLogErrorA(messagestring : ::windows_sys::core::PCSTR, severity : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupLogErrorW(messagestring : ::windows_sys::core::PCWSTR, severity : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupLogFileA(fileloghandle : *const ::core::ffi::c_void, logsectionname : ::windows_sys::core::PCSTR, sourcefilename : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR, checksum : u32, disktagfile : ::windows_sys::core::PCSTR, diskdescription : ::windows_sys::core::PCSTR, otherinfo : ::windows_sys::core::PCSTR, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupLogFileW(fileloghandle : *const ::core::ffi::c_void, logsectionname : ::windows_sys::core::PCWSTR, sourcefilename : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR, checksum : u32, disktagfile : ::windows_sys::core::PCWSTR, diskdescription : ::windows_sys::core::PCWSTR, otherinfo : ::windows_sys::core::PCWSTR, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupOpenAppendInfFileA(filename : ::windows_sys::core::PCSTR, infhandle : *const ::core::ffi::c_void, errorline : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupOpenAppendInfFileW(filename : ::windows_sys::core::PCWSTR, infhandle : *const ::core::ffi::c_void, errorline : *mut u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupOpenFileQueue() -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupOpenInfFileA(filename : ::windows_sys::core::PCSTR, infclass : ::windows_sys::core::PCSTR, infstyle : INF_STYLE, errorline : *mut u32) -> *mut ::core::ffi::c_void);
::windows_targets::link!("setupapi.dll" "system" fn SetupOpenInfFileW(filename : ::windows_sys::core::PCWSTR, infclass : ::windows_sys::core::PCWSTR, infstyle : INF_STYLE, errorline : *mut u32) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupOpenLog(erase : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupOpenMasterInf() -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupPrepareQueueForRestoreA(queuehandle : *const ::core::ffi::c_void, backuppath : ::windows_sys::core::PCSTR, restoreflags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupPrepareQueueForRestoreW(queuehandle : *const ::core::ffi::c_void, backuppath : ::windows_sys::core::PCWSTR, restoreflags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupPromptForDiskA(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCSTR, diskname : ::windows_sys::core::PCSTR, pathtosource : ::windows_sys::core::PCSTR, filesought : ::windows_sys::core::PCSTR, tagfile : ::windows_sys::core::PCSTR, diskpromptstyle : u32, pathbuffer : ::windows_sys::core::PSTR, pathbuffersize : u32, pathrequiredsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupPromptForDiskW(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCWSTR, diskname : ::windows_sys::core::PCWSTR, pathtosource : ::windows_sys::core::PCWSTR, filesought : ::windows_sys::core::PCWSTR, tagfile : ::windows_sys::core::PCWSTR, diskpromptstyle : u32, pathbuffer : ::windows_sys::core::PWSTR, pathbuffersize : u32, pathrequiredsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupPromptReboot(filequeue : *const ::core::ffi::c_void, owner : super::super::Foundation:: HWND, scanonly : super::super::Foundation:: BOOL) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryDrivesInDiskSpaceListA(diskspace : *const ::core::ffi::c_void, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryDrivesInDiskSpaceListW(diskspace : *const ::core::ffi::c_void, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryFileLogA(fileloghandle : *const ::core::ffi::c_void, logsectionname : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR, desiredinfo : SetupFileLogInfo, dataout : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryFileLogW(fileloghandle : *const ::core::ffi::c_void, logsectionname : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR, desiredinfo : SetupFileLogInfo, dataout : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryInfFileInformationA(infinformation : *const SP_INF_INFORMATION, infindex : u32, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryInfFileInformationW(infinformation : *const SP_INF_INFORMATION, infindex : u32, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupQueryInfOriginalFileInformationA(infinformation : *const SP_INF_INFORMATION, infindex : u32, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, originalfileinfo : *mut SP_ORIGINAL_FILE_INFO_A) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupQueryInfOriginalFileInformationW(infinformation : *const SP_INF_INFORMATION, infindex : u32, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, originalfileinfo : *mut SP_ORIGINAL_FILE_INFO_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryInfVersionInformationA(infinformation : *const SP_INF_INFORMATION, infindex : u32, key : ::windows_sys::core::PCSTR, returnbuffer : ::windows_sys::core::PSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueryInfVersionInformationW(infinformation : *const SP_INF_INFORMATION, infindex : u32, key : ::windows_sys::core::PCWSTR, returnbuffer : ::windows_sys::core::PWSTR, returnbuffersize : u32, requiredsize : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQuerySourceListA(flags : u32, list : *mut *mut ::windows_sys::core::PCSTR, count : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQuerySourceListW(flags : u32, list : *mut *mut ::windows_sys::core::PCWSTR, count : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQuerySpaceRequiredOnDriveA(diskspace : *const ::core::ffi::c_void, drivespec : ::windows_sys::core::PCSTR, spacerequired : *mut i64, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQuerySpaceRequiredOnDriveW(diskspace : *const ::core::ffi::c_void, drivespec : ::windows_sys::core::PCWSTR, spacerequired : *mut i64, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueCopyA(queuehandle : *const ::core::ffi::c_void, sourcerootpath : ::windows_sys::core::PCSTR, sourcepath : ::windows_sys::core::PCSTR, sourcefilename : ::windows_sys::core::PCSTR, sourcedescription : ::windows_sys::core::PCSTR, sourcetagfile : ::windows_sys::core::PCSTR, targetdirectory : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR, copystyle : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueCopyIndirectA(copyparams : *const SP_FILE_COPY_PARAMS_A) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueCopyIndirectW(copyparams : *const SP_FILE_COPY_PARAMS_W) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueCopySectionA(queuehandle : *const ::core::ffi::c_void, sourcerootpath : ::windows_sys::core::PCSTR, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR, copystyle : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueCopySectionW(queuehandle : *const ::core::ffi::c_void, sourcerootpath : ::windows_sys::core::PCWSTR, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR, copystyle : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueCopyW(queuehandle : *const ::core::ffi::c_void, sourcerootpath : ::windows_sys::core::PCWSTR, sourcepath : ::windows_sys::core::PCWSTR, sourcefilename : ::windows_sys::core::PCWSTR, sourcedescription : ::windows_sys::core::PCWSTR, sourcetagfile : ::windows_sys::core::PCWSTR, targetdirectory : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR, copystyle : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueDefaultCopyA(queuehandle : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, sourcerootpath : ::windows_sys::core::PCSTR, sourcefilename : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR, copystyle : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueDefaultCopyW(queuehandle : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, sourcerootpath : ::windows_sys::core::PCWSTR, sourcefilename : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR, copystyle : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueDeleteA(queuehandle : *const ::core::ffi::c_void, pathpart1 : ::windows_sys::core::PCSTR, pathpart2 : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueDeleteSectionA(queuehandle : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueDeleteSectionW(queuehandle : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueDeleteW(queuehandle : *const ::core::ffi::c_void, pathpart1 : ::windows_sys::core::PCWSTR, pathpart2 : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueRenameA(queuehandle : *const ::core::ffi::c_void, sourcepath : ::windows_sys::core::PCSTR, sourcefilename : ::windows_sys::core::PCSTR, targetpath : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueRenameSectionA(queuehandle : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueRenameSectionW(queuehandle : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, section : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupQueueRenameW(queuehandle : *const ::core::ffi::c_void, sourcepath : ::windows_sys::core::PCWSTR, sourcefilename : ::windows_sys::core::PCWSTR, targetpath : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveFileLogEntryA(fileloghandle : *const ::core::ffi::c_void, logsectionname : ::windows_sys::core::PCSTR, targetfilename : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveFileLogEntryW(fileloghandle : *const ::core::ffi::c_void, logsectionname : ::windows_sys::core::PCWSTR, targetfilename : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveFromDiskSpaceListA(diskspace : *const ::core::ffi::c_void, targetfilespec : ::windows_sys::core::PCSTR, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveFromDiskSpaceListW(diskspace : *const ::core::ffi::c_void, targetfilespec : ::windows_sys::core::PCWSTR, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveFromSourceListA(flags : u32, source : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveFromSourceListW(flags : u32, source : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveInstallSectionFromDiskSpaceListA(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, layoutinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveInstallSectionFromDiskSpaceListW(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, layoutinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveSectionFromDiskSpaceListA(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCSTR, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRemoveSectionFromDiskSpaceListW(diskspace : *const ::core::ffi::c_void, infhandle : *const ::core::ffi::c_void, listinfhandle : *const ::core::ffi::c_void, sectionname : ::windows_sys::core::PCWSTR, operation : SETUP_FILE_OPERATION, reserved1 : *const ::core::ffi::c_void, reserved2 : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRenameErrorA(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCSTR, sourcefile : ::windows_sys::core::PCSTR, targetfile : ::windows_sys::core::PCSTR, win32errorcode : u32, style : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupRenameErrorW(hwndparent : super::super::Foundation:: HWND, dialogtitle : ::windows_sys::core::PCWSTR, sourcefile : ::windows_sys::core::PCWSTR, targetfile : ::windows_sys::core::PCWSTR, win32errorcode : u32, style : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupScanFileQueueA(filequeue : *const ::core::ffi::c_void, flags : SETUPSCANFILEQUEUE_FLAGS, window : super::super::Foundation:: HWND, callbackroutine : PSP_FILE_CALLBACK_A, callbackcontext : *const ::core::ffi::c_void, result : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupScanFileQueueW(filequeue : *const ::core::ffi::c_void, flags : SETUPSCANFILEQUEUE_FLAGS, window : super::super::Foundation:: HWND, callbackroutine : PSP_FILE_CALLBACK_W, callbackcontext : *const ::core::ffi::c_void, result : *mut u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetDirectoryIdA(infhandle : *const ::core::ffi::c_void, id : u32, directory : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetDirectoryIdExA(infhandle : *const ::core::ffi::c_void, id : u32, directory : ::windows_sys::core::PCSTR, flags : u32, reserved1 : u32, reserved2 : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetDirectoryIdExW(infhandle : *const ::core::ffi::c_void, id : u32, directory : ::windows_sys::core::PCWSTR, flags : u32, reserved1 : u32, reserved2 : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetDirectoryIdW(infhandle : *const ::core::ffi::c_void, id : u32, directory : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupSetFileQueueAlternatePlatformA(queuehandle : *const ::core::ffi::c_void, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, alternatedefaultcatalogfile : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupSetFileQueueAlternatePlatformW(queuehandle : *const ::core::ffi::c_void, alternateplatforminfo : *const SP_ALTPLATFORM_INFO_V2, alternatedefaultcatalogfile : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetFileQueueFlags(filequeue : *const ::core::ffi::c_void, flagmask : u32, flags : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetNonInteractiveMode(noninteractiveflag : super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetPlatformPathOverrideA(r#override : ::windows_sys::core::PCSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetPlatformPathOverrideW(r#override : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetSourceListA(flags : u32, sourcelist : *const ::windows_sys::core::PCSTR, sourcecount : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupSetSourceListW(flags : u32, sourcelist : *const ::windows_sys::core::PCWSTR, sourcecount : u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "system" fn SetupSetThreadLogToken(logtoken : u64) -> ());
::windows_targets::link!("setupapi.dll" "system" fn SetupTermDefaultQueueCallback(context : *const ::core::ffi::c_void) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupTerminateFileLog(fileloghandle : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupUninstallNewlyCopiedInfs(filequeue : *const ::core::ffi::c_void, flags : u32, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupUninstallOEMInfA(inffilename : ::windows_sys::core::PCSTR, flags : u32, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetupUninstallOEMInfW(inffilename : ::windows_sys::core::PCWSTR, flags : u32, reserved : *const ::core::ffi::c_void) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupVerifyInfFileA(infname : ::windows_sys::core::PCSTR, altplatforminfo : *const SP_ALTPLATFORM_INFO_V2, infsignerinfo : *mut SP_INF_SIGNER_INFO_V2_A) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Diagnostics_Debug"))]
::windows_targets::link!("setupapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Diagnostics_Debug\"`"] fn SetupVerifyInfFileW(infname : ::windows_sys::core::PCWSTR, altplatforminfo : *const SP_ALTPLATFORM_INFO_V2, infsignerinfo : *mut SP_INF_SIGNER_INFO_V2_W) -> super::super::Foundation:: BOOL);
::windows_targets::link!("setupapi.dll" "cdecl" fn SetupWriteTextLog(logtoken : u64, category : u32, flags : u32, messagestr : ::windows_sys::core::PCSTR, ...) -> ());
::windows_targets::link!("setupapi.dll" "cdecl" fn SetupWriteTextLogError(logtoken : u64, category : u32, logflags : u32, error : u32, messagestr : ::windows_sys::core::PCSTR, ...) -> ());
::windows_targets::link!("setupapi.dll" "system" fn SetupWriteTextLogInfLine(logtoken : u64, flags : u32, infhandle : *const ::core::ffi::c_void, context : *const INFCONTEXT) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UpdateDriverForPlugAndPlayDevicesA(hwndparent : super::super::Foundation:: HWND, hardwareid : ::windows_sys::core::PCSTR, fullinfpath : ::windows_sys::core::PCSTR, installflags : UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS, brebootrequired : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("newdev.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn UpdateDriverForPlugAndPlayDevicesW(hwndparent : super::super::Foundation:: HWND, hardwareid : ::windows_sys::core::PCWSTR, fullinfpath : ::windows_sys::core::PCWSTR, installflags : UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS, brebootrequired : *mut super::super::Foundation:: BOOL) -> super::super::Foundation:: BOOL);
pub const ALLOC_LOG_CONF: CM_LOG_CONF = 2u32;
pub const BASIC_LOG_CONF: CM_LOG_CONF = 0u32;
pub const BOOT_LOG_CONF: CM_LOG_CONF = 3u32;
pub const CM_ADD_ID_BITS: u32 = 1u32;
pub const CM_ADD_ID_COMPATIBLE: u32 = 1u32;
pub const CM_ADD_ID_HARDWARE: u32 = 0u32;
pub const CM_ADD_RANGE_ADDIFCONFLICT: u32 = 0u32;
pub const CM_ADD_RANGE_BITS: u32 = 1u32;
pub const CM_ADD_RANGE_DONOTADDIFCONFLICT: u32 = 1u32;
pub const CM_CDFLAGS_DRIVER: CM_CDFLAGS = 1u32;
pub const CM_CDFLAGS_RESERVED: CM_CDFLAGS = 4u32;
pub const CM_CDFLAGS_ROOT_OWNED: CM_CDFLAGS = 2u32;
pub const CM_CDMASK_DESCRIPTION: CM_CDMASK = 8u32;
pub const CM_CDMASK_DEVINST: CM_CDMASK = 1u32;
pub const CM_CDMASK_FLAGS: CM_CDMASK = 4u32;
pub const CM_CDMASK_RESDES: CM_CDMASK = 2u32;
pub const CM_CDMASK_VALID: CM_CDMASK = 15u32;
pub const CM_CLASS_PROPERTY_BITS: u32 = 1u32;
pub const CM_CLASS_PROPERTY_INSTALLER: u32 = 0u32;
pub const CM_CLASS_PROPERTY_INTERFACE: u32 = 1u32;
pub const CM_CREATE_DEVINST_BITS: u32 = 15u32;
pub const CM_CREATE_DEVINST_DO_NOT_INSTALL: u32 = 8u32;
pub const CM_CREATE_DEVINST_GENERATE_ID: u32 = 4u32;
pub const CM_CREATE_DEVINST_NORMAL: u32 = 0u32;
pub const CM_CREATE_DEVINST_NO_WAIT_INSTALL: u32 = 1u32;
pub const CM_CREATE_DEVINST_PHANTOM: u32 = 2u32;
pub const CM_CREATE_DEVNODE_BITS: u32 = 15u32;
pub const CM_CREATE_DEVNODE_DO_NOT_INSTALL: u32 = 8u32;
pub const CM_CREATE_DEVNODE_GENERATE_ID: u32 = 4u32;
pub const CM_CREATE_DEVNODE_NORMAL: u32 = 0u32;
pub const CM_CREATE_DEVNODE_NO_WAIT_INSTALL: u32 = 1u32;
pub const CM_CREATE_DEVNODE_PHANTOM: u32 = 2u32;
pub const CM_CRP_CHARACTERISTICS: u32 = 28u32;
pub const CM_CRP_DEVTYPE: u32 = 26u32;
pub const CM_CRP_EXCLUSIVE: u32 = 27u32;
pub const CM_CRP_LOWERFILTERS: u32 = 19u32;
pub const CM_CRP_MAX: u32 = 37u32;
pub const CM_CRP_MIN: u32 = 1u32;
pub const CM_CRP_SECURITY: u32 = 24u32;
pub const CM_CRP_SECURITY_SDS: u32 = 25u32;
pub const CM_CRP_UPPERFILTERS: u32 = 18u32;
pub const CM_CUSTOMDEVPROP_BITS: u32 = 1u32;
pub const CM_CUSTOMDEVPROP_MERGE_MULTISZ: u32 = 1u32;
pub const CM_DELETE_CLASS_BITS: u32 = 3u32;
pub const CM_DELETE_CLASS_INTERFACE: u32 = 2u32;
pub const CM_DELETE_CLASS_ONLY: u32 = 0u32;
pub const CM_DELETE_CLASS_SUBKEYS: u32 = 1u32;
pub const CM_DETECT_BITS: u32 = 2147483655u32;
pub const CM_DETECT_CRASHED: u32 = 2u32;
pub const CM_DETECT_HWPROF_FIRST_BOOT: u32 = 4u32;
pub const CM_DETECT_NEW_PROFILE: u32 = 1u32;
pub const CM_DETECT_RUN: u32 = 2147483648u32;
pub const CM_DEVCAP_DOCKDEVICE: CM_DEVCAP = 8u32;
pub const CM_DEVCAP_EJECTSUPPORTED: CM_DEVCAP = 2u32;
pub const CM_DEVCAP_HARDWAREDISABLED: CM_DEVCAP = 256u32;
pub const CM_DEVCAP_LOCKSUPPORTED: CM_DEVCAP = 1u32;
pub const CM_DEVCAP_NONDYNAMIC: CM_DEVCAP = 512u32;
pub const CM_DEVCAP_RAWDEVICEOK: CM_DEVCAP = 64u32;
pub const CM_DEVCAP_REMOVABLE: CM_DEVCAP = 4u32;
pub const CM_DEVCAP_SECUREDEVICE: CM_DEVCAP = 1024u32;
pub const CM_DEVCAP_SILENTINSTALL: CM_DEVCAP = 32u32;
pub const CM_DEVCAP_SURPRISEREMOVALOK: CM_DEVCAP = 128u32;
pub const CM_DEVCAP_UNIQUEID: CM_DEVCAP = 16u32;
pub const CM_DEVICE_PANEL_EDGE_BOTTOM: u32 = 2u32;
pub const CM_DEVICE_PANEL_EDGE_LEFT: u32 = 3u32;
pub const CM_DEVICE_PANEL_EDGE_RIGHT: u32 = 4u32;
pub const CM_DEVICE_PANEL_EDGE_TOP: u32 = 1u32;
pub const CM_DEVICE_PANEL_EDGE_UNKNOWN: u32 = 0u32;
pub const CM_DEVICE_PANEL_JOINT_TYPE_HINGE: u32 = 2u32;
pub const CM_DEVICE_PANEL_JOINT_TYPE_PIVOT: u32 = 3u32;
pub const CM_DEVICE_PANEL_JOINT_TYPE_PLANAR: u32 = 1u32;
pub const CM_DEVICE_PANEL_JOINT_TYPE_SWIVEL: u32 = 4u32;
pub const CM_DEVICE_PANEL_JOINT_TYPE_UNKNOWN: u32 = 0u32;
pub const CM_DEVICE_PANEL_ORIENTATION_HORIZONTAL: u32 = 0u32;
pub const CM_DEVICE_PANEL_ORIENTATION_VERTICAL: u32 = 1u32;
pub const CM_DEVICE_PANEL_SHAPE_OVAL: u32 = 2u32;
pub const CM_DEVICE_PANEL_SHAPE_RECTANGLE: u32 = 1u32;
pub const CM_DEVICE_PANEL_SHAPE_UNKNOWN: u32 = 0u32;
pub const CM_DEVICE_PANEL_SIDE_BACK: u32 = 6u32;
pub const CM_DEVICE_PANEL_SIDE_BOTTOM: u32 = 2u32;
pub const CM_DEVICE_PANEL_SIDE_FRONT: u32 = 5u32;
pub const CM_DEVICE_PANEL_SIDE_LEFT: u32 = 3u32;
pub const CM_DEVICE_PANEL_SIDE_RIGHT: u32 = 4u32;
pub const CM_DEVICE_PANEL_SIDE_TOP: u32 = 1u32;
pub const CM_DEVICE_PANEL_SIDE_UNKNOWN: u32 = 0u32;
pub const CM_DISABLE_ABSOLUTE: u32 = 1u32;
pub const CM_DISABLE_BITS: u32 = 15u32;
pub const CM_DISABLE_HARDWARE: u32 = 2u32;
pub const CM_DISABLE_PERSIST: u32 = 8u32;
pub const CM_DISABLE_POLITE: u32 = 0u32;
pub const CM_DISABLE_UI_NOT_OK: u32 = 4u32;
pub const CM_DRP_ADDRESS: u32 = 29u32;
pub const CM_DRP_BASE_CONTAINERID: u32 = 37u32;
pub const CM_DRP_BUSNUMBER: u32 = 22u32;
pub const CM_DRP_BUSTYPEGUID: u32 = 20u32;
pub const CM_DRP_CAPABILITIES: u32 = 16u32;
pub const CM_DRP_CHARACTERISTICS: u32 = 28u32;
pub const CM_DRP_CLASS: u32 = 8u32;
pub const CM_DRP_CLASSGUID: u32 = 9u32;
pub const CM_DRP_COMPATIBLEIDS: u32 = 3u32;
pub const CM_DRP_CONFIGFLAGS: u32 = 11u32;
pub const CM_DRP_DEVICEDESC: u32 = 1u32;
pub const CM_DRP_DEVICE_POWER_DATA: u32 = 31u32;
pub const CM_DRP_DEVTYPE: u32 = 26u32;
pub const CM_DRP_DRIVER: u32 = 10u32;
pub const CM_DRP_ENUMERATOR_NAME: u32 = 23u32;
pub const CM_DRP_EXCLUSIVE: u32 = 27u32;
pub const CM_DRP_FRIENDLYNAME: u32 = 13u32;
pub const CM_DRP_HARDWAREID: u32 = 2u32;
pub const CM_DRP_INSTALL_STATE: u32 = 35u32;
pub const CM_DRP_LEGACYBUSTYPE: u32 = 21u32;
pub const CM_DRP_LOCATION_INFORMATION: u32 = 14u32;
pub const CM_DRP_LOCATION_PATHS: u32 = 36u32;
pub const CM_DRP_LOWERFILTERS: u32 = 19u32;
pub const CM_DRP_MAX: u32 = 37u32;
pub const CM_DRP_MFG: u32 = 12u32;
pub const CM_DRP_MIN: u32 = 1u32;
pub const CM_DRP_PHYSICAL_DEVICE_OBJECT_NAME: u32 = 15u32;
pub const CM_DRP_REMOVAL_POLICY: u32 = 32u32;
pub const CM_DRP_REMOVAL_POLICY_HW_DEFAULT: u32 = 33u32;
pub const CM_DRP_REMOVAL_POLICY_OVERRIDE: u32 = 34u32;
pub const CM_DRP_SECURITY: u32 = 24u32;
pub const CM_DRP_SECURITY_SDS: u32 = 25u32;
pub const CM_DRP_SERVICE: u32 = 5u32;
pub const CM_DRP_UI_NUMBER: u32 = 17u32;
pub const CM_DRP_UI_NUMBER_DESC_FORMAT: u32 = 30u32;
pub const CM_DRP_UNUSED0: u32 = 4u32;
pub const CM_DRP_UNUSED1: u32 = 6u32;
pub const CM_DRP_UNUSED2: u32 = 7u32;
pub const CM_DRP_UPPERFILTERS: u32 = 18u32;
pub const CM_ENUMERATE_CLASSES_BITS: CM_ENUMERATE_FLAGS = 1u32;
pub const CM_ENUMERATE_CLASSES_INSTALLER: CM_ENUMERATE_FLAGS = 0u32;
pub const CM_ENUMERATE_CLASSES_INTERFACE: CM_ENUMERATE_FLAGS = 1u32;
pub const CM_GETIDLIST_DONOTGENERATE: u32 = 268435520u32;
pub const CM_GETIDLIST_FILTER_BITS: u32 = 268435583u32;
pub const CM_GETIDLIST_FILTER_BUSRELATIONS: u32 = 32u32;
pub const CM_GETIDLIST_FILTER_CLASS: u32 = 512u32;
pub const CM_GETIDLIST_FILTER_EJECTRELATIONS: u32 = 4u32;
pub const CM_GETIDLIST_FILTER_ENUMERATOR: u32 = 1u32;
pub const CM_GETIDLIST_FILTER_NONE: u32 = 0u32;
pub const CM_GETIDLIST_FILTER_POWERRELATIONS: u32 = 16u32;
pub const CM_GETIDLIST_FILTER_PRESENT: u32 = 256u32;
pub const CM_GETIDLIST_FILTER_REMOVALRELATIONS: u32 = 8u32;
pub const CM_GETIDLIST_FILTER_SERVICE: u32 = 2u32;
pub const CM_GETIDLIST_FILTER_TRANSPORTRELATIONS: u32 = 128u32;
pub const CM_GET_DEVICE_INTERFACE_LIST_ALL_DEVICES: CM_GET_DEVICE_INTERFACE_LIST_FLAGS = 1u32;
pub const CM_GET_DEVICE_INTERFACE_LIST_BITS: CM_GET_DEVICE_INTERFACE_LIST_FLAGS = 1u32;
pub const CM_GET_DEVICE_INTERFACE_LIST_PRESENT: CM_GET_DEVICE_INTERFACE_LIST_FLAGS = 0u32;
pub const CM_GLOBAL_STATE_CAN_DO_UI: u32 = 1u32;
pub const CM_GLOBAL_STATE_DETECTION_PENDING: u32 = 16u32;
pub const CM_GLOBAL_STATE_ON_BIG_STACK: u32 = 2u32;
pub const CM_GLOBAL_STATE_REBOOT_REQUIRED: u32 = 32u32;
pub const CM_GLOBAL_STATE_SERVICES_AVAILABLE: u32 = 4u32;
pub const CM_GLOBAL_STATE_SHUTTING_DOWN: u32 = 8u32;
pub const CM_HWPI_DOCKED: u32 = 2u32;
pub const CM_HWPI_NOT_DOCKABLE: u32 = 0u32;
pub const CM_HWPI_UNDOCKED: u32 = 1u32;
pub const CM_INSTALL_STATE_FAILED_INSTALL: CM_INSTALL_STATE = 2u32;
pub const CM_INSTALL_STATE_FINISH_INSTALL: CM_INSTALL_STATE = 3u32;
pub const CM_INSTALL_STATE_INSTALLED: CM_INSTALL_STATE = 0u32;
pub const CM_INSTALL_STATE_NEEDS_REINSTALL: CM_INSTALL_STATE = 1u32;
pub const CM_LOCATE_DEVNODE_BITS: CM_LOCATE_DEVNODE_FLAGS = 7u32;
pub const CM_LOCATE_DEVNODE_CANCELREMOVE: CM_LOCATE_DEVNODE_FLAGS = 2u32;
pub const CM_LOCATE_DEVNODE_NORMAL: CM_LOCATE_DEVNODE_FLAGS = 0u32;
pub const CM_LOCATE_DEVNODE_NOVALIDATION: CM_LOCATE_DEVNODE_FLAGS = 4u32;
pub const CM_LOCATE_DEVNODE_PHANTOM: CM_LOCATE_DEVNODE_FLAGS = 1u32;
pub const CM_NAME_ATTRIBUTE_NAME_RETRIEVED_FROM_DEVICE: u32 = 1u32;
pub const CM_NAME_ATTRIBUTE_USER_ASSIGNED_NAME: u32 = 2u32;
pub const CM_NOTIFY_ACTION_DEVICECUSTOMEVENT: CM_NOTIFY_ACTION = 6i32;
pub const CM_NOTIFY_ACTION_DEVICEINSTANCEENUMERATED: CM_NOTIFY_ACTION = 7i32;
pub const CM_NOTIFY_ACTION_DEVICEINSTANCEREMOVED: CM_NOTIFY_ACTION = 9i32;
pub const CM_NOTIFY_ACTION_DEVICEINSTANCESTARTED: CM_NOTIFY_ACTION = 8i32;
pub const CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL: CM_NOTIFY_ACTION = 0i32;
pub const CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL: CM_NOTIFY_ACTION = 1i32;
pub const CM_NOTIFY_ACTION_DEVICEQUERYREMOVE: CM_NOTIFY_ACTION = 2i32;
pub const CM_NOTIFY_ACTION_DEVICEQUERYREMOVEFAILED: CM_NOTIFY_ACTION = 3i32;
pub const CM_NOTIFY_ACTION_DEVICEREMOVECOMPLETE: CM_NOTIFY_ACTION = 5i32;
pub const CM_NOTIFY_ACTION_DEVICEREMOVEPENDING: CM_NOTIFY_ACTION = 4i32;
pub const CM_NOTIFY_ACTION_MAX: CM_NOTIFY_ACTION = 10i32;
pub const CM_NOTIFY_FILTER_FLAG_ALL_DEVICE_INSTANCES: u32 = 2u32;
pub const CM_NOTIFY_FILTER_FLAG_ALL_INTERFACE_CLASSES: u32 = 1u32;
pub const CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE: CM_NOTIFY_FILTER_TYPE = 1i32;
pub const CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE: CM_NOTIFY_FILTER_TYPE = 2i32;
pub const CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE: CM_NOTIFY_FILTER_TYPE = 0i32;
pub const CM_NOTIFY_FILTER_TYPE_MAX: CM_NOTIFY_FILTER_TYPE = 3i32;
pub const CM_OPEN_CLASS_KEY_BITS: u32 = 1u32;
pub const CM_OPEN_CLASS_KEY_INSTALLER: u32 = 0u32;
pub const CM_OPEN_CLASS_KEY_INTERFACE: u32 = 1u32;
pub const CM_PROB_BIOS_TABLE: CM_PROB = 35u32;
pub const CM_PROB_BOOT_CONFIG_CONFLICT: CM_PROB = 6u32;
pub const CM_PROB_CANT_SHARE_IRQ: CM_PROB = 30u32;
pub const CM_PROB_CONSOLE_LOCKED: CM_PROB = 55u32;
pub const CM_PROB_DEVICE_NOT_THERE: CM_PROB = 24u32;
pub const CM_PROB_DEVICE_RESET: CM_PROB = 54u32;
pub const CM_PROB_DEVLOADER_FAILED: CM_PROB = 2u32;
pub const CM_PROB_DEVLOADER_NOT_FOUND: CM_PROB = 8u32;
pub const CM_PROB_DEVLOADER_NOT_READY: CM_PROB = 23u32;
pub const CM_PROB_DISABLED: CM_PROB = 22u32;
pub const CM_PROB_DISABLED_SERVICE: CM_PROB = 32u32;
pub const CM_PROB_DRIVER_BLOCKED: CM_PROB = 48u32;
pub const CM_PROB_DRIVER_FAILED_LOAD: CM_PROB = 39u32;
pub const CM_PROB_DRIVER_FAILED_PRIOR_UNLOAD: CM_PROB = 38u32;
pub const CM_PROB_DRIVER_SERVICE_KEY_INVALID: CM_PROB = 40u32;
pub const CM_PROB_DUPLICATE_DEVICE: CM_PROB = 42u32;
pub const CM_PROB_ENTRY_IS_WRONG_TYPE: CM_PROB = 4u32;
pub const CM_PROB_FAILED_ADD: CM_PROB = 31u32;
pub const CM_PROB_FAILED_DRIVER_ENTRY: CM_PROB = 37u32;
pub const CM_PROB_FAILED_FILTER: CM_PROB = 7u32;
pub const CM_PROB_FAILED_INSTALL: CM_PROB = 28u32;
pub const CM_PROB_FAILED_POST_START: CM_PROB = 43u32;
pub const CM_PROB_FAILED_START: CM_PROB = 10u32;
pub const CM_PROB_GUEST_ASSIGNMENT_FAILED: CM_PROB = 57u32;
pub const CM_PROB_HALTED: CM_PROB = 44u32;
pub const CM_PROB_HARDWARE_DISABLED: CM_PROB = 29u32;
pub const CM_PROB_HELD_FOR_EJECT: CM_PROB = 47u32;
pub const CM_PROB_INVALID_DATA: CM_PROB = 9u32;
pub const CM_PROB_IRQ_TRANSLATION_FAILED: CM_PROB = 36u32;
pub const CM_PROB_LACKED_ARBITRATOR: CM_PROB = 5u32;
pub const CM_PROB_LEGACY_SERVICE_NO_DEVICES: CM_PROB = 41u32;
pub const CM_PROB_LIAR: CM_PROB = 11u32;
pub const CM_PROB_MOVED: CM_PROB = 25u32;
pub const CM_PROB_NEED_CLASS_CONFIG: CM_PROB = 56u32;
pub const CM_PROB_NEED_RESTART: CM_PROB = 14u32;
pub const CM_PROB_NORMAL_CONFLICT: CM_PROB = 12u32;
pub const CM_PROB_NOT_CONFIGURED: CM_PROB = 1u32;
pub const CM_PROB_NOT_VERIFIED: CM_PROB = 13u32;
pub const CM_PROB_NO_SOFTCONFIG: CM_PROB = 34u32;
pub const CM_PROB_NO_VALID_LOG_CONF: CM_PROB = 27u32;
pub const CM_PROB_OUT_OF_MEMORY: CM_PROB = 3u32;
pub const CM_PROB_PARTIAL_LOG_CONF: CM_PROB = 16u32;
pub const CM_PROB_PHANTOM: CM_PROB = 45u32;
pub const CM_PROB_REENUMERATION: CM_PROB = 15u32;
pub const CM_PROB_REGISTRY: CM_PROB = 19u32;
pub const CM_PROB_REGISTRY_TOO_LARGE: CM_PROB = 49u32;
pub const CM_PROB_REINSTALL: CM_PROB = 18u32;
pub const CM_PROB_SETPROPERTIES_FAILED: CM_PROB = 50u32;
pub const CM_PROB_SYSTEM_SHUTDOWN: CM_PROB = 46u32;
pub const CM_PROB_TOO_EARLY: CM_PROB = 26u32;
pub const CM_PROB_TRANSLATION_FAILED: CM_PROB = 33u32;
pub const CM_PROB_UNKNOWN_RESOURCE: CM_PROB = 17u32;
pub const CM_PROB_UNSIGNED_DRIVER: CM_PROB = 52u32;
pub const CM_PROB_USED_BY_DEBUGGER: CM_PROB = 53u32;
pub const CM_PROB_VXDLDR: CM_PROB = 20u32;
pub const CM_PROB_WAITING_ON_DEPENDENCY: CM_PROB = 51u32;
pub const CM_PROB_WILL_BE_REMOVED: CM_PROB = 21u32;
pub const CM_QUERY_ARBITRATOR_BITS: u32 = 1u32;
pub const CM_QUERY_ARBITRATOR_RAW: u32 = 0u32;
pub const CM_QUERY_ARBITRATOR_TRANSLATED: u32 = 1u32;
pub const CM_QUERY_REMOVE_UI_NOT_OK: u32 = 1u32;
pub const CM_QUERY_REMOVE_UI_OK: u32 = 0u32;
pub const CM_REENUMERATE_ASYNCHRONOUS: CM_REENUMERATE_FLAGS = 4u32;
pub const CM_REENUMERATE_BITS: CM_REENUMERATE_FLAGS = 7u32;
pub const CM_REENUMERATE_NORMAL: CM_REENUMERATE_FLAGS = 0u32;
pub const CM_REENUMERATE_RETRY_INSTALLATION: CM_REENUMERATE_FLAGS = 2u32;
pub const CM_REENUMERATE_SYNCHRONOUS: CM_REENUMERATE_FLAGS = 1u32;
pub const CM_REGISTER_DEVICE_DRIVER_BITS: u32 = 3u32;
pub const CM_REGISTER_DEVICE_DRIVER_DISABLEABLE: u32 = 1u32;
pub const CM_REGISTER_DEVICE_DRIVER_REMOVABLE: u32 = 2u32;
pub const CM_REGISTER_DEVICE_DRIVER_STATIC: u32 = 0u32;
pub const CM_REGISTRY_BITS: u32 = 769u32;
pub const CM_REGISTRY_CONFIG: u32 = 512u32;
pub const CM_REGISTRY_HARDWARE: u32 = 0u32;
pub const CM_REGISTRY_SOFTWARE: u32 = 1u32;
pub const CM_REGISTRY_USER: u32 = 256u32;
pub const CM_REMOVAL_POLICY_EXPECT_NO_REMOVAL: CM_REMOVAL_POLICY = 1u32;
pub const CM_REMOVAL_POLICY_EXPECT_ORDERLY_REMOVAL: CM_REMOVAL_POLICY = 2u32;
pub const CM_REMOVAL_POLICY_EXPECT_SURPRISE_REMOVAL: CM_REMOVAL_POLICY = 3u32;
pub const CM_REMOVE_BITS: u32 = 7u32;
pub const CM_REMOVE_DISABLE: u32 = 4u32;
pub const CM_REMOVE_NO_RESTART: u32 = 2u32;
pub const CM_REMOVE_UI_NOT_OK: u32 = 1u32;
pub const CM_REMOVE_UI_OK: u32 = 0u32;
pub const CM_RESDES_WIDTH_32: u32 = 1u32;
pub const CM_RESDES_WIDTH_64: u32 = 2u32;
pub const CM_RESDES_WIDTH_BITS: u32 = 3u32;
pub const CM_RESDES_WIDTH_DEFAULT: u32 = 0u32;
pub const CM_SETUP_BITS: u32 = 15u32;
pub const CM_SETUP_DEVINST_CONFIG: u32 = 5u32;
pub const CM_SETUP_DEVINST_CONFIG_CLASS: u32 = 6u32;
pub const CM_SETUP_DEVINST_CONFIG_EXTENSIONS: u32 = 7u32;
pub const CM_SETUP_DEVINST_CONFIG_RESET: u32 = 8u32;
pub const CM_SETUP_DEVINST_READY: u32 = 0u32;
pub const CM_SETUP_DEVINST_RESET: u32 = 4u32;
pub const CM_SETUP_DEVNODE_CONFIG: u32 = 5u32;
pub const CM_SETUP_DEVNODE_CONFIG_CLASS: u32 = 6u32;
pub const CM_SETUP_DEVNODE_CONFIG_EXTENSIONS: u32 = 7u32;
pub const CM_SETUP_DEVNODE_CONFIG_RESET: u32 = 8u32;
pub const CM_SETUP_DEVNODE_READY: u32 = 0u32;
pub const CM_SETUP_DEVNODE_RESET: u32 = 4u32;
pub const CM_SETUP_DOWNLOAD: u32 = 1u32;
pub const CM_SETUP_PROP_CHANGE: u32 = 3u32;
pub const CM_SETUP_WRITE_LOG_CONFS: u32 = 2u32;
pub const CM_SET_DEVINST_PROBLEM_BITS: u32 = 1u32;
pub const CM_SET_DEVINST_PROBLEM_NORMAL: u32 = 0u32;
pub const CM_SET_DEVINST_PROBLEM_OVERRIDE: u32 = 1u32;
pub const CM_SET_DEVNODE_PROBLEM_BITS: u32 = 1u32;
pub const CM_SET_DEVNODE_PROBLEM_NORMAL: u32 = 0u32;
pub const CM_SET_DEVNODE_PROBLEM_OVERRIDE: u32 = 1u32;
pub const CM_SET_HW_PROF_FLAGS_BITS: u32 = 1u32;
pub const CM_SET_HW_PROF_FLAGS_UI_NOT_OK: u32 = 1u32;
pub const CONFIGMG_VERSION: u32 = 1024u32;
pub const COPYFLG_FORCE_FILE_IN_USE: u32 = 8u32;
pub const COPYFLG_IN_USE_TRY_RENAME: u32 = 16384u32;
pub const COPYFLG_NODECOMP: u32 = 2048u32;
pub const COPYFLG_NOPRUNE: u32 = 8192u32;
pub const COPYFLG_NOSKIP: u32 = 2u32;
pub const COPYFLG_NOVERSIONCHECK: u32 = 4u32;
pub const COPYFLG_NO_OVERWRITE: u32 = 16u32;
pub const COPYFLG_NO_VERSION_DIALOG: u32 = 32u32;
pub const COPYFLG_OVERWRITE_OLDER_ONLY: u32 = 64u32;
pub const COPYFLG_PROTECTED_WINDOWS_DRIVER_FILE: u32 = 256u32;
pub const COPYFLG_REPLACEONLY: u32 = 1024u32;
pub const COPYFLG_REPLACE_BOOT_FILE: u32 = 4096u32;
pub const COPYFLG_WARN_IF_SKIP: u32 = 1u32;
pub const CR_ACCESS_DENIED: CONFIGRET = 51u32;
pub const CR_ALREADY_SUCH_DEVINST: CONFIGRET = 16u32;
pub const CR_ALREADY_SUCH_DEVNODE: CONFIGRET = 16u32;
pub const CR_APM_VETOED: CONFIGRET = 24u32;
pub const CR_BUFFER_SMALL: CONFIGRET = 26u32;
pub const CR_CALL_NOT_IMPLEMENTED: CONFIGRET = 52u32;
pub const CR_CANT_SHARE_IRQ: CONFIGRET = 43u32;
pub const CR_CREATE_BLOCKED: CONFIGRET = 21u32;
pub const CR_DEFAULT: CONFIGRET = 1u32;
pub const CR_DEVICE_INTERFACE_ACTIVE: CONFIGRET = 54u32;
pub const CR_DEVICE_NOT_THERE: CONFIGRET = 36u32;
pub const CR_DEVINST_HAS_REQS: CONFIGRET = 10u32;
pub const CR_DEVLOADER_NOT_READY: CONFIGRET = 33u32;
pub const CR_DEVNODE_HAS_REQS: CONFIGRET = 10u32;
pub const CR_DLVXD_NOT_FOUND: CONFIGRET = 12u32;
pub const CR_FAILURE: CONFIGRET = 19u32;
pub const CR_FREE_RESOURCES: CONFIGRET = 41u32;
pub const CR_INVALID_API: CONFIGRET = 32u32;
pub const CR_INVALID_ARBITRATOR: CONFIGRET = 8u32;
pub const CR_INVALID_CONFLICT_LIST: CONFIGRET = 57u32;
pub const CR_INVALID_DATA: CONFIGRET = 31u32;
pub const CR_INVALID_DEVICE_ID: CONFIGRET = 30u32;
pub const CR_INVALID_DEVINST: CONFIGRET = 5u32;
pub const CR_INVALID_DEVNODE: CONFIGRET = 5u32;
pub const CR_INVALID_FLAG: CONFIGRET = 4u32;
pub const CR_INVALID_INDEX: CONFIGRET = 58u32;
pub const CR_INVALID_LOAD_TYPE: CONFIGRET = 25u32;
pub const CR_INVALID_LOG_CONF: CONFIGRET = 7u32;
pub const CR_INVALID_MACHINENAME: CONFIGRET = 47u32;
pub const CR_INVALID_NODELIST: CONFIGRET = 9u32;
pub const CR_INVALID_POINTER: CONFIGRET = 3u32;
pub const CR_INVALID_PRIORITY: CONFIGRET = 39u32;
pub const CR_INVALID_PROPERTY: CONFIGRET = 53u32;
pub const CR_INVALID_RANGE: CONFIGRET = 18u32;
pub const CR_INVALID_RANGE_LIST: CONFIGRET = 17u32;
pub const CR_INVALID_REFERENCE_STRING: CONFIGRET = 56u32;
pub const CR_INVALID_RESOURCEID: CONFIGRET = 11u32;
pub const CR_INVALID_RES_DES: CONFIGRET = 6u32;
pub const CR_INVALID_STRUCTURE_SIZE: CONFIGRET = 59u32;
pub const CR_MACHINE_UNAVAILABLE: CONFIGRET = 49u32;
pub const CR_NEED_RESTART: CONFIGRET = 34u32;
pub const CR_NOT_DISABLEABLE: CONFIGRET = 40u32;
pub const CR_NOT_SYSTEM_VM: CONFIGRET = 22u32;
pub const CR_NO_ARBITRATOR: CONFIGRET = 27u32;
pub const CR_NO_CM_SERVICES: CONFIGRET = 50u32;
pub const CR_NO_DEPENDENT: CONFIGRET = 44u32;
pub const CR_NO_MORE_HW_PROFILES: CONFIGRET = 35u32;
pub const CR_NO_MORE_LOG_CONF: CONFIGRET = 14u32;
pub const CR_NO_MORE_RES_DES: CONFIGRET = 15u32;
pub const CR_NO_REGISTRY_HANDLE: CONFIGRET = 28u32;
pub const CR_NO_SUCH_DEVICE_INTERFACE: CONFIGRET = 55u32;
pub const CR_NO_SUCH_DEVINST: CONFIGRET = 13u32;
pub const CR_NO_SUCH_DEVNODE: CONFIGRET = 13u32;
pub const CR_NO_SUCH_LOGICAL_DEV: CONFIGRET = 20u32;
pub const CR_NO_SUCH_REGISTRY_KEY: CONFIGRET = 46u32;
pub const CR_NO_SUCH_VALUE: CONFIGRET = 37u32;
pub const CR_OUT_OF_MEMORY: CONFIGRET = 2u32;
pub const CR_QUERY_VETOED: CONFIGRET = 42u32;
pub const CR_REGISTRY_ERROR: CONFIGRET = 29u32;
pub const CR_REMOTE_COMM_FAILURE: CONFIGRET = 48u32;
pub const CR_REMOVE_VETOED: CONFIGRET = 23u32;
pub const CR_SAME_RESOURCES: CONFIGRET = 45u32;
pub const CR_SUCCESS: CONFIGRET = 0u32;
pub const CR_WRONG_TYPE: CONFIGRET = 38u32;
pub const DELFLG_IN_USE: u32 = 1u32;
pub const DELFLG_IN_USE1: u32 = 65536u32;
pub const DIBCI_NODISPLAYCLASS: u32 = 2u32;
pub const DIBCI_NOINSTALLCLASS: u32 = 1u32;
pub const DICD_GENERATE_ID: u32 = 1u32;
pub const DICD_INHERIT_CLASSDRVS: u32 = 2u32;
pub const DICLASSPROP_INSTALLER: u32 = 1u32;
pub const DICLASSPROP_INTERFACE: u32 = 2u32;
pub const DICS_DISABLE: u32 = 2u32;
pub const DICS_ENABLE: u32 = 1u32;
pub const DICS_FLAG_CONFIGGENERAL: u32 = 4u32;
pub const DICS_FLAG_CONFIGSPECIFIC: u32 = 2u32;
pub const DICS_FLAG_GLOBAL: u32 = 1u32;
pub const DICS_PROPCHANGE: u32 = 3u32;
pub const DICS_START: u32 = 4u32;
pub const DICS_STOP: u32 = 5u32;
pub const DICUSTOMDEVPROP_MERGE_MULTISZ: u32 = 1u32;
pub const DIF_ADDPROPERTYPAGE_ADVANCED: u32 = 35u32;
pub const DIF_ADDPROPERTYPAGE_BASIC: u32 = 36u32;
pub const DIF_ADDREMOTEPROPERTYPAGE_ADVANCED: u32 = 40u32;
pub const DIF_ALLOW_INSTALL: u32 = 24u32;
pub const DIF_ASSIGNRESOURCES: u32 = 3u32;
pub const DIF_CALCDISKSPACE: u32 = 11u32;
pub const DIF_DESTROYPRIVATEDATA: u32 = 12u32;
pub const DIF_DESTROYWIZARDDATA: u32 = 17u32;
pub const DIF_DETECT: u32 = 15u32;
pub const DIF_DETECTCANCEL: u32 = 33u32;
pub const DIF_DETECTVERIFY: u32 = 20u32;
pub const DIF_ENABLECLASS: u32 = 19u32;
pub const DIF_FINISHINSTALL_ACTION: u32 = 42u32;
pub const DIF_FIRSTTIMESETUP: u32 = 6u32;
pub const DIF_FOUNDDEVICE: u32 = 7u32;
pub const DIF_INSTALLCLASSDRIVERS: u32 = 10u32;
pub const DIF_INSTALLDEVICE: u32 = 2u32;
pub const DIF_INSTALLDEVICEFILES: u32 = 21u32;
pub const DIF_INSTALLINTERFACES: u32 = 32u32;
pub const DIF_INSTALLWIZARD: u32 = 16u32;
pub const DIF_MOVEDEVICE: u32 = 14u32;
pub const DIF_NEWDEVICEWIZARD_FINISHINSTALL: u32 = 30u32;
pub const DIF_NEWDEVICEWIZARD_POSTANALYZE: u32 = 29u32;
pub const DIF_NEWDEVICEWIZARD_PREANALYZE: u32 = 28u32;
pub const DIF_NEWDEVICEWIZARD_PRESELECT: u32 = 26u32;
pub const DIF_NEWDEVICEWIZARD_SELECT: u32 = 27u32;
pub const DIF_POWERMESSAGEWAKE: u32 = 39u32;
pub const DIF_PROPERTIES: u32 = 4u32;
pub const DIF_PROPERTYCHANGE: u32 = 18u32;
pub const DIF_REGISTERDEVICE: u32 = 25u32;
pub const DIF_REGISTER_COINSTALLERS: u32 = 34u32;
pub const DIF_REMOVE: u32 = 5u32;
pub const DIF_RESERVED1: u32 = 37u32;
pub const DIF_RESERVED2: u32 = 48u32;
pub const DIF_SELECTBESTCOMPATDRV: u32 = 23u32;
pub const DIF_SELECTCLASSDRIVERS: u32 = 8u32;
pub const DIF_SELECTDEVICE: u32 = 1u32;
pub const DIF_TROUBLESHOOTER: u32 = 38u32;
pub const DIF_UNREMOVE: u32 = 22u32;
pub const DIF_UNUSED1: u32 = 31u32;
pub const DIF_UPDATEDRIVER_UI: u32 = 41u32;
pub const DIF_VALIDATECLASSDRIVERS: u32 = 9u32;
pub const DIF_VALIDATEDRIVER: u32 = 13u32;
pub const DIGCDP_FLAG_ADVANCED: u32 = 2u32;
pub const DIGCDP_FLAG_BASIC: u32 = 1u32;
pub const DIGCDP_FLAG_REMOTE_ADVANCED: u32 = 4u32;
pub const DIGCDP_FLAG_REMOTE_BASIC: u32 = 3u32;
pub const DIGCF_ALLCLASSES: u32 = 4u32;
pub const DIGCF_DEFAULT: u32 = 1u32;
pub const DIGCF_DEVICEINTERFACE: u32 = 16u32;
pub const DIGCF_INTERFACEDEVICE: u32 = 16u32;
pub const DIGCF_PRESENT: u32 = 2u32;
pub const DIGCF_PROFILE: u32 = 8u32;
pub const DIIDFLAG_BITS: DIINSTALLDEVICE_FLAGS = 15u32;
pub const DIIDFLAG_INSTALLCOPYINFDRIVERS: DIINSTALLDEVICE_FLAGS = 8u32;
pub const DIIDFLAG_INSTALLNULLDRIVER: DIINSTALLDEVICE_FLAGS = 4u32;
pub const DIIDFLAG_NOFINISHINSTALLUI: DIINSTALLDEVICE_FLAGS = 2u32;
pub const DIIDFLAG_SHOWSEARCHUI: DIINSTALLDEVICE_FLAGS = 1u32;
pub const DIIRFLAG_BITS: DIINSTALLDRIVER_FLAGS = 106u32;
pub const DIIRFLAG_FORCE_INF: DIINSTALLDRIVER_FLAGS = 2u32;
pub const DIIRFLAG_HOTPATCH: DIINSTALLDRIVER_FLAGS = 8u32;
pub const DIIRFLAG_HW_USING_THE_INF: DIINSTALLDRIVER_FLAGS = 4u32;
pub const DIIRFLAG_INF_ALREADY_COPIED: DIINSTALLDRIVER_FLAGS = 1u32;
pub const DIIRFLAG_INSTALL_AS_SET: DIINSTALLDRIVER_FLAGS = 64u32;
pub const DIIRFLAG_NOBACKUP: DIINSTALLDRIVER_FLAGS = 16u32;
pub const DIIRFLAG_PRE_CONFIGURE_INF: DIINSTALLDRIVER_FLAGS = 32u32;
pub const DIIRFLAG_SYSTEM_BITS: DIINSTALLDRIVER_FLAGS = 127u32;
pub const DIOCR_INSTALLER: u32 = 1u32;
pub const DIOCR_INTERFACE: u32 = 2u32;
pub const DIODI_NO_ADD: u32 = 1u32;
pub const DIOD_CANCEL_REMOVE: u32 = 4u32;
pub const DIOD_INHERIT_CLASSDRVS: u32 = 2u32;
pub const DIREG_BOTH: u32 = 4u32;
pub const DIREG_DEV: u32 = 1u32;
pub const DIREG_DRV: u32 = 2u32;
pub const DIRID_ABSOLUTE: i32 = -1i32;
pub const DIRID_ABSOLUTE_16BIT: u32 = 65535u32;
pub const DIRID_APPS: u32 = 24u32;
pub const DIRID_BOOT: u32 = 30u32;
pub const DIRID_COLOR: u32 = 23u32;
pub const DIRID_COMMON_APPDATA: u32 = 16419u32;
pub const DIRID_COMMON_DESKTOPDIRECTORY: u32 = 16409u32;
pub const DIRID_COMMON_DOCUMENTS: u32 = 16430u32;
pub const DIRID_COMMON_FAVORITES: u32 = 16415u32;
pub const DIRID_COMMON_PROGRAMS: u32 = 16407u32;
pub const DIRID_COMMON_STARTMENU: u32 = 16406u32;
pub const DIRID_COMMON_STARTUP: u32 = 16408u32;
pub const DIRID_COMMON_TEMPLATES: u32 = 16429u32;
pub const DIRID_DEFAULT: u32 = 11u32;
pub const DIRID_DRIVERS: u32 = 12u32;
pub const DIRID_DRIVER_STORE: u32 = 13u32;
pub const DIRID_FONTS: u32 = 20u32;
pub const DIRID_HELP: u32 = 18u32;
pub const DIRID_INF: u32 = 17u32;
pub const DIRID_IOSUBSYS: u32 = 12u32;
pub const DIRID_LOADER: u32 = 54u32;
pub const DIRID_NULL: u32 = 0u32;
pub const DIRID_PRINTPROCESSOR: u32 = 55u32;
pub const DIRID_PROGRAM_FILES: u32 = 16422u32;
pub const DIRID_PROGRAM_FILES_COMMON: u32 = 16427u32;
pub const DIRID_PROGRAM_FILES_COMMONX86: u32 = 16428u32;
pub const DIRID_PROGRAM_FILES_X86: u32 = 16426u32;
pub const DIRID_SHARED: u32 = 25u32;
pub const DIRID_SPOOL: u32 = 51u32;
pub const DIRID_SPOOLDRIVERS: u32 = 52u32;
pub const DIRID_SRCPATH: u32 = 1u32;
pub const DIRID_SYSTEM: u32 = 11u32;
pub const DIRID_SYSTEM16: u32 = 50u32;
pub const DIRID_SYSTEM_X86: u32 = 16425u32;
pub const DIRID_USER: u32 = 32768u32;
pub const DIRID_USERPROFILE: u32 = 53u32;
pub const DIRID_VIEWERS: u32 = 21u32;
pub const DIRID_WINDOWS: u32 = 10u32;
pub const DIURFLAG_NO_REMOVE_INF: DIUNINSTALLDRIVER_FLAGS = 1u32;
pub const DIURFLAG_RESERVED: DIUNINSTALLDRIVER_FLAGS = 2u32;
pub const DIURFLAG_VALID: DIUNINSTALLDRIVER_FLAGS = 3u32;
pub const DI_AUTOASSIGNRES: i32 = 64i32;
pub const DI_CLASSINSTALLPARAMS: i32 = 1048576i32;
pub const DI_COMPAT_FROM_CLASS: i32 = 524288i32;
pub const DI_DIDCLASS: i32 = 32i32;
pub const DI_DIDCOMPAT: i32 = 16i32;
pub const DI_DISABLED: i32 = 2048i32;
pub const DI_DONOTCALLCONFIGMG: i32 = 131072i32;
pub const DI_DRIVERPAGE_ADDED: i32 = 67108864i32;
pub const DI_ENUMSINGLEINF: i32 = 65536i32;
pub const DI_FLAGSEX_ALLOWEXCLUDEDDRVS: i32 = 2048i32;
pub const DI_FLAGSEX_ALTPLATFORM_DRVSEARCH: i32 = 268435456i32;
pub const DI_FLAGSEX_ALWAYSWRITEIDS: i32 = 512i32;
pub const DI_FLAGSEX_APPENDDRIVERLIST: i32 = 262144i32;
pub const DI_FLAGSEX_BACKUPONREPLACE: i32 = 1048576i32;
pub const DI_FLAGSEX_CI_FAILED: i32 = 4i32;
pub const DI_FLAGSEX_DEVICECHANGE: i32 = 256i32;
pub const DI_FLAGSEX_DIDCOMPATINFO: i32 = 32i32;
pub const DI_FLAGSEX_DIDINFOLIST: i32 = 16i32;
pub const DI_FLAGSEX_DRIVERLIST_FROM_URL: i32 = 2097152i32;
pub const DI_FLAGSEX_EXCLUDE_OLD_INET_DRIVERS: i32 = 8388608i32;
pub const DI_FLAGSEX_FILTERCLASSES: i32 = 64i32;
pub const DI_FLAGSEX_FILTERSIMILARDRIVERS: i32 = 33554432i32;
pub const DI_FLAGSEX_FINISHINSTALL_ACTION: i32 = 8i32;
pub const DI_FLAGSEX_INET_DRIVER: i32 = 131072i32;
pub const DI_FLAGSEX_INSTALLEDDRIVER: i32 = 67108864i32;
pub const DI_FLAGSEX_IN_SYSTEM_SETUP: i32 = 65536i32;
pub const DI_FLAGSEX_NOUIONQUERYREMOVE: i32 = 4096i32;
pub const DI_FLAGSEX_NO_CLASSLIST_NODE_MERGE: i32 = 134217728i32;
pub const DI_FLAGSEX_NO_DRVREG_MODIFY: i32 = 32768i32;
pub const DI_FLAGSEX_POWERPAGE_ADDED: i32 = 16777216i32;
pub const DI_FLAGSEX_PREINSTALLBACKUP: i32 = 524288i32;
pub const DI_FLAGSEX_PROPCHANGE_PENDING: i32 = 1024i32;
pub const DI_FLAGSEX_RECURSIVESEARCH: i32 = 1073741824i32;
pub const DI_FLAGSEX_RESERVED1: i32 = 4194304i32;
pub const DI_FLAGSEX_RESERVED2: i32 = 1i32;
pub const DI_FLAGSEX_RESERVED3: i32 = 2i32;
pub const DI_FLAGSEX_RESERVED4: i32 = 16384i32;
pub const DI_FLAGSEX_RESTART_DEVICE_ONLY: i32 = 536870912i32;
pub const DI_FLAGSEX_SEARCH_PUBLISHED_INFS: i32 = -2147483648i32;
pub const DI_FLAGSEX_SETFAILEDINSTALL: i32 = 128i32;
pub const DI_FLAGSEX_USECLASSFORCOMPAT: i32 = 8192i32;
pub const DI_FORCECOPY: i32 = 33554432i32;
pub const DI_GENERALPAGE_ADDED: i32 = 4096i32;
pub const DI_INF_IS_SORTED: i32 = 32768i32;
pub const DI_INSTALLDISABLED: i32 = 262144i32;
pub const DI_MULTMFGS: i32 = 1024i32;
pub const DI_NEEDREBOOT: i32 = 256i32;
pub const DI_NEEDRESTART: i32 = 128i32;
pub const DI_NOBROWSE: i32 = 512i32;
pub const DI_NODI_DEFAULTACTION: i32 = 2097152i32;
pub const DI_NOFILECOPY: i32 = 16777216i32;
pub const DI_NOSELECTICONS: i32 = 1073741824i32;
pub const DI_NOVCP: i32 = 8i32;
pub const DI_NOWRITE_IDS: i32 = -2147483648i32;
pub const DI_OVERRIDE_INFFLAGS: i32 = 268435456i32;
pub const DI_PROPERTIES_CHANGE: i32 = 16384i32;
pub const DI_PROPS_NOCHANGEUSAGE: i32 = 536870912i32;
pub const DI_QUIETINSTALL: i32 = 8388608i32;
pub const DI_REMOVEDEVICE_CONFIGSPECIFIC: u32 = 2u32;
pub const DI_REMOVEDEVICE_GLOBAL: u32 = 1u32;
pub const DI_RESOURCEPAGE_ADDED: i32 = 8192i32;
pub const DI_SHOWALL: i32 = 7i32;
pub const DI_SHOWCLASS: i32 = 4i32;
pub const DI_SHOWCOMPAT: i32 = 2i32;
pub const DI_SHOWOEM: i32 = 1i32;
pub const DI_UNREMOVEDEVICE_CONFIGSPECIFIC: u32 = 2u32;
pub const DI_USECI_SELECTSTRINGS: i32 = 134217728i32;
pub const DMI_BKCOLOR: u32 = 2u32;
pub const DMI_MASK: u32 = 1u32;
pub const DMI_USERECT: u32 = 4u32;
pub const DNF_ALWAYSEXCLUDEFROMLIST: u32 = 524288u32;
pub const DNF_AUTHENTICODE_SIGNED: u32 = 131072u32;
pub const DNF_BAD_DRIVER: u32 = 2048u32;
pub const DNF_BASIC_DRIVER: u32 = 65536u32;
pub const DNF_CLASS_DRIVER: u32 = 32u32;
pub const DNF_COMPATIBLE_DRIVER: u32 = 64u32;
pub const DNF_DUPDESC: u32 = 1u32;
pub const DNF_DUPDRIVERVER: u32 = 32768u32;
pub const DNF_DUPPROVIDER: u32 = 4096u32;
pub const DNF_EXCLUDEFROMLIST: u32 = 4u32;
pub const DNF_INBOX_DRIVER: u32 = 1048576u32;
pub const DNF_INET_DRIVER: u32 = 128u32;
pub const DNF_INF_IS_SIGNED: u32 = 8192u32;
pub const DNF_INSTALLEDDRIVER: u32 = 262144u32;
pub const DNF_LEGACYINF: u32 = 16u32;
pub const DNF_NODRIVER: u32 = 8u32;
pub const DNF_OEM_F6_INF: u32 = 16384u32;
pub const DNF_OLDDRIVER: u32 = 2u32;
pub const DNF_OLD_INET_DRIVER: u32 = 1024u32;
pub const DNF_REQUESTADDITIONALSOFTWARE: u32 = 2097152u32;
pub const DNF_UNUSED1: u32 = 256u32;
pub const DNF_UNUSED2: u32 = 512u32;
pub const DNF_UNUSED_22: u32 = 4194304u32;
pub const DNF_UNUSED_23: u32 = 8388608u32;
pub const DNF_UNUSED_24: u32 = 16777216u32;
pub const DNF_UNUSED_25: u32 = 33554432u32;
pub const DNF_UNUSED_26: u32 = 67108864u32;
pub const DNF_UNUSED_27: u32 = 134217728u32;
pub const DNF_UNUSED_28: u32 = 268435456u32;
pub const DNF_UNUSED_29: u32 = 536870912u32;
pub const DNF_UNUSED_30: u32 = 1073741824u32;
pub const DNF_UNUSED_31: u32 = 2147483648u32;
pub const DN_APM_DRIVER: CM_DEVNODE_STATUS_FLAGS = 268435456u32;
pub const DN_APM_ENUMERATOR: CM_DEVNODE_STATUS_FLAGS = 134217728u32;
pub const DN_ARM_WAKEUP: CM_DEVNODE_STATUS_FLAGS = 67108864u32;
pub const DN_BAD_PARTIAL: CM_DEVNODE_STATUS_FLAGS = 4194304u32;
pub const DN_BOOT_LOG_PROB: CM_DEVNODE_STATUS_FLAGS = 2147483648u32;
pub const DN_CHANGEABLE_FLAGS: CM_DEVNODE_STATUS_FLAGS = 1639670464u32;
pub const DN_CHILD_WITH_INVALID_ID: CM_DEVNODE_STATUS_FLAGS = 512u32;
pub const DN_DEVICE_DISCONNECTED: CM_DEVNODE_STATUS_FLAGS = 33554432u32;
pub const DN_DISABLEABLE: CM_DEVNODE_STATUS_FLAGS = 8192u32;
pub const DN_DRIVER_BLOCKED: CM_DEVNODE_STATUS_FLAGS = 64u32;
pub const DN_DRIVER_LOADED: CM_DEVNODE_STATUS_FLAGS = 2u32;
pub const DN_ENUM_LOADED: CM_DEVNODE_STATUS_FLAGS = 4u32;
pub const DN_FILTERED: CM_DEVNODE_STATUS_FLAGS = 2048u32;
pub const DN_HARDWARE_ENUM: CM_DEVNODE_STATUS_FLAGS = 128u32;
pub const DN_HAS_MARK: CM_DEVNODE_STATUS_FLAGS = 512u32;
pub const DN_HAS_PROBLEM: CM_DEVNODE_STATUS_FLAGS = 1024u32;
pub const DN_LEGACY_DRIVER: CM_DEVNODE_STATUS_FLAGS = 4096u32;
pub const DN_LIAR: CM_DEVNODE_STATUS_FLAGS = 256u32;
pub const DN_MANUAL: CM_DEVNODE_STATUS_FLAGS = 16u32;
pub const DN_MF_CHILD: CM_DEVNODE_STATUS_FLAGS = 131072u32;
pub const DN_MF_PARENT: CM_DEVNODE_STATUS_FLAGS = 65536u32;
pub const DN_MOVED: CM_DEVNODE_STATUS_FLAGS = 4096u32;
pub const DN_NEEDS_LOCKING: CM_DEVNODE_STATUS_FLAGS = 33554432u32;
pub const DN_NEED_RESTART: CM_DEVNODE_STATUS_FLAGS = 256u32;
pub const DN_NEED_TO_ENUM: CM_DEVNODE_STATUS_FLAGS = 32u32;
pub const DN_NOT_FIRST_TIME: CM_DEVNODE_STATUS_FLAGS = 64u32;
pub const DN_NOT_FIRST_TIMEE: CM_DEVNODE_STATUS_FLAGS = 524288u32;
pub const DN_NO_SHOW_IN_DM: CM_DEVNODE_STATUS_FLAGS = 1073741824u32;
pub const DN_NT_DRIVER: CM_DEVNODE_STATUS_FLAGS = 16777216u32;
pub const DN_NT_ENUMERATOR: CM_DEVNODE_STATUS_FLAGS = 8388608u32;
pub const DN_PRIVATE_PROBLEM: CM_DEVNODE_STATUS_FLAGS = 32768u32;
pub const DN_QUERY_REMOVE_ACTIVE: CM_DEVNODE_STATUS_FLAGS = 131072u32;
pub const DN_QUERY_REMOVE_PENDING: CM_DEVNODE_STATUS_FLAGS = 65536u32;
pub const DN_REBAL_CANDIDATE: CM_DEVNODE_STATUS_FLAGS = 2097152u32;
pub const DN_REMOVABLE: CM_DEVNODE_STATUS_FLAGS = 16384u32;
pub const DN_ROOT_ENUMERATED: CM_DEVNODE_STATUS_FLAGS = 1u32;
pub const DN_SILENT_INSTALL: CM_DEVNODE_STATUS_FLAGS = 536870912u32;
pub const DN_STARTED: CM_DEVNODE_STATUS_FLAGS = 8u32;
pub const DN_STOP_FREE_RES: CM_DEVNODE_STATUS_FLAGS = 1048576u32;
pub const DN_WILL_BE_REMOVED: CM_DEVNODE_STATUS_FLAGS = 262144u32;
pub const DPROMPT_BUFFERTOOSMALL: u32 = 3u32;
pub const DPROMPT_CANCEL: u32 = 1u32;
pub const DPROMPT_OUTOFMEMORY: u32 = 4u32;
pub const DPROMPT_SKIPFILE: u32 = 2u32;
pub const DPROMPT_SUCCESS: u32 = 0u32;
pub const DRIVER_COMPATID_RANK: u32 = 16383u32;
pub const DRIVER_HARDWAREID_MASK: u32 = 2147487743u32;
pub const DRIVER_HARDWAREID_RANK: u32 = 4095u32;
pub const DRIVER_UNTRUSTED_COMPATID_RANK: u32 = 49151u32;
pub const DRIVER_UNTRUSTED_HARDWAREID_RANK: u32 = 36863u32;
pub const DRIVER_UNTRUSTED_RANK: u32 = 2147483648u32;
pub const DRIVER_W9X_SUSPECT_COMPATID_RANK: u32 = 65535u32;
pub const DRIVER_W9X_SUSPECT_HARDWAREID_RANK: u32 = 53247u32;
pub const DRIVER_W9X_SUSPECT_RANK: u32 = 3221225472u32;
pub const DWORD_MAX: u32 = 4294967295u32;
pub const DYNAWIZ_FLAG_ANALYZE_HANDLECONFLICT: u32 = 8u32;
pub const DYNAWIZ_FLAG_INSTALLDET_NEXT: u32 = 2u32;
pub const DYNAWIZ_FLAG_INSTALLDET_PREV: u32 = 4u32;
pub const DYNAWIZ_FLAG_PAGESADDED: u32 = 1u32;
pub const ENABLECLASS_FAILURE: u32 = 2u32;
pub const ENABLECLASS_QUERY: u32 = 0u32;
pub const ENABLECLASS_SUCCESS: u32 = 1u32;
pub const FILEOP_ABORT: u32 = 0u32;
pub const FILEOP_BACKUP: u32 = 3u32;
pub const FILEOP_COPY: SETUP_FILE_OPERATION = 0u32;
pub const FILEOP_DELETE: SETUP_FILE_OPERATION = 2u32;
pub const FILEOP_DOIT: u32 = 1u32;
pub const FILEOP_NEWPATH: u32 = 4u32;
pub const FILEOP_RENAME: u32 = 1u32;
pub const FILEOP_RETRY: u32 = 1u32;
pub const FILEOP_SKIP: u32 = 2u32;
pub const FILE_COMPRESSION_MSZIP: u32 = 2u32;
pub const FILE_COMPRESSION_NONE: u32 = 0u32;
pub const FILE_COMPRESSION_NTCAB: u32 = 3u32;
pub const FILE_COMPRESSION_WINLZA: u32 = 1u32;
pub const FILTERED_LOG_CONF: CM_LOG_CONF = 1u32;
pub const FLG_ADDPROPERTY_AND: u32 = 16u32;
pub const FLG_ADDPROPERTY_APPEND: u32 = 4u32;
pub const FLG_ADDPROPERTY_NOCLOBBER: u32 = 1u32;
pub const FLG_ADDPROPERTY_OR: u32 = 8u32;
pub const FLG_ADDPROPERTY_OVERWRITEONLY: u32 = 2u32;
pub const FLG_ADDREG_32BITKEY: u32 = 16384u32;
pub const FLG_ADDREG_64BITKEY: u32 = 4096u32;
pub const FLG_ADDREG_APPEND: u32 = 8u32;
pub const FLG_ADDREG_BINVALUETYPE: u32 = 1u32;
pub const FLG_ADDREG_DELREG_BIT: u32 = 32768u32;
pub const FLG_ADDREG_DELVAL: u32 = 4u32;
pub const FLG_ADDREG_KEYONLY: u32 = 16u32;
pub const FLG_ADDREG_KEYONLY_COMMON: u32 = 8192u32;
pub const FLG_ADDREG_NOCLOBBER: u32 = 2u32;
pub const FLG_ADDREG_OVERWRITEONLY: u32 = 32u32;
pub const FLG_ADDREG_TYPE_EXPAND_SZ: u32 = 131072u32;
pub const FLG_ADDREG_TYPE_MULTI_SZ: u32 = 65536u32;
pub const FLG_ADDREG_TYPE_SZ: u32 = 0u32;
pub const FLG_BITREG_32BITKEY: u32 = 16384u32;
pub const FLG_BITREG_64BITKEY: u32 = 4096u32;
pub const FLG_BITREG_CLEARBITS: u32 = 0u32;
pub const FLG_BITREG_SETBITS: u32 = 1u32;
pub const FLG_DELPROPERTY_MULTI_SZ_DELSTRING: u32 = 1u32;
pub const FLG_DELREG_32BITKEY: u32 = 16384u32;
pub const FLG_DELREG_64BITKEY: u32 = 4096u32;
pub const FLG_DELREG_KEYONLY_COMMON: u32 = 8192u32;
pub const FLG_DELREG_OPERATION_MASK: u32 = 254u32;
pub const FLG_DELREG_TYPE_EXPAND_SZ: u32 = 131072u32;
pub const FLG_DELREG_TYPE_MULTI_SZ: u32 = 65536u32;
pub const FLG_DELREG_TYPE_SZ: u32 = 0u32;
pub const FLG_DELREG_VALUE: u32 = 0u32;
pub const FLG_INI2REG_32BITKEY: u32 = 16384u32;
pub const FLG_INI2REG_64BITKEY: u32 = 4096u32;
pub const FLG_PROFITEM_CSIDL: u32 = 8u32;
pub const FLG_PROFITEM_CURRENTUSER: u32 = 1u32;
pub const FLG_PROFITEM_DELETE: u32 = 2u32;
pub const FLG_PROFITEM_GROUP: u32 = 4u32;
pub const FLG_REGSVR_DLLINSTALL: u32 = 2u32;
pub const FLG_REGSVR_DLLREGISTER: u32 = 1u32;
pub const FORCED_LOG_CONF: CM_LOG_CONF = 4u32;
pub const GUID_ACPI_CMOS_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3a8d0384_6505_40ca_bc39_56c15f8c5fed);
pub const GUID_ACPI_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb091a08a_ba97_11d0_bd14_00aa00b7b32a);
pub const GUID_ACPI_INTERFACE_STANDARD2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe8695f63_1831_4870_a8cf_9c2f03f9dcb5);
pub const GUID_ACPI_PORT_RANGES_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf14f609b_cbbd_4957_a674_bc00213f1c97);
pub const GUID_ACPI_REGS_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x06141966_7245_6369_462e_4e656c736f6e);
pub const GUID_AGP_TARGET_BUS_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb15cfce8_06d1_4d37_9d4c_bedde0c2a6ff);
pub const GUID_ARBITER_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe644f185_8c0e_11d0_becf_08002be2092f);
pub const GUID_BUS_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x496b8280_6f25_11d0_beaf_08002be2092f);
pub const GUID_BUS_RESOURCE_UPDATE_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x27d0102d_bfb2_4164_81dd_dbb82f968b48);
pub const GUID_BUS_TYPE_1394: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf74e73eb_9ac5_45eb_be4d_772cc71ddfb3);
pub const GUID_BUS_TYPE_ACPI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd7b46895_001a_4942_891f_a7d46610a843);
pub const GUID_BUS_TYPE_AVC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc06ff265_ae09_48f0_812c_16753d7cba83);
pub const GUID_BUS_TYPE_DOT4PRT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x441ee001_4342_11d5_a184_00c04f60524d);
pub const GUID_BUS_TYPE_EISA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xddc35509_f3fc_11d0_a537_0000f8753ed1);
pub const GUID_BUS_TYPE_HID: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeeaf37d0_1963_47c4_aa48_72476db7cf49);
pub const GUID_BUS_TYPE_INTERNAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1530ea73_086b_11d1_a09f_00c04fc340b1);
pub const GUID_BUS_TYPE_IRDA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7ae17dc1_c944_44d6_881f_4c2e61053bc1);
pub const GUID_BUS_TYPE_ISAPNP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe676f854_d87d_11d0_92b2_00a0c9055fc5);
pub const GUID_BUS_TYPE_LPTENUM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc4ca1000_2ddc_11d5_a17a_00c04f60524d);
pub const GUID_BUS_TYPE_MCA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1c75997a_dc33_11d0_92b2_00a0c9055fc5);
pub const GUID_BUS_TYPE_PCI: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc8ebdfb0_b510_11d0_80e5_00a0c92542e3);
pub const GUID_BUS_TYPE_PCMCIA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x09343630_af9f_11d0_92e9_0000f81e1b30);
pub const GUID_BUS_TYPE_SCM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x375a5912_804c_45aa_bdc2_fdd25a1d9512);
pub const GUID_BUS_TYPE_SD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe700cc04_4036_4e89_9579_89ebf45f00cd);
pub const GUID_BUS_TYPE_SERENUM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x77114a87_8944_11d1_bd90_00a0c906be2d);
pub const GUID_BUS_TYPE_SW_DEVICE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x06d10322_7de0_4cef_8e25_197d0e7442e2);
pub const GUID_BUS_TYPE_USB: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9d7debbc_c85d_11d1_9eb4_006008c3a19a);
pub const GUID_BUS_TYPE_USBPRINT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x441ee000_4342_11d5_a184_00c04f60524d);
pub const GUID_D3COLD_AUX_POWER_AND_TIMING_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0044d8aa_f664_4588_9ffc_2afeaf5950b9);
pub const GUID_D3COLD_SUPPORT_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb38290e5_3cd0_4f9d_9937_f5fe2b44d47a);
pub const GUID_DEVCLASS_1394: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bdd1fc1_810f_11d0_bec7_08002be2092f);
pub const GUID_DEVCLASS_1394DEBUG: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x66f250d6_7801_4a64_b139_eea80a450b24);
pub const GUID_DEVCLASS_61883: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7ebefbc0_3200_11d2_b4c2_00a0c9697d07);
pub const GUID_DEVCLASS_ADAPTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e964_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_APMSUPPORT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd45b1c18_c8fa_11d1_9f77_0000f805f530);
pub const GUID_DEVCLASS_AVC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc06ff265_ae09_48f0_812c_16753d7cba83);
pub const GUID_DEVCLASS_BATTERY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x72631e54_78a4_11d0_bcf7_00aa00b7b32a);
pub const GUID_DEVCLASS_BIOMETRIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53d29ef7_377c_4d14_864b_eb3a85769359);
pub const GUID_DEVCLASS_BLUETOOTH: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe0cbf06c_cd8b_4647_bb8a_263b43f0f974);
pub const GUID_DEVCLASS_CAMERA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xca3e7ab9_b4c3_4ae6_8251_579ef933890f);
pub const GUID_DEVCLASS_CDROM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e965_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_COMPUTEACCELERATOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf01a9d53_3ff6_48d2_9f97_c8a7004be10c);
pub const GUID_DEVCLASS_COMPUTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e966_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_DECODER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bdd1fc2_810f_11d0_bec7_08002be2092f);
pub const GUID_DEVCLASS_DISKDRIVE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e967_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_DISPLAY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e968_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_DOT4: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x48721b56_6795_11d2_b1a8_0080c72e74a2);
pub const GUID_DEVCLASS_DOT4PRINT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x49ce6ac8_6f86_11d2_b1e5_0080c72e74a2);
pub const GUID_DEVCLASS_EHSTORAGESILO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9da2b80f_f89f_4a49_a5c2_511b085b9e8a);
pub const GUID_DEVCLASS_ENUM1394: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc459df55_db08_11d1_b009_00a0c9081ff6);
pub const GUID_DEVCLASS_EXTENSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe2f84ce7_8efa_411c_aa69_97454ca4cb57);
pub const GUID_DEVCLASS_FDC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e969_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_FIRMWARE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e7dd72_6468_4e36_b6f1_6488f42c1b52);
pub const GUID_DEVCLASS_FLOPPYDISK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e980_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_FSFILTER_ACTIVITYMONITOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb86dff51_a31e_4bac_b3cf_e8cfe75c9fc2);
pub const GUID_DEVCLASS_FSFILTER_ANTIVIRUS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb1d1a169_c54f_4379_81db_bee7d88d7454);
pub const GUID_DEVCLASS_FSFILTER_BOTTOM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x37765ea0_5958_4fc9_b04b_2fdfef97e59e);
pub const GUID_DEVCLASS_FSFILTER_CFSMETADATASERVER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcdcf0939_b75b_4630_bf76_80f7ba655884);
pub const GUID_DEVCLASS_FSFILTER_COMPRESSION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf3586baf_b5aa_49b5_8d6c_0569284c639f);
pub const GUID_DEVCLASS_FSFILTER_CONTENTSCREENER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3e3f0674_c83c_4558_bb26_9820e1eba5c5);
pub const GUID_DEVCLASS_FSFILTER_CONTINUOUSBACKUP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x71aa14f8_6fad_4622_ad77_92bb9d7e6947);
pub const GUID_DEVCLASS_FSFILTER_COPYPROTECTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x89786ff1_9c12_402f_9c9e_17753c7f4375);
pub const GUID_DEVCLASS_FSFILTER_ENCRYPTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa0a701c0_a511_42ff_aa6c_06dc0395576f);
pub const GUID_DEVCLASS_FSFILTER_HSM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd546500a_2aeb_45f6_9482_f4b1799c3177);
pub const GUID_DEVCLASS_FSFILTER_INFRASTRUCTURE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe55fa6f9_128c_4d04_abab_630c74b1453a);
pub const GUID_DEVCLASS_FSFILTER_OPENFILEBACKUP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf8ecafa6_66d1_41a5_899b_66585d7216b7);
pub const GUID_DEVCLASS_FSFILTER_PHYSICALQUOTAMANAGEMENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6a0a8e78_bba6_4fc4_a709_1e33cd09d67e);
pub const GUID_DEVCLASS_FSFILTER_QUOTAMANAGEMENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8503c911_a6c7_4919_8f79_5028f5866b0c);
pub const GUID_DEVCLASS_FSFILTER_REPLICATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x48d3ebc4_4cf8_48ff_b869_9c68ad42eb9f);
pub const GUID_DEVCLASS_FSFILTER_SECURITYENHANCER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd02bc3da_0c8e_4945_9bd5_f1883c226c8c);
pub const GUID_DEVCLASS_FSFILTER_SYSTEM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5d1b9aaa_01e2_46af_849f_272b3f324c46);
pub const GUID_DEVCLASS_FSFILTER_SYSTEMRECOVERY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2db15374_706e_4131_a0c7_d7c78eb0289a);
pub const GUID_DEVCLASS_FSFILTER_TOP: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb369baf4_5568_4e82_a87e_a93eb16bca87);
pub const GUID_DEVCLASS_FSFILTER_UNDELETE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfe8f1572_c67a_48c0_bbac_0b5c6d66cafb);
pub const GUID_DEVCLASS_FSFILTER_VIRTUALIZATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf75a86c0_10d8_4c3a_b233_ed60e4cdfaac);
pub const GUID_DEVCLASS_GENERIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xff494df1_c4ed_4fac_9b3f_3786f6e91e7e);
pub const GUID_DEVCLASS_GPS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bdd1fc3_810f_11d0_bec7_08002be2092f);
pub const GUID_DEVCLASS_HDC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e96a_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_HIDCLASS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x745a17a0_74d3_11d0_b6fe_00a0c90f57da);
pub const GUID_DEVCLASS_HOLOGRAPHIC: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd612553d_06b1_49ca_8938_e39ef80eb16f);
pub const GUID_DEVCLASS_IMAGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bdd1fc6_810f_11d0_bec7_08002be2092f);
pub const GUID_DEVCLASS_INFINIBAND: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x30ef7132_d858_4a0c_ac24_b9028a5cca3f);
pub const GUID_DEVCLASS_INFRARED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6bdd1fc5_810f_11d0_bec7_08002be2092f);
pub const GUID_DEVCLASS_KEYBOARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e96b_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_LEGACYDRIVER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8ecc055d_047f_11d1_a537_0000f8753ed1);
pub const GUID_DEVCLASS_MEDIA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e96c_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_MEDIUM_CHANGER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xce5939ae_ebde_11d0_b181_0000f8753ec4);
pub const GUID_DEVCLASS_MEMORY: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5099944a_f6b9_4057_a056_8c550228544c);
pub const GUID_DEVCLASS_MODEM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e96d_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_MONITOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e96e_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_MOUSE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e96f_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_MTD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e970_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_MULTIFUNCTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e971_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_MULTIPORTSERIAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50906cb8_ba12_11d1_bf5d_0000f805f530);
pub const GUID_DEVCLASS_NET: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e972_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_NETCLIENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e973_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_NETDRIVER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x87ef9ad1_8f70_49ee_b215_ab1fcadcbe3c);
pub const GUID_DEVCLASS_NETSERVICE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e974_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_NETTRANS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e975_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_NETUIO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x78912bc1_cb8e_4b28_a329_f322ebadbe0f);
pub const GUID_DEVCLASS_NODRIVER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e976_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_PCMCIA: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e977_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_PNPPRINTERS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4658ee7e_f050_11d1_b6bd_00c04fa372a7);
pub const GUID_DEVCLASS_PORTS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e978_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_PRIMITIVE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x242681d1_eed3_41d2_a1ef_1468fc843106);
pub const GUID_DEVCLASS_PRINTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e979_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_PRINTERUPGRADE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e97a_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_PRINTQUEUE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1ed2bbf9_11f0_4084_b21f_ad83a8e6dcdc);
pub const GUID_DEVCLASS_PROCESSOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50127dc3_0f36_415e_a6cc_4cb3be910b65);
pub const GUID_DEVCLASS_SBP2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd48179be_ec20_11d1_b6b8_00c04fa372a7);
pub const GUID_DEVCLASS_SCMDISK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53966cb1_4d46_4166_bf23_c522403cd495);
pub const GUID_DEVCLASS_SCMVOLUME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53ccb149_e543_4c84_b6e0_bce4f6b7e806);
pub const GUID_DEVCLASS_SCSIADAPTER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e97b_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_SECURITYACCELERATOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x268c95a1_edfe_11d3_95c3_0010dc4050a5);
pub const GUID_DEVCLASS_SENSOR: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5175d334_c371_4806_b3ba_71fd53c9258d);
pub const GUID_DEVCLASS_SIDESHOW: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x997b5d8d_c442_4f2e_baf3_9c8e671e9e21);
pub const GUID_DEVCLASS_SMARTCARDREADER: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50dd5230_ba8a_11d1_bf5d_0000f805f530);
pub const GUID_DEVCLASS_SMRDISK: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53487c23_680f_4585_acc3_1f10d6777e82);
pub const GUID_DEVCLASS_SMRVOLUME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53b3cf03_8f5a_4788_91b6_d19ed9fcccbf);
pub const GUID_DEVCLASS_SOFTWARECOMPONENT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x5c4c3332_344d_483c_8739_259e934c9cc8);
pub const GUID_DEVCLASS_SOUND: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e97c_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_SYSTEM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e97d_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_TAPEDRIVE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6d807884_7d21_11cf_801c_08002be10318);
pub const GUID_DEVCLASS_UCM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe6f1aa1c_7f3b_4473_b2e8_c97d8ac71d53);
pub const GUID_DEVCLASS_UNKNOWN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d36e97e_e325_11ce_bfc1_08002be10318);
pub const GUID_DEVCLASS_USB: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x36fc9e60_c465_11cf_8056_444553540000);
pub const GUID_DEVCLASS_VOLUME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x71a27cdd_812a_11d0_bec7_08002be2092f);
pub const GUID_DEVCLASS_VOLUMESNAPSHOT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x533c5b84_ec70_11d2_9505_00c04f79deaf);
pub const GUID_DEVCLASS_WCEUSBS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x25dbce51_6c8f_4a72_8a6d_b54c2b4fc835);
pub const GUID_DEVCLASS_WPD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xeec5ad98_8080_425f_922a_dabf3de3f69a);
pub const GUID_DEVICE_INTERFACE_ARRIVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4004_46f0_11d0_b08f_00609713053f);
pub const GUID_DEVICE_INTERFACE_REMOVAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4005_46f0_11d0_b08f_00609713053f);
pub const GUID_DEVICE_RESET_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x649fdf26_3bc0_4813_ad24_7e0c1eda3fa3);
pub const GUID_DMA_CACHE_COHERENCY_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb520f7fa_8a5a_4e40_a3f6_6be1e162d935);
pub const GUID_HWPROFILE_CHANGE_CANCELLED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4002_46f0_11d0_b08f_00609713053f);
pub const GUID_HWPROFILE_CHANGE_COMPLETE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4003_46f0_11d0_b08f_00609713053f);
pub const GUID_HWPROFILE_QUERY_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4001_46f0_11d0_b08f_00609713053f);
pub const GUID_INT_ROUTE_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x70941bf4_0073_11d1_a09e_00c04fc340b1);
pub const GUID_IOMMU_BUS_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1efee0b2_d278_4ae4_bddc_1b34dd648043);
pub const GUID_KERNEL_SOFT_RESTART_CANCEL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x31d737e7_8c0b_468a_956e_9f433ec358fb);
pub const GUID_KERNEL_SOFT_RESTART_FINALIZE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x20e91abd_350a_4d4f_8577_99c81507473a);
pub const GUID_KERNEL_SOFT_RESTART_PREPARE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xde373def_a85c_4f76_8cbf_f96bea8bd10f);
pub const GUID_LEGACY_DEVICE_DETECTION_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x50feb0de_596a_11d2_a5b8_0000f81a4619);
pub const GUID_MF_ENUMERATION_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaeb895f0_5586_11d1_8d84_00a0c906b244);
pub const GUID_MSIX_TABLE_CONFIG_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x1a6a460b_194f_455d_b34b_b84c5b05712b);
pub const GUID_NPEM_CONTROL_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4d95573d_b774_488a_b120_4f284a9eff51);
pub const GUID_PARTITION_UNIT_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x52363f5b_d891_429b_8195_aec5fef6853c);
pub const GUID_PCC_INTERFACE_INTERNAL: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7cce62ce_c189_4814_a6a7_12112089e938);
pub const GUID_PCC_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3ee8ba63_0f59_4a24_8a45_35808bdd1249);
pub const GUID_PCI_ATS_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x010a7fe8_96f5_4943_bedf_95e651b93412);
pub const GUID_PCI_BUS_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x496b8281_6f25_11d0_beaf_08002be2092f);
pub const GUID_PCI_BUS_INTERFACE_STANDARD2: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xde94e966_fdff_4c9c_9998_6747b150e74c);
pub const GUID_PCI_DEVICE_PRESENT_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd1b82c26_bf49_45ef_b216_71cbd7889b57);
pub const GUID_PCI_EXPRESS_LINK_QUIESCENT_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x146cd41c_dae3_4437_8aff_2af3f038099b);
pub const GUID_PCI_EXPRESS_ROOT_PORT_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x83a7734a_84c7_4161_9a98_6000ed0c4a33);
pub const GUID_PCI_FPGA_CONTROL_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2df3f7a8_b9b3_4063_9215_b5d14a0b266e);
pub const GUID_PCI_PTM_CONTROL_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x348a5ebb_ba24_44b7_9916_285687735117);
pub const GUID_PCI_SECURITY_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6e7f1451_199e_4acc_ba2d_762b4edf4674);
pub const GUID_PCI_VIRTUALIZATION_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x64897b47_3a4a_4d75_bc74_89dd6c078293);
pub const GUID_PCMCIA_BUS_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x76173af0_c504_11d1_947f_00c04fb960ee);
pub const GUID_PNP_CUSTOM_NOTIFICATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaca73f8e_8d23_11d1_ac7d_0000f87571d0);
pub const GUID_PNP_EXTENDED_ADDRESS_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb8e992ec_a797_4dc4_8846_84d041707446);
pub const GUID_PNP_LOCATION_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x70211b0e_0afb_47db_afc1_410bf842497a);
pub const GUID_PNP_POWER_NOTIFICATION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc2cf0660_eb7a_11d1_bd7f_0000f87571d0);
pub const GUID_PNP_POWER_SETTING_CHANGE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x29c69b3e_c79a_43bf_bbde_a932fa1bea7e);
pub const GUID_POWER_DEVICE_ENABLE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x827c0a6f_feb0_11d0_bd26_00aa00b7b32a);
pub const GUID_POWER_DEVICE_TIMEOUTS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa45da735_feb0_11d0_bd26_00aa00b7b32a);
pub const GUID_POWER_DEVICE_WAKE_ENABLE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa9546a82_feb0_11d0_bd26_00aa00b7b32a);
pub const GUID_PROCESSOR_PCC_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x37b17e9a_c21c_4296_972d_11c4b32b28f0);
pub const GUID_QUERY_CRASHDUMP_FUNCTIONS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9cc6b8ff_32e2_4834_b1de_b32ef8880a4b);
pub const GUID_RECOVERY_NVMED_PREPARE_SHUTDOWN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4b9770ea_bde7_400b_a9b9_4f684f54cc2a);
pub const GUID_RECOVERY_PCI_PREPARE_SHUTDOWN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x90d889de_8704_44cf_8115_ed8528d2b2da);
pub const GUID_REENUMERATE_SELF_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2aeb0243_6a6e_486b_82fc_d815f6b97006);
pub const GUID_SCM_BUS_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x25944783_ce79_4232_815e_4a30014e8eb4);
pub const GUID_SCM_BUS_LD_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x9b89307d_d76b_4f48_b186_54041ae92e8d);
pub const GUID_SCM_BUS_NVD_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x8de064ff_b630_42e4_88ea_6f24c8641175);
pub const GUID_SCM_PHYSICAL_NVDIMM_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x0079c21b_917e_405e_a9ce_0732b5bbcebd);
pub const GUID_SDEV_IDENTIFIER_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x49d67af8_916c_4ee8_9df1_889f17d21e91);
pub const GUID_SECURE_DRIVER_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x370f67e1_4ff5_4a94_9a35_06c5d9cc30e2);
pub const GUID_TARGET_DEVICE_QUERY_REMOVE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4006_46f0_11d0_b08f_00609713053f);
pub const GUID_TARGET_DEVICE_REMOVE_CANCELLED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4007_46f0_11d0_b08f_00609713053f);
pub const GUID_TARGET_DEVICE_REMOVE_COMPLETE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcb3a4008_46f0_11d0_b08f_00609713053f);
pub const GUID_TARGET_DEVICE_TRANSPORT_RELATIONS_CHANGED: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xfcf528f6_a82f_47b1_ad3a_8050594cad28);
pub const GUID_THERMAL_COOLING_INTERFACE: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xecbe47a8_c498_4bb9_bd70_e867e0940d22);
pub const GUID_TRANSLATOR_INTERFACE_STANDARD: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x6c154a92_aacf_11d0_8d2a_00a0c906b244);
pub const GUID_WUDF_DEVICE_HOST_PROBLEM: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc43d25bd_9346_40ee_a2d2_d70c15f8b75b);
pub const IDD_DYNAWIZ_ANALYZEDEV_PAGE: u32 = 10010u32;
pub const IDD_DYNAWIZ_ANALYZE_NEXTPAGE: u32 = 10004u32;
pub const IDD_DYNAWIZ_ANALYZE_PREVPAGE: u32 = 10003u32;
pub const IDD_DYNAWIZ_FIRSTPAGE: u32 = 10000u32;
pub const IDD_DYNAWIZ_INSTALLDETECTEDDEVS_PAGE: u32 = 10011u32;
pub const IDD_DYNAWIZ_INSTALLDETECTED_NEXTPAGE: u32 = 10007u32;
pub const IDD_DYNAWIZ_INSTALLDETECTED_NODEVS: u32 = 10008u32;
pub const IDD_DYNAWIZ_INSTALLDETECTED_PREVPAGE: u32 = 10006u32;
pub const IDD_DYNAWIZ_SELECTCLASS_PAGE: u32 = 10012u32;
pub const IDD_DYNAWIZ_SELECTDEV_PAGE: u32 = 10009u32;
pub const IDD_DYNAWIZ_SELECT_NEXTPAGE: u32 = 10002u32;
pub const IDD_DYNAWIZ_SELECT_PREVPAGE: u32 = 10001u32;
pub const IDF_CHECKFIRST: u32 = 256u32;
pub const IDF_NOBEEP: u32 = 512u32;
pub const IDF_NOBROWSE: u32 = 1u32;
pub const IDF_NOCOMPRESSED: u32 = 8u32;
pub const IDF_NODETAILS: u32 = 4u32;
pub const IDF_NOFOREGROUND: u32 = 1024u32;
pub const IDF_NOREMOVABLEMEDIAPROMPT: u32 = 4096u32;
pub const IDF_NOSKIP: u32 = 2u32;
pub const IDF_OEMDISK: u32 = 2147483648u32;
pub const IDF_USEDISKNAMEASPROMPT: u32 = 8192u32;
pub const IDF_WARNIFSKIP: u32 = 2048u32;
pub const IDI_CLASSICON_OVERLAYFIRST: u32 = 500u32;
pub const IDI_CLASSICON_OVERLAYLAST: u32 = 502u32;
pub const IDI_CONFLICT: u32 = 161u32;
pub const IDI_DISABLED_OVL: u32 = 501u32;
pub const IDI_FORCED_OVL: u32 = 502u32;
pub const IDI_PROBLEM_OVL: u32 = 500u32;
pub const IDI_RESOURCE: u32 = 159u32;
pub const IDI_RESOURCEFIRST: u32 = 159u32;
pub const IDI_RESOURCELAST: u32 = 161u32;
pub const IDI_RESOURCEOVERLAYFIRST: u32 = 161u32;
pub const IDI_RESOURCEOVERLAYLAST: u32 = 161u32;
pub const INFINFO_DEFAULT_SEARCH: u32 = 3u32;
pub const INFINFO_INF_NAME_IS_ABSOLUTE: u32 = 2u32;
pub const INFINFO_INF_PATH_LIST_SEARCH: u32 = 5u32;
pub const INFINFO_INF_SPEC_IS_HINF: u32 = 1u32;
pub const INFINFO_REVERSE_DEFAULT_SEARCH: u32 = 4u32;
pub const INFSTR_BUS_ALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BUS_ALL");
pub const INFSTR_BUS_EISA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BUS_EISA");
pub const INFSTR_BUS_ISA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BUS_ISA");
pub const INFSTR_BUS_MCA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BUS_MCA");
pub const INFSTR_CFGPRI_DESIRED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DESIRED");
pub const INFSTR_CFGPRI_DISABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DISABLED");
pub const INFSTR_CFGPRI_FORCECONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FORCECONFIG");
pub const INFSTR_CFGPRI_HARDRECONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HARDRECONFIG");
pub const INFSTR_CFGPRI_HARDWIRED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HARDWIRED");
pub const INFSTR_CFGPRI_NORMAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NORMAL");
pub const INFSTR_CFGPRI_POWEROFF: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("POWEROFF");
pub const INFSTR_CFGPRI_REBOOT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("REBOOT");
pub const INFSTR_CFGPRI_RESTART: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RESTART");
pub const INFSTR_CFGPRI_SUBOPTIMAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SUBOPTIMAL");
pub const INFSTR_CFGTYPE_BASIC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BASIC");
pub const INFSTR_CFGTYPE_FORCED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FORCED");
pub const INFSTR_CFGTYPE_OVERRIDE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OVERRIDE");
pub const INFSTR_CLASS_SAFEEXCL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SAFE_EXCL");
pub const INFSTR_CONTROLFLAGS_SECTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ControlFlags");
pub const INFSTR_DRIVERSELECT_FUNCTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverSelectFunctions");
pub const INFSTR_DRIVERSELECT_SECTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverSelect");
pub const INFSTR_DRIVERVERSION_SECTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverVer");
pub const INFSTR_KEY_ACTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Action");
pub const INFSTR_KEY_ALWAYSEXCLUDEFROMSELECT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AlwaysExcludeFromSelect");
pub const INFSTR_KEY_BUFFER_SIZE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BufferSize");
pub const INFSTR_KEY_CATALOGFILE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CatalogFile");
pub const INFSTR_KEY_CHANNEL_ACCESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Access");
pub const INFSTR_KEY_CHANNEL_ENABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Enabled");
pub const INFSTR_KEY_CHANNEL_ISOLATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Isolation");
pub const INFSTR_KEY_CHANNEL_VALUE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Value");
pub const INFSTR_KEY_CLASS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Class");
pub const INFSTR_KEY_CLASSGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClassGUID");
pub const INFSTR_KEY_CLOCK_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClockType");
pub const INFSTR_KEY_CONFIGPRIORITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ConfigPriority");
pub const INFSTR_KEY_COPYFILESONLY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CopyFilesOnly");
pub const INFSTR_KEY_DATA_ITEM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DataItem");
pub const INFSTR_KEY_DELAYEDAUTOSTART: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DelayedAutoStart");
pub const INFSTR_KEY_DEPENDENCIES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Dependencies");
pub const INFSTR_KEY_DESCRIPTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const INFSTR_KEY_DETECTLIST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DetectList");
pub const INFSTR_KEY_DETPARAMS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Params");
pub const INFSTR_KEY_DISABLE_REALTIME_PERSISTENCE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DisableRealtimePersistence");
pub const INFSTR_KEY_DISPLAYNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DisplayName");
pub const INFSTR_KEY_DMA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DMA");
pub const INFSTR_KEY_DMACONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DMAConfig");
pub const INFSTR_KEY_DRIVERSET: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverSet");
pub const INFSTR_KEY_ENABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Enabled");
pub const INFSTR_KEY_ENABLE_FLAGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableFlags");
pub const INFSTR_KEY_ENABLE_LEVEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableLevel");
pub const INFSTR_KEY_ENABLE_PROPERTY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableProperty");
pub const INFSTR_KEY_ERRORCONTROL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ErrorControl");
pub const INFSTR_KEY_EXCLUDEFROMSELECT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ExcludeFromSelect");
pub const INFSTR_KEY_EXCLUDERES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ExcludeRes");
pub const INFSTR_KEY_EXTENSIONID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ExtensionId");
pub const INFSTR_KEY_FAILURE_ACTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Action");
pub const INFSTR_KEY_FILE_MAX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FileMax");
pub const INFSTR_KEY_FILE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FileName");
pub const INFSTR_KEY_FLUSH_TIMER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FlushTimer");
pub const INFSTR_KEY_FROMINET: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FromINet");
pub const INFSTR_KEY_HARDWARE_CLASS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Class");
pub const INFSTR_KEY_HARDWARE_CLASSGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClassGUID");
pub const INFSTR_KEY_INTERACTIVEINSTALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InteractiveInstall");
pub const INFSTR_KEY_IO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IO");
pub const INFSTR_KEY_IOCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IOConfig");
pub const INFSTR_KEY_IRQ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IRQ");
pub const INFSTR_KEY_IRQCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IRQConfig");
pub const INFSTR_KEY_LOADORDERGROUP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LoadOrderGroup");
pub const INFSTR_KEY_LOGGING_AUTOBACKUP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LoggingAutoBackup");
pub const INFSTR_KEY_LOGGING_MAXSIZE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LoggingMaxSize");
pub const INFSTR_KEY_LOGGING_RETENTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LoggingRetention");
pub const INFSTR_KEY_LOG_FILE_MODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogFileMode");
pub const INFSTR_KEY_MATCH_ALL_KEYWORD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MatchAllKeyword");
pub const INFSTR_KEY_MATCH_ANY_KEYWORD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MatchAnyKeyword");
pub const INFSTR_KEY_MAXIMUM_BUFFERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaximumBuffers");
pub const INFSTR_KEY_MAX_FILE_SIZE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaxFileSize");
pub const INFSTR_KEY_MEM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Mem");
pub const INFSTR_KEY_MEMCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MemConfig");
pub const INFSTR_KEY_MEMLARGECONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MemLargeConfig");
pub const INFSTR_KEY_MESSAGE_FILE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MessageFile");
pub const INFSTR_KEY_MFCARDCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MfCardConfig");
pub const INFSTR_KEY_MINIMUM_BUFFERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MinimumBuffers");
pub const INFSTR_KEY_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const INFSTR_KEY_NON_CRASH_FAILURES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NonCrashFailures");
pub const INFSTR_KEY_NOSETUPINF: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NoSetupInf");
pub const INFSTR_KEY_PARAMETER_FILE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ParameterFile");
pub const INFSTR_KEY_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Path");
pub const INFSTR_KEY_PCCARDCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PcCardConfig");
pub const INFSTR_KEY_PNPLOCKDOWN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PnpLockDown");
pub const INFSTR_KEY_PROVIDER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Provider");
pub const INFSTR_KEY_PROVIDER_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ProviderName");
pub const INFSTR_KEY_REQUESTADDITIONALSOFTWARE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RequestAdditionalSoftware");
pub const INFSTR_KEY_REQUIREDPRIVILEGES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RequiredPrivileges");
pub const INFSTR_KEY_RESET_PERIOD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResetPeriod");
pub const INFSTR_KEY_RESOURCE_FILE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResourceFile");
pub const INFSTR_KEY_SECURITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Security");
pub const INFSTR_KEY_SERVICEBINARY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ServiceBinary");
pub const INFSTR_KEY_SERVICESIDTYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ServiceSidType");
pub const INFSTR_KEY_SERVICETYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ServiceType");
pub const INFSTR_KEY_SIGNATURE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Signature");
pub const INFSTR_KEY_SKIPLIST: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SkipList");
pub const INFSTR_KEY_START: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Start");
pub const INFSTR_KEY_STARTNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartName");
pub const INFSTR_KEY_STARTTYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartType");
pub const INFSTR_KEY_SUB_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SubType");
pub const INFSTR_KEY_TRIGGER_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TriggerType");
pub const INFSTR_PLATFORM_NT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NT");
pub const INFSTR_PLATFORM_NTALPHA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTAlpha");
pub const INFSTR_PLATFORM_NTAMD64: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTAMD64");
pub const INFSTR_PLATFORM_NTARM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTARM");
pub const INFSTR_PLATFORM_NTARM64: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTARM64");
pub const INFSTR_PLATFORM_NTAXP64: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTAXP64");
pub const INFSTR_PLATFORM_NTIA64: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTIA64");
pub const INFSTR_PLATFORM_NTMIPS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTMIPS");
pub const INFSTR_PLATFORM_NTPPC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTPPC");
pub const INFSTR_PLATFORM_NTX86: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NTx86");
pub const INFSTR_PLATFORM_WIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Win");
pub const INFSTR_REBOOT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Reboot");
pub const INFSTR_RESTART: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Restart");
pub const INFSTR_RISK_BIOSROMRD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_BIOSROMRD");
pub const INFSTR_RISK_DELICATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_DELICATE");
pub const INFSTR_RISK_IORD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_IORD");
pub const INFSTR_RISK_IOWR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_IOWR");
pub const INFSTR_RISK_LOW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_LOW");
pub const INFSTR_RISK_MEMRD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_MEMRD");
pub const INFSTR_RISK_MEMWR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_MEMWR");
pub const INFSTR_RISK_NONE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_NONE");
pub const INFSTR_RISK_QUERYDRV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_QUERYDRV");
pub const INFSTR_RISK_SWINT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_SWINT");
pub const INFSTR_RISK_UNRELIABLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_UNRELIABLE");
pub const INFSTR_RISK_VERYHIGH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_VERYHIGH");
pub const INFSTR_RISK_VERYLOW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RISK_VERYLOW");
pub const INFSTR_SECT_AUTOEXECBAT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AutoexecBatDrivers");
pub const INFSTR_SECT_AVOIDCFGSYSDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.AvoidCfgSysDev");
pub const INFSTR_SECT_AVOIDENVDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.AvoidEnvDev");
pub const INFSTR_SECT_AVOIDINIDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.AvoidIniDev");
pub const INFSTR_SECT_BADACPIBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadACPIBios");
pub const INFSTR_SECT_BADDISKBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadDiskBios");
pub const INFSTR_SECT_BADDSBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadDSBios");
pub const INFSTR_SECT_BADPMCALLBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadProtectedModeCallBios");
pub const INFSTR_SECT_BADPNPBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadPnpBios");
pub const INFSTR_SECT_BADRMCALLBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadRealModeCallBios");
pub const INFSTR_SECT_BADROUTINGTABLEBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BadPCIIRQRoutingTableBios");
pub const INFSTR_SECT_CFGSYS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ConfigSysDrivers");
pub const INFSTR_SECT_CLASS_INSTALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClassInstall");
pub const INFSTR_SECT_CLASS_INSTALL_32: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClassInstall32");
pub const INFSTR_SECT_DEFAULT_INSTALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultInstall");
pub const INFSTR_SECT_DEFAULT_UNINSTALL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultUninstall");
pub const INFSTR_SECT_DETCLASSINFO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.ClassInfo");
pub const INFSTR_SECT_DETMODULES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.Modules");
pub const INFSTR_SECT_DETOPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.Options");
pub const INFSTR_SECT_DEVINFS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.DevINFs");
pub const INFSTR_SECT_DISPLAY_CLEANUP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DisplayCleanup");
pub const INFSTR_SECT_EXTENSIONCONTRACTS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ExtensionContracts");
pub const INFSTR_SECT_FORCEHWVERIFY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.ForceHWVerify");
pub const INFSTR_SECT_GOODACPIBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GoodACPIBios");
pub const INFSTR_SECT_HPOMNIBOOK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.HPOmnibook");
pub const INFSTR_SECT_INTERFACE_INSTALL_32: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InterfaceInstall32");
pub const INFSTR_SECT_MACHINEIDBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MachineIDBios");
pub const INFSTR_SECT_MANUALDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.ManualDev");
pub const INFSTR_SECT_MFG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Manufacturer");
pub const INFSTR_SECT_REGCFGSYSDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.RegCfgSysDev");
pub const INFSTR_SECT_REGENVDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.RegEnvDev");
pub const INFSTR_SECT_REGINIDEV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det.RegIniDev");
pub const INFSTR_SECT_SYSINI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SystemIniDrivers");
pub const INFSTR_SECT_SYSINIDRV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SystemIniDriversLine");
pub const INFSTR_SECT_TARGETCOMPUTERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TargetComputers");
pub const INFSTR_SECT_VERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Version");
pub const INFSTR_SECT_WININIRUN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WinIniRunLine");
pub const INFSTR_SOFTWAREVERSION_SECTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SoftwareVersion");
pub const INFSTR_STRKEY_DRVDESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriverDesc");
pub const INFSTR_SUBKEY_COINSTALLERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CoInstallers");
pub const INFSTR_SUBKEY_CTL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CTL");
pub const INFSTR_SUBKEY_DET: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Det");
pub const INFSTR_SUBKEY_EVENTS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Events");
pub const INFSTR_SUBKEY_FACTDEF: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FactDef");
pub const INFSTR_SUBKEY_FILTERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Filters");
pub const INFSTR_SUBKEY_HW: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Hw");
pub const INFSTR_SUBKEY_INTERFACES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Interfaces");
pub const INFSTR_SUBKEY_LOGCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogConfig");
pub const INFSTR_SUBKEY_LOGCONFIGOVERRIDE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogConfigOverride");
pub const INFSTR_SUBKEY_NORESOURCEDUPS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NoResDup");
pub const INFSTR_SUBKEY_POSSIBLEDUPS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PosDup");
pub const INFSTR_SUBKEY_SERVICES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Services");
pub const INFSTR_SUBKEY_SOFTWARE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Software");
pub const INFSTR_SUBKEY_WMI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WMI");
pub const INF_STYLE_CACHE_DISABLE: INF_STYLE = 32u32;
pub const INF_STYLE_CACHE_ENABLE: INF_STYLE = 16u32;
pub const INF_STYLE_CACHE_IGNORE: INF_STYLE = 64u32;
pub const INF_STYLE_NONE: INF_STYLE = 0u32;
pub const INF_STYLE_OLDNT: INF_STYLE = 1u32;
pub const INF_STYLE_WIN4: INF_STYLE = 2u32;
pub const INSTALLFLAG_BITS: UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS = 7u32;
pub const INSTALLFLAG_FORCE: UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS = 1u32;
pub const INSTALLFLAG_NONINTERACTIVE: UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS = 4u32;
pub const INSTALLFLAG_READONLY: UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS = 2u32;
pub const IOA_Local: u32 = 255u32;
pub const IO_ALIAS_10_BIT_DECODE: u32 = 4u32;
pub const IO_ALIAS_12_BIT_DECODE: u32 = 16u32;
pub const IO_ALIAS_16_BIT_DECODE: u32 = 0u32;
pub const IO_ALIAS_POSITIVE_DECODE: u32 = 255u32;
pub const LCPRI_BOOTCONFIG: u32 = 1u32;
pub const LCPRI_DESIRED: u32 = 8192u32;
pub const LCPRI_DISABLED: u32 = 65535u32;
pub const LCPRI_FORCECONFIG: u32 = 0u32;
pub const LCPRI_HARDRECONFIG: u32 = 49152u32;
pub const LCPRI_HARDWIRED: u32 = 57344u32;
pub const LCPRI_IMPOSSIBLE: u32 = 61440u32;
pub const LCPRI_LASTBESTCONFIG: u32 = 16383u32;
pub const LCPRI_LASTSOFTCONFIG: u32 = 32767u32;
pub const LCPRI_NORMAL: u32 = 12288u32;
pub const LCPRI_POWEROFF: u32 = 40960u32;
pub const LCPRI_REBOOT: u32 = 36864u32;
pub const LCPRI_RESTART: u32 = 32768u32;
pub const LCPRI_SUBOPTIMAL: u32 = 20480u32;
pub const LINE_LEN: u32 = 256u32;
pub const LOG_CONF_BITS: u32 = 7u32;
pub const LogSevError: u32 = 2u32;
pub const LogSevFatalError: u32 = 3u32;
pub const LogSevInformation: u32 = 0u32;
pub const LogSevMaximum: u32 = 4u32;
pub const LogSevWarning: u32 = 1u32;
pub const MAX_CLASS_NAME_LEN: u32 = 32u32;
pub const MAX_CONFIG_VALUE: u32 = 9999u32;
pub const MAX_DEVICE_ID_LEN: u32 = 200u32;
pub const MAX_DEVNODE_ID_LEN: u32 = 200u32;
pub const MAX_DMA_CHANNELS: u32 = 7u32;
pub const MAX_GUID_STRING_LEN: u32 = 39u32;
pub const MAX_IDD_DYNAWIZ_RESOURCE_ID: u32 = 11000u32;
pub const MAX_INFSTR_STRKEY_LEN: u32 = 32u32;
pub const MAX_INF_FLAG: u32 = 20u32;
pub const MAX_INF_SECTION_NAME_LENGTH: u32 = 255u32;
pub const MAX_INF_STRING_LENGTH: u32 = 4096u32;
pub const MAX_INSTALLWIZARD_DYNAPAGES: u32 = 20u32;
pub const MAX_INSTANCE_VALUE: u32 = 9999u32;
pub const MAX_INSTRUCTION_LEN: u32 = 256u32;
pub const MAX_IO_PORTS: u32 = 20u32;
pub const MAX_IRQS: u32 = 7u32;
pub const MAX_KEY_LEN: u32 = 100u32;
pub const MAX_LABEL_LEN: u32 = 30u32;
pub const MAX_LCPRI: u32 = 65535u32;
pub const MAX_MEM_REGISTERS: u32 = 9u32;
pub const MAX_PRIORITYSTR_LEN: u32 = 16u32;
pub const MAX_PROFILE_LEN: u32 = 80u32;
pub const MAX_SERVICE_NAME_LEN: u32 = 256u32;
pub const MAX_SUBTITLE_LEN: u32 = 256u32;
pub const MAX_TITLE_LEN: u32 = 60u32;
pub const MIN_IDD_DYNAWIZ_RESOURCE_ID: u32 = 10000u32;
pub const NDW_INSTALLFLAG_CI_PICKED_OEM: u32 = 32768u32;
pub const NDW_INSTALLFLAG_DIDFACTDEFS: u32 = 1u32;
pub const NDW_INSTALLFLAG_EXPRESSINTRO: u32 = 1024u32;
pub const NDW_INSTALLFLAG_HARDWAREALLREADYIN: u32 = 2u32;
pub const NDW_INSTALLFLAG_INSTALLSPECIFIC: u32 = 8192u32;
pub const NDW_INSTALLFLAG_KNOWNCLASS: u32 = 524288u32;
pub const NDW_INSTALLFLAG_NEEDREBOOT: i32 = 256i32;
pub const NDW_INSTALLFLAG_NEEDRESTART: i32 = 128i32;
pub const NDW_INSTALLFLAG_NEEDSHUTDOWN: u32 = 512u32;
pub const NDW_INSTALLFLAG_NODETECTEDDEVS: u32 = 4096u32;
pub const NDW_INSTALLFLAG_PCMCIADEVICE: u32 = 131072u32;
pub const NDW_INSTALLFLAG_PCMCIAMODE: u32 = 65536u32;
pub const NDW_INSTALLFLAG_SKIPCLASSLIST: u32 = 16384u32;
pub const NDW_INSTALLFLAG_SKIPISDEVINSTALLED: u32 = 2048u32;
pub const NDW_INSTALLFLAG_USERCANCEL: u32 = 262144u32;
pub const NUM_CM_PROB: u32 = 58u32;
pub const NUM_CM_PROB_V1: u32 = 37u32;
pub const NUM_CM_PROB_V2: u32 = 50u32;
pub const NUM_CM_PROB_V3: u32 = 51u32;
pub const NUM_CM_PROB_V4: u32 = 52u32;
pub const NUM_CM_PROB_V5: u32 = 53u32;
pub const NUM_CM_PROB_V6: u32 = 54u32;
pub const NUM_CM_PROB_V7: u32 = 55u32;
pub const NUM_CM_PROB_V8: u32 = 57u32;
pub const NUM_CM_PROB_V9: u32 = 58u32;
pub const NUM_CR_RESULTS: CONFIGRET = 60u32;
pub const NUM_LOG_CONF: CM_LOG_CONF = 6u32;
pub const OVERRIDE_LOG_CONF: CM_LOG_CONF = 5u32;
pub const PCD_MAX_IO: u32 = 2u32;
pub const PCD_MAX_MEMORY: u32 = 2u32;
pub const PNP_VetoAlreadyRemoved: PNP_VETO_TYPE = 13i32;
pub const PNP_VetoDevice: PNP_VETO_TYPE = 6i32;
pub const PNP_VetoDriver: PNP_VETO_TYPE = 7i32;
pub const PNP_VetoIllegalDeviceRequest: PNP_VETO_TYPE = 8i32;
pub const PNP_VetoInsufficientPower: PNP_VETO_TYPE = 9i32;
pub const PNP_VetoInsufficientRights: PNP_VETO_TYPE = 12i32;
pub const PNP_VetoLegacyDevice: PNP_VETO_TYPE = 1i32;
pub const PNP_VetoLegacyDriver: PNP_VETO_TYPE = 11i32;
pub const PNP_VetoNonDisableable: PNP_VETO_TYPE = 10i32;
pub const PNP_VetoOutstandingOpen: PNP_VETO_TYPE = 5i32;
pub const PNP_VetoPendingClose: PNP_VETO_TYPE = 2i32;
pub const PNP_VetoTypeUnknown: PNP_VETO_TYPE = 0i32;
pub const PNP_VetoWindowsApp: PNP_VETO_TYPE = 3i32;
pub const PNP_VetoWindowsService: PNP_VETO_TYPE = 4i32;
pub const PRIORITY_BIT: u32 = 8u32;
pub const PRIORITY_EQUAL_FIRST: u32 = 8u32;
pub const PRIORITY_EQUAL_LAST: u32 = 0u32;
pub const ROLLBACK_BITS: DIROLLBACKDRIVER_FLAGS = 1u32;
pub const ROLLBACK_FLAG_NO_UI: DIROLLBACKDRIVER_FLAGS = 1u32;
pub const RegDisposition_Bits: u32 = 1u32;
pub const RegDisposition_OpenAlways: u32 = 0u32;
pub const RegDisposition_OpenExisting: u32 = 1u32;
pub const ResType_All: CM_RESTYPE = 0u32;
pub const ResType_BusNumber: CM_RESTYPE = 6u32;
pub const ResType_ClassSpecific: CM_RESTYPE = 65535u32;
pub const ResType_Connection: CM_RESTYPE = 32772u32;
pub const ResType_DMA: CM_RESTYPE = 3u32;
pub const ResType_DevicePrivate: CM_RESTYPE = 32769u32;
pub const ResType_DoNotUse: CM_RESTYPE = 5u32;
pub const ResType_IO: CM_RESTYPE = 2u32;
pub const ResType_IRQ: CM_RESTYPE = 4u32;
pub const ResType_Ignored_Bit: CM_RESTYPE = 32768u32;
pub const ResType_MAX: CM_RESTYPE = 7u32;
pub const ResType_Mem: CM_RESTYPE = 1u32;
pub const ResType_MemLarge: CM_RESTYPE = 7u32;
pub const ResType_MfCardConfig: CM_RESTYPE = 32771u32;
pub const ResType_None: CM_RESTYPE = 0u32;
pub const ResType_PcCardConfig: CM_RESTYPE = 32770u32;
pub const ResType_Reserved: CM_RESTYPE = 32768u32;
pub const SCWMI_CLOBBER_SECURITY: u32 = 1u32;
pub const SETDIRID_NOT_FULL_PATH: u32 = 1u32;
pub const SIGNERSCORE_AUTHENTICODE: u32 = 251658240u32;
pub const SIGNERSCORE_INBOX: u32 = 218103811u32;
pub const SIGNERSCORE_LOGO_PREMIUM: u32 = 218103809u32;
pub const SIGNERSCORE_LOGO_STANDARD: u32 = 218103810u32;
pub const SIGNERSCORE_MASK: u32 = 4278190080u32;
pub const SIGNERSCORE_SIGNED_MASK: u32 = 4026531840u32;
pub const SIGNERSCORE_UNCLASSIFIED: u32 = 218103812u32;
pub const SIGNERSCORE_UNKNOWN: u32 = 4278190080u32;
pub const SIGNERSCORE_UNSIGNED: u32 = 2147483648u32;
pub const SIGNERSCORE_W9X_SUSPECT: u32 = 3221225472u32;
pub const SIGNERSCORE_WHQL: u32 = 218103813u32;
pub const SPCRP_CHARACTERISTICS: u32 = 27u32;
pub const SPCRP_DEVTYPE: u32 = 25u32;
pub const SPCRP_EXCLUSIVE: u32 = 26u32;
pub const SPCRP_LOWERFILTERS: u32 = 18u32;
pub const SPCRP_MAXIMUM_PROPERTY: u32 = 28u32;
pub const SPCRP_SECURITY: u32 = 23u32;
pub const SPCRP_SECURITY_SDS: u32 = 24u32;
pub const SPCRP_UPPERFILTERS: u32 = 17u32;
pub const SPDIT_CLASSDRIVER: SETUP_DI_BUILD_DRIVER_DRIVER_TYPE = 1u32;
pub const SPDIT_COMPATDRIVER: SETUP_DI_BUILD_DRIVER_DRIVER_TYPE = 2u32;
pub const SPDIT_NODRIVER: u32 = 0u32;
pub const SPDRP_ADDRESS: u32 = 28u32;
pub const SPDRP_BASE_CONTAINERID: u32 = 36u32;
pub const SPDRP_BUSNUMBER: u32 = 21u32;
pub const SPDRP_BUSTYPEGUID: u32 = 19u32;
pub const SPDRP_CAPABILITIES: u32 = 15u32;
pub const SPDRP_CHARACTERISTICS: u32 = 27u32;
pub const SPDRP_CLASS: u32 = 7u32;
pub const SPDRP_CLASSGUID: u32 = 8u32;
pub const SPDRP_COMPATIBLEIDS: u32 = 2u32;
pub const SPDRP_CONFIGFLAGS: u32 = 10u32;
pub const SPDRP_DEVICEDESC: u32 = 0u32;
pub const SPDRP_DEVICE_POWER_DATA: u32 = 30u32;
pub const SPDRP_DEVTYPE: u32 = 25u32;
pub const SPDRP_DRIVER: u32 = 9u32;
pub const SPDRP_ENUMERATOR_NAME: u32 = 22u32;
pub const SPDRP_EXCLUSIVE: u32 = 26u32;
pub const SPDRP_FRIENDLYNAME: u32 = 12u32;
pub const SPDRP_HARDWAREID: u32 = 1u32;
pub const SPDRP_INSTALL_STATE: u32 = 34u32;
pub const SPDRP_LEGACYBUSTYPE: u32 = 20u32;
pub const SPDRP_LOCATION_INFORMATION: u32 = 13u32;
pub const SPDRP_LOCATION_PATHS: u32 = 35u32;
pub const SPDRP_LOWERFILTERS: u32 = 18u32;
pub const SPDRP_MAXIMUM_PROPERTY: u32 = 37u32;
pub const SPDRP_MFG: u32 = 11u32;
pub const SPDRP_PHYSICAL_DEVICE_OBJECT_NAME: u32 = 14u32;
pub const SPDRP_REMOVAL_POLICY: u32 = 31u32;
pub const SPDRP_REMOVAL_POLICY_HW_DEFAULT: u32 = 32u32;
pub const SPDRP_REMOVAL_POLICY_OVERRIDE: u32 = 33u32;
pub const SPDRP_SECURITY: u32 = 23u32;
pub const SPDRP_SECURITY_SDS: u32 = 24u32;
pub const SPDRP_SERVICE: u32 = 4u32;
pub const SPDRP_UI_NUMBER: u32 = 16u32;
pub const SPDRP_UI_NUMBER_DESC_FORMAT: u32 = 29u32;
pub const SPDRP_UNUSED0: u32 = 3u32;
pub const SPDRP_UNUSED1: u32 = 5u32;
pub const SPDRP_UNUSED2: u32 = 6u32;
pub const SPDRP_UPPERFILTERS: u32 = 17u32;
pub const SPDSL_DISALLOW_NEGATIVE_ADJUST: u32 = 2u32;
pub const SPDSL_IGNORE_DISK: u32 = 1u32;
pub const SPFILELOG_FORCENEW: u32 = 2u32;
pub const SPFILELOG_OEMFILE: u32 = 1u32;
pub const SPFILELOG_QUERYONLY: u32 = 4u32;
pub const SPFILELOG_SYSTEMLOG: u32 = 1u32;
pub const SPFILENOTIFY_BACKUPERROR: u32 = 22u32;
pub const SPFILENOTIFY_CABINETINFO: u32 = 16u32;
pub const SPFILENOTIFY_COPYERROR: u32 = 13u32;
pub const SPFILENOTIFY_DELETEERROR: u32 = 7u32;
pub const SPFILENOTIFY_ENDBACKUP: u32 = 23u32;
pub const SPFILENOTIFY_ENDCOPY: u32 = 12u32;
pub const SPFILENOTIFY_ENDDELETE: u32 = 6u32;
pub const SPFILENOTIFY_ENDQUEUE: u32 = 2u32;
pub const SPFILENOTIFY_ENDREGISTRATION: u32 = 32u32;
pub const SPFILENOTIFY_ENDRENAME: u32 = 9u32;
pub const SPFILENOTIFY_ENDSUBQUEUE: u32 = 4u32;
pub const SPFILENOTIFY_FILEEXTRACTED: u32 = 19u32;
pub const SPFILENOTIFY_FILEINCABINET: u32 = 17u32;
pub const SPFILENOTIFY_FILEOPDELAYED: u32 = 20u32;
pub const SPFILENOTIFY_LANGMISMATCH: u32 = 65536u32;
pub const SPFILENOTIFY_NEEDMEDIA: u32 = 14u32;
pub const SPFILENOTIFY_NEEDNEWCABINET: u32 = 18u32;
pub const SPFILENOTIFY_QUEUESCAN: u32 = 15u32;
pub const SPFILENOTIFY_QUEUESCAN_EX: u32 = 24u32;
pub const SPFILENOTIFY_QUEUESCAN_SIGNERINFO: u32 = 64u32;
pub const SPFILENOTIFY_RENAMEERROR: u32 = 10u32;
pub const SPFILENOTIFY_STARTBACKUP: u32 = 21u32;
pub const SPFILENOTIFY_STARTCOPY: u32 = 11u32;
pub const SPFILENOTIFY_STARTDELETE: u32 = 5u32;
pub const SPFILENOTIFY_STARTQUEUE: u32 = 1u32;
pub const SPFILENOTIFY_STARTREGISTRATION: u32 = 25u32;
pub const SPFILENOTIFY_STARTRENAME: u32 = 8u32;
pub const SPFILENOTIFY_STARTSUBQUEUE: u32 = 3u32;
pub const SPFILENOTIFY_TARGETEXISTS: u32 = 131072u32;
pub const SPFILENOTIFY_TARGETNEWER: u32 = 262144u32;
pub const SPFILEQ_FILE_IN_USE: u32 = 1u32;
pub const SPFILEQ_REBOOT_IN_PROGRESS: u32 = 4u32;
pub const SPFILEQ_REBOOT_RECOMMENDED: u32 = 2u32;
pub const SPID_ACTIVE: u32 = 1u32;
pub const SPID_DEFAULT: u32 = 2u32;
pub const SPID_REMOVED: u32 = 4u32;
pub const SPINST_ALL: u32 = 2047u32;
pub const SPINST_BITREG: u32 = 32u32;
pub const SPINST_COPYINF: u32 = 512u32;
pub const SPINST_DEVICEINSTALL: u32 = 1048576u32;
pub const SPINST_FILES: u32 = 16u32;
pub const SPINST_INI2REG: u32 = 8u32;
pub const SPINST_INIFILES: u32 = 2u32;
pub const SPINST_LOGCONFIG: u32 = 1u32;
pub const SPINST_LOGCONFIGS_ARE_OVERRIDES: u32 = 262144u32;
pub const SPINST_LOGCONFIG_IS_FORCED: u32 = 131072u32;
pub const SPINST_PROFILEITEMS: u32 = 256u32;
pub const SPINST_PROPERTIES: u32 = 1024u32;
pub const SPINST_REGISTERCALLBACKAWARE: u32 = 524288u32;
pub const SPINST_REGISTRY: u32 = 4u32;
pub const SPINST_REGSVR: u32 = 64u32;
pub const SPINST_SINGLESECTION: u32 = 65536u32;
pub const SPINST_UNREGSVR: u32 = 128u32;
pub const SPINT_ACTIVE: u32 = 1u32;
pub const SPINT_DEFAULT: u32 = 2u32;
pub const SPINT_REMOVED: u32 = 4u32;
pub const SPOST_MAX: u32 = 3u32;
pub const SPOST_NONE: OEM_SOURCE_MEDIA_TYPE = 0u32;
pub const SPOST_PATH: OEM_SOURCE_MEDIA_TYPE = 1u32;
pub const SPOST_URL: OEM_SOURCE_MEDIA_TYPE = 2u32;
pub const SPPSR_ENUM_ADV_DEVICE_PROPERTIES: u32 = 3u32;
pub const SPPSR_ENUM_BASIC_DEVICE_PROPERTIES: u32 = 2u32;
pub const SPPSR_SELECT_DEVICE_RESOURCES: u32 = 1u32;
pub const SPQ_DELAYED_COPY: u32 = 1u32;
pub const SPQ_FLAG_ABORT_IF_UNSIGNED: u32 = 2u32;
pub const SPQ_FLAG_BACKUP_AWARE: u32 = 1u32;
pub const SPQ_FLAG_DO_SHUFFLEMOVE: u32 = 8u32;
pub const SPQ_FLAG_FILES_MODIFIED: u32 = 4u32;
pub const SPQ_FLAG_VALID: u32 = 15u32;
pub const SPQ_SCAN_ACTIVATE_DRP: SETUPSCANFILEQUEUE_FLAGS = 1024u32;
pub const SPQ_SCAN_FILE_COMPARISON: SETUPSCANFILEQUEUE_FLAGS = 512u32;
pub const SPQ_SCAN_FILE_PRESENCE: SETUPSCANFILEQUEUE_FLAGS = 1u32;
pub const SPQ_SCAN_FILE_PRESENCE_WITHOUT_SOURCE: SETUPSCANFILEQUEUE_FLAGS = 256u32;
pub const SPQ_SCAN_FILE_VALIDITY: SETUPSCANFILEQUEUE_FLAGS = 2u32;
pub const SPQ_SCAN_INFORM_USER: SETUPSCANFILEQUEUE_FLAGS = 16u32;
pub const SPQ_SCAN_PRUNE_COPY_QUEUE: SETUPSCANFILEQUEUE_FLAGS = 32u32;
pub const SPQ_SCAN_PRUNE_DELREN: SETUPSCANFILEQUEUE_FLAGS = 128u32;
pub const SPQ_SCAN_USE_CALLBACK: SETUPSCANFILEQUEUE_FLAGS = 4u32;
pub const SPQ_SCAN_USE_CALLBACKEX: SETUPSCANFILEQUEUE_FLAGS = 8u32;
pub const SPQ_SCAN_USE_CALLBACK_SIGNERINFO: SETUPSCANFILEQUEUE_FLAGS = 64u32;
pub const SPRDI_FIND_DUPS: u32 = 1u32;
pub const SPREG_DLLINSTALL: u32 = 4u32;
pub const SPREG_GETPROCADDR: u32 = 2u32;
pub const SPREG_LOADLIBRARY: u32 = 1u32;
pub const SPREG_REGSVR: u32 = 3u32;
pub const SPREG_SUCCESS: u32 = 0u32;
pub const SPREG_TIMEOUT: u32 = 5u32;
pub const SPREG_UNKNOWN: u32 = 4294967295u32;
pub const SPSVCINST_ASSOCSERVICE: SPSVCINST_FLAGS = 2u32;
pub const SPSVCINST_CLOBBER_SECURITY: SPSVCINST_FLAGS = 1024u32;
pub const SPSVCINST_DELETEEVENTLOGENTRY: SPSVCINST_FLAGS = 4u32;
pub const SPSVCINST_NOCLOBBER_DELAYEDAUTOSTART: SPSVCINST_FLAGS = 32768u32;
pub const SPSVCINST_NOCLOBBER_DEPENDENCIES: SPSVCINST_FLAGS = 128u32;
pub const SPSVCINST_NOCLOBBER_DESCRIPTION: SPSVCINST_FLAGS = 256u32;
pub const SPSVCINST_NOCLOBBER_DISPLAYNAME: SPSVCINST_FLAGS = 8u32;
pub const SPSVCINST_NOCLOBBER_ERRORCONTROL: SPSVCINST_FLAGS = 32u32;
pub const SPSVCINST_NOCLOBBER_FAILUREACTIONS: SPSVCINST_FLAGS = 131072u32;
pub const SPSVCINST_NOCLOBBER_LOADORDERGROUP: SPSVCINST_FLAGS = 64u32;
pub const SPSVCINST_NOCLOBBER_REQUIREDPRIVILEGES: SPSVCINST_FLAGS = 4096u32;
pub const SPSVCINST_NOCLOBBER_SERVICESIDTYPE: SPSVCINST_FLAGS = 16384u32;
pub const SPSVCINST_NOCLOBBER_STARTTYPE: SPSVCINST_FLAGS = 16u32;
pub const SPSVCINST_NOCLOBBER_TRIGGERS: SPSVCINST_FLAGS = 8192u32;
pub const SPSVCINST_STARTSERVICE: SPSVCINST_FLAGS = 2048u32;
pub const SPSVCINST_STOPSERVICE: SPSVCINST_FLAGS = 512u32;
pub const SPSVCINST_TAGTOFRONT: SPSVCINST_FLAGS = 1u32;
pub const SPSVCINST_UNIQUE_NAME: SPSVCINST_FLAGS = 65536u32;
pub const SPWPT_SELECTDEVICE: u32 = 1u32;
pub const SPWP_USE_DEVINFO_DATA: u32 = 1u32;
pub const SP_ALTPLATFORM_FLAGS_SUITE_MASK: u32 = 2u32;
pub const SP_ALTPLATFORM_FLAGS_VERSION_RANGE: u32 = 1u32;
pub const SP_BACKUP_BACKUPPASS: u32 = 1u32;
pub const SP_BACKUP_BOOTFILE: u32 = 8u32;
pub const SP_BACKUP_DEMANDPASS: u32 = 2u32;
pub const SP_BACKUP_SPECIAL: u32 = 4u32;
pub const SP_COPY_ALREADYDECOMP: SP_COPY_STYLE = 4194304u32;
pub const SP_COPY_DELETESOURCE: SP_COPY_STYLE = 1u32;
pub const SP_COPY_FORCE_IN_USE: SP_COPY_STYLE = 512u32;
pub const SP_COPY_FORCE_NEWER: SP_COPY_STYLE = 8192u32;
pub const SP_COPY_FORCE_NOOVERWRITE: SP_COPY_STYLE = 4096u32;
pub const SP_COPY_HARDLINK: SP_COPY_STYLE = 268435456u32;
pub const SP_COPY_INBOX_INF: SP_COPY_STYLE = 134217728u32;
pub const SP_COPY_IN_USE_NEEDS_REBOOT: SP_COPY_STYLE = 256u32;
pub const SP_COPY_IN_USE_TRY_RENAME: SP_COPY_STYLE = 67108864u32;
pub const SP_COPY_LANGUAGEAWARE: SP_COPY_STYLE = 32u32;
pub const SP_COPY_NEWER: SP_COPY_STYLE = 4u32;
pub const SP_COPY_NEWER_ONLY: SP_COPY_STYLE = 65536u32;
pub const SP_COPY_NEWER_OR_SAME: SP_COPY_STYLE = 4u32;
pub const SP_COPY_NOBROWSE: SP_COPY_STYLE = 32768u32;
pub const SP_COPY_NODECOMP: SP_COPY_STYLE = 16u32;
pub const SP_COPY_NOOVERWRITE: SP_COPY_STYLE = 8u32;
pub const SP_COPY_NOPRUNE: SP_COPY_STYLE = 1048576u32;
pub const SP_COPY_NOSKIP: SP_COPY_STYLE = 1024u32;
pub const SP_COPY_OEMINF_CATALOG_ONLY: SP_COPY_STYLE = 262144u32;
pub const SP_COPY_OEM_F6_INF: SP_COPY_STYLE = 2097152u32;
pub const SP_COPY_PNPLOCKED: SP_COPY_STYLE = 33554432u32;
pub const SP_COPY_REPLACEONLY: SP_COPY_STYLE = 2u32;
pub const SP_COPY_REPLACE_BOOT_FILE: SP_COPY_STYLE = 524288u32;
pub const SP_COPY_RESERVED: SP_COPY_STYLE = 131072u32;
pub const SP_COPY_SOURCEPATH_ABSOLUTE: SP_COPY_STYLE = 128u32;
pub const SP_COPY_SOURCE_ABSOLUTE: SP_COPY_STYLE = 64u32;
pub const SP_COPY_WARNIFSKIP: SP_COPY_STYLE = 16384u32;
pub const SP_COPY_WINDOWS_SIGNED: SP_COPY_STYLE = 16777216u32;
pub const SP_FLAG_CABINETCONTINUATION: u32 = 2048u32;
pub const SP_MAX_MACHINENAME_LENGTH: u32 = 263u32;
pub const SRCINFO_DESCRIPTION: u32 = 3u32;
pub const SRCINFO_FLAGS: u32 = 4u32;
pub const SRCINFO_PATH: u32 = 1u32;
pub const SRCINFO_TAGFILE: u32 = 2u32;
pub const SRCINFO_TAGFILE2: u32 = 5u32;
pub const SRCLIST_APPEND: u32 = 512u32;
pub const SRCLIST_NOBROWSE: u32 = 2u32;
pub const SRCLIST_NOSTRIPPLATFORM: u32 = 1024u32;
pub const SRCLIST_SUBDIRS: u32 = 256u32;
pub const SRCLIST_SYSIFADMIN: u32 = 64u32;
pub const SRCLIST_SYSTEM: u32 = 16u32;
pub const SRCLIST_TEMPORARY: u32 = 1u32;
pub const SRCLIST_USER: u32 = 32u32;
pub const SRC_FLAGS_CABFILE: u32 = 16u32;
pub const SUOI_FORCEDELETE: u32 = 1u32;
pub const SUOI_INTERNAL1: u32 = 2u32;
pub const SZ_KEY_ADDAUTOLOGGER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddAutoLogger");
pub const SZ_KEY_ADDAUTOLOGGERPROVIDER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddAutoLoggerProvider");
pub const SZ_KEY_ADDCHANNEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddChannel");
pub const SZ_KEY_ADDEVENTPROVIDER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddEventProvider");
pub const SZ_KEY_ADDFILTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddFilter");
pub const SZ_KEY_ADDIME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddIme");
pub const SZ_KEY_ADDINTERFACE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddInterface");
pub const SZ_KEY_ADDPOWERSETTING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddPowerSetting");
pub const SZ_KEY_ADDPROP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddProperty");
pub const SZ_KEY_ADDREG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddReg");
pub const SZ_KEY_ADDREGNOCLOBBER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddRegNoClobber");
pub const SZ_KEY_ADDSERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddService");
pub const SZ_KEY_ADDTRIGGER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddTrigger");
pub const SZ_KEY_BITREG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BitReg");
pub const SZ_KEY_CLEANONLY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CleanOnly");
pub const SZ_KEY_COPYFILES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CopyFiles");
pub const SZ_KEY_COPYINF: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CopyINF");
pub const SZ_KEY_DEFAULTOPTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultOption");
pub const SZ_KEY_DEFDESTDIR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultDestDir");
pub const SZ_KEY_DELFILES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DelFiles");
pub const SZ_KEY_DELIME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DelIme");
pub const SZ_KEY_DELPROP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DelProperty");
pub const SZ_KEY_DELREG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DelReg");
pub const SZ_KEY_DELSERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DelService");
pub const SZ_KEY_DESTDIRS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DestinationDirs");
pub const SZ_KEY_EXCLUDEID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ExcludeId");
pub const SZ_KEY_FAILUREACTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailureActions");
pub const SZ_KEY_FEATURESCORE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FeatureScore");
pub const SZ_KEY_FILTERLEVEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FilterLevel");
pub const SZ_KEY_FILTERPOSITION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FilterPosition");
pub const SZ_KEY_HARDWARE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Hardware");
pub const SZ_KEY_IMPORTCHANNEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ImportChannel");
pub const SZ_KEY_INI2REG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Ini2Reg");
pub const SZ_KEY_LAYOUT_FILE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LayoutFile");
pub const SZ_KEY_LDIDOEM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LdidOEM");
pub const SZ_KEY_LFN_SECTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VarLDID.LFN");
pub const SZ_KEY_LISTOPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ListOptions");
pub const SZ_KEY_LOGCONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogConfig");
pub const SZ_KEY_MODULES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Modules");
pub const SZ_KEY_OPTIONDESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OptionDesc");
pub const SZ_KEY_PHASE1: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Phase1");
pub const SZ_KEY_PROFILEITEMS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ProfileItems");
pub const SZ_KEY_REGSVR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RegisterDlls");
pub const SZ_KEY_RENFILES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RenFiles");
pub const SZ_KEY_SFN_SECTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VarLDID.SFN");
pub const SZ_KEY_SRCDISKFILES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SourceDisksFiles");
pub const SZ_KEY_SRCDISKNAMES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SourceDisksNames");
pub const SZ_KEY_STRINGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Strings");
pub const SZ_KEY_UNREGSVR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UnregisterDlls");
pub const SZ_KEY_UPDATEAUTOLOGGER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UpdateAutoLogger");
pub const SZ_KEY_UPDATEINIFIELDS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UpdateIniFields");
pub const SZ_KEY_UPDATEINIS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UpdateInis");
pub const SZ_KEY_UPGRADEONLY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UpgradeOnly");
pub const SetupFileLogChecksum: SetupFileLogInfo = 1i32;
pub const SetupFileLogDiskDescription: SetupFileLogInfo = 3i32;
pub const SetupFileLogDiskTagfile: SetupFileLogInfo = 2i32;
pub const SetupFileLogMax: SetupFileLogInfo = 5i32;
pub const SetupFileLogOtherInfo: SetupFileLogInfo = 4i32;
pub const SetupFileLogSourceFilename: SetupFileLogInfo = 0i32;
pub const fDD_BYTE: u32 = 0u32;
pub const fDD_BYTE_AND_WORD: u32 = 3u32;
pub const fDD_BusMaster: u32 = 4u32;
pub const fDD_DWORD: u32 = 2u32;
pub const fDD_NoBusMaster: u32 = 0u32;
pub const fDD_TypeA: u32 = 8u32;
pub const fDD_TypeB: u32 = 16u32;
pub const fDD_TypeF: u32 = 24u32;
pub const fDD_TypeStandard: u32 = 0u32;
pub const fDD_WORD: u32 = 1u32;
pub const fIOD_10_BIT_DECODE: u32 = 4u32;
pub const fIOD_12_BIT_DECODE: u32 = 8u32;
pub const fIOD_16_BIT_DECODE: u32 = 16u32;
pub const fIOD_DECODE: u32 = 252u32;
pub const fIOD_IO: u32 = 1u32;
pub const fIOD_Memory: u32 = 0u32;
pub const fIOD_PASSIVE_DECODE: u32 = 64u32;
pub const fIOD_PORT_BAR: u32 = 256u32;
pub const fIOD_POSITIVE_DECODE: u32 = 32u32;
pub const fIOD_PortType: u32 = 1u32;
pub const fIOD_WINDOW_DECODE: u32 = 128u32;
pub const fIRQD_Edge: u32 = 2u32;
pub const fIRQD_Exclusive: u32 = 0u32;
pub const fIRQD_Level: u32 = 0u32;
pub const fIRQD_Level_Bit: u32 = 1u32;
pub const fIRQD_Share: u32 = 1u32;
pub const fIRQD_Share_Bit: u32 = 0u32;
pub const fMD_24: u32 = 0u32;
pub const fMD_32: u32 = 2u32;
pub const fMD_32_24: u32 = 2u32;
pub const fMD_Cacheable: u32 = 32u32;
pub const fMD_CombinedWrite: u32 = 16u32;
pub const fMD_CombinedWriteAllowed: u32 = 16u32;
pub const fMD_CombinedWriteDisallowed: u32 = 0u32;
pub const fMD_MEMORY_BAR: u32 = 128u32;
pub const fMD_MemoryType: u32 = 1u32;
pub const fMD_NonCacheable: u32 = 0u32;
pub const fMD_Pref: u32 = 4u32;
pub const fMD_PrefetchAllowed: u32 = 4u32;
pub const fMD_PrefetchDisallowed: u32 = 0u32;
pub const fMD_Prefetchable: u32 = 4u32;
pub const fMD_RAM: u32 = 1u32;
pub const fMD_ROM: u32 = 0u32;
pub const fMD_ReadAllowed: u32 = 0u32;
pub const fMD_ReadDisallowed: u32 = 8u32;
pub const fMD_Readable: u32 = 8u32;
pub const fMD_WINDOW_DECODE: u32 = 64u32;
pub const fPCD_ATTRIBUTES_PER_WINDOW: u32 = 32768u32;
pub const fPCD_IO1_16: u32 = 65536u32;
pub const fPCD_IO1_SRC_16: u32 = 262144u32;
pub const fPCD_IO1_WS_16: u32 = 524288u32;
pub const fPCD_IO1_ZW_8: u32 = 131072u32;
pub const fPCD_IO2_16: u32 = 1048576u32;
pub const fPCD_IO2_SRC_16: u32 = 4194304u32;
pub const fPCD_IO2_WS_16: u32 = 8388608u32;
pub const fPCD_IO2_ZW_8: u32 = 2097152u32;
pub const fPCD_IO_16: u32 = 1u32;
pub const fPCD_IO_8: u32 = 0u32;
pub const fPCD_IO_SRC_16: u32 = 32u32;
pub const fPCD_IO_WS_16: u32 = 64u32;
pub const fPCD_IO_ZW_8: u32 = 16u32;
pub const fPCD_MEM1_16: u32 = 67108864u32;
pub const fPCD_MEM1_A: u32 = 4u32;
pub const fPCD_MEM1_WS_ONE: u32 = 16777216u32;
pub const fPCD_MEM1_WS_THREE: u32 = 50331648u32;
pub const fPCD_MEM1_WS_TWO: u32 = 33554432u32;
pub const fPCD_MEM2_16: u32 = 1073741824u32;
pub const fPCD_MEM2_A: u32 = 8u32;
pub const fPCD_MEM2_WS_ONE: u32 = 268435456u32;
pub const fPCD_MEM2_WS_THREE: u32 = 805306368u32;
pub const fPCD_MEM2_WS_TWO: u32 = 536870912u32;
pub const fPCD_MEM_16: u32 = 2u32;
pub const fPCD_MEM_8: u32 = 0u32;
pub const fPCD_MEM_A: u32 = 4u32;
pub const fPCD_MEM_WS_ONE: u32 = 256u32;
pub const fPCD_MEM_WS_THREE: u32 = 768u32;
pub const fPCD_MEM_WS_TWO: u32 = 512u32;
pub const fPMF_AUDIO_ENABLE: u32 = 8u32;
pub const mDD_BusMaster: u32 = 4u32;
pub const mDD_Type: u32 = 24u32;
pub const mDD_Width: u32 = 3u32;
pub const mIRQD_Edge_Level: u32 = 2u32;
pub const mIRQD_Share: u32 = 1u32;
pub const mMD_32_24: u32 = 2u32;
pub const mMD_Cacheable: u32 = 32u32;
pub const mMD_CombinedWrite: u32 = 16u32;
pub const mMD_MemoryType: u32 = 1u32;
pub const mMD_Prefetchable: u32 = 4u32;
pub const mMD_Readable: u32 = 8u32;
pub const mPCD_IO_8_16: u32 = 1u32;
pub const mPCD_MEM1_WS: u32 = 50331648u32;
pub const mPCD_MEM2_WS: u32 = 805306368u32;
pub const mPCD_MEM_8_16: u32 = 2u32;
pub const mPCD_MEM_A_C: u32 = 12u32;
pub const mPCD_MEM_WS: u32 = 768u32;
pub const mPMF_AUDIO_ENABLE: u32 = 8u32;
pub type CM_CDFLAGS = u32;
pub type CM_CDMASK = u32;
pub type CM_DEVCAP = u32;
pub type CM_DEVNODE_STATUS_FLAGS = u32;
pub type CM_ENUMERATE_FLAGS = u32;
pub type CM_GET_DEVICE_INTERFACE_LIST_FLAGS = u32;
pub type CM_INSTALL_STATE = u32;
pub type CM_LOCATE_DEVNODE_FLAGS = u32;
pub type CM_LOG_CONF = u32;
pub type CM_NOTIFY_ACTION = i32;
pub type CM_NOTIFY_FILTER_TYPE = i32;
pub type CM_PROB = u32;
pub type CM_REENUMERATE_FLAGS = u32;
pub type CM_REMOVAL_POLICY = u32;
pub type CM_RESTYPE = u32;
pub type CONFIGRET = u32;
pub type DIINSTALLDEVICE_FLAGS = u32;
pub type DIINSTALLDRIVER_FLAGS = u32;
pub type DIROLLBACKDRIVER_FLAGS = u32;
pub type DIUNINSTALLDRIVER_FLAGS = u32;
pub type INF_STYLE = u32;
pub type OEM_SOURCE_MEDIA_TYPE = u32;
pub type PNP_VETO_TYPE = i32;
pub type SETUPSCANFILEQUEUE_FLAGS = u32;
pub type SETUP_DI_BUILD_DRIVER_DRIVER_TYPE = u32;
pub type SETUP_FILE_OPERATION = u32;
pub type SPSVCINST_FLAGS = u32;
pub type SP_COPY_STYLE = u32;
pub type SetupFileLogInfo = i32;
pub type UPDATEDRIVERFORPLUGANDPLAYDEVICES_FLAGS = u32;
#[repr(C, packed(1))]
pub struct BUSNUMBER_DES {
    pub BUSD_Count: u32,
    pub BUSD_Type: u32,
    pub BUSD_Flags: u32,
    pub BUSD_Alloc_Base: u32,
    pub BUSD_Alloc_End: u32,
}
impl ::core::marker::Copy for BUSNUMBER_DES {}
impl ::core::clone::Clone for BUSNUMBER_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BUSNUMBER_RANGE {
    pub BUSR_Min: u32,
    pub BUSR_Max: u32,
    pub BUSR_nBusNumbers: u32,
    pub BUSR_Flags: u32,
}
impl ::core::marker::Copy for BUSNUMBER_RANGE {}
impl ::core::clone::Clone for BUSNUMBER_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct BUSNUMBER_RESOURCE {
    pub BusNumber_Header: BUSNUMBER_DES,
    pub BusNumber_Data: [BUSNUMBER_RANGE; 1],
}
impl ::core::marker::Copy for BUSNUMBER_RESOURCE {}
impl ::core::clone::Clone for BUSNUMBER_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct CABINET_INFO_A {
    pub CabinetPath: ::windows_sys::core::PCSTR,
    pub CabinetFile: ::windows_sys::core::PCSTR,
    pub DiskName: ::windows_sys::core::PCSTR,
    pub SetId: u16,
    pub CabinetNumber: u16,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for CABINET_INFO_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for CABINET_INFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct CABINET_INFO_A {
    pub CabinetPath: ::windows_sys::core::PCSTR,
    pub CabinetFile: ::windows_sys::core::PCSTR,
    pub DiskName: ::windows_sys::core::PCSTR,
    pub SetId: u16,
    pub CabinetNumber: u16,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for CABINET_INFO_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for CABINET_INFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct CABINET_INFO_W {
    pub CabinetPath: ::windows_sys::core::PCWSTR,
    pub CabinetFile: ::windows_sys::core::PCWSTR,
    pub DiskName: ::windows_sys::core::PCWSTR,
    pub SetId: u16,
    pub CabinetNumber: u16,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for CABINET_INFO_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for CABINET_INFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct CABINET_INFO_W {
    pub CabinetPath: ::windows_sys::core::PCWSTR,
    pub CabinetFile: ::windows_sys::core::PCWSTR,
    pub DiskName: ::windows_sys::core::PCWSTR,
    pub SetId: u16,
    pub CabinetNumber: u16,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for CABINET_INFO_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for CABINET_INFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CM_NOTIFY_EVENT_DATA {
    pub FilterType: CM_NOTIFY_FILTER_TYPE,
    pub Reserved: u32,
    pub u: CM_NOTIFY_EVENT_DATA_0,
}
impl ::core::marker::Copy for CM_NOTIFY_EVENT_DATA {}
impl ::core::clone::Clone for CM_NOTIFY_EVENT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CM_NOTIFY_EVENT_DATA_0 {
    pub DeviceInterface: CM_NOTIFY_EVENT_DATA_0_2,
    pub DeviceHandle: CM_NOTIFY_EVENT_DATA_0_0,
    pub DeviceInstance: CM_NOTIFY_EVENT_DATA_0_1,
}
impl ::core::marker::Copy for CM_NOTIFY_EVENT_DATA_0 {}
impl ::core::clone::Clone for CM_NOTIFY_EVENT_DATA_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CM_NOTIFY_EVENT_DATA_0_0 {
    pub EventGuid: ::windows_sys::core::GUID,
    pub NameOffset: i32,
    pub DataSize: u32,
    pub Data: [u8; 1],
}
impl ::core::marker::Copy for CM_NOTIFY_EVENT_DATA_0_0 {}
impl ::core::clone::Clone for CM_NOTIFY_EVENT_DATA_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CM_NOTIFY_EVENT_DATA_0_1 {
    pub InstanceId: [u16; 1],
}
impl ::core::marker::Copy for CM_NOTIFY_EVENT_DATA_0_1 {}
impl ::core::clone::Clone for CM_NOTIFY_EVENT_DATA_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CM_NOTIFY_EVENT_DATA_0_2 {
    pub ClassGuid: ::windows_sys::core::GUID,
    pub SymbolicLink: [u16; 1],
}
impl ::core::marker::Copy for CM_NOTIFY_EVENT_DATA_0_2 {}
impl ::core::clone::Clone for CM_NOTIFY_EVENT_DATA_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CM_NOTIFY_FILTER {
    pub cbSize: u32,
    pub Flags: u32,
    pub FilterType: CM_NOTIFY_FILTER_TYPE,
    pub Reserved: u32,
    pub u: CM_NOTIFY_FILTER_0,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CM_NOTIFY_FILTER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CM_NOTIFY_FILTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union CM_NOTIFY_FILTER_0 {
    pub DeviceInterface: CM_NOTIFY_FILTER_0_2,
    pub DeviceHandle: CM_NOTIFY_FILTER_0_0,
    pub DeviceInstance: CM_NOTIFY_FILTER_0_1,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CM_NOTIFY_FILTER_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CM_NOTIFY_FILTER_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CM_NOTIFY_FILTER_0_0 {
    pub hTarget: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CM_NOTIFY_FILTER_0_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CM_NOTIFY_FILTER_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CM_NOTIFY_FILTER_0_1 {
    pub InstanceId: [u16; 200],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CM_NOTIFY_FILTER_0_1 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CM_NOTIFY_FILTER_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CM_NOTIFY_FILTER_0_2 {
    pub ClassGuid: ::windows_sys::core::GUID,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CM_NOTIFY_FILTER_0_2 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CM_NOTIFY_FILTER_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct COINSTALLER_CONTEXT_DATA {
    pub PostProcessing: super::super::Foundation::BOOL,
    pub InstallResult: u32,
    pub PrivateData: *mut ::core::ffi::c_void,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for COINSTALLER_CONTEXT_DATA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for COINSTALLER_CONTEXT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct COINSTALLER_CONTEXT_DATA {
    pub PostProcessing: super::super::Foundation::BOOL,
    pub InstallResult: u32,
    pub PrivateData: *mut ::core::ffi::c_void,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for COINSTALLER_CONTEXT_DATA {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for COINSTALLER_CONTEXT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONFLICT_DETAILS_A {
    pub CD_ulSize: u32,
    pub CD_ulMask: CM_CDMASK,
    pub CD_dnDevInst: u32,
    pub CD_rdResDes: usize,
    pub CD_ulFlags: CM_CDFLAGS,
    pub CD_szDescription: [u8; 260],
}
impl ::core::marker::Copy for CONFLICT_DETAILS_A {}
impl ::core::clone::Clone for CONFLICT_DETAILS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CONFLICT_DETAILS_W {
    pub CD_ulSize: u32,
    pub CD_ulMask: CM_CDMASK,
    pub CD_dnDevInst: u32,
    pub CD_rdResDes: usize,
    pub CD_ulFlags: CM_CDFLAGS,
    pub CD_szDescription: [u16; 260],
}
impl ::core::marker::Copy for CONFLICT_DETAILS_W {}
impl ::core::clone::Clone for CONFLICT_DETAILS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct CONNECTION_DES {
    pub COND_Type: u32,
    pub COND_Flags: u32,
    pub COND_Class: u8,
    pub COND_ClassType: u8,
    pub COND_Reserved1: u8,
    pub COND_Reserved2: u8,
    pub COND_Id: i64,
}
impl ::core::marker::Copy for CONNECTION_DES {}
impl ::core::clone::Clone for CONNECTION_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct CONNECTION_RESOURCE {
    pub Connection_Header: CONNECTION_DES,
}
impl ::core::marker::Copy for CONNECTION_RESOURCE {}
impl ::core::clone::Clone for CONNECTION_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct CS_DES {
    pub CSD_SignatureLength: u32,
    pub CSD_LegacyDataOffset: u32,
    pub CSD_LegacyDataSize: u32,
    pub CSD_Flags: u32,
    pub CSD_ClassGuid: ::windows_sys::core::GUID,
    pub CSD_Signature: [u8; 1],
}
impl ::core::marker::Copy for CS_DES {}
impl ::core::clone::Clone for CS_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct CS_RESOURCE {
    pub CS_Header: CS_DES,
}
impl ::core::marker::Copy for CS_RESOURCE {}
impl ::core::clone::Clone for CS_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DEVPRIVATE_DES {
    pub PD_Count: u32,
    pub PD_Type: u32,
    pub PD_Data1: u32,
    pub PD_Data2: u32,
    pub PD_Data3: u32,
    pub PD_Flags: u32,
}
impl ::core::marker::Copy for DEVPRIVATE_DES {}
impl ::core::clone::Clone for DEVPRIVATE_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DEVPRIVATE_RANGE {
    pub PR_Data1: u32,
    pub PR_Data2: u32,
    pub PR_Data3: u32,
}
impl ::core::marker::Copy for DEVPRIVATE_RANGE {}
impl ::core::clone::Clone for DEVPRIVATE_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DEVPRIVATE_RESOURCE {
    pub PRV_Header: DEVPRIVATE_DES,
    pub PRV_Data: [DEVPRIVATE_RANGE; 1],
}
impl ::core::marker::Copy for DEVPRIVATE_RESOURCE {}
impl ::core::clone::Clone for DEVPRIVATE_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DMA_DES {
    pub DD_Count: u32,
    pub DD_Type: u32,
    pub DD_Flags: u32,
    pub DD_Alloc_Chan: u32,
}
impl ::core::marker::Copy for DMA_DES {}
impl ::core::clone::Clone for DMA_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DMA_RANGE {
    pub DR_Min: u32,
    pub DR_Max: u32,
    pub DR_Flags: u32,
}
impl ::core::marker::Copy for DMA_RANGE {}
impl ::core::clone::Clone for DMA_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct DMA_RESOURCE {
    pub DMA_Header: DMA_DES,
    pub DMA_Data: [DMA_RANGE; 1],
}
impl ::core::marker::Copy for DMA_RESOURCE {}
impl ::core::clone::Clone for DMA_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct FILEPATHS_A {
    pub Target: ::windows_sys::core::PCSTR,
    pub Source: ::windows_sys::core::PCSTR,
    pub Win32Error: u32,
    pub Flags: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for FILEPATHS_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for FILEPATHS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct FILEPATHS_A {
    pub Target: ::windows_sys::core::PCSTR,
    pub Source: ::windows_sys::core::PCSTR,
    pub Win32Error: u32,
    pub Flags: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for FILEPATHS_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for FILEPATHS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct FILEPATHS_SIGNERINFO_A {
    pub Target: ::windows_sys::core::PCSTR,
    pub Source: ::windows_sys::core::PCSTR,
    pub Win32Error: u32,
    pub Flags: u32,
    pub DigitalSigner: ::windows_sys::core::PCSTR,
    pub Version: ::windows_sys::core::PCSTR,
    pub CatalogFile: ::windows_sys::core::PCSTR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for FILEPATHS_SIGNERINFO_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for FILEPATHS_SIGNERINFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct FILEPATHS_SIGNERINFO_A {
    pub Target: ::windows_sys::core::PCSTR,
    pub Source: ::windows_sys::core::PCSTR,
    pub Win32Error: u32,
    pub Flags: u32,
    pub DigitalSigner: ::windows_sys::core::PCSTR,
    pub Version: ::windows_sys::core::PCSTR,
    pub CatalogFile: ::windows_sys::core::PCSTR,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for FILEPATHS_SIGNERINFO_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for FILEPATHS_SIGNERINFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct FILEPATHS_SIGNERINFO_W {
    pub Target: ::windows_sys::core::PCWSTR,
    pub Source: ::windows_sys::core::PCWSTR,
    pub Win32Error: u32,
    pub Flags: u32,
    pub DigitalSigner: ::windows_sys::core::PCWSTR,
    pub Version: ::windows_sys::core::PCWSTR,
    pub CatalogFile: ::windows_sys::core::PCWSTR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for FILEPATHS_SIGNERINFO_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for FILEPATHS_SIGNERINFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct FILEPATHS_SIGNERINFO_W {
    pub Target: ::windows_sys::core::PCWSTR,
    pub Source: ::windows_sys::core::PCWSTR,
    pub Win32Error: u32,
    pub Flags: u32,
    pub DigitalSigner: ::windows_sys::core::PCWSTR,
    pub Version: ::windows_sys::core::PCWSTR,
    pub CatalogFile: ::windows_sys::core::PCWSTR,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for FILEPATHS_SIGNERINFO_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for FILEPATHS_SIGNERINFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct FILEPATHS_W {
    pub Target: ::windows_sys::core::PCWSTR,
    pub Source: ::windows_sys::core::PCWSTR,
    pub Win32Error: u32,
    pub Flags: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for FILEPATHS_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for FILEPATHS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct FILEPATHS_W {
    pub Target: ::windows_sys::core::PCWSTR,
    pub Source: ::windows_sys::core::PCWSTR,
    pub Win32Error: u32,
    pub Flags: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for FILEPATHS_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for FILEPATHS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct FILE_IN_CABINET_INFO_A {
    pub NameInCabinet: ::windows_sys::core::PCSTR,
    pub FileSize: u32,
    pub Win32Error: u32,
    pub DosDate: u16,
    pub DosTime: u16,
    pub DosAttribs: u16,
    pub FullTargetName: [u8; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for FILE_IN_CABINET_INFO_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for FILE_IN_CABINET_INFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct FILE_IN_CABINET_INFO_A {
    pub NameInCabinet: ::windows_sys::core::PCSTR,
    pub FileSize: u32,
    pub Win32Error: u32,
    pub DosDate: u16,
    pub DosTime: u16,
    pub DosAttribs: u16,
    pub FullTargetName: [u8; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for FILE_IN_CABINET_INFO_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for FILE_IN_CABINET_INFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct FILE_IN_CABINET_INFO_W {
    pub NameInCabinet: ::windows_sys::core::PCWSTR,
    pub FileSize: u32,
    pub Win32Error: u32,
    pub DosDate: u16,
    pub DosTime: u16,
    pub DosAttribs: u16,
    pub FullTargetName: [u16; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for FILE_IN_CABINET_INFO_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for FILE_IN_CABINET_INFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct FILE_IN_CABINET_INFO_W {
    pub NameInCabinet: ::windows_sys::core::PCWSTR,
    pub FileSize: u32,
    pub Win32Error: u32,
    pub DosDate: u16,
    pub DosTime: u16,
    pub DosAttribs: u16,
    pub FullTargetName: [u16; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for FILE_IN_CABINET_INFO_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for FILE_IN_CABINET_INFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HCMNOTIFICATION = isize;
pub type HDEVINFO = isize;
#[repr(C, packed(1))]
pub struct HWPROFILEINFO_A {
    pub HWPI_ulHWProfile: u32,
    pub HWPI_szFriendlyName: [u8; 80],
    pub HWPI_dwFlags: u32,
}
impl ::core::marker::Copy for HWPROFILEINFO_A {}
impl ::core::clone::Clone for HWPROFILEINFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct HWPROFILEINFO_W {
    pub HWPI_ulHWProfile: u32,
    pub HWPI_szFriendlyName: [u16; 80],
    pub HWPI_dwFlags: u32,
}
impl ::core::marker::Copy for HWPROFILEINFO_W {}
impl ::core::clone::Clone for HWPROFILEINFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct INFCONTEXT {
    pub Inf: *mut ::core::ffi::c_void,
    pub CurrentInf: *mut ::core::ffi::c_void,
    pub Section: u32,
    pub Line: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for INFCONTEXT {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for INFCONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct INFCONTEXT {
    pub Inf: *mut ::core::ffi::c_void,
    pub CurrentInf: *mut ::core::ffi::c_void,
    pub Section: u32,
    pub Line: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for INFCONTEXT {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for INFCONTEXT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IO_DES {
    pub IOD_Count: u32,
    pub IOD_Type: u32,
    pub IOD_Alloc_Base: u64,
    pub IOD_Alloc_End: u64,
    pub IOD_DesFlags: u32,
}
impl ::core::marker::Copy for IO_DES {}
impl ::core::clone::Clone for IO_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IO_RANGE {
    pub IOR_Align: u64,
    pub IOR_nPorts: u32,
    pub IOR_Min: u64,
    pub IOR_Max: u64,
    pub IOR_RangeFlags: u32,
    pub IOR_Alias: u64,
}
impl ::core::marker::Copy for IO_RANGE {}
impl ::core::clone::Clone for IO_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct IO_RESOURCE {
    pub IO_Header: IO_DES,
    pub IO_Data: [IO_RANGE; 1],
}
impl ::core::marker::Copy for IO_RESOURCE {}
impl ::core::clone::Clone for IO_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IRQ_DES_32 {
    pub IRQD_Count: u32,
    pub IRQD_Type: u32,
    pub IRQD_Flags: u32,
    pub IRQD_Alloc_Num: u32,
    pub IRQD_Affinity: u32,
}
impl ::core::marker::Copy for IRQ_DES_32 {}
impl ::core::clone::Clone for IRQ_DES_32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IRQ_DES_64 {
    pub IRQD_Count: u32,
    pub IRQD_Type: u32,
    pub IRQD_Flags: u32,
    pub IRQD_Alloc_Num: u32,
    pub IRQD_Affinity: u64,
}
impl ::core::marker::Copy for IRQ_DES_64 {}
impl ::core::clone::Clone for IRQ_DES_64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IRQ_RANGE {
    pub IRQR_Min: u32,
    pub IRQR_Max: u32,
    pub IRQR_Flags: u32,
}
impl ::core::marker::Copy for IRQ_RANGE {}
impl ::core::clone::Clone for IRQ_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IRQ_RESOURCE_32 {
    pub IRQ_Header: IRQ_DES_32,
    pub IRQ_Data: [IRQ_RANGE; 1],
}
impl ::core::marker::Copy for IRQ_RESOURCE_32 {}
impl ::core::clone::Clone for IRQ_RESOURCE_32 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct IRQ_RESOURCE_64 {
    pub IRQ_Header: IRQ_DES_64,
    pub IRQ_Data: [IRQ_RANGE; 1],
}
impl ::core::marker::Copy for IRQ_RESOURCE_64 {}
impl ::core::clone::Clone for IRQ_RESOURCE_64 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MEM_DES {
    pub MD_Count: u32,
    pub MD_Type: u32,
    pub MD_Alloc_Base: u64,
    pub MD_Alloc_End: u64,
    pub MD_Flags: u32,
    pub MD_Reserved: u32,
}
impl ::core::marker::Copy for MEM_DES {}
impl ::core::clone::Clone for MEM_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MEM_LARGE_DES {
    pub MLD_Count: u32,
    pub MLD_Type: u32,
    pub MLD_Alloc_Base: u64,
    pub MLD_Alloc_End: u64,
    pub MLD_Flags: u32,
    pub MLD_Reserved: u32,
}
impl ::core::marker::Copy for MEM_LARGE_DES {}
impl ::core::clone::Clone for MEM_LARGE_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MEM_LARGE_RANGE {
    pub MLR_Align: u64,
    pub MLR_nBytes: u64,
    pub MLR_Min: u64,
    pub MLR_Max: u64,
    pub MLR_Flags: u32,
    pub MLR_Reserved: u32,
}
impl ::core::marker::Copy for MEM_LARGE_RANGE {}
impl ::core::clone::Clone for MEM_LARGE_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MEM_LARGE_RESOURCE {
    pub MEM_LARGE_Header: MEM_LARGE_DES,
    pub MEM_LARGE_Data: [MEM_LARGE_RANGE; 1],
}
impl ::core::marker::Copy for MEM_LARGE_RESOURCE {}
impl ::core::clone::Clone for MEM_LARGE_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MEM_RANGE {
    pub MR_Align: u64,
    pub MR_nBytes: u32,
    pub MR_Min: u64,
    pub MR_Max: u64,
    pub MR_Flags: u32,
    pub MR_Reserved: u32,
}
impl ::core::marker::Copy for MEM_RANGE {}
impl ::core::clone::Clone for MEM_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MEM_RESOURCE {
    pub MEM_Header: MEM_DES,
    pub MEM_Data: [MEM_RANGE; 1],
}
impl ::core::marker::Copy for MEM_RESOURCE {}
impl ::core::clone::Clone for MEM_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MFCARD_DES {
    pub PMF_Count: u32,
    pub PMF_Type: u32,
    pub PMF_Flags: u32,
    pub PMF_ConfigOptions: u8,
    pub PMF_IoResourceIndex: u8,
    pub PMF_Reserved: [u8; 2],
    pub PMF_ConfigRegisterBase: u32,
}
impl ::core::marker::Copy for MFCARD_DES {}
impl ::core::clone::Clone for MFCARD_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct MFCARD_RESOURCE {
    pub MfCard_Header: MFCARD_DES,
}
impl ::core::marker::Copy for MFCARD_RESOURCE {}
impl ::core::clone::Clone for MFCARD_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PCCARD_DES {
    pub PCD_Count: u32,
    pub PCD_Type: u32,
    pub PCD_Flags: u32,
    pub PCD_ConfigIndex: u8,
    pub PCD_Reserved: [u8; 3],
    pub PCD_MemoryCardBase1: u32,
    pub PCD_MemoryCardBase2: u32,
    pub PCD_MemoryCardBase: [u32; 2],
    pub PCD_MemoryFlags: [u16; 2],
    pub PCD_IoFlags: [u8; 2],
}
impl ::core::marker::Copy for PCCARD_DES {}
impl ::core::clone::Clone for PCCARD_DES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
pub struct PCCARD_RESOURCE {
    pub PcCard_Header: PCCARD_DES,
}
impl ::core::marker::Copy for PCCARD_RESOURCE {}
impl ::core::clone::Clone for PCCARD_RESOURCE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SOURCE_MEDIA_A {
    pub Reserved: ::windows_sys::core::PCSTR,
    pub Tagfile: ::windows_sys::core::PCSTR,
    pub Description: ::windows_sys::core::PCSTR,
    pub SourcePath: ::windows_sys::core::PCSTR,
    pub SourceFile: ::windows_sys::core::PCSTR,
    pub Flags: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SOURCE_MEDIA_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SOURCE_MEDIA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SOURCE_MEDIA_A {
    pub Reserved: ::windows_sys::core::PCSTR,
    pub Tagfile: ::windows_sys::core::PCSTR,
    pub Description: ::windows_sys::core::PCSTR,
    pub SourcePath: ::windows_sys::core::PCSTR,
    pub SourceFile: ::windows_sys::core::PCSTR,
    pub Flags: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SOURCE_MEDIA_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SOURCE_MEDIA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SOURCE_MEDIA_W {
    pub Reserved: ::windows_sys::core::PCWSTR,
    pub Tagfile: ::windows_sys::core::PCWSTR,
    pub Description: ::windows_sys::core::PCWSTR,
    pub SourcePath: ::windows_sys::core::PCWSTR,
    pub SourceFile: ::windows_sys::core::PCWSTR,
    pub Flags: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SOURCE_MEDIA_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SOURCE_MEDIA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SOURCE_MEDIA_W {
    pub Reserved: ::windows_sys::core::PCWSTR,
    pub Tagfile: ::windows_sys::core::PCWSTR,
    pub Description: ::windows_sys::core::PCWSTR,
    pub SourcePath: ::windows_sys::core::PCWSTR,
    pub SourceFile: ::windows_sys::core::PCWSTR,
    pub Flags: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SOURCE_MEDIA_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SOURCE_MEDIA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
pub struct SP_ALTPLATFORM_INFO_V1 {
    pub cbSize: u32,
    pub Platform: super::super::System::Diagnostics::Debug::VER_PLATFORM,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub ProcessorArchitecture: u16,
    pub Reserved: u16,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V1 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
pub struct SP_ALTPLATFORM_INFO_V1 {
    pub cbSize: u32,
    pub Platform: super::super::System::Diagnostics::Debug::VER_PLATFORM,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub ProcessorArchitecture: u16,
    pub Reserved: u16,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V1 {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
pub struct SP_ALTPLATFORM_INFO_V2 {
    pub cbSize: u32,
    pub Platform: super::super::System::Diagnostics::Debug::VER_PLATFORM,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub ProcessorArchitecture: u16,
    pub Anonymous: SP_ALTPLATFORM_INFO_V2_0,
    pub FirstValidatedMajorVersion: u32,
    pub FirstValidatedMinorVersion: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V2 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
pub union SP_ALTPLATFORM_INFO_V2_0 {
    pub Reserved: u16,
    pub Flags: u16,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V2_0 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
pub struct SP_ALTPLATFORM_INFO_V2 {
    pub cbSize: u32,
    pub Platform: super::super::System::Diagnostics::Debug::VER_PLATFORM,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub ProcessorArchitecture: u16,
    pub Anonymous: SP_ALTPLATFORM_INFO_V2_0,
    pub FirstValidatedMajorVersion: u32,
    pub FirstValidatedMinorVersion: u32,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V2 {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_System_Diagnostics_Debug\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
pub union SP_ALTPLATFORM_INFO_V2_0 {
    pub Reserved: u16,
    pub Flags: u16,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V2_0 {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_System_Diagnostics_Debug")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V2_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_ALTPLATFORM_INFO_V3 {
    pub cbSize: u32,
    pub Platform: u32,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub ProcessorArchitecture: u16,
    pub Anonymous: SP_ALTPLATFORM_INFO_V3_0,
    pub FirstValidatedMajorVersion: u32,
    pub FirstValidatedMinorVersion: u32,
    pub ProductType: u8,
    pub SuiteMask: u16,
    pub BuildNumber: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V3 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub union SP_ALTPLATFORM_INFO_V3_0 {
    pub Reserved: u16,
    pub Flags: u16,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V3_0 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V3_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_ALTPLATFORM_INFO_V3 {
    pub cbSize: u32,
    pub Platform: u32,
    pub MajorVersion: u32,
    pub MinorVersion: u32,
    pub ProcessorArchitecture: u16,
    pub Anonymous: SP_ALTPLATFORM_INFO_V3_0,
    pub FirstValidatedMajorVersion: u32,
    pub FirstValidatedMinorVersion: u32,
    pub ProductType: u8,
    pub SuiteMask: u16,
    pub BuildNumber: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V3 {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V3 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub union SP_ALTPLATFORM_INFO_V3_0 {
    pub Reserved: u16,
    pub Flags: u16,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_ALTPLATFORM_INFO_V3_0 {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_ALTPLATFORM_INFO_V3_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_BACKUP_QUEUE_PARAMS_V1_A {
    pub cbSize: u32,
    pub FullInfPath: [u8; 260],
    pub FilenameOffset: i32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V1_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_BACKUP_QUEUE_PARAMS_V1_A {
    pub cbSize: u32,
    pub FullInfPath: [u8; 260],
    pub FilenameOffset: i32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V1_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_BACKUP_QUEUE_PARAMS_V1_W {
    pub cbSize: u32,
    pub FullInfPath: [u16; 260],
    pub FilenameOffset: i32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V1_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_BACKUP_QUEUE_PARAMS_V1_W {
    pub cbSize: u32,
    pub FullInfPath: [u16; 260],
    pub FilenameOffset: i32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V1_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_BACKUP_QUEUE_PARAMS_V2_A {
    pub cbSize: u32,
    pub FullInfPath: [u8; 260],
    pub FilenameOffset: i32,
    pub ReinstallInstance: [u8; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V2_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_BACKUP_QUEUE_PARAMS_V2_A {
    pub cbSize: u32,
    pub FullInfPath: [u8; 260],
    pub FilenameOffset: i32,
    pub ReinstallInstance: [u8; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V2_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_BACKUP_QUEUE_PARAMS_V2_W {
    pub cbSize: u32,
    pub FullInfPath: [u16; 260],
    pub FilenameOffset: i32,
    pub ReinstallInstance: [u16; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V2_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_BACKUP_QUEUE_PARAMS_V2_W {
    pub cbSize: u32,
    pub FullInfPath: [u16; 260],
    pub FilenameOffset: i32,
    pub ReinstallInstance: [u16; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_BACKUP_QUEUE_PARAMS_V2_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_BACKUP_QUEUE_PARAMS_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_UI_Controls\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_UI_Controls")]
pub struct SP_CLASSIMAGELIST_DATA {
    pub cbSize: u32,
    pub ImageList: super::super::UI::Controls::HIMAGELIST,
    pub Reserved: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_UI_Controls")]
impl ::core::marker::Copy for SP_CLASSIMAGELIST_DATA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_UI_Controls")]
impl ::core::clone::Clone for SP_CLASSIMAGELIST_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_UI_Controls\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_UI_Controls")]
pub struct SP_CLASSIMAGELIST_DATA {
    pub cbSize: u32,
    pub ImageList: super::super::UI::Controls::HIMAGELIST,
    pub Reserved: usize,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_UI_Controls")]
impl ::core::marker::Copy for SP_CLASSIMAGELIST_DATA {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_UI_Controls")]
impl ::core::clone::Clone for SP_CLASSIMAGELIST_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_CLASSINSTALL_HEADER {
    pub cbSize: u32,
    pub InstallFunction: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_CLASSINSTALL_HEADER {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_CLASSINSTALL_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_CLASSINSTALL_HEADER {
    pub cbSize: u32,
    pub InstallFunction: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_CLASSINSTALL_HEADER {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_CLASSINSTALL_HEADER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DETECTDEVICE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub DetectProgressNotify: PDETECT_PROGRESS_NOTIFY,
    pub ProgressNotifyParam: *mut ::core::ffi::c_void,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DETECTDEVICE_PARAMS {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DETECTDEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DETECTDEVICE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub DetectProgressNotify: PDETECT_PROGRESS_NOTIFY,
    pub ProgressNotifyParam: *mut ::core::ffi::c_void,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DETECTDEVICE_PARAMS {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DETECTDEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DEVICE_INTERFACE_DATA {
    pub cbSize: u32,
    pub InterfaceClassGuid: ::windows_sys::core::GUID,
    pub Flags: u32,
    pub Reserved: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DEVICE_INTERFACE_DATA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DEVICE_INTERFACE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DEVICE_INTERFACE_DATA {
    pub cbSize: u32,
    pub InterfaceClassGuid: ::windows_sys::core::GUID,
    pub Flags: u32,
    pub Reserved: usize,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DEVICE_INTERFACE_DATA {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DEVICE_INTERFACE_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DEVICE_INTERFACE_DETAIL_DATA_A {
    pub cbSize: u32,
    pub DevicePath: [u8; 1],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DEVICE_INTERFACE_DETAIL_DATA_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DEVICE_INTERFACE_DETAIL_DATA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DEVICE_INTERFACE_DETAIL_DATA_A {
    pub cbSize: u32,
    pub DevicePath: [u8; 1],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DEVICE_INTERFACE_DETAIL_DATA_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DEVICE_INTERFACE_DETAIL_DATA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DEVICE_INTERFACE_DETAIL_DATA_W {
    pub cbSize: u32,
    pub DevicePath: [u16; 1],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DEVICE_INTERFACE_DETAIL_DATA_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DEVICE_INTERFACE_DETAIL_DATA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DEVICE_INTERFACE_DETAIL_DATA_W {
    pub cbSize: u32,
    pub DevicePath: [u16; 1],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DEVICE_INTERFACE_DETAIL_DATA_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DEVICE_INTERFACE_DETAIL_DATA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DEVINFO_DATA {
    pub cbSize: u32,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub DevInst: u32,
    pub Reserved: usize,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DEVINFO_DATA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DEVINFO_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DEVINFO_DATA {
    pub cbSize: u32,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub DevInst: u32,
    pub Reserved: usize,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DEVINFO_DATA {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DEVINFO_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINFO_LIST_DETAIL_DATA_A {
    pub cbSize: u32,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub RemoteMachineHandle: super::super::Foundation::HANDLE,
    pub RemoteMachineName: [u8; 263],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINFO_LIST_DETAIL_DATA_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINFO_LIST_DETAIL_DATA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINFO_LIST_DETAIL_DATA_A {
    pub cbSize: u32,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub RemoteMachineHandle: super::super::Foundation::HANDLE,
    pub RemoteMachineName: [u8; 263],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINFO_LIST_DETAIL_DATA_A {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINFO_LIST_DETAIL_DATA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINFO_LIST_DETAIL_DATA_W {
    pub cbSize: u32,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub RemoteMachineHandle: super::super::Foundation::HANDLE,
    pub RemoteMachineName: [u16; 263],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINFO_LIST_DETAIL_DATA_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINFO_LIST_DETAIL_DATA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINFO_LIST_DETAIL_DATA_W {
    pub cbSize: u32,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub RemoteMachineHandle: super::super::Foundation::HANDLE,
    pub RemoteMachineName: [u16; 263],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINFO_LIST_DETAIL_DATA_W {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINFO_LIST_DETAIL_DATA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINSTALL_PARAMS_A {
    pub cbSize: u32,
    pub Flags: u32,
    pub FlagsEx: u32,
    pub hwndParent: super::super::Foundation::HWND,
    pub InstallMsgHandler: PSP_FILE_CALLBACK_A,
    pub InstallMsgHandlerContext: *mut ::core::ffi::c_void,
    pub FileQueue: *mut ::core::ffi::c_void,
    pub ClassInstallReserved: usize,
    pub Reserved: u32,
    pub DriverPath: [u8; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINSTALL_PARAMS_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINSTALL_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINSTALL_PARAMS_A {
    pub cbSize: u32,
    pub Flags: u32,
    pub FlagsEx: u32,
    pub hwndParent: super::super::Foundation::HWND,
    pub InstallMsgHandler: PSP_FILE_CALLBACK_A,
    pub InstallMsgHandlerContext: *mut ::core::ffi::c_void,
    pub FileQueue: *mut ::core::ffi::c_void,
    pub ClassInstallReserved: usize,
    pub Reserved: u32,
    pub DriverPath: [u8; 260],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINSTALL_PARAMS_A {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINSTALL_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINSTALL_PARAMS_W {
    pub cbSize: u32,
    pub Flags: u32,
    pub FlagsEx: u32,
    pub hwndParent: super::super::Foundation::HWND,
    pub InstallMsgHandler: PSP_FILE_CALLBACK_W,
    pub InstallMsgHandlerContext: *mut ::core::ffi::c_void,
    pub FileQueue: *mut ::core::ffi::c_void,
    pub ClassInstallReserved: usize,
    pub Reserved: u32,
    pub DriverPath: [u16; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINSTALL_PARAMS_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINSTALL_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DEVINSTALL_PARAMS_W {
    pub cbSize: u32,
    pub Flags: u32,
    pub FlagsEx: u32,
    pub hwndParent: super::super::Foundation::HWND,
    pub InstallMsgHandler: PSP_FILE_CALLBACK_W,
    pub InstallMsgHandlerContext: *mut ::core::ffi::c_void,
    pub FileQueue: *mut ::core::ffi::c_void,
    pub ClassInstallReserved: usize,
    pub Reserved: u32,
    pub DriverPath: [u16; 260],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DEVINSTALL_PARAMS_W {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DEVINSTALL_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DRVINFO_DATA_V1_A {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u8; 256],
    pub MfgName: [u8; 256],
    pub ProviderName: [u8; 256],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V1_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DRVINFO_DATA_V1_A {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u8; 256],
    pub MfgName: [u8; 256],
    pub ProviderName: [u8; 256],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V1_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DRVINFO_DATA_V1_W {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u16; 256],
    pub MfgName: [u16; 256],
    pub ProviderName: [u16; 256],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V1_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DRVINFO_DATA_V1_W {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u16; 256],
    pub MfgName: [u16; 256],
    pub ProviderName: [u16; 256],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V1_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DATA_V2_A {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u8; 256],
    pub MfgName: [u8; 256],
    pub ProviderName: [u8; 256],
    pub DriverDate: super::super::Foundation::FILETIME,
    pub DriverVersion: u64,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V2_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DATA_V2_A {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u8; 256],
    pub MfgName: [u8; 256],
    pub ProviderName: [u8; 256],
    pub DriverDate: super::super::Foundation::FILETIME,
    pub DriverVersion: u64,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V2_A {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DATA_V2_W {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u16; 256],
    pub MfgName: [u16; 256],
    pub ProviderName: [u16; 256],
    pub DriverDate: super::super::Foundation::FILETIME,
    pub DriverVersion: u64,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V2_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DATA_V2_W {
    pub cbSize: u32,
    pub DriverType: u32,
    pub Reserved: usize,
    pub Description: [u16; 256],
    pub MfgName: [u16; 256],
    pub ProviderName: [u16; 256],
    pub DriverDate: super::super::Foundation::FILETIME,
    pub DriverVersion: u64,
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DATA_V2_W {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DATA_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DETAIL_DATA_A {
    pub cbSize: u32,
    pub InfDate: super::super::Foundation::FILETIME,
    pub CompatIDsOffset: u32,
    pub CompatIDsLength: u32,
    pub Reserved: usize,
    pub SectionName: [u8; 256],
    pub InfFileName: [u8; 260],
    pub DrvDescription: [u8; 256],
    pub HardwareID: [u8; 1],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DETAIL_DATA_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DETAIL_DATA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DETAIL_DATA_A {
    pub cbSize: u32,
    pub InfDate: super::super::Foundation::FILETIME,
    pub CompatIDsOffset: u32,
    pub CompatIDsLength: u32,
    pub Reserved: usize,
    pub SectionName: [u8; 256],
    pub InfFileName: [u8; 260],
    pub DrvDescription: [u8; 256],
    pub HardwareID: [u8; 1],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DETAIL_DATA_A {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DETAIL_DATA_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DETAIL_DATA_W {
    pub cbSize: u32,
    pub InfDate: super::super::Foundation::FILETIME,
    pub CompatIDsOffset: u32,
    pub CompatIDsLength: u32,
    pub Reserved: usize,
    pub SectionName: [u16; 256],
    pub InfFileName: [u16; 260],
    pub DrvDescription: [u16; 256],
    pub HardwareID: [u16; 1],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DETAIL_DATA_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DETAIL_DATA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
pub struct SP_DRVINFO_DETAIL_DATA_W {
    pub cbSize: u32,
    pub InfDate: super::super::Foundation::FILETIME,
    pub CompatIDsOffset: u32,
    pub CompatIDsLength: u32,
    pub Reserved: usize,
    pub SectionName: [u16; 256],
    pub InfFileName: [u16; 260],
    pub DrvDescription: [u16; 256],
    pub HardwareID: [u16; 1],
}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SP_DRVINFO_DETAIL_DATA_W {}
#[cfg(target_arch = "x86")]
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SP_DRVINFO_DETAIL_DATA_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_DRVINSTALL_PARAMS {
    pub cbSize: u32,
    pub Rank: u32,
    pub Flags: u32,
    pub PrivateData: usize,
    pub Reserved: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_DRVINSTALL_PARAMS {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_DRVINSTALL_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_DRVINSTALL_PARAMS {
    pub cbSize: u32,
    pub Rank: u32,
    pub Flags: u32,
    pub PrivateData: usize,
    pub Reserved: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_DRVINSTALL_PARAMS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_DRVINSTALL_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_ENABLECLASS_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub EnableMessage: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_ENABLECLASS_PARAMS {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_ENABLECLASS_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_ENABLECLASS_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub ClassGuid: ::windows_sys::core::GUID,
    pub EnableMessage: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_ENABLECLASS_PARAMS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_ENABLECLASS_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_FILE_COPY_PARAMS_A {
    pub cbSize: u32,
    pub QueueHandle: *mut ::core::ffi::c_void,
    pub SourceRootPath: ::windows_sys::core::PCSTR,
    pub SourcePath: ::windows_sys::core::PCSTR,
    pub SourceFilename: ::windows_sys::core::PCSTR,
    pub SourceDescription: ::windows_sys::core::PCSTR,
    pub SourceTagfile: ::windows_sys::core::PCSTR,
    pub TargetDirectory: ::windows_sys::core::PCSTR,
    pub TargetFilename: ::windows_sys::core::PCSTR,
    pub CopyStyle: u32,
    pub LayoutInf: *mut ::core::ffi::c_void,
    pub SecurityDescriptor: ::windows_sys::core::PCSTR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_FILE_COPY_PARAMS_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_FILE_COPY_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_FILE_COPY_PARAMS_A {
    pub cbSize: u32,
    pub QueueHandle: *mut ::core::ffi::c_void,
    pub SourceRootPath: ::windows_sys::core::PCSTR,
    pub SourcePath: ::windows_sys::core::PCSTR,
    pub SourceFilename: ::windows_sys::core::PCSTR,
    pub SourceDescription: ::windows_sys::core::PCSTR,
    pub SourceTagfile: ::windows_sys::core::PCSTR,
    pub TargetDirectory: ::windows_sys::core::PCSTR,
    pub TargetFilename: ::windows_sys::core::PCSTR,
    pub CopyStyle: u32,
    pub LayoutInf: *mut ::core::ffi::c_void,
    pub SecurityDescriptor: ::windows_sys::core::PCSTR,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_FILE_COPY_PARAMS_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_FILE_COPY_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_FILE_COPY_PARAMS_W {
    pub cbSize: u32,
    pub QueueHandle: *mut ::core::ffi::c_void,
    pub SourceRootPath: ::windows_sys::core::PCWSTR,
    pub SourcePath: ::windows_sys::core::PCWSTR,
    pub SourceFilename: ::windows_sys::core::PCWSTR,
    pub SourceDescription: ::windows_sys::core::PCWSTR,
    pub SourceTagfile: ::windows_sys::core::PCWSTR,
    pub TargetDirectory: ::windows_sys::core::PCWSTR,
    pub TargetFilename: ::windows_sys::core::PCWSTR,
    pub CopyStyle: u32,
    pub LayoutInf: *mut ::core::ffi::c_void,
    pub SecurityDescriptor: ::windows_sys::core::PCWSTR,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_FILE_COPY_PARAMS_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_FILE_COPY_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_FILE_COPY_PARAMS_W {
    pub cbSize: u32,
    pub QueueHandle: *mut ::core::ffi::c_void,
    pub SourceRootPath: ::windows_sys::core::PCWSTR,
    pub SourcePath: ::windows_sys::core::PCWSTR,
    pub SourceFilename: ::windows_sys::core::PCWSTR,
    pub SourceDescription: ::windows_sys::core::PCWSTR,
    pub SourceTagfile: ::windows_sys::core::PCWSTR,
    pub TargetDirectory: ::windows_sys::core::PCWSTR,
    pub TargetFilename: ::windows_sys::core::PCWSTR,
    pub CopyStyle: u32,
    pub LayoutInf: *mut ::core::ffi::c_void,
    pub SecurityDescriptor: ::windows_sys::core::PCWSTR,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_FILE_COPY_PARAMS_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_FILE_COPY_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_INF_INFORMATION {
    pub InfStyle: INF_STYLE,
    pub InfCount: u32,
    pub VersionData: [u8; 1],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_INF_INFORMATION {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_INF_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_INF_INFORMATION {
    pub InfStyle: INF_STYLE,
    pub InfCount: u32,
    pub VersionData: [u8; 1],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_INF_INFORMATION {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_INF_INFORMATION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_INF_SIGNER_INFO_V1_A {
    pub cbSize: u32,
    pub CatalogFile: [u8; 260],
    pub DigitalSigner: [u8; 260],
    pub DigitalSignerVersion: [u8; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V1_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_INF_SIGNER_INFO_V1_A {
    pub cbSize: u32,
    pub CatalogFile: [u8; 260],
    pub DigitalSigner: [u8; 260],
    pub DigitalSignerVersion: [u8; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V1_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V1_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_INF_SIGNER_INFO_V1_W {
    pub cbSize: u32,
    pub CatalogFile: [u16; 260],
    pub DigitalSigner: [u16; 260],
    pub DigitalSignerVersion: [u16; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V1_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_INF_SIGNER_INFO_V1_W {
    pub cbSize: u32,
    pub CatalogFile: [u16; 260],
    pub DigitalSigner: [u16; 260],
    pub DigitalSignerVersion: [u16; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V1_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V1_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_INF_SIGNER_INFO_V2_A {
    pub cbSize: u32,
    pub CatalogFile: [u8; 260],
    pub DigitalSigner: [u8; 260],
    pub DigitalSignerVersion: [u8; 260],
    pub SignerScore: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V2_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_INF_SIGNER_INFO_V2_A {
    pub cbSize: u32,
    pub CatalogFile: [u8; 260],
    pub DigitalSigner: [u8; 260],
    pub DigitalSignerVersion: [u8; 260],
    pub SignerScore: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V2_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V2_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_INF_SIGNER_INFO_V2_W {
    pub cbSize: u32,
    pub CatalogFile: [u16; 260],
    pub DigitalSigner: [u16; 260],
    pub DigitalSignerVersion: [u16; 260],
    pub SignerScore: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V2_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_INF_SIGNER_INFO_V2_W {
    pub cbSize: u32,
    pub CatalogFile: [u16; 260],
    pub DigitalSigner: [u16; 260],
    pub DigitalSignerVersion: [u16; 260],
    pub SignerScore: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_INF_SIGNER_INFO_V2_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_INF_SIGNER_INFO_V2_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub struct SP_INSTALLWIZARD_DATA {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Flags: u32,
    pub DynamicPages: [super::super::UI::Controls::HPROPSHEETPAGE; 20],
    pub NumDynamicPages: u32,
    pub DynamicPageFlags: u32,
    pub PrivateFlags: u32,
    pub PrivateData: super::super::Foundation::LPARAM,
    pub hwndWizardDlg: super::super::Foundation::HWND,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::marker::Copy for SP_INSTALLWIZARD_DATA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::clone::Clone for SP_INSTALLWIZARD_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub struct SP_INSTALLWIZARD_DATA {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Flags: u32,
    pub DynamicPages: [super::super::UI::Controls::HPROPSHEETPAGE; 20],
    pub NumDynamicPages: u32,
    pub DynamicPageFlags: u32,
    pub PrivateFlags: u32,
    pub PrivateData: super::super::Foundation::LPARAM,
    pub hwndWizardDlg: super::super::Foundation::HWND,
}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::marker::Copy for SP_INSTALLWIZARD_DATA {}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::clone::Clone for SP_INSTALLWIZARD_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub struct SP_NEWDEVICEWIZARD_DATA {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Flags: u32,
    pub DynamicPages: [super::super::UI::Controls::HPROPSHEETPAGE; 20],
    pub NumDynamicPages: u32,
    pub hwndWizardDlg: super::super::Foundation::HWND,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::marker::Copy for SP_NEWDEVICEWIZARD_DATA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::clone::Clone for SP_NEWDEVICEWIZARD_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_UI_Controls\"`"]
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
pub struct SP_NEWDEVICEWIZARD_DATA {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Flags: u32,
    pub DynamicPages: [super::super::UI::Controls::HPROPSHEETPAGE; 20],
    pub NumDynamicPages: u32,
    pub hwndWizardDlg: super::super::Foundation::HWND,
}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::marker::Copy for SP_NEWDEVICEWIZARD_DATA {}
#[cfg(target_arch = "x86")]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_UI_Controls"))]
impl ::core::clone::Clone for SP_NEWDEVICEWIZARD_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_ORIGINAL_FILE_INFO_A {
    pub cbSize: u32,
    pub OriginalInfName: [u8; 260],
    pub OriginalCatalogName: [u8; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_ORIGINAL_FILE_INFO_A {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_ORIGINAL_FILE_INFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_ORIGINAL_FILE_INFO_A {
    pub cbSize: u32,
    pub OriginalInfName: [u8; 260],
    pub OriginalCatalogName: [u8; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_ORIGINAL_FILE_INFO_A {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_ORIGINAL_FILE_INFO_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_ORIGINAL_FILE_INFO_W {
    pub cbSize: u32,
    pub OriginalInfName: [u16; 260],
    pub OriginalCatalogName: [u16; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_ORIGINAL_FILE_INFO_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_ORIGINAL_FILE_INFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_ORIGINAL_FILE_INFO_W {
    pub cbSize: u32,
    pub OriginalInfName: [u16; 260],
    pub OriginalCatalogName: [u16; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_ORIGINAL_FILE_INFO_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_ORIGINAL_FILE_INFO_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SP_POWERMESSAGEWAKE_PARAMS_A {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub PowerMessageWake: [u8; 512],
}
impl ::core::marker::Copy for SP_POWERMESSAGEWAKE_PARAMS_A {}
impl ::core::clone::Clone for SP_POWERMESSAGEWAKE_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_POWERMESSAGEWAKE_PARAMS_W {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub PowerMessageWake: [u16; 512],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_POWERMESSAGEWAKE_PARAMS_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_POWERMESSAGEWAKE_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_POWERMESSAGEWAKE_PARAMS_W {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub PowerMessageWake: [u16; 512],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_POWERMESSAGEWAKE_PARAMS_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_POWERMESSAGEWAKE_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_PROPCHANGE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub StateChange: u32,
    pub Scope: u32,
    pub HwProfile: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_PROPCHANGE_PARAMS {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_PROPCHANGE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_PROPCHANGE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub StateChange: u32,
    pub Scope: u32,
    pub HwProfile: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_PROPCHANGE_PARAMS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_PROPCHANGE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_PROPSHEETPAGE_REQUEST {
    pub cbSize: u32,
    pub PageRequested: u32,
    pub DeviceInfoSet: HDEVINFO,
    pub DeviceInfoData: *mut SP_DEVINFO_DATA,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_PROPSHEETPAGE_REQUEST {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_PROPSHEETPAGE_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_PROPSHEETPAGE_REQUEST {
    pub cbSize: u32,
    pub PageRequested: u32,
    pub DeviceInfoSet: HDEVINFO,
    pub DeviceInfoData: *mut SP_DEVINFO_DATA,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_PROPSHEETPAGE_REQUEST {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_PROPSHEETPAGE_REQUEST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_REGISTER_CONTROL_STATUSA {
    pub cbSize: u32,
    pub FileName: ::windows_sys::core::PCSTR,
    pub Win32Error: u32,
    pub FailureCode: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_REGISTER_CONTROL_STATUSA {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_REGISTER_CONTROL_STATUSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_REGISTER_CONTROL_STATUSA {
    pub cbSize: u32,
    pub FileName: ::windows_sys::core::PCSTR,
    pub Win32Error: u32,
    pub FailureCode: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_REGISTER_CONTROL_STATUSA {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_REGISTER_CONTROL_STATUSA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_REGISTER_CONTROL_STATUSW {
    pub cbSize: u32,
    pub FileName: ::windows_sys::core::PCWSTR,
    pub Win32Error: u32,
    pub FailureCode: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_REGISTER_CONTROL_STATUSW {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_REGISTER_CONTROL_STATUSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_REGISTER_CONTROL_STATUSW {
    pub cbSize: u32,
    pub FileName: ::windows_sys::core::PCWSTR,
    pub Win32Error: u32,
    pub FailureCode: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_REGISTER_CONTROL_STATUSW {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_REGISTER_CONTROL_STATUSW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_REMOVEDEVICE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Scope: u32,
    pub HwProfile: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_REMOVEDEVICE_PARAMS {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_REMOVEDEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_REMOVEDEVICE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Scope: u32,
    pub HwProfile: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_REMOVEDEVICE_PARAMS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_REMOVEDEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SP_SELECTDEVICE_PARAMS_A {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Title: [u8; 60],
    pub Instructions: [u8; 256],
    pub ListLabel: [u8; 30],
    pub SubTitle: [u8; 256],
    pub Reserved: [u8; 2],
}
impl ::core::marker::Copy for SP_SELECTDEVICE_PARAMS_A {}
impl ::core::clone::Clone for SP_SELECTDEVICE_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_SELECTDEVICE_PARAMS_W {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Title: [u16; 60],
    pub Instructions: [u16; 256],
    pub ListLabel: [u16; 30],
    pub SubTitle: [u16; 256],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_SELECTDEVICE_PARAMS_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_SELECTDEVICE_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_SELECTDEVICE_PARAMS_W {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Title: [u16; 60],
    pub Instructions: [u16; 256],
    pub ListLabel: [u16; 30],
    pub SubTitle: [u16; 256],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_SELECTDEVICE_PARAMS_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_SELECTDEVICE_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SP_TROUBLESHOOTER_PARAMS_A {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub ChmFile: [u8; 260],
    pub HtmlTroubleShooter: [u8; 260],
}
impl ::core::marker::Copy for SP_TROUBLESHOOTER_PARAMS_A {}
impl ::core::clone::Clone for SP_TROUBLESHOOTER_PARAMS_A {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_TROUBLESHOOTER_PARAMS_W {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub ChmFile: [u16; 260],
    pub HtmlTroubleShooter: [u16; 260],
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_TROUBLESHOOTER_PARAMS_W {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_TROUBLESHOOTER_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_TROUBLESHOOTER_PARAMS_W {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub ChmFile: [u16; 260],
    pub HtmlTroubleShooter: [u16; 260],
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_TROUBLESHOOTER_PARAMS_W {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_TROUBLESHOOTER_PARAMS_W {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct SP_UNREMOVEDEVICE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Scope: u32,
    pub HwProfile: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for SP_UNREMOVEDEVICE_PARAMS {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for SP_UNREMOVEDEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(1))]
#[cfg(target_arch = "x86")]
pub struct SP_UNREMOVEDEVICE_PARAMS {
    pub ClassInstallHeader: SP_CLASSINSTALL_HEADER,
    pub Scope: u32,
    pub HwProfile: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for SP_UNREMOVEDEVICE_PARAMS {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for SP_UNREMOVEDEVICE_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
pub type PCM_NOTIFY_CALLBACK = ::core::option::Option<unsafe extern "system" fn(hnotify: HCMNOTIFICATION, context: *const ::core::ffi::c_void, action: CM_NOTIFY_ACTION, eventdata: *const CM_NOTIFY_EVENT_DATA, eventdatasize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PDETECT_PROGRESS_NOTIFY = ::core::option::Option<unsafe extern "system" fn(progressnotifyparam: *const ::core::ffi::c_void, detectcomplete: u32) -> super::super::Foundation::BOOL>;
pub type PSP_DETSIG_CMPPROC = ::core::option::Option<unsafe extern "system" fn(deviceinfoset: HDEVINFO, newdevicedata: *const SP_DEVINFO_DATA, existingdevicedata: *const SP_DEVINFO_DATA, comparecontext: *const ::core::ffi::c_void) -> u32>;
pub type PSP_FILE_CALLBACK_A = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, notification: u32, param1: usize, param2: usize) -> u32>;
pub type PSP_FILE_CALLBACK_W = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, notification: u32, param1: usize, param2: usize) -> u32>;
