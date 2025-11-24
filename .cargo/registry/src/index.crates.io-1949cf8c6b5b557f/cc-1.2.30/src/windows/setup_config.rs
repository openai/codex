// Copyright © 2017 winapi-rs developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// All files in the project carrying such notice may not be copied, modified, or distributed
// except according to those terms.

#![allow(bad_style)]
#![allow(unused)]

use crate::windows::{
    com::{BStr, ComPtr},
    winapi::{
        IUnknown, IUnknownVtbl, Interface, LCID, LPCOLESTR, LPCWSTR, LPFILETIME, LPSAFEARRAY,
        PULONGLONG, ULONG,
    },
    windows_sys::{CoCreateInstance, BSTR, CLSCTX_ALL, HRESULT, S_FALSE},
};

use std::{
    ffi::OsString,
    ptr::{null, null_mut},
};

// Bindings to the Setup.Configuration stuff
pub type InstanceState = u32;

pub const eNone: InstanceState = 0;
pub const eLocal: InstanceState = 1;
pub const eRegistered: InstanceState = 2;
pub const eNoRebootRequired: InstanceState = 4;
pub const eComplete: InstanceState = -1i32 as u32;

RIDL! {#[uuid(0xb41463c3, 0x8866, 0x43b5, 0xbc, 0x33, 0x2b, 0x06, 0x76, 0xf7, 0xf4, 0x2e)]
interface ISetupInstance(ISetupInstanceVtbl): IUnknown(IUnknownVtbl) {
    fn GetInstanceId(
        pbstrInstanceId: *mut BSTR,
    ) -> HRESULT,
    fn GetInstallDate(
        pInstallDate: LPFILETIME,
    ) -> HRESULT,
    fn GetInstallationName(
        pbstrInstallationName: *mut BSTR,
    ) -> HRESULT,
    fn GetInstallationPath(
        pbstrInstallationPath: *mut BSTR,
    ) -> HRESULT,
    fn GetInstallationVersion(
        pbstrInstallationVersion: *mut BSTR,
    ) -> HRESULT,
    fn GetDisplayName(
        lcid: LCID,
        pbstrDisplayName: *mut BSTR,
    ) -> HRESULT,
    fn GetDescription(
        lcid: LCID,
        pbstrDescription: *mut BSTR,
    ) -> HRESULT,
    fn ResolvePath(
        pwszRelativePath: LPCOLESTR,
        pbstrAbsolutePath: *mut BSTR,
    ) -> HRESULT,
}}

RIDL! {#[uuid(0x89143c9a, 0x05af, 0x49b0, 0xb7, 0x17, 0x72, 0xe2, 0x18, 0xa2, 0x18, 0x5c)]
interface ISetupInstance2(ISetupInstance2Vtbl): ISetupInstance(ISetupInstanceVtbl) {
    fn GetState(
        pState: *mut InstanceState,
    ) -> HRESULT,
    fn GetPackages(
        ppsaPackages: *mut LPSAFEARRAY,
    ) -> HRESULT,
    fn GetProduct(
        ppPackage: *mut *mut ISetupPackageReference,
    ) -> HRESULT,
    fn GetProductPath(
        pbstrProductPath: *mut BSTR,
    ) -> HRESULT,
}}

RIDL! {#[uuid(0x6380bcff, 0x41d3, 0x4b2e, 0x8b, 0x2e, 0xbf, 0x8a, 0x68, 0x10, 0xc8, 0x48)]
interface IEnumSetupInstances(IEnumSetupInstancesVtbl): IUnknown(IUnknownVtbl) {
    fn Next(
        celt: ULONG,
        rgelt: *mut *mut ISetupInstance,
        pceltFetched: *mut ULONG,
    ) -> HRESULT,
    fn Skip(
        celt: ULONG,
    ) -> HRESULT,
    fn Reset() -> HRESULT,
    fn Clone(
        ppenum: *mut *mut IEnumSetupInstances,
    ) -> HRESULT,
}}

RIDL! {#[uuid(0x42843719, 0xdb4c, 0x46c2, 0x8e, 0x7c, 0x64, 0xf1, 0x81, 0x6e, 0xfd, 0x5b)]
interface ISetupConfiguration(ISetupConfigurationVtbl): IUnknown(IUnknownVtbl) {
    fn EnumInstances(
        ppEnumInstances: *mut *mut IEnumSetupInstances,
    ) -> HRESULT,
    fn GetInstanceForCurrentProcess(
        ppInstance: *mut *mut ISetupInstance,
    ) -> HRESULT,
    fn GetInstanceForPath(
        wzPath: LPCWSTR,
        ppInstance: *mut *mut ISetupInstance,
    ) -> HRESULT,
}}

RIDL! {#[uuid(0x26aab78c, 0x4a60, 0x49d6, 0xaf, 0x3b, 0x3c, 0x35, 0xbc, 0x93, 0x36, 0x5d)]
interface ISetupConfiguration2(ISetupConfiguration2Vtbl):
    ISetupConfiguration(ISetupConfigurationVtbl) {
    fn EnumAllInstances(
        ppEnumInstances: *mut *mut IEnumSetupInstances,
    ) -> HRESULT,
}}

RIDL! {#[uuid(0xda8d8a16, 0xb2b6, 0x4487, 0xa2, 0xf1, 0x59, 0x4c, 0xcc, 0xcd, 0x6b, 0xf5)]
interface ISetupPackageReference(ISetupPackageReferenceVtbl): IUnknown(IUnknownVtbl) {
    fn GetId(
        pbstrId: *mut BSTR,
    ) -> HRESULT,
    fn GetVersion(
        pbstrVersion: *mut BSTR,
    ) -> HRESULT,
    fn GetChip(
        pbstrChip: *mut BSTR,
    ) -> HRESULT,
    fn GetLanguage(
        pbstrLanguage: *mut BSTR,
    ) -> HRESULT,
    fn GetBranch(
        pbstrBranch: *mut BSTR,
    ) -> HRESULT,
    fn GetType(
        pbstrType: *mut BSTR,
    ) -> HRESULT,
    fn GetUniqueId(
        pbstrUniqueId: *mut BSTR,
    ) -> HRESULT,
}}

RIDL! {#[uuid(0x42b21b78, 0x6192, 0x463e, 0x87, 0xbf, 0xd5, 0x77, 0x83, 0x8f, 0x1d, 0x5c)]
interface ISetupHelper(ISetupHelperVtbl): IUnknown(IUnknownVtbl) {
    fn ParseVersion(
        pwszVersion: LPCOLESTR,
        pullVersion: PULONGLONG,
    ) -> HRESULT,
    fn ParseVersionRange(
        pwszVersionRange: LPCOLESTR,
        pullMinVersion: PULONGLONG,
        pullMaxVersion: PULONGLONG,
    ) -> HRESULT,
}}

DEFINE_GUID! {CLSID_SetupConfiguration,
0x177f0c4a, 0x1cd3, 0x4de7, 0xa3, 0x2c, 0x71, 0xdb, 0xbb, 0x9f, 0xa3, 0x6d}

// Safe wrapper around the COM interfaces
pub struct SetupConfiguration(ComPtr<ISetupConfiguration>);

impl SetupConfiguration {
    pub fn new() -> Result<SetupConfiguration, i32> {
        let mut obj = null_mut();
        let err = unsafe {
            CoCreateInstance(
                &CLSID_SetupConfiguration,
                null_mut(),
                CLSCTX_ALL,
                &ISetupConfiguration::uuidof(),
                &mut obj,
            )
        };
        if err < 0 {
            return Err(err);
        }
        let obj = unsafe { ComPtr::from_raw(obj as *mut ISetupConfiguration) };
        Ok(SetupConfiguration(obj))
    }
    pub fn get_instance_for_current_process(&self) -> Result<SetupInstance, i32> {
        let mut obj = null_mut();
        let err = unsafe { self.0.GetInstanceForCurrentProcess(&mut obj) };
        if err < 0 {
            return Err(err);
        }
        Ok(unsafe { SetupInstance::from_raw(obj) })
    }
    pub fn enum_instances(&self) -> Result<EnumSetupInstances, i32> {
        let mut obj = null_mut();
        let err = unsafe { self.0.EnumInstances(&mut obj) };
        if err < 0 {
            return Err(err);
        }
        Ok(unsafe { EnumSetupInstances::from_raw(obj) })
    }
    pub fn enum_all_instances(&self) -> Result<EnumSetupInstances, i32> {
        let mut obj = null_mut();
        let this = self.0.cast::<ISetupConfiguration2>()?;
        let err = unsafe { this.EnumAllInstances(&mut obj) };
        if err < 0 {
            return Err(err);
        }
        Ok(unsafe { EnumSetupInstances::from_raw(obj) })
    }
}

pub struct SetupInstance(ComPtr<ISetupInstance>);

impl SetupInstance {
    pub unsafe fn from_raw(obj: *mut ISetupInstance) -> SetupInstance {
        SetupInstance(ComPtr::from_raw(obj))
    }
    pub fn instance_id(&self) -> Result<OsString, i32> {
        let mut s = null();
        let err = unsafe { self.0.GetInstanceId(&mut s) };
        let bstr = unsafe { BStr::from_raw(s) };
        if err < 0 {
            return Err(err);
        }
        Ok(bstr.to_osstring())
    }
    pub fn installation_name(&self) -> Result<OsString, i32> {
        let mut s = null();
        let err = unsafe { self.0.GetInstallationName(&mut s) };
        let bstr = unsafe { BStr::from_raw(s) };
        if err < 0 {
            return Err(err);
        }
        Ok(bstr.to_osstring())
    }
    pub fn installation_path(&self) -> Result<OsString, i32> {
        let mut s = null();
        let err = unsafe { self.0.GetInstallationPath(&mut s) };
        let bstr = unsafe { BStr::from_raw(s) };
        if err < 0 {
            return Err(err);
        }
        Ok(bstr.to_osstring())
    }
    pub fn installation_version(&self) -> Result<OsString, i32> {
        let mut s = null();
        let err = unsafe { self.0.GetInstallationVersion(&mut s) };
        let bstr = unsafe { BStr::from_raw(s) };
        if err < 0 {
            return Err(err);
        }
        Ok(bstr.to_osstring())
    }
    pub fn product_path(&self) -> Result<OsString, i32> {
        let mut s = null();
        let this = self.0.cast::<ISetupInstance2>()?;
        let err = unsafe { this.GetProductPath(&mut s) };
        let bstr = unsafe { BStr::from_raw(s) };
        if err < 0 {
            return Err(err);
        }
        Ok(bstr.to_osstring())
    }
}

pub struct EnumSetupInstances(ComPtr<IEnumSetupInstances>);

impl EnumSetupInstances {
    pub unsafe fn from_raw(obj: *mut IEnumSetupInstances) -> EnumSetupInstances {
        EnumSetupInstances(ComPtr::from_raw(obj))
    }
}

impl Iterator for EnumSetupInstances {
    type Item = Result<SetupInstance, i32>;
    fn next(&mut self) -> Option<Result<SetupInstance, i32>> {
        let mut obj = null_mut();
        let err = unsafe { self.0.Next(1, &mut obj, null_mut()) };
        if err < 0 {
            return Some(Err(err));
        }
        if err == S_FALSE {
            return None;
        }
        Some(Ok(unsafe { SetupInstance::from_raw(obj) }))
    }
}
