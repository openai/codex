//! Pins only cleanup-owned login sessions; every other sandbox login still blocks cleanup.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::OwnedHandle;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LUID;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::TOKEN_STATISTICS;
use windows_sys::Win32::Security::TokenStatistics;

use super::principals::DisabledSandboxUsers;

#[derive(Default)]
pub(super) struct RetainedLogons {
    // Keeping each token open prevents its AuthenticationId from being recycled.
    _tokens: Vec<OwnedHandle>,
    ids: Vec<LUID>,
}

impl RetainedLogons {
    pub(super) fn capture(
        tokens: &[BorrowedHandle<'_>],
        users: &DisabledSandboxUsers,
    ) -> Result<Self> {
        ensure!(tokens.len() <= 2, "too many retained cleanup tokens");
        let mut retained = Self::default();
        let mut seen_sids = Vec::new();
        for token in tokens {
            let token = token.try_clone_to_owned()?;
            let handle = token.as_raw_handle() as HANDLE;
            let sid = unsafe { crate::token::get_user_sid_bytes(handle) }?;
            ensure!(
                users.sids().any(|candidate| candidate == sid.as_slice())
                    && !seen_sids.contains(&sid),
                "cleanup token does not identify a distinct managed sandbox account"
            );
            let mut stats: TOKEN_STATISTICS = unsafe { std::mem::zeroed() };
            let mut returned = 0;
            if unsafe {
                GetTokenInformation(
                    handle,
                    TokenStatistics,
                    (&mut stats as *mut TOKEN_STATISTICS).cast(),
                    std::mem::size_of::<TOKEN_STATISTICS>() as u32,
                    &mut returned,
                )
            } == 0
            {
                return Err(io::Error::last_os_error()).context("identify retained cleanup login");
            }
            ensure!(
                returned as usize == std::mem::size_of::<TOKEN_STATISTICS>(),
                "unexpected cleanup token statistics length"
            );
            seen_sids.push(sid);
            retained.ids.push(stats.AuthenticationId);
            retained._tokens.push(token);
        }
        Ok(retained)
    }

    pub(super) fn contains(&self, id: LUID) -> bool {
        self.ids
            .iter()
            .any(|held| held.LowPart == id.LowPart && held.HighPart == id.HighPart)
    }
}

#[cfg(test)]
#[path = "retained_logons_tests.rs"]
mod tests;
