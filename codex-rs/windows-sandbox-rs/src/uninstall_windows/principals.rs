//! Disables sandbox accounts for cleanup and removes sandbox principals.
//! Callers must restore original flags if preparation for cleanup fails.

use std::ptr::null;

use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use windows_sys::Win32::NetworkManagement::NetManagement as network;

use crate::setup::OFFLINE_USERNAME;
use crate::setup::ONLINE_USERNAME;
use crate::winutil::local_user_flags;
use crate::winutil::resolve_sid;
use crate::winutil::set_local_user_flags;
use crate::winutil::to_wide;

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
