use crate::temp_user::EphemeralUser;
use anyhow::Context;
use anyhow::Result;
use windows::Win32::Foundation::HLOCAL;
use windows::Win32::Foundation::PSID;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule3;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_ACTION_BLOCK;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_ALL;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIRECTION;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule;
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::System::Com::CLSCTX_INPROC_SERVER;
use windows::Win32::System::Com::COINIT_MULTITHREADED;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CoUninitialize;
use windows::core::BSTR;
use windows::core::Interface;

pub struct OutboundBlockGuard {
    rules: INetFwPolicy2,
    name: String,
}

impl Drop for OutboundBlockGuard {
    fn drop(&mut self) {
        let _ = unsafe {
            self.rules
                .Rules()
                .and_then(|r| r.Remove(&BSTR::from(self.name.as_str())))
        };
        unsafe { CoUninitialize() };
    }
}

pub fn install_for_user(user: &EphemeralUser) -> Result<OutboundBlockGuard> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .context("CoInitializeEx")?;
    }
    unsafe {
        let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)?;
        let rules = policy.Rules()?;
        let rule: INetFwRule3 = CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)?;

        // SID → "S-1-5-21-..."
        let mut pwstr = windows::core::PWSTR::null();
        ConvertSidToStringSidW(PSID(user.sid.as_ptr() as *mut _), &mut pwstr)
            .context("ConvertSidToStringSidW")?;
        let sid_str = if pwstr.is_null() {
            String::new()
        } else {
            let mut len = 0usize;
            while *pwstr.0.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(pwstr.0, len))
        };
        if !pwstr.is_null() {
            let _ = windows::Win32::Foundation::LocalFree(HLOCAL(pwstr.0.cast()));
        }

        let rule_name = format!("codex-sbx-block-{}", user.username);
        rule.SetName(&BSTR::from(rule_name.as_str()))?;
        rule.SetDescription(&BSTR::from("Codex sandbox: block outbound"))?;
        rule.SetAction(NET_FW_ACTION_BLOCK)?;
        rule.SetDirection(NET_FW_RULE_DIRECTION(2))?; // OUTBOUND
        rule.SetEnabled(windows::Win32::Foundation::VARIANT_TRUE)?;
        rule.SetProfiles(NET_FW_PROFILE2_ALL.0)?;
        rule.SetInterfaceTypes(&BSTR::from("All"))?;
        rule.SetLocalUserAuthorizedList(&BSTR::from(sid_str.as_str()))?;

        let base: windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule = rule.cast()?;
        rules.Add(&base)?;

        Ok(OutboundBlockGuard {
            rules: policy,
            name: rule_name,
        })
    }
}
