//! Disables sandbox accounts for cleanup and removes their profiles and principals.
//! Callers must restore original flags if preparation for cleanup fails.

use std::ptr::null;

use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::NetworkManagement::NetManagement as network;
use windows_sys::Win32::System::Registry as registry;
use windows_sys::Win32::UI::Shell::DeleteProfileW;

use crate::setup::OFFLINE_USERNAME;
use crate::setup::ONLINE_USERNAME;
use crate::winutil::local_user_flags;
use crate::winutil::resolve_sid;
use crate::winutil::set_local_user_flags;
use crate::winutil::string_from_sid_bytes;
use crate::winutil::to_wide;

use super::retained_logons::RetainedLogons;

#[derive(Default)]
pub(super) struct DisabledSandboxUsers {
    users: Vec<SandboxUser>,
}

struct SandboxUser {
    name: &'static str,
    original_flags: u32,
    sid: Vec<u8>,
}

impl DisabledSandboxUsers {
    pub(super) fn disable(&mut self) -> Result<()> {
        for name in [OFFLINE_USERNAME, ONLINE_USERNAME] {
            let Some(original_flags) = local_user_flags(name)? else {
                continue;
            };
            let sid = resolve_sid(name)?;

            self.users.push(SandboxUser {
                name,
                original_flags,
                sid,
            });
            set_local_user_flags(name, original_flags | network::UF_ACCOUNTDISABLE)?;
        }
        Ok(())
    }

    pub(super) fn sids(&self) -> impl Iterator<Item = &[u8]> {
        self.users.iter().map(|user| user.sid.as_slice())
    }

    pub(super) fn validate_current(&self) -> Result<()> {
        for name in [OFFLINE_USERNAME, ONLINE_USERNAME] {
            let Some(flags) = local_user_flags(name)? else {
                continue;
            };
            let captured = self.users.iter().find(|user| user.name == name);
            ensure!(
                captured.is_some_and(|user| resolve_sid(name).is_ok_and(|sid| sid == user.sid))
                    && flags & network::UF_ACCOUNTDISABLE != 0,
                "sandbox account changed after being disabled: {name}"
            );
        }
        Ok(())
    }

    pub(super) fn restore(&self) -> Result<()> {
        let mut errors = Vec::new();
        for user in &self.users {
            if let Err(error) = set_local_user_flags(user.name, user.original_flags)
                && error
                    .downcast_ref::<std::io::Error>()
                    .and_then(std::io::Error::raw_os_error)
                    != Some(network::NERR_UserNotFound as i32)
            {
                errors.push(format!("{error:#}"));
            }
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("; "));
        }
        Ok(())
    }

    pub(super) fn remove_users(
        &self,
        retained: &RetainedLogons,
        report: impl Fn(&str),
    ) -> Result<()> {
        let mut errors = Vec::new();
        for user in &self.users {
            if retained.contains_sid(&user.sid) {
                report(&format!(
                    "deferring {} profile and account to runtime cleanup",
                    user.name
                ));
                continue;
            }
            match user.remove() {
                Ok(()) => report(&format!("removed {} profile and account", user.name)),
                Err(error) => {
                    report(&format!(
                        "remove {} profile and account: failed, {error:#}",
                        user.name
                    ));
                    errors.push(format!("{error:#}"));
                }
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

impl SandboxUser {
    fn remove(&self) -> Result<()> {
        let Self { name, sid, .. } = self;
        let sid = string_from_sid_bytes(sid).map_err(anyhow::Error::msg)?;
        let profile_key = to_wide(format!(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"
        ));
        let mut key = 0;
        let status = unsafe {
            registry::RegOpenKeyExW(
                registry::HKEY_LOCAL_MACHINE,
                profile_key.as_ptr(),
                /*uloptions*/ 0,
                registry::KEY_READ | registry::KEY_WOW64_64KEY,
                &mut key,
            )
        };
        match status {
            ERROR_SUCCESS => {
                unsafe { registry::RegCloseKey(key) };
                // Keep the account's SID available for retry if its profile is still loaded.
                if unsafe { DeleteProfileW(to_wide(sid).as_ptr(), null(), null()) } == 0 {
                    bail!(
                        "remove sandbox profile for {name}: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
            ERROR_FILE_NOT_FOUND => {}
            status => bail!("open sandbox profile for {name}: {status}"),
        }
        remove_sandbox_principal(name)
    }
}

#[cfg(test)]
impl DisabledSandboxUsers {
    // In-memory admission fixture only; this does not resolve or change a local account.
    pub(super) fn for_token_test(sid: Vec<u8>) -> Self {
        Self {
            users: vec![SandboxUser {
                name: OFFLINE_USERNAME,
                original_flags: network::UF_ACCOUNTDISABLE,
                sid,
            }],
        }
    }
}

pub(super) fn remove_sandbox_principal(name: &str) -> Result<()> {
    let name_wide = to_wide(name);
    let status = if name == "CodexSandboxUsers" {
        unsafe { network::NetLocalGroupDel(null(), name_wide.as_ptr()) }
    } else {
        unsafe { network::NetUserDel(null(), name_wide.as_ptr()) }
    };
    match status {
        network::NERR_Success | network::NERR_GroupNotFound | network::NERR_UserNotFound => Ok(()),
        status => bail!("remove local sandbox principal {name}: {status}"),
    }
}

#[cfg(test)]
#[path = "principals_tests.rs"]
mod tests;
