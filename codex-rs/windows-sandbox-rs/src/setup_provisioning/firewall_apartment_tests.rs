//! Verifies firewall COM access preserves a service thread's existing apartment.

use super::CLSCTX_INPROC_SERVER;
use super::CoCreateInstance;
use super::FirewallComApartment;
use super::INetFwPolicy2;
use super::NetFwPolicy2;
use pretty_assertions::assert_eq;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;
use windows::Win32::System::Com::COINIT_MULTITHREADED;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CoUninitialize;

#[test]
fn firewall_access_preserves_the_services_existing_mta() {
    std::thread::spawn(|| {
        let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        assert!(initialized.is_ok(), "initialize MTA: {initialized:?}");

        // Open the rules collection without adding, changing, or deleting a rule.
        let result = (|| -> anyhow::Result<()> {
            let _apartment = FirewallComApartment::initialize()?;
            let policy: INetFwPolicy2 =
                unsafe { CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)? };
            let _rules = unsafe { policy.Rules()? };
            Ok(())
        })();
        let apartment = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        unsafe { CoUninitialize() };

        assert_eq!(
            apartment, RPC_E_CHANGED_MODE,
            "caller's MTA must remain initialized"
        );
        result.expect("read firewall rules from the existing MTA");
    })
    .join()
    .expect("firewall COM test thread");
}
