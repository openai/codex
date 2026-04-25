use super::DeviceKeyBindingRecord;
use super::StateRuntime;
use super::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn device_key_binding_round_trips_by_key_id() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");

    let first = DeviceKeyBindingRecord {
        key_id: "dk_tpm_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        account_user_id: "account-user-a".to_string(),
        client_id: "cli_a".to_string(),
    };
    let second = DeviceKeyBindingRecord {
        key_id: "dk_tpm_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".to_string(),
        account_user_id: "account-user-b".to_string(),
        client_id: "cli_b".to_string(),
    };

    runtime
        .upsert_device_key_binding(&first)
        .await
        .expect("insert first binding");
    runtime
        .upsert_device_key_binding(&second)
        .await
        .expect("insert second binding");

    assert_eq!(
        runtime
            .get_device_key_binding(&first.key_id)
            .await
            .expect("load first binding"),
        Some(first)
    );
    assert_eq!(
        runtime
            .get_device_key_binding("dk_tpm_missing")
            .await
            .expect("load missing binding"),
        None
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}

#[tokio::test]
async fn device_key_binding_upsert_updates_existing_binding() {
    let codex_home = unique_temp_dir();
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");

    let key_id = "dk_tpm_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string();
    runtime
        .upsert_device_key_binding(&DeviceKeyBindingRecord {
            key_id: key_id.clone(),
            account_user_id: "account-user-a".to_string(),
            client_id: "cli_a".to_string(),
        })
        .await
        .expect("insert binding");
    runtime
        .upsert_device_key_binding(&DeviceKeyBindingRecord {
            key_id: key_id.clone(),
            account_user_id: "account-user-b".to_string(),
            client_id: "cli_b".to_string(),
        })
        .await
        .expect("update binding");

    assert_eq!(
        runtime
            .get_device_key_binding(&key_id)
            .await
            .expect("load updated binding"),
        Some(DeviceKeyBindingRecord {
            key_id,
            account_user_id: "account-user-b".to_string(),
            client_id: "cli_b".to_string(),
        })
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}
