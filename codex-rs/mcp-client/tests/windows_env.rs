#![cfg(target_os = "windows")]

use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;

#[test]
fn passes_through_msvc_and_sdk_vars() {
    let mut base = HashMap::new();
    for key in [
        "COMSPEC",
        "SYSTEMROOT",
        "APPDATA",
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "VSINSTALLDIR",
        "VCINSTALLDIR",
        "VCToolsInstallDir",
        "WindowsSdkDir",
        "WindowsSDKVersion",
        "FrameworkDir",
        "FrameworkVersion",
        "VSCMD_VER",
        "VSCMD_ARG_TGT_ARCH",
    ] {
        base.insert(OsString::from(key), OsString::from("VAL"));
    }

    let cmd = codex_mcp_client::build_test_command(&base);

    for key in [
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "VSINSTALLDIR",
        "WindowsSdkDir",
        "FrameworkDir",
        "VSCMD_VER",
    ] {
        assert!(
            cmd.get_envs()
                .any(|(name, value)| name == OsStr::new(key) && value.is_some()),
            "expected {key} in env"
        );
    }
}
