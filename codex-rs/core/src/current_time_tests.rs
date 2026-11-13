#![cfg(test)]

use super::*;

#[test]
fn explicit_external_time_requires_a_provider() {
    let config = CurrentTimeReminderConfig {
        clock_source: CurrentTimeSource::External,
        ..Default::default()
    };
    let err = resolve_time_provider(Some(&config), /*external_provider*/ None)
        .err()
        .expect("missing provider");
    assert!(
        err.to_string()
            .contains("no external current-time provider")
    );
    let provider: Arc<dyn TimeProvider> = Arc::new(SystemTimeProvider);
    let selected = resolve_time_provider(Some(&config), Some(Arc::clone(&provider))).unwrap();
    assert!(Arc::ptr_eq(&selected, &provider));
    assert!(resolve_time_provider(/*config*/ None, /*external_provider*/ None).is_ok());
}
