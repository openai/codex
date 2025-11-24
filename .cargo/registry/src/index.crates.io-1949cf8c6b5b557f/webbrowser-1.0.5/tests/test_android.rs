mod common;

mod tests {
    const TEST_PLATFORM: &str = "android";

    use super::common::check_request_received_using;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use webbrowser::Browser;

    // to run this test, run it as:
    // cargo test --test test_android -- --ignored
    //
    // For this to run, we need ANDROID_NDK_ROOT env defined, e.g.
    // ANDROID_NDK_ROOT=$ANDROID_SDK_ROOT/ndk/22.1.7171670
    //
    #[ignore]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_android() {
        let mut app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        app_dir.push("tests/test-android-app");
        let uri = format!("/{TEST_PLATFORM}");

        check_request_received_using(uri, "127.0.0.1", |url, port| {
            // modify android app code to use the correct url
            let mut lib_rs = PathBuf::from(&app_dir);
            lib_rs.push("src/lib.rs");
            let old_code =
                fs::read_to_string(&lib_rs).expect("failed to read lib.rs for android app");
            let new_code = old_code
                .split('\n')
                .map(|s| {
                    if s.starts_with("const SERVER_URL") {
                        format!("const SERVER_URL: &str = \"{url}\";")
                    } else {
                        s.into()
                    }
                })
                .collect::<Vec<String>>()
                .join("\n");
            fs::write(&lib_rs, new_code).expect("failed to modify src/lib.rs");
            log::debug!("modified src/lib.rs to use {url}");

            // uninstall previous app version if existing
            let mut adb_cmd = Command::new("adb");
            adb_cmd.arg("uninstall").arg("rust.test_android_app");
            if let Ok(status) = adb_cmd.current_dir(&app_dir).status() {
                if status.success() {
                    log::info!("adb uninstall successful");
                } else {
                    log::error!("adb uninstall failed");
                }
            } else {
                log::error!("failed to run {adb_cmd:?}");
            }

            let adb_reverse_port = format!("tcp:{port}");
            let mut adb_cmd = Command::new("adb");
            adb_cmd
                .arg("reverse")
                .arg(&adb_reverse_port)
                .arg(&adb_reverse_port);
            assert!(
                adb_cmd.status().expect("Failed to invoke").success(),
                "Failed to run {adb_cmd:?}"
            );

            // invoke app in android
            let mut apk_run_cmd = Command::new("cargo");
            apk_run_cmd.arg("apk").arg("run");
            if let Some(target) = option_env!("ANDROID_TARGET") {
                apk_run_cmd.arg("--target").arg(target);
            }
            apk_run_cmd.arg("--no-logcat");

            let apk_run_status = apk_run_cmd.current_dir(&app_dir).status();

            // revert to the old code
            fs::write(&lib_rs, &old_code).expect("failed to modify src/lib.rs");

            // check for apk run status
            assert!(
                apk_run_status.expect("Failed to invoke").success(),
                "failed to run {apk_run_cmd:?}"
            );
        })
        .await;
    }

    #[test]
    fn test_existence_default() {
        assert!(Browser::is_available(), "should have found a browser");
    }

    #[test]
    fn test_non_existence_safari() {
        assert!(!Browser::Safari.exists(), "should not have found Safari");
    }
}
