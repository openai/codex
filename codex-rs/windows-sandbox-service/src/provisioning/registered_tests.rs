//! Checks which credential failures may trigger a full setup repair.

use super::credentials_need_repair;
use anyhow::anyhow;
use codex_windows_sandbox::SandboxAccountCredentialMismatch;
use pretty_assertions::assert_eq;

#[test]
fn only_password_failures_on_owned_accounts_allow_repair() {
    enum Logon {
        Healthy,
        Password,
        Policy,
    }
    use Logon::Healthy;
    use Logon::Password;
    use Logon::Policy;
    for (errors, ownership_ok, expected, ownership_checked) in [
        ([Healthy, Healthy], true, Ok(false), false),
        ([Password, Healthy], true, Ok(true), true),
        ([Healthy, Password], true, Ok(true), true),
        ([Password, Policy], true, Err("policy"), false),
        ([Policy, Password], true, Err("policy"), false),
        ([Password, Healthy], false, Err("replacement"), true),
    ] {
        let mut errors = errors.into_iter();
        let mut checked = false;
        let result = credentials_need_repair(
            |_| match errors.next().unwrap() {
                Healthy => Ok(()),
                Password => Err(anyhow!(SandboxAccountCredentialMismatch)),
                Policy => Err(anyhow!("policy")),
            },
            || {
                checked = true;
                anyhow::ensure!(ownership_ok, "replacement");
                Ok(())
            },
        )
        .map_err(|error| error.to_string());
        assert_eq!(
            (result, checked),
            (expected.map_err(str::to_owned), ownership_checked)
        );
    }
}
