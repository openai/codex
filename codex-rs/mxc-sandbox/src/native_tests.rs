#![cfg(test)]

use super::create_process_error;

#[test]
fn returns_only_the_native_create_process_error() {
    let api = "CreateProcessW(PROC_THREAD_ATTRIBUTE_SECURITY_ENVIRONMENT) failed: ";
    assert_eq!(
        create_process_error(&format!("{api}WIN32_ERROR(5) (working directory: private)")),
        Some(5)
    );
    assert_eq!(
        create_process_error(&format!("{api}WIN32_ERROR(private)")),
        None
    );
    assert_eq!(
        create_process_error("another API failed: WIN32_ERROR(5)"),
        None
    );
}
