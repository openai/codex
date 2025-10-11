use anyhow::Context;
use anyhow::Result;
use rand::Rng;
use rand::distr::Alphanumeric;
use windows::Win32::Foundation::PSID;
use windows::Win32::NetworkManagement::NetManagement::NERR_Success;
use windows::Win32::NetworkManagement::NetManagement::NetUserAdd;
use windows::Win32::NetworkManagement::NetManagement::UF_NORMAL_ACCOUNT;
use windows::Win32::NetworkManagement::NetManagement::USER_ACCOUNT_FLAGS;
use windows::Win32::NetworkManagement::NetManagement::USER_INFO_1;
use windows::Win32::NetworkManagement::NetManagement::USER_PRIV_USER;
use windows::Win32::Security::LookupAccountNameW;
use windows::Win32::Security::SID_NAME_USE;
use windows::core::PCWSTR;
use windows::core::PWSTR;

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
            let ui = USER_INFO_1 {
                usri1_name: PWSTR(uname.as_ptr() as *mut _),
                usri1_password: PWSTR(pwd.as_ptr() as *mut _),
                usri1_password_age: 0,
                usri1_priv: USER_PRIV_USER,
                usri1_home_dir: PWSTR(std::ptr::null_mut()),
                usri1_comment: PWSTR(std::ptr::null_mut()),
                usri1_flags: USER_ACCOUNT_FLAGS(UF_NORMAL_ACCOUNT),
                usri1_script_path: PWSTR(std::ptr::null_mut()),
            };
            let mut param_err: u32 = 0;
            let param_err_ptr = &mut param_err as *mut u32;
            let status = NetUserAdd(
                PCWSTR::null(),
                1,
                (&ui as *const USER_INFO_1).cast(),
                Some(param_err_ptr),
            );
            if status != NERR_Success {
                anyhow::bail!(
                    "NetUserAdd({username}) failed with status {status} param_err={param_err}"
                );
            }
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
        let mut use_type = SID_NAME_USE(0);
        let wname = widestring::U16CString::from_str(name)?;
        let account = PCWSTR(wname.as_ptr());
        let _ = LookupAccountNameW(
            PCWSTR::null(),
            account,
            PSID::default(),
            &mut sid_len,
            PWSTR::null(),
            &mut dom_len,
            &mut use_type,
        );
        let mut sid = vec![0u8; sid_len as usize];
        let mut dom = vec![0u16; dom_len as usize];
        LookupAccountNameW(
            PCWSTR::null(),
            account,
            PSID(sid.as_mut_ptr() as _),
            &mut sid_len,
            PWSTR(dom.as_mut_ptr()),
            &mut dom_len,
            &mut use_type,
        )
        .context("LookupAccountNameW(second)")?;
        Ok(sid)
    }
}

fn rand_id(n: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
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
