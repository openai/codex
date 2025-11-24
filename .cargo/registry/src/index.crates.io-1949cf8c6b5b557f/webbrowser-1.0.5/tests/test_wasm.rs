#[cfg(any(target_os = "macos", target_os = "linux"))]
mod common;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod tests {
    const TEST_PLATFORM: &str = "wasm32";

    use super::common::check_request_received_using;
    use std::fs;
    use std::path::PathBuf;

    // to run this test, run it as:
    // cargo test --test test_wasm32 -- --ignored
    //
    #[ignore]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_wasm32() {
        let uri = &format!("/{TEST_PLATFORM}");
        let ipv4 = "127.0.0.1";
        check_request_received_using(uri.into(), ipv4, |url, _port| {
            // modify html to use the correct url
            let mut app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            app_dir.push("tests/test-wasm-app");
            let mut src_html = PathBuf::from(&app_dir);
            src_html.push("test.html");
            let mut dst_html = PathBuf::from(&app_dir);
            dst_html.push("pkg/test.html");
            let old_html = fs::read_to_string(&src_html).expect("failed to read test.html");
            let new_html = old_html.replace("DYNAMIC_URL_TBD", url);
            fs::write(&dst_html, new_html).expect("failed to update dst test.html");

            // ensure favicon is present
            let mut favicon = PathBuf::from(&app_dir);
            favicon.push("pkg/favicon.ico");
            fs::write(&favicon, "").expect("failed to create favicon.ico");

            // open browser
            let html_url = url.replace(uri, "/static/wasm/test.html");
            //println!("URL: {}", html_url);
            let status = webbrowser::open(&html_url);

            // check for browser run status
            status.expect("browser open failed");
        })
        .await;
    }
}
