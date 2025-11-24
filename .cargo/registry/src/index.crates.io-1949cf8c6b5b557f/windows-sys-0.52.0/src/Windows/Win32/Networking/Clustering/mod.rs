::windows_targets::link!("clusapi.dll" "system" fn AddClusterGroupDependency(hdependentgroup : HGROUP, hprovidergroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterGroupDependencyEx(hdependentgroup : HGROUP, hprovidergroup : HGROUP, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterGroupSetDependency(hdependentgroupset : HGROUPSET, hprovidergroupset : HGROUPSET) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterGroupSetDependencyEx(hdependentgroupset : HGROUPSET, hprovidergroupset : HGROUPSET, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterGroupToGroupSetDependency(hdependentgroup : HGROUP, hprovidergroupset : HGROUPSET) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterGroupToGroupSetDependencyEx(hdependentgroup : HGROUP, hprovidergroupset : HGROUPSET, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AddClusterNode(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR, pfnprogresscallback : PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void) -> HNODE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AddClusterNodeEx(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR, dwflags : u32, pfnprogresscallback : PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void) -> HNODE);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterResourceDependency(hresource : HRESOURCE, hdependson : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterResourceDependencyEx(hresource : HRESOURCE, hdependson : HRESOURCE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterResourceNode(hresource : HRESOURCE, hnode : HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddClusterResourceNodeEx(hresource : HRESOURCE, hnode : HNODE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn AddClusterStorageNode(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR, pfnprogresscallback : PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void, lpszclusterstoragenodedescription : ::windows_sys::core::PCWSTR, lpszclusterstoragenodelocation : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddCrossClusterGroupSetDependency(hdependentgroupset : HGROUPSET, lpremoteclustername : ::windows_sys::core::PCWSTR, lpremotegroupsetname : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn AddResourceToClusterSharedVolumes(hresource : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn BackupClusterDatabase(hcluster : HCLUSTER, lpszpathname : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CanResourceBeDependent(hresource : HRESOURCE, hresourcedependent : HRESOURCE) -> super::super::Foundation:: BOOL);
::windows_targets::link!("clusapi.dll" "system" fn CancelClusterGroupOperation(hgroup : HGROUP, dwcancelflags_reserved : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ChangeClusterResourceGroup(hresource : HRESOURCE, hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ChangeClusterResourceGroupEx(hresource : HRESOURCE, hgroup : HGROUP, flags : u64) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ChangeClusterResourceGroupEx2(hresource : HRESOURCE, hgroup : HGROUP, flags : u64, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseCluster(hcluster : HCLUSTER) -> super::super::Foundation:: BOOL);
::windows_targets::link!("resutils.dll" "system" fn CloseClusterCryptProvider(hcluscryptprovider : HCLUSCRYPTPROVIDER) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterGroup(hgroup : HGROUP) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterGroupSet(hgroupset : HGROUPSET) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterNetInterface(hnetinterface : HNETINTERFACE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterNetwork(hnetwork : HNETWORK) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterNode(hnode : HNODE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterNotifyPort(hchange : HCHANGE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CloseClusterResource(hresource : HRESOURCE) -> super::super::Foundation:: BOOL);
::windows_targets::link!("resutils.dll" "system" fn ClusAddClusterHealthFault(hcluster : HCLUSTER, failure : *const CLUSTER_HEALTH_FAULT, param2 : u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ClusGetClusterHealthFaults(hcluster : HCLUSTER, objects : *mut CLUSTER_HEALTH_FAULT_ARRAY, flags : u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ClusRemoveClusterHealthFault(hcluster : HCLUSTER, id : ::windows_sys::core::PCWSTR, flags : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusWorkerCheckTerminate(lpworker : *mut CLUS_WORKER) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusWorkerCreate(lpworker : *mut CLUS_WORKER, lpstartaddress : PWORKER_START_ROUTINE, lpparameter : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusWorkerTerminate(lpworker : *const CLUS_WORKER) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusWorkerTerminateEx(clusworker : *mut CLUS_WORKER, timeoutinmilliseconds : u32, waitonly : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusWorkersTerminate(clusworkers : *mut *mut CLUS_WORKER, clusworkerscount : usize, timeoutinmilliseconds : u32, waitonly : super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusapiSetReasonHandler(lphandler : *const CLUSAPI_REASON_HANDLER) -> *mut CLUSAPI_REASON_HANDLER);
::windows_targets::link!("clusapi.dll" "system" fn ClusterAddGroupToAffinityRule(hcluster : HCLUSTER, rulename : ::windows_sys::core::PCWSTR, hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterAddGroupToGroupSet(hgroupset : HGROUPSET, hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterAddGroupToGroupSetWithDomains(hgroupset : HGROUPSET, hgroup : HGROUP, faultdomain : u32, updatedomain : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterAddGroupToGroupSetWithDomainsEx(hgroupset : HGROUPSET, hgroup : HGROUP, faultdomain : u32, updatedomain : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterAffinityRuleControl(hcluster : HCLUSTER, affinityrulename : ::windows_sys::core::PCWSTR, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ClusterClearBackupStateForSharedVolume(lpszvolumepathname : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterCloseEnum(henum : HCLUSENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterCloseEnumEx(hclusterenum : HCLUSENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterControl(hcluster : HCLUSTER, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterControlEx(hcluster : HCLUSTER, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterCreateAffinityRule(hcluster : HCLUSTER, rulename : ::windows_sys::core::PCWSTR, ruletype : CLUS_AFFINITY_RULE_TYPE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ClusterDecrypt(hcluscryptprovider : HCLUSCRYPTPROVIDER, pcryptinput : *const u8, cbcryptinput : u32, ppcryptoutput : *mut *mut u8, pcbcryptoutput : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ClusterEncrypt(hcluscryptprovider : HCLUSCRYPTPROVIDER, pdata : *const u8, cbdata : u32, ppdata : *mut *mut u8, pcbdata : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterEnum(henum : HCLUSENUM, dwindex : u32, lpdwtype : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterEnumEx(hclusterenum : HCLUSENUMEX, dwindex : u32, pitem : *mut CLUSTER_ENUM_ITEM, cbitem : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGetEnumCount(henum : HCLUSENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGetEnumCountEx(hclusterenum : HCLUSENUMEX) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusterGetVolumeNameForVolumeMountPoint(lpszvolumemountpoint : ::windows_sys::core::PCWSTR, lpszvolumename : ::windows_sys::core::PWSTR, cchbufferlength : u32) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusterGetVolumePathName(lpszfilename : ::windows_sys::core::PCWSTR, lpszvolumepathname : ::windows_sys::core::PWSTR, cchbufferlength : u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupCloseEnum(hgroupenum : HGROUPENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupCloseEnumEx(hgroupenumex : HGROUPENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupControl(hgroup : HGROUP, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupControlEx(hgroup : HGROUP, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupEnum(hgroupenum : HGROUPENUM, dwindex : u32, lpdwtype : *mut u32, lpszresourcename : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupEnumEx(hgroupenumex : HGROUPENUMEX, dwindex : u32, pitem : *mut CLUSTER_GROUP_ENUM_ITEM, cbitem : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupGetEnumCount(hgroupenum : HGROUPENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupGetEnumCountEx(hgroupenumex : HGROUPENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupOpenEnum(hgroup : HGROUP, dwtype : u32) -> HGROUPENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupOpenEnumEx(hcluster : HCLUSTER, lpszproperties : ::windows_sys::core::PCWSTR, cbproperties : u32, lpszroproperties : ::windows_sys::core::PCWSTR, cbroproperties : u32, dwflags : u32) -> HGROUPENUMEX);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupSetCloseEnum(hgroupsetenum : HGROUPSETENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupSetControl(hgroupset : HGROUPSET, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupSetControlEx(hgroupset : HGROUPSET, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupSetEnum(hgroupsetenum : HGROUPSETENUM, dwindex : u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupSetGetEnumCount(hgroupsetenum : HGROUPSETENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterGroupSetOpenEnum(hcluster : HCLUSTER) -> HGROUPSETENUM);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusterIsPathOnSharedVolume(lpszpathname : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetInterfaceCloseEnum(hnetinterfaceenum : HNETINTERFACEENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetInterfaceControl(hnetinterface : HNETINTERFACE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetInterfaceControlEx(hnetinterface : HNETINTERFACE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetInterfaceEnum(hnetinterfaceenum : HNETINTERFACEENUM, dwindex : u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetInterfaceOpenEnum(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR, lpsznetworkname : ::windows_sys::core::PCWSTR) -> HNETINTERFACEENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetworkCloseEnum(hnetworkenum : HNETWORKENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetworkControl(hnetwork : HNETWORK, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetworkControlEx(hnetwork : HNETWORK, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetworkEnum(hnetworkenum : HNETWORKENUM, dwindex : u32, lpdwtype : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetworkGetEnumCount(hnetworkenum : HNETWORKENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNetworkOpenEnum(hnetwork : HNETWORK, dwtype : u32) -> HNETWORKENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeCloseEnum(hnodeenum : HNODEENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeCloseEnumEx(hnodeenum : HNODEENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeControl(hnode : HNODE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeControlEx(hnode : HNODE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeEnum(hnodeenum : HNODEENUM, dwindex : u32, lpdwtype : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeEnumEx(hnodeenum : HNODEENUMEX, dwindex : u32, pitem : *mut CLUSTER_ENUM_ITEM, cbitem : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeGetEnumCount(hnodeenum : HNODEENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeGetEnumCountEx(hnodeenum : HNODEENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeOpenEnum(hnode : HNODE, dwtype : u32) -> HNODEENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeOpenEnumEx(hnode : HNODE, dwtype : u32, poptions : *const ::core::ffi::c_void) -> HNODEENUMEX);
::windows_targets::link!("clusapi.dll" "system" fn ClusterNodeReplacement(hcluster : HCLUSTER, lpsznodenamecurrent : ::windows_sys::core::PCWSTR, lpsznodenamenew : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterOpenEnum(hcluster : HCLUSTER, dwtype : u32) -> HCLUSENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterOpenEnumEx(hcluster : HCLUSTER, dwtype : u32, poptions : *const ::core::ffi::c_void) -> HCLUSENUMEX);
::windows_targets::link!("resutils.dll" "system" fn ClusterPrepareSharedVolumeForBackup(lpszfilename : ::windows_sys::core::PCWSTR, lpszvolumepathname : ::windows_sys::core::PWSTR, lpcchvolumepathname : *mut u32, lpszvolumename : ::windows_sys::core::PWSTR, lpcchvolumename : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegBatchAddCommand(hregbatch : HREGBATCH, dwcommand : CLUSTER_REG_COMMAND, wzname : ::windows_sys::core::PCWSTR, dwoptions : u32, lpdata : *const ::core::ffi::c_void, cbdata : u32) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegBatchCloseNotification(hbatchnotification : HREGBATCHNOTIFICATION) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegBatchReadCommand(hbatchnotification : HREGBATCHNOTIFICATION, pbatchcommand : *mut CLUSTER_BATCH_COMMAND) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusterRegCloseBatch(hregbatch : HREGBATCH, bcommit : super::super::Foundation:: BOOL, failedcommandnumber : *mut i32) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegCloseBatchEx(hregbatch : HREGBATCH, flags : u32, failedcommandnumber : *mut i32) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegCloseBatchNotifyPort(hbatchnotifyport : HREGBATCHPORT) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegCloseKey(hkey : super::super::System::Registry:: HKEY) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegCloseReadBatch(hregreadbatch : HREGREADBATCH, phregreadbatchreply : *mut HREGREADBATCHREPLY) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegCloseReadBatchEx(hregreadbatch : HREGREADBATCH, flags : u32, phregreadbatchreply : *mut HREGREADBATCHREPLY) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegCloseReadBatchReply(hregreadbatchreply : HREGREADBATCHREPLY) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegCreateBatch(hkey : super::super::System::Registry:: HKEY, phregbatch : *mut HREGBATCH) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegCreateBatchNotifyPort(hkey : super::super::System::Registry:: HKEY, phbatchnotifyport : *mut HREGBATCHPORT) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Registry\"`"] fn ClusterRegCreateKey(hkey : super::super::System::Registry:: HKEY, lpszsubkey : ::windows_sys::core::PCWSTR, dwoptions : u32, samdesired : u32, lpsecurityattributes : *const super::super::Security:: SECURITY_ATTRIBUTES, phkresult : *mut super::super::System::Registry:: HKEY, lpdwdisposition : *mut u32) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Registry\"`"] fn ClusterRegCreateKeyEx(hkey : super::super::System::Registry:: HKEY, lpsubkey : ::windows_sys::core::PCWSTR, dwoptions : u32, samdesired : u32, lpsecurityattributes : *const super::super::Security:: SECURITY_ATTRIBUTES, phkresult : *mut super::super::System::Registry:: HKEY, lpdwdisposition : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegCreateReadBatch(hkey : super::super::System::Registry:: HKEY, phregreadbatch : *mut HREGREADBATCH) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegDeleteKey(hkey : super::super::System::Registry:: HKEY, lpszsubkey : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegDeleteKeyEx(hkey : super::super::System::Registry:: HKEY, lpsubkey : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegDeleteValue(hkey : super::super::System::Registry:: HKEY, lpszvaluename : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegDeleteValueEx(hkey : super::super::System::Registry:: HKEY, lpszvaluename : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ClusterRegEnumKey(hkey : super::super::System::Registry:: HKEY, dwindex : u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32, lpftlastwritetime : *mut super::super::Foundation:: FILETIME) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegEnumValue(hkey : super::super::System::Registry:: HKEY, dwindex : u32, lpszvaluename : ::windows_sys::core::PWSTR, lpcchvaluename : *mut u32, lpdwtype : *mut u32, lpdata : *mut u8, lpcbdata : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegGetBatchNotification(hbatchnotify : HREGBATCHPORT, phbatchnotification : *mut HREGBATCHNOTIFICATION) -> i32);
#[cfg(all(feature = "Win32_Security", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Security\"`, `\"Win32_System_Registry\"`"] fn ClusterRegGetKeySecurity(hkey : super::super::System::Registry:: HKEY, requestedinformation : u32, psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR, lpcbsecuritydescriptor : *mut u32) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegOpenKey(hkey : super::super::System::Registry:: HKEY, lpszsubkey : ::windows_sys::core::PCWSTR, samdesired : u32, phkresult : *mut super::super::System::Registry:: HKEY) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ClusterRegQueryInfoKey(hkey : super::super::System::Registry:: HKEY, lpcsubkeys : *const u32, lpcchmaxsubkeylen : *const u32, lpcvalues : *const u32, lpcchmaxvaluenamelen : *const u32, lpcbmaxvaluelen : *const u32, lpcbsecuritydescriptor : *const u32, lpftlastwritetime : *const super::super::Foundation:: FILETIME) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegQueryValue(hkey : super::super::System::Registry:: HKEY, lpszvaluename : ::windows_sys::core::PCWSTR, lpdwvaluetype : *mut u32, lpdata : *mut u8, lpcbdata : *mut u32) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegReadBatchAddCommand(hregreadbatch : HREGREADBATCH, wzsubkeyname : ::windows_sys::core::PCWSTR, wzvaluename : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegReadBatchReplyNextCommand(hregreadbatchreply : HREGREADBATCHREPLY, pbatchcommand : *mut CLUSTER_READ_BATCH_COMMAND) -> i32);
#[cfg(all(feature = "Win32_Security", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Security\"`, `\"Win32_System_Registry\"`"] fn ClusterRegSetKeySecurity(hkey : super::super::System::Registry:: HKEY, securityinformation : u32, psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR) -> i32);
#[cfg(all(feature = "Win32_Security", feature = "Win32_System_Registry"))]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Security\"`, `\"Win32_System_Registry\"`"] fn ClusterRegSetKeySecurityEx(hkey : super::super::System::Registry:: HKEY, securityinformation : super::super::Security:: OBJECT_SECURITY_INFORMATION, psecuritydescriptor : super::super::Security:: PSECURITY_DESCRIPTOR, lpszreason : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegSetValue(hkey : super::super::System::Registry:: HKEY, lpszvaluename : ::windows_sys::core::PCWSTR, dwtype : u32, lpdata : *const u8, cbdata : u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ClusterRegSetValueEx(hkey : super::super::System::Registry:: HKEY, lpszvaluename : ::windows_sys::core::PCWSTR, dwtype : u32, lpdata : *const u8, cbdata : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRegSyncDatabase(hcluster : HCLUSTER, flags : u32) -> i32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRemoveAffinityRule(hcluster : HCLUSTER, rulename : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRemoveGroupFromAffinityRule(hcluster : HCLUSTER, rulename : ::windows_sys::core::PCWSTR, hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRemoveGroupFromGroupSet(hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterRemoveGroupFromGroupSetEx(hgroup : HGROUP, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceCloseEnum(hresenum : HRESENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceCloseEnumEx(hresourceenumex : HRESENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceControl(hresource : HRESOURCE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceControlAsUser(hresource : HRESOURCE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceControlAsUserEx(hresource : HRESOURCE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceControlEx(hresource : HRESOURCE, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, cbinbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, cboutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceEnum(hresenum : HRESENUM, dwindex : u32, lpdwtype : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceEnumEx(hresourceenumex : HRESENUMEX, dwindex : u32, pitem : *mut CLUSTER_RESOURCE_ENUM_ITEM, cbitem : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceGetEnumCount(hresenum : HRESENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceGetEnumCountEx(hresourceenumex : HRESENUMEX) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceOpenEnum(hresource : HRESOURCE, dwtype : u32) -> HRESENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceOpenEnumEx(hcluster : HCLUSTER, lpszproperties : ::windows_sys::core::PCWSTR, cbproperties : u32, lpszroproperties : ::windows_sys::core::PCWSTR, cbroproperties : u32, dwflags : u32) -> HRESENUMEX);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeCloseEnum(hrestypeenum : HRESTYPEENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeControl(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeControlAsUser(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeControlAsUserEx(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeControlEx(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, hhostnode : HNODE, dwcontrolcode : u32, lpinbuffer : *const ::core::ffi::c_void, ninbuffersize : u32, lpoutbuffer : *mut ::core::ffi::c_void, noutbuffersize : u32, lpbytesreturned : *mut u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeEnum(hrestypeenum : HRESTYPEENUM, dwindex : u32, lpdwtype : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeGetEnumCount(hrestypeenum : HRESTYPEENUM) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterResourceTypeOpenEnum(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, dwtype : u32) -> HRESTYPEENUM);
::windows_targets::link!("clusapi.dll" "system" fn ClusterSetAccountAccess(hcluster : HCLUSTER, szaccountsid : ::windows_sys::core::PCWSTR, dwaccess : u32, dwcontroltype : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ClusterSharedVolumeSetSnapshotState(guidsnapshotset : ::windows_sys::core::GUID, lpszvolumename : ::windows_sys::core::PCWSTR, state : CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ClusterUpgradeFunctionalLevel(hcluster : HCLUSTER, perform : super::super::Foundation:: BOOL, pfnprogresscallback : PCLUSTER_UPGRADE_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateCluster(pconfig : *const CREATE_CLUSTER_CONFIG, pfnprogresscallback : PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void) -> HCLUSTER);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateClusterAvailabilitySet(hcluster : HCLUSTER, lpavailabilitysetname : ::windows_sys::core::PCWSTR, pavailabilitysetconfig : *const CLUSTER_AVAILABILITY_SET_CONFIG) -> HGROUPSET);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterGroup(hcluster : HCLUSTER, lpszgroupname : ::windows_sys::core::PCWSTR) -> HGROUP);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterGroupEx(hcluster : HCLUSTER, lpszgroupname : ::windows_sys::core::PCWSTR, pgroupinfo : *const CLUSTER_CREATE_GROUP_INFO) -> HGROUP);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterGroupSet(hcluster : HCLUSTER, groupsetname : ::windows_sys::core::PCWSTR) -> HGROUPSET);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CreateClusterNameAccount(hcluster : HCLUSTER, pconfig : *const CREATE_CLUSTER_NAME_ACCOUNT, pfnprogresscallback : PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterNotifyPort(hchange : HCHANGE, hcluster : HCLUSTER, dwfilter : u32, dwnotifykey : usize) -> HCHANGE);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterNotifyPortV2(hchange : HCHANGE, hcluster : HCLUSTER, filters : *const NOTIFY_FILTER_AND_TYPE, dwfiltercount : u32, dwnotifykey : usize) -> HCHANGE);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterResource(hgroup : HGROUP, lpszresourcename : ::windows_sys::core::PCWSTR, lpszresourcetype : ::windows_sys::core::PCWSTR, dwflags : u32) -> HRESOURCE);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterResourceEx(hgroup : HGROUP, lpszresourcename : ::windows_sys::core::PCWSTR, lpszresourcetype : ::windows_sys::core::PCWSTR, dwflags : u32, lpszreason : ::windows_sys::core::PCWSTR) -> HRESOURCE);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterResourceType(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, lpszdisplayname : ::windows_sys::core::PCWSTR, lpszresourcetypedll : ::windows_sys::core::PCWSTR, dwlooksalivepollinterval : u32, dwisalivepollinterval : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn CreateClusterResourceTypeEx(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR, lpszdisplayname : ::windows_sys::core::PCWSTR, lpszresourcetypedll : ::windows_sys::core::PCWSTR, dwlooksalivepollinterval : u32, dwisalivepollinterval : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterGroup(hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterGroupEx(hgroup : HGROUP, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterGroupSet(hgroupset : HGROUPSET) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterGroupSetEx(hgroupset : HGROUPSET, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterResource(hresource : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterResourceEx(hresource : HRESOURCE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterResourceType(hcluster : HCLUSTER, lpszresourcetypename : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DeleteClusterResourceTypeEx(hcluster : HCLUSTER, lpsztypename : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn DestroyCluster(hcluster : HCLUSTER, pfnprogresscallback : PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg : *const ::core::ffi::c_void, fdeletevirtualcomputerobjects : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DestroyClusterGroup(hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DestroyClusterGroupEx(hgroup : HGROUP, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DetermineCNOResTypeFromCluster(hcluster : HCLUSTER, pcnorestype : *mut CLUSTER_MGMT_POINT_RESTYPE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DetermineCNOResTypeFromNodelist(cnodes : u32, ppsznodenames : *const ::windows_sys::core::PCWSTR, pcnorestype : *mut CLUSTER_MGMT_POINT_RESTYPE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DetermineClusterCloudTypeFromCluster(hcluster : HCLUSTER, pcloudtype : *mut CLUSTER_CLOUD_TYPE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn DetermineClusterCloudTypeFromNodelist(cnodes : u32, ppsznodenames : *const ::windows_sys::core::PCWSTR, pcloudtype : *mut CLUSTER_CLOUD_TYPE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn EvictClusterNode(hnode : HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn EvictClusterNodeEx(hnode : HNODE, dwtimeout : u32, phrcleanupstatus : *mut ::windows_sys::core::HRESULT) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn EvictClusterNodeEx2(hnode : HNODE, dwtimeout : u32, phrcleanupstatus : *mut ::windows_sys::core::HRESULT, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn FailClusterResource(hresource : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn FailClusterResourceEx(hresource : HRESOURCE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("resutils.dll" "system" fn FreeClusterCrypt(pcryptinfo : *const ::core::ffi::c_void) -> u32);
::windows_targets::link!("resutils.dll" "system" fn FreeClusterHealthFault(clusterhealthfault : *mut CLUSTER_HEALTH_FAULT) -> u32);
::windows_targets::link!("resutils.dll" "system" fn FreeClusterHealthFaultArray(clusterhealthfaultarray : *mut CLUSTER_HEALTH_FAULT_ARRAY) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterFromGroup(hgroup : HGROUP) -> HCLUSTER);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterFromNetInterface(hnetinterface : HNETINTERFACE) -> HCLUSTER);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterFromNetwork(hnetwork : HNETWORK) -> HCLUSTER);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterFromNode(hnode : HNODE) -> HCLUSTER);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterFromResource(hresource : HRESOURCE) -> HCLUSTER);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterGroupKey(hgroup : HGROUP, samdesired : u32) -> super::super::System::Registry:: HKEY);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterGroupState(hgroup : HGROUP, lpsznodename : ::windows_sys::core::PWSTR, lpcchnodename : *mut u32) -> CLUSTER_GROUP_STATE);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterInformation(hcluster : HCLUSTER, lpszclustername : ::windows_sys::core::PWSTR, lpcchclustername : *mut u32, lpclusterinfo : *mut CLUSTERVERSIONINFO) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterKey(hcluster : HCLUSTER, samdesired : u32) -> super::super::System::Registry:: HKEY);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNetInterface(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR, lpsznetworkname : ::windows_sys::core::PCWSTR, lpszinterfacename : ::windows_sys::core::PWSTR, lpcchinterfacename : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterNetInterfaceKey(hnetinterface : HNETINTERFACE, samdesired : u32) -> super::super::System::Registry:: HKEY);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNetInterfaceState(hnetinterface : HNETINTERFACE) -> CLUSTER_NETINTERFACE_STATE);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNetworkId(hnetwork : HNETWORK, lpsznetworkid : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterNetworkKey(hnetwork : HNETWORK, samdesired : u32) -> super::super::System::Registry:: HKEY);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNetworkState(hnetwork : HNETWORK) -> CLUSTER_NETWORK_STATE);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNodeId(hnode : HNODE, lpsznodeid : ::windows_sys::core::PWSTR, lpcchname : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterNodeKey(hnode : HNODE, samdesired : u32) -> super::super::System::Registry:: HKEY);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNodeState(hnode : HNODE) -> CLUSTER_NODE_STATE);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNotify(hchange : HCHANGE, lpdwnotifykey : *mut usize, lpdwfiltertype : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32, dwmilliseconds : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterNotifyV2(hchange : HCHANGE, lpdwnotifykey : *mut usize, pfilterandtype : *mut NOTIFY_FILTER_AND_TYPE, buffer : *mut u8, lpbbuffersize : *mut u32, lpszobjectid : ::windows_sys::core::PWSTR, lpcchobjectid : *mut u32, lpszparentid : ::windows_sys::core::PWSTR, lpcchparentid : *mut u32, lpszname : ::windows_sys::core::PWSTR, lpcchname : *mut u32, lpsztype : ::windows_sys::core::PWSTR, lpcchtype : *mut u32, dwmilliseconds : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterQuorumResource(hcluster : HCLUSTER, lpszresourcename : ::windows_sys::core::PWSTR, lpcchresourcename : *mut u32, lpszdevicename : ::windows_sys::core::PWSTR, lpcchdevicename : *mut u32, lpdwmaxquorumlogsize : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterResourceDependencyExpression(hresource : HRESOURCE, lpszdependencyexpression : ::windows_sys::core::PWSTR, lpcchdependencyexpression : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterResourceKey(hresource : HRESOURCE, samdesired : u32) -> super::super::System::Registry:: HKEY);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetClusterResourceNetworkName(hresource : HRESOURCE, lpbuffer : ::windows_sys::core::PWSTR, nsize : *mut u32) -> super::super::Foundation:: BOOL);
::windows_targets::link!("clusapi.dll" "system" fn GetClusterResourceState(hresource : HRESOURCE, lpsznodename : ::windows_sys::core::PWSTR, lpcchnodename : *mut u32, lpszgroupname : ::windows_sys::core::PWSTR, lpcchgroupname : *mut u32) -> CLUSTER_RESOURCE_STATE);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn GetClusterResourceTypeKey(hcluster : HCLUSTER, lpsztypename : ::windows_sys::core::PCWSTR, samdesired : u32) -> super::super::System::Registry:: HKEY);
::windows_targets::link!("clusapi.dll" "system" fn GetNodeCloudTypeDW(ppsznodename : ::windows_sys::core::PCWSTR, nodecloudtype : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn GetNodeClusterState(lpsznodename : ::windows_sys::core::PCWSTR, pdwclusterstate : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNotifyEventHandle(hchange : HCHANGE, lphtargetevent : *mut super::super::Foundation:: HANDLE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn InitializeClusterHealthFault(clusterhealthfault : *mut CLUSTER_HEALTH_FAULT) -> u32);
::windows_targets::link!("resutils.dll" "system" fn InitializeClusterHealthFaultArray(clusterhealthfaultarray : *mut CLUSTER_HEALTH_FAULT_ARRAY) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsFileOnClusterSharedVolume(lpszpathname : ::windows_sys::core::PCWSTR, pbfileisonsharedvolume : *mut super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn MoveClusterGroup(hgroup : HGROUP, hdestinationnode : HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn MoveClusterGroupEx(hgroup : HGROUP, hdestinationnode : HNODE, dwmoveflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn MoveClusterGroupEx2(hgroup : HGROUP, hdestinationnode : HNODE, dwmoveflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OfflineClusterGroup(hgroup : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OfflineClusterGroupEx(hgroup : HGROUP, dwofflineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OfflineClusterGroupEx2(hgroup : HGROUP, dwofflineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OfflineClusterResource(hresource : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OfflineClusterResourceEx(hresource : HRESOURCE, dwofflineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OfflineClusterResourceEx2(hresource : HRESOURCE, dwofflineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OnlineClusterGroup(hgroup : HGROUP, hdestinationnode : HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OnlineClusterGroupEx(hgroup : HGROUP, hdestinationnode : HNODE, dwonlineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OnlineClusterGroupEx2(hgroup : HGROUP, hdestinationnode : HNODE, dwonlineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OnlineClusterResource(hresource : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OnlineClusterResourceEx(hresource : HRESOURCE, dwonlineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OnlineClusterResourceEx2(hresource : HRESOURCE, dwonlineflags : u32, lpinbuffer : *const u8, cbinbuffersize : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn OpenCluster(lpszclustername : ::windows_sys::core::PCWSTR) -> HCLUSTER);
::windows_targets::link!("resutils.dll" "system" fn OpenClusterCryptProvider(lpszresource : ::windows_sys::core::PCWSTR, lpszprovider : *const i8, dwtype : u32, dwflags : u32) -> HCLUSCRYPTPROVIDER);
::windows_targets::link!("resutils.dll" "system" fn OpenClusterCryptProviderEx(lpszresource : ::windows_sys::core::PCWSTR, lpszkeyname : ::windows_sys::core::PCWSTR, lpszprovider : *const i8, dwtype : u32, dwflags : u32) -> HCLUSCRYPTPROVIDER);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterEx(lpszclustername : ::windows_sys::core::PCWSTR, desiredaccess : u32, grantedaccess : *mut u32) -> HCLUSTER);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterGroup(hcluster : HCLUSTER, lpszgroupname : ::windows_sys::core::PCWSTR) -> HGROUP);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterGroupEx(hcluster : HCLUSTER, lpszgroupname : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32, lpdwgrantedaccess : *mut u32) -> HGROUP);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterGroupSet(hcluster : HCLUSTER, lpszgroupsetname : ::windows_sys::core::PCWSTR) -> HGROUPSET);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNetInterface(hcluster : HCLUSTER, lpszinterfacename : ::windows_sys::core::PCWSTR) -> HNETINTERFACE);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNetInterfaceEx(hcluster : HCLUSTER, lpszinterfacename : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32, lpdwgrantedaccess : *mut u32) -> HNETINTERFACE);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNetwork(hcluster : HCLUSTER, lpsznetworkname : ::windows_sys::core::PCWSTR) -> HNETWORK);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNetworkEx(hcluster : HCLUSTER, lpsznetworkname : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32, lpdwgrantedaccess : *mut u32) -> HNETWORK);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNode(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR) -> HNODE);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNodeById(hcluster : HCLUSTER, nodeid : u32) -> HNODE);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterNodeEx(hcluster : HCLUSTER, lpsznodename : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32, lpdwgrantedaccess : *mut u32) -> HNODE);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterResource(hcluster : HCLUSTER, lpszresourcename : ::windows_sys::core::PCWSTR) -> HRESOURCE);
::windows_targets::link!("clusapi.dll" "system" fn OpenClusterResourceEx(hcluster : HCLUSTER, lpszresourcename : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32, lpdwgrantedaccess : *mut u32) -> HRESOURCE);
::windows_targets::link!("clusapi.dll" "system" fn PauseClusterNode(hnode : HNODE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PauseClusterNodeEx(hnode : HNODE, bdrainnode : super::super::Foundation:: BOOL, dwpauseflags : u32, hnodedraintarget : HNODE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn PauseClusterNodeEx2(hnode : HNODE, bdrainnode : super::super::Foundation:: BOOL, dwpauseflags : u32, hnodedraintarget : HNODE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntlanman.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn QueryAppInstanceVersion(appinstanceid : *const ::windows_sys::core::GUID, instanceversionhigh : *mut u64, instanceversionlow : *mut u64, versionstatus : *mut super::super::Foundation:: NTSTATUS) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntlanman.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterAppInstance(processhandle : super::super::Foundation:: HANDLE, appinstanceid : *const ::windows_sys::core::GUID, childreninheritappinstance : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("ntlanman.dll" "system" fn RegisterAppInstanceVersion(appinstanceid : *const ::windows_sys::core::GUID, instanceversionhigh : u64, instanceversionlow : u64) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterClusterNotify(hchange : HCHANGE, dwfiltertype : u32, hobject : super::super::Foundation:: HANDLE, dwnotifykey : usize) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RegisterClusterNotifyV2(hchange : HCHANGE, filter : NOTIFY_FILTER_AND_TYPE, hobject : super::super::Foundation:: HANDLE, dwnotifykey : usize) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RegisterClusterResourceTypeNotifyV2(hchange : HCHANGE, hcluster : HCLUSTER, flags : i64, restypename : ::windows_sys::core::PCWSTR, dwnotifykey : usize) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterGroupDependency(hgroup : HGROUP, hdependson : HGROUP) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterGroupDependencyEx(hgroup : HGROUP, hdependson : HGROUP, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterGroupSetDependency(hgroupset : HGROUPSET, hdependson : HGROUPSET) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterGroupSetDependencyEx(hgroupset : HGROUPSET, hdependson : HGROUPSET, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterGroupToGroupSetDependency(hgroup : HGROUP, hdependson : HGROUPSET) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterGroupToGroupSetDependencyEx(hgroup : HGROUP, hdependson : HGROUPSET, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RemoveClusterNameAccount(hcluster : HCLUSTER, bdeletecomputerobjects : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterResourceDependency(hresource : HRESOURCE, hdependson : HRESOURCE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterResourceDependencyEx(hresource : HRESOURCE, hdependson : HRESOURCE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterResourceNode(hresource : HRESOURCE, hnode : HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterResourceNodeEx(hresource : HRESOURCE, hnode : HNODE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveClusterStorageNode(hcluster : HCLUSTER, lpszclusterstorageenclosurename : ::windows_sys::core::PCWSTR, dwtimeout : u32, dwflags : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveCrossClusterGroupSetDependency(hdependentgroupset : HGROUPSET, lpremoteclustername : ::windows_sys::core::PCWSTR, lpremotegroupsetname : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RemoveResourceFromClusterSharedVolumes(hresource : HRESOURCE) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilAddUnknownProperties(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, poutpropertylist : *mut ::core::ffi::c_void, pcboutpropertylistsize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilCreateDirectoryTree(pszpath : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilDupGroup(group : HGROUP, copy : *mut HGROUP) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilDupParameterBlock(poutparams : *mut u8, pinparams : *const u8, ppropertytable : *const RESUTIL_PROPERTY_ITEM) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilDupResource(group : HRESOURCE, copy : *mut HRESOURCE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilDupString(pszinstring : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("resutils.dll" "system" fn ResUtilEnumGroups(hcluster : HCLUSTER, hself : HGROUP, prescallback : LPGROUP_CALLBACK_EX, pparameter : *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilEnumGroupsEx(hcluster : HCLUSTER, hself : HGROUP, grouptype : CLUSGROUP_TYPE, prescallback : LPGROUP_CALLBACK_EX, pparameter : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilEnumPrivateProperties(hkeyclusterkey : super::super::System::Registry:: HKEY, pszoutproperties : ::windows_sys::core::PWSTR, cboutpropertiessize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilEnumProperties(ppropertytable : *const RESUTIL_PROPERTY_ITEM, pszoutproperties : ::windows_sys::core::PWSTR, cboutpropertiessize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilEnumResources(hself : HRESOURCE, lpszrestypename : ::windows_sys::core::PCWSTR, prescallback : LPRESOURCE_CALLBACK, pparameter : *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilEnumResourcesEx(hcluster : HCLUSTER, hself : HRESOURCE, lpszrestypename : ::windows_sys::core::PCWSTR, prescallback : LPRESOURCE_CALLBACK_EX, pparameter : *mut ::core::ffi::c_void) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilEnumResourcesEx2(hcluster : HCLUSTER, hself : HRESOURCE, lpszrestypename : ::windows_sys::core::PCWSTR, prescallback : LPRESOURCE_CALLBACK_EX, pparameter : *mut ::core::ffi::c_void, dwdesiredaccess : u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilExpandEnvironmentStrings(pszsrc : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindBinaryProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pbpropertyvalue : *mut *mut u8, pcbpropertyvaluesize : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindDependentDiskResourceDriveLetter(hcluster : HCLUSTER, hresource : HRESOURCE, pszdriveletter : ::windows_sys::core::PWSTR, pcchdriveletter : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindDwordProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pdwpropertyvalue : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindExpandSzProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pszpropertyvalue : *mut ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindExpandedSzProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pszpropertyvalue : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilFindFileTimeProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pftpropertyvalue : *mut super::super::Foundation:: FILETIME) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindLongProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, plpropertyvalue : *mut i32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindMultiSzProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pszpropertyvalue : *mut ::windows_sys::core::PWSTR, pcbpropertyvaluesize : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindSzProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, pszpropertyvalue : *mut ::windows_sys::core::PWSTR) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFindULargeIntegerProperty(ppropertylist : *const ::core::ffi::c_void, cbpropertylistsize : u32, pszpropertyname : ::windows_sys::core::PCWSTR, plpropertyvalue : *mut u64) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilFreeEnvironment(lpenvironment : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilFreeParameterBlock(poutparams : *mut u8, pinparams : *const u8, ppropertytable : *const RESUTIL_PROPERTY_ITEM) -> ());
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilGetAllProperties(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, poutpropertylist : *mut ::core::ffi::c_void, cboutpropertylistsize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetBinaryProperty(ppboutvalue : *mut *mut u8, pcboutvaluesize : *mut u32, pvaluestruct : *const CLUSPROP_BINARY, pboldvalue : *const u8, cboldvaluesize : u32, pppropertylist : *mut *mut u8, pcbpropertylistsize : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilGetBinaryValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, ppboutvalue : *mut *mut u8, pcboutvaluesize : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetClusterGroupType(hgroup : HGROUP, grouptype : *mut CLUSGROUP_TYPE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetClusterId(hcluster : HCLUSTER, guid : *mut ::windows_sys::core::GUID) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetClusterRoleState(hcluster : HCLUSTER, eclusterrole : CLUSTER_ROLE) -> CLUSTER_ROLE_STATE);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetCoreClusterResources(hcluster : HCLUSTER, phclusternameresource : *mut HRESOURCE, phclusteripaddressresource : *mut HRESOURCE, phclusterquorumresource : *mut HRESOURCE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetCoreClusterResourcesEx(hclusterin : HCLUSTER, phclusternameresourceout : *mut HRESOURCE, phclusterquorumresourceout : *mut HRESOURCE, dwdesiredaccess : u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetCoreGroup(hcluster : HCLUSTER) -> HGROUP);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetDwordProperty(pdwoutvalue : *mut u32, pvaluestruct : *const CLUSPROP_DWORD, dwoldvalue : u32, dwminimum : u32, dwmaximum : u32, pppropertylist : *mut *mut u8, pcbpropertylistsize : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilGetDwordValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, pdwoutvalue : *mut u32, dwdefaultvalue : u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetEnvironmentWithNetName(hresource : HRESOURCE) -> *mut ::core::ffi::c_void);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetFileTimeProperty(pftoutvalue : *mut super::super::Foundation:: FILETIME, pvaluestruct : *const CLUSPROP_FILETIME, ftoldvalue : super::super::Foundation:: FILETIME, ftminimum : super::super::Foundation:: FILETIME, ftmaximum : super::super::Foundation:: FILETIME, pppropertylist : *mut *mut u8, pcbpropertylistsize : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetLongProperty(ploutvalue : *mut i32, pvaluestruct : *const CLUSPROP_LONG, loldvalue : i32, lminimum : i32, lmaximum : i32, pppropertylist : *mut *mut u8, pcbpropertylistsize : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetMultiSzProperty(ppszoutvalue : *mut ::windows_sys::core::PWSTR, pcboutvaluesize : *mut u32, pvaluestruct : *const CLUSPROP_SZ, pszoldvalue : ::windows_sys::core::PCWSTR, cboldvaluesize : u32, pppropertylist : *mut *mut u8, pcbpropertylistsize : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilGetPrivateProperties(hkeyclusterkey : super::super::System::Registry:: HKEY, poutpropertylist : *mut ::core::ffi::c_void, cboutpropertylistsize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilGetProperties(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, poutpropertylist : *mut ::core::ffi::c_void, cboutpropertylistsize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilGetPropertiesToParameterBlock(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, poutparams : *mut u8, bcheckforrequiredproperties : super::super::Foundation:: BOOL, psznameofpropinerror : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilGetProperty(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytableitem : *const RESUTIL_PROPERTY_ITEM, poutpropertyitem : *mut *mut ::core::ffi::c_void, pcboutpropertyitemsize : *mut u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetPropertyFormats(ppropertytable : *const RESUTIL_PROPERTY_ITEM, poutpropertyformatlist : *mut ::core::ffi::c_void, cbpropertyformatlistsize : u32, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilGetPropertySize(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytableitem : *const RESUTIL_PROPERTY_ITEM, pcboutpropertylistsize : *mut u32, pnpropertycount : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilGetQwordValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, pqwoutvalue : *mut u64, qwdefaultvalue : u64) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetResourceDependency(hself : super::super::Foundation:: HANDLE, lpszresourcetype : ::windows_sys::core::PCWSTR) -> HRESOURCE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetResourceDependencyByClass(hcluster : HCLUSTER, hself : super::super::Foundation:: HANDLE, prci : *mut CLUS_RESOURCE_CLASS_INFO, brecurse : super::super::Foundation:: BOOL) -> HRESOURCE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetResourceDependencyByClassEx(hcluster : HCLUSTER, hself : super::super::Foundation:: HANDLE, prci : *mut CLUS_RESOURCE_CLASS_INFO, brecurse : super::super::Foundation:: BOOL, dwdesiredaccess : u32) -> HRESOURCE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetResourceDependencyByName(hcluster : HCLUSTER, hself : super::super::Foundation:: HANDLE, lpszresourcetype : ::windows_sys::core::PCWSTR, brecurse : super::super::Foundation:: BOOL) -> HRESOURCE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetResourceDependencyByNameEx(hcluster : HCLUSTER, hself : super::super::Foundation:: HANDLE, lpszresourcetype : ::windows_sys::core::PCWSTR, brecurse : super::super::Foundation:: BOOL, dwdesiredaccess : u32) -> HRESOURCE);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGetResourceDependencyEx(hself : super::super::Foundation:: HANDLE, lpszresourcetype : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32) -> HRESOURCE);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetResourceDependentIPAddressProps(hresource : HRESOURCE, pszaddress : ::windows_sys::core::PWSTR, pcchaddress : *mut u32, pszsubnetmask : ::windows_sys::core::PWSTR, pcchsubnetmask : *mut u32, psznetwork : ::windows_sys::core::PWSTR, pcchnetwork : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetResourceName(hresource : HRESOURCE, pszresourcename : ::windows_sys::core::PWSTR, pcchresourcenameinout : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetResourceNameDependency(lpszresourcename : ::windows_sys::core::PCWSTR, lpszresourcetype : ::windows_sys::core::PCWSTR) -> HRESOURCE);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetResourceNameDependencyEx(lpszresourcename : ::windows_sys::core::PCWSTR, lpszresourcetype : ::windows_sys::core::PCWSTR, dwdesiredaccess : u32) -> HRESOURCE);
::windows_targets::link!("resutils.dll" "system" fn ResUtilGetSzProperty(ppszoutvalue : *mut ::windows_sys::core::PWSTR, pvaluestruct : *const CLUSPROP_SZ, pszoldvalue : ::windows_sys::core::PCWSTR, pppropertylist : *mut *mut u8, pcbpropertylistsize : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilGetSzValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilGroupsEqual(hself : HGROUP, hgroup : HGROUP, pequal : *mut super::super::Foundation:: BOOL) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilIsPathValid(pszpath : ::windows_sys::core::PCWSTR) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilIsResourceClassEqual(prci : *mut CLUS_RESOURCE_CLASS_INFO, hresource : HRESOURCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilLeftPaxosIsLessThanRight(left : *const PaxosTagCStruct, right : *const PaxosTagCStruct) -> super::super::Foundation:: BOOL);
::windows_targets::link!("resutils.dll" "system" fn ResUtilNodeEnum(hcluster : HCLUSTER, pnodecallback : LPNODE_CALLBACK, pparameter : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilPaxosComparer(left : *const PaxosTagCStruct, right : *const PaxosTagCStruct) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilPropertyListFromParameterBlock(ppropertytable : *const RESUTIL_PROPERTY_ITEM, poutpropertylist : *mut ::core::ffi::c_void, pcboutpropertylistsize : *mut u32, pinparams : *const u8, pcbbytesreturned : *mut u32, pcbrequired : *mut u32) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilRemoveResourceServiceEnvironment(pszservicename : ::windows_sys::core::PCWSTR, pfnlogevent : PLOG_EVENT_ROUTINE, hresourcehandle : isize) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilResourceDepEnum(hself : HRESOURCE, enumtype : u32, prescallback : LPRESOURCE_CALLBACK_EX, pparameter : *mut ::core::ffi::c_void) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilResourceTypesEqual(lpszresourcetypename : ::windows_sys::core::PCWSTR, hresource : HRESOURCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilResourcesEqual(hself : HRESOURCE, hresource : HRESOURCE) -> super::super::Foundation:: BOOL);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetBinaryValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, pbnewvalue : *const u8, cbnewvaluesize : u32, ppboutvalue : *mut *mut u8, pcboutvaluesize : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetDwordValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, dwnewvalue : u32, pdwoutvalue : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetExpandSzValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, psznewvalue : ::windows_sys::core::PCWSTR, ppszoutstring : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetMultiSzValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, psznewvalue : ::windows_sys::core::PCWSTR, cbnewvaluesize : u32, ppszoutvalue : *mut ::windows_sys::core::PWSTR, pcboutvaluesize : *mut u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetPrivatePropertyList(hkeyclusterkey : super::super::System::Registry:: HKEY, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilSetPropertyParameterBlock(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, reserved : *mut ::core::ffi::c_void, pinparams : *const u8, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32, poutparams : *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilSetPropertyParameterBlockEx(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, reserved : *mut ::core::ffi::c_void, pinparams : *const u8, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32, bforcewrite : super::super::Foundation:: BOOL, poutparams : *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilSetPropertyTable(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, reserved : *const ::core::ffi::c_void, ballowunknownproperties : super::super::Foundation:: BOOL, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32, poutparams : *mut u8) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilSetPropertyTableEx(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, reserved : *mut ::core::ffi::c_void, ballowunknownproperties : super::super::Foundation:: BOOL, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32, bforcewrite : super::super::Foundation:: BOOL, poutparams : *mut u8) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetQwordValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, qwnewvalue : u64, pqwoutvalue : *mut u64) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilSetResourceServiceEnvironment(pszservicename : ::windows_sys::core::PCWSTR, hresource : HRESOURCE, pfnlogevent : PLOG_EVENT_ROUTINE, hresourcehandle : isize) -> u32);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn ResUtilSetResourceServiceStartParameters(pszservicename : ::windows_sys::core::PCWSTR, schscmhandle : super::super::Security:: SC_HANDLE, phservice : *mut super::super::Security:: SC_HANDLE, pfnlogevent : PLOG_EVENT_ROUTINE, hresourcehandle : isize) -> u32);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn ResUtilSetResourceServiceStartParametersEx(pszservicename : ::windows_sys::core::PCWSTR, schscmhandle : super::super::Security:: SC_HANDLE, phservice : *mut super::super::Security:: SC_HANDLE, dwdesiredaccess : u32, pfnlogevent : PLOG_EVENT_ROUTINE, hresourcehandle : isize) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetSzValue(hkeyclusterkey : super::super::System::Registry:: HKEY, pszvaluename : ::windows_sys::core::PCWSTR, psznewvalue : ::windows_sys::core::PCWSTR, ppszoutstring : *mut ::windows_sys::core::PWSTR) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilSetUnknownProperties(hkeyclusterkey : super::super::System::Registry:: HKEY, ppropertytable : *const RESUTIL_PROPERTY_ITEM, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32) -> u32);
#[cfg(feature = "Win32_System_Registry")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_System_Registry\"`"] fn ResUtilSetValueEx(hkeyclusterkey : super::super::System::Registry:: HKEY, valuename : ::windows_sys::core::PCWSTR, valuetype : u32, valuedata : *const u8, valuesize : u32, flags : u32) -> u32);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn ResUtilStartResourceService(pszservicename : ::windows_sys::core::PCWSTR, phservicehandle : *mut super::super::Security:: SC_HANDLE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilStopResourceService(pszservicename : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn ResUtilStopService(hservicehandle : super::super::Security:: SC_HANDLE) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilTerminateServiceProcessFromResDll(dwservicepid : u32, boffline : super::super::Foundation:: BOOL, pdwresourcestate : *mut u32, pfnlogevent : PLOG_EVENT_ROUTINE, hresourcehandle : isize) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilVerifyPrivatePropertyList(pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ResUtilVerifyPropertyTable(ppropertytable : *const RESUTIL_PROPERTY_ITEM, reserved : *const ::core::ffi::c_void, ballowunknownproperties : super::super::Foundation:: BOOL, pinpropertylist : *const ::core::ffi::c_void, cbinpropertylistsize : u32, poutparams : *mut u8) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilVerifyResourceService(pszservicename : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Security")]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Security\"`"] fn ResUtilVerifyService(hservicehandle : super::super::Security:: SC_HANDLE) -> u32);
::windows_targets::link!("resutils.dll" "system" fn ResUtilVerifyShutdownSafe(flags : u32, reason : u32, presult : *mut u32) -> u32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
::windows_targets::link!("resutils.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"] fn ResUtilsDeleteKeyTree(key : super::super::System::Registry:: HKEY, keyname : ::windows_sys::core::PCWSTR, treatnokeyaserror : super::super::Foundation:: BOOL) -> u32);
::windows_targets::link!("ntlanman.dll" "system" fn ResetAllAppInstanceVersions() -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RestartClusterResource(hresource : HRESOURCE, dwflags : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn RestartClusterResourceEx(hresource : HRESOURCE, dwflags : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn RestoreClusterDatabase(lpszpathname : ::windows_sys::core::PCWSTR, bforce : super::super::Foundation:: BOOL, lpszquorumdriveletter : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ResumeClusterNode(hnode : HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ResumeClusterNodeEx(hnode : HNODE, eresumefailbacktype : CLUSTER_NODE_RESUME_FAILBACK_TYPE, dwresumeflagsreserved : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn ResumeClusterNodeEx2(hnode : HNODE, eresumefailbacktype : CLUSTER_NODE_RESUME_FAILBACK_TYPE, dwresumeflagsreserved : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("ntlanman.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetAppInstanceCsvFlags(processhandle : super::super::Foundation:: HANDLE, mask : u32, flags : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterGroupName(hgroup : HGROUP, lpszgroupname : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterGroupNameEx(hgroup : HGROUP, lpszgroupname : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterGroupNodeList(hgroup : HGROUP, nodecount : u32, nodelist : *const HNODE) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterGroupNodeListEx(hgroup : HGROUP, nodecount : u32, nodelist : *const HNODE, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterGroupSetDependencyExpression(hgroupset : HGROUPSET, lpszdependencyexprssion : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterGroupSetDependencyExpressionEx(hgroupset : HGROUPSET, lpszdependencyexpression : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterName(hcluster : HCLUSTER, lpsznewclustername : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterNameEx(hcluster : HCLUSTER, lpsznewclustername : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterNetworkName(hnetwork : HNETWORK, lpszname : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterNetworkNameEx(hnetwork : HNETWORK, lpszname : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterNetworkPriorityOrder(hcluster : HCLUSTER, networkcount : u32, networklist : *const HNETWORK) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterQuorumResource(hresource : HRESOURCE, lpszdevicename : ::windows_sys::core::PCWSTR, dwmaxquologsize : u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterQuorumResourceEx(hresource : HRESOURCE, lpszdevicename : ::windows_sys::core::PCWSTR, dwmaxquorumlogsize : u32, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterResourceDependencyExpression(hresource : HRESOURCE, lpszdependencyexpression : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterResourceName(hresource : HRESOURCE, lpszresourcename : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetClusterResourceNameEx(hresource : HRESOURCE, lpszresourcename : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("clusapi.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetClusterServiceAccountPassword(lpszclustername : ::windows_sys::core::PCWSTR, lpsznewpassword : ::windows_sys::core::PCWSTR, dwflags : u32, lpreturnstatusbuffer : *mut CLUSTER_SET_PASSWORD_STATUS, lpcbreturnstatusbuffersize : *mut u32) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetGroupDependencyExpression(hgroup : HGROUP, lpszdependencyexpression : ::windows_sys::core::PCWSTR) -> u32);
::windows_targets::link!("clusapi.dll" "system" fn SetGroupDependencyExpressionEx(hgroup : HGROUP, lpszdependencyexpression : ::windows_sys::core::PCWSTR, lpszreason : ::windows_sys::core::PCWSTR) -> u32);
pub type IGetClusterDataInfo = *mut ::core::ffi::c_void;
pub type IGetClusterGroupInfo = *mut ::core::ffi::c_void;
pub type IGetClusterNetInterfaceInfo = *mut ::core::ffi::c_void;
pub type IGetClusterNetworkInfo = *mut ::core::ffi::c_void;
pub type IGetClusterNodeInfo = *mut ::core::ffi::c_void;
pub type IGetClusterObjectInfo = *mut ::core::ffi::c_void;
pub type IGetClusterResourceInfo = *mut ::core::ffi::c_void;
pub type IGetClusterUIInfo = *mut ::core::ffi::c_void;
pub type ISClusApplication = *mut ::core::ffi::c_void;
pub type ISClusCryptoKeys = *mut ::core::ffi::c_void;
pub type ISClusDisk = *mut ::core::ffi::c_void;
pub type ISClusDisks = *mut ::core::ffi::c_void;
pub type ISClusNetInterface = *mut ::core::ffi::c_void;
pub type ISClusNetInterfaces = *mut ::core::ffi::c_void;
pub type ISClusNetwork = *mut ::core::ffi::c_void;
pub type ISClusNetworkNetInterfaces = *mut ::core::ffi::c_void;
pub type ISClusNetworks = *mut ::core::ffi::c_void;
pub type ISClusNode = *mut ::core::ffi::c_void;
pub type ISClusNodeNetInterfaces = *mut ::core::ffi::c_void;
pub type ISClusNodes = *mut ::core::ffi::c_void;
pub type ISClusPartition = *mut ::core::ffi::c_void;
pub type ISClusPartitionEx = *mut ::core::ffi::c_void;
pub type ISClusPartitions = *mut ::core::ffi::c_void;
pub type ISClusProperties = *mut ::core::ffi::c_void;
pub type ISClusProperty = *mut ::core::ffi::c_void;
pub type ISClusPropertyValue = *mut ::core::ffi::c_void;
pub type ISClusPropertyValueData = *mut ::core::ffi::c_void;
pub type ISClusPropertyValues = *mut ::core::ffi::c_void;
pub type ISClusRefObject = *mut ::core::ffi::c_void;
pub type ISClusRegistryKeys = *mut ::core::ffi::c_void;
pub type ISClusResDependencies = *mut ::core::ffi::c_void;
pub type ISClusResDependents = *mut ::core::ffi::c_void;
pub type ISClusResGroup = *mut ::core::ffi::c_void;
pub type ISClusResGroupPreferredOwnerNodes = *mut ::core::ffi::c_void;
pub type ISClusResGroupResources = *mut ::core::ffi::c_void;
pub type ISClusResGroups = *mut ::core::ffi::c_void;
pub type ISClusResPossibleOwnerNodes = *mut ::core::ffi::c_void;
pub type ISClusResType = *mut ::core::ffi::c_void;
pub type ISClusResTypePossibleOwnerNodes = *mut ::core::ffi::c_void;
pub type ISClusResTypeResources = *mut ::core::ffi::c_void;
pub type ISClusResTypes = *mut ::core::ffi::c_void;
pub type ISClusResource = *mut ::core::ffi::c_void;
pub type ISClusResources = *mut ::core::ffi::c_void;
pub type ISClusScsiAddress = *mut ::core::ffi::c_void;
pub type ISClusVersion = *mut ::core::ffi::c_void;
pub type ISCluster = *mut ::core::ffi::c_void;
pub type ISClusterNames = *mut ::core::ffi::c_void;
pub type ISDomainNames = *mut ::core::ffi::c_void;
pub type IWCContextMenuCallback = *mut ::core::ffi::c_void;
pub type IWCPropertySheetCallback = *mut ::core::ffi::c_void;
pub type IWCWizard97Callback = *mut ::core::ffi::c_void;
pub type IWCWizardCallback = *mut ::core::ffi::c_void;
pub type IWEExtendContextMenu = *mut ::core::ffi::c_void;
pub type IWEExtendPropertySheet = *mut ::core::ffi::c_void;
pub type IWEExtendWizard = *mut ::core::ffi::c_void;
pub type IWEExtendWizard97 = *mut ::core::ffi::c_void;
pub type IWEInvokeCommand = *mut ::core::ffi::c_void;
pub const BitLockerDecrypted: i32 = 4i32;
pub const BitLockerDecrypting: i32 = 16i32;
pub const BitLockerEnabled: i32 = 1i32;
pub const BitLockerPaused: i32 = 64i32;
pub const BitLockerStopped: i32 = 128i32;
pub const BitlockerEncrypted: i32 = 8i32;
pub const BitlockerEncrypting: i32 = 32i32;
pub const CA_UPGRADE_VERSION: u32 = 1u32;
pub const CLCTL_ADD_CRYPTO_CHECKPOINT: CLCTL_CODES = 4194478i32;
pub const CLCTL_ADD_CRYPTO_CHECKPOINT_EX: CLCTL_CODES = 4195030i32;
pub const CLCTL_ADD_DEPENDENCY: CLCTL_CODES = 5242898i32;
pub const CLCTL_ADD_OWNER: CLCTL_CODES = 5242906i32;
pub const CLCTL_ADD_REGISTRY_CHECKPOINT: CLCTL_CODES = 4194466i32;
pub const CLCTL_ADD_REGISTRY_CHECKPOINT_32BIT: CLCTL_CODES = 4194498i32;
pub const CLCTL_ADD_REGISTRY_CHECKPOINT_64BIT: CLCTL_CODES = 4194494i32;
pub const CLCTL_BATCH_BLOCK_KEY: CLCTL_CODES = 574i32;
pub const CLCTL_BATCH_UNBLOCK_KEY: CLCTL_CODES = 577i32;
pub const CLCTL_BLOCK_GEM_SEND_RECV: CLCTL_CODES = 717i32;
pub const CLCTL_CHECK_DRAIN_VETO: CLCTL_CODES = 1057069i32;
pub const CLCTL_CHECK_VOTER_DOWN: CLCTL_CODES = 73i32;
pub const CLCTL_CHECK_VOTER_DOWN_WITNESS: CLCTL_CODES = 113i32;
pub const CLCTL_CHECK_VOTER_EVICT: CLCTL_CODES = 69i32;
pub const CLCTL_CHECK_VOTER_EVICT_WITNESS: CLCTL_CODES = 109i32;
pub const CLCTL_CLEAR_NODE_CONNECTION_INFO: CLCTL_CODES = 4195078i32;
pub const CLCTL_CLOUD_WITNESS_RESOURCE_TYPE_VALIDATE_CREDENTIALS: CLCTL_CODES = 8417i32;
pub const CLCTL_CLOUD_WITNESS_RESOURCE_TYPE_VALIDATE_CREDENTIALS_WITH_KEY: CLCTL_CODES = 8433i32;
pub const CLCTL_CLOUD_WITNESS_RESOURCE_UPDATE_KEY: CLCTL_CODES = 4202742i32;
pub const CLCTL_CLOUD_WITNESS_RESOURCE_UPDATE_TOKEN: CLCTL_CODES = 4202726i32;
pub const CLCTL_CLUSTER_BASE: u32 = 0u32;
pub const CLCTL_CLUSTER_NAME_CHANGED: CLCTL_CODES = 5242922i32;
pub const CLCTL_CLUSTER_VERSION_CHANGED: CLCTL_CODES = 5242926i32;
pub const CLCTL_DELETE: CLCTL_CODES = 5242886i32;
pub const CLCTL_DELETE_CRYPTO_CHECKPOINT: CLCTL_CODES = 4194482i32;
pub const CLCTL_DELETE_REGISTRY_CHECKPOINT: CLCTL_CODES = 4194470i32;
pub const CLCTL_DISABLE_SHARED_VOLUME_DIRECTIO: CLCTL_CODES = 4194958i32;
pub const CLCTL_ENABLE_SHARED_VOLUME_DIRECTIO: CLCTL_CODES = 4194954i32;
pub const CLCTL_ENUM_AFFINITY_RULE_NAMES: CLCTL_CODES = 11741i32;
pub const CLCTL_ENUM_COMMON_PROPERTIES: CLCTL_CODES = 81i32;
pub const CLCTL_ENUM_PRIVATE_PROPERTIES: CLCTL_CODES = 121i32;
pub const CLCTL_EVICT_NODE: CLCTL_CODES = 5242894i32;
pub const CLCTL_FILESERVER_SHARE_ADD: CLCTL_CODES = 4194886i32;
pub const CLCTL_FILESERVER_SHARE_DEL: CLCTL_CODES = 4194890i32;
pub const CLCTL_FILESERVER_SHARE_MODIFY: CLCTL_CODES = 4194894i32;
pub const CLCTL_FILESERVER_SHARE_REPORT: CLCTL_CODES = 593i32;
pub const CLCTL_FIXUP_ON_UPGRADE: CLCTL_CODES = 5242930i32;
pub const CLCTL_FORCE_DB_FLUSH: CLCTL_CODES = 4206054i32;
pub const CLCTL_FORCE_QUORUM: CLCTL_CODES = 5242950i32;
pub const CLCTL_FSWITNESS_GET_EPOCH_INFO: CLCTL_CODES = 1048669i32;
pub const CLCTL_FSWITNESS_RELEASE_LOCK: CLCTL_CODES = 5242982i32;
pub const CLCTL_FSWITNESS_SET_EPOCH_INFO: CLCTL_CODES = 5242978i32;
pub const CLCTL_GET_ARB_TIMEOUT: CLCTL_CODES = 21i32;
pub const CLCTL_GET_CHARACTERISTICS: CLCTL_CODES = 5i32;
pub const CLCTL_GET_CLASS_INFO: CLCTL_CODES = 13i32;
pub const CLCTL_GET_CLUSDB_TIMESTAMP: CLCTL_CODES = 681i32;
pub const CLCTL_GET_CLUSTER_SERVICE_ACCOUNT_NAME: CLCTL_CODES = 65i32;
pub const CLCTL_GET_COMMON_PROPERTIES: CLCTL_CODES = 89i32;
pub const CLCTL_GET_COMMON_PROPERTY_FMTS: CLCTL_CODES = 101i32;
pub const CLCTL_GET_COMMON_RESOURCE_PROPERTY_FMTS: CLCTL_CODES = 105i32;
pub const CLCTL_GET_CRYPTO_CHECKPOINTS: CLCTL_CODES = 181i32;
pub const CLCTL_GET_DNS_NAME: CLCTL_CODES = 373i32;
pub const CLCTL_GET_FAILURE_INFO: CLCTL_CODES = 25i32;
pub const CLCTL_GET_FLAGS: CLCTL_CODES = 9i32;
pub const CLCTL_GET_FQDN: CLCTL_CODES = 61i32;
pub const CLCTL_GET_GEMID_VECTOR: CLCTL_CODES = 721i32;
pub const CLCTL_GET_GUM_LOCK_OWNER: CLCTL_CODES = 697i32;
pub const CLCTL_GET_ID: CLCTL_CODES = 57i32;
pub const CLCTL_GET_INFRASTRUCTURE_SOFS_BUFFER: CLCTL_CODES = 11657i32;
pub const CLCTL_GET_LOADBAL_PROCESS_LIST: CLCTL_CODES = 201i32;
pub const CLCTL_GET_NAME: CLCTL_CODES = 41i32;
pub const CLCTL_GET_NETWORK: CLCTL_CODES = 53i32;
pub const CLCTL_GET_NETWORK_NAME: CLCTL_CODES = 361i32;
pub const CLCTL_GET_NODE: CLCTL_CODES = 49i32;
pub const CLCTL_GET_NODES_IN_FD: CLCTL_CODES = 11745i32;
pub const CLCTL_GET_OPERATION_CONTEXT: CLCTL_CODES = 1057001i32;
pub const CLCTL_GET_PRIVATE_PROPERTIES: CLCTL_CODES = 129i32;
pub const CLCTL_GET_PRIVATE_PROPERTY_FMTS: CLCTL_CODES = 141i32;
pub const CLCTL_GET_PRIVATE_RESOURCE_PROPERTY_FMTS: CLCTL_CODES = 145i32;
pub const CLCTL_GET_REGISTRY_CHECKPOINTS: CLCTL_CODES = 169i32;
pub const CLCTL_GET_REQUIRED_DEPENDENCIES: CLCTL_CODES = 17i32;
pub const CLCTL_GET_RESOURCE_TYPE: CLCTL_CODES = 45i32;
pub const CLCTL_GET_RO_COMMON_PROPERTIES: CLCTL_CODES = 85i32;
pub const CLCTL_GET_RO_PRIVATE_PROPERTIES: CLCTL_CODES = 125i32;
pub const CLCTL_GET_SHARED_VOLUME_ID: CLCTL_CODES = 657i32;
pub const CLCTL_GET_STATE_CHANGE_TIME: CLCTL_CODES = 11613i32;
pub const CLCTL_GET_STORAGE_CONFIGURATION: CLCTL_CODES = 741i32;
pub const CLCTL_GET_STORAGE_CONFIG_ATTRIBUTES: CLCTL_CODES = 745i32;
pub const CLCTL_GET_STUCK_NODES: CLCTL_CODES = 701i32;
pub const CLCTL_GLOBAL_SHIFT: u32 = 23u32;
pub const CLCTL_GROUPSET_GET_GROUPS: CLCTL_CODES = 11633i32;
pub const CLCTL_GROUPSET_GET_PROVIDER_GROUPS: CLCTL_CODES = 11637i32;
pub const CLCTL_GROUPSET_GET_PROVIDER_GROUPSETS: CLCTL_CODES = 11641i32;
pub const CLCTL_GROUP_GET_LAST_MOVE_TIME: CLCTL_CODES = 729i32;
pub const CLCTL_GROUP_GET_PROVIDER_GROUPS: CLCTL_CODES = 11645i32;
pub const CLCTL_GROUP_GET_PROVIDER_GROUPSETS: CLCTL_CODES = 11649i32;
pub const CLCTL_GROUP_SET_CCF_FROM_MASTER: CLCTL_CODES = 4205958i32;
pub const CLCTL_HOLD_IO: CLCTL_CODES = 5242942i32;
pub const CLCTL_INITIALIZE: CLCTL_CODES = 5242954i32;
pub const CLCTL_INJECT_GEM_FAULT: CLCTL_CODES = 705i32;
pub const CLCTL_INSTALL_NODE: CLCTL_CODES = 5242890i32;
pub const CLCTL_INTERNAL_SHIFT: u32 = 20u32;
pub const CLCTL_INTRODUCE_GEM_REPAIR_DELAY: CLCTL_CODES = 709i32;
pub const CLCTL_IPADDRESS_RELEASE_LEASE: CLCTL_CODES = 4194754i32;
pub const CLCTL_IPADDRESS_RENEW_LEASE: CLCTL_CODES = 4194750i32;
pub const CLCTL_IS_FEATURE_INSTALLED: CLCTL_CODES = 753i32;
pub const CLCTL_IS_QUORUM_BLOCKED: CLCTL_CODES = 689i32;
pub const CLCTL_IS_S2D_FEATURE_SUPPORTED: CLCTL_CODES = 757i32;
pub const CLCTL_JOINING_GROUP: CLCTL_CODES = 5242970i32;
pub const CLCTL_LEAVING_GROUP: CLCTL_CODES = 5242966i32;
pub const CLCTL_MODIFY_SHIFT: u32 = 22u32;
pub const CLCTL_NETNAME_CREDS_NOTIFYCAM: CLCTL_CODES = 5242986i32;
pub const CLCTL_NETNAME_DELETE_CO: CLCTL_CODES = 382i32;
pub const CLCTL_NETNAME_GET_OU_FOR_VCO: CLCTL_CODES = 4194926i32;
pub const CLCTL_NETNAME_GET_VIRTUAL_SERVER_TOKEN: CLCTL_CODES = 365i32;
pub const CLCTL_NETNAME_REGISTER_DNS_RECORDS: CLCTL_CODES = 370i32;
pub const CLCTL_NETNAME_REPAIR_VCO: CLCTL_CODES = 397i32;
pub const CLCTL_NETNAME_RESET_VCO: CLCTL_CODES = 389i32;
pub const CLCTL_NETNAME_SET_PWD_INFO: CLCTL_CODES = 378i32;
pub const CLCTL_NETNAME_SET_PWD_INFOEX: CLCTL_CODES = 794i32;
pub const CLCTL_NETNAME_VALIDATE_VCO: CLCTL_CODES = 385i32;
pub const CLCTL_NOTIFY_DRAIN_COMPLETE: CLCTL_CODES = 1057073i32;
pub const CLCTL_NOTIFY_INFRASTRUCTURE_SOFS_CHANGED: CLCTL_CODES = 4205970i32;
pub const CLCTL_NOTIFY_MONITOR_SHUTTING_DOWN: CLCTL_CODES = 1048705i32;
pub const CLCTL_NOTIFY_OWNER_CHANGE: CLCTL_CODES = 5251362i32;
pub const CLCTL_NOTIFY_QUORUM_STATUS: CLCTL_CODES = 5243006i32;
pub const CLCTL_POOL_GET_DRIVE_INFO: CLCTL_CODES = 693i32;
pub const CLCTL_PROVIDER_STATE_CHANGE: CLCTL_CODES = 5242962i32;
pub const CLCTL_QUERY_DELETE: CLCTL_CODES = 441i32;
pub const CLCTL_QUERY_MAINTENANCE_MODE: CLCTL_CODES = 481i32;
pub const CLCTL_RELOAD_AUTOLOGGER_CONFIG: CLCTL_CODES = 11730i32;
pub const CLCTL_REMOVE_DEPENDENCY: CLCTL_CODES = 5242902i32;
pub const CLCTL_REMOVE_NODE: CLCTL_CODES = 4195054i32;
pub const CLCTL_REMOVE_OWNER: CLCTL_CODES = 5242910i32;
pub const CLCTL_REPLICATION_ADD_REPLICATION_GROUP: CLCTL_CODES = 8514i32;
pub const CLCTL_REPLICATION_GET_ELIGIBLE_LOGDISKS: CLCTL_CODES = 8521i32;
pub const CLCTL_REPLICATION_GET_ELIGIBLE_SOURCE_DATADISKS: CLCTL_CODES = 8529i32;
pub const CLCTL_REPLICATION_GET_ELIGIBLE_TARGET_DATADISKS: CLCTL_CODES = 8525i32;
pub const CLCTL_REPLICATION_GET_LOG_INFO: CLCTL_CODES = 8517i32;
pub const CLCTL_REPLICATION_GET_LOG_VOLUME: CLCTL_CODES = 8541i32;
pub const CLCTL_REPLICATION_GET_REPLICATED_DISKS: CLCTL_CODES = 8533i32;
pub const CLCTL_REPLICATION_GET_REPLICATED_PARTITION_INFO: CLCTL_CODES = 8549i32;
pub const CLCTL_REPLICATION_GET_REPLICA_VOLUMES: CLCTL_CODES = 8537i32;
pub const CLCTL_REPLICATION_GET_RESOURCE_GROUP: CLCTL_CODES = 8545i32;
pub const CLCTL_RESOURCE_PREPARE_UPGRADE: CLCTL_CODES = 4202730i32;
pub const CLCTL_RESOURCE_UPGRADE_COMPLETED: CLCTL_CODES = 4202734i32;
pub const CLCTL_RESOURCE_UPGRADE_DLL: CLCTL_CODES = 4194490i32;
pub const CLCTL_RESUME_IO: CLCTL_CODES = 5242946i32;
pub const CLCTL_RW_MODIFY_NOOP: CLCTL_CODES = 4194990i32;
pub const CLCTL_SCALEOUT_COMMAND: CLCTL_CODES = 4205974i32;
pub const CLCTL_SCALEOUT_CONTROL: CLCTL_CODES = 4205978i32;
pub const CLCTL_SCALEOUT_GET_CLUSTERS: CLCTL_CODES = 4205981i32;
pub const CLCTL_SEND_DUMMY_GEM_MESSAGES: CLCTL_CODES = 713i32;
pub const CLCTL_SET_ACCOUNT_ACCESS: CLCTL_CODES = 4194546i32;
pub const CLCTL_SET_CLUSTER_S2D_CACHE_METADATA_RESERVE_BYTES: CLCTL_CODES = 4205934i32;
pub const CLCTL_SET_CLUSTER_S2D_ENABLED: CLCTL_CODES = 4205922i32;
pub const CLCTL_SET_COMMON_PROPERTIES: CLCTL_CODES = 4194398i32;
pub const CLCTL_SET_CSV_MAINTENANCE_MODE: CLCTL_CODES = 4194966i32;
pub const CLCTL_SET_DNS_DOMAIN: CLCTL_CODES = 4195082i32;
pub const CLCTL_SET_INFRASTRUCTURE_SOFS_BUFFER: CLCTL_CODES = 4205966i32;
pub const CLCTL_SET_MAINTENANCE_MODE: CLCTL_CODES = 4194790i32;
pub const CLCTL_SET_NAME: CLCTL_CODES = 5242918i32;
pub const CLCTL_SET_PRIVATE_PROPERTIES: CLCTL_CODES = 4194438i32;
pub const CLCTL_SET_SHARED_VOLUME_BACKUP_MODE: CLCTL_CODES = 4194970i32;
pub const CLCTL_SET_STORAGE_CONFIGURATION: CLCTL_CODES = 4195042i32;
pub const CLCTL_SHUTDOWN: CLCTL_CODES = 77i32;
pub const CLCTL_STARTING_PHASE1: CLCTL_CODES = 5242934i32;
pub const CLCTL_STARTING_PHASE2: CLCTL_CODES = 5242938i32;
pub const CLCTL_STATE_CHANGE_REASON: CLCTL_CODES = 5242958i32;
pub const CLCTL_STORAGE_GET_AVAILABLE_DISKS: CLCTL_CODES = 405i32;
pub const CLCTL_STORAGE_GET_AVAILABLE_DISKS_EX: CLCTL_CODES = 501i32;
pub const CLCTL_STORAGE_GET_AVAILABLE_DISKS_EX2_INT: CLCTL_CODES = 8161i32;
pub const CLCTL_STORAGE_GET_CLUSBFLT_PATHINFO: CLCTL_CODES = 769i32;
pub const CLCTL_STORAGE_GET_CLUSBFLT_PATHS: CLCTL_CODES = 765i32;
pub const CLCTL_STORAGE_GET_CLUSPORT_DISK_COUNT: CLCTL_CODES = 509i32;
pub const CLCTL_STORAGE_GET_DIRTY: CLCTL_CODES = 537i32;
pub const CLCTL_STORAGE_GET_DISKID: CLCTL_CODES = 517i32;
pub const CLCTL_STORAGE_GET_DISK_INFO: CLCTL_CODES = 401i32;
pub const CLCTL_STORAGE_GET_DISK_INFO_EX: CLCTL_CODES = 497i32;
pub const CLCTL_STORAGE_GET_DISK_INFO_EX2: CLCTL_CODES = 505i32;
pub const CLCTL_STORAGE_GET_DISK_NUMBER_INFO: CLCTL_CODES = 417i32;
pub const CLCTL_STORAGE_GET_DRIVELETTERS: CLCTL_CODES = 493i32;
pub const CLCTL_STORAGE_GET_MOUNTPOINTS: CLCTL_CODES = 529i32;
pub const CLCTL_STORAGE_GET_PHYSICAL_DISK_INFO: CLCTL_CODES = 761i32;
pub const CLCTL_STORAGE_GET_RESOURCEID: CLCTL_CODES = 557i32;
pub const CLCTL_STORAGE_GET_SHARED_VOLUME_INFO: CLCTL_CODES = 549i32;
pub const CLCTL_STORAGE_GET_SHARED_VOLUME_PARTITION_NAMES: CLCTL_CODES = 669i32;
pub const CLCTL_STORAGE_GET_SHARED_VOLUME_STATES: CLCTL_CODES = 4194978i32;
pub const CLCTL_STORAGE_IS_CLUSTERABLE: CLCTL_CODES = 521i32;
pub const CLCTL_STORAGE_IS_CSV_FILE: CLCTL_CODES = 553i32;
pub const CLCTL_STORAGE_IS_PATH_VALID: CLCTL_CODES = 409i32;
pub const CLCTL_STORAGE_IS_SHARED_VOLUME: CLCTL_CODES = 677i32;
pub const CLCTL_STORAGE_REMAP_DRIVELETTER: CLCTL_CODES = 513i32;
pub const CLCTL_STORAGE_REMOVE_VM_OWNERSHIP: CLCTL_CODES = 4194830i32;
pub const CLCTL_STORAGE_RENAME_SHARED_VOLUME: CLCTL_CODES = 11734i32;
pub const CLCTL_STORAGE_RENAME_SHARED_VOLUME_GUID: CLCTL_CODES = 11738i32;
pub const CLCTL_STORAGE_SET_DRIVELETTER: CLCTL_CODES = 4194794i32;
pub const CLCTL_STORAGE_SYNC_CLUSDISK_DB: CLCTL_CODES = 4194718i32;
pub const CLCTL_UNDELETE: CLCTL_CODES = 5243014i32;
pub const CLCTL_UNKNOWN: CLCTL_CODES = 0i32;
pub const CLCTL_USER_SHIFT: u32 = 21u32;
pub const CLCTL_VALIDATE_CHANGE_GROUP: CLCTL_CODES = 1057061i32;
pub const CLCTL_VALIDATE_COMMON_PROPERTIES: CLCTL_CODES = 97i32;
pub const CLCTL_VALIDATE_DIRECTORY: CLCTL_CODES = 569i32;
pub const CLCTL_VALIDATE_NETNAME: CLCTL_CODES = 565i32;
pub const CLCTL_VALIDATE_PATH: CLCTL_CODES = 561i32;
pub const CLCTL_VALIDATE_PRIVATE_PROPERTIES: CLCTL_CODES = 137i32;
pub const CLOUD_WITNESS_CONTAINER_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("msft-cloud-witness");
pub const CLRES_VERSION_V1_00: u32 = 256u32;
pub const CLRES_VERSION_V2_00: u32 = 512u32;
pub const CLRES_VERSION_V3_00: u32 = 768u32;
pub const CLRES_VERSION_V4_00: u32 = 1024u32;
pub const CLUADMEX_OT_CLUSTER: CLUADMEX_OBJECT_TYPE = 1i32;
pub const CLUADMEX_OT_GROUP: CLUADMEX_OBJECT_TYPE = 3i32;
pub const CLUADMEX_OT_NETINTERFACE: CLUADMEX_OBJECT_TYPE = 7i32;
pub const CLUADMEX_OT_NETWORK: CLUADMEX_OBJECT_TYPE = 6i32;
pub const CLUADMEX_OT_NODE: CLUADMEX_OBJECT_TYPE = 2i32;
pub const CLUADMEX_OT_NONE: CLUADMEX_OBJECT_TYPE = 0i32;
pub const CLUADMEX_OT_RESOURCE: CLUADMEX_OBJECT_TYPE = 4i32;
pub const CLUADMEX_OT_RESOURCETYPE: CLUADMEX_OBJECT_TYPE = 5i32;
pub const CLUSAPI_CHANGE_ACCESS: i32 = 2i32;
pub const CLUSAPI_CHANGE_RESOURCE_GROUP_FORCE_MOVE_TO_CSV: u64 = 1u64;
pub const CLUSAPI_GROUP_MOVE_FAILBACK: u32 = 16u32;
pub const CLUSAPI_GROUP_MOVE_HIGH_PRIORITY_START: u32 = 8u32;
pub const CLUSAPI_GROUP_MOVE_IGNORE_AFFINITY_RULE: u32 = 32u32;
pub const CLUSAPI_GROUP_MOVE_IGNORE_RESOURCE_STATUS: u32 = 1u32;
pub const CLUSAPI_GROUP_MOVE_QUEUE_ENABLED: u32 = 4u32;
pub const CLUSAPI_GROUP_MOVE_RETURN_TO_SOURCE_NODE_ON_ERROR: u32 = 2u32;
pub const CLUSAPI_GROUP_OFFLINE_IGNORE_RESOURCE_STATUS: u32 = 1u32;
pub const CLUSAPI_GROUP_ONLINE_BEST_POSSIBLE_NODE: u32 = 4u32;
pub const CLUSAPI_GROUP_ONLINE_IGNORE_AFFINITY_RULE: u32 = 8u32;
pub const CLUSAPI_GROUP_ONLINE_IGNORE_RESOURCE_STATUS: u32 = 1u32;
pub const CLUSAPI_GROUP_ONLINE_SYNCHRONOUS: u32 = 2u32;
pub const CLUSAPI_NODE_AVOID_PLACEMENT: u32 = 2u32;
pub const CLUSAPI_NODE_PAUSE_REMAIN_ON_PAUSED_NODE_ON_MOVE_ERROR: u32 = 1u32;
pub const CLUSAPI_NODE_PAUSE_RETRY_DRAIN_ON_FAILURE: u32 = 4u32;
pub const CLUSAPI_NODE_RESUME_FAILBACK_PINNED_VMS_ONLY: u32 = 4u32;
pub const CLUSAPI_NODE_RESUME_FAILBACK_STORAGE: u32 = 1u32;
pub const CLUSAPI_NODE_RESUME_FAILBACK_VMS: u32 = 2u32;
pub const CLUSAPI_NO_ACCESS: i32 = 4i32;
pub const CLUSAPI_READ_ACCESS: i32 = 1i32;
pub const CLUSAPI_RESOURCE_OFFLINE_DO_NOT_UPDATE_PERSISTENT_STATE: u32 = 4u32;
pub const CLUSAPI_RESOURCE_OFFLINE_FORCE_WITH_TERMINATION: u32 = 2u32;
pub const CLUSAPI_RESOURCE_OFFLINE_IGNORE_RESOURCE_STATUS: u32 = 1u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_BEING_DELETED: u32 = 8u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_BEING_RESTARTED: u32 = 16u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_MOVING: u32 = 2u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_NONE: u32 = 0u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_PREEMPTED: u32 = 32u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_SHUTTING_DOWN: u32 = 64u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_UNKNOWN: u32 = 1u32;
pub const CLUSAPI_RESOURCE_OFFLINE_REASON_USER_REQUESTED: u32 = 4u32;
pub const CLUSAPI_RESOURCE_ONLINE_BEST_POSSIBLE_NODE: u32 = 8u32;
pub const CLUSAPI_RESOURCE_ONLINE_DO_NOT_UPDATE_PERSISTENT_STATE: u32 = 2u32;
pub const CLUSAPI_RESOURCE_ONLINE_IGNORE_AFFINITY_RULE: u32 = 32u32;
pub const CLUSAPI_RESOURCE_ONLINE_IGNORE_RESOURCE_STATUS: u32 = 1u32;
pub const CLUSAPI_RESOURCE_ONLINE_NECESSARY_FOR_QUORUM: u32 = 4u32;
pub const CLUSAPI_VALID_CHANGE_RESOURCE_GROUP_FLAGS: u64 = 1u64;
pub const CLUSAPI_VERSION: u32 = 2572u32;
pub const CLUSAPI_VERSION_NI: u32 = 2572u32;
pub const CLUSAPI_VERSION_RS3: u32 = 2560u32;
pub const CLUSAPI_VERSION_SERVER2008: u32 = 1536u32;
pub const CLUSAPI_VERSION_SERVER2008R2: u32 = 1792u32;
pub const CLUSAPI_VERSION_WINDOWS8: u32 = 1793u32;
pub const CLUSAPI_VERSION_WINDOWSBLUE: u32 = 1794u32;
pub const CLUSAPI_VERSION_WINTHRESHOLD: u32 = 1795u32;
pub const CLUSCTL_ACCESS_MODE_MASK: u32 = 3u32;
pub const CLUSCTL_ACCESS_SHIFT: u32 = 0u32;
pub const CLUSCTL_AFFINITYRULE_GET_COMMON_PROPERTIES: CLUSCTL_AFFINITYRULE_CODES = 150995033i32;
pub const CLUSCTL_AFFINITYRULE_GET_GROUPNAMES: CLUSCTL_AFFINITYRULE_CODES = 151006577i32;
pub const CLUSCTL_AFFINITYRULE_GET_ID: CLUSCTL_AFFINITYRULE_CODES = 150995001i32;
pub const CLUSCTL_AFFINITYRULE_GET_RO_COMMON_PROPERTIES: CLUSCTL_AFFINITYRULE_CODES = 150995029i32;
pub const CLUSCTL_AFFINITYRULE_SET_COMMON_PROPERTIES: CLUSCTL_AFFINITYRULE_CODES = 155189342i32;
pub const CLUSCTL_CLOUD_WITNESS_RESOURCE_TYPE_VALIDATE_CREDENTIALS: CLUSCTL_RESOURCE_TYPE_CODES = 33562849i32;
pub const CLUSCTL_CLOUD_WITNESS_RESOURCE_TYPE_VALIDATE_CREDENTIALS_WITH_KEY: CLUSCTL_RESOURCE_TYPE_CODES = 33562865i32;
pub const CLUSCTL_CLOUD_WITNESS_RESOURCE_UPDATE_KEY: CLUSCTL_RESOURCE_CODES = 20979958i32;
pub const CLUSCTL_CLOUD_WITNESS_RESOURCE_UPDATE_TOKEN: CLUSCTL_RESOURCE_CODES = 20979942i32;
pub const CLUSCTL_CLUSTER_BATCH_BLOCK_KEY: CLUSCTL_CLUSTER_CODES = 117441086i32;
pub const CLUSCTL_CLUSTER_BATCH_UNBLOCK_KEY: CLUSCTL_CLUSTER_CODES = 117441089i32;
pub const CLUSCTL_CLUSTER_CHECK_VOTER_DOWN: CLUSCTL_CLUSTER_CODES = 117440585i32;
pub const CLUSCTL_CLUSTER_CHECK_VOTER_DOWN_WITNESS: CLUSCTL_CLUSTER_CODES = 117440625i32;
pub const CLUSCTL_CLUSTER_CHECK_VOTER_EVICT: CLUSCTL_CLUSTER_CODES = 117440581i32;
pub const CLUSCTL_CLUSTER_CHECK_VOTER_EVICT_WITNESS: CLUSCTL_CLUSTER_CODES = 117440621i32;
pub const CLUSCTL_CLUSTER_CLEAR_NODE_CONNECTION_INFO: CLUSCTL_CLUSTER_CODES = 121635590i32;
pub const CLUSCTL_CLUSTER_ENUM_AFFINITY_RULE_NAMES: CLUSCTL_CLUSTER_CODES = 117452253i32;
pub const CLUSCTL_CLUSTER_ENUM_COMMON_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440593i32;
pub const CLUSCTL_CLUSTER_ENUM_PRIVATE_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440633i32;
pub const CLUSCTL_CLUSTER_FORCE_FLUSH_DB: CLUSCTL_CLUSTER_CODES = 121646566i32;
pub const CLUSCTL_CLUSTER_GET_CLMUSR_TOKEN: CLUSCTL_CLUSTER_CODES = 117440877i32;
pub const CLUSCTL_CLUSTER_GET_CLUSDB_TIMESTAMP: CLUSCTL_CLUSTER_CODES = 117441193i32;
pub const CLUSCTL_CLUSTER_GET_COMMON_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440601i32;
pub const CLUSCTL_CLUSTER_GET_COMMON_PROPERTY_FMTS: CLUSCTL_CLUSTER_CODES = 117440613i32;
pub const CLUSCTL_CLUSTER_GET_FQDN: CLUSCTL_CLUSTER_CODES = 117440573i32;
pub const CLUSCTL_CLUSTER_GET_GUM_LOCK_OWNER: CLUSCTL_CLUSTER_CODES = 117441209i32;
pub const CLUSCTL_CLUSTER_GET_NODES_IN_FD: CLUSCTL_CLUSTER_CODES = 117452257i32;
pub const CLUSCTL_CLUSTER_GET_PRIVATE_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440641i32;
pub const CLUSCTL_CLUSTER_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_CLUSTER_CODES = 117440653i32;
pub const CLUSCTL_CLUSTER_GET_RO_COMMON_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440597i32;
pub const CLUSCTL_CLUSTER_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440637i32;
pub const CLUSCTL_CLUSTER_GET_SHARED_VOLUME_ID: CLUSCTL_CLUSTER_CODES = 117441169i32;
pub const CLUSCTL_CLUSTER_GET_STORAGE_CONFIGURATION: CLUSCTL_CLUSTER_CODES = 117441253i32;
pub const CLUSCTL_CLUSTER_GET_STORAGE_CONFIG_ATTRIBUTES: CLUSCTL_CLUSTER_CODES = 117441257i32;
pub const CLUSCTL_CLUSTER_RELOAD_AUTOLOGGER_CONFIG: CLUSCTL_CLUSTER_CODES = 117452242i32;
pub const CLUSCTL_CLUSTER_REMOVE_NODE: CLUSCTL_CLUSTER_CODES = 121635566i32;
pub const CLUSCTL_CLUSTER_SET_ACCOUNT_ACCESS: CLUSCTL_CLUSTER_CODES = 121635058i32;
pub const CLUSCTL_CLUSTER_SET_CLUSTER_S2D_CACHE_METADATA_RESERVE_BYTES: CLUSCTL_CLUSTER_CODES = 121646446i32;
pub const CLUSCTL_CLUSTER_SET_CLUSTER_S2D_ENABLED: CLUSCTL_CLUSTER_CODES = 121646434i32;
pub const CLUSCTL_CLUSTER_SET_COMMON_PROPERTIES: CLUSCTL_CLUSTER_CODES = 121634910i32;
pub const CLUSCTL_CLUSTER_SET_DNS_DOMAIN: CLUSCTL_CLUSTER_CODES = 121635594i32;
pub const CLUSCTL_CLUSTER_SET_PRIVATE_PROPERTIES: CLUSCTL_CLUSTER_CODES = 121634950i32;
pub const CLUSCTL_CLUSTER_SET_STORAGE_CONFIGURATION: CLUSCTL_CLUSTER_CODES = 121635554i32;
pub const CLUSCTL_CLUSTER_SHUTDOWN: CLUSCTL_CLUSTER_CODES = 117440589i32;
pub const CLUSCTL_CLUSTER_STORAGE_RENAME_SHARED_VOLUME: CLUSCTL_CLUSTER_CODES = 117452246i32;
pub const CLUSCTL_CLUSTER_STORAGE_RENAME_SHARED_VOLUME_GUID: CLUSCTL_CLUSTER_CODES = 117452250i32;
pub const CLUSCTL_CLUSTER_UNKNOWN: CLUSCTL_CLUSTER_CODES = 117440512i32;
pub const CLUSCTL_CLUSTER_VALIDATE_COMMON_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440609i32;
pub const CLUSCTL_CLUSTER_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_CLUSTER_CODES = 117440649i32;
pub const CLUSCTL_CONTROL_CODE_MASK: u32 = 4194303u32;
pub const CLUSCTL_FUNCTION_SHIFT: u32 = 2u32;
pub const CLUSCTL_GET_OPERATION_CONTEXT_PARAMS_VERSION_1: u32 = 1u32;
pub const CLUSCTL_GROUPSET_GET_COMMON_PROPERTIES: CLUSCTL_GROUPSET_CODES = 134217817i32;
pub const CLUSCTL_GROUPSET_GET_GROUPS: CLUSCTL_GROUPSET_CODES = 134229361i32;
pub const CLUSCTL_GROUPSET_GET_ID: CLUSCTL_GROUPSET_CODES = 134217785i32;
pub const CLUSCTL_GROUPSET_GET_PROVIDER_GROUPS: CLUSCTL_GROUPSET_CODES = 134229365i32;
pub const CLUSCTL_GROUPSET_GET_PROVIDER_GROUPSETS: CLUSCTL_GROUPSET_CODES = 134229369i32;
pub const CLUSCTL_GROUPSET_GET_RO_COMMON_PROPERTIES: CLUSCTL_GROUPSET_CODES = 134217813i32;
pub const CLUSCTL_GROUPSET_SET_COMMON_PROPERTIES: CLUSCTL_GROUPSET_CODES = 138412126i32;
pub const CLUSCTL_GROUP_ENUM_COMMON_PROPERTIES: CLUSCTL_GROUP_CODES = 50331729i32;
pub const CLUSCTL_GROUP_ENUM_PRIVATE_PROPERTIES: CLUSCTL_GROUP_CODES = 50331769i32;
pub const CLUSCTL_GROUP_GET_CHARACTERISTICS: CLUSCTL_GROUP_CODES = 50331653i32;
pub const CLUSCTL_GROUP_GET_COMMON_PROPERTIES: CLUSCTL_GROUP_CODES = 50331737i32;
pub const CLUSCTL_GROUP_GET_COMMON_PROPERTY_FMTS: CLUSCTL_GROUP_CODES = 50331749i32;
pub const CLUSCTL_GROUP_GET_FAILURE_INFO: CLUSCTL_GROUP_CODES = 50331673i32;
pub const CLUSCTL_GROUP_GET_FLAGS: CLUSCTL_GROUP_CODES = 50331657i32;
pub const CLUSCTL_GROUP_GET_ID: CLUSCTL_GROUP_CODES = 50331705i32;
pub const CLUSCTL_GROUP_GET_LAST_MOVE_TIME: CLUSCTL_GROUP_CODES = 50332377i32;
pub const CLUSCTL_GROUP_GET_NAME: CLUSCTL_GROUP_CODES = 50331689i32;
pub const CLUSCTL_GROUP_GET_PRIVATE_PROPERTIES: CLUSCTL_GROUP_CODES = 50331777i32;
pub const CLUSCTL_GROUP_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_GROUP_CODES = 50331789i32;
pub const CLUSCTL_GROUP_GET_PROVIDER_GROUPS: CLUSCTL_GROUPSET_CODES = 134229373i32;
pub const CLUSCTL_GROUP_GET_PROVIDER_GROUPSETS: CLUSCTL_GROUPSET_CODES = 134229377i32;
pub const CLUSCTL_GROUP_GET_RO_COMMON_PROPERTIES: CLUSCTL_GROUP_CODES = 50331733i32;
pub const CLUSCTL_GROUP_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_GROUP_CODES = 50331773i32;
pub const CLUSCTL_GROUP_QUERY_DELETE: CLUSCTL_GROUP_CODES = 50332089i32;
pub const CLUSCTL_GROUP_SET_CCF_FROM_MASTER: CLUSCTL_GROUP_CODES = 54537606i32;
pub const CLUSCTL_GROUP_SET_COMMON_PROPERTIES: CLUSCTL_GROUP_CODES = 54526046i32;
pub const CLUSCTL_GROUP_SET_PRIVATE_PROPERTIES: CLUSCTL_GROUP_CODES = 54526086i32;
pub const CLUSCTL_GROUP_UNKNOWN: CLUSCTL_GROUP_CODES = 50331648i32;
pub const CLUSCTL_GROUP_VALIDATE_COMMON_PROPERTIES: CLUSCTL_GROUP_CODES = 50331745i32;
pub const CLUSCTL_GROUP_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_GROUP_CODES = 50331785i32;
pub const CLUSCTL_NETINTERFACE_ENUM_COMMON_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663377i32;
pub const CLUSCTL_NETINTERFACE_ENUM_PRIVATE_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663417i32;
pub const CLUSCTL_NETINTERFACE_GET_CHARACTERISTICS: CLUSCTL_NETINTERFACE_CODES = 100663301i32;
pub const CLUSCTL_NETINTERFACE_GET_COMMON_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663385i32;
pub const CLUSCTL_NETINTERFACE_GET_COMMON_PROPERTY_FMTS: CLUSCTL_NETINTERFACE_CODES = 100663397i32;
pub const CLUSCTL_NETINTERFACE_GET_FLAGS: CLUSCTL_NETINTERFACE_CODES = 100663305i32;
pub const CLUSCTL_NETINTERFACE_GET_ID: CLUSCTL_NETINTERFACE_CODES = 100663353i32;
pub const CLUSCTL_NETINTERFACE_GET_NAME: CLUSCTL_NETINTERFACE_CODES = 100663337i32;
pub const CLUSCTL_NETINTERFACE_GET_NETWORK: CLUSCTL_NETINTERFACE_CODES = 100663349i32;
pub const CLUSCTL_NETINTERFACE_GET_NODE: CLUSCTL_NETINTERFACE_CODES = 100663345i32;
pub const CLUSCTL_NETINTERFACE_GET_PRIVATE_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663425i32;
pub const CLUSCTL_NETINTERFACE_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_NETINTERFACE_CODES = 100663437i32;
pub const CLUSCTL_NETINTERFACE_GET_RO_COMMON_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663381i32;
pub const CLUSCTL_NETINTERFACE_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663421i32;
pub const CLUSCTL_NETINTERFACE_SET_COMMON_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 104857694i32;
pub const CLUSCTL_NETINTERFACE_SET_PRIVATE_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 104857734i32;
pub const CLUSCTL_NETINTERFACE_UNKNOWN: CLUSCTL_NETINTERFACE_CODES = 100663296i32;
pub const CLUSCTL_NETINTERFACE_VALIDATE_COMMON_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663393i32;
pub const CLUSCTL_NETINTERFACE_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_NETINTERFACE_CODES = 100663433i32;
pub const CLUSCTL_NETWORK_ENUM_COMMON_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886161i32;
pub const CLUSCTL_NETWORK_ENUM_PRIVATE_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886201i32;
pub const CLUSCTL_NETWORK_GET_CHARACTERISTICS: CLUSCTL_NETWORK_CODES = 83886085i32;
pub const CLUSCTL_NETWORK_GET_COMMON_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886169i32;
pub const CLUSCTL_NETWORK_GET_COMMON_PROPERTY_FMTS: CLUSCTL_NETWORK_CODES = 83886181i32;
pub const CLUSCTL_NETWORK_GET_FLAGS: CLUSCTL_NETWORK_CODES = 83886089i32;
pub const CLUSCTL_NETWORK_GET_ID: CLUSCTL_NETWORK_CODES = 83886137i32;
pub const CLUSCTL_NETWORK_GET_NAME: CLUSCTL_NETWORK_CODES = 83886121i32;
pub const CLUSCTL_NETWORK_GET_PRIVATE_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886209i32;
pub const CLUSCTL_NETWORK_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_NETWORK_CODES = 83886221i32;
pub const CLUSCTL_NETWORK_GET_RO_COMMON_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886165i32;
pub const CLUSCTL_NETWORK_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886205i32;
pub const CLUSCTL_NETWORK_SET_COMMON_PROPERTIES: CLUSCTL_NETWORK_CODES = 88080478i32;
pub const CLUSCTL_NETWORK_SET_PRIVATE_PROPERTIES: CLUSCTL_NETWORK_CODES = 88080518i32;
pub const CLUSCTL_NETWORK_UNKNOWN: CLUSCTL_NETWORK_CODES = 83886080i32;
pub const CLUSCTL_NETWORK_VALIDATE_COMMON_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886177i32;
pub const CLUSCTL_NETWORK_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_NETWORK_CODES = 83886217i32;
pub const CLUSCTL_NODE_BLOCK_GEM_SEND_RECV: CLUSCTL_NODE_CODES = 67109581i32;
pub const CLUSCTL_NODE_ENUM_COMMON_PROPERTIES: CLUSCTL_NODE_CODES = 67108945i32;
pub const CLUSCTL_NODE_ENUM_PRIVATE_PROPERTIES: CLUSCTL_NODE_CODES = 67108985i32;
pub const CLUSCTL_NODE_GET_CHARACTERISTICS: CLUSCTL_NODE_CODES = 67108869i32;
pub const CLUSCTL_NODE_GET_CLUSTER_SERVICE_ACCOUNT_NAME: CLUSCTL_NODE_CODES = 67108929i32;
pub const CLUSCTL_NODE_GET_COMMON_PROPERTIES: CLUSCTL_NODE_CODES = 67108953i32;
pub const CLUSCTL_NODE_GET_COMMON_PROPERTY_FMTS: CLUSCTL_NODE_CODES = 67108965i32;
pub const CLUSCTL_NODE_GET_FLAGS: CLUSCTL_NODE_CODES = 67108873i32;
pub const CLUSCTL_NODE_GET_GEMID_VECTOR: CLUSCTL_NODE_CODES = 67109585i32;
pub const CLUSCTL_NODE_GET_ID: CLUSCTL_NODE_CODES = 67108921i32;
pub const CLUSCTL_NODE_GET_NAME: CLUSCTL_NODE_CODES = 67108905i32;
pub const CLUSCTL_NODE_GET_PRIVATE_PROPERTIES: CLUSCTL_NODE_CODES = 67108993i32;
pub const CLUSCTL_NODE_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_NODE_CODES = 67109005i32;
pub const CLUSCTL_NODE_GET_RO_COMMON_PROPERTIES: CLUSCTL_NODE_CODES = 67108949i32;
pub const CLUSCTL_NODE_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_NODE_CODES = 67108989i32;
pub const CLUSCTL_NODE_GET_STUCK_NODES: CLUSCTL_NODE_CODES = 67109565i32;
pub const CLUSCTL_NODE_INJECT_GEM_FAULT: CLUSCTL_NODE_CODES = 67109569i32;
pub const CLUSCTL_NODE_INTRODUCE_GEM_REPAIR_DELAY: CLUSCTL_NODE_CODES = 67109573i32;
pub const CLUSCTL_NODE_SEND_DUMMY_GEM_MESSAGES: CLUSCTL_NODE_CODES = 67109577i32;
pub const CLUSCTL_NODE_SET_COMMON_PROPERTIES: CLUSCTL_NODE_CODES = 71303262i32;
pub const CLUSCTL_NODE_SET_PRIVATE_PROPERTIES: CLUSCTL_NODE_CODES = 71303302i32;
pub const CLUSCTL_NODE_UNKNOWN: CLUSCTL_NODE_CODES = 67108864i32;
pub const CLUSCTL_NODE_VALIDATE_COMMON_PROPERTIES: CLUSCTL_NODE_CODES = 67108961i32;
pub const CLUSCTL_NODE_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_NODE_CODES = 67109001i32;
pub const CLUSCTL_OBJECT_MASK: u32 = 255u32;
pub const CLUSCTL_OBJECT_SHIFT: u32 = 24u32;
pub const CLUSCTL_RESOURCE_ADD_CRYPTO_CHECKPOINT: CLUSCTL_RESOURCE_CODES = 20971694i32;
pub const CLUSCTL_RESOURCE_ADD_CRYPTO_CHECKPOINT_EX: CLUSCTL_RESOURCE_CODES = 20972246i32;
pub const CLUSCTL_RESOURCE_ADD_DEPENDENCY: CLUSCTL_RESOURCE_CODES = 22020114i32;
pub const CLUSCTL_RESOURCE_ADD_OWNER: CLUSCTL_RESOURCE_CODES = 22020122i32;
pub const CLUSCTL_RESOURCE_ADD_REGISTRY_CHECKPOINT: CLUSCTL_RESOURCE_CODES = 20971682i32;
pub const CLUSCTL_RESOURCE_ADD_REGISTRY_CHECKPOINT_32BIT: CLUSCTL_RESOURCE_CODES = 20971714i32;
pub const CLUSCTL_RESOURCE_ADD_REGISTRY_CHECKPOINT_64BIT: CLUSCTL_RESOURCE_CODES = 20971710i32;
pub const CLUSCTL_RESOURCE_CHECK_DRAIN_VETO: CLUSCTL_RESOURCE_CODES = 17834285i32;
pub const CLUSCTL_RESOURCE_CLUSTER_NAME_CHANGED: CLUSCTL_RESOURCE_CODES = 22020138i32;
pub const CLUSCTL_RESOURCE_CLUSTER_VERSION_CHANGED: CLUSCTL_RESOURCE_CODES = 22020142i32;
pub const CLUSCTL_RESOURCE_DELETE: CLUSCTL_RESOURCE_CODES = 22020102i32;
pub const CLUSCTL_RESOURCE_DELETE_CRYPTO_CHECKPOINT: CLUSCTL_RESOURCE_CODES = 20971698i32;
pub const CLUSCTL_RESOURCE_DELETE_REGISTRY_CHECKPOINT: CLUSCTL_RESOURCE_CODES = 20971686i32;
pub const CLUSCTL_RESOURCE_DISABLE_SHARED_VOLUME_DIRECTIO: CLUSCTL_RESOURCE_CODES = 20972174i32;
pub const CLUSCTL_RESOURCE_ENABLE_SHARED_VOLUME_DIRECTIO: CLUSCTL_RESOURCE_CODES = 20972170i32;
pub const CLUSCTL_RESOURCE_ENUM_COMMON_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777297i32;
pub const CLUSCTL_RESOURCE_ENUM_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777337i32;
pub const CLUSCTL_RESOURCE_EVICT_NODE: CLUSCTL_RESOURCE_CODES = 22020110i32;
pub const CLUSCTL_RESOURCE_FORCE_QUORUM: CLUSCTL_RESOURCE_CODES = 22020166i32;
pub const CLUSCTL_RESOURCE_FSWITNESS_GET_EPOCH_INFO: CLUSCTL_RESOURCE_CODES = 17825885i32;
pub const CLUSCTL_RESOURCE_FSWITNESS_RELEASE_LOCK: CLUSCTL_RESOURCE_CODES = 22020198i32;
pub const CLUSCTL_RESOURCE_FSWITNESS_SET_EPOCH_INFO: CLUSCTL_RESOURCE_CODES = 22020194i32;
pub const CLUSCTL_RESOURCE_GET_CHARACTERISTICS: CLUSCTL_RESOURCE_CODES = 16777221i32;
pub const CLUSCTL_RESOURCE_GET_CLASS_INFO: CLUSCTL_RESOURCE_CODES = 16777229i32;
pub const CLUSCTL_RESOURCE_GET_COMMON_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777305i32;
pub const CLUSCTL_RESOURCE_GET_COMMON_PROPERTY_FMTS: CLUSCTL_RESOURCE_CODES = 16777317i32;
pub const CLUSCTL_RESOURCE_GET_CRYPTO_CHECKPOINTS: CLUSCTL_RESOURCE_CODES = 16777397i32;
pub const CLUSCTL_RESOURCE_GET_DNS_NAME: CLUSCTL_RESOURCE_CODES = 16777589i32;
pub const CLUSCTL_RESOURCE_GET_FAILURE_INFO: CLUSCTL_RESOURCE_CODES = 16777241i32;
pub const CLUSCTL_RESOURCE_GET_FLAGS: CLUSCTL_RESOURCE_CODES = 16777225i32;
pub const CLUSCTL_RESOURCE_GET_ID: CLUSCTL_RESOURCE_CODES = 16777273i32;
pub const CLUSCTL_RESOURCE_GET_INFRASTRUCTURE_SOFS_BUFFER: CLUSCTL_RESOURCE_CODES = 16788873i32;
pub const CLUSCTL_RESOURCE_GET_LOADBAL_PROCESS_LIST: CLUSCTL_RESOURCE_CODES = 16777417i32;
pub const CLUSCTL_RESOURCE_GET_NAME: CLUSCTL_RESOURCE_CODES = 16777257i32;
pub const CLUSCTL_RESOURCE_GET_NETWORK_NAME: CLUSCTL_RESOURCE_CODES = 16777577i32;
pub const CLUSCTL_RESOURCE_GET_NODES_IN_FD: CLUSCTL_RESOURCE_CODES = 16788961i32;
pub const CLUSCTL_RESOURCE_GET_OPERATION_CONTEXT: CLUSCTL_RESOURCE_CODES = 17834217i32;
pub const CLUSCTL_RESOURCE_GET_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777345i32;
pub const CLUSCTL_RESOURCE_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_RESOURCE_CODES = 16777357i32;
pub const CLUSCTL_RESOURCE_GET_REGISTRY_CHECKPOINTS: CLUSCTL_RESOURCE_CODES = 16777385i32;
pub const CLUSCTL_RESOURCE_GET_REQUIRED_DEPENDENCIES: CLUSCTL_RESOURCE_CODES = 16777233i32;
pub const CLUSCTL_RESOURCE_GET_RESOURCE_TYPE: CLUSCTL_RESOURCE_CODES = 16777261i32;
pub const CLUSCTL_RESOURCE_GET_RO_COMMON_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777301i32;
pub const CLUSCTL_RESOURCE_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777341i32;
pub const CLUSCTL_RESOURCE_GET_STATE_CHANGE_TIME: CLUSCTL_RESOURCE_CODES = 16788829i32;
pub const CLUSCTL_RESOURCE_INITIALIZE: CLUSCTL_RESOURCE_CODES = 22020170i32;
pub const CLUSCTL_RESOURCE_INSTALL_NODE: CLUSCTL_RESOURCE_CODES = 22020106i32;
pub const CLUSCTL_RESOURCE_IPADDRESS_RELEASE_LEASE: CLUSCTL_RESOURCE_CODES = 20971970i32;
pub const CLUSCTL_RESOURCE_IPADDRESS_RENEW_LEASE: CLUSCTL_RESOURCE_CODES = 20971966i32;
pub const CLUSCTL_RESOURCE_IS_QUORUM_BLOCKED: CLUSCTL_RESOURCE_CODES = 16777905i32;
pub const CLUSCTL_RESOURCE_JOINING_GROUP: CLUSCTL_RESOURCE_CODES = 22020186i32;
pub const CLUSCTL_RESOURCE_LEAVING_GROUP: CLUSCTL_RESOURCE_CODES = 22020182i32;
pub const CLUSCTL_RESOURCE_NETNAME_CREDS_NOTIFYCAM: CLUSCTL_RESOURCE_CODES = 22020202i32;
pub const CLUSCTL_RESOURCE_NETNAME_DELETE_CO: CLUSCTL_RESOURCE_CODES = 16777598i32;
pub const CLUSCTL_RESOURCE_NETNAME_GET_VIRTUAL_SERVER_TOKEN: CLUSCTL_RESOURCE_CODES = 16777581i32;
pub const CLUSCTL_RESOURCE_NETNAME_REGISTER_DNS_RECORDS: CLUSCTL_RESOURCE_CODES = 16777586i32;
pub const CLUSCTL_RESOURCE_NETNAME_REPAIR_VCO: CLUSCTL_RESOURCE_CODES = 16777613i32;
pub const CLUSCTL_RESOURCE_NETNAME_RESET_VCO: CLUSCTL_RESOURCE_CODES = 16777605i32;
pub const CLUSCTL_RESOURCE_NETNAME_SET_PWD_INFO: CLUSCTL_RESOURCE_CODES = 16777594i32;
pub const CLUSCTL_RESOURCE_NETNAME_SET_PWD_INFOEX: CLUSCTL_RESOURCE_CODES = 16778010i32;
pub const CLUSCTL_RESOURCE_NETNAME_VALIDATE_VCO: CLUSCTL_RESOURCE_CODES = 16777601i32;
pub const CLUSCTL_RESOURCE_NOTIFY_DRAIN_COMPLETE: CLUSCTL_RESOURCE_CODES = 17834289i32;
pub const CLUSCTL_RESOURCE_NOTIFY_OWNER_CHANGE: CLUSCTL_RESOURCE_CODES = 22028578i32;
pub const CLUSCTL_RESOURCE_NOTIFY_QUORUM_STATUS: CLUSCTL_RESOURCE_CODES = 22020222i32;
pub const CLUSCTL_RESOURCE_POOL_GET_DRIVE_INFO: CLUSCTL_RESOURCE_CODES = 16777909i32;
pub const CLUSCTL_RESOURCE_PREPARE_UPGRADE: CLUSCTL_RESOURCE_CODES = 20979946i32;
pub const CLUSCTL_RESOURCE_PROVIDER_STATE_CHANGE: CLUSCTL_RESOURCE_CODES = 22020178i32;
pub const CLUSCTL_RESOURCE_QUERY_DELETE: CLUSCTL_RESOURCE_CODES = 16777657i32;
pub const CLUSCTL_RESOURCE_QUERY_MAINTENANCE_MODE: CLUSCTL_RESOURCE_CODES = 16777697i32;
pub const CLUSCTL_RESOURCE_REMOVE_DEPENDENCY: CLUSCTL_RESOURCE_CODES = 22020118i32;
pub const CLUSCTL_RESOURCE_REMOVE_OWNER: CLUSCTL_RESOURCE_CODES = 22020126i32;
pub const CLUSCTL_RESOURCE_RLUA_GET_VIRTUAL_SERVER_TOKEN: CLUSCTL_RESOURCE_CODES = 16777581i32;
pub const CLUSCTL_RESOURCE_RLUA_SET_PWD_INFO: CLUSCTL_RESOURCE_CODES = 16777594i32;
pub const CLUSCTL_RESOURCE_RLUA_SET_PWD_INFOEX: CLUSCTL_RESOURCE_CODES = 16778010i32;
pub const CLUSCTL_RESOURCE_RW_MODIFY_NOOP: CLUSCTL_RESOURCE_CODES = 20972206i32;
pub const CLUSCTL_RESOURCE_SCALEOUT_COMMAND: CLUSCTL_RESOURCE_CODES = 20983190i32;
pub const CLUSCTL_RESOURCE_SCALEOUT_CONTROL: CLUSCTL_RESOURCE_CODES = 20983194i32;
pub const CLUSCTL_RESOURCE_SCALEOUT_GET_CLUSTERS: CLUSCTL_RESOURCE_CODES = 20983197i32;
pub const CLUSCTL_RESOURCE_SET_COMMON_PROPERTIES: CLUSCTL_RESOURCE_CODES = 20971614i32;
pub const CLUSCTL_RESOURCE_SET_CSV_MAINTENANCE_MODE: CLUSCTL_RESOURCE_CODES = 20972182i32;
pub const CLUSCTL_RESOURCE_SET_INFRASTRUCTURE_SOFS_BUFFER: CLUSCTL_RESOURCE_CODES = 20983182i32;
pub const CLUSCTL_RESOURCE_SET_MAINTENANCE_MODE: CLUSCTL_RESOURCE_CODES = 20972006i32;
pub const CLUSCTL_RESOURCE_SET_NAME: CLUSCTL_RESOURCE_CODES = 22020134i32;
pub const CLUSCTL_RESOURCE_SET_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_CODES = 20971654i32;
pub const CLUSCTL_RESOURCE_SET_SHARED_VOLUME_BACKUP_MODE: CLUSCTL_RESOURCE_CODES = 20972186i32;
pub const CLUSCTL_RESOURCE_STATE_CHANGE_REASON: CLUSCTL_RESOURCE_CODES = 22020174i32;
pub const CLUSCTL_RESOURCE_STATE_CHANGE_REASON_VERSION_1: u32 = 1u32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_DIRTY: CLUSCTL_RESOURCE_CODES = 16777753i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_DISKID: CLUSCTL_RESOURCE_CODES = 16777733i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_DISK_INFO: CLUSCTL_RESOURCE_CODES = 16777617i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_DISK_INFO_EX: CLUSCTL_RESOURCE_CODES = 16777713i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_DISK_INFO_EX2: CLUSCTL_RESOURCE_CODES = 16777721i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_DISK_NUMBER_INFO: CLUSCTL_RESOURCE_CODES = 16777633i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_MOUNTPOINTS: CLUSCTL_RESOURCE_CODES = 16777745i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_SHARED_VOLUME_INFO: CLUSCTL_RESOURCE_CODES = 16777765i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_SHARED_VOLUME_PARTITION_NAMES: CLUSCTL_RESOURCE_CODES = 16777885i32;
pub const CLUSCTL_RESOURCE_STORAGE_GET_SHARED_VOLUME_STATES: CLUSCTL_RESOURCE_CODES = 20972194i32;
pub const CLUSCTL_RESOURCE_STORAGE_IS_PATH_VALID: CLUSCTL_RESOURCE_CODES = 16777625i32;
pub const CLUSCTL_RESOURCE_STORAGE_IS_SHARED_VOLUME: CLUSCTL_RESOURCE_CODES = 16777893i32;
pub const CLUSCTL_RESOURCE_STORAGE_RENAME_SHARED_VOLUME: CLUSCTL_RESOURCE_CODES = 16788950i32;
pub const CLUSCTL_RESOURCE_STORAGE_RENAME_SHARED_VOLUME_GUID: CLUSCTL_RESOURCE_CODES = 16788954i32;
pub const CLUSCTL_RESOURCE_STORAGE_SET_DRIVELETTER: CLUSCTL_RESOURCE_CODES = 20972010i32;
pub const CLUSCTL_RESOURCE_TYPE_CHECK_DRAIN_VETO: CLUSCTL_RESOURCE_TYPE_CODES = 34611501i32;
pub const CLUSCTL_RESOURCE_TYPE_CLUSTER_VERSION_CHANGED: CLUSCTL_RESOURCE_TYPE_CODES = 38797358i32;
pub const CLUSCTL_RESOURCE_TYPE_ENUM_COMMON_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554513i32;
pub const CLUSCTL_RESOURCE_TYPE_ENUM_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554553i32;
pub const CLUSCTL_RESOURCE_TYPE_EVICT_NODE: CLUSCTL_RESOURCE_TYPE_CODES = 38797326i32;
pub const CLUSCTL_RESOURCE_TYPE_FIXUP_ON_UPGRADE: CLUSCTL_RESOURCE_TYPE_CODES = 38797362i32;
pub const CLUSCTL_RESOURCE_TYPE_GEN_APP_VALIDATE_DIRECTORY: CLUSCTL_RESOURCE_TYPE_CODES = 33555001i32;
pub const CLUSCTL_RESOURCE_TYPE_GEN_APP_VALIDATE_PATH: CLUSCTL_RESOURCE_TYPE_CODES = 33554993i32;
pub const CLUSCTL_RESOURCE_TYPE_GEN_SCRIPT_VALIDATE_PATH: CLUSCTL_RESOURCE_TYPE_CODES = 33554993i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_ARB_TIMEOUT: CLUSCTL_RESOURCE_TYPE_CODES = 33554453i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_CHARACTERISTICS: CLUSCTL_RESOURCE_TYPE_CODES = 33554437i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_CLASS_INFO: CLUSCTL_RESOURCE_TYPE_CODES = 33554445i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_COMMON_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554521i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_COMMON_PROPERTY_FMTS: CLUSCTL_RESOURCE_TYPE_CODES = 33554533i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_COMMON_RESOURCE_PROPERTY_FMTS: CLUSCTL_RESOURCE_TYPE_CODES = 33554537i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_CRYPTO_CHECKPOINTS: CLUSCTL_RESOURCE_TYPE_CODES = 33554613i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_FLAGS: CLUSCTL_RESOURCE_TYPE_CODES = 33554441i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554561i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_PRIVATE_PROPERTY_FMTS: CLUSCTL_RESOURCE_TYPE_CODES = 33554573i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_PRIVATE_RESOURCE_PROPERTY_FMTS: CLUSCTL_RESOURCE_TYPE_CODES = 33554577i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_REGISTRY_CHECKPOINTS: CLUSCTL_RESOURCE_TYPE_CODES = 33554601i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_REQUIRED_DEPENDENCIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554449i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_RO_COMMON_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554517i32;
pub const CLUSCTL_RESOURCE_TYPE_GET_RO_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554557i32;
pub const CLUSCTL_RESOURCE_TYPE_HOLD_IO: CLUSCTL_RESOURCE_TYPE_CODES = 38797374i32;
pub const CLUSCTL_RESOURCE_TYPE_INSTALL_NODE: CLUSCTL_RESOURCE_TYPE_CODES = 38797322i32;
pub const CLUSCTL_RESOURCE_TYPE_NETNAME_GET_OU_FOR_VCO: CLUSCTL_RESOURCE_TYPE_CODES = 37749358i32;
pub const CLUSCTL_RESOURCE_TYPE_NETNAME_VALIDATE_NETNAME: CLUSCTL_RESOURCE_TYPE_CODES = 33554997i32;
pub const CLUSCTL_RESOURCE_TYPE_NOTIFY_DRAIN_COMPLETE: CLUSCTL_RESOURCE_TYPE_CODES = 34611505i32;
pub const CLUSCTL_RESOURCE_TYPE_NOTIFY_MONITOR_SHUTTING_DOWN: CLUSCTL_RESOURCE_TYPE_CODES = 34603137i32;
pub const CLUSCTL_RESOURCE_TYPE_PREPARE_UPGRADE: CLUSCTL_RESOURCE_TYPE_CODES = 37757162i32;
pub const CLUSCTL_RESOURCE_TYPE_QUERY_DELETE: CLUSCTL_RESOURCE_TYPE_CODES = 33554873i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_ADD_REPLICATION_GROUP: CLUSCTL_RESOURCE_TYPE_CODES = 33562946i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_ELIGIBLE_LOGDISKS: CLUSCTL_RESOURCE_TYPE_CODES = 33562953i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_ELIGIBLE_SOURCE_DATADISKS: CLUSCTL_RESOURCE_TYPE_CODES = 33562961i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_ELIGIBLE_TARGET_DATADISKS: CLUSCTL_RESOURCE_TYPE_CODES = 33562957i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_LOG_INFO: CLUSCTL_RESOURCE_TYPE_CODES = 33562949i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_LOG_VOLUME: CLUSCTL_RESOURCE_TYPE_CODES = 33562973i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_REPLICATED_DISKS: CLUSCTL_RESOURCE_TYPE_CODES = 33562965i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_REPLICATED_PARTITION_INFO: CLUSCTL_RESOURCE_TYPE_CODES = 33562981i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_REPLICA_VOLUMES: CLUSCTL_RESOURCE_TYPE_CODES = 33562969i32;
pub const CLUSCTL_RESOURCE_TYPE_REPLICATION_GET_RESOURCE_GROUP: CLUSCTL_RESOURCE_TYPE_CODES = 33562977i32;
pub const CLUSCTL_RESOURCE_TYPE_RESUME_IO: CLUSCTL_RESOURCE_TYPE_CODES = 38797378i32;
pub const CLUSCTL_RESOURCE_TYPE_SET_COMMON_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 37748830i32;
pub const CLUSCTL_RESOURCE_TYPE_SET_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 37748870i32;
pub const CLUSCTL_RESOURCE_TYPE_STARTING_PHASE1: CLUSCTL_RESOURCE_TYPE_CODES = 38797366i32;
pub const CLUSCTL_RESOURCE_TYPE_STARTING_PHASE2: CLUSCTL_RESOURCE_TYPE_CODES = 38797370i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS: CLUSCTL_RESOURCE_TYPE_CODES = 33554837i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX: CLUSCTL_RESOURCE_TYPE_CODES = 33554933i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_FLAG_ADD_VOLUME_INFO: u32 = 1u32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_FLAG_FILTER_BY_POOL: u32 = 2u32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_FLAG_INCLUDE_NON_SHARED_DISKS: u32 = 4u32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_INT: CLUSCTL_RESOURCE_TYPE_CODES = 33562593i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_DISKID: CLUSCTL_RESOURCE_TYPE_CODES = 33554949i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_DRIVELETTERS: CLUSCTL_RESOURCE_TYPE_CODES = 33554925i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_GET_RESOURCEID: CLUSCTL_RESOURCE_TYPE_CODES = 33554989i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_IS_CLUSTERABLE: CLUSCTL_RESOURCE_TYPE_CODES = 33554953i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_IS_CSV_FILE: CLUSCTL_RESOURCE_TYPE_CODES = 16777769i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_REMAP_DRIVELETTER: CLUSCTL_RESOURCE_TYPE_CODES = 33554945i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_REMOVE_VM_OWNERSHIP: CLUSCTL_RESOURCE_TYPE_CODES = 37749262i32;
pub const CLUSCTL_RESOURCE_TYPE_STORAGE_SYNC_CLUSDISK_DB: CLUSCTL_RESOURCE_TYPE_CODES = 37749150i32;
pub const CLUSCTL_RESOURCE_TYPE_UNKNOWN: CLUSCTL_RESOURCE_TYPE_CODES = 33554432i32;
pub const CLUSCTL_RESOURCE_TYPE_UPGRADE_COMPLETED: CLUSCTL_RESOURCE_TYPE_CODES = 37757166i32;
pub const CLUSCTL_RESOURCE_TYPE_VALIDATE_COMMON_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554529i32;
pub const CLUSCTL_RESOURCE_TYPE_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_TYPE_CODES = 33554569i32;
pub const CLUSCTL_RESOURCE_TYPE_WITNESS_VALIDATE_PATH: CLUSCTL_RESOURCE_TYPE_CODES = 33554993i32;
pub const CLUSCTL_RESOURCE_UNDELETE: CLUSCTL_RESOURCE_CODES = 22020230i32;
pub const CLUSCTL_RESOURCE_UNKNOWN: CLUSCTL_RESOURCE_CODES = 16777216i32;
pub const CLUSCTL_RESOURCE_UPGRADE_COMPLETED: CLUSCTL_RESOURCE_CODES = 20979950i32;
pub const CLUSCTL_RESOURCE_UPGRADE_DLL: CLUSCTL_RESOURCE_CODES = 20971706i32;
pub const CLUSCTL_RESOURCE_VALIDATE_CHANGE_GROUP: CLUSCTL_RESOURCE_CODES = 17834277i32;
pub const CLUSCTL_RESOURCE_VALIDATE_COMMON_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777313i32;
pub const CLUSCTL_RESOURCE_VALIDATE_PRIVATE_PROPERTIES: CLUSCTL_RESOURCE_CODES = 16777353i32;
pub const CLUSGROUPSET_STATUS_APPLICATION_READY: u64 = 8u64;
pub const CLUSGROUPSET_STATUS_GROUPS_ONLINE: u64 = 2u64;
pub const CLUSGROUPSET_STATUS_GROUPS_PENDING: u64 = 1u64;
pub const CLUSGROUPSET_STATUS_OS_HEARTBEAT: u64 = 4u64;
pub const CLUSGRP_STATUS_APPLICATION_READY: u64 = 1024u64;
pub const CLUSGRP_STATUS_EMBEDDED_FAILURE: u64 = 32u64;
pub const CLUSGRP_STATUS_LOCKED_MODE: u64 = 1u64;
pub const CLUSGRP_STATUS_NETWORK_FAILURE: u64 = 128u64;
pub const CLUSGRP_STATUS_OFFLINE_DUE_TO_ANTIAFFINITY_CONFLICT: u64 = 64u64;
pub const CLUSGRP_STATUS_OFFLINE_NOT_LOCAL_DISK_OWNER: u64 = 2048u64;
pub const CLUSGRP_STATUS_OS_HEARTBEAT: u64 = 512u64;
pub const CLUSGRP_STATUS_PHYSICAL_RESOURCES_LACKING: u64 = 8u64;
pub const CLUSGRP_STATUS_PREEMPTED: u64 = 2u64;
pub const CLUSGRP_STATUS_UNMONITORED: u64 = 256u64;
pub const CLUSGRP_STATUS_WAITING_FOR_DEPENDENCIES: u64 = 4096u64;
pub const CLUSGRP_STATUS_WAITING_IN_QUEUE_FOR_MOVE: u64 = 4u64;
pub const CLUSGRP_STATUS_WAITING_TO_START: u64 = 16u64;
pub const CLUSPROP_FORMAT_BINARY: CLUSTER_PROPERTY_FORMAT = 1i32;
pub const CLUSPROP_FORMAT_DWORD: CLUSTER_PROPERTY_FORMAT = 2i32;
pub const CLUSPROP_FORMAT_EXPANDED_SZ: CLUSTER_PROPERTY_FORMAT = 8i32;
pub const CLUSPROP_FORMAT_EXPAND_SZ: CLUSTER_PROPERTY_FORMAT = 4i32;
pub const CLUSPROP_FORMAT_FILETIME: CLUSTER_PROPERTY_FORMAT = 12i32;
pub const CLUSPROP_FORMAT_LARGE_INTEGER: CLUSTER_PROPERTY_FORMAT = 10i32;
pub const CLUSPROP_FORMAT_LONG: CLUSTER_PROPERTY_FORMAT = 7i32;
pub const CLUSPROP_FORMAT_MULTI_SZ: CLUSTER_PROPERTY_FORMAT = 5i32;
pub const CLUSPROP_FORMAT_PROPERTY_LIST: CLUSTER_PROPERTY_FORMAT = 14i32;
pub const CLUSPROP_FORMAT_SECURITY_DESCRIPTOR: CLUSTER_PROPERTY_FORMAT = 9i32;
pub const CLUSPROP_FORMAT_SZ: CLUSTER_PROPERTY_FORMAT = 3i32;
pub const CLUSPROP_FORMAT_ULARGE_INTEGER: CLUSTER_PROPERTY_FORMAT = 6i32;
pub const CLUSPROP_FORMAT_UNKNOWN: CLUSTER_PROPERTY_FORMAT = 0i32;
pub const CLUSPROP_FORMAT_USER: CLUSTER_PROPERTY_FORMAT = 32768i32;
pub const CLUSPROP_FORMAT_VALUE_LIST: CLUSTER_PROPERTY_FORMAT = 13i32;
pub const CLUSPROP_FORMAT_WORD: CLUSTER_PROPERTY_FORMAT = 11i32;
pub const CLUSPROP_IPADDR_ENABLENETBIOS_DISABLED: CLUSPROP_IPADDR_ENABLENETBIOS = 0i32;
pub const CLUSPROP_IPADDR_ENABLENETBIOS_ENABLED: CLUSPROP_IPADDR_ENABLENETBIOS = 1i32;
pub const CLUSPROP_IPADDR_ENABLENETBIOS_TRACK_NIC: CLUSPROP_IPADDR_ENABLENETBIOS = 2i32;
pub const CLUSPROP_PIFLAG_DEFAULT_QUORUM: CLUSPROP_PIFLAGS = 8i32;
pub const CLUSPROP_PIFLAG_ENCRYPTION_ENABLED: CLUSPROP_PIFLAGS = 32i32;
pub const CLUSPROP_PIFLAG_RAW: CLUSPROP_PIFLAGS = 64i32;
pub const CLUSPROP_PIFLAG_REMOVABLE: CLUSPROP_PIFLAGS = 2i32;
pub const CLUSPROP_PIFLAG_STICKY: CLUSPROP_PIFLAGS = 1i32;
pub const CLUSPROP_PIFLAG_UNKNOWN: CLUSPROP_PIFLAGS = -2147483648i32;
pub const CLUSPROP_PIFLAG_USABLE: CLUSPROP_PIFLAGS = 4i32;
pub const CLUSPROP_PIFLAG_USABLE_FOR_CSV: CLUSPROP_PIFLAGS = 16i32;
pub const CLUSPROP_SYNTAX_DISK_GUID: CLUSTER_PROPERTY_SYNTAX = 720899u32;
pub const CLUSPROP_SYNTAX_DISK_NUMBER: CLUSTER_PROPERTY_SYNTAX = 458754u32;
pub const CLUSPROP_SYNTAX_DISK_SERIALNUMBER: CLUSTER_PROPERTY_SYNTAX = 655363u32;
pub const CLUSPROP_SYNTAX_DISK_SIGNATURE: CLUSTER_PROPERTY_SYNTAX = 327682u32;
pub const CLUSPROP_SYNTAX_DISK_SIZE: CLUSTER_PROPERTY_SYNTAX = 786438u32;
pub const CLUSPROP_SYNTAX_ENDMARK: CLUSTER_PROPERTY_SYNTAX = 0u32;
pub const CLUSPROP_SYNTAX_FTSET_INFO: CLUSTER_PROPERTY_SYNTAX = 589825u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_BINARY: CLUSTER_PROPERTY_SYNTAX = 65537u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_DWORD: CLUSTER_PROPERTY_SYNTAX = 65538u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_EXPANDED_SZ: CLUSTER_PROPERTY_SYNTAX = 65544u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_EXPAND_SZ: CLUSTER_PROPERTY_SYNTAX = 65540u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_FILETIME: CLUSTER_PROPERTY_SYNTAX = 65548u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_LARGE_INTEGER: CLUSTER_PROPERTY_SYNTAX = 65546u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_LONG: CLUSTER_PROPERTY_SYNTAX = 65543u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_MULTI_SZ: CLUSTER_PROPERTY_SYNTAX = 65541u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_PROPERTY_LIST: CLUSTER_PROPERTY_SYNTAX = 65550u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_SECURITY_DESCRIPTOR: CLUSTER_PROPERTY_SYNTAX = 65545u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_SZ: CLUSTER_PROPERTY_SYNTAX = 65539u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_ULARGE_INTEGER: CLUSTER_PROPERTY_SYNTAX = 65542u32;
pub const CLUSPROP_SYNTAX_LIST_VALUE_WORD: CLUSTER_PROPERTY_SYNTAX = 65547u32;
pub const CLUSPROP_SYNTAX_NAME: CLUSTER_PROPERTY_SYNTAX = 262147u32;
pub const CLUSPROP_SYNTAX_PARTITION_INFO: CLUSTER_PROPERTY_SYNTAX = 524289u32;
pub const CLUSPROP_SYNTAX_PARTITION_INFO_EX: CLUSTER_PROPERTY_SYNTAX = 851969u32;
pub const CLUSPROP_SYNTAX_PARTITION_INFO_EX2: CLUSTER_PROPERTY_SYNTAX = 917505u32;
pub const CLUSPROP_SYNTAX_RESCLASS: CLUSTER_PROPERTY_SYNTAX = 131074u32;
pub const CLUSPROP_SYNTAX_SCSI_ADDRESS: CLUSTER_PROPERTY_SYNTAX = 393218u32;
pub const CLUSPROP_SYNTAX_STORAGE_DEVICE_ID_DESCRIPTOR: CLUSTER_PROPERTY_SYNTAX = 983041u32;
pub const CLUSPROP_TYPE_DISK_GUID: CLUSTER_PROPERTY_TYPE = 11i32;
pub const CLUSPROP_TYPE_DISK_NUMBER: CLUSTER_PROPERTY_TYPE = 7i32;
pub const CLUSPROP_TYPE_DISK_SERIALNUMBER: CLUSTER_PROPERTY_TYPE = 10i32;
pub const CLUSPROP_TYPE_DISK_SIZE: CLUSTER_PROPERTY_TYPE = 12i32;
pub const CLUSPROP_TYPE_ENDMARK: CLUSTER_PROPERTY_TYPE = 0i32;
pub const CLUSPROP_TYPE_FTSET_INFO: CLUSTER_PROPERTY_TYPE = 9i32;
pub const CLUSPROP_TYPE_LIST_VALUE: CLUSTER_PROPERTY_TYPE = 1i32;
pub const CLUSPROP_TYPE_NAME: CLUSTER_PROPERTY_TYPE = 4i32;
pub const CLUSPROP_TYPE_PARTITION_INFO: CLUSTER_PROPERTY_TYPE = 8i32;
pub const CLUSPROP_TYPE_PARTITION_INFO_EX: CLUSTER_PROPERTY_TYPE = 13i32;
pub const CLUSPROP_TYPE_PARTITION_INFO_EX2: CLUSTER_PROPERTY_TYPE = 14i32;
pub const CLUSPROP_TYPE_RESCLASS: CLUSTER_PROPERTY_TYPE = 2i32;
pub const CLUSPROP_TYPE_RESERVED1: CLUSTER_PROPERTY_TYPE = 3i32;
pub const CLUSPROP_TYPE_SCSI_ADDRESS: CLUSTER_PROPERTY_TYPE = 6i32;
pub const CLUSPROP_TYPE_SIGNATURE: CLUSTER_PROPERTY_TYPE = 5i32;
pub const CLUSPROP_TYPE_STORAGE_DEVICE_ID_DESCRIPTOR: CLUSTER_PROPERTY_TYPE = 15i32;
pub const CLUSPROP_TYPE_UNKNOWN: CLUSTER_PROPERTY_TYPE = -1i32;
pub const CLUSPROP_TYPE_USER: CLUSTER_PROPERTY_TYPE = 32768i32;
pub const CLUSREG_COMMAND_NONE: CLUSTER_REG_COMMAND = 0i32;
pub const CLUSREG_CONDITION_EXISTS: CLUSTER_REG_COMMAND = 11i32;
pub const CLUSREG_CONDITION_IS_EQUAL: CLUSTER_REG_COMMAND = 13i32;
pub const CLUSREG_CONDITION_IS_GREATER_THAN: CLUSTER_REG_COMMAND = 15i32;
pub const CLUSREG_CONDITION_IS_LESS_THAN: CLUSTER_REG_COMMAND = 16i32;
pub const CLUSREG_CONDITION_IS_NOT_EQUAL: CLUSTER_REG_COMMAND = 14i32;
pub const CLUSREG_CONDITION_KEY_EXISTS: CLUSTER_REG_COMMAND = 17i32;
pub const CLUSREG_CONDITION_KEY_NOT_EXISTS: CLUSTER_REG_COMMAND = 18i32;
pub const CLUSREG_CONDITION_NOT_EXISTS: CLUSTER_REG_COMMAND = 12i32;
pub const CLUSREG_CONTROL_COMMAND: CLUSTER_REG_COMMAND = 10i32;
pub const CLUSREG_CREATE_KEY: CLUSTER_REG_COMMAND = 2i32;
pub const CLUSREG_DATABASE_ISOLATE_READ: u32 = 2u32;
pub const CLUSREG_DATABASE_SYNC_WRITE_TO_ALL_NODES: u32 = 1u32;
pub const CLUSREG_DELETE_KEY: CLUSTER_REG_COMMAND = 3i32;
pub const CLUSREG_DELETE_VALUE: CLUSTER_REG_COMMAND = 4i32;
pub const CLUSREG_KEYNAME_OBJECTGUIDS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ObjectGUIDs");
pub const CLUSREG_LAST_COMMAND: CLUSTER_REG_COMMAND = 19i32;
pub const CLUSREG_NAME_AFFINITYRULE_ENABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Enabled");
pub const CLUSREG_NAME_AFFINITYRULE_GROUPS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Groups");
pub const CLUSREG_NAME_AFFINITYRULE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_AFFINITYRULE_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RuleType");
pub const CLUSREG_NAME_CLOUDWITNESS_ACCOUNT_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AccountName");
pub const CLUSREG_NAME_CLOUDWITNESS_CONTAINER_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ContainerName");
pub const CLUSREG_NAME_CLOUDWITNESS_ENDPOINT_INFO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EndpointInfo");
pub const CLUSREG_NAME_CLOUDWITNESS_PRIMARY_KEY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PrimaryKey");
pub const CLUSREG_NAME_CLOUDWITNESS_PRIMARY_TOKEN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PrimaryToken");
pub const CLUSREG_NAME_CLUS_DEFAULT_NETWORK_ROLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultNetworkRole");
pub const CLUSREG_NAME_CLUS_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_CLUS_SD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Security Descriptor");
pub const CLUSREG_NAME_CROSS_SITE_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CrossSiteDelay");
pub const CLUSREG_NAME_CROSS_SITE_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CrossSiteThreshold");
pub const CLUSREG_NAME_CROSS_SUBNET_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CrossSubnetDelay");
pub const CLUSREG_NAME_CROSS_SUBNET_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CrossSubnetThreshold");
pub const CLUSREG_NAME_CSV_BLOCK_CACHE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BlockCacheSize");
pub const CLUSREG_NAME_CSV_MDS_SD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharedVolumeSecurityDescriptor");
pub const CLUSREG_NAME_DATABASE_READ_WRITE_MODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DatabaseReadWriteMode");
pub const CLUSREG_NAME_DDA_DEVICE_ALLOCATIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DdaDeviceAllocations");
pub const CLUSREG_NAME_DHCP_BACKUP_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BackupPath");
pub const CLUSREG_NAME_DHCP_DATABASE_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DatabasePath");
pub const CLUSREG_NAME_DRAIN_ON_SHUTDOWN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DrainOnShutdown");
pub const CLUSREG_NAME_ENABLED_EVENT_LOGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnabledEventLogs");
pub const CLUSREG_NAME_FAILOVER_MOVE_MIGRATION_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailoverMoveMigrationType");
pub const CLUSREG_NAME_FILESHR_CA_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CATimeout");
pub const CLUSREG_NAME_FILESHR_HIDE_SUBDIR_SHARES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HideSubDirShares");
pub const CLUSREG_NAME_FILESHR_IS_DFS_ROOT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsDfsRoot");
pub const CLUSREG_NAME_FILESHR_MAX_USERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaxUsers");
pub const CLUSREG_NAME_FILESHR_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Path");
pub const CLUSREG_NAME_FILESHR_QOS_FLOWSCOPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("QosFlowScope");
pub const CLUSREG_NAME_FILESHR_QOS_POLICYID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("QosPolicyId");
pub const CLUSREG_NAME_FILESHR_REMARK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Remark");
pub const CLUSREG_NAME_FILESHR_SD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Security Descriptor");
pub const CLUSREG_NAME_FILESHR_SERVER_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ServerName");
pub const CLUSREG_NAME_FILESHR_SHARE_FLAGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ShareFlags");
pub const CLUSREG_NAME_FILESHR_SHARE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ShareName");
pub const CLUSREG_NAME_FILESHR_SHARE_SUBDIRS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ShareSubDirs");
pub const CLUSREG_NAME_FIXQUORUM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FixQuorum");
pub const CLUSREG_NAME_FSWITNESS_ARB_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ArbitrationDelay");
pub const CLUSREG_NAME_FSWITNESS_IMPERSONATE_CNO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ImpersonateCNO");
pub const CLUSREG_NAME_FSWITNESS_SHARE_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharePath");
pub const CLUSREG_NAME_FUNCTIONAL_LEVEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterFunctionalLevel");
pub const CLUSREG_NAME_GENAPP_COMMAND_LINE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CommandLine");
pub const CLUSREG_NAME_GENAPP_CURRENT_DIRECTORY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CurrentDirectory");
pub const CLUSREG_NAME_GENAPP_USE_NETWORK_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UseNetworkName");
pub const CLUSREG_NAME_GENSCRIPT_SCRIPT_FILEPATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ScriptFilepath");
pub const CLUSREG_NAME_GENSVC_SERVICE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ServiceName");
pub const CLUSREG_NAME_GENSVC_STARTUP_PARAMS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartupParameters");
pub const CLUSREG_NAME_GENSVC_USE_NETWORK_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UseNetworkName");
pub const CLUSREG_NAME_GPUP_DEVICE_ALLOCATIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GpupDeviceAllocations");
pub const CLUSREG_NAME_GROUPSET_AVAILABILITY_SET_INDEX_TO_NODE_MAPPING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeDomainInfo");
pub const CLUSREG_NAME_GROUPSET_FAULT_DOMAINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FaultDomains");
pub const CLUSREG_NAME_GROUPSET_IS_AVAILABILITY_SET: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsAvailabilitySet");
pub const CLUSREG_NAME_GROUPSET_IS_GLOBAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsGlobal");
pub const CLUSREG_NAME_GROUPSET_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_GROUPSET_RESERVE_NODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ReserveSpareNode");
pub const CLUSREG_NAME_GROUPSET_STARTUP_COUNT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartupCount");
pub const CLUSREG_NAME_GROUPSET_STARTUP_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartupDelay");
pub const CLUSREG_NAME_GROUPSET_STARTUP_SETTING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartupSetting");
pub const CLUSREG_NAME_GROUPSET_STATUS_INFORMATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusInformation");
pub const CLUSREG_NAME_GROUPSET_UPDATE_DOMAINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UpdateDomains");
pub const CLUSREG_NAME_GROUP_DEPENDENCY_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GroupDependencyTimeout");
pub const CLUSREG_NAME_GRP_ANTI_AFFINITY_CLASS_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AntiAffinityClassNames");
pub const CLUSREG_NAME_GRP_CCF_EPOCH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CCFEpoch");
pub const CLUSREG_NAME_GRP_CCF_EPOCH_HIGH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CCFEpochHigh");
pub const CLUSREG_NAME_GRP_COLD_START_SETTING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ColdStartSetting");
pub const CLUSREG_NAME_GRP_DEFAULT_OWNER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultOwner");
pub const CLUSREG_NAME_GRP_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_GRP_FAILBACK_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AutoFailbackType");
pub const CLUSREG_NAME_GRP_FAILBACK_WIN_END: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailbackWindowEnd");
pub const CLUSREG_NAME_GRP_FAILBACK_WIN_START: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailbackWindowStart");
pub const CLUSREG_NAME_GRP_FAILOVER_PERIOD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailoverPeriod");
pub const CLUSREG_NAME_GRP_FAILOVER_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FailoverThreshold");
pub const CLUSREG_NAME_GRP_FAULT_DOMAIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FaultDomain");
pub const CLUSREG_NAME_GRP_LOCK_MOVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LockedFromMoving");
pub const CLUSREG_NAME_GRP_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_GRP_PERSISTENT_STATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PersistentState");
pub const CLUSREG_NAME_GRP_PLACEMENT_OPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PlacementOptions");
pub const CLUSREG_NAME_GRP_PREFERRED_SITE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PreferredSite");
pub const CLUSREG_NAME_GRP_PRIORITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Priority");
pub const CLUSREG_NAME_GRP_RESILIENCY_PERIOD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResiliencyPeriod");
pub const CLUSREG_NAME_GRP_START_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GroupStartDelay");
pub const CLUSREG_NAME_GRP_STATUS_INFORMATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusInformation");
pub const CLUSREG_NAME_GRP_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("GroupType");
pub const CLUSREG_NAME_GRP_UPDATE_DOMAIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UpdateDomain");
pub const CLUSREG_NAME_IGNORE_PERSISTENT_STATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IgnorePersistentStateOnStartup");
pub const CLUSREG_NAME_IPADDR_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Address");
pub const CLUSREG_NAME_IPADDR_DHCP_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DhcpAddress");
pub const CLUSREG_NAME_IPADDR_DHCP_SERVER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DhcpServer");
pub const CLUSREG_NAME_IPADDR_DHCP_SUBNET_MASK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DhcpSubnetMask");
pub const CLUSREG_NAME_IPADDR_ENABLE_DHCP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableDhcp");
pub const CLUSREG_NAME_IPADDR_ENABLE_NETBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableNetBIOS");
pub const CLUSREG_NAME_IPADDR_LEASE_OBTAINED_TIME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LeaseObtainedTime");
pub const CLUSREG_NAME_IPADDR_LEASE_TERMINATES_TIME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LeaseExpiresTime");
pub const CLUSREG_NAME_IPADDR_NETWORK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Network");
pub const CLUSREG_NAME_IPADDR_OVERRIDE_ADDRMATCH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("OverrideAddressMatch");
pub const CLUSREG_NAME_IPADDR_PROBE_FAILURE_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ProbeFailureThreshold");
pub const CLUSREG_NAME_IPADDR_PROBE_PORT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ProbePort");
pub const CLUSREG_NAME_IPADDR_SHARED_NETNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharedNetname");
pub const CLUSREG_NAME_IPADDR_SUBNET_MASK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SubnetMask");
pub const CLUSREG_NAME_IPADDR_T1: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("T1");
pub const CLUSREG_NAME_IPADDR_T2: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("T2");
pub const CLUSREG_NAME_IPV6_NATIVE_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Address");
pub const CLUSREG_NAME_IPV6_NATIVE_NETWORK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Network");
pub const CLUSREG_NAME_IPV6_NATIVE_PREFIX_LENGTH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PrefixLength");
pub const CLUSREG_NAME_IPV6_TUNNEL_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Address");
pub const CLUSREG_NAME_IPV6_TUNNEL_TUNNELTYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TunnelType");
pub const CLUSREG_NAME_LAST_RECENT_EVENTS_RESET_TIME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RecentEventsResetTime");
pub const CLUSREG_NAME_LOG_FILE_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogFilePath");
pub const CLUSREG_NAME_MESSAGE_BUFFER_LENGTH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MessageBufferLength");
pub const CLUSREG_NAME_MIXED_MODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MixedMode");
pub const CLUSREG_NAME_NETFT_IPSEC_ENABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NetftIPSecEnabled");
pub const CLUSREG_NAME_NETIFACE_ADAPTER_ID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AdapterId");
pub const CLUSREG_NAME_NETIFACE_ADAPTER_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Adapter");
pub const CLUSREG_NAME_NETIFACE_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Address");
pub const CLUSREG_NAME_NETIFACE_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_NETIFACE_DHCP_ENABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DhcpEnabled");
pub const CLUSREG_NAME_NETIFACE_IPV4_ADDRESSES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv4Addresses");
pub const CLUSREG_NAME_NETIFACE_IPV6_ADDRESSES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv6Addresses");
pub const CLUSREG_NAME_NETIFACE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_NETIFACE_NETWORK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Network");
pub const CLUSREG_NAME_NETIFACE_NODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Node");
pub const CLUSREG_NAME_NETNAME_AD_AWARE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ADAware");
pub const CLUSREG_NAME_NETNAME_ALIASES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Aliases");
pub const CLUSREG_NAME_NETNAME_CONTAINERGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CryptoContainerGUID");
pub const CLUSREG_NAME_NETNAME_CREATING_DC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CreatingDC");
pub const CLUSREG_NAME_NETNAME_DNN_DISABLE_CLONES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DisableClones");
pub const CLUSREG_NAME_NETNAME_DNS_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DnsName");
pub const CLUSREG_NAME_NETNAME_DNS_SUFFIX: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DnsSuffix");
pub const CLUSREG_NAME_NETNAME_EXCLUDE_NETWORKS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ExcludeNetworks");
pub const CLUSREG_NAME_NETNAME_HOST_TTL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HostRecordTTL");
pub const CLUSREG_NAME_NETNAME_IN_USE_NETWORKS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("InUseNetworks");
pub const CLUSREG_NAME_NETNAME_LAST_DNS_UPDATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LastDNSUpdateTime");
pub const CLUSREG_NAME_NETNAME_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_NETNAME_OBJECT_ID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ObjectGUID");
pub const CLUSREG_NAME_NETNAME_PUBLISH_PTR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PublishPTRRecords");
pub const CLUSREG_NAME_NETNAME_REGISTER_ALL_IP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RegisterAllProvidersIP");
pub const CLUSREG_NAME_NETNAME_REMAP_PIPE_NAMES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RemapPipeNames");
pub const CLUSREG_NAME_NETNAME_REMOVEVCO_ONDELETE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DeleteVcoOnResCleanup");
pub const CLUSREG_NAME_NETNAME_RESOURCE_DATA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResourceData");
pub const CLUSREG_NAME_NETNAME_STATUS_DNS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusDNS");
pub const CLUSREG_NAME_NETNAME_STATUS_KERBEROS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusKerberos");
pub const CLUSREG_NAME_NETNAME_STATUS_NETBIOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusNetBIOS");
pub const CLUSREG_NAME_NETNAME_VCO_CONTAINER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VcoContainer");
pub const CLUSREG_NAME_NET_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Address");
pub const CLUSREG_NAME_NET_ADDRESS_MASK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddressMask");
pub const CLUSREG_NAME_NET_AUTOMETRIC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AutoMetric");
pub const CLUSREG_NAME_NET_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_NET_IPV4_ADDRESSES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv4Addresses");
pub const CLUSREG_NAME_NET_IPV4_PREFIXLENGTHS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv4PrefixLengths");
pub const CLUSREG_NAME_NET_IPV6_ADDRESSES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv6Addresses");
pub const CLUSREG_NAME_NET_IPV6_PREFIXLENGTHS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv6PrefixLengths");
pub const CLUSREG_NAME_NET_METRIC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Metric");
pub const CLUSREG_NAME_NET_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_NET_RDMA_CAPABLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RdmaCapable");
pub const CLUSREG_NAME_NET_ROLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Role");
pub const CLUSREG_NAME_NET_RSS_CAPABLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RssCapable");
pub const CLUSREG_NAME_NET_SPEED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LinkSpeed");
pub const CLUSREG_NAME_NODE_BUILD_NUMBER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BuildNumber");
pub const CLUSREG_NAME_NODE_CSDVERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CSDVersion");
pub const CLUSREG_NAME_NODE_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_NODE_DRAIN_STATUS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeDrainStatus");
pub const CLUSREG_NAME_NODE_DRAIN_TARGET: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeDrainTarget");
pub const CLUSREG_NAME_NODE_DYNAMIC_WEIGHT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DynamicWeight");
pub const CLUSREG_NAME_NODE_FAULT_DOMAIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FaultDomain");
pub const CLUSREG_NAME_NODE_FDID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FaultDomainId");
pub const CLUSREG_NAME_NODE_HIGHEST_VERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeHighestVersion");
pub const CLUSREG_NAME_NODE_IS_PRIMARY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsPrimary");
pub const CLUSREG_NAME_NODE_LOWEST_VERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeLowestVersion");
pub const CLUSREG_NAME_NODE_MAJOR_VERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MajorVersion");
pub const CLUSREG_NAME_NODE_MANUFACTURER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Manufacturer");
pub const CLUSREG_NAME_NODE_MINOR_VERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MinorVersion");
pub const CLUSREG_NAME_NODE_MODEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Model");
pub const CLUSREG_NAME_NODE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeName");
pub const CLUSREG_NAME_NODE_NEEDS_PQ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NeedsPreventQuorum");
pub const CLUSREG_NAME_NODE_SERIALNUMBER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SerialNumber");
pub const CLUSREG_NAME_NODE_STATUS_INFO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusInformation");
pub const CLUSREG_NAME_NODE_UNIQUEID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UniqueID");
pub const CLUSREG_NAME_NODE_WEIGHT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NodeWeight");
pub const CLUSREG_NAME_PHYSDISK_CSVBLOCKCACHE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableBlockCache");
pub const CLUSREG_NAME_PHYSDISK_CSVSNAPSHOTAGELIMIT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SnapshotAgeLimit");
pub const CLUSREG_NAME_PHYSDISK_CSVSNAPSHOTDIFFAREASIZE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SnapshotDiffSize");
pub const CLUSREG_NAME_PHYSDISK_CSVWRITETHROUGH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CsvEnforceWriteThrough");
pub const CLUSREG_NAME_PHYSDISK_DISKARBINTERVAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskArbInterval");
pub const CLUSREG_NAME_PHYSDISK_DISKARBTYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskArbType");
pub const CLUSREG_NAME_PHYSDISK_DISKGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskGuid");
pub const CLUSREG_NAME_PHYSDISK_DISKIDGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskIdGuid");
pub const CLUSREG_NAME_PHYSDISK_DISKIDTYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskIdType");
pub const CLUSREG_NAME_PHYSDISK_DISKIODELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaxIoLatency");
pub const CLUSREG_NAME_PHYSDISK_DISKPATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskPath");
pub const CLUSREG_NAME_PHYSDISK_DISKRECOVERYACTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskRecoveryAction");
pub const CLUSREG_NAME_PHYSDISK_DISKRELOAD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskReload");
pub const CLUSREG_NAME_PHYSDISK_DISKRUNCHKDSK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskRunChkDsk");
pub const CLUSREG_NAME_PHYSDISK_DISKSIGNATURE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskSignature");
pub const CLUSREG_NAME_PHYSDISK_DISKUNIQUEIDS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskUniqueIds");
pub const CLUSREG_NAME_PHYSDISK_DISKVOLUMEINFO: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DiskVolumeInfo");
pub const CLUSREG_NAME_PHYSDISK_FASTONLINEARBITRATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("FastOnlineArbitrate");
pub const CLUSREG_NAME_PHYSDISK_MAINTMODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaintenanceMode");
pub const CLUSREG_NAME_PHYSDISK_MIGRATEFIXUP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MigrateDriveLetters");
pub const CLUSREG_NAME_PHYSDISK_SPACEIDGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskId");
pub const CLUSREG_NAME_PHYSDISK_VOLSNAPACTIVATETIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VolsnapActivateTimeout");
pub const CLUSREG_NAME_PLACEMENT_OPTIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PlacementOptions");
pub const CLUSREG_NAME_PLUMB_ALL_CROSS_SUBNET_ROUTES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PlumbAllCrossSubnetRoutes");
pub const CLUSREG_NAME_PREVENTQUORUM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PreventQuorum");
pub const CLUSREG_NAME_PRTSPOOL_DEFAULT_SPOOL_DIR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DefaultSpoolDirectory");
pub const CLUSREG_NAME_PRTSPOOL_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("JobCompletionTimeout");
pub const CLUSREG_NAME_QUARANTINE_DURATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("QuarantineDuration");
pub const CLUSREG_NAME_QUARANTINE_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("QuarantineThreshold");
pub const CLUSREG_NAME_QUORUM_ARBITRATION_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("QuorumArbitrationTimeMax");
pub const CLUSREG_NAME_RESILIENCY_DEFAULT_SECONDS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResiliencyDefaultPeriod");
pub const CLUSREG_NAME_RESILIENCY_LEVEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResiliencyLevel");
pub const CLUSREG_NAME_RESTYPE_ADMIN_EXTENSIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AdminExtensions");
pub const CLUSREG_NAME_RESTYPE_DEADLOCK_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DeadlockTimeout");
pub const CLUSREG_NAME_RESTYPE_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_RESTYPE_DLL_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DllName");
pub const CLUSREG_NAME_RESTYPE_DUMP_LOG_QUERY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DumpLogQuery");
pub const CLUSREG_NAME_RESTYPE_DUMP_POLICY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DumpPolicy");
pub const CLUSREG_NAME_RESTYPE_DUMP_SERVICES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DumpServices");
pub const CLUSREG_NAME_RESTYPE_ENABLED_EVENT_LOGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnabledEventLogs");
pub const CLUSREG_NAME_RESTYPE_IS_ALIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsAlivePollInterval");
pub const CLUSREG_NAME_RESTYPE_LOOKS_ALIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LooksAlivePollInterval");
pub const CLUSREG_NAME_RESTYPE_MAX_MONITORS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MaximumMonitors");
pub const CLUSREG_NAME_RESTYPE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_RESTYPE_PENDING_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PendingTimeout");
pub const CLUSREG_NAME_RESTYPE_WPR_PROFILES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WprProfiles");
pub const CLUSREG_NAME_RESTYPE_WPR_START_AFTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WprStartAfter");
pub const CLUSREG_NAME_RES_DATA1: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResourceSpecificData1");
pub const CLUSREG_NAME_RES_DATA2: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResourceSpecificData2");
pub const CLUSREG_NAME_RES_DEADLOCK_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DeadlockTimeout");
pub const CLUSREG_NAME_RES_DESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_RES_EMBEDDED_FAILURE_ACTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EmbeddedFailureAction");
pub const CLUSREG_NAME_RES_IS_ALIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IsAlivePollInterval");
pub const CLUSREG_NAME_RES_LAST_OPERATION_STATUS_CODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LastOperationStatusCode");
pub const CLUSREG_NAME_RES_LOOKS_ALIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LooksAlivePollInterval");
pub const CLUSREG_NAME_RES_MONITOR_PID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MonitorProcessId");
pub const CLUSREG_NAME_RES_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_RES_PENDING_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PendingTimeout");
pub const CLUSREG_NAME_RES_PERSISTENT_STATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PersistentState");
pub const CLUSREG_NAME_RES_RESTART_ACTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RestartAction");
pub const CLUSREG_NAME_RES_RESTART_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RestartDelay");
pub const CLUSREG_NAME_RES_RESTART_PERIOD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RestartPeriod");
pub const CLUSREG_NAME_RES_RESTART_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RestartThreshold");
pub const CLUSREG_NAME_RES_RETRY_PERIOD_ON_FAILURE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RetryPeriodOnFailure");
pub const CLUSREG_NAME_RES_SEPARATE_MONITOR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SeparateMonitor");
pub const CLUSREG_NAME_RES_STATUS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ResourceSpecificStatus");
pub const CLUSREG_NAME_RES_STATUS_INFORMATION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StatusInformation");
pub const CLUSREG_NAME_RES_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Type");
pub const CLUSREG_NAME_ROUTE_HISTORY_LENGTH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RouteHistoryLength");
pub const CLUSREG_NAME_SAME_SUBNET_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SameSubnetDelay");
pub const CLUSREG_NAME_SAME_SUBNET_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SameSubnetThreshold");
pub const CLUSREG_NAME_SHUTDOWN_TIMEOUT_MINUTES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ShutdownTimeoutInMinutes");
pub const CLUSREG_NAME_SOFS_SMBASYMMETRYMODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SmbAsymmetryMode");
pub const CLUSREG_NAME_START_MEMORY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("StartMemory");
pub const CLUSREG_NAME_STORAGESPACE_DESCRIPTION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskDescription");
pub const CLUSREG_NAME_STORAGESPACE_HEALTH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskHealth");
pub const CLUSREG_NAME_STORAGESPACE_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskName");
pub const CLUSREG_NAME_STORAGESPACE_POOLARBITRATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Arbitrate");
pub const CLUSREG_NAME_STORAGESPACE_POOLCONSUMEDCAPACITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ConsumedCapacity");
pub const CLUSREG_NAME_STORAGESPACE_POOLDESC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSREG_NAME_STORAGESPACE_POOLDRIVEIDS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DriveIds");
pub const CLUSREG_NAME_STORAGESPACE_POOLHEALTH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Health");
pub const CLUSREG_NAME_STORAGESPACE_POOLIDGUID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PoolId");
pub const CLUSREG_NAME_STORAGESPACE_POOLNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Name");
pub const CLUSREG_NAME_STORAGESPACE_POOLQUORUMSHARE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PoolQuorumShare");
pub const CLUSREG_NAME_STORAGESPACE_POOLQUORUMUSERACCOUNT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PoolQuorumUserAccount");
pub const CLUSREG_NAME_STORAGESPACE_POOLREEVALTIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ReEvaluatePlacementTimeout");
pub const CLUSREG_NAME_STORAGESPACE_POOLSTATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("State");
pub const CLUSREG_NAME_STORAGESPACE_POOLTOTALCAPACITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TotalCapacity");
pub const CLUSREG_NAME_STORAGESPACE_PROVISIONING: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskProvisioning");
pub const CLUSREG_NAME_STORAGESPACE_RESILIENCYCOLUMNS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskResiliencyColumns");
pub const CLUSREG_NAME_STORAGESPACE_RESILIENCYINTERLEAVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskResiliencyInterleave");
pub const CLUSREG_NAME_STORAGESPACE_RESILIENCYTYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskResiliencyType");
pub const CLUSREG_NAME_STORAGESPACE_STATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualDiskState");
pub const CLUSREG_NAME_UPGRADE_VERSION: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterUpgradeVersion");
pub const CLUSREG_NAME_VIP_ADAPTER_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AdapterName");
pub const CLUSREG_NAME_VIP_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Address");
pub const CLUSREG_NAME_VIP_PREFIX_LENGTH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PrefixLength");
pub const CLUSREG_NAME_VIP_RDID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RDID");
pub const CLUSREG_NAME_VIP_VSID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VSID");
pub const CLUSREG_NAME_VIRTUAL_NUMA_COUNT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("VirtualNumaCount");
pub const CLUSREG_NAME_VSSTASK_APPNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ApplicationName");
pub const CLUSREG_NAME_VSSTASK_APPPARAMS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ApplicationParams");
pub const CLUSREG_NAME_VSSTASK_CURRENTDIRECTORY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("CurrentDirectory");
pub const CLUSREG_NAME_VSSTASK_TRIGGERARRAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TriggerArray");
pub const CLUSREG_NAME_WINS_BACKUP_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BackupPath");
pub const CLUSREG_NAME_WINS_DATABASE_PATH: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DatabasePath");
pub const CLUSREG_NAME_WITNESS_DYNAMIC_WEIGHT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WitnessDynamicWeight");
pub const CLUSREG_READ_ERROR: CLUSTER_REG_COMMAND = 9i32;
pub const CLUSREG_READ_KEY: CLUSTER_REG_COMMAND = 7i32;
pub const CLUSREG_READ_VALUE: CLUSTER_REG_COMMAND = 8i32;
pub const CLUSREG_SET_KEY_SECURITY: CLUSTER_REG_COMMAND = 5i32;
pub const CLUSREG_SET_VALUE: CLUSTER_REG_COMMAND = 1i32;
pub const CLUSREG_VALUE_DELETED: CLUSTER_REG_COMMAND = 6i32;
pub const CLUSRESDLL_STATUS_DO_NOT_COLLECT_WER_REPORT: u32 = 1073741824u32;
pub const CLUSRESDLL_STATUS_DUMP_NOW: u32 = 2147483648u32;
pub const CLUSRESDLL_STATUS_INSUFFICIENT_MEMORY: u32 = 16u32;
pub const CLUSRESDLL_STATUS_INSUFFICIENT_OTHER_RESOURCES: u32 = 64u32;
pub const CLUSRESDLL_STATUS_INSUFFICIENT_PROCESSOR: u32 = 32u32;
pub const CLUSRESDLL_STATUS_INVALID_PARAMETERS: u32 = 128u32;
pub const CLUSRESDLL_STATUS_NETWORK_NOT_AVAILABLE: u32 = 256u32;
pub const CLUSRESDLL_STATUS_OFFLINE_BUSY: u32 = 1u32;
pub const CLUSRESDLL_STATUS_OFFLINE_DESTINATION_REJECTED: u32 = 8u32;
pub const CLUSRESDLL_STATUS_OFFLINE_DESTINATION_THROTTLED: u32 = 4u32;
pub const CLUSRESDLL_STATUS_OFFLINE_SOURCE_THROTTLED: u32 = 2u32;
pub const CLUSRES_DISABLE_WPR_WATCHDOG_FOR_OFFLINE_CALLS: u32 = 2u32;
pub const CLUSRES_DISABLE_WPR_WATCHDOG_FOR_ONLINE_CALLS: u32 = 1u32;
pub const CLUSRES_NAME_GET_OPERATION_CONTEXT_FLAGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Flags");
pub const CLUSRES_STATUS_APPLICATION_READY: u64 = 256u64;
pub const CLUSRES_STATUS_EMBEDDED_FAILURE: u64 = 2u64;
pub const CLUSRES_STATUS_FAILED_DUE_TO_INSUFFICIENT_CPU: u64 = 4u64;
pub const CLUSRES_STATUS_FAILED_DUE_TO_INSUFFICIENT_GENERIC_RESOURCES: u64 = 16u64;
pub const CLUSRES_STATUS_FAILED_DUE_TO_INSUFFICIENT_MEMORY: u64 = 8u64;
pub const CLUSRES_STATUS_LOCKED_MODE: u64 = 1u64;
pub const CLUSRES_STATUS_NETWORK_FAILURE: u64 = 32u64;
pub const CLUSRES_STATUS_OFFLINE_NOT_LOCAL_DISK_OWNER: u64 = 512u64;
pub const CLUSRES_STATUS_OS_HEARTBEAT: u64 = 128u64;
pub const CLUSRES_STATUS_UNMONITORED: u64 = 64u64;
pub const CLUSTERSET_OBJECT_TYPE_DATABASE: CLUSTERSET_OBJECT_TYPE = 3i32;
pub const CLUSTERSET_OBJECT_TYPE_MEMBER: CLUSTERSET_OBJECT_TYPE = 1i32;
pub const CLUSTERSET_OBJECT_TYPE_NONE: CLUSTERSET_OBJECT_TYPE = 0i32;
pub const CLUSTERSET_OBJECT_TYPE_WORKLOAD: CLUSTERSET_OBJECT_TYPE = 2i32;
pub const CLUSTER_ADD_EVICT_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AddEvictDelay");
pub const CLUSTER_AVAILABILITY_SET_CONFIG_V1: u32 = 1u32;
pub const CLUSTER_CHANGE_ALL: CLUSTER_CHANGE = -1i32;
pub const CLUSTER_CHANGE_CLUSTER_ALL_V2: CLUSTER_CHANGE_CLUSTER_V2 = 8191i32;
pub const CLUSTER_CHANGE_CLUSTER_COMMON_PROPERTY_V2: CLUSTER_CHANGE_CLUSTER_V2 = 128i32;
pub const CLUSTER_CHANGE_CLUSTER_GROUP_ADDED_V2: CLUSTER_CHANGE_CLUSTER_V2 = 4i32;
pub const CLUSTER_CHANGE_CLUSTER_HANDLE_CLOSE_V2: CLUSTER_CHANGE_CLUSTER_V2 = 8i32;
pub const CLUSTER_CHANGE_CLUSTER_LOST_NOTIFICATIONS_V2: CLUSTER_CHANGE_CLUSTER_V2 = 512i32;
pub const CLUSTER_CHANGE_CLUSTER_MEMBERSHIP_V2: CLUSTER_CHANGE_CLUSTER_V2 = 2048i32;
pub const CLUSTER_CHANGE_CLUSTER_NETWORK_ADDED_V2: CLUSTER_CHANGE_CLUSTER_V2 = 16i32;
pub const CLUSTER_CHANGE_CLUSTER_NODE_ADDED_V2: CLUSTER_CHANGE_CLUSTER_V2 = 32i32;
pub const CLUSTER_CHANGE_CLUSTER_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_CLUSTER_V2 = 256i32;
pub const CLUSTER_CHANGE_CLUSTER_PROPERTY: CLUSTER_CHANGE = 1073741824i32;
pub const CLUSTER_CHANGE_CLUSTER_RECONNECT: CLUSTER_CHANGE = 524288i32;
pub const CLUSTER_CHANGE_CLUSTER_RECONNECT_V2: CLUSTER_CHANGE_CLUSTER_V2 = 1i32;
pub const CLUSTER_CHANGE_CLUSTER_RENAME_V2: CLUSTER_CHANGE_CLUSTER_V2 = 1024i32;
pub const CLUSTER_CHANGE_CLUSTER_RESOURCE_TYPE_ADDED_V2: CLUSTER_CHANGE_CLUSTER_V2 = 64i32;
pub const CLUSTER_CHANGE_CLUSTER_STATE: CLUSTER_CHANGE = 536870912i32;
pub const CLUSTER_CHANGE_CLUSTER_STATE_V2: CLUSTER_CHANGE_CLUSTER_V2 = 2i32;
pub const CLUSTER_CHANGE_CLUSTER_UPGRADED_V2: CLUSTER_CHANGE_CLUSTER_V2 = 4096i32;
pub const CLUSTER_CHANGE_GROUPSET_ALL_V2: CLUSTER_CHANGE_GROUPSET_V2 = 511i32;
pub const CLUSTER_CHANGE_GROUPSET_COMMON_PROPERTY_V2: CLUSTER_CHANGE_GROUPSET_V2 = 2i32;
pub const CLUSTER_CHANGE_GROUPSET_DELETED_v2: CLUSTER_CHANGE_GROUPSET_V2 = 1i32;
pub const CLUSTER_CHANGE_GROUPSET_DEPENDENCIES_V2: CLUSTER_CHANGE_GROUPSET_V2 = 64i32;
pub const CLUSTER_CHANGE_GROUPSET_DEPENDENTS_V2: CLUSTER_CHANGE_GROUPSET_V2 = 128i32;
pub const CLUSTER_CHANGE_GROUPSET_GROUP_ADDED: CLUSTER_CHANGE_GROUPSET_V2 = 16i32;
pub const CLUSTER_CHANGE_GROUPSET_GROUP_REMOVED: CLUSTER_CHANGE_GROUPSET_V2 = 32i32;
pub const CLUSTER_CHANGE_GROUPSET_HANDLE_CLOSE_v2: CLUSTER_CHANGE_GROUPSET_V2 = 256i32;
pub const CLUSTER_CHANGE_GROUPSET_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_GROUPSET_V2 = 4i32;
pub const CLUSTER_CHANGE_GROUPSET_STATE_V2: CLUSTER_CHANGE_GROUPSET_V2 = 8i32;
pub const CLUSTER_CHANGE_GROUP_ADDED: CLUSTER_CHANGE = 16384i32;
pub const CLUSTER_CHANGE_GROUP_ALL_V2: CLUSTER_CHANGE_GROUP_V2 = 1023i32;
pub const CLUSTER_CHANGE_GROUP_COMMON_PROPERTY_V2: CLUSTER_CHANGE_GROUP_V2 = 2i32;
pub const CLUSTER_CHANGE_GROUP_DELETED: CLUSTER_CHANGE = 8192i32;
pub const CLUSTER_CHANGE_GROUP_DELETED_V2: CLUSTER_CHANGE_GROUP_V2 = 1i32;
pub const CLUSTER_CHANGE_GROUP_HANDLE_CLOSE_V2: CLUSTER_CHANGE_GROUP_V2 = 512i32;
pub const CLUSTER_CHANGE_GROUP_OWNER_NODE_V2: CLUSTER_CHANGE_GROUP_V2 = 16i32;
pub const CLUSTER_CHANGE_GROUP_PREFERRED_OWNERS_V2: CLUSTER_CHANGE_GROUP_V2 = 32i32;
pub const CLUSTER_CHANGE_GROUP_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_GROUP_V2 = 4i32;
pub const CLUSTER_CHANGE_GROUP_PROPERTY: CLUSTER_CHANGE = 32768i32;
pub const CLUSTER_CHANGE_GROUP_RESOURCE_ADDED_V2: CLUSTER_CHANGE_GROUP_V2 = 64i32;
pub const CLUSTER_CHANGE_GROUP_RESOURCE_GAINED_V2: CLUSTER_CHANGE_GROUP_V2 = 128i32;
pub const CLUSTER_CHANGE_GROUP_RESOURCE_LOST_V2: CLUSTER_CHANGE_GROUP_V2 = 256i32;
pub const CLUSTER_CHANGE_GROUP_STATE: CLUSTER_CHANGE = 4096i32;
pub const CLUSTER_CHANGE_GROUP_STATE_V2: CLUSTER_CHANGE_GROUP_V2 = 8i32;
pub const CLUSTER_CHANGE_HANDLE_CLOSE: CLUSTER_CHANGE = -2147483648i32;
pub const CLUSTER_CHANGE_NETINTERFACE_ADDED: CLUSTER_CHANGE = 67108864i32;
pub const CLUSTER_CHANGE_NETINTERFACE_ALL_V2: CLUSTER_CHANGE_NETINTERFACE_V2 = 31i32;
pub const CLUSTER_CHANGE_NETINTERFACE_COMMON_PROPERTY_V2: CLUSTER_CHANGE_NETINTERFACE_V2 = 2i32;
pub const CLUSTER_CHANGE_NETINTERFACE_DELETED: CLUSTER_CHANGE = 33554432i32;
pub const CLUSTER_CHANGE_NETINTERFACE_DELETED_V2: CLUSTER_CHANGE_NETINTERFACE_V2 = 1i32;
pub const CLUSTER_CHANGE_NETINTERFACE_HANDLE_CLOSE_V2: CLUSTER_CHANGE_NETINTERFACE_V2 = 16i32;
pub const CLUSTER_CHANGE_NETINTERFACE_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_NETINTERFACE_V2 = 4i32;
pub const CLUSTER_CHANGE_NETINTERFACE_PROPERTY: CLUSTER_CHANGE = 134217728i32;
pub const CLUSTER_CHANGE_NETINTERFACE_STATE: CLUSTER_CHANGE = 16777216i32;
pub const CLUSTER_CHANGE_NETINTERFACE_STATE_V2: CLUSTER_CHANGE_NETINTERFACE_V2 = 8i32;
pub const CLUSTER_CHANGE_NETWORK_ADDED: CLUSTER_CHANGE = 4194304i32;
pub const CLUSTER_CHANGE_NETWORK_ALL_V2: CLUSTER_CHANGE_NETWORK_V2 = 31i32;
pub const CLUSTER_CHANGE_NETWORK_COMMON_PROPERTY_V2: CLUSTER_CHANGE_NETWORK_V2 = 2i32;
pub const CLUSTER_CHANGE_NETWORK_DELETED: CLUSTER_CHANGE = 2097152i32;
pub const CLUSTER_CHANGE_NETWORK_DELETED_V2: CLUSTER_CHANGE_NETWORK_V2 = 1i32;
pub const CLUSTER_CHANGE_NETWORK_HANDLE_CLOSE_V2: CLUSTER_CHANGE_NETWORK_V2 = 16i32;
pub const CLUSTER_CHANGE_NETWORK_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_NETWORK_V2 = 4i32;
pub const CLUSTER_CHANGE_NETWORK_PROPERTY: CLUSTER_CHANGE = 8388608i32;
pub const CLUSTER_CHANGE_NETWORK_STATE: CLUSTER_CHANGE = 1048576i32;
pub const CLUSTER_CHANGE_NETWORK_STATE_V2: CLUSTER_CHANGE_NETWORK_V2 = 8i32;
pub const CLUSTER_CHANGE_NODE_ADDED: CLUSTER_CHANGE = 4i32;
pub const CLUSTER_CHANGE_NODE_ALL_V2: CLUSTER_CHANGE_NODE_V2 = 255i32;
pub const CLUSTER_CHANGE_NODE_COMMON_PROPERTY_V2: CLUSTER_CHANGE_NODE_V2 = 4i32;
pub const CLUSTER_CHANGE_NODE_DELETED: CLUSTER_CHANGE = 2i32;
pub const CLUSTER_CHANGE_NODE_DELETED_V2: CLUSTER_CHANGE_NODE_V2 = 2i32;
pub const CLUSTER_CHANGE_NODE_GROUP_GAINED_V2: CLUSTER_CHANGE_NODE_V2 = 32i32;
pub const CLUSTER_CHANGE_NODE_GROUP_LOST_V2: CLUSTER_CHANGE_NODE_V2 = 64i32;
pub const CLUSTER_CHANGE_NODE_HANDLE_CLOSE_V2: CLUSTER_CHANGE_NODE_V2 = 128i32;
pub const CLUSTER_CHANGE_NODE_NETINTERFACE_ADDED_V2: CLUSTER_CHANGE_NODE_V2 = 1i32;
pub const CLUSTER_CHANGE_NODE_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_NODE_V2 = 8i32;
pub const CLUSTER_CHANGE_NODE_PROPERTY: CLUSTER_CHANGE = 8i32;
pub const CLUSTER_CHANGE_NODE_STATE: CLUSTER_CHANGE = 1i32;
pub const CLUSTER_CHANGE_NODE_STATE_V2: CLUSTER_CHANGE_NODE_V2 = 16i32;
pub const CLUSTER_CHANGE_QUORUM_ALL_V2: CLUSTER_CHANGE_QUORUM_V2 = 1i32;
pub const CLUSTER_CHANGE_QUORUM_STATE: CLUSTER_CHANGE = 268435456i32;
pub const CLUSTER_CHANGE_QUORUM_STATE_V2: CLUSTER_CHANGE_QUORUM_V2 = 1i32;
pub const CLUSTER_CHANGE_REGISTRY_ALL_V2: CLUSTER_CHANGE_REGISTRY_V2 = 31i32;
pub const CLUSTER_CHANGE_REGISTRY_ATTRIBUTES: CLUSTER_CHANGE = 32i32;
pub const CLUSTER_CHANGE_REGISTRY_ATTRIBUTES_V2: CLUSTER_CHANGE_REGISTRY_V2 = 1i32;
pub const CLUSTER_CHANGE_REGISTRY_HANDLE_CLOSE_V2: CLUSTER_CHANGE_REGISTRY_V2 = 16i32;
pub const CLUSTER_CHANGE_REGISTRY_NAME: CLUSTER_CHANGE = 16i32;
pub const CLUSTER_CHANGE_REGISTRY_NAME_V2: CLUSTER_CHANGE_REGISTRY_V2 = 2i32;
pub const CLUSTER_CHANGE_REGISTRY_SUBTREE: CLUSTER_CHANGE = 128i32;
pub const CLUSTER_CHANGE_REGISTRY_SUBTREE_V2: CLUSTER_CHANGE_REGISTRY_V2 = 4i32;
pub const CLUSTER_CHANGE_REGISTRY_VALUE: CLUSTER_CHANGE = 64i32;
pub const CLUSTER_CHANGE_REGISTRY_VALUE_V2: CLUSTER_CHANGE_REGISTRY_V2 = 8i32;
pub const CLUSTER_CHANGE_RESOURCE_ADDED: CLUSTER_CHANGE = 1024i32;
pub const CLUSTER_CHANGE_RESOURCE_ALL_V2: CLUSTER_CHANGE_RESOURCE_V2 = 2047i32;
pub const CLUSTER_CHANGE_RESOURCE_COMMON_PROPERTY_V2: CLUSTER_CHANGE_RESOURCE_V2 = 1i32;
pub const CLUSTER_CHANGE_RESOURCE_DELETED: CLUSTER_CHANGE = 512i32;
pub const CLUSTER_CHANGE_RESOURCE_DELETED_V2: CLUSTER_CHANGE_RESOURCE_V2 = 128i32;
pub const CLUSTER_CHANGE_RESOURCE_DEPENDENCIES_V2: CLUSTER_CHANGE_RESOURCE_V2 = 16i32;
pub const CLUSTER_CHANGE_RESOURCE_DEPENDENTS_V2: CLUSTER_CHANGE_RESOURCE_V2 = 32i32;
pub const CLUSTER_CHANGE_RESOURCE_DLL_UPGRADED_V2: CLUSTER_CHANGE_RESOURCE_V2 = 256i32;
pub const CLUSTER_CHANGE_RESOURCE_HANDLE_CLOSE_V2: CLUSTER_CHANGE_RESOURCE_V2 = 512i32;
pub const CLUSTER_CHANGE_RESOURCE_OWNER_GROUP_V2: CLUSTER_CHANGE_RESOURCE_V2 = 8i32;
pub const CLUSTER_CHANGE_RESOURCE_POSSIBLE_OWNERS_V2: CLUSTER_CHANGE_RESOURCE_V2 = 64i32;
pub const CLUSTER_CHANGE_RESOURCE_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_RESOURCE_V2 = 2i32;
pub const CLUSTER_CHANGE_RESOURCE_PROPERTY: CLUSTER_CHANGE = 2048i32;
pub const CLUSTER_CHANGE_RESOURCE_STATE: CLUSTER_CHANGE = 256i32;
pub const CLUSTER_CHANGE_RESOURCE_STATE_V2: CLUSTER_CHANGE_RESOURCE_V2 = 4i32;
pub const CLUSTER_CHANGE_RESOURCE_TERMINAL_STATE_V2: CLUSTER_CHANGE_RESOURCE_V2 = 1024i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_ADDED: CLUSTER_CHANGE = 131072i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_ALL_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 63i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_COMMON_PROPERTY_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 2i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_DELETED: CLUSTER_CHANGE = 65536i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_DELETED_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 1i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_DLL_UPGRADED_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 16i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_POSSIBLE_OWNERS_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 8i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_PRIVATE_PROPERTY_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 4i32;
pub const CLUSTER_CHANGE_RESOURCE_TYPE_PROPERTY: CLUSTER_CHANGE = 262144i32;
pub const CLUSTER_CHANGE_SHARED_VOLUME_ADDED_V2: CLUSTER_CHANGE_SHARED_VOLUME_V2 = 2i32;
pub const CLUSTER_CHANGE_SHARED_VOLUME_ALL_V2: CLUSTER_CHANGE_SHARED_VOLUME_V2 = 7i32;
pub const CLUSTER_CHANGE_SHARED_VOLUME_REMOVED_V2: CLUSTER_CHANGE_SHARED_VOLUME_V2 = 4i32;
pub const CLUSTER_CHANGE_SHARED_VOLUME_STATE_V2: CLUSTER_CHANGE_SHARED_VOLUME_V2 = 1i32;
pub const CLUSTER_CHANGE_SPACEPORT_CUSTOM_PNP_V2: CLUSTER_CHANGE_SPACEPORT_V2 = 1i32;
pub const CLUSTER_CHANGE_UPGRADE_ALL: CLUSTER_CHANGE_NODE_UPGRADE_PHASE_V2 = 7i32;
pub const CLUSTER_CHANGE_UPGRADE_NODE_COMMIT: CLUSTER_CHANGE_NODE_UPGRADE_PHASE_V2 = 2i32;
pub const CLUSTER_CHANGE_UPGRADE_NODE_POSTCOMMIT: CLUSTER_CHANGE_NODE_UPGRADE_PHASE_V2 = 4i32;
pub const CLUSTER_CHANGE_UPGRADE_NODE_PREPARE: CLUSTER_CHANGE_NODE_UPGRADE_PHASE_V2 = 1i32;
pub const CLUSTER_CLOUD_TYPE_AZURE: CLUSTER_CLOUD_TYPE = 1i32;
pub const CLUSTER_CLOUD_TYPE_MIXED: CLUSTER_CLOUD_TYPE = 128i32;
pub const CLUSTER_CLOUD_TYPE_NONE: CLUSTER_CLOUD_TYPE = 0i32;
pub const CLUSTER_CLOUD_TYPE_UNKNOWN: CLUSTER_CLOUD_TYPE = -1i32;
pub const CLUSTER_CONFIGURED: u32 = 2u32;
pub const CLUSTER_CREATE_GROUP_INFO_VERSION: u32 = 1u32;
pub const CLUSTER_CREATE_GROUP_INFO_VERSION_1: u32 = 1u32;
pub const CLUSTER_CSA_VSS_STATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("BackupInProgress");
pub const CLUSTER_CSV_COMPATIBLE_FILTERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharedVolumeCompatibleFilters");
pub const CLUSTER_CSV_INCOMPATIBLE_FILTERS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharedVolumeIncompatibleFilters");
pub const CLUSTER_DELETE_ACCESS_CONTROL_ENTRY: u32 = 2u32;
pub const CLUSTER_ENFORCED_ANTIAFFINITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterEnforcedAntiaffinity");
pub const CLUSTER_ENUM_ALL: CLUSTER_ENUM = 63i32;
pub const CLUSTER_ENUM_GROUP: CLUSTER_ENUM = 8i32;
pub const CLUSTER_ENUM_INTERNAL_NETWORK: CLUSTER_ENUM = -2147483648i32;
pub const CLUSTER_ENUM_ITEM_VERSION: u32 = 1u32;
pub const CLUSTER_ENUM_ITEM_VERSION_1: u32 = 1u32;
pub const CLUSTER_ENUM_NETINTERFACE: CLUSTER_ENUM = 32i32;
pub const CLUSTER_ENUM_NETWORK: CLUSTER_ENUM = 16i32;
pub const CLUSTER_ENUM_NODE: CLUSTER_ENUM = 1i32;
pub const CLUSTER_ENUM_RESOURCE: CLUSTER_ENUM = 4i32;
pub const CLUSTER_ENUM_RESTYPE: CLUSTER_ENUM = 2i32;
pub const CLUSTER_ENUM_SHARED_VOLUME_GROUP: CLUSTER_ENUM = 536870912i32;
pub const CLUSTER_ENUM_SHARED_VOLUME_RESOURCE: CLUSTER_ENUM = 1073741824i32;
pub const CLUSTER_GROUP_ENUM_ALL: CLUSTER_GROUP_ENUM = 3i32;
pub const CLUSTER_GROUP_ENUM_CONTAINS: CLUSTER_GROUP_ENUM = 1i32;
pub const CLUSTER_GROUP_ENUM_ITEM_VERSION: u32 = 1u32;
pub const CLUSTER_GROUP_ENUM_ITEM_VERSION_1: u32 = 1u32;
pub const CLUSTER_GROUP_ENUM_NODES: CLUSTER_GROUP_ENUM = 2i32;
pub const CLUSTER_GROUP_WAIT_DELAY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterGroupWaitDelay");
pub const CLUSTER_HANG_RECOVERY_ACTION_KEYNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HangRecoveryAction");
pub const CLUSTER_HANG_TIMEOUT_KEYNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusSvcHangTimeout");
pub const CLUSTER_HEALTH_FAULT_ARGS: u32 = 7u32;
pub const CLUSTER_HEALTH_FAULT_DESCRIPTION: u32 = 3u32;
pub const CLUSTER_HEALTH_FAULT_DESCRIPTION_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Description");
pub const CLUSTER_HEALTH_FAULT_ERRORCODE: u32 = 2u32;
pub const CLUSTER_HEALTH_FAULT_ERRORCODE_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ErrorCode");
pub const CLUSTER_HEALTH_FAULT_ERRORTYPE: u32 = 1u32;
pub const CLUSTER_HEALTH_FAULT_ERRORTYPE_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ErrorType");
pub const CLUSTER_HEALTH_FAULT_FLAGS: u32 = 5u32;
pub const CLUSTER_HEALTH_FAULT_FLAGS_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Flags");
pub const CLUSTER_HEALTH_FAULT_ID: u32 = 0u32;
pub const CLUSTER_HEALTH_FAULT_ID_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Id");
pub const CLUSTER_HEALTH_FAULT_PROPERTY_NAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterHealth");
pub const CLUSTER_HEALTH_FAULT_PROVIDER: u32 = 4u32;
pub const CLUSTER_HEALTH_FAULT_PROVIDER_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Provider");
pub const CLUSTER_HEALTH_FAULT_RESERVED: u32 = 6u32;
pub const CLUSTER_HEALTH_FAULT_RESERVED_LABEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Reserved");
pub const CLUSTER_INSTALLED: u32 = 1u32;
pub const CLUSTER_MGMT_POINT_RESTYPE_AUTO: CLUSTER_MGMT_POINT_RESTYPE = 0i32;
pub const CLUSTER_MGMT_POINT_RESTYPE_DNN: CLUSTER_MGMT_POINT_RESTYPE = 2i32;
pub const CLUSTER_MGMT_POINT_RESTYPE_SNN: CLUSTER_MGMT_POINT_RESTYPE = 1i32;
pub const CLUSTER_MGMT_POINT_TYPE_CNO: CLUSTER_MGMT_POINT_TYPE = 1i32;
pub const CLUSTER_MGMT_POINT_TYPE_CNO_ONLY: CLUSTER_MGMT_POINT_TYPE = 3i32;
pub const CLUSTER_MGMT_POINT_TYPE_DNS_ONLY: CLUSTER_MGMT_POINT_TYPE = 2i32;
pub const CLUSTER_MGMT_POINT_TYPE_NONE: CLUSTER_MGMT_POINT_TYPE = 0i32;
pub const CLUSTER_NAME_AUTO_BALANCER_LEVEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AutoBalancerLevel");
pub const CLUSTER_NAME_AUTO_BALANCER_MODE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("AutoBalancerMode");
pub const CLUSTER_NAME_PREFERRED_SITE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("PreferredSite");
pub const CLUSTER_NETWORK_ENUM_ALL: CLUSTER_NETWORK_ENUM = 1i32;
pub const CLUSTER_NETWORK_ENUM_NETINTERFACES: CLUSTER_NETWORK_ENUM = 1i32;
pub const CLUSTER_NODE_ENUM_ALL: CLUSTER_NODE_ENUM = 3i32;
pub const CLUSTER_NODE_ENUM_GROUPS: CLUSTER_NODE_ENUM = 2i32;
pub const CLUSTER_NODE_ENUM_NETINTERFACES: CLUSTER_NODE_ENUM = 1i32;
pub const CLUSTER_NODE_ENUM_PREFERRED_GROUPS: CLUSTER_NODE_ENUM = 4i32;
pub const CLUSTER_NOTIFICATIONS_V1: CLUSTER_NOTIFICATIONS_VERSION = 1i32;
pub const CLUSTER_NOTIFICATIONS_V2: CLUSTER_NOTIFICATIONS_VERSION = 2i32;
pub const CLUSTER_OBJECT_TYPE_AFFINITYRULE: CLUSTER_OBJECT_TYPE = 16i32;
pub const CLUSTER_OBJECT_TYPE_CLUSTER: CLUSTER_OBJECT_TYPE = 1i32;
pub const CLUSTER_OBJECT_TYPE_FAULTDOMAIN: CLUSTER_OBJECT_TYPE = 17i32;
pub const CLUSTER_OBJECT_TYPE_GROUP: CLUSTER_OBJECT_TYPE = 2i32;
pub const CLUSTER_OBJECT_TYPE_GROUPSET: CLUSTER_OBJECT_TYPE = 13i32;
pub const CLUSTER_OBJECT_TYPE_NETWORK: CLUSTER_OBJECT_TYPE = 6i32;
pub const CLUSTER_OBJECT_TYPE_NETWORK_INTERFACE: CLUSTER_OBJECT_TYPE = 5i32;
pub const CLUSTER_OBJECT_TYPE_NODE: CLUSTER_OBJECT_TYPE = 7i32;
pub const CLUSTER_OBJECT_TYPE_NONE: CLUSTER_OBJECT_TYPE = 0i32;
pub const CLUSTER_OBJECT_TYPE_QUORUM: CLUSTER_OBJECT_TYPE = 9i32;
pub const CLUSTER_OBJECT_TYPE_REGISTRY: CLUSTER_OBJECT_TYPE = 8i32;
pub const CLUSTER_OBJECT_TYPE_RESOURCE: CLUSTER_OBJECT_TYPE = 3i32;
pub const CLUSTER_OBJECT_TYPE_RESOURCE_TYPE: CLUSTER_OBJECT_TYPE = 4i32;
pub const CLUSTER_OBJECT_TYPE_SHARED_VOLUME: CLUSTER_OBJECT_TYPE = 10i32;
pub const CLUSTER_QUORUM_LOST: CLUSTER_QUORUM_VALUE = 1i32;
pub const CLUSTER_QUORUM_MAINTAINED: CLUSTER_QUORUM_VALUE = 0i32;
pub const CLUSTER_REQUEST_REPLY_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("RequestReplyTimeout");
pub const CLUSTER_RESOURCE_DEFAULT_MONITOR: CLUSTER_RESOURCE_CREATE_FLAGS = 0i32;
pub const CLUSTER_RESOURCE_ENUM_ALL: CLUSTER_RESOURCE_ENUM = 7i32;
pub const CLUSTER_RESOURCE_ENUM_DEPENDS: CLUSTER_RESOURCE_ENUM = 1i32;
pub const CLUSTER_RESOURCE_ENUM_ITEM_VERSION: u32 = 1u32;
pub const CLUSTER_RESOURCE_ENUM_ITEM_VERSION_1: u32 = 1u32;
pub const CLUSTER_RESOURCE_ENUM_NODES: CLUSTER_RESOURCE_ENUM = 4i32;
pub const CLUSTER_RESOURCE_ENUM_PROVIDES: CLUSTER_RESOURCE_ENUM = 2i32;
pub const CLUSTER_RESOURCE_SEPARATE_MONITOR: CLUSTER_RESOURCE_CREATE_FLAGS = 1i32;
pub const CLUSTER_RESOURCE_TYPE_ENUM_ALL: CLUSTER_RESOURCE_TYPE_ENUM = 3i32;
pub const CLUSTER_RESOURCE_TYPE_ENUM_NODES: CLUSTER_RESOURCE_TYPE_ENUM = 1i32;
pub const CLUSTER_RESOURCE_TYPE_ENUM_RESOURCES: CLUSTER_RESOURCE_TYPE_ENUM = 2i32;
pub const CLUSTER_RESOURCE_TYPE_SPECIFIC_V2: CLUSTER_CHANGE_RESOURCE_TYPE_V2 = 32i32;
pub const CLUSTER_RESOURCE_VALID_FLAGS: CLUSTER_RESOURCE_CREATE_FLAGS = 1i32;
pub const CLUSTER_RUNNING: u32 = 16u32;
pub const CLUSTER_S2D_BUS_TYPES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DBusTypes");
pub const CLUSTER_S2D_CACHE_BEHAVIOR_FLAGS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DCacheBehavior");
pub const CLUSTER_S2D_CACHE_DESIRED_STATE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DCacheDesiredState");
pub const CLUSTER_S2D_CACHE_FLASH_RESERVE_PERCENT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DCacheFlashReservePercent");
pub const CLUSTER_S2D_CACHE_METADATA_RESERVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DCacheMetadataReserveBytes");
pub const CLUSTER_S2D_CACHE_PAGE_SIZE_KBYTES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DCachePageSizeKBytes");
pub const CLUSTER_S2D_ENABLED: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DEnabled");
pub const CLUSTER_S2D_IO_LATENCY_THRESHOLD: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DIOLatencyThreshold");
pub const CLUSTER_S2D_OPTIMIZATIONS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("S2DOptimizations");
pub const CLUSTER_SET_ACCESS_TYPE_ALLOWED: u32 = 0u32;
pub const CLUSTER_SET_ACCESS_TYPE_DENIED: u32 = 1u32;
pub const CLUSTER_SHARED_VOLUMES_ROOT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharedVolumesRoot");
pub const CLUSTER_SHARED_VOLUME_VSS_WRITER_OPERATION_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SharedVolumeVssWriterOperationTimeout");
pub const CLUSTER_VERSION_FLAG_MIXED_MODE: u32 = 1u32;
pub const CLUSTER_VERSION_UNKNOWN: u32 = 4294967295u32;
pub const CLUSTER_WITNESS_DATABASE_WRITE_TIMEOUT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WitnessDatabaseWriteTimeout");
pub const CLUSTER_WITNESS_FAILED_RESTART_INTERVAL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WitnessRestartInterval");
pub const CLUS_ACCESS_ANY: u32 = 0u32;
pub const CLUS_ACCESS_READ: u32 = 1u32;
pub const CLUS_ACCESS_WRITE: u32 = 2u32;
pub const CLUS_AFFINITY_RULE_DIFFERENT_FAULT_DOMAIN: CLUS_AFFINITY_RULE_TYPE = 3i32;
pub const CLUS_AFFINITY_RULE_DIFFERENT_NODE: CLUS_AFFINITY_RULE_TYPE = 4i32;
pub const CLUS_AFFINITY_RULE_MAX: CLUS_AFFINITY_RULE_TYPE = 4i32;
pub const CLUS_AFFINITY_RULE_MIN: CLUS_AFFINITY_RULE_TYPE = 0i32;
pub const CLUS_AFFINITY_RULE_NONE: CLUS_AFFINITY_RULE_TYPE = 0i32;
pub const CLUS_AFFINITY_RULE_SAME_FAULT_DOMAIN: CLUS_AFFINITY_RULE_TYPE = 1i32;
pub const CLUS_AFFINITY_RULE_SAME_NODE: CLUS_AFFINITY_RULE_TYPE = 2i32;
pub const CLUS_CHAR_BROADCAST_DELETE: CLUS_CHARACTERISTICS = 32i32;
pub const CLUS_CHAR_CLONES: CLUS_CHARACTERISTICS = 8192i32;
pub const CLUS_CHAR_COEXIST_IN_SHARED_VOLUME_GROUP: CLUS_CHARACTERISTICS = 256i32;
pub const CLUS_CHAR_DELETE_REQUIRES_ALL_NODES: CLUS_CHARACTERISTICS = 2i32;
pub const CLUS_CHAR_DRAIN_LOCAL_OFFLINE: CLUS_CHARACTERISTICS = 524288i32;
pub const CLUS_CHAR_INFRASTRUCTURE: CLUS_CHARACTERISTICS = 131072i32;
pub const CLUS_CHAR_LOCAL_QUORUM: CLUS_CHARACTERISTICS = 4i32;
pub const CLUS_CHAR_LOCAL_QUORUM_DEBUG: CLUS_CHARACTERISTICS = 8i32;
pub const CLUS_CHAR_MONITOR_DETACH: CLUS_CHARACTERISTICS = 1024i32;
pub const CLUS_CHAR_MONITOR_REATTACH: CLUS_CHARACTERISTICS = 2048i32;
pub const CLUS_CHAR_NOTIFY_NEW_OWNER: CLUS_CHARACTERISTICS = 32768i32;
pub const CLUS_CHAR_NOT_PREEMPTABLE: CLUS_CHARACTERISTICS = 16384i32;
pub const CLUS_CHAR_OPERATION_CONTEXT: CLUS_CHARACTERISTICS = 4096i32;
pub const CLUS_CHAR_PLACEMENT_DATA: CLUS_CHARACTERISTICS = 512i32;
pub const CLUS_CHAR_QUORUM: CLUS_CHARACTERISTICS = 1i32;
pub const CLUS_CHAR_REQUIRES_STATE_CHANGE_REASON: CLUS_CHARACTERISTICS = 16i32;
pub const CLUS_CHAR_SINGLE_CLUSTER_INSTANCE: CLUS_CHARACTERISTICS = 64i32;
pub const CLUS_CHAR_SINGLE_GROUP_INSTANCE: CLUS_CHARACTERISTICS = 128i32;
pub const CLUS_CHAR_SUPPORTS_UNMONITORED_STATE: CLUS_CHARACTERISTICS = 65536i32;
pub const CLUS_CHAR_UNKNOWN: CLUS_CHARACTERISTICS = 0i32;
pub const CLUS_CHAR_VETO_DRAIN: CLUS_CHARACTERISTICS = 262144i32;
pub const CLUS_CREATE_CRYPT_CONTAINER_NOT_FOUND: u32 = 1u32;
pub const CLUS_FLAG_CORE: CLUS_FLAGS = 1i32;
pub const CLUS_GLOBAL: u32 = 1u32;
pub const CLUS_GROUP_DO_NOT_START: CLUS_GROUP_START_SETTING = 1i32;
pub const CLUS_GROUP_START_ALLOWED: CLUS_GROUP_START_SETTING = 2i32;
pub const CLUS_GROUP_START_ALWAYS: CLUS_GROUP_START_SETTING = 0i32;
pub const CLUS_GRP_MOVE_ALLOWED: u32 = 0u32;
pub const CLUS_GRP_MOVE_LOCKED: u32 = 1u32;
pub const CLUS_HYBRID_QUORUM: u32 = 1024u32;
pub const CLUS_MODIFY: u32 = 1u32;
pub const CLUS_NAME_RES_TYPE_CLUSTER_GROUPID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterGroupId");
pub const CLUS_NAME_RES_TYPE_DATA_RESID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DataResourceId");
pub const CLUS_NAME_RES_TYPE_LOG_MULTIPLE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogSizeMultiple");
pub const CLUS_NAME_RES_TYPE_LOG_RESID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogResourceId");
pub const CLUS_NAME_RES_TYPE_LOG_VOLUME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("LogVolume");
pub const CLUS_NAME_RES_TYPE_MINIMUM_LOG_SIZE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MinimumLogSizeInBytes");
pub const CLUS_NAME_RES_TYPE_REPLICATION_GROUPID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ReplicationGroupId");
pub const CLUS_NAME_RES_TYPE_REPLICATION_GROUP_TYPE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ReplicationClusterGroupType");
pub const CLUS_NAME_RES_TYPE_SOURCE_RESID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SourceResourceId");
pub const CLUS_NAME_RES_TYPE_SOURCE_VOLUMES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SourceVolumes");
pub const CLUS_NAME_RES_TYPE_TARGET_RESID: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TargetResourceId");
pub const CLUS_NAME_RES_TYPE_TARGET_VOLUMES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("TargetVolumes");
pub const CLUS_NAME_RES_TYPE_UNIT_LOG_SIZE_CHANGE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UnitOfLogSizeChangeInBytes");
pub const CLUS_NODE_MAJORITY_QUORUM: u32 = 0u32;
pub const CLUS_NOT_GLOBAL: u32 = 0u32;
pub const CLUS_NO_MODIFY: u32 = 0u32;
pub const CLUS_OBJECT_AFFINITYRULE: CLUSTER_CONTROL_OBJECT = 9i32;
pub const CLUS_OBJECT_CLUSTER: CLUSTER_CONTROL_OBJECT = 7i32;
pub const CLUS_OBJECT_GROUP: CLUSTER_CONTROL_OBJECT = 3i32;
pub const CLUS_OBJECT_GROUPSET: CLUSTER_CONTROL_OBJECT = 8i32;
pub const CLUS_OBJECT_INVALID: CLUSTER_CONTROL_OBJECT = 0i32;
pub const CLUS_OBJECT_NETINTERFACE: CLUSTER_CONTROL_OBJECT = 6i32;
pub const CLUS_OBJECT_NETWORK: CLUSTER_CONTROL_OBJECT = 5i32;
pub const CLUS_OBJECT_NODE: CLUSTER_CONTROL_OBJECT = 4i32;
pub const CLUS_OBJECT_RESOURCE: CLUSTER_CONTROL_OBJECT = 1i32;
pub const CLUS_OBJECT_RESOURCE_TYPE: CLUSTER_CONTROL_OBJECT = 2i32;
pub const CLUS_OBJECT_USER: CLUSTER_CONTROL_OBJECT = 128i32;
pub const CLUS_RESCLASS_NETWORK: CLUSTER_RESOURCE_CLASS = 2i32;
pub const CLUS_RESCLASS_STORAGE: CLUSTER_RESOURCE_CLASS = 1i32;
pub const CLUS_RESCLASS_UNKNOWN: CLUSTER_RESOURCE_CLASS = 0i32;
pub const CLUS_RESCLASS_USER: CLUSTER_RESOURCE_CLASS = 32768i32;
pub const CLUS_RESDLL_OFFLINE_DO_NOT_UPDATE_PERSISTENT_STATE: u32 = 64u32;
pub const CLUS_RESDLL_OFFLINE_DUE_TO_EMBEDDED_FAILURE: u32 = 16u32;
pub const CLUS_RESDLL_OFFLINE_IGNORE_NETWORK_CONNECTIVITY: u32 = 32u32;
pub const CLUS_RESDLL_OFFLINE_IGNORE_RESOURCE_STATUS: u32 = 1u32;
pub const CLUS_RESDLL_OFFLINE_QUEUE_ENABLED: u32 = 4u32;
pub const CLUS_RESDLL_OFFLINE_RETURNING_TO_SOURCE_NODE_BECAUSE_OF_ERROR: u32 = 8u32;
pub const CLUS_RESDLL_OFFLINE_RETURN_TO_SOURCE_NODE_ON_ERROR: u32 = 2u32;
pub const CLUS_RESDLL_ONLINE_IGNORE_NETWORK_CONNECTIVITY: u32 = 16u32;
pub const CLUS_RESDLL_ONLINE_IGNORE_RESOURCE_STATUS: u32 = 2u32;
pub const CLUS_RESDLL_ONLINE_RECOVER_MONITOR_STATE: u32 = 1u32;
pub const CLUS_RESDLL_ONLINE_RESTORE_ONLINE_STATE: u32 = 8u32;
pub const CLUS_RESDLL_ONLINE_RETURN_TO_SOURCE_NODE_ON_ERROR: u32 = 4u32;
pub const CLUS_RESDLL_OPEN_DONT_DELETE_TEMP_DISK: u32 = 2u32;
pub const CLUS_RESDLL_OPEN_RECOVER_MONITOR_STATE: u32 = 1u32;
pub const CLUS_RESSUBCLASS_NETWORK_INTERNET_PROTOCOL: CLUS_RESSUBCLASS_NETWORK = -2147483648i32;
pub const CLUS_RESSUBCLASS_SHARED: CLUS_RESSUBCLASS = -2147483648i32;
pub const CLUS_RESSUBCLASS_STORAGE_DISK: CLUS_RESSUBCLASS_STORAGE = 1073741824i32;
pub const CLUS_RESSUBCLASS_STORAGE_REPLICATION: CLUS_RESSUBCLASS_STORAGE = 268435456i32;
pub const CLUS_RESSUBCLASS_STORAGE_SHARED_BUS: CLUS_RESSUBCLASS_STORAGE = -2147483648i32;
pub const CLUS_RESTYPE_NAME_CAU: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("ClusterAwareUpdatingResource");
pub const CLUS_RESTYPE_NAME_CLOUD_WITNESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Cloud Witness");
pub const CLUS_RESTYPE_NAME_CONTAINER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Container");
pub const CLUS_RESTYPE_NAME_CROSS_CLUSTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Cross Cluster Dependency Orchestrator");
pub const CLUS_RESTYPE_NAME_DFS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Distributed File System");
pub const CLUS_RESTYPE_NAME_DFSR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DFS Replicated Folder");
pub const CLUS_RESTYPE_NAME_DHCP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("DHCP Service");
pub const CLUS_RESTYPE_NAME_DNN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Distributed Network Name");
pub const CLUS_RESTYPE_NAME_FILESERVER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("File Server");
pub const CLUS_RESTYPE_NAME_FILESHR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("File Share");
pub const CLUS_RESTYPE_NAME_FSWITNESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("File Share Witness");
pub const CLUS_RESTYPE_NAME_GENAPP: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Generic Application");
pub const CLUS_RESTYPE_NAME_GENSCRIPT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Generic Script");
pub const CLUS_RESTYPE_NAME_GENSVC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Generic Service");
pub const CLUS_RESTYPE_NAME_HARDDISK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Physical Disk");
pub const CLUS_RESTYPE_NAME_HCSVM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("HCS Virtual Machine");
pub const CLUS_RESTYPE_NAME_HEALTH_SERVICE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Health Service");
pub const CLUS_RESTYPE_NAME_IPADDR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IP Address");
pub const CLUS_RESTYPE_NAME_IPV6_NATIVE: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv6 Address");
pub const CLUS_RESTYPE_NAME_IPV6_TUNNEL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("IPv6 Tunnel Address");
pub const CLUS_RESTYPE_NAME_ISCSITARGET: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("iSCSI Target Server");
pub const CLUS_RESTYPE_NAME_ISNS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Microsoft iSNS");
pub const CLUS_RESTYPE_NAME_MSDTC: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Distributed Transaction Coordinator");
pub const CLUS_RESTYPE_NAME_MSMQ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Microsoft Message Queue Server");
pub const CLUS_RESTYPE_NAME_MSMQ_TRIGGER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSMQTriggers");
pub const CLUS_RESTYPE_NAME_NAT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Nat");
pub const CLUS_RESTYPE_NAME_NETNAME: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Network Name");
pub const CLUS_RESTYPE_NAME_NETWORK_FILE_SYSTEM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Network File System");
pub const CLUS_RESTYPE_NAME_NEW_MSMQ: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MSMQ");
pub const CLUS_RESTYPE_NAME_NFS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NFS Share");
pub const CLUS_RESTYPE_NAME_NFS_MSNS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("NFS Multi Server Namespace");
pub const CLUS_RESTYPE_NAME_NFS_V2: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Network File System");
pub const CLUS_RESTYPE_NAME_NV_PROVIDER_ADDRESS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Provider Address");
pub const CLUS_RESTYPE_NAME_PHYS_DISK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Physical Disk");
pub const CLUS_RESTYPE_NAME_PRTSPLR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Print Spooler");
pub const CLUS_RESTYPE_NAME_SCALEOUT_MASTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Scaleout Master");
pub const CLUS_RESTYPE_NAME_SCALEOUT_WORKER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Scaleout Worker");
pub const CLUS_RESTYPE_NAME_SDDC_MANAGEMENT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("SDDC Management");
pub const CLUS_RESTYPE_NAME_SODAFILESERVER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Scale Out File Server");
pub const CLUS_RESTYPE_NAME_STORAGE_POLICIES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Storage Policies");
pub const CLUS_RESTYPE_NAME_STORAGE_POOL: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Storage Pool");
pub const CLUS_RESTYPE_NAME_STORAGE_REPLICA: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Storage Replica");
pub const CLUS_RESTYPE_NAME_STORQOS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Storage QoS Policy Manager");
pub const CLUS_RESTYPE_NAME_TASKSCHEDULER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Task Scheduler");
pub const CLUS_RESTYPE_NAME_VIRTUAL_IPV4: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Disjoint IPv4 Address");
pub const CLUS_RESTYPE_NAME_VIRTUAL_IPV6: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Disjoint IPv6 Address");
pub const CLUS_RESTYPE_NAME_VM: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Virtual Machine");
pub const CLUS_RESTYPE_NAME_VMREPLICA_BROKER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Virtual Machine Replication Broker");
pub const CLUS_RESTYPE_NAME_VMREPLICA_COORDINATOR: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Virtual Machine Replication Coordinator");
pub const CLUS_RESTYPE_NAME_VM_CONFIG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Virtual Machine Configuration");
pub const CLUS_RESTYPE_NAME_VM_WMI: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Virtual Machine Cluster WMI");
pub const CLUS_RESTYPE_NAME_VSSTASK: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Volume Shadow Copy Service Task");
pub const CLUS_RESTYPE_NAME_WINS: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("WINS Service");
pub const CLUS_RES_NAME_SCALEOUT_MASTER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Scaleout Master");
pub const CLUS_RES_NAME_SCALEOUT_WORKER: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("Scaleout Worker");
pub const CREATEDC_PRESENT: u32 = 2u32;
pub const CREATE_CLUSTER_MAJOR_VERSION_MASK: u32 = 4294967040u32;
pub const CREATE_CLUSTER_VERSION: u32 = 1536u32;
pub const CTCTL_GET_FAULT_DOMAIN_STATE: CLCTL_CODES = 789i32;
pub const CTCTL_GET_ROUTESTATUS_BASIC: CLCTL_CODES = 781i32;
pub const CTCTL_GET_ROUTESTATUS_EXTENDED: CLCTL_CODES = 785i32;
pub const ClusApplication: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606e5_2631_11d1_89f1_00a0c90d061e);
pub const ClusCryptoKeys: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6072b_2631_11d1_89f1_00a0c90d061e);
pub const ClusDisk: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60723_2631_11d1_89f1_00a0c90d061e);
pub const ClusDisks: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60725_2631_11d1_89f1_00a0c90d061e);
pub const ClusGroupTypeAvailableStorage: CLUSGROUP_TYPE = 2i32;
pub const ClusGroupTypeClusterUpdateAgent: CLUSGROUP_TYPE = 117i32;
pub const ClusGroupTypeCoreCluster: CLUSGROUP_TYPE = 1i32;
pub const ClusGroupTypeCoreSddc: CLUSGROUP_TYPE = 123i32;
pub const ClusGroupTypeCrossClusterOrchestrator: CLUSGROUP_TYPE = 121i32;
pub const ClusGroupTypeDhcpServer: CLUSGROUP_TYPE = 102i32;
pub const ClusGroupTypeDtc: CLUSGROUP_TYPE = 103i32;
pub const ClusGroupTypeFileServer: CLUSGROUP_TYPE = 100i32;
pub const ClusGroupTypeGenericApplication: CLUSGROUP_TYPE = 107i32;
pub const ClusGroupTypeGenericScript: CLUSGROUP_TYPE = 109i32;
pub const ClusGroupTypeGenericService: CLUSGROUP_TYPE = 108i32;
pub const ClusGroupTypeIScsiNameService: CLUSGROUP_TYPE = 110i32;
pub const ClusGroupTypeIScsiTarget: CLUSGROUP_TYPE = 113i32;
pub const ClusGroupTypeInfrastructureFileServer: CLUSGROUP_TYPE = 122i32;
pub const ClusGroupTypeMsmq: CLUSGROUP_TYPE = 104i32;
pub const ClusGroupTypePrintServer: CLUSGROUP_TYPE = 101i32;
pub const ClusGroupTypeScaleoutCluster: CLUSGROUP_TYPE = 118i32;
pub const ClusGroupTypeScaleoutFileServer: CLUSGROUP_TYPE = 114i32;
pub const ClusGroupTypeSharedVolume: CLUSGROUP_TYPE = 4i32;
pub const ClusGroupTypeStandAloneDfs: CLUSGROUP_TYPE = 106i32;
pub const ClusGroupTypeStoragePool: CLUSGROUP_TYPE = 5i32;
pub const ClusGroupTypeStorageReplica: CLUSGROUP_TYPE = 119i32;
pub const ClusGroupTypeTaskScheduler: CLUSGROUP_TYPE = 116i32;
pub const ClusGroupTypeTemporary: CLUSGROUP_TYPE = 3i32;
pub const ClusGroupTypeTsSessionBroker: CLUSGROUP_TYPE = 112i32;
pub const ClusGroupTypeUnknown: CLUSGROUP_TYPE = 9999i32;
pub const ClusGroupTypeVMReplicaBroker: CLUSGROUP_TYPE = 115i32;
pub const ClusGroupTypeVMReplicaCoordinator: CLUSGROUP_TYPE = 120i32;
pub const ClusGroupTypeVirtualMachine: CLUSGROUP_TYPE = 111i32;
pub const ClusGroupTypeWins: CLUSGROUP_TYPE = 105i32;
pub const ClusNetInterface: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606ed_2631_11d1_89f1_00a0c90d061e);
pub const ClusNetInterfaces: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606ef_2631_11d1_89f1_00a0c90d061e);
pub const ClusNetwork: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606f1_2631_11d1_89f1_00a0c90d061e);
pub const ClusNetworkNetInterfaces: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606f5_2631_11d1_89f1_00a0c90d061e);
pub const ClusNetworks: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606f3_2631_11d1_89f1_00a0c90d061e);
pub const ClusNode: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606f7_2631_11d1_89f1_00a0c90d061e);
pub const ClusNodeNetInterfaces: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606fb_2631_11d1_89f1_00a0c90d061e);
pub const ClusNodes: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606f9_2631_11d1_89f1_00a0c90d061e);
pub const ClusPartition: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6071f_2631_11d1_89f1_00a0c90d061e);
pub const ClusPartitionEx: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x53d51d26_b51b_4a79_b2c3_5048d93a98fc);
pub const ClusPartitions: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60721_2631_11d1_89f1_00a0c90d061e);
pub const ClusProperties: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606ff_2631_11d1_89f1_00a0c90d061e);
pub const ClusProperty: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606fd_2631_11d1_89f1_00a0c90d061e);
pub const ClusPropertyValue: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60719_2631_11d1_89f1_00a0c90d061e);
pub const ClusPropertyValueData: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6071d_2631_11d1_89f1_00a0c90d061e);
pub const ClusPropertyValues: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6071b_2631_11d1_89f1_00a0c90d061e);
pub const ClusRefObject: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60701_2631_11d1_89f1_00a0c90d061e);
pub const ClusRegistryKeys: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60729_2631_11d1_89f1_00a0c90d061e);
pub const ClusResDependencies: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60703_2631_11d1_89f1_00a0c90d061e);
pub const ClusResDependents: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6072d_2631_11d1_89f1_00a0c90d061e);
pub const ClusResGroup: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60705_2631_11d1_89f1_00a0c90d061e);
pub const ClusResGroupPreferredOwnerNodes: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606e7_2631_11d1_89f1_00a0c90d061e);
pub const ClusResGroupResources: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606e9_2631_11d1_89f1_00a0c90d061e);
pub const ClusResGroups: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60707_2631_11d1_89f1_00a0c90d061e);
pub const ClusResPossibleOwnerNodes: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6070d_2631_11d1_89f1_00a0c90d061e);
pub const ClusResType: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6070f_2631_11d1_89f1_00a0c90d061e);
pub const ClusResTypePossibleOwnerNodes: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60717_2631_11d1_89f1_00a0c90d061e);
pub const ClusResTypeResources: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60713_2631_11d1_89f1_00a0c90d061e);
pub const ClusResTypes: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60711_2631_11d1_89f1_00a0c90d061e);
pub const ClusResource: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60709_2631_11d1_89f1_00a0c90d061e);
pub const ClusResources: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e6070b_2631_11d1_89f1_00a0c90d061e);
pub const ClusScsiAddress: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60727_2631_11d1_89f1_00a0c90d061e);
pub const ClusVersion: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e60715_2631_11d1_89f1_00a0c90d061e);
pub const Cluster: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606e3_2631_11d1_89f1_00a0c90d061e);
pub const ClusterGroupAllowFailback: CLUSTER_GROUP_AUTOFAILBACK_TYPE = 1i32;
pub const ClusterGroupFailbackTypeCount: CLUSTER_GROUP_AUTOFAILBACK_TYPE = 2i32;
pub const ClusterGroupFailed: CLUSTER_GROUP_STATE = 2i32;
pub const ClusterGroupOffline: CLUSTER_GROUP_STATE = 1i32;
pub const ClusterGroupOnline: CLUSTER_GROUP_STATE = 0i32;
pub const ClusterGroupPartialOnline: CLUSTER_GROUP_STATE = 3i32;
pub const ClusterGroupPending: CLUSTER_GROUP_STATE = 4i32;
pub const ClusterGroupPreventFailback: CLUSTER_GROUP_AUTOFAILBACK_TYPE = 0i32;
pub const ClusterGroupStateUnknown: CLUSTER_GROUP_STATE = -1i32;
pub const ClusterNames: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606eb_2631_11d1_89f1_00a0c90d061e);
pub const ClusterNetInterfaceFailed: CLUSTER_NETINTERFACE_STATE = 1i32;
pub const ClusterNetInterfaceStateUnknown: CLUSTER_NETINTERFACE_STATE = -1i32;
pub const ClusterNetInterfaceUnavailable: CLUSTER_NETINTERFACE_STATE = 0i32;
pub const ClusterNetInterfaceUnreachable: CLUSTER_NETINTERFACE_STATE = 2i32;
pub const ClusterNetInterfaceUp: CLUSTER_NETINTERFACE_STATE = 3i32;
pub const ClusterNetworkDown: CLUSTER_NETWORK_STATE = 1i32;
pub const ClusterNetworkPartitioned: CLUSTER_NETWORK_STATE = 2i32;
pub const ClusterNetworkRoleClientAccess: CLUSTER_NETWORK_ROLE = 2i32;
pub const ClusterNetworkRoleInternalAndClient: CLUSTER_NETWORK_ROLE = 3i32;
pub const ClusterNetworkRoleInternalUse: CLUSTER_NETWORK_ROLE = 1i32;
pub const ClusterNetworkRoleNone: CLUSTER_NETWORK_ROLE = 0i32;
pub const ClusterNetworkStateUnknown: CLUSTER_NETWORK_STATE = -1i32;
pub const ClusterNetworkUnavailable: CLUSTER_NETWORK_STATE = 0i32;
pub const ClusterNetworkUp: CLUSTER_NETWORK_STATE = 3i32;
pub const ClusterNodeDown: CLUSTER_NODE_STATE = 1i32;
pub const ClusterNodeDrainStatusCount: CLUSTER_NODE_DRAIN_STATUS = 4i32;
pub const ClusterNodeJoining: CLUSTER_NODE_STATE = 3i32;
pub const ClusterNodePaused: CLUSTER_NODE_STATE = 2i32;
pub const ClusterNodeResumeFailbackTypeCount: CLUSTER_NODE_RESUME_FAILBACK_TYPE = 3i32;
pub const ClusterNodeStateUnknown: CLUSTER_NODE_STATE = -1i32;
pub const ClusterNodeUp: CLUSTER_NODE_STATE = 0i32;
pub const ClusterResourceApplicationOSHeartBeat: CLUSTER_RESOURCE_APPLICATION_STATE = 2i32;
pub const ClusterResourceApplicationReady: CLUSTER_RESOURCE_APPLICATION_STATE = 3i32;
pub const ClusterResourceApplicationStateUnknown: CLUSTER_RESOURCE_APPLICATION_STATE = 1i32;
pub const ClusterResourceDontRestart: CLUSTER_RESOURCE_RESTART_ACTION = 0i32;
pub const ClusterResourceEmbeddedFailureActionLogOnly: CLUSTER_RESOURCE_EMBEDDED_FAILURE_ACTION = 1i32;
pub const ClusterResourceEmbeddedFailureActionNone: CLUSTER_RESOURCE_EMBEDDED_FAILURE_ACTION = 0i32;
pub const ClusterResourceEmbeddedFailureActionRecover: CLUSTER_RESOURCE_EMBEDDED_FAILURE_ACTION = 2i32;
pub const ClusterResourceFailed: CLUSTER_RESOURCE_STATE = 4i32;
pub const ClusterResourceInherited: CLUSTER_RESOURCE_STATE = 0i32;
pub const ClusterResourceInitializing: CLUSTER_RESOURCE_STATE = 1i32;
pub const ClusterResourceOffline: CLUSTER_RESOURCE_STATE = 3i32;
pub const ClusterResourceOfflinePending: CLUSTER_RESOURCE_STATE = 130i32;
pub const ClusterResourceOnline: CLUSTER_RESOURCE_STATE = 2i32;
pub const ClusterResourceOnlinePending: CLUSTER_RESOURCE_STATE = 129i32;
pub const ClusterResourcePending: CLUSTER_RESOURCE_STATE = 128i32;
pub const ClusterResourceRestartActionCount: CLUSTER_RESOURCE_RESTART_ACTION = 3i32;
pub const ClusterResourceRestartNoNotify: CLUSTER_RESOURCE_RESTART_ACTION = 1i32;
pub const ClusterResourceRestartNotify: CLUSTER_RESOURCE_RESTART_ACTION = 2i32;
pub const ClusterResourceStateUnknown: CLUSTER_RESOURCE_STATE = -1i32;
pub const ClusterRoleClustered: CLUSTER_ROLE_STATE = 0i32;
pub const ClusterRoleDFSReplicatedFolder: CLUSTER_ROLE = 15i32;
pub const ClusterRoleDHCP: CLUSTER_ROLE = 0i32;
pub const ClusterRoleDTC: CLUSTER_ROLE = 1i32;
pub const ClusterRoleDistributedFileSystem: CLUSTER_ROLE = 16i32;
pub const ClusterRoleDistributedNetworkName: CLUSTER_ROLE = 17i32;
pub const ClusterRoleFileServer: CLUSTER_ROLE = 2i32;
pub const ClusterRoleFileShare: CLUSTER_ROLE = 18i32;
pub const ClusterRoleFileShareWitness: CLUSTER_ROLE = 19i32;
pub const ClusterRoleGenericApplication: CLUSTER_ROLE = 3i32;
pub const ClusterRoleGenericScript: CLUSTER_ROLE = 4i32;
pub const ClusterRoleGenericService: CLUSTER_ROLE = 5i32;
pub const ClusterRoleHardDisk: CLUSTER_ROLE = 20i32;
pub const ClusterRoleIPAddress: CLUSTER_ROLE = 21i32;
pub const ClusterRoleIPV6Address: CLUSTER_ROLE = 22i32;
pub const ClusterRoleIPV6TunnelAddress: CLUSTER_ROLE = 23i32;
pub const ClusterRoleISCSINameServer: CLUSTER_ROLE = 6i32;
pub const ClusterRoleISCSITargetServer: CLUSTER_ROLE = 24i32;
pub const ClusterRoleMSMQ: CLUSTER_ROLE = 7i32;
pub const ClusterRoleNFS: CLUSTER_ROLE = 8i32;
pub const ClusterRoleNetworkFileSystem: CLUSTER_ROLE = 14i32;
pub const ClusterRoleNetworkName: CLUSTER_ROLE = 25i32;
pub const ClusterRolePhysicalDisk: CLUSTER_ROLE = 26i32;
pub const ClusterRolePrintServer: CLUSTER_ROLE = 9i32;
pub const ClusterRoleSODAFileServer: CLUSTER_ROLE = 27i32;
pub const ClusterRoleStandAloneNamespaceServer: CLUSTER_ROLE = 10i32;
pub const ClusterRoleStoragePool: CLUSTER_ROLE = 28i32;
pub const ClusterRoleTaskScheduler: CLUSTER_ROLE = 13i32;
pub const ClusterRoleUnclustered: CLUSTER_ROLE_STATE = 1i32;
pub const ClusterRoleUnknown: CLUSTER_ROLE_STATE = -1i32;
pub const ClusterRoleVirtualMachine: CLUSTER_ROLE = 29i32;
pub const ClusterRoleVirtualMachineConfiguration: CLUSTER_ROLE = 30i32;
pub const ClusterRoleVirtualMachineReplicaBroker: CLUSTER_ROLE = 31i32;
pub const ClusterRoleVolumeShadowCopyServiceTask: CLUSTER_ROLE = 11i32;
pub const ClusterRoleWINS: CLUSTER_ROLE = 12i32;
pub const ClusterSetupPhaseAddClusterProperties: CLUSTER_SETUP_PHASE = 201i32;
pub const ClusterSetupPhaseAddNodeToCluster: CLUSTER_SETUP_PHASE = 301i32;
pub const ClusterSetupPhaseCleanupCOs: CLUSTER_SETUP_PHASE = 402i32;
pub const ClusterSetupPhaseCleanupNode: CLUSTER_SETUP_PHASE = 405i32;
pub const ClusterSetupPhaseClusterGroupOnline: CLUSTER_SETUP_PHASE = 206i32;
pub const ClusterSetupPhaseConfigureClusSvc: CLUSTER_SETUP_PHASE = 104i32;
pub const ClusterSetupPhaseConfigureClusterAccount: CLUSTER_SETUP_PHASE = 109i32;
pub const ClusterSetupPhaseContinue: CLUSTER_SETUP_PHASE_TYPE = 2i32;
pub const ClusterSetupPhaseCoreGroupCleanup: CLUSTER_SETUP_PHASE = 406i32;
pub const ClusterSetupPhaseCreateClusterAccount: CLUSTER_SETUP_PHASE = 108i32;
pub const ClusterSetupPhaseCreateGroups: CLUSTER_SETUP_PHASE = 203i32;
pub const ClusterSetupPhaseCreateIPAddressResources: CLUSTER_SETUP_PHASE = 204i32;
pub const ClusterSetupPhaseCreateNetworkName: CLUSTER_SETUP_PHASE = 205i32;
pub const ClusterSetupPhaseCreateResourceTypes: CLUSTER_SETUP_PHASE = 202i32;
pub const ClusterSetupPhaseDeleteGroup: CLUSTER_SETUP_PHASE = 401i32;
pub const ClusterSetupPhaseEnd: CLUSTER_SETUP_PHASE_TYPE = 3i32;
pub const ClusterSetupPhaseEvictNode: CLUSTER_SETUP_PHASE = 404i32;
pub const ClusterSetupPhaseFailureCleanup: CLUSTER_SETUP_PHASE = 999i32;
pub const ClusterSetupPhaseFatal: CLUSTER_SETUP_PHASE_SEVERITY = 3i32;
pub const ClusterSetupPhaseFormingCluster: CLUSTER_SETUP_PHASE = 200i32;
pub const ClusterSetupPhaseGettingCurrentMembership: CLUSTER_SETUP_PHASE = 300i32;
pub const ClusterSetupPhaseInformational: CLUSTER_SETUP_PHASE_SEVERITY = 1i32;
pub const ClusterSetupPhaseInitialize: CLUSTER_SETUP_PHASE = 1i32;
pub const ClusterSetupPhaseMoveGroup: CLUSTER_SETUP_PHASE = 400i32;
pub const ClusterSetupPhaseNodeUp: CLUSTER_SETUP_PHASE = 302i32;
pub const ClusterSetupPhaseOfflineGroup: CLUSTER_SETUP_PHASE = 403i32;
pub const ClusterSetupPhaseQueryClusterNameAccount: CLUSTER_SETUP_PHASE = 106i32;
pub const ClusterSetupPhaseReport: CLUSTER_SETUP_PHASE_TYPE = 4i32;
pub const ClusterSetupPhaseStart: CLUSTER_SETUP_PHASE_TYPE = 1i32;
pub const ClusterSetupPhaseStartingClusSvc: CLUSTER_SETUP_PHASE = 105i32;
pub const ClusterSetupPhaseValidateClusDisk: CLUSTER_SETUP_PHASE = 103i32;
pub const ClusterSetupPhaseValidateClusterNameAccount: CLUSTER_SETUP_PHASE = 107i32;
pub const ClusterSetupPhaseValidateNetft: CLUSTER_SETUP_PHASE = 102i32;
pub const ClusterSetupPhaseValidateNodeState: CLUSTER_SETUP_PHASE = 100i32;
pub const ClusterSetupPhaseWarning: CLUSTER_SETUP_PHASE_SEVERITY = 2i32;
pub const ClusterSharedVolumeHWSnapshotCompleted: CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE = 2i32;
pub const ClusterSharedVolumePrepareForFreeze: CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE = 3i32;
pub const ClusterSharedVolumePrepareForHWSnapshot: CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE = 1i32;
pub const ClusterSharedVolumeRenameInputTypeNone: CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE = 0i32;
pub const ClusterSharedVolumeRenameInputTypeVolumeGuid: CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE = 4i32;
pub const ClusterSharedVolumeRenameInputTypeVolumeId: CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE = 2i32;
pub const ClusterSharedVolumeRenameInputTypeVolumeName: CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE = 3i32;
pub const ClusterSharedVolumeRenameInputTypeVolumeOffset: CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE = 1i32;
pub const ClusterSharedVolumeSnapshotStateUnknown: CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE = 0i32;
pub const ClusterStateNotConfigured: NODE_CLUSTER_STATE = 1i32;
pub const ClusterStateNotInstalled: NODE_CLUSTER_STATE = 0i32;
pub const ClusterStateNotRunning: NODE_CLUSTER_STATE = 3i32;
pub const ClusterStateRunning: NODE_CLUSTER_STATE = 19i32;
pub const ClusterStorageNodeDown: CLUSTER_STORAGENODE_STATE = 2i32;
pub const ClusterStorageNodePaused: CLUSTER_STORAGENODE_STATE = 3i32;
pub const ClusterStorageNodeStarting: CLUSTER_STORAGENODE_STATE = 4i32;
pub const ClusterStorageNodeStateUnknown: CLUSTER_STORAGENODE_STATE = 0i32;
pub const ClusterStorageNodeStopping: CLUSTER_STORAGENODE_STATE = 5i32;
pub const ClusterStorageNodeUp: CLUSTER_STORAGENODE_STATE = 1i32;
pub const ClusterUpgradePhaseInitialize: CLUSTER_UPGRADE_PHASE = 1i32;
pub const ClusterUpgradePhaseInstallingNewComponents: CLUSTER_UPGRADE_PHASE = 4i32;
pub const ClusterUpgradePhaseUpgradeComplete: CLUSTER_UPGRADE_PHASE = 5i32;
pub const ClusterUpgradePhaseUpgradingComponents: CLUSTER_UPGRADE_PHASE = 3i32;
pub const ClusterUpgradePhaseValidatingUpgrade: CLUSTER_UPGRADE_PHASE = 2i32;
pub const DNS_LENGTH: u32 = 64u32;
pub const DoNotFailbackGroups: CLUSTER_NODE_RESUME_FAILBACK_TYPE = 0i32;
pub const DomainNames: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf2e606e1_2631_11d1_89f1_00a0c90d061e);
pub const ENABLE_CLUSTER_SHARED_VOLUMES: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("EnableSharedVolumes");
pub const FAILURE_TYPE_EMBEDDED: FAILURE_TYPE = 1i32;
pub const FAILURE_TYPE_GENERAL: FAILURE_TYPE = 0i32;
pub const FAILURE_TYPE_NETWORK_LOSS: FAILURE_TYPE = 2i32;
pub const FE_UPGRADE_VERSION: u32 = 4u32;
pub const FILESHARE_CHANGE_ADD: FILESHARE_CHANGE_ENUM = 1i32;
pub const FILESHARE_CHANGE_DEL: FILESHARE_CHANGE_ENUM = 2i32;
pub const FILESHARE_CHANGE_MODIFY: FILESHARE_CHANGE_ENUM = 3i32;
pub const FILESHARE_CHANGE_NONE: FILESHARE_CHANGE_ENUM = 0i32;
pub const FailbackGroupsImmediately: CLUSTER_NODE_RESUME_FAILBACK_TYPE = 1i32;
pub const FailbackGroupsPerPolicy: CLUSTER_NODE_RESUME_FAILBACK_TYPE = 2i32;
pub const GROUPSET_READY_SETTING_APPLICATION_READY: u32 = 4u32;
pub const GROUPSET_READY_SETTING_DELAY: u32 = 1u32;
pub const GROUPSET_READY_SETTING_ONLINE: u32 = 2u32;
pub const GROUPSET_READY_SETTING_OS_HEARTBEAT: u32 = 3u32;
pub const GROUP_FAILURE_INFO_VERSION_1: u32 = 1u32;
pub const GRP_PLACEMENT_OPTIONS_ALL: GRP_PLACEMENT_OPTIONS = 1i32;
pub const GRP_PLACEMENT_OPTIONS_DEFAULT: GRP_PLACEMENT_OPTIONS = 0i32;
pub const GRP_PLACEMENT_OPTIONS_DISABLE_AUTOBALANCING: GRP_PLACEMENT_OPTIONS = 1i32;
pub const GRP_PLACEMENT_OPTIONS_MIN_VALUE: GRP_PLACEMENT_OPTIONS = 0i32;
pub const GUID_PRESENT: u32 = 1u32;
pub const HCI_UPGRADE_BIT: u32 = 32768u32;
pub const LOCKED_MODE_FLAGS_DONT_REMOVE_FROM_MOVE_QUEUE: u32 = 1u32;
pub const LOG_ERROR: LOG_LEVEL = 2i32;
pub const LOG_INFORMATION: LOG_LEVEL = 0i32;
pub const LOG_SEVERE: LOG_LEVEL = 3i32;
pub const LOG_WARNING: LOG_LEVEL = 1i32;
pub const MAINTENANCE_MODE_V2_SIG: u32 = 2881155087u32;
pub const MAX_CLUSTERNAME_LENGTH: u32 = 63u32;
pub const MAX_CO_PASSWORD_LENGTH: u32 = 16u32;
pub const MAX_CO_PASSWORD_LENGTHEX: u32 = 127u32;
pub const MAX_CO_PASSWORD_STORAGEEX: u32 = 128u32;
pub const MAX_CREATINGDC_LENGTH: u32 = 256u32;
pub const MAX_OBJECTID: u32 = 64u32;
pub const MINIMUM_NEVER_PREEMPT_PRIORITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MinimumNeverPreemptPriority");
pub const MINIMUM_PREEMPTOR_PRIORITY: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("MinimumPreemptorPriority");
pub const MN_UPGRADE_VERSION: u32 = 3u32;
pub const MaintenanceModeTypeDisableIsAliveCheck: MAINTENANCE_MODE_TYPE_ENUM = 1i32;
pub const MaintenanceModeTypeOfflineResource: MAINTENANCE_MODE_TYPE_ENUM = 2i32;
pub const MaintenanceModeTypeUnclusterResource: MAINTENANCE_MODE_TYPE_ENUM = 3i32;
pub const ModifyQuorum: CLUSTER_QUORUM_TYPE = 1i32;
pub const NINETEEN_H1_UPGRADE_VERSION: u32 = 1u32;
pub const NINETEEN_H2_UPGRADE_VERSION: u32 = 2u32;
pub const NI_UPGRADE_VERSION: u32 = 2u32;
pub const NNLEN: u32 = 80u32;
pub const NT10_MAJOR_VERSION: u32 = 9u32;
pub const NT11_MAJOR_VERSION: u32 = 10u32;
pub const NT12_MAJOR_VERSION: u32 = 11u32;
pub const NT13_MAJOR_VERSION: u32 = 12u32;
pub const NT4SP4_MAJOR_VERSION: u32 = 2u32;
pub const NT4_MAJOR_VERSION: u32 = 1u32;
pub const NT51_MAJOR_VERSION: u32 = 4u32;
pub const NT5_MAJOR_VERSION: u32 = 3u32;
pub const NT6_MAJOR_VERSION: u32 = 5u32;
pub const NT7_MAJOR_VERSION: u32 = 6u32;
pub const NT8_MAJOR_VERSION: u32 = 7u32;
pub const NT9_MAJOR_VERSION: u32 = 8u32;
pub const NodeDrainStatusCompleted: CLUSTER_NODE_DRAIN_STATUS = 2i32;
pub const NodeDrainStatusFailed: CLUSTER_NODE_DRAIN_STATUS = 3i32;
pub const NodeDrainStatusInProgress: CLUSTER_NODE_DRAIN_STATUS = 1i32;
pub const NodeDrainStatusNotInitiated: CLUSTER_NODE_DRAIN_STATUS = 0i32;
pub const NodeStatusAvoidPlacement: CLUSTER_NODE_STATUS = 32i32;
pub const NodeStatusDrainCompleted: CLUSTER_NODE_STATUS = 8i32;
pub const NodeStatusDrainFailed: CLUSTER_NODE_STATUS = 16i32;
pub const NodeStatusDrainInProgress: CLUSTER_NODE_STATUS = 4i32;
pub const NodeStatusIsolated: CLUSTER_NODE_STATUS = 1i32;
pub const NodeStatusMax: CLUSTER_NODE_STATUS = 51i32;
pub const NodeStatusNormal: CLUSTER_NODE_STATUS = 0i32;
pub const NodeStatusQuarantined: CLUSTER_NODE_STATUS = 2i32;
pub const OperationalQuorum: CLUSTER_QUORUM_TYPE = 0i32;
pub const PLACEMENT_OPTIONS_ALL: PLACEMENT_OPTIONS = 1023i32;
pub const PLACEMENT_OPTIONS_AVAILABILITY_SET_DOMAIN_AFFINITY: PLACEMENT_OPTIONS = 512i32;
pub const PLACEMENT_OPTIONS_CONSIDER_OFFLINE_VMS: PLACEMENT_OPTIONS = 2i32;
pub const PLACEMENT_OPTIONS_DEFAULT_PLACEMENT_OPTIONS: PLACEMENT_OPTIONS = 0i32;
pub const PLACEMENT_OPTIONS_DISABLE_CSV_VM_DEPENDENCY: PLACEMENT_OPTIONS = 1i32;
pub const PLACEMENT_OPTIONS_DONT_RESUME_AVAILABILTY_SET_VMS_WITH_EXISTING_TEMP_DISK: PLACEMENT_OPTIONS = 128i32;
pub const PLACEMENT_OPTIONS_DONT_RESUME_VMS_WITH_EXISTING_TEMP_DISK: PLACEMENT_OPTIONS = 32i32;
pub const PLACEMENT_OPTIONS_DONT_USE_CPU: PLACEMENT_OPTIONS = 8i32;
pub const PLACEMENT_OPTIONS_DONT_USE_LOCAL_TEMP_DISK: PLACEMENT_OPTIONS = 16i32;
pub const PLACEMENT_OPTIONS_DONT_USE_MEMORY: PLACEMENT_OPTIONS = 4i32;
pub const PLACEMENT_OPTIONS_MIN_VALUE: PLACEMENT_OPTIONS = 0i32;
pub const PLACEMENT_OPTIONS_SAVE_AVAILABILTY_SET_VMS_WITH_LOCAL_DISK_ON_DRAIN_OVERWRITE: PLACEMENT_OPTIONS = 256i32;
pub const PLACEMENT_OPTIONS_SAVE_VMS_WITH_LOCAL_DISK_ON_DRAIN_OVERWRITE: PLACEMENT_OPTIONS = 64i32;
pub const PriorityDisabled: CLUSTER_GROUP_PRIORITY = 0i32;
pub const PriorityHigh: CLUSTER_GROUP_PRIORITY = 3000i32;
pub const PriorityLow: CLUSTER_GROUP_PRIORITY = 1000i32;
pub const PriorityMedium: CLUSTER_GROUP_PRIORITY = 2000i32;
pub const RESOURCE_FAILURE_INFO_VERSION_1: u32 = 1u32;
pub const RESTYPE_MONITOR_SHUTTING_DOWN_CLUSSVC_CRASH: u32 = 2u32;
pub const RESTYPE_MONITOR_SHUTTING_DOWN_NODE_STOP: u32 = 1u32;
pub const RESUTIL_PROPITEM_IN_MEMORY: u32 = 8u32;
pub const RESUTIL_PROPITEM_READ_ONLY: u32 = 1u32;
pub const RESUTIL_PROPITEM_REQUIRED: u32 = 2u32;
pub const RESUTIL_PROPITEM_SIGNED: u32 = 4u32;
pub const RS3_UPGRADE_VERSION: u32 = 1u32;
pub const RS4_UPGRADE_VERSION: u32 = 2u32;
pub const RS5_UPGRADE_VERSION: u32 = 3u32;
pub const RedirectedIOReasonBitLockerInitializing: u64 = 16u64;
pub const RedirectedIOReasonFileSystemTiering: u64 = 8u64;
pub const RedirectedIOReasonMax: u64 = 9223372036854775808u64;
pub const RedirectedIOReasonReFs: u64 = 32u64;
pub const RedirectedIOReasonUnsafeFileSystemFilter: u64 = 2u64;
pub const RedirectedIOReasonUnsafeVolumeFilter: u64 = 4u64;
pub const RedirectedIOReasonUserRequest: u64 = 1u64;
pub const ResdllContextOperationTypeDrain: RESDLL_CONTEXT_OPERATION_TYPE = 1i32;
pub const ResdllContextOperationTypeDrainFailure: RESDLL_CONTEXT_OPERATION_TYPE = 2i32;
pub const ResdllContextOperationTypeEmbeddedFailure: RESDLL_CONTEXT_OPERATION_TYPE = 3i32;
pub const ResdllContextOperationTypeFailback: RESDLL_CONTEXT_OPERATION_TYPE = 0i32;
pub const ResdllContextOperationTypeNetworkDisconnect: RESDLL_CONTEXT_OPERATION_TYPE = 5i32;
pub const ResdllContextOperationTypeNetworkDisconnectMoveRetry: RESDLL_CONTEXT_OPERATION_TYPE = 6i32;
pub const ResdllContextOperationTypePreemption: RESDLL_CONTEXT_OPERATION_TYPE = 4i32;
pub const ResourceExitStateContinue: RESOURCE_EXIT_STATE = 0i32;
pub const ResourceExitStateMax: RESOURCE_EXIT_STATE = 2i32;
pub const ResourceExitStateTerminate: RESOURCE_EXIT_STATE = 1i32;
pub const RmonArbitrateResource: RESOURCE_MONITOR_STATE = 10i32;
pub const RmonDeadlocked: RESOURCE_MONITOR_STATE = 15i32;
pub const RmonDeletingResource: RESOURCE_MONITOR_STATE = 7i32;
pub const RmonIdle: RESOURCE_MONITOR_STATE = 1i32;
pub const RmonInitializing: RESOURCE_MONITOR_STATE = 0i32;
pub const RmonInitializingResource: RESOURCE_MONITOR_STATE = 3i32;
pub const RmonIsAlivePoll: RESOURCE_MONITOR_STATE = 8i32;
pub const RmonLooksAlivePoll: RESOURCE_MONITOR_STATE = 9i32;
pub const RmonOfflineResource: RESOURCE_MONITOR_STATE = 5i32;
pub const RmonOnlineResource: RESOURCE_MONITOR_STATE = 4i32;
pub const RmonReleaseResource: RESOURCE_MONITOR_STATE = 11i32;
pub const RmonResourceControl: RESOURCE_MONITOR_STATE = 12i32;
pub const RmonResourceTypeControl: RESOURCE_MONITOR_STATE = 13i32;
pub const RmonShutdownResource: RESOURCE_MONITOR_STATE = 6i32;
pub const RmonStartingResource: RESOURCE_MONITOR_STATE = 2i32;
pub const RmonTerminateResource: RESOURCE_MONITOR_STATE = 14i32;
pub const SET_APPINSTANCE_CSV_FLAGS_VALID_ONLY_IF_CSV_COORDINATOR: u32 = 1u32;
pub const SR_REPLICATED_PARTITION_DISALLOW_MULTINODE_IO: u32 = 1u32;
pub const STARTUP_EX_ROUTINE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("StartupEx");
pub const STARTUP_ROUTINE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Startup");
pub const SharedVolumeStateActive: CLUSTER_SHARED_VOLUME_STATE = 2i32;
pub const SharedVolumeStateActiveRedirected: CLUSTER_SHARED_VOLUME_STATE = 3i32;
pub const SharedVolumeStateActiveVolumeRedirected: CLUSTER_SHARED_VOLUME_STATE = 4i32;
pub const SharedVolumeStatePaused: CLUSTER_SHARED_VOLUME_STATE = 1i32;
pub const SharedVolumeStateUnavailable: CLUSTER_SHARED_VOLUME_STATE = 0i32;
pub const SrDiskReplicationEligibleAlreadyInReplication: SR_DISK_REPLICATION_ELIGIBLE = 9i32;
pub const SrDiskReplicationEligibleFileSystemNotSupported: SR_DISK_REPLICATION_ELIGIBLE = 8i32;
pub const SrDiskReplicationEligibleInSameSite: SR_DISK_REPLICATION_ELIGIBLE = 7i32;
pub const SrDiskReplicationEligibleInsufficientFreeSpace: SR_DISK_REPLICATION_ELIGIBLE = 5i32;
pub const SrDiskReplicationEligibleNone: SR_DISK_REPLICATION_ELIGIBLE = 0i32;
pub const SrDiskReplicationEligibleNotGpt: SR_DISK_REPLICATION_ELIGIBLE = 3i32;
pub const SrDiskReplicationEligibleNotInSameSite: SR_DISK_REPLICATION_ELIGIBLE = 6i32;
pub const SrDiskReplicationEligibleOffline: SR_DISK_REPLICATION_ELIGIBLE = 2i32;
pub const SrDiskReplicationEligibleOther: SR_DISK_REPLICATION_ELIGIBLE = 9999i32;
pub const SrDiskReplicationEligiblePartitionLayoutMismatch: SR_DISK_REPLICATION_ELIGIBLE = 4i32;
pub const SrDiskReplicationEligibleSameAsSpecifiedDisk: SR_DISK_REPLICATION_ELIGIBLE = 10i32;
pub const SrDiskReplicationEligibleYes: SR_DISK_REPLICATION_ELIGIBLE = 1i32;
pub const SrReplicatedDiskTypeDestination: SR_REPLICATED_DISK_TYPE = 3i32;
pub const SrReplicatedDiskTypeLogDestination: SR_REPLICATED_DISK_TYPE = 4i32;
pub const SrReplicatedDiskTypeLogNotInParthership: SR_REPLICATED_DISK_TYPE = 6i32;
pub const SrReplicatedDiskTypeLogSource: SR_REPLICATED_DISK_TYPE = 2i32;
pub const SrReplicatedDiskTypeNone: SR_REPLICATED_DISK_TYPE = 0i32;
pub const SrReplicatedDiskTypeNotInParthership: SR_REPLICATED_DISK_TYPE = 5i32;
pub const SrReplicatedDiskTypeOther: SR_REPLICATED_DISK_TYPE = 7i32;
pub const SrReplicatedDiskTypeSource: SR_REPLICATED_DISK_TYPE = 1i32;
pub const USE_CLIENT_ACCESS_NETWORKS_FOR_CSV: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("UseClientAccessNetworksForSharedVolumes");
pub const VmResdllContextLiveMigration: VM_RESDLL_CONTEXT = 4i32;
pub const VmResdllContextSave: VM_RESDLL_CONTEXT = 1i32;
pub const VmResdllContextShutdown: VM_RESDLL_CONTEXT = 2i32;
pub const VmResdllContextShutdownForce: VM_RESDLL_CONTEXT = 3i32;
pub const VmResdllContextTurnOff: VM_RESDLL_CONTEXT = 0i32;
pub const VolumeBackupInProgress: CLUSTER_SHARED_VOLUME_BACKUP_STATE = 1i32;
pub const VolumeBackupNone: CLUSTER_SHARED_VOLUME_BACKUP_STATE = 0i32;
pub const VolumeRedirectedIOReasonMax: u64 = 9223372036854775808u64;
pub const VolumeRedirectedIOReasonNoDiskConnectivity: u64 = 1u64;
pub const VolumeRedirectedIOReasonStorageSpaceNotAttached: u64 = 2u64;
pub const VolumeRedirectedIOReasonVolumeReplicationEnabled: u64 = 4u64;
pub const VolumeStateDismounted: CLUSTER_CSV_VOLUME_FAULT_STATE = 8i32;
pub const VolumeStateInMaintenance: CLUSTER_CSV_VOLUME_FAULT_STATE = 4i32;
pub const VolumeStateNoAccess: CLUSTER_CSV_VOLUME_FAULT_STATE = 2i32;
pub const VolumeStateNoDirectIO: CLUSTER_CSV_VOLUME_FAULT_STATE = 1i32;
pub const VolumeStateNoFaults: CLUSTER_CSV_VOLUME_FAULT_STATE = 0i32;
pub const WS2016_RTM_UPGRADE_VERSION: u32 = 8u32;
pub const WS2016_TP4_UPGRADE_VERSION: u32 = 6u32;
pub const WS2016_TP5_UPGRADE_VERSION: u32 = 7u32;
pub const eResourceStateChangeReasonFailedMove: CLUSTER_RESOURCE_STATE_CHANGE_REASON = 3i32;
pub const eResourceStateChangeReasonFailover: CLUSTER_RESOURCE_STATE_CHANGE_REASON = 2i32;
pub const eResourceStateChangeReasonMove: CLUSTER_RESOURCE_STATE_CHANGE_REASON = 1i32;
pub const eResourceStateChangeReasonRundown: CLUSTER_RESOURCE_STATE_CHANGE_REASON = 5i32;
pub const eResourceStateChangeReasonShutdown: CLUSTER_RESOURCE_STATE_CHANGE_REASON = 4i32;
pub const eResourceStateChangeReasonUnknown: CLUSTER_RESOURCE_STATE_CHANGE_REASON = 0i32;
pub type CLCTL_CODES = i32;
pub type CLUADMEX_OBJECT_TYPE = i32;
pub type CLUSCTL_AFFINITYRULE_CODES = i32;
pub type CLUSCTL_CLUSTER_CODES = i32;
pub type CLUSCTL_GROUPSET_CODES = i32;
pub type CLUSCTL_GROUP_CODES = i32;
pub type CLUSCTL_NETINTERFACE_CODES = i32;
pub type CLUSCTL_NETWORK_CODES = i32;
pub type CLUSCTL_NODE_CODES = i32;
pub type CLUSCTL_RESOURCE_CODES = i32;
pub type CLUSCTL_RESOURCE_TYPE_CODES = i32;
pub type CLUSGROUP_TYPE = i32;
pub type CLUSPROP_IPADDR_ENABLENETBIOS = i32;
pub type CLUSPROP_PIFLAGS = i32;
pub type CLUSTERSET_OBJECT_TYPE = i32;
pub type CLUSTER_CHANGE = i32;
pub type CLUSTER_CHANGE_CLUSTER_V2 = i32;
pub type CLUSTER_CHANGE_GROUPSET_V2 = i32;
pub type CLUSTER_CHANGE_GROUP_V2 = i32;
pub type CLUSTER_CHANGE_NETINTERFACE_V2 = i32;
pub type CLUSTER_CHANGE_NETWORK_V2 = i32;
pub type CLUSTER_CHANGE_NODE_UPGRADE_PHASE_V2 = i32;
pub type CLUSTER_CHANGE_NODE_V2 = i32;
pub type CLUSTER_CHANGE_QUORUM_V2 = i32;
pub type CLUSTER_CHANGE_REGISTRY_V2 = i32;
pub type CLUSTER_CHANGE_RESOURCE_TYPE_V2 = i32;
pub type CLUSTER_CHANGE_RESOURCE_V2 = i32;
pub type CLUSTER_CHANGE_SHARED_VOLUME_V2 = i32;
pub type CLUSTER_CHANGE_SPACEPORT_V2 = i32;
pub type CLUSTER_CLOUD_TYPE = i32;
pub type CLUSTER_CONTROL_OBJECT = i32;
pub type CLUSTER_CSV_VOLUME_FAULT_STATE = i32;
pub type CLUSTER_ENUM = i32;
pub type CLUSTER_GROUP_AUTOFAILBACK_TYPE = i32;
pub type CLUSTER_GROUP_ENUM = i32;
pub type CLUSTER_GROUP_PRIORITY = i32;
pub type CLUSTER_GROUP_STATE = i32;
pub type CLUSTER_MGMT_POINT_RESTYPE = i32;
pub type CLUSTER_MGMT_POINT_TYPE = i32;
pub type CLUSTER_NETINTERFACE_STATE = i32;
pub type CLUSTER_NETWORK_ENUM = i32;
pub type CLUSTER_NETWORK_ROLE = i32;
pub type CLUSTER_NETWORK_STATE = i32;
pub type CLUSTER_NODE_DRAIN_STATUS = i32;
pub type CLUSTER_NODE_ENUM = i32;
pub type CLUSTER_NODE_RESUME_FAILBACK_TYPE = i32;
pub type CLUSTER_NODE_STATE = i32;
pub type CLUSTER_NODE_STATUS = i32;
pub type CLUSTER_NOTIFICATIONS_VERSION = i32;
pub type CLUSTER_OBJECT_TYPE = i32;
pub type CLUSTER_PROPERTY_FORMAT = i32;
pub type CLUSTER_PROPERTY_SYNTAX = u32;
pub type CLUSTER_PROPERTY_TYPE = i32;
pub type CLUSTER_QUORUM_TYPE = i32;
pub type CLUSTER_QUORUM_VALUE = i32;
pub type CLUSTER_REG_COMMAND = i32;
pub type CLUSTER_RESOURCE_APPLICATION_STATE = i32;
pub type CLUSTER_RESOURCE_CLASS = i32;
pub type CLUSTER_RESOURCE_CREATE_FLAGS = i32;
pub type CLUSTER_RESOURCE_EMBEDDED_FAILURE_ACTION = i32;
pub type CLUSTER_RESOURCE_ENUM = i32;
pub type CLUSTER_RESOURCE_RESTART_ACTION = i32;
pub type CLUSTER_RESOURCE_STATE = i32;
pub type CLUSTER_RESOURCE_STATE_CHANGE_REASON = i32;
pub type CLUSTER_RESOURCE_TYPE_ENUM = i32;
pub type CLUSTER_ROLE = i32;
pub type CLUSTER_ROLE_STATE = i32;
pub type CLUSTER_SETUP_PHASE = i32;
pub type CLUSTER_SETUP_PHASE_SEVERITY = i32;
pub type CLUSTER_SETUP_PHASE_TYPE = i32;
pub type CLUSTER_SHARED_VOLUME_BACKUP_STATE = i32;
pub type CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE = i32;
pub type CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE = i32;
pub type CLUSTER_SHARED_VOLUME_STATE = i32;
pub type CLUSTER_STORAGENODE_STATE = i32;
pub type CLUSTER_UPGRADE_PHASE = i32;
pub type CLUS_AFFINITY_RULE_TYPE = i32;
pub type CLUS_CHARACTERISTICS = i32;
pub type CLUS_FLAGS = i32;
pub type CLUS_GROUP_START_SETTING = i32;
pub type CLUS_RESSUBCLASS = i32;
pub type CLUS_RESSUBCLASS_NETWORK = i32;
pub type CLUS_RESSUBCLASS_STORAGE = i32;
pub type FAILURE_TYPE = i32;
pub type FILESHARE_CHANGE_ENUM = i32;
pub type GRP_PLACEMENT_OPTIONS = i32;
pub type LOG_LEVEL = i32;
pub type MAINTENANCE_MODE_TYPE_ENUM = i32;
pub type NODE_CLUSTER_STATE = i32;
pub type PLACEMENT_OPTIONS = i32;
pub type RESDLL_CONTEXT_OPERATION_TYPE = i32;
pub type RESOURCE_EXIT_STATE = i32;
pub type RESOURCE_MONITOR_STATE = i32;
pub type SR_DISK_REPLICATION_ELIGIBLE = i32;
pub type SR_REPLICATED_DISK_TYPE = i32;
pub type VM_RESDLL_CONTEXT = i32;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLRES_CALLBACK_FUNCTION_TABLE {
    pub LogEvent: PLOG_EVENT_ROUTINE,
    pub SetResourceStatusEx: PSET_RESOURCE_STATUS_ROUTINE_EX,
    pub SetResourceLockedMode: PSET_RESOURCE_LOCKED_MODE_ROUTINE,
    pub SignalFailure: PSIGNAL_FAILURE_ROUTINE,
    pub SetResourceInMemoryNodeLocalProperties: PSET_RESOURCE_INMEMORY_NODELOCAL_PROPERTIES_ROUTINE,
    pub EndControlCall: PEND_CONTROL_CALL,
    pub EndTypeControlCall: PEND_TYPE_CONTROL_CALL,
    pub ExtendControlCall: PEXTEND_RES_CONTROL_CALL,
    pub ExtendTypeControlCall: PEXTEND_RES_TYPE_CONTROL_CALL,
    pub RaiseResTypeNotification: PRAISE_RES_TYPE_NOTIFICATION,
    pub ChangeResourceProcessForDumps: PCHANGE_RESOURCE_PROCESS_FOR_DUMPS,
    pub ChangeResTypeProcessForDumps: PCHANGE_RES_TYPE_PROCESS_FOR_DUMPS,
    pub SetInternalState: PSET_INTERNAL_STATE,
    pub SetResourceLockedModeEx: PSET_RESOURCE_LOCKED_MODE_EX_ROUTINE,
    pub RequestDump: PREQUEST_DUMP_ROUTINE,
    pub SetResourceWprPolicy: PSET_RESOURCE_WPR_POLICY_ROUTINE,
    pub ArmWprWatchdogForCurrentResourceCall: PARM_WPR_WATCHDOG_FOR_CURRENT_RESOURCE_CALL_ROUTINE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLRES_CALLBACK_FUNCTION_TABLE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLRES_CALLBACK_FUNCTION_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub struct CLRES_FUNCTION_TABLE {
    pub TableSize: u32,
    pub Version: u32,
    pub Anonymous: CLRES_FUNCTION_TABLE_0,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for CLRES_FUNCTION_TABLE {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for CLRES_FUNCTION_TABLE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub union CLRES_FUNCTION_TABLE_0 {
    pub V1Functions: CLRES_V1_FUNCTIONS,
    pub V2Functions: CLRES_V2_FUNCTIONS,
    pub V3Functions: CLRES_V3_FUNCTIONS,
    pub V4Functions: CLRES_V4_FUNCTIONS,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for CLRES_FUNCTION_TABLE_0 {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for CLRES_FUNCTION_TABLE_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub struct CLRES_V1_FUNCTIONS {
    pub Open: POPEN_ROUTINE,
    pub Close: PCLOSE_ROUTINE,
    pub Online: PONLINE_ROUTINE,
    pub Offline: POFFLINE_ROUTINE,
    pub Terminate: PTERMINATE_ROUTINE,
    pub LooksAlive: PLOOKS_ALIVE_ROUTINE,
    pub IsAlive: PIS_ALIVE_ROUTINE,
    pub Arbitrate: PARBITRATE_ROUTINE,
    pub Release: PRELEASE_ROUTINE,
    pub ResourceControl: PRESOURCE_CONTROL_ROUTINE,
    pub ResourceTypeControl: PRESOURCE_TYPE_CONTROL_ROUTINE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for CLRES_V1_FUNCTIONS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for CLRES_V1_FUNCTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub struct CLRES_V2_FUNCTIONS {
    pub Open: POPEN_V2_ROUTINE,
    pub Close: PCLOSE_ROUTINE,
    pub Online: PONLINE_V2_ROUTINE,
    pub Offline: POFFLINE_V2_ROUTINE,
    pub Terminate: PTERMINATE_ROUTINE,
    pub LooksAlive: PLOOKS_ALIVE_ROUTINE,
    pub IsAlive: PIS_ALIVE_ROUTINE,
    pub Arbitrate: PARBITRATE_ROUTINE,
    pub Release: PRELEASE_ROUTINE,
    pub ResourceControl: PRESOURCE_CONTROL_ROUTINE,
    pub ResourceTypeControl: PRESOURCE_TYPE_CONTROL_ROUTINE,
    pub Cancel: PCANCEL_ROUTINE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for CLRES_V2_FUNCTIONS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for CLRES_V2_FUNCTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub struct CLRES_V3_FUNCTIONS {
    pub Open: POPEN_V2_ROUTINE,
    pub Close: PCLOSE_ROUTINE,
    pub Online: PONLINE_V2_ROUTINE,
    pub Offline: POFFLINE_V2_ROUTINE,
    pub Terminate: PTERMINATE_ROUTINE,
    pub LooksAlive: PLOOKS_ALIVE_ROUTINE,
    pub IsAlive: PIS_ALIVE_ROUTINE,
    pub Arbitrate: PARBITRATE_ROUTINE,
    pub Release: PRELEASE_ROUTINE,
    pub BeginResourceControl: PBEGIN_RESCALL_ROUTINE,
    pub BeginResourceTypeControl: PBEGIN_RESTYPECALL_ROUTINE,
    pub Cancel: PCANCEL_ROUTINE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for CLRES_V3_FUNCTIONS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for CLRES_V3_FUNCTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub struct CLRES_V4_FUNCTIONS {
    pub Open: POPEN_V2_ROUTINE,
    pub Close: PCLOSE_ROUTINE,
    pub Online: PONLINE_V2_ROUTINE,
    pub Offline: POFFLINE_V2_ROUTINE,
    pub Terminate: PTERMINATE_ROUTINE,
    pub LooksAlive: PLOOKS_ALIVE_ROUTINE,
    pub IsAlive: PIS_ALIVE_ROUTINE,
    pub Arbitrate: PARBITRATE_ROUTINE,
    pub Release: PRELEASE_ROUTINE,
    pub BeginResourceControl: PBEGIN_RESCALL_ROUTINE,
    pub BeginResourceTypeControl: PBEGIN_RESTYPECALL_ROUTINE,
    pub Cancel: PCANCEL_ROUTINE,
    pub BeginResourceControlAsUser: PBEGIN_RESCALL_AS_USER_ROUTINE,
    pub BeginResourceTypeControlAsUser: PBEGIN_RESTYPECALL_AS_USER_ROUTINE,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::marker::Copy for CLRES_V4_FUNCTIONS {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
impl ::core::clone::Clone for CLRES_V4_FUNCTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUSAPI_REASON_HANDLER {
    pub lpParameter: *mut ::core::ffi::c_void,
    pub pfnHandler: PCLUSAPI_PFN_REASON_HANDLER,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUSAPI_REASON_HANDLER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUSAPI_REASON_HANDLER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUSCTL_GROUP_GET_LAST_MOVE_TIME_OUTPUT {
    pub GetTickCount64: u64,
    pub GetSystemTime: super::super::Foundation::SYSTEMTIME,
    pub NodeId: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUSCTL_GROUP_GET_LAST_MOVE_TIME_OUTPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUSCTL_GROUP_GET_LAST_MOVE_TIME_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSCTL_RESOURCE_STATE_CHANGE_REASON_STRUCT {
    pub dwSize: u32,
    pub dwVersion: u32,
    pub eReason: CLUSTER_RESOURCE_STATE_CHANGE_REASON,
}
impl ::core::marker::Copy for CLUSCTL_RESOURCE_STATE_CHANGE_REASON_STRUCT {}
impl ::core::clone::Clone for CLUSCTL_RESOURCE_STATE_CHANGE_REASON_STRUCT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_INPUT {
    pub dwFlags: u32,
    pub guidPoolFilter: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_INPUT {}
impl ::core::clone::Clone for CLUSCTL_RESOURCE_TYPE_STORAGE_GET_AVAILABLE_DISKS_EX2_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_BINARY {
    pub Base: CLUSPROP_VALUE,
    pub rgb: [u8; 1],
}
impl ::core::marker::Copy for CLUSPROP_BINARY {}
impl ::core::clone::Clone for CLUSPROP_BINARY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
pub union CLUSPROP_BUFFER_HELPER {
    pub pb: *mut u8,
    pub pw: *mut u16,
    pub pdw: *mut u32,
    pub pl: *mut i32,
    pub psz: ::windows_sys::core::PWSTR,
    pub pList: *mut CLUSPROP_LIST,
    pub pSyntax: *mut CLUSPROP_SYNTAX,
    pub pName: *mut CLUSPROP_SZ,
    pub pValue: *mut CLUSPROP_VALUE,
    pub pBinaryValue: *mut CLUSPROP_BINARY,
    pub pWordValue: *mut CLUSPROP_WORD,
    pub pDwordValue: *mut CLUSPROP_DWORD,
    pub pLongValue: *mut CLUSPROP_LONG,
    pub pULargeIntegerValue: *mut CLUSPROP_ULARGE_INTEGER,
    pub pLargeIntegerValue: *mut CLUSPROP_LARGE_INTEGER,
    pub pStringValue: *mut CLUSPROP_SZ,
    pub pMultiSzValue: *mut CLUSPROP_SZ,
    pub pSecurityDescriptor: *mut CLUSPROP_SECURITY_DESCRIPTOR,
    pub pResourceClassValue: *mut CLUSPROP_RESOURCE_CLASS,
    pub pResourceClassInfoValue: *mut CLUSPROP_RESOURCE_CLASS_INFO,
    pub pDiskSignatureValue: *mut CLUSPROP_DWORD,
    pub pScsiAddressValue: *mut CLUSPROP_SCSI_ADDRESS,
    pub pDiskNumberValue: *mut CLUSPROP_DWORD,
    pub pPartitionInfoValue: *mut CLUSPROP_PARTITION_INFO,
    pub pRequiredDependencyValue: *mut CLUSPROP_REQUIRED_DEPENDENCY,
    pub pPartitionInfoValueEx: *mut CLUSPROP_PARTITION_INFO_EX,
    pub pPartitionInfoValueEx2: *mut CLUSPROP_PARTITION_INFO_EX2,
    pub pFileTimeValue: *mut CLUSPROP_FILETIME,
}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::marker::Copy for CLUSPROP_BUFFER_HELPER {}
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security"))]
impl ::core::clone::Clone for CLUSPROP_BUFFER_HELPER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_DWORD {
    pub Base: CLUSPROP_VALUE,
    pub dw: u32,
}
impl ::core::marker::Copy for CLUSPROP_DWORD {}
impl ::core::clone::Clone for CLUSPROP_DWORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUSPROP_FILETIME {
    pub Base: CLUSPROP_VALUE,
    pub ft: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUSPROP_FILETIME {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUSPROP_FILETIME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_FTSET_INFO {
    pub Base: CLUSPROP_VALUE,
    pub Base2: CLUS_FTSET_INFO,
}
impl ::core::marker::Copy for CLUSPROP_FTSET_INFO {}
impl ::core::clone::Clone for CLUSPROP_FTSET_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_LARGE_INTEGER {
    pub Base: CLUSPROP_VALUE,
    pub li: i64,
}
impl ::core::marker::Copy for CLUSPROP_LARGE_INTEGER {}
impl ::core::clone::Clone for CLUSPROP_LARGE_INTEGER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_LIST {
    pub nPropertyCount: u32,
    pub PropertyName: CLUSPROP_SZ,
}
impl ::core::marker::Copy for CLUSPROP_LIST {}
impl ::core::clone::Clone for CLUSPROP_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_LONG {
    pub Base: CLUSPROP_VALUE,
    pub l: i32,
}
impl ::core::marker::Copy for CLUSPROP_LONG {}
impl ::core::clone::Clone for CLUSPROP_LONG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_PARTITION_INFO {
    pub Base: CLUSPROP_VALUE,
    pub Base2: CLUS_PARTITION_INFO,
}
impl ::core::marker::Copy for CLUSPROP_PARTITION_INFO {}
impl ::core::clone::Clone for CLUSPROP_PARTITION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_PARTITION_INFO_EX {
    pub Base: CLUSPROP_VALUE,
    pub Base2: CLUS_PARTITION_INFO_EX,
}
impl ::core::marker::Copy for CLUSPROP_PARTITION_INFO_EX {}
impl ::core::clone::Clone for CLUSPROP_PARTITION_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_PARTITION_INFO_EX2 {
    pub Base: CLUSPROP_PARTITION_INFO_EX,
    pub Base2: CLUS_PARTITION_INFO_EX2,
}
impl ::core::marker::Copy for CLUSPROP_PARTITION_INFO_EX2 {}
impl ::core::clone::Clone for CLUSPROP_PARTITION_INFO_EX2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLUSPROP_REQUIRED_DEPENDENCY {
    pub Value: CLUSPROP_VALUE,
    pub ResClass: CLUSPROP_RESOURCE_CLASS,
    pub ResTypeName: CLUSPROP_SZ,
}
impl ::core::marker::Copy for CLUSPROP_REQUIRED_DEPENDENCY {}
impl ::core::clone::Clone for CLUSPROP_REQUIRED_DEPENDENCY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_RESOURCE_CLASS {
    pub Base: CLUSPROP_VALUE,
    pub rc: CLUSTER_RESOURCE_CLASS,
}
impl ::core::marker::Copy for CLUSPROP_RESOURCE_CLASS {}
impl ::core::clone::Clone for CLUSPROP_RESOURCE_CLASS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_RESOURCE_CLASS_INFO {
    pub Base: CLUSPROP_VALUE,
    pub Base2: CLUS_RESOURCE_CLASS_INFO,
}
impl ::core::marker::Copy for CLUSPROP_RESOURCE_CLASS_INFO {}
impl ::core::clone::Clone for CLUSPROP_RESOURCE_CLASS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_SCSI_ADDRESS {
    pub Base: CLUSPROP_VALUE,
    pub Base2: CLUS_SCSI_ADDRESS,
}
impl ::core::marker::Copy for CLUSPROP_SCSI_ADDRESS {}
impl ::core::clone::Clone for CLUSPROP_SCSI_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub struct CLUSPROP_SECURITY_DESCRIPTOR {
    pub Base: CLUSPROP_VALUE,
    pub Anonymous: CLUSPROP_SECURITY_DESCRIPTOR_0,
}
#[cfg(feature = "Win32_Security")]
impl ::core::marker::Copy for CLUSPROP_SECURITY_DESCRIPTOR {}
#[cfg(feature = "Win32_Security")]
impl ::core::clone::Clone for CLUSPROP_SECURITY_DESCRIPTOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub union CLUSPROP_SECURITY_DESCRIPTOR_0 {
    pub sd: super::super::Security::SECURITY_DESCRIPTOR_RELATIVE,
    pub rgbSecurityDescriptor: [u8; 1],
}
#[cfg(feature = "Win32_Security")]
impl ::core::marker::Copy for CLUSPROP_SECURITY_DESCRIPTOR_0 {}
#[cfg(feature = "Win32_Security")]
impl ::core::clone::Clone for CLUSPROP_SECURITY_DESCRIPTOR_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLUSPROP_SYNTAX {
    pub dw: u32,
    pub Anonymous: CLUSPROP_SYNTAX_0,
}
impl ::core::marker::Copy for CLUSPROP_SYNTAX {}
impl ::core::clone::Clone for CLUSPROP_SYNTAX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_SYNTAX_0 {
    pub wFormat: u16,
    pub wType: u16,
}
impl ::core::marker::Copy for CLUSPROP_SYNTAX_0 {}
impl ::core::clone::Clone for CLUSPROP_SYNTAX_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_SZ {
    pub Base: CLUSPROP_VALUE,
    pub sz: [u16; 1],
}
impl ::core::marker::Copy for CLUSPROP_SZ {}
impl ::core::clone::Clone for CLUSPROP_SZ {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_ULARGE_INTEGER {
    pub Base: CLUSPROP_VALUE,
    pub li: u64,
}
impl ::core::marker::Copy for CLUSPROP_ULARGE_INTEGER {}
impl ::core::clone::Clone for CLUSPROP_ULARGE_INTEGER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_VALUE {
    pub Syntax: CLUSPROP_SYNTAX,
    pub cbLength: u32,
}
impl ::core::marker::Copy for CLUSPROP_VALUE {}
impl ::core::clone::Clone for CLUSPROP_VALUE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSPROP_WORD {
    pub Base: CLUSPROP_VALUE,
    pub w: u16,
}
impl ::core::marker::Copy for CLUSPROP_WORD {}
impl ::core::clone::Clone for CLUSPROP_WORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTERVERSIONINFO {
    pub dwVersionInfoSize: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub BuildNumber: u16,
    pub szVendorId: [u16; 64],
    pub szCSDVersion: [u16; 64],
    pub dwClusterHighestVersion: u32,
    pub dwClusterLowestVersion: u32,
    pub dwFlags: u32,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for CLUSTERVERSIONINFO {}
impl ::core::clone::Clone for CLUSTERVERSIONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTERVERSIONINFO_NT4 {
    pub dwVersionInfoSize: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub BuildNumber: u16,
    pub szVendorId: [u16; 64],
    pub szCSDVersion: [u16; 64],
}
impl ::core::marker::Copy for CLUSTERVERSIONINFO_NT4 {}
impl ::core::clone::Clone for CLUSTERVERSIONINFO_NT4 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUSTER_AVAILABILITY_SET_CONFIG {
    pub dwVersion: u32,
    pub dwUpdateDomains: u32,
    pub dwFaultDomains: u32,
    pub bReserveSpareNode: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUSTER_AVAILABILITY_SET_CONFIG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUSTER_AVAILABILITY_SET_CONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_BATCH_COMMAND {
    pub Command: CLUSTER_REG_COMMAND,
    pub dwOptions: u32,
    pub wzName: ::windows_sys::core::PCWSTR,
    pub lpData: *const u8,
    pub cbData: u32,
}
impl ::core::marker::Copy for CLUSTER_BATCH_COMMAND {}
impl ::core::clone::Clone for CLUSTER_BATCH_COMMAND {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_CREATE_GROUP_INFO {
    pub dwVersion: u32,
    pub groupType: CLUSGROUP_TYPE,
}
impl ::core::marker::Copy for CLUSTER_CREATE_GROUP_INFO {}
impl ::core::clone::Clone for CLUSTER_CREATE_GROUP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_ENUM_ITEM {
    pub dwVersion: u32,
    pub dwType: u32,
    pub cbId: u32,
    pub lpszId: ::windows_sys::core::PWSTR,
    pub cbName: u32,
    pub lpszName: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for CLUSTER_ENUM_ITEM {}
impl ::core::clone::Clone for CLUSTER_ENUM_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_GROUP_ENUM_ITEM {
    pub dwVersion: u32,
    pub cbId: u32,
    pub lpszId: ::windows_sys::core::PWSTR,
    pub cbName: u32,
    pub lpszName: ::windows_sys::core::PWSTR,
    pub state: CLUSTER_GROUP_STATE,
    pub cbOwnerNode: u32,
    pub lpszOwnerNode: ::windows_sys::core::PWSTR,
    pub dwFlags: u32,
    pub cbProperties: u32,
    pub pProperties: *mut ::core::ffi::c_void,
    pub cbRoProperties: u32,
    pub pRoProperties: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for CLUSTER_GROUP_ENUM_ITEM {}
impl ::core::clone::Clone for CLUSTER_GROUP_ENUM_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_HEALTH_FAULT {
    pub Id: ::windows_sys::core::PWSTR,
    pub ErrorType: u32,
    pub ErrorCode: u32,
    pub Description: ::windows_sys::core::PWSTR,
    pub Provider: ::windows_sys::core::PWSTR,
    pub Flags: u32,
    pub Reserved: u32,
}
impl ::core::marker::Copy for CLUSTER_HEALTH_FAULT {}
impl ::core::clone::Clone for CLUSTER_HEALTH_FAULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_HEALTH_FAULT_ARRAY {
    pub numFaults: u32,
    pub faults: *mut CLUSTER_HEALTH_FAULT,
}
impl ::core::marker::Copy for CLUSTER_HEALTH_FAULT_ARRAY {}
impl ::core::clone::Clone for CLUSTER_HEALTH_FAULT_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_IP_ENTRY {
    pub lpszIpAddress: ::windows_sys::core::PCWSTR,
    pub dwPrefixLength: u32,
}
impl ::core::marker::Copy for CLUSTER_IP_ENTRY {}
impl ::core::clone::Clone for CLUSTER_IP_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUSTER_MEMBERSHIP_INFO {
    pub HasQuorum: super::super::Foundation::BOOL,
    pub UpnodesSize: u32,
    pub Upnodes: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUSTER_MEMBERSHIP_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUSTER_MEMBERSHIP_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_READ_BATCH_COMMAND {
    pub Command: CLUSTER_REG_COMMAND,
    pub dwOptions: u32,
    pub wzSubkeyName: ::windows_sys::core::PCWSTR,
    pub wzValueName: ::windows_sys::core::PCWSTR,
    pub lpData: *const u8,
    pub cbData: u32,
}
impl ::core::marker::Copy for CLUSTER_READ_BATCH_COMMAND {}
impl ::core::clone::Clone for CLUSTER_READ_BATCH_COMMAND {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_RESOURCE_ENUM_ITEM {
    pub dwVersion: u32,
    pub cbId: u32,
    pub lpszId: ::windows_sys::core::PWSTR,
    pub cbName: u32,
    pub lpszName: ::windows_sys::core::PWSTR,
    pub cbOwnerGroupName: u32,
    pub lpszOwnerGroupName: ::windows_sys::core::PWSTR,
    pub cbOwnerGroupId: u32,
    pub lpszOwnerGroupId: ::windows_sys::core::PWSTR,
    pub cbProperties: u32,
    pub pProperties: *mut ::core::ffi::c_void,
    pub cbRoProperties: u32,
    pub pRoProperties: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for CLUSTER_RESOURCE_ENUM_ITEM {}
impl ::core::clone::Clone for CLUSTER_RESOURCE_ENUM_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUSTER_SET_PASSWORD_STATUS {
    pub NodeId: u32,
    pub SetAttempted: super::super::Foundation::BOOLEAN,
    pub ReturnStatus: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUSTER_SET_PASSWORD_STATUS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUSTER_SET_PASSWORD_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_RENAME_GUID_INPUT {
    pub Base: CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME,
    pub Base2: CLUSTER_SHARED_VOLUME_RENAME_INPUT_GUID_NAME,
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_RENAME_GUID_INPUT {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_RENAME_GUID_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_RENAME_INPUT {
    pub Base: CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME,
    pub Base2: CLUSTER_SHARED_VOLUME_RENAME_INPUT_NAME,
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_RENAME_INPUT {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_RENAME_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_RENAME_INPUT_GUID_NAME {
    pub NewVolumeName: [u16; 260],
    pub NewVolumeGuid: [u16; 50],
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_RENAME_INPUT_GUID_NAME {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_RENAME_INPUT_GUID_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_RENAME_INPUT_NAME {
    pub NewVolumeName: [u16; 260],
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_RENAME_INPUT_NAME {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_RENAME_INPUT_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME {
    pub InputType: CLUSTER_SHARED_VOLUME_RENAME_INPUT_TYPE,
    pub Anonymous: CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME_0,
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME_0 {
    pub VolumeOffset: u64,
    pub VolumeId: [u16; 260],
    pub VolumeName: [u16; 260],
    pub VolumeGuid: [u16; 50],
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME_0 {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_RENAME_INPUT_VOLUME_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_STATE_INFO {
    pub szVolumeName: [u16; 260],
    pub szNodeName: [u16; 260],
    pub VolumeState: CLUSTER_SHARED_VOLUME_STATE,
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_STATE_INFO {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_STATE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_SHARED_VOLUME_STATE_INFO_EX {
    pub szVolumeName: [u16; 260],
    pub szNodeName: [u16; 260],
    pub VolumeState: CLUSTER_SHARED_VOLUME_STATE,
    pub szVolumeFriendlyName: [u16; 260],
    pub RedirectedIOReason: u64,
    pub VolumeRedirectedIOReason: u64,
}
impl ::core::marker::Copy for CLUSTER_SHARED_VOLUME_STATE_INFO_EX {}
impl ::core::clone::Clone for CLUSTER_SHARED_VOLUME_STATE_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_VALIDATE_CSV_FILENAME {
    pub szFileName: [u16; 1],
}
impl ::core::marker::Copy for CLUSTER_VALIDATE_CSV_FILENAME {}
impl ::core::clone::Clone for CLUSTER_VALIDATE_CSV_FILENAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_VALIDATE_DIRECTORY {
    pub szPath: [u16; 1],
}
impl ::core::marker::Copy for CLUSTER_VALIDATE_DIRECTORY {}
impl ::core::clone::Clone for CLUSTER_VALIDATE_DIRECTORY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_VALIDATE_NETNAME {
    pub szNetworkName: [u16; 1],
}
impl ::core::marker::Copy for CLUSTER_VALIDATE_NETNAME {}
impl ::core::clone::Clone for CLUSTER_VALIDATE_NETNAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUSTER_VALIDATE_PATH {
    pub szPath: [u16; 1],
}
impl ::core::marker::Copy for CLUSTER_VALIDATE_PATH {}
impl ::core::clone::Clone for CLUSTER_VALIDATE_PATH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_CHKDSK_INFO {
    pub PartitionNumber: u32,
    pub ChkdskState: u32,
    pub FileIdCount: u32,
    pub FileIdList: [u64; 1],
}
impl ::core::marker::Copy for CLUS_CHKDSK_INFO {}
impl ::core::clone::Clone for CLUS_CHKDSK_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_CREATE_INFRASTRUCTURE_FILESERVER_INPUT {
    pub FileServerName: [u16; 16],
}
impl ::core::marker::Copy for CLUS_CREATE_INFRASTRUCTURE_FILESERVER_INPUT {}
impl ::core::clone::Clone for CLUS_CREATE_INFRASTRUCTURE_FILESERVER_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_CREATE_INFRASTRUCTURE_FILESERVER_OUTPUT {
    pub FileServerName: [u16; 260],
}
impl ::core::marker::Copy for CLUS_CREATE_INFRASTRUCTURE_FILESERVER_OUTPUT {}
impl ::core::clone::Clone for CLUS_CREATE_INFRASTRUCTURE_FILESERVER_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_CSV_MAINTENANCE_MODE_INFO {
    pub InMaintenance: super::super::Foundation::BOOL,
    pub VolumeName: [u16; 260],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_CSV_MAINTENANCE_MODE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_CSV_MAINTENANCE_MODE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_CSV_VOLUME_INFO {
    pub VolumeOffset: u64,
    pub PartitionNumber: u32,
    pub FaultState: CLUSTER_CSV_VOLUME_FAULT_STATE,
    pub BackupState: CLUSTER_SHARED_VOLUME_BACKUP_STATE,
    pub szVolumeFriendlyName: [u16; 260],
    pub szVolumeName: [u16; 50],
}
impl ::core::marker::Copy for CLUS_CSV_VOLUME_INFO {}
impl ::core::clone::Clone for CLUS_CSV_VOLUME_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_CSV_VOLUME_NAME {
    pub VolumeOffset: i64,
    pub szVolumeName: [u16; 260],
    pub szRootPath: [u16; 263],
}
impl ::core::marker::Copy for CLUS_CSV_VOLUME_NAME {}
impl ::core::clone::Clone for CLUS_CSV_VOLUME_NAME {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_DISK_NUMBER_INFO {
    pub DiskNumber: u32,
    pub BytesPerSector: u32,
}
impl ::core::marker::Copy for CLUS_DISK_NUMBER_INFO {}
impl ::core::clone::Clone for CLUS_DISK_NUMBER_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_DNN_LEADER_STATUS {
    pub IsOnline: super::super::Foundation::BOOL,
    pub IsFileServerPresent: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_DNN_LEADER_STATUS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_DNN_LEADER_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_DNN_SODAFS_CLONE_STATUS {
    pub NodeId: u32,
    pub Status: CLUSTER_RESOURCE_STATE,
}
impl ::core::marker::Copy for CLUS_DNN_SODAFS_CLONE_STATUS {}
impl ::core::clone::Clone for CLUS_DNN_SODAFS_CLONE_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_FORCE_QUORUM_INFO {
    pub dwSize: u32,
    pub dwNodeBitMask: u32,
    pub dwMaxNumberofNodes: u32,
    pub multiszNodeList: [u16; 1],
}
impl ::core::marker::Copy for CLUS_FORCE_QUORUM_INFO {}
impl ::core::clone::Clone for CLUS_FORCE_QUORUM_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_FTSET_INFO {
    pub dwRootSignature: u32,
    pub dwFtType: u32,
}
impl ::core::marker::Copy for CLUS_FTSET_INFO {}
impl ::core::clone::Clone for CLUS_FTSET_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_MAINTENANCE_MODE_INFO {
    pub InMaintenance: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_MAINTENANCE_MODE_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_MAINTENANCE_MODE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_MAINTENANCE_MODE_INFOEX {
    pub InMaintenance: super::super::Foundation::BOOL,
    pub MaintainenceModeType: MAINTENANCE_MODE_TYPE_ENUM,
    pub InternalState: CLUSTER_RESOURCE_STATE,
    pub Signature: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_MAINTENANCE_MODE_INFOEX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_MAINTENANCE_MODE_INFOEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_NETNAME_IP_INFO_ENTRY {
    pub NodeId: u32,
    pub AddressSize: u32,
    pub Address: [u8; 1],
}
impl ::core::marker::Copy for CLUS_NETNAME_IP_INFO_ENTRY {}
impl ::core::clone::Clone for CLUS_NETNAME_IP_INFO_ENTRY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_NETNAME_IP_INFO_FOR_MULTICHANNEL {
    pub szName: [u16; 64],
    pub NumEntries: u32,
    pub IpInfo: [CLUS_NETNAME_IP_INFO_ENTRY; 1],
}
impl ::core::marker::Copy for CLUS_NETNAME_IP_INFO_FOR_MULTICHANNEL {}
impl ::core::clone::Clone for CLUS_NETNAME_IP_INFO_FOR_MULTICHANNEL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_NETNAME_PWD_INFO {
    pub Flags: u32,
    pub Password: [u16; 16],
    pub CreatingDC: [u16; 258],
    pub ObjectGuid: [u16; 64],
}
impl ::core::marker::Copy for CLUS_NETNAME_PWD_INFO {}
impl ::core::clone::Clone for CLUS_NETNAME_PWD_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_NETNAME_PWD_INFOEX {
    pub Flags: u32,
    pub Password: [u16; 128],
    pub CreatingDC: [u16; 258],
    pub ObjectGuid: [u16; 64],
}
impl ::core::marker::Copy for CLUS_NETNAME_PWD_INFOEX {}
impl ::core::clone::Clone for CLUS_NETNAME_PWD_INFOEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_NETNAME_VS_TOKEN_INFO {
    pub ProcessID: u32,
    pub DesiredAccess: u32,
    pub InheritHandle: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_NETNAME_VS_TOKEN_INFO {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_NETNAME_VS_TOKEN_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_PARTITION_INFO {
    pub dwFlags: u32,
    pub szDeviceName: [u16; 260],
    pub szVolumeLabel: [u16; 260],
    pub dwSerialNumber: u32,
    pub rgdwMaximumComponentLength: u32,
    pub dwFileSystemFlags: u32,
    pub szFileSystem: [u16; 32],
}
impl ::core::marker::Copy for CLUS_PARTITION_INFO {}
impl ::core::clone::Clone for CLUS_PARTITION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_PARTITION_INFO_EX {
    pub dwFlags: u32,
    pub szDeviceName: [u16; 260],
    pub szVolumeLabel: [u16; 260],
    pub dwSerialNumber: u32,
    pub rgdwMaximumComponentLength: u32,
    pub dwFileSystemFlags: u32,
    pub szFileSystem: [u16; 32],
    pub TotalSizeInBytes: u64,
    pub FreeSizeInBytes: u64,
    pub DeviceNumber: u32,
    pub PartitionNumber: u32,
    pub VolumeGuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for CLUS_PARTITION_INFO_EX {}
impl ::core::clone::Clone for CLUS_PARTITION_INFO_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_PARTITION_INFO_EX2 {
    pub GptPartitionId: ::windows_sys::core::GUID,
    pub szPartitionName: [u16; 260],
    pub EncryptionFlags: u32,
}
impl ::core::marker::Copy for CLUS_PARTITION_INFO_EX2 {}
impl ::core::clone::Clone for CLUS_PARTITION_INFO_EX2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_PROVIDER_STATE_CHANGE_INFO {
    pub dwSize: u32,
    pub resourceState: CLUSTER_RESOURCE_STATE,
    pub szProviderId: [u16; 1],
}
impl ::core::marker::Copy for CLUS_PROVIDER_STATE_CHANGE_INFO {}
impl ::core::clone::Clone for CLUS_PROVIDER_STATE_CHANGE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_RESOURCE_CLASS_INFO {
    pub Anonymous: CLUS_RESOURCE_CLASS_INFO_0,
}
impl ::core::marker::Copy for CLUS_RESOURCE_CLASS_INFO {}
impl ::core::clone::Clone for CLUS_RESOURCE_CLASS_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLUS_RESOURCE_CLASS_INFO_0 {
    pub Anonymous: CLUS_RESOURCE_CLASS_INFO_0_0,
    pub li: u64,
}
impl ::core::marker::Copy for CLUS_RESOURCE_CLASS_INFO_0 {}
impl ::core::clone::Clone for CLUS_RESOURCE_CLASS_INFO_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_RESOURCE_CLASS_INFO_0_0 {
    pub Anonymous: CLUS_RESOURCE_CLASS_INFO_0_0_0,
    pub SubClass: u32,
}
impl ::core::marker::Copy for CLUS_RESOURCE_CLASS_INFO_0_0 {}
impl ::core::clone::Clone for CLUS_RESOURCE_CLASS_INFO_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLUS_RESOURCE_CLASS_INFO_0_0_0 {
    pub dw: u32,
    pub rc: CLUSTER_RESOURCE_CLASS,
}
impl ::core::marker::Copy for CLUS_RESOURCE_CLASS_INFO_0_0_0 {}
impl ::core::clone::Clone for CLUS_RESOURCE_CLASS_INFO_0_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_SCSI_ADDRESS {
    pub Anonymous: CLUS_SCSI_ADDRESS_0,
}
impl ::core::marker::Copy for CLUS_SCSI_ADDRESS {}
impl ::core::clone::Clone for CLUS_SCSI_ADDRESS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union CLUS_SCSI_ADDRESS_0 {
    pub Anonymous: CLUS_SCSI_ADDRESS_0_0,
    pub dw: u32,
}
impl ::core::marker::Copy for CLUS_SCSI_ADDRESS_0 {}
impl ::core::clone::Clone for CLUS_SCSI_ADDRESS_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_SCSI_ADDRESS_0_0 {
    pub PortNumber: u8,
    pub PathId: u8,
    pub TargetId: u8,
    pub Lun: u8,
}
impl ::core::marker::Copy for CLUS_SCSI_ADDRESS_0_0 {}
impl ::core::clone::Clone for CLUS_SCSI_ADDRESS_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_SET_MAINTENANCE_MODE_INPUT {
    pub InMaintenance: super::super::Foundation::BOOL,
    pub ExtraParameterSize: u32,
    pub ExtraParameter: [u8; 1],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_SET_MAINTENANCE_MODE_INPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_SET_MAINTENANCE_MODE_INPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_SHARED_VOLUME_BACKUP_MODE {
    pub BackupState: CLUSTER_SHARED_VOLUME_BACKUP_STATE,
    pub DelayTimerInSecs: u32,
    pub VolumeName: [u16; 260],
}
impl ::core::marker::Copy for CLUS_SHARED_VOLUME_BACKUP_MODE {}
impl ::core::clone::Clone for CLUS_SHARED_VOLUME_BACKUP_MODE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_STARTING_PARAMS {
    pub dwSize: u32,
    pub bForm: super::super::Foundation::BOOL,
    pub bFirst: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_STARTING_PARAMS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_STARTING_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_STORAGE_GET_AVAILABLE_DRIVELETTERS {
    pub AvailDrivelettersMask: u32,
}
impl ::core::marker::Copy for CLUS_STORAGE_GET_AVAILABLE_DRIVELETTERS {}
impl ::core::clone::Clone for CLUS_STORAGE_GET_AVAILABLE_DRIVELETTERS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_STORAGE_REMAP_DRIVELETTER {
    pub CurrentDriveLetterMask: u32,
    pub TargetDriveLetterMask: u32,
}
impl ::core::marker::Copy for CLUS_STORAGE_REMAP_DRIVELETTER {}
impl ::core::clone::Clone for CLUS_STORAGE_REMAP_DRIVELETTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CLUS_STORAGE_SET_DRIVELETTER {
    pub PartitionNumber: u32,
    pub DriveLetterMask: u32,
}
impl ::core::marker::Copy for CLUS_STORAGE_SET_DRIVELETTER {}
impl ::core::clone::Clone for CLUS_STORAGE_SET_DRIVELETTER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CLUS_WORKER {
    pub hThread: super::super::Foundation::HANDLE,
    pub Terminate: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CLUS_WORKER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CLUS_WORKER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CREATE_CLUSTER_CONFIG {
    pub dwVersion: u32,
    pub lpszClusterName: ::windows_sys::core::PCWSTR,
    pub cNodes: u32,
    pub ppszNodeNames: *const ::windows_sys::core::PCWSTR,
    pub cIpEntries: u32,
    pub pIpEntries: *mut CLUSTER_IP_ENTRY,
    pub fEmptyCluster: super::super::Foundation::BOOLEAN,
    pub managementPointType: CLUSTER_MGMT_POINT_TYPE,
    pub managementPointResType: CLUSTER_MGMT_POINT_RESTYPE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CREATE_CLUSTER_CONFIG {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CREATE_CLUSTER_CONFIG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct CREATE_CLUSTER_NAME_ACCOUNT {
    pub dwVersion: u32,
    pub lpszClusterName: ::windows_sys::core::PCWSTR,
    pub dwFlags: u32,
    pub pszUserName: ::windows_sys::core::PCWSTR,
    pub pszPassword: ::windows_sys::core::PCWSTR,
    pub pszDomain: ::windows_sys::core::PCWSTR,
    pub managementPointType: CLUSTER_MGMT_POINT_TYPE,
    pub managementPointResType: CLUSTER_MGMT_POINT_RESTYPE,
    pub bUpgradeVCOs: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for CREATE_CLUSTER_NAME_ACCOUNT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for CREATE_CLUSTER_NAME_ACCOUNT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILESHARE_CHANGE {
    pub Change: FILESHARE_CHANGE_ENUM,
    pub ShareName: [u16; 84],
}
impl ::core::marker::Copy for FILESHARE_CHANGE {}
impl ::core::clone::Clone for FILESHARE_CHANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILESHARE_CHANGE_LIST {
    pub NumEntries: u32,
    pub ChangeEntry: [FILESHARE_CHANGE; 1],
}
impl ::core::marker::Copy for FILESHARE_CHANGE_LIST {}
impl ::core::clone::Clone for FILESHARE_CHANGE_LIST {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GET_OPERATION_CONTEXT_PARAMS {
    pub Size: u32,
    pub Version: u32,
    pub Type: RESDLL_CONTEXT_OPERATION_TYPE,
    pub Priority: u32,
}
impl ::core::marker::Copy for GET_OPERATION_CONTEXT_PARAMS {}
impl ::core::clone::Clone for GET_OPERATION_CONTEXT_PARAMS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GROUP_FAILURE_INFO {
    pub dwFailoverAttemptsRemaining: u32,
    pub dwFailoverPeriodRemaining: u32,
}
impl ::core::marker::Copy for GROUP_FAILURE_INFO {}
impl ::core::clone::Clone for GROUP_FAILURE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GROUP_FAILURE_INFO_BUFFER {
    pub dwVersion: u32,
    pub Info: GROUP_FAILURE_INFO,
}
impl ::core::marker::Copy for GROUP_FAILURE_INFO_BUFFER {}
impl ::core::clone::Clone for GROUP_FAILURE_INFO_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HCHANGE = isize;
pub type HCLUSCRYPTPROVIDER = isize;
pub type HCLUSENUM = isize;
pub type HCLUSENUMEX = isize;
pub type HCLUSTER = isize;
pub type HGROUP = isize;
pub type HGROUPENUM = isize;
pub type HGROUPENUMEX = isize;
pub type HGROUPSET = isize;
pub type HGROUPSETENUM = isize;
pub type HNETINTERFACE = isize;
pub type HNETINTERFACEENUM = isize;
pub type HNETWORK = isize;
pub type HNETWORKENUM = isize;
pub type HNODE = isize;
pub type HNODEENUM = isize;
pub type HNODEENUMEX = isize;
pub type HREGBATCH = isize;
pub type HREGBATCHNOTIFICATION = isize;
pub type HREGBATCHPORT = isize;
pub type HREGREADBATCH = isize;
pub type HREGREADBATCHREPLY = isize;
pub type HRESENUM = isize;
pub type HRESENUMEX = isize;
pub type HRESOURCE = isize;
pub type HRESTYPEENUM = isize;
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct MONITOR_STATE {
    pub LastUpdate: i64,
    pub State: RESOURCE_MONITOR_STATE,
    pub ActiveResource: super::super::Foundation::HANDLE,
    pub ResmonStop: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for MONITOR_STATE {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for MONITOR_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NOTIFY_FILTER_AND_TYPE {
    pub dwObjectType: u32,
    pub FilterFlags: i64,
}
impl ::core::marker::Copy for NOTIFY_FILTER_AND_TYPE {}
impl ::core::clone::Clone for NOTIFY_FILTER_AND_TYPE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NodeUtilizationInfoElement {
    pub Id: u64,
    pub AvailableMemory: u64,
    pub AvailableMemoryAfterReclamation: u64,
}
impl ::core::marker::Copy for NodeUtilizationInfoElement {}
impl ::core::clone::Clone for NodeUtilizationInfoElement {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct POST_UPGRADE_VERSION_INFO {
    pub newMajorVersion: u32,
    pub newUpgradeVersion: u32,
    pub oldMajorVersion: u32,
    pub oldUpgradeVersion: u32,
    pub reserved: u32,
}
impl ::core::marker::Copy for POST_UPGRADE_VERSION_INFO {}
impl ::core::clone::Clone for POST_UPGRADE_VERSION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PaxosTagCStruct {
    pub __padding__PaxosTagVtable: u64,
    pub __padding__NextEpochVtable: u64,
    pub __padding__NextEpoch_DateTimeVtable: u64,
    pub NextEpoch_DateTime_ticks: u64,
    pub NextEpoch_Value: i32,
    pub __padding__BoundryNextEpoch: u32,
    pub __padding__EpochVtable: u64,
    pub __padding__Epoch_DateTimeVtable: u64,
    pub Epoch_DateTime_ticks: u64,
    pub Epoch_Value: i32,
    pub __padding__BoundryEpoch: u32,
    pub Sequence: i32,
    pub __padding__BoundrySequence: u32,
}
impl ::core::marker::Copy for PaxosTagCStruct {}
impl ::core::clone::Clone for PaxosTagCStruct {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESOURCE_FAILURE_INFO {
    pub dwRestartAttemptsRemaining: u32,
    pub dwRestartPeriodRemaining: u32,
}
impl ::core::marker::Copy for RESOURCE_FAILURE_INFO {}
impl ::core::clone::Clone for RESOURCE_FAILURE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESOURCE_FAILURE_INFO_BUFFER {
    pub dwVersion: u32,
    pub Info: RESOURCE_FAILURE_INFO,
}
impl ::core::marker::Copy for RESOURCE_FAILURE_INFO_BUFFER {}
impl ::core::clone::Clone for RESOURCE_FAILURE_INFO_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESOURCE_STATUS {
    pub ResourceState: CLUSTER_RESOURCE_STATE,
    pub CheckPoint: u32,
    pub WaitHint: u32,
    pub EventHandle: super::super::Foundation::HANDLE,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESOURCE_STATUS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESOURCE_STATUS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESOURCE_STATUS_EX {
    pub ResourceState: CLUSTER_RESOURCE_STATE,
    pub CheckPoint: u32,
    pub EventHandle: super::super::Foundation::HANDLE,
    pub ApplicationSpecificErrorCode: u32,
    pub Flags: u32,
    pub WaitHint: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESOURCE_STATUS_EX {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESOURCE_STATUS_EX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESOURCE_TERMINAL_FAILURE_INFO_BUFFER {
    pub isTerminalFailure: super::super::Foundation::BOOL,
    pub restartPeriodRemaining: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESOURCE_TERMINAL_FAILURE_INFO_BUFFER {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESOURCE_TERMINAL_FAILURE_INFO_BUFFER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESUTIL_FILETIME_DATA {
    pub Default: super::super::Foundation::FILETIME,
    pub Minimum: super::super::Foundation::FILETIME,
    pub Maximum: super::super::Foundation::FILETIME,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESUTIL_FILETIME_DATA {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESUTIL_FILETIME_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESUTIL_LARGEINT_DATA {
    pub Default: i64,
    pub Minimum: i64,
    pub Maximum: i64,
}
impl ::core::marker::Copy for RESUTIL_LARGEINT_DATA {}
impl ::core::clone::Clone for RESUTIL_LARGEINT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct RESUTIL_PROPERTY_ITEM {
    pub Name: ::windows_sys::core::PWSTR,
    pub KeyName: ::windows_sys::core::PWSTR,
    pub Format: u32,
    pub Anonymous: RESUTIL_PROPERTY_ITEM_0,
    pub Minimum: u32,
    pub Maximum: u32,
    pub Flags: u32,
    pub Offset: u32,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESUTIL_PROPERTY_ITEM {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESUTIL_PROPERTY_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub union RESUTIL_PROPERTY_ITEM_0 {
    pub DefaultPtr: usize,
    pub Default: u32,
    pub lpDefault: *mut ::core::ffi::c_void,
    pub LargeIntData: *mut RESUTIL_LARGEINT_DATA,
    pub ULargeIntData: *mut RESUTIL_ULARGEINT_DATA,
    pub FileTimeData: *mut RESUTIL_FILETIME_DATA,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for RESUTIL_PROPERTY_ITEM_0 {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for RESUTIL_PROPERTY_ITEM_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RESUTIL_ULARGEINT_DATA {
    pub Default: u64,
    pub Minimum: u64,
    pub Maximum: u64,
}
impl ::core::marker::Copy for RESUTIL_ULARGEINT_DATA {}
impl ::core::clone::Clone for RESUTIL_ULARGEINT_DATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct ResourceUtilizationInfoElement {
    pub PhysicalNumaId: u64,
    pub CurrentMemory: u64,
}
impl ::core::marker::Copy for ResourceUtilizationInfoElement {}
impl ::core::clone::Clone for ResourceUtilizationInfoElement {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SR_RESOURCE_TYPE_ADD_REPLICATION_GROUP {
    pub ReplicationGroupName: [u16; 260],
    pub Description: [u16; 260],
    pub LogPath: [u16; 260],
    pub MaxLogSizeInBytes: u64,
    pub LogType: u16,
    pub ReplicationMode: u32,
    pub MinimumPartnersInSync: u32,
    pub EnableWriteConsistency: super::super::Foundation::BOOLEAN,
    pub EnableEncryption: super::super::Foundation::BOOLEAN,
    pub EnableCompression: super::super::Foundation::BOOLEAN,
    pub CertificateThumbprint: [u16; 260],
    pub VolumeNameCount: u32,
    pub VolumeNames: [u16; 260],
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SR_RESOURCE_TYPE_ADD_REPLICATION_GROUP {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SR_RESOURCE_TYPE_ADD_REPLICATION_GROUP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_ADD_REPLICATION_GROUP_RESULT {
    pub Result: u32,
    pub ErrorString: [u16; 260],
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_ADD_REPLICATION_GROUP_RESULT {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_ADD_REPLICATION_GROUP_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_DISK_INFO {
    pub Reason: SR_DISK_REPLICATION_ELIGIBLE,
    pub DiskGuid: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_DISK_INFO {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_DISK_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_ELIGIBLE_DISKS_RESULT {
    pub Count: u16,
    pub DiskInfo: [SR_RESOURCE_TYPE_DISK_INFO; 1],
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_ELIGIBLE_DISKS_RESULT {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_ELIGIBLE_DISKS_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SR_RESOURCE_TYPE_QUERY_ELIGIBLE_LOGDISKS {
    pub DataDiskGuid: ::windows_sys::core::GUID,
    pub IncludeOfflineDisks: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SR_RESOURCE_TYPE_QUERY_ELIGIBLE_LOGDISKS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SR_RESOURCE_TYPE_QUERY_ELIGIBLE_LOGDISKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SR_RESOURCE_TYPE_QUERY_ELIGIBLE_SOURCE_DATADISKS {
    pub DataDiskGuid: ::windows_sys::core::GUID,
    pub IncludeAvailableStoargeDisks: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SR_RESOURCE_TYPE_QUERY_ELIGIBLE_SOURCE_DATADISKS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SR_RESOURCE_TYPE_QUERY_ELIGIBLE_SOURCE_DATADISKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct SR_RESOURCE_TYPE_QUERY_ELIGIBLE_TARGET_DATADISKS {
    pub SourceDataDiskGuid: ::windows_sys::core::GUID,
    pub TargetReplicationGroupGuid: ::windows_sys::core::GUID,
    pub SkipConnectivityCheck: super::super::Foundation::BOOLEAN,
    pub IncludeOfflineDisks: super::super::Foundation::BOOLEAN,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for SR_RESOURCE_TYPE_QUERY_ELIGIBLE_TARGET_DATADISKS {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for SR_RESOURCE_TYPE_QUERY_ELIGIBLE_TARGET_DATADISKS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_REPLICATED_DISK {
    pub Type: SR_REPLICATED_DISK_TYPE,
    pub ClusterDiskResourceGuid: ::windows_sys::core::GUID,
    pub ReplicationGroupId: ::windows_sys::core::GUID,
    pub ReplicationGroupName: [u16; 260],
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_REPLICATED_DISK {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_REPLICATED_DISK {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_REPLICATED_DISKS_RESULT {
    pub Count: u16,
    pub ReplicatedDisks: [SR_RESOURCE_TYPE_REPLICATED_DISK; 1],
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_REPLICATED_DISKS_RESULT {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_REPLICATED_DISKS_RESULT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_REPLICATED_PARTITION_ARRAY {
    pub Count: u32,
    pub PartitionArray: [SR_RESOURCE_TYPE_REPLICATED_PARTITION_INFO; 1],
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_REPLICATED_PARTITION_ARRAY {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_REPLICATED_PARTITION_ARRAY {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SR_RESOURCE_TYPE_REPLICATED_PARTITION_INFO {
    pub PartitionOffset: u64,
    pub Capabilities: u32,
}
impl ::core::marker::Copy for SR_RESOURCE_TYPE_REPLICATED_PARTITION_INFO {}
impl ::core::clone::Clone for SR_RESOURCE_TYPE_REPLICATED_PARTITION_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WitnessTagHelper {
    pub Version: i32,
    pub paxosToValidate: PaxosTagCStruct,
}
impl ::core::marker::Copy for WitnessTagHelper {}
impl ::core::clone::Clone for WitnessTagHelper {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct WitnessTagUpdateHelper {
    pub Version: i32,
    pub paxosToSet: PaxosTagCStruct,
    pub paxosToValidate: PaxosTagCStruct,
}
impl ::core::marker::Copy for WitnessTagUpdateHelper {}
impl ::core::clone::Clone for WitnessTagUpdateHelper {
    fn clone(&self) -> Self {
        *self
    }
}
pub type LPGROUP_CALLBACK_EX = ::core::option::Option<unsafe extern "system" fn(param0: HCLUSTER, param1: HGROUP, param2: HGROUP, param3: *mut ::core::ffi::c_void) -> u32>;
pub type LPNODE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(param0: HCLUSTER, param1: HNODE, param2: CLUSTER_NODE_STATE, param3: *mut ::core::ffi::c_void) -> u32>;
pub type LPRESOURCE_CALLBACK = ::core::option::Option<unsafe extern "system" fn(param0: HRESOURCE, param1: HRESOURCE, param2: *mut ::core::ffi::c_void) -> u32>;
pub type LPRESOURCE_CALLBACK_EX = ::core::option::Option<unsafe extern "system" fn(param0: HCLUSTER, param1: HRESOURCE, param2: HRESOURCE, param3: *mut ::core::ffi::c_void) -> u32>;
pub type PARBITRATE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void, lostquorumresource: PQUORUM_RESOURCE_LOST) -> u32>;
pub type PARM_WPR_WATCHDOG_FOR_CURRENT_RESOURCE_CALL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, timeoutinms: u64) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PBEGIN_RESCALL_AS_USER_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void, tokenhandle: super::super::Foundation::HANDLE, controlcode: u32, inbuffer: *mut ::core::ffi::c_void, inbuffersize: u32, outbuffer: *mut ::core::ffi::c_void, outbuffersize: u32, bytesreturned: *mut u32, context: i64, returnedasynchronously: *mut super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PBEGIN_RESCALL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void, controlcode: u32, inbuffer: *mut ::core::ffi::c_void, inbuffersize: u32, outbuffer: *mut ::core::ffi::c_void, outbuffersize: u32, bytesreturned: *mut u32, context: i64, returnedasynchronously: *mut super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PBEGIN_RESTYPECALL_AS_USER_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcetypename: ::windows_sys::core::PCWSTR, tokenhandle: super::super::Foundation::HANDLE, controlcode: u32, inbuffer: *mut ::core::ffi::c_void, inbuffersize: u32, outbuffer: *mut ::core::ffi::c_void, outbuffersize: u32, bytesreturned: *mut u32, context: i64, returnedasynchronously: *mut super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PBEGIN_RESTYPECALL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcetypename: ::windows_sys::core::PCWSTR, controlcode: u32, inbuffer: *mut ::core::ffi::c_void, inbuffersize: u32, outbuffer: *mut ::core::ffi::c_void, outbuffersize: u32, bytesreturned: *mut u32, context: i64, returnedasynchronously: *mut super::super::Foundation::BOOL) -> u32>;
pub type PCANCEL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void, cancelflags_reserved: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCHANGE_RESOURCE_PROCESS_FOR_DUMPS = ::core::option::Option<unsafe extern "system" fn(resource: isize, processname: ::windows_sys::core::PCWSTR, processid: u32, isadd: super::super::Foundation::BOOL) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCHANGE_RES_TYPE_PROCESS_FOR_DUMPS = ::core::option::Option<unsafe extern "system" fn(resourcetypename: ::windows_sys::core::PCWSTR, processname: ::windows_sys::core::PCWSTR, processid: u32, isadd: super::super::Foundation::BOOL) -> u32>;
pub type PCLOSE_CLUSTER_CRYPT_PROVIDER = ::core::option::Option<unsafe extern "system" fn(hcluscryptprovider: HCLUSCRYPTPROVIDER) -> u32>;
pub type PCLOSE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPIClusWorkerCheckTerminate = ::core::option::Option<unsafe extern "system" fn(lpworker: *mut CLUS_WORKER) -> super::super::Foundation::BOOL>;
pub type PCLUSAPI_ADD_CLUSTER_GROUP_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hdependentgroup: HGROUP, hprovidergroup: HGROUP) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_GROUP_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hdependentgroup: HGROUP, hprovidergroup: HGROUP, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_GROUP_GROUPSET_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hdependentgroupset: HGROUPSET, hprovidergroupset: HGROUPSET) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_GROUP_GROUPSET_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hdependentgroupset: HGROUPSET, hprovidergroupset: HGROUPSET, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_GROUP_TO_GROUP_GROUPSET_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hdependentgroup: HGROUP, hprovidergroupset: HGROUPSET) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_GROUP_TO_GROUP_GROUPSET_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hdependentgroup: HGROUP, hprovidergroupset: HGROUPSET, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_ADD_CLUSTER_NODE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznodename: ::windows_sys::core::PCWSTR, pfnprogresscallback: PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void) -> HNODE>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_ADD_CLUSTER_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznodename: ::windows_sys::core::PCWSTR, dwflags: u32, pfnprogresscallback: PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void) -> HNODE>;
pub type PCLUSAPI_ADD_CLUSTER_RESOURCE_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hdependson: HRESOURCE) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_RESOURCE_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hdependson: HRESOURCE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_RESOURCE_NODE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hnode: HNODE) -> u32>;
pub type PCLUSAPI_ADD_CLUSTER_RESOURCE_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hnode: HNODE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_ADD_CROSS_CLUSTER_GROUPSET_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hdependentgroupset: HGROUPSET, lpremoteclustername: ::windows_sys::core::PCWSTR, lpremotegroupsetname: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_ADD_RESOURCE_TO_CLUSTER_SHARED_VOLUMES = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> u32>;
pub type PCLUSAPI_BACKUP_CLUSTER_DATABASE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszpathname: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CAN_RESOURCE_BE_DEPENDENT = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hresourcedependent: HRESOURCE) -> super::super::Foundation::BOOL>;
pub type PCLUSAPI_CHANGE_CLUSTER_RESOURCE_GROUP = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_CHANGE_CLUSTER_RESOURCE_GROUP_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hgroup: HGROUP, flags: u64) -> u32>;
pub type PCLUSAPI_CHANGE_CLUSTER_RESOURCE_GROUP_EX2 = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hgroup: HGROUP, flags: u64, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_GROUP_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_NETWORK = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_NET_INTERFACE = ::core::option::Option<unsafe extern "system" fn(hnetinterface: HNETINTERFACE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_NODE = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_NOTIFY_PORT = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLOSE_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> super::super::Foundation::BOOL>;
pub type PCLUSAPI_CLUSTER_ADD_GROUP_TO_AFFINITY_RULE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, rulename: ::windows_sys::core::PCWSTR, hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_CLUSTER_ADD_GROUP_TO_GROUPSET_WITH_DOMAINS_EX = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, hgroup: HGROUP, faultdomain: u32, updatedomain: u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_ADD_GROUP_TO_GROUP_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_CLUSTER_AFFINITY_RULE_CONTROL = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, affinityrulename: ::windows_sys::core::PCWSTR, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, cbinbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, cboutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_CLOSE_ENUM = ::core::option::Option<unsafe extern "system" fn(henum: HCLUSENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_CLOSE_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hclusterenum: HCLUSENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_CONTROL = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_CREATE_AFFINITY_RULE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, rulename: ::windows_sys::core::PCWSTR, ruletype: CLUS_AFFINITY_RULE_TYPE) -> u32>;
pub type PCLUSAPI_CLUSTER_ENUM = ::core::option::Option<unsafe extern "system" fn(henum: HCLUSENUM, dwindex: u32, lpdwtype: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hclusterenum: HCLUSENUMEX, dwindex: u32, pitem: *mut CLUSTER_ENUM_ITEM, cbitem: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_GET_ENUM_COUNT = ::core::option::Option<unsafe extern "system" fn(henum: HCLUSENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_GET_ENUM_COUNT_EX = ::core::option::Option<unsafe extern "system" fn(hclusterenum: HCLUSENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_CLOSE_ENUM = ::core::option::Option<unsafe extern "system" fn(hgroupenum: HGROUPENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_CLOSE_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hgroupenumex: HGROUPENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_CONTROL = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_ENUM = ::core::option::Option<unsafe extern "system" fn(hgroupenum: HGROUPENUM, dwindex: u32, lpdwtype: *mut u32, lpszresourcename: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hgroupenumex: HGROUPENUMEX, dwindex: u32, pitem: *mut CLUSTER_GROUP_ENUM_ITEM, cbitem: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_GET_ENUM_COUNT = ::core::option::Option<unsafe extern "system" fn(hgroupenum: HGROUPENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_GET_ENUM_COUNT_EX = ::core::option::Option<unsafe extern "system" fn(hgroupenumex: HGROUPENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_GROUPSET_CONTROL = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, cbinbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, cboutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_GROUPSET_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, cbinbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, cboutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_GROUP_OPEN_ENUM = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, dwtype: u32) -> HGROUPENUM>;
pub type PCLUSAPI_CLUSTER_GROUP_OPEN_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszproperties: ::windows_sys::core::PCWSTR, cbproperties: u32, lpszroproperties: ::windows_sys::core::PCWSTR, cbroproperties: u32, dwflags: u32) -> HGROUPENUMEX>;
pub type PCLUSAPI_CLUSTER_NETWORK_CLOSE_ENUM = ::core::option::Option<unsafe extern "system" fn(hnetworkenum: HNETWORKENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_NETWORK_CONTROL = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_NETWORK_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_NETWORK_ENUM = ::core::option::Option<unsafe extern "system" fn(hnetworkenum: HNETWORKENUM, dwindex: u32, lpdwtype: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_NETWORK_GET_ENUM_COUNT = ::core::option::Option<unsafe extern "system" fn(hnetworkenum: HNETWORKENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_NETWORK_OPEN_ENUM = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, dwtype: u32) -> HNETWORKENUM>;
pub type PCLUSAPI_CLUSTER_NET_INTERFACE_CONTROL = ::core::option::Option<unsafe extern "system" fn(hnetinterface: HNETINTERFACE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_NET_INTERFACE_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hnetinterface: HNETINTERFACE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_CLOSE_ENUM = ::core::option::Option<unsafe extern "system" fn(hnodeenum: HNODEENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_CLOSE_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hnodeenum: HNODEENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_CONTROL = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_ENUM = ::core::option::Option<unsafe extern "system" fn(hnodeenum: HNODEENUM, dwindex: u32, lpdwtype: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hnodeenum: HNODEENUMEX, dwindex: u32, pitem: *mut CLUSTER_ENUM_ITEM, cbitem: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_GET_ENUM_COUNT = ::core::option::Option<unsafe extern "system" fn(hnodeenum: HNODEENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_GET_ENUM_COUNT_EX = ::core::option::Option<unsafe extern "system" fn(hnodeenum: HNODEENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_NODE_OPEN_ENUM = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, dwtype: u32) -> HNODEENUM>;
pub type PCLUSAPI_CLUSTER_NODE_OPEN_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, dwtype: u32, poptions: *const ::core::ffi::c_void) -> HNODEENUMEX>;
pub type PCLUSAPI_CLUSTER_OPEN_ENUM = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, dwtype: u32) -> HCLUSENUM>;
pub type PCLUSAPI_CLUSTER_OPEN_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, dwtype: u32, poptions: *const ::core::ffi::c_void) -> HCLUSENUMEX>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_CLOSE_KEY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_CREATE_BATCH = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, phregbatch: *mut HREGBATCH) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_CREATE_KEY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszsubkey: ::windows_sys::core::PCWSTR, dwoptions: u32, samdesired: u32, lpsecurityattributes: *const super::super::Security::SECURITY_ATTRIBUTES, phkresult: *mut super::super::System::Registry::HKEY, lpdwdisposition: *mut u32) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Security\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Security", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_CREATE_KEY_EX = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszsubkey: ::windows_sys::core::PCWSTR, dwoptions: u32, samdesired: u32, lpsecurityattributes: *const super::super::Security::SECURITY_ATTRIBUTES, phkresult: *mut super::super::System::Registry::HKEY, lpdwdisposition: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_DELETE_KEY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszsubkey: ::windows_sys::core::PCWSTR) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_DELETE_KEY_EX = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpsubkey: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_DELETE_VALUE = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszvaluename: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_DELETE_VALUE_EX = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszvaluename: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_ENUM_KEY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, dwindex: u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32, lpftlastwritetime: *mut super::super::Foundation::FILETIME) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_ENUM_VALUE = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, dwindex: u32, lpszvaluename: ::windows_sys::core::PWSTR, lpcchvaluename: *mut u32, lpdwtype: *mut u32, lpdata: *mut u8, lpcbdata: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Security\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Security", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_GET_KEY_SECURITY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, requestedinformation: u32, psecuritydescriptor: super::super::Security::PSECURITY_DESCRIPTOR, lpcbsecuritydescriptor: *mut u32) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_OPEN_KEY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszsubkey: ::windows_sys::core::PCWSTR, samdesired: u32, phkresult: *mut super::super::System::Registry::HKEY) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_QUERY_INFO_KEY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpcsubkeys: *mut u32, lpcbmaxsubkeylen: *mut u32, lpcvalues: *mut u32, lpcbmaxvaluenamelen: *mut u32, lpcbmaxvaluelen: *mut u32, lpcbsecuritydescriptor: *mut u32, lpftlastwritetime: *mut super::super::Foundation::FILETIME) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_QUERY_VALUE = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszvaluename: ::windows_sys::core::PCWSTR, lpdwvaluetype: *mut u32, lpdata: *mut u8, lpcbdata: *mut u32) -> i32>;
#[doc = "Required features: `\"Win32_Security\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Security", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_SET_KEY_SECURITY = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, securityinformation: u32, psecuritydescriptor: super::super::Security::PSECURITY_DESCRIPTOR) -> i32>;
#[doc = "Required features: `\"Win32_Security\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Security", feature = "Win32_System_Registry"))]
pub type PCLUSAPI_CLUSTER_REG_SET_KEY_SECURITY_EX = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, securityinformation: u32, psecuritydescriptor: super::super::Security::PSECURITY_DESCRIPTOR, lpszreason: ::windows_sys::core::PCWSTR) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_SET_VALUE = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszvaluename: ::windows_sys::core::PCWSTR, dwtype: u32, lpdata: *const u8, cbdata: u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_CLUSTER_REG_SET_VALUE_EX = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, lpszvaluename: ::windows_sys::core::PCWSTR, dwtype: u32, lpdata: *const u8, cbdata: u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_REG_SYNC_DATABASE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, flags: u32) -> i32>;
pub type PCLUSAPI_CLUSTER_REMOVE_AFFINITY_RULE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, rulename: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_REMOVE_GROUP_FROM_AFFINITY_RULE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, rulename: ::windows_sys::core::PCWSTR, hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_CLUSTER_REMOVE_GROUP_FROM_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET) -> u32>;
pub type PCLUSAPI_CLUSTER_REMOVE_GROUP_FROM_GROUPSET_EX = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_CLOSE_ENUM = ::core::option::Option<unsafe extern "system" fn(hresenum: HRESENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_CLOSE_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hresourceenumex: HRESENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_CONTROL = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, cbinbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, cboutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_CONTROL_AS_USER_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, cbinbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, cboutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, cbinbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, cboutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_ENUM = ::core::option::Option<unsafe extern "system" fn(hresenum: HRESENUM, dwindex: u32, lpdwtype: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hresourceenumex: HRESENUMEX, dwindex: u32, pitem: *mut CLUSTER_RESOURCE_ENUM_ITEM, cbitem: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_GET_ENUM_COUNT = ::core::option::Option<unsafe extern "system" fn(hresenum: HRESENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_GET_ENUM_COUNT_EX = ::core::option::Option<unsafe extern "system" fn(hresourceenumex: HRESENUMEX) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_OPEN_ENUM = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, dwtype: u32) -> HRESENUM>;
pub type PCLUSAPI_CLUSTER_RESOURCE_OPEN_ENUM_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszproperties: ::windows_sys::core::PCWSTR, cbproperties: u32, lpszroproperties: ::windows_sys::core::PCWSTR, cbroproperties: u32, dwflags: u32) -> HRESENUMEX>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_CLOSE_ENUM = ::core::option::Option<unsafe extern "system" fn(hrestypeenum: HRESTYPEENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_CONTROL = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_CONTROL_AS_USER_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_CONTROL_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR, hhostnode: HNODE, dwcontrolcode: u32, lpinbuffer: *const ::core::ffi::c_void, ninbuffersize: u32, lpoutbuffer: *mut ::core::ffi::c_void, noutbuffersize: u32, lpbytesreturned: *mut u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_ENUM = ::core::option::Option<unsafe extern "system" fn(hrestypeenum: HRESTYPEENUM, dwindex: u32, lpdwtype: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_GET_ENUM_COUNT = ::core::option::Option<unsafe extern "system" fn(hrestypeenum: HRESTYPEENUM) -> u32>;
pub type PCLUSAPI_CLUSTER_RESOURCE_TYPE_OPEN_ENUM = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR, dwtype: u32) -> HRESTYPEENUM>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLUSTER_UPGRADE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, perform: super::super::Foundation::BOOL, pfnprogresscallback: PCLUSTER_UPGRADE_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLUS_WORKER_CREATE = ::core::option::Option<unsafe extern "system" fn(lpworker: *mut CLUS_WORKER, lpstartaddress: PWORKER_START_ROUTINE, lpparameter: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CLUS_WORKER_TERMINATE = ::core::option::Option<unsafe extern "system" fn(lpworker: *const CLUS_WORKER) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CREATE_CLUSTER = ::core::option::Option<unsafe extern "system" fn(pconfig: *const CREATE_CLUSTER_CONFIG, pfnprogresscallback: PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void) -> HCLUSTER>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CREATE_CLUSTER_AVAILABILITY_SET = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpavailabilitysetname: ::windows_sys::core::PCWSTR, pavailabilitysetconfig: *const CLUSTER_AVAILABILITY_SET_CONFIG) -> HGROUPSET>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CREATE_CLUSTER_CNOLESS = ::core::option::Option<unsafe extern "system" fn(pconfig: *const CREATE_CLUSTER_CONFIG, pfnprogresscallback: PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void) -> HCLUSTER>;
pub type PCLUSAPI_CREATE_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszgroupname: ::windows_sys::core::PCWSTR) -> HGROUP>;
pub type PCLUSAPI_CREATE_CLUSTER_GROUPEX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszgroupname: ::windows_sys::core::PCWSTR, pgroupinfo: *const CLUSTER_CREATE_GROUP_INFO) -> HGROUP>;
pub type PCLUSAPI_CREATE_CLUSTER_GROUP_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszgroupsetname: ::windows_sys::core::PCWSTR) -> HGROUPSET>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_CREATE_CLUSTER_NAME_ACCOUNT = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, pconfig: *const CREATE_CLUSTER_NAME_ACCOUNT, pfnprogresscallback: PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void) -> u32>;
pub type PCLUSAPI_CREATE_CLUSTER_NOTIFY_PORT = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, hcluster: HCLUSTER, dwfilter: u32, dwnotifykey: usize) -> HCHANGE>;
pub type PCLUSAPI_CREATE_CLUSTER_NOTIFY_PORT_V2 = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, hcluster: HCLUSTER, filters: *const NOTIFY_FILTER_AND_TYPE, dwfiltercount: u32, dwnotifykey: usize) -> HCHANGE>;
pub type PCLUSAPI_CREATE_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszresourcename: ::windows_sys::core::PCWSTR, lpszresourcetype: ::windows_sys::core::PCWSTR, dwflags: u32) -> HRESOURCE>;
pub type PCLUSAPI_CREATE_CLUSTER_RESOURCE_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszresourcename: ::windows_sys::core::PCWSTR, lpszresourcetype: ::windows_sys::core::PCWSTR, dwflags: u32, lpszreason: ::windows_sys::core::PCWSTR) -> HRESOURCE>;
pub type PCLUSAPI_CREATE_CLUSTER_RESOURCE_TYPE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR, lpszdisplayname: ::windows_sys::core::PCWSTR, lpszresourcetypedll: ::windows_sys::core::PCWSTR, dwlooksalivepollinterval: u32, dwisalivepollinterval: u32) -> u32>;
pub type PCLUSAPI_CREATE_CLUSTER_RESOURCE_TYPE_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR, lpszdisplayname: ::windows_sys::core::PCWSTR, lpszresourcetypedll: ::windows_sys::core::PCWSTR, dwlooksalivepollinterval: u32, dwisalivepollinterval: u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_GROUP_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_GROUP_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_GROUP_GROUPSET_EX = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_RESOURCE_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_RESOURCE_TYPE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcetypename: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_DELETE_CLUSTER_RESOURCE_TYPE_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsztypename: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_DESTROY_CLUSTER = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, pfnprogresscallback: PCLUSTER_SETUP_PROGRESS_CALLBACK, pvcallbackarg: *const ::core::ffi::c_void, fdeletevirtualcomputerobjects: super::super::Foundation::BOOL) -> u32>;
pub type PCLUSAPI_DESTROY_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_DESTROY_CLUSTER_GROUP_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_EVICT_CLUSTER_NODE = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE) -> u32>;
pub type PCLUSAPI_EVICT_CLUSTER_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, dwtimeout: u32, phrcleanupstatus: *mut ::windows_sys::core::HRESULT) -> u32>;
pub type PCLUSAPI_EVICT_CLUSTER_NODE_EX2 = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, dwtimeout: u32, phrcleanupstatus: *mut ::windows_sys::core::HRESULT, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_FAIL_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> u32>;
pub type PCLUSAPI_FAIL_CLUSTER_RESOURCE_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_GET_CLUSTER_FROM_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP) -> HCLUSTER>;
pub type PCLUSAPI_GET_CLUSTER_FROM_GROUP_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET) -> HCLUSTER>;
pub type PCLUSAPI_GET_CLUSTER_FROM_NETWORK = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK) -> HCLUSTER>;
pub type PCLUSAPI_GET_CLUSTER_FROM_NET_INTERFACE = ::core::option::Option<unsafe extern "system" fn(hnetinterface: HNETINTERFACE) -> HCLUSTER>;
pub type PCLUSAPI_GET_CLUSTER_FROM_NODE = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE) -> HCLUSTER>;
pub type PCLUSAPI_GET_CLUSTER_FROM_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> HCLUSTER>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_GROUP_KEY = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, samdesired: u32) -> super::super::System::Registry::HKEY>;
pub type PCLUSAPI_GET_CLUSTER_GROUP_STATE = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpsznodename: ::windows_sys::core::PWSTR, lpcchnodename: *mut u32) -> CLUSTER_GROUP_STATE>;
pub type PCLUSAPI_GET_CLUSTER_INFORMATION = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszclustername: ::windows_sys::core::PWSTR, lpcchclustername: *mut u32, lpclusterinfo: *mut CLUSTERVERSIONINFO) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_KEY = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, samdesired: u32) -> super::super::System::Registry::HKEY>;
pub type PCLUSAPI_GET_CLUSTER_NETWORK_ID = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, lpsznetworkid: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_NETWORK_KEY = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, samdesired: u32) -> super::super::System::Registry::HKEY>;
pub type PCLUSAPI_GET_CLUSTER_NETWORK_STATE = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK) -> CLUSTER_NETWORK_STATE>;
pub type PCLUSAPI_GET_CLUSTER_NET_INTERFACE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznodename: ::windows_sys::core::PCWSTR, lpsznetworkname: ::windows_sys::core::PCWSTR, lpszinterfacename: ::windows_sys::core::PWSTR, lpcchinterfacename: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_NET_INTERFACE_KEY = ::core::option::Option<unsafe extern "system" fn(hnetinterface: HNETINTERFACE, samdesired: u32) -> super::super::System::Registry::HKEY>;
pub type PCLUSAPI_GET_CLUSTER_NET_INTERFACE_STATE = ::core::option::Option<unsafe extern "system" fn(hnetinterface: HNETINTERFACE) -> CLUSTER_NETINTERFACE_STATE>;
pub type PCLUSAPI_GET_CLUSTER_NODE_ID = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, lpsznodeid: ::windows_sys::core::PWSTR, lpcchname: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_NODE_KEY = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, samdesired: u32) -> super::super::System::Registry::HKEY>;
pub type PCLUSAPI_GET_CLUSTER_NODE_STATE = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE) -> CLUSTER_NODE_STATE>;
pub type PCLUSAPI_GET_CLUSTER_NOTIFY = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, lpdwnotifykey: *mut usize, lpdwfiltertype: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32, dwmilliseconds: u32) -> u32>;
pub type PCLUSAPI_GET_CLUSTER_NOTIFY_V2 = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, lpdwnotifykey: *mut usize, pfilterandtype: *mut NOTIFY_FILTER_AND_TYPE, buffer: *mut u8, lpcchbuffersize: *mut u32, lpszobjectid: ::windows_sys::core::PWSTR, lpcchobjectid: *mut u32, lpszparentid: ::windows_sys::core::PWSTR, lpcchparentid: *mut u32, lpszname: ::windows_sys::core::PWSTR, lpcchname: *mut u32, lpsztype: ::windows_sys::core::PWSTR, lpcchtype: *mut u32, dwmilliseconds: u32) -> u32>;
pub type PCLUSAPI_GET_CLUSTER_QUORUM_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcename: ::windows_sys::core::PWSTR, lpcchresourcename: *mut u32, lpszdevicename: ::windows_sys::core::PWSTR, lpcchdevicename: *mut u32, lpdwmaxquorumlogsize: *mut u32) -> u32>;
pub type PCLUSAPI_GET_CLUSTER_RESOURCE_DEPENDENCY_EXPRESSION = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszdependencyexpression: ::windows_sys::core::PWSTR, lpcchdependencyexpression: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_RESOURCE_KEY = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, samdesired: u32) -> super::super::System::Registry::HKEY>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_GET_CLUSTER_RESOURCE_NETWORK_NAME = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpbuffer: ::windows_sys::core::PWSTR, nsize: *mut u32) -> super::super::Foundation::BOOL>;
pub type PCLUSAPI_GET_CLUSTER_RESOURCE_STATE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpsznodename: ::windows_sys::core::PWSTR, lpcchnodename: *mut u32, lpszgroupname: ::windows_sys::core::PWSTR, lpcchgroupname: *mut u32) -> CLUSTER_RESOURCE_STATE>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSAPI_GET_CLUSTER_RESOURCE_TYPE_KEY = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsztypename: ::windows_sys::core::PCWSTR, samdesired: u32) -> super::super::System::Registry::HKEY>;
pub type PCLUSAPI_GET_NODE_CLUSTER_STATE = ::core::option::Option<unsafe extern "system" fn(lpsznodename: ::windows_sys::core::PCWSTR, pdwclusterstate: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_GET_NOTIFY_EVENT_HANDLE_V2 = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, lphtargetevent: *mut super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_IS_FILE_ON_CLUSTER_SHARED_VOLUME = ::core::option::Option<unsafe extern "system" fn(lpszpathname: ::windows_sys::core::PCWSTR, pbfileisonsharedvolume: *mut super::super::Foundation::BOOL) -> u32>;
pub type PCLUSAPI_MOVE_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hdestinationnode: HNODE) -> u32>;
pub type PCLUSAPI_OFFLINE_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP) -> u32>;
pub type PCLUSAPI_OFFLINE_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> u32>;
pub type PCLUSAPI_ONLINE_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hdestinationnode: HNODE) -> u32>;
pub type PCLUSAPI_ONLINE_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> u32>;
pub type PCLUSAPI_OPEN_CLUSTER = ::core::option::Option<unsafe extern "system" fn(lpszclustername: ::windows_sys::core::PCWSTR) -> HCLUSTER>;
pub type PCLUSAPI_OPEN_CLUSTER_EX = ::core::option::Option<unsafe extern "system" fn(lpszclustername: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32, lpdwgrantedaccess: *mut u32) -> HCLUSTER>;
pub type PCLUSAPI_OPEN_CLUSTER_GROUP = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszgroupname: ::windows_sys::core::PCWSTR) -> HGROUP>;
pub type PCLUSAPI_OPEN_CLUSTER_GROUP_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszgroupname: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32, lpdwgrantedaccess: *mut u32) -> HGROUP>;
pub type PCLUSAPI_OPEN_CLUSTER_GROUP_GROUPSET = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszgroupsetname: ::windows_sys::core::PCWSTR) -> HGROUPSET>;
pub type PCLUSAPI_OPEN_CLUSTER_NETINTERFACE_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznetinterfacename: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32, lpdwgrantedaccess: *mut u32) -> HNETINTERFACE>;
pub type PCLUSAPI_OPEN_CLUSTER_NETWORK = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznetworkname: ::windows_sys::core::PCWSTR) -> HNETWORK>;
pub type PCLUSAPI_OPEN_CLUSTER_NETWORK_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznetworkname: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32, lpdwgrantedaccess: *mut u32) -> HNETWORK>;
pub type PCLUSAPI_OPEN_CLUSTER_NET_INTERFACE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszinterfacename: ::windows_sys::core::PCWSTR) -> HNETINTERFACE>;
pub type PCLUSAPI_OPEN_CLUSTER_NODE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznodename: ::windows_sys::core::PCWSTR) -> HNODE>;
pub type PCLUSAPI_OPEN_CLUSTER_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznodename: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32, lpdwgrantedaccess: *mut u32) -> HNODE>;
pub type PCLUSAPI_OPEN_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcename: ::windows_sys::core::PCWSTR) -> HRESOURCE>;
pub type PCLUSAPI_OPEN_CLUSTER_RESOURCE_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpszresourcename: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32, lpdwgrantedaccess: *mut u32) -> HRESOURCE>;
pub type PCLUSAPI_OPEN_NODE_BY_ID = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, nodeid: u32) -> HNODE>;
pub type PCLUSAPI_PAUSE_CLUSTER_NODE = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_PAUSE_CLUSTER_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, bdrainnode: super::super::Foundation::BOOL, dwpauseflags: u32, hnodedraintarget: HNODE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_PAUSE_CLUSTER_NODE_EX2 = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, bdrainnode: super::super::Foundation::BOOL, dwpauseflags: u32, hnodedraintarget: HNODE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_PFN_REASON_HANDLER = ::core::option::Option<unsafe extern "system" fn(lpparameter: *const ::core::ffi::c_void, hcluster: HCLUSTER, szreason: ::windows_sys::core::PWSTR, lpsize: *mut u32) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_REGISTER_CLUSTER_NOTIFY = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, dwfiltertype: u32, hobject: super::super::Foundation::HANDLE, dwnotifykey: usize) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_REGISTER_CLUSTER_NOTIFY_V2 = ::core::option::Option<unsafe extern "system" fn(hchange: HCHANGE, filter: NOTIFY_FILTER_AND_TYPE, hobject: super::super::Foundation::HANDLE, dwnotifykey: usize) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_GROUP_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hdependson: HGROUP) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_GROUP_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hdependson: HGROUP, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_GROUP_GROUPSET_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, hdependson: HGROUPSET) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_GROUP_GROUPSET_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, hdependson: HGROUPSET, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_GROUP_TO_GROUP_GROUPSET_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hdependson: HGROUPSET) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_GROUP_TO_GROUP_GROUPSET_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, hdependson: HGROUPSET, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_NAME_ACCOUNT = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_RESOURCE_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hdependson: HRESOURCE) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_RESOURCE_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hdependson: HRESOURCE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_RESOURCE_NODE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hnode: HNODE) -> u32>;
pub type PCLUSAPI_REMOVE_CLUSTER_RESOURCE_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, hnode: HNODE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_REMOVE_CROSS_CLUSTER_GROUPSET_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hdependentgroupset: HGROUPSET, lpremoteclustername: ::windows_sys::core::PCWSTR, lpremotegroupsetname: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_REMOVE_RESOURCE_FROM_CLUSTER_SHARED_VOLUMES = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> u32>;
pub type PCLUSAPI_RESTART_CLUSTER_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, dwflags: u32) -> u32>;
pub type PCLUSAPI_RESTART_CLUSTER_RESOURCE_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, dwflags: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_RESTORE_CLUSTER_DATABASE = ::core::option::Option<unsafe extern "system" fn(lpszpathname: ::windows_sys::core::PCWSTR, bforce: super::super::Foundation::BOOL, lpszquorumdriveletter: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_RESUME_CLUSTER_NODE = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE) -> u32>;
pub type PCLUSAPI_RESUME_CLUSTER_NODE_EX = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, eresumefailbacktype: CLUSTER_NODE_RESUME_FAILBACK_TYPE, dwresumeflagsreserved: u32) -> u32>;
pub type PCLUSAPI_RESUME_CLUSTER_NODE_EX2 = ::core::option::Option<unsafe extern "system" fn(hnode: HNODE, eresumefailbacktype: CLUSTER_NODE_RESUME_FAILBACK_TYPE, dwresumeflagsreserved: u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_GROUP_GROUPSET_DEPENDENCY_EXPRESSION = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, lpszdependencyexpression: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_GROUP_GROUPSET_DEPENDENCY_EXPRESSION_EX = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUPSET, lpszdependencyexpression: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_GROUP_NAME = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszgroupname: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_GROUP_NAME_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszgroupname: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_GROUP_NODE_LIST = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, nodecount: u32, nodelist: *const HNODE) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_GROUP_NODE_LIST_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, nodecount: u32, nodelist: *const HNODE, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_NAME_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznewclustername: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_NETWORK_NAME = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, lpszname: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_NETWORK_NAME_EX = ::core::option::Option<unsafe extern "system" fn(hnetwork: HNETWORK, lpszname: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_NETWORK_PRIORITY_ORDER = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, networkcount: u32, networklist: *const HNETWORK) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_QUORUM_RESOURCE = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszdevicename: ::windows_sys::core::PCWSTR, dwmaxquologsize: u32) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_QUORUM_RESOURCE_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszdevicename: ::windows_sys::core::PCWSTR, dwmaxquorumlogsize: u32, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_RESOURCE_DEPENDENCY_EXPRESSION = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszdependencyexpression: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_RESOURCE_NAME = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszresourcename: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_CLUSTER_RESOURCE_NAME_EX = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, lpszresourcename: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_SET_CLUSTER_SERVICE_ACCOUNT_PASSWORD = ::core::option::Option<unsafe extern "system" fn(lpszclustername: ::windows_sys::core::PCWSTR, lpsznewpassword: ::windows_sys::core::PCWSTR, dwflags: u32, lpreturnstatusbuffer: *mut CLUSTER_SET_PASSWORD_STATUS, lpcbreturnstatusbuffersize: *mut u32) -> u32>;
pub type PCLUSAPI_SET_GROUP_DEPENDENCY_EXPRESSION = ::core::option::Option<unsafe extern "system" fn(hgroupset: HGROUP, lpszdependencyexpression: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSAPI_SET_GROUP_DEPENDENCY_EXPRESSION_EX = ::core::option::Option<unsafe extern "system" fn(hgroup: HGROUP, lpszdependencyexpression: ::windows_sys::core::PCWSTR, lpszreason: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSAPI_SET_REASON_HANDLER = ::core::option::Option<unsafe extern "system" fn(lphandler: *const CLUSAPI_REASON_HANDLER) -> *mut CLUSAPI_REASON_HANDLER>;
pub type PCLUSAPI_SHARED_VOLUME_SET_SNAPSHOT_STATE = ::core::option::Option<unsafe extern "system" fn(guidsnapshotset: ::windows_sys::core::GUID, lpszvolumename: ::windows_sys::core::PCWSTR, state: CLUSTER_SHARED_VOLUME_SNAPSHOT_STATE) -> u32>;
pub type PCLUSAPI_SetClusterName = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, lpsznewclustername: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSTER_CLEAR_BACKUP_STATE_FOR_SHARED_VOLUME = ::core::option::Option<unsafe extern "system" fn(lpszvolumepathname: ::windows_sys::core::PCWSTR) -> u32>;
pub type PCLUSTER_DECRYPT = ::core::option::Option<unsafe extern "system" fn(hcluscryptprovider: HCLUSCRYPTPROVIDER, pcryptinput: *const u8, cbcryptinput: u32, ppcryptoutput: *mut *mut u8, pcbcryptoutput: *mut u32) -> u32>;
pub type PCLUSTER_ENCRYPT = ::core::option::Option<unsafe extern "system" fn(hcluscryptprovider: HCLUSCRYPTPROVIDER, pdata: *const u8, cbdata: u32, ppdata: *mut *mut u8, pcbdata: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSTER_GET_VOLUME_NAME_FOR_VOLUME_MOUNT_POINT = ::core::option::Option<unsafe extern "system" fn(lpszvolumemountpoint: ::windows_sys::core::PCWSTR, lpszvolumename: ::windows_sys::core::PWSTR, cchbufferlength: u32) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSTER_GET_VOLUME_PATH_NAME = ::core::option::Option<unsafe extern "system" fn(lpszfilename: ::windows_sys::core::PCWSTR, lpszvolumepathname: ::windows_sys::core::PWSTR, cchbufferlength: u32) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSTER_IS_PATH_ON_SHARED_VOLUME = ::core::option::Option<unsafe extern "system" fn(lpszpathname: ::windows_sys::core::PCWSTR) -> super::super::Foundation::BOOL>;
pub type PCLUSTER_PREPARE_SHARED_VOLUME_FOR_BACKUP = ::core::option::Option<unsafe extern "system" fn(lpszfilename: ::windows_sys::core::PCWSTR, lpszvolumepathname: ::windows_sys::core::PWSTR, lpcchvolumepathname: *mut u32, lpszvolumename: ::windows_sys::core::PWSTR, lpcchvolumename: *mut u32) -> u32>;
pub type PCLUSTER_REG_BATCH_ADD_COMMAND = ::core::option::Option<unsafe extern "system" fn(hregbatch: HREGBATCH, dwcommand: CLUSTER_REG_COMMAND, wzname: ::windows_sys::core::PCWSTR, dwoptions: u32, lpdata: *const ::core::ffi::c_void, cbdata: u32) -> i32>;
pub type PCLUSTER_REG_BATCH_CLOSE_NOTIFICATION = ::core::option::Option<unsafe extern "system" fn(hbatchnotification: HREGBATCHNOTIFICATION) -> i32>;
pub type PCLUSTER_REG_BATCH_READ_COMMAND = ::core::option::Option<unsafe extern "system" fn(hbatchnotification: HREGBATCHNOTIFICATION, pbatchcommand: *mut CLUSTER_BATCH_COMMAND) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSTER_REG_CLOSE_BATCH = ::core::option::Option<unsafe extern "system" fn(hregbatch: HREGBATCH, bcommit: super::super::Foundation::BOOL, failedcommandnumber: *mut i32) -> i32>;
pub type PCLUSTER_REG_CLOSE_BATCH_NOTIFY_PORT = ::core::option::Option<unsafe extern "system" fn(hbatchnotifyport: HREGBATCHPORT) -> i32>;
pub type PCLUSTER_REG_CLOSE_READ_BATCH = ::core::option::Option<unsafe extern "system" fn(hregreadbatch: HREGREADBATCH, phregreadbatchreply: *mut HREGREADBATCHREPLY) -> i32>;
pub type PCLUSTER_REG_CLOSE_READ_BATCH_EX = ::core::option::Option<unsafe extern "system" fn(hregreadbatch: HREGREADBATCH, flags: u32, phregreadbatchreply: *mut HREGREADBATCHREPLY) -> i32>;
pub type PCLUSTER_REG_CLOSE_READ_BATCH_REPLY = ::core::option::Option<unsafe extern "system" fn(hregreadbatchreply: HREGREADBATCHREPLY) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSTER_REG_CREATE_BATCH_NOTIFY_PORT = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, phbatchnotifyport: *mut HREGBATCHPORT) -> i32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PCLUSTER_REG_CREATE_READ_BATCH = ::core::option::Option<unsafe extern "system" fn(hkey: super::super::System::Registry::HKEY, phregreadbatch: *mut HREGREADBATCH) -> i32>;
pub type PCLUSTER_REG_GET_BATCH_NOTIFICATION = ::core::option::Option<unsafe extern "system" fn(hbatchnotify: HREGBATCHPORT, phbatchnotification: *mut HREGBATCHNOTIFICATION) -> i32>;
pub type PCLUSTER_REG_READ_BATCH_ADD_COMMAND = ::core::option::Option<unsafe extern "system" fn(hregreadbatch: HREGREADBATCH, wzsubkeyname: ::windows_sys::core::PCWSTR, wzvaluename: ::windows_sys::core::PCWSTR) -> i32>;
pub type PCLUSTER_REG_READ_BATCH_REPLY_NEXT_COMMAND = ::core::option::Option<unsafe extern "system" fn(hregreadbatchreply: HREGREADBATCHREPLY, pbatchcommand: *mut CLUSTER_READ_BATCH_COMMAND) -> i32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSTER_SETUP_PROGRESS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pvcallbackarg: *mut ::core::ffi::c_void, esetupphase: CLUSTER_SETUP_PHASE, ephasetype: CLUSTER_SETUP_PHASE_TYPE, ephaseseverity: CLUSTER_SETUP_PHASE_SEVERITY, dwpercentcomplete: u32, lpszobjectname: ::windows_sys::core::PCWSTR, dwstatus: u32) -> super::super::Foundation::BOOL>;
pub type PCLUSTER_SET_ACCOUNT_ACCESS = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, szaccountsid: ::windows_sys::core::PCWSTR, dwaccess: u32, dwcontroltype: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PCLUSTER_UPGRADE_PROGRESS_CALLBACK = ::core::option::Option<unsafe extern "system" fn(pvcallbackarg: *mut ::core::ffi::c_void, eupgradephase: CLUSTER_UPGRADE_PHASE) -> super::super::Foundation::BOOL>;
pub type PEND_CONTROL_CALL = ::core::option::Option<unsafe extern "system" fn(context: i64, status: u32) -> u32>;
pub type PEND_TYPE_CONTROL_CALL = ::core::option::Option<unsafe extern "system" fn(context: i64, status: u32) -> u32>;
pub type PEXTEND_RES_CONTROL_CALL = ::core::option::Option<unsafe extern "system" fn(context: i64, newtimeoutinms: u32) -> u32>;
pub type PEXTEND_RES_TYPE_CONTROL_CALL = ::core::option::Option<unsafe extern "system" fn(context: i64, newtimeoutinms: u32) -> u32>;
pub type PFREE_CLUSTER_CRYPT = ::core::option::Option<unsafe extern "system" fn(pcryptinfo: *const ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PIS_ALIVE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
pub type PLOG_EVENT_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, loglevel: LOG_LEVEL, formatstring: ::windows_sys::core::PCWSTR) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PLOOKS_ALIVE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void) -> super::super::Foundation::BOOL>;
pub type POFFLINE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void) -> u32>;
pub type POFFLINE_V2_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *const ::core::ffi::c_void, destinationnodename: ::windows_sys::core::PCWSTR, offlineflags: u32, inbuffer: *const u8, inbuffersize: u32, reserved: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PONLINE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void, eventhandle: *mut super::super::Foundation::HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PONLINE_V2_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *const ::core::ffi::c_void, eventhandle: *mut super::super::Foundation::HANDLE, onlineflags: u32, inbuffer: *const u8, inbuffersize: u32, reserved: u32) -> u32>;
pub type POPEN_CLUSTER_CRYPT_PROVIDER = ::core::option::Option<unsafe extern "system" fn(lpszresource: ::windows_sys::core::PCWSTR, lpszprovider: *const i8, dwtype: u32, dwflags: u32) -> HCLUSCRYPTPROVIDER>;
pub type POPEN_CLUSTER_CRYPT_PROVIDEREX = ::core::option::Option<unsafe extern "system" fn(lpszresource: ::windows_sys::core::PCWSTR, lpszkeyname: ::windows_sys::core::PCWSTR, lpszprovider: *const i8, dwtype: u32, dwflags: u32) -> HCLUSCRYPTPROVIDER>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type POPEN_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcename: ::windows_sys::core::PCWSTR, resourcekey: super::super::System::Registry::HKEY, resourcehandle: isize) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type POPEN_V2_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcename: ::windows_sys::core::PCWSTR, resourcekey: super::super::System::Registry::HKEY, resourcehandle: isize, openflags: u32) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PQUERY_APPINSTANCE_VERSION = ::core::option::Option<unsafe extern "system" fn(appinstanceid: *const ::windows_sys::core::GUID, instanceversionhigh: *mut u64, instanceversionlow: *mut u64, versionstatus: *mut super::super::Foundation::NTSTATUS) -> u32>;
pub type PQUORUM_RESOURCE_LOST = ::core::option::Option<unsafe extern "system" fn(resource: isize) -> ()>;
pub type PRAISE_RES_TYPE_NOTIFICATION = ::core::option::Option<unsafe extern "system" fn(resourcetype: ::windows_sys::core::PCWSTR, ppayload: *const u8, payloadsize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PREGISTER_APPINSTANCE = ::core::option::Option<unsafe extern "system" fn(processhandle: super::super::Foundation::HANDLE, appinstanceid: *const ::windows_sys::core::GUID, childreninheritappinstance: super::super::Foundation::BOOL) -> u32>;
pub type PREGISTER_APPINSTANCE_VERSION = ::core::option::Option<unsafe extern "system" fn(appinstanceid: *const ::windows_sys::core::GUID, instanceversionhigh: u64, instanceversionlow: u64) -> u32>;
pub type PRELEASE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PREQUEST_DUMP_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, dumpduetocallinprogress: super::super::Foundation::BOOL, dumpdelayinms: u32) -> u32>;
pub type PRESET_ALL_APPINSTANCE_VERSIONS = ::core::option::Option<unsafe extern "system" fn() -> u32>;
pub type PRESOURCE_CONTROL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void, controlcode: u32, inbuffer: *mut ::core::ffi::c_void, inbuffersize: u32, outbuffer: *mut ::core::ffi::c_void, outbuffersize: u32, bytesreturned: *mut u32) -> u32>;
pub type PRESOURCE_TYPE_CONTROL_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcetypename: ::windows_sys::core::PCWSTR, controlcode: u32, inbuffer: *mut ::core::ffi::c_void, inbuffersize: u32, outbuffer: *mut ::core::ffi::c_void, outbuffersize: u32, bytesreturned: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_ADD_UNKNOWN_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, poutpropertylist: *mut ::core::ffi::c_void, pcboutpropertylistsize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
pub type PRESUTIL_CREATE_DIRECTORY_TREE = ::core::option::Option<unsafe extern "system" fn(pszpath: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_DUP_PARAMETER_BLOCK = ::core::option::Option<unsafe extern "system" fn(poutparams: *mut u8, pinparams: *const u8, ppropertytable: *const RESUTIL_PROPERTY_ITEM) -> u32>;
pub type PRESUTIL_DUP_STRING = ::core::option::Option<unsafe extern "system" fn(pszinstring: ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_ENUM_PRIVATE_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszoutproperties: ::windows_sys::core::PWSTR, cboutpropertiessize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_ENUM_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(ppropertytable: *const RESUTIL_PROPERTY_ITEM, pszoutproperties: ::windows_sys::core::PWSTR, cboutpropertiessize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
pub type PRESUTIL_ENUM_RESOURCES = ::core::option::Option<unsafe extern "system" fn(hself: HRESOURCE, lpszrestypename: ::windows_sys::core::PCWSTR, prescallback: LPRESOURCE_CALLBACK, pparameter: *mut ::core::ffi::c_void) -> u32>;
pub type PRESUTIL_ENUM_RESOURCES_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hself: HRESOURCE, lpszrestypename: ::windows_sys::core::PCWSTR, prescallback: LPRESOURCE_CALLBACK_EX, pparameter: *mut ::core::ffi::c_void) -> u32>;
pub type PRESUTIL_ENUM_RESOURCES_EX2 = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hself: HRESOURCE, lpszrestypename: ::windows_sys::core::PCWSTR, prescallback: LPRESOURCE_CALLBACK_EX, pparameter: *mut ::core::ffi::c_void, dwdesiredaccess: u32) -> u32>;
pub type PRESUTIL_EXPAND_ENVIRONMENT_STRINGS = ::core::option::Option<unsafe extern "system" fn(pszsrc: ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR>;
pub type PRESUTIL_FIND_BINARY_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pbpropertyvalue: *mut *mut u8, pcbpropertyvaluesize: *mut u32) -> u32>;
pub type PRESUTIL_FIND_DEPENDENT_DISK_RESOURCE_DRIVE_LETTER = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hresource: HRESOURCE, pszdriveletter: ::windows_sys::core::PWSTR, pcchdriveletter: *mut u32) -> u32>;
pub type PRESUTIL_FIND_DWORD_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pdwpropertyvalue: *mut u32) -> u32>;
pub type PRESUTIL_FIND_EXPANDED_SZ_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pszpropertyvalue: *mut ::windows_sys::core::PWSTR) -> u32>;
pub type PRESUTIL_FIND_EXPAND_SZ_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pszpropertyvalue: *mut ::windows_sys::core::PWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_FIND_FILETIME_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pftpropertyvalue: *mut super::super::Foundation::FILETIME) -> u32>;
pub type PRESUTIL_FIND_LONG_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, plpropertyvalue: *mut i32) -> u32>;
pub type PRESUTIL_FIND_MULTI_SZ_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pszpropertyvalue: *mut ::windows_sys::core::PWSTR, pcbpropertyvaluesize: *mut u32) -> u32>;
pub type PRESUTIL_FIND_SZ_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, pszpropertyvalue: *mut ::windows_sys::core::PWSTR) -> u32>;
pub type PRESUTIL_FIND_ULARGEINTEGER_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppropertylist: *const ::core::ffi::c_void, cbpropertylistsize: u32, pszpropertyname: ::windows_sys::core::PCWSTR, plpropertyvalue: *mut u64) -> u32>;
pub type PRESUTIL_FREE_ENVIRONMENT = ::core::option::Option<unsafe extern "system" fn(lpenvironment: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_FREE_PARAMETER_BLOCK = ::core::option::Option<unsafe extern "system" fn(poutparams: *mut u8, pinparams: *const u8, ppropertytable: *const RESUTIL_PROPERTY_ITEM) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_GET_ALL_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, poutpropertylist: *mut ::core::ffi::c_void, cboutpropertylistsize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
pub type PRESUTIL_GET_BINARY_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppboutvalue: *mut *mut u8, pcboutvaluesize: *mut u32, pvaluestruct: *const CLUSPROP_BINARY, pboldvalue: *const u8, cboldvaluesize: u32, pppropertylist: *mut *mut u8, pcbpropertylistsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_GET_BINARY_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, ppboutvalue: *mut *mut u8, pcboutvaluesize: *mut u32) -> u32>;
pub type PRESUTIL_GET_CORE_CLUSTER_RESOURCES = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, phclusternameresource: *mut HRESOURCE, phclusteripaddressresource: *mut HRESOURCE, phclusterquorumresource: *mut HRESOURCE) -> u32>;
pub type PRESUTIL_GET_CORE_CLUSTER_RESOURCES_EX = ::core::option::Option<unsafe extern "system" fn(hclusterin: HCLUSTER, phclusternameresourceout: *mut HRESOURCE, phclusteripaddressresourceout: *mut HRESOURCE, phclusterquorumresourceout: *mut HRESOURCE, dwdesiredaccess: u32) -> u32>;
pub type PRESUTIL_GET_DWORD_PROPERTY = ::core::option::Option<unsafe extern "system" fn(pdwoutvalue: *mut u32, pvaluestruct: *const CLUSPROP_DWORD, dwoldvalue: u32, dwminimum: u32, dwmaximum: u32, pppropertylist: *mut *mut u8, pcbpropertylistsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_GET_DWORD_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, pdwoutvalue: *mut u32, dwdefaultvalue: u32) -> u32>;
pub type PRESUTIL_GET_ENVIRONMENT_WITH_NET_NAME = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE) -> *mut ::core::ffi::c_void>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_GET_EXPAND_SZ_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, bexpand: super::super::Foundation::BOOL) -> ::windows_sys::core::PWSTR>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_FILETIME_PROPERTY = ::core::option::Option<unsafe extern "system" fn(pftoutvalue: *mut super::super::Foundation::FILETIME, pvaluestruct: *const CLUSPROP_FILETIME, ftoldvalue: super::super::Foundation::FILETIME, ftminimum: super::super::Foundation::FILETIME, ftmaximum: super::super::Foundation::FILETIME, pppropertylist: *mut *mut u8, pcbpropertylistsize: *mut u32) -> u32>;
pub type PRESUTIL_GET_LONG_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ploutvalue: *mut i32, pvaluestruct: *const CLUSPROP_LONG, loldvalue: i32, lminimum: i32, lmaximum: i32, pppropertylist: *mut *mut u8, pcbpropertylistsize: *mut u32) -> u32>;
pub type PRESUTIL_GET_MULTI_SZ_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppszoutvalue: *mut ::windows_sys::core::PWSTR, pcboutvaluesize: *mut u32, pvaluestruct: *const CLUSPROP_SZ, pszoldvalue: ::windows_sys::core::PCWSTR, cboldvaluesize: u32, pppropertylist: *mut *mut u8, pcbpropertylistsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_GET_PRIVATE_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, poutpropertylist: *mut ::core::ffi::c_void, cboutpropertylistsize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_GET_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, poutpropertylist: *mut ::core::ffi::c_void, cboutpropertylistsize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_GET_PROPERTIES_TO_PARAMETER_BLOCK = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, poutparams: *mut u8, bcheckforrequiredproperties: super::super::Foundation::BOOL, psznameofpropinerror: *mut ::windows_sys::core::PWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_GET_PROPERTY = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytableitem: *const RESUTIL_PROPERTY_ITEM, poutpropertyitem: *mut *mut ::core::ffi::c_void, pcboutpropertyitemsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_PROPERTY_FORMATS = ::core::option::Option<unsafe extern "system" fn(ppropertytable: *const RESUTIL_PROPERTY_ITEM, poutpropertyformatlist: *mut ::core::ffi::c_void, cbpropertyformatlistsize: u32, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_GET_PROPERTY_SIZE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytableitem: *const RESUTIL_PROPERTY_ITEM, pcboutpropertylistsize: *mut u32, pnpropertycount: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_GET_QWORD_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, pqwoutvalue: *mut u64, qwdefaultvalue: u64) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_RESOURCE_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(hself: super::super::Foundation::HANDLE, lpszresourcetype: ::windows_sys::core::PCWSTR) -> HRESOURCE>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_RESOURCE_DEPENDENCY_BY_CLASS = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hself: super::super::Foundation::HANDLE, prci: *mut CLUS_RESOURCE_CLASS_INFO, brecurse: super::super::Foundation::BOOL) -> HRESOURCE>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_RESOURCE_DEPENDENCY_BY_CLASS_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hself: super::super::Foundation::HANDLE, prci: *mut CLUS_RESOURCE_CLASS_INFO, brecurse: super::super::Foundation::BOOL, dwdesiredaccess: u32) -> HRESOURCE>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_RESOURCE_DEPENDENCY_BY_NAME = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hself: super::super::Foundation::HANDLE, lpszresourcetype: ::windows_sys::core::PCWSTR, brecurse: super::super::Foundation::BOOL) -> HRESOURCE>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_RESOURCE_DEPENDENCY_BY_NAME_EX = ::core::option::Option<unsafe extern "system" fn(hcluster: HCLUSTER, hself: super::super::Foundation::HANDLE, lpszresourcetype: ::windows_sys::core::PCWSTR, brecurse: super::super::Foundation::BOOL, dwdesiredaccess: u32) -> HRESOURCE>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_GET_RESOURCE_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(hself: super::super::Foundation::HANDLE, lpszresourcetype: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32) -> HRESOURCE>;
pub type PRESUTIL_GET_RESOURCE_DEPENDENTIP_ADDRESS_PROPS = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, pszaddress: ::windows_sys::core::PWSTR, pcchaddress: *mut u32, pszsubnetmask: ::windows_sys::core::PWSTR, pcchsubnetmask: *mut u32, psznetwork: ::windows_sys::core::PWSTR, pcchnetwork: *mut u32) -> u32>;
pub type PRESUTIL_GET_RESOURCE_NAME = ::core::option::Option<unsafe extern "system" fn(hresource: HRESOURCE, pszresourcename: ::windows_sys::core::PWSTR, pcchresourcenameinout: *mut u32) -> u32>;
pub type PRESUTIL_GET_RESOURCE_NAME_DEPENDENCY = ::core::option::Option<unsafe extern "system" fn(lpszresourcename: ::windows_sys::core::PCWSTR, lpszresourcetype: ::windows_sys::core::PCWSTR) -> HRESOURCE>;
pub type PRESUTIL_GET_RESOURCE_NAME_DEPENDENCY_EX = ::core::option::Option<unsafe extern "system" fn(lpszresourcename: ::windows_sys::core::PCWSTR, lpszresourcetype: ::windows_sys::core::PCWSTR, dwdesiredaccess: u32) -> HRESOURCE>;
pub type PRESUTIL_GET_SZ_PROPERTY = ::core::option::Option<unsafe extern "system" fn(ppszoutvalue: *mut ::windows_sys::core::PWSTR, pvaluestruct: *const CLUSPROP_SZ, pszoldvalue: ::windows_sys::core::PCWSTR, pppropertylist: *mut *mut u8, pcbpropertylistsize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_GET_SZ_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_IS_PATH_VALID = ::core::option::Option<unsafe extern "system" fn(pszpath: ::windows_sys::core::PCWSTR) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_IS_RESOURCE_CLASS_EQUAL = ::core::option::Option<unsafe extern "system" fn(prci: *mut CLUS_RESOURCE_CLASS_INFO, hresource: HRESOURCE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_PROPERTY_LIST_FROM_PARAMETER_BLOCK = ::core::option::Option<unsafe extern "system" fn(ppropertytable: *const RESUTIL_PROPERTY_ITEM, poutpropertylist: *mut ::core::ffi::c_void, pcboutpropertylistsize: *mut u32, pinparams: *const u8, pcbbytesreturned: *mut u32, pcbrequired: *mut u32) -> u32>;
pub type PRESUTIL_REMOVE_RESOURCE_SERVICE_ENVIRONMENT = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR, pfnlogevent: PLOG_EVENT_ROUTINE, hresourcehandle: isize) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_RESOURCES_EQUAL = ::core::option::Option<unsafe extern "system" fn(hself: HRESOURCE, hresource: HRESOURCE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_RESOURCE_TYPES_EQUAL = ::core::option::Option<unsafe extern "system" fn(lpszresourcetypename: ::windows_sys::core::PCWSTR, hresource: HRESOURCE) -> super::super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_BINARY_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, pbnewvalue: *const u8, cbnewvaluesize: u32, ppboutvalue: *mut *mut u8, pcboutvaluesize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_DWORD_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, dwnewvalue: u32, pdwoutvalue: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_EXPAND_SZ_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, psznewvalue: ::windows_sys::core::PCWSTR, ppszoutstring: *mut ::windows_sys::core::PWSTR) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_MULTI_SZ_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, psznewvalue: ::windows_sys::core::PCWSTR, cbnewvaluesize: u32, ppszoutvalue: *mut ::windows_sys::core::PWSTR, pcboutvaluesize: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_PRIVATE_PROPERTY_LIST = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_SET_PROPERTY_PARAMETER_BLOCK = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, reserved: *mut ::core::ffi::c_void, pinparams: *const u8, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32, poutparams: *mut u8) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_SET_PROPERTY_PARAMETER_BLOCK_EX = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, reserved: *mut ::core::ffi::c_void, pinparams: *const u8, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32, bforcewrite: super::super::Foundation::BOOL, poutparams: *mut u8) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_SET_PROPERTY_TABLE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, reserved: *const ::core::ffi::c_void, ballowunknownproperties: super::super::Foundation::BOOL, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32, poutparams: *mut u8) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_SET_PROPERTY_TABLE_EX = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, reserved: *mut ::core::ffi::c_void, ballowunknownproperties: super::super::Foundation::BOOL, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32, bforcewrite: super::super::Foundation::BOOL, poutparams: *mut u8) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_QWORD_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, qwnewvalue: u64, pqwoutvalue: *mut u64) -> u32>;
pub type PRESUTIL_SET_RESOURCE_SERVICE_ENVIRONMENT = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR, hresource: HRESOURCE, pfnlogevent: PLOG_EVENT_ROUTINE, hresourcehandle: isize) -> u32>;
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub type PRESUTIL_SET_RESOURCE_SERVICE_START_PARAMETERS = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR, schscmhandle: super::super::Security::SC_HANDLE, phservice: *mut super::super::Security::SC_HANDLE, pfnlogevent: PLOG_EVENT_ROUTINE, hresourcehandle: isize) -> u32>;
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub type PRESUTIL_SET_RESOURCE_SERVICE_START_PARAMETERS_EX = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR, schscmhandle: super::super::Security::SC_HANDLE, phservice: *mut super::super::Security::SC_HANDLE, dwdesiredaccess: u32, pfnlogevent: PLOG_EVENT_ROUTINE, hresourcehandle: isize) -> u32>;
#[doc = "Required features: `\"Win32_System_Registry\"`"]
#[cfg(feature = "Win32_System_Registry")]
pub type PRESUTIL_SET_SZ_VALUE = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, pszvaluename: ::windows_sys::core::PCWSTR, psznewvalue: ::windows_sys::core::PCWSTR, ppszoutstring: *mut ::windows_sys::core::PWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PRESUTIL_SET_UNKNOWN_PROPERTIES = ::core::option::Option<unsafe extern "system" fn(hkeyclusterkey: super::super::System::Registry::HKEY, ppropertytable: *const RESUTIL_PROPERTY_ITEM, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub type PRESUTIL_START_RESOURCE_SERVICE = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR, phservicehandle: *mut super::super::Security::SC_HANDLE) -> u32>;
pub type PRESUTIL_STOP_RESOURCE_SERVICE = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub type PRESUTIL_STOP_SERVICE = ::core::option::Option<unsafe extern "system" fn(hservicehandle: super::super::Security::SC_HANDLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_TERMINATE_SERVICE_PROCESS_FROM_RES_DLL = ::core::option::Option<unsafe extern "system" fn(dwservicepid: u32, boffline: super::super::Foundation::BOOL, pdwresourcestate: *mut u32, pfnlogevent: PLOG_EVENT_ROUTINE, hresourcehandle: isize) -> u32>;
pub type PRESUTIL_VERIFY_PRIVATE_PROPERTY_LIST = ::core::option::Option<unsafe extern "system" fn(pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PRESUTIL_VERIFY_PROPERTY_TABLE = ::core::option::Option<unsafe extern "system" fn(ppropertytable: *const RESUTIL_PROPERTY_ITEM, reserved: *const ::core::ffi::c_void, ballowunknownproperties: super::super::Foundation::BOOL, pinpropertylist: *const ::core::ffi::c_void, cbinpropertylistsize: u32, poutparams: *mut u8) -> u32>;
pub type PRESUTIL_VERIFY_RESOURCE_SERVICE = ::core::option::Option<unsafe extern "system" fn(pszservicename: ::windows_sys::core::PCWSTR) -> u32>;
#[doc = "Required features: `\"Win32_Security\"`"]
#[cfg(feature = "Win32_Security")]
pub type PRESUTIL_VERIFY_SERVICE = ::core::option::Option<unsafe extern "system" fn(hservicehandle: super::super::Security::SC_HANDLE) -> u32>;
pub type PRES_UTIL_VERIFY_SHUTDOWN_SAFE = ::core::option::Option<unsafe extern "system" fn(flags: u32, reason: u32, presult: *mut u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSET_INTERNAL_STATE = ::core::option::Option<unsafe extern "system" fn(param0: isize, statetype: CLUSTER_RESOURCE_APPLICATION_STATE, active: super::super::Foundation::BOOL) -> u32>;
pub type PSET_RESOURCE_INMEMORY_NODELOCAL_PROPERTIES_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, propertylistbuffer: *const u8, propertylistbuffersize: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSET_RESOURCE_LOCKED_MODE_EX_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, lockedmodeenabled: super::super::Foundation::BOOL, lockedmodereason: u32, lockedmodeflags: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSET_RESOURCE_LOCKED_MODE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, lockedmodeenabled: super::super::Foundation::BOOL, lockedmodereason: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSET_RESOURCE_STATUS_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, resourcestatus: *mut RESOURCE_STATUS) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PSET_RESOURCE_STATUS_ROUTINE_EX = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, resourcestatus: *mut RESOURCE_STATUS_EX) -> u32>;
pub type PSET_RESOURCE_WPR_POLICY_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, wprpolicyflags: u32) -> u32>;
pub type PSIGNAL_FAILURE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcehandle: isize, failuretype: FAILURE_TYPE, applicationspecificerrorcode: u32) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PSTARTUP_EX_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcetype: ::windows_sys::core::PCWSTR, minversionsupported: u32, maxversionsupported: u32, monitorcallbackfunctions: *mut CLRES_CALLBACK_FUNCTION_TABLE, resourcedllinterfacefunctions: *mut *mut CLRES_FUNCTION_TABLE) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_System_Registry\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_System_Registry"))]
pub type PSTARTUP_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resourcetype: ::windows_sys::core::PCWSTR, minversionsupported: u32, maxversionsupported: u32, setresourcestatus: PSET_RESOURCE_STATUS_ROUTINE, logevent: PLOG_EVENT_ROUTINE, functiontable: *mut *mut CLRES_FUNCTION_TABLE) -> u32>;
pub type PTERMINATE_ROUTINE = ::core::option::Option<unsafe extern "system" fn(resource: *mut ::core::ffi::c_void) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type PWORKER_START_ROUTINE = ::core::option::Option<unsafe extern "system" fn(pworker: *mut CLUS_WORKER, lpthreadparameter: *mut ::core::ffi::c_void) -> u32>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type SET_APP_INSTANCE_CSV_FLAGS = ::core::option::Option<unsafe extern "system" fn(processhandle: super::super::Foundation::HANDLE, mask: u32, flags: u32) -> u32>;
