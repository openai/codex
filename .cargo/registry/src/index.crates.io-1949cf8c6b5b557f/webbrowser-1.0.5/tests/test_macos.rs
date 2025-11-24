#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
mod tests {
    const TEST_PLATFORM: &str = "macos";

    use super::common::*;
    use webbrowser::Browser;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_open_default() {
        check_browser(Browser::Default, TEST_PLATFORM).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_open_safari() {
        check_browser(Browser::Safari, TEST_PLATFORM).await;
    }

    // #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    // #[ignore]
    // async fn test_open_firefox() {
    //     check_browser(Browser::Firefox, TEST_PLATFORM).await;
    // }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn test_open_chrome() {
        check_browser(Browser::Chrome, TEST_PLATFORM).await;
    }

    #[test]
    fn test_existence_default() {
        assert!(Browser::is_available(), "should have found a browser");
    }

    #[test]
    fn test_existence_safari() {
        assert!(Browser::Safari.exists(), "should have found Safari");
    }

    #[test]
    fn test_non_existence_webpositive() {
        assert!(
            !Browser::WebPositive.exists(),
            "should not have found WebPositive",
        );
    }

    #[cfg(not(feature = "hardened"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_local_file_abs_path() {
        check_local_file(Browser::Default, None, |pb| {
            pb.as_os_str().to_string_lossy().into()
        })
        .await;
    }

    #[cfg(not(feature = "hardened"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_local_file_rel_path() {
        let cwd = std::env::current_dir().expect("unable to get current dir");
        check_local_file(Browser::Default, None, |pb| {
            pb.strip_prefix(cwd)
                .expect("strip prefix failed")
                .as_os_str()
                .to_string_lossy()
                .into()
        })
        .await;
    }

    #[cfg(not(feature = "hardened"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_local_file_uri() {
        check_local_file(Browser::Default, None, |pb| {
            url::Url::from_file_path(pb)
                .expect("failed to convert path to url")
                .to_string()
        })
        .await;
    }

    #[cfg(feature = "hardened")]
    #[test]
    fn test_hardened_mode() {
        let err = webbrowser::open("file:///etc/passwd")
            .expect_err("expected non-http url to fail in hardened mode");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
