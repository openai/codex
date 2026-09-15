//! The registered-Core setup transaction: admit, provision, register, then publish readiness.

use std::sync::atomic::AtomicBool;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::SandboxProvisioningResponse;

use crate::ipc::ProvisioningRequest;
use codex_windows_sandbox::string_from_sid_bytes;
use windows::ApplicationModel::Package;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_ACCOUNTDISABLE;

use crate::installation_record::InstallationRecord;
use crate::installation_record::RuntimeRegistration;
use crate::ipc::ClientIdentity;
use crate::ipc::OwnedHandle;

pub(super) fn run(
    identity: &ClientIdentity,
    request: ProvisioningRequest,
    sandbox_sid: &[u8],
    shutdown: &AtomicBool,
    on_authenticated_user: &dyn Fn(
        InstallationRecord,
        OwnedHandle,
        codex_windows_sandbox::SetupRuntime,
    ) -> Result<InstallationRecord>,
) -> Result<SandboxProvisioningResponse> {
    // Keep admission, account provisioning and package registration in one transaction.
    let _setup_lock = codex_windows_sandbox::acquire_sandbox_setup_lock(/*timeout_ms*/ 5_000)?;
    ensure!(
        codex_windows_sandbox::resolve_sid(codex_windows_sandbox::SANDBOX_USERS_GROUP)
            .is_ok_and(|current| current == sandbox_sid),
        codex_windows_sandbox::SANDBOX_GROUP_CHANGED
    );
    let settings = request.settings;
    let mut setup_complete = super::setup_is_complete(identity, &settings)?;
    if request.refresh_only {
        // Startup may update registrations, but must never create accounts or repair teardown.
        // Check under the setup lock, before changing ownership or sandbox accounts.
        let previous = crate::installation_record::load_runtime()?
            .context("registration refresh requires completed setup")?;
        let runtime = previous.runtime()?;
        // Windows may restart this service during registration, after readiness was
        // revoked. Resume only the same provisioned accounts; never repair setup.
        ensure!(
            previous.user_sid == identity.user_sid
                && previous.codex_home == identity.codex_home
                && crate::installation_record::is_current_package_family(&previous)?
                && runtime.can_resume_registration()
                && setup_complete,
            "registration refresh requires unchanged sandbox ownership and settings"
        );
        for entry in &runtime.accounts {
            let account = entry.account.username();
            ensure!(
                codex_windows_sandbox::local_user_flags(account)?
                    .is_some_and(|flags| flags & UF_ACCOUNTDISABLE == 0)
                    && string_from_sid_bytes(&codex_windows_sandbox::resolve_sid(account)?)
                        .map_err(anyhow::Error::msg)?
                        == entry.user_sid,
                "registration refresh cannot replace a sandbox account"
            );
        }
    }
    let mut installation = super::register_owner(identity, on_authenticated_user)?;
    let result = (|| -> Result<()> {
        let package = Package::Current()?.Id()?;
        installation.runtime.get_or_insert(RuntimeRegistration {
            package_family: package.FamilyName()?.to_string(),
            accounts: Vec::new(),
            metadata_roots: Vec::new(),
            ready_package: None,
            retiring: None,
        });
        // Persist the authenticated owner before setup can rotate shared credentials.
        crate::installation_record::save_runtime(&installation)?;
        for account in [
            codex_windows_sandbox::OFFLINE_USERNAME,
            codex_windows_sandbox::ONLINE_USERNAME,
        ] {
            setup_complete &= codex_windows_sandbox::local_user_flags(account)?
                .is_some_and(|flags| flags & UF_ACCOUNTDISABLE == 0);
        }
        if !setup_complete {
            // Account state can change outside our setup lock; refresh must never repair it.
            ensure!(
                !request.refresh_only,
                "registration refresh cannot repair sandbox setup"
            );
            // Setup resolves this name for ACL trustees; it must still name the held token's user.
            let setup_owner_sid = codex_windows_sandbox::resolve_sid(&identity.account)?;
            ensure!(
                string_from_sid_bytes(&setup_owner_sid).map_err(anyhow::Error::msg)?
                    == identity.user_sid,
                "sandbox setup account no longer matches the authenticated owner"
            );
            codex_windows_sandbox::provision_sandbox_in_process(
                &identity.codex_home,
                &identity.account,
                settings,
                identity.runtime,
            )?;
        }
        crate::registered_runtime::provision(
            identity,
            &mut installation,
            &package.FullName()?,
            shutdown,
        )
    })();
    result
        .inspect_err(|error| {
            crate::service::log_error(
                crate::service::EVENT_PROVISIONING_FAILED,
                &format!("Codex sandbox provisioning failed: {error}"),
            );
        })
        .context("registered sandbox provisioning failed")?;
    crate::service::log_information(
        crate::service::EVENT_PROVISIONING_SUCCEEDED,
        "Codex sandbox provisioning completed successfully.",
    );
    Ok(SandboxProvisioningResponse::Ok)
}
