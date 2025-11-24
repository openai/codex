#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
mod tests {
    const TEST_PLATFORM: &str = "ios";

    use super::common::check_request_received_using;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Command, ExitStatus, Stdio};
    use webbrowser::Browser;

    // to run this test, run it as:
    // cargo test --test test_ios -- --ignored
    //
    // MAKE SURE: an iOS simulator instance is already running
    #[ignore]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ios() {
        let uri = format!("/{}", TEST_PLATFORM);
        let ipv4 = get_ipv4_address();

        let mut app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        app_dir.push("tests/test-ios-app");

        // build test glue code
        let mut glue_dir = PathBuf::from(&app_dir);
        glue_dir.push("testglue");
        run_cmd(&glue_dir, &["./build"]).expect("glue code build failed");

        let compile_app = || {
            run_cmd(
                &app_dir,
                &[
                    "xcrun",
                    "xcodebuild",
                    "-project",
                    "test-ios-app.xcodeproj",
                    "-configuration",
                    "Debug",
                    "-sdk",
                    "iphonesimulator",
                    "-destination",
                    "platform=iOS Simulator,name=iphone-latest",
                    "-arch",
                    if cfg!(target_arch = "aarch64") {
                        "arm64"
                    } else {
                        "x86_64"
                    },
                ],
            )
        };
        compile_app().expect("compilation warm up failed for the app");

        // invoke server
        check_request_received_using(uri, &ipv4, |url, _port| {
            // modify ios app code to use the correct url
            let mut swift_src = PathBuf::from(&app_dir);
            swift_src.push("test-ios-app/ContentView.swift");
            let old_code =
                fs::read_to_string(&swift_src).expect("failed to read ContentView.swift");
            let new_code = old_code
                .split('\n')
                .map(|s| {
                    if s.starts_with("let SERVER_URL") {
                        format!("let SERVER_URL = \"{}\"", url)
                    } else {
                        s.into()
                    }
                })
                .collect::<Vec<String>>()
                .join("\n");
            fs::write(&swift_src, new_code).expect("failed to modify ContentView.swift");
            let revert_code = || fs::write(&swift_src, &old_code).expect("failed to revert code");
            let handle_exec_result = |result: std::io::Result<ExitStatus>, err_msg: &str| {
                revert_code();
                let success = match result {
                    Ok(status) => status.success(),
                    Err(_) => false,
                };
                if !success {
                    eprintln!("{err_msg}");
                    std::process::exit(1);
                }
            };

            // build app
            let exec_result = compile_app();
            handle_exec_result(exec_result, "failed to build ios app");

            // launch app on simulator
            let exec_result = run_cmd(
                &app_dir,
                &[
                    "xcrun",
                    "simctl",
                    "install",
                    "booted",
                    "build/Debug-iphonesimulator/test-ios-app.app",
                ],
            );
            handle_exec_result(exec_result, "failed to install app on simulator");

            let exec_result = run_cmd(
                &app_dir,
                &[
                    "xcrun",
                    "simctl",
                    "launch",
                    "booted",
                    "in.rootnet.webbrowser.test-ios-app",
                ],
            );
            handle_exec_result(exec_result, "failed to launch app on simulator");

            // revert to the old code
            revert_code();
        })
        .await;
    }

    fn get_ipv4_address() -> String {
        let output = Command::new("sh")
            .arg("-c")
            .arg("ifconfig | grep 'inet ' | awk '{ print $2 }' | grep -v ^127.0.0")
            .output()
            .expect("failed to get non-local ipv4 address");
        std::str::from_utf8(&output.stdout)
            .expect("unable to parse output into utf8")
            .split('\n')
            .next()
            .expect("no ip address found")
            .into()
    }

    #[test]
    fn test_existence_default() {
        assert!(Browser::is_available(), "should have found a browser");
    }

    fn run_cmd(app_dir: &PathBuf, args: &[&str]) -> std::io::Result<ExitStatus> {
        Command::new(args[0])
            .args(&args[1..])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .current_dir(app_dir)
            .status()
    }
}
