use anyhow::Context;
use anyhow::Result;
use rand::Rng;
use rand::distributions::Alphanumeric;
use std::ffi::OsString;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::PWSTR;
use windows::Win32::NetworkManagement::NetManagement::NetUserAdd;
use windows::Win32::NetworkManagement::NetManagement::UF_NORMAL_ACCOUNT;
use windows::Win32::NetworkManagement::NetManagement::USER_INFO_1;
use windows::Win32::NetworkManagement::NetManagement::USER_PRIV_USER;
use windows::Win32::Security::LookupAccountNameW;
use windows::Win32::Security::PSID;
use windows::Win32::Security::SidNameUse;
use windows::core::BSTR;
use windows::core::PCWSTR;

pub struct EphemeralUser {
    pub username: String,
    pub password: String,
    pub sid: Vec<u8>,
}

impl EphemeralUser {
    pub fn create() -> Result<Self> {
        let username = format!("codex-sbx-{}", rand_id(10));
        let password = strong_password();

        unsafe {
            // Create local user
            let uname = widestring::U16CString::from_str(&username)?;
            let pwd = widestring::U16CString::from_str(&password)?;
            let mut ui = USER_INFO_1 {
                usri1_name: PWSTR(uname.as_ptr() as _),
                usri1_password: PWSTR(pwd.as_ptr() as _),
                usri1_priv: USER_PRIV_USER,
                usri1_home_dir: PWSTR(std::ptr::null_mut()),
                usri1_comment: PWSTR(std::ptr::null_mut()),
                usri1_flags: UF_NORMAL_ACCOUNT,
                usri1_script_path: PWSTR(std::ptr::null_mut()),
            };
            let mut param_err: u32 = 0;
            NetUserAdd(None, 1, &mut ui as *mut _ as _, &mut param_err)
                .ok()
                .with_context(|| format!("NetUserAdd({username}) param_err={param_err}"))?;
        }

        // Resolve SID
        let sid = lookup_sid(&username).context("LookupAccountNameW")?;

        Ok(Self {
            username,
            password,
            sid,
        })
    }
}

fn lookup_sid(name: &str) -> Result<Vec<u8>> {
    unsafe {
        let mut sid_len = 0u32;
        let mut dom_len = 0u32;
        let mut use_type = SidNameUse(0);
        let wname = widestring::U16CString::from_str(name)?;
        let _ = LookupAccountNameW(
            None,
            wname.as_pwstr(),
            PSID::default(),
            &mut sid_len,
            PWSTR(std::ptr::null_mut()),
            &mut dom_len,
            &mut use_type,
        );
        let mut sid = vec![0u8; sid_len as usize];
        let mut dom = vec![0u16; dom_len as usize];
        LookupAccountNameW(
            None,
            wname.as_pwstr(),
            PSID(sid.as_mut_ptr() as _),
            &mut sid_len,
            PWSTR(dom.as_mut_ptr()),
            &mut dom_len,
            &mut use_type,
        )
        .ok()
        .context("LookupAccountNameW(second)")?;
        Ok(sid)
    }
}

fn rand_id(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}
fn strong_password() -> String {
    let mut s = String::with_capacity(24);
    s.push_str("A1!");
    s.push_str(&rand_id(21));
    s
}
