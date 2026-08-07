use super::Gauge;
use super::snapshot;
use pretty_assertions::assert_eq;

static GUARD_GAUGE: Gauge = Gauge::new("diagnostics.tests.guard");
static REGISTERED_GAUGE: Gauge = Gauge::new("diagnostics.tests.registered");

#[test]
fn gauge_guards_follow_the_measured_lifetime() {
    let first = GUARD_GAUGE.track();
    let second = GUARD_GAUGE.track();
    let value = || {
        snapshot()
            .gauges
            .into_iter()
            .find(|gauge| gauge.name == "diagnostics.tests.guard")
            .expect("used gauge should be registered")
            .value
    };

    assert_eq!(value(), 2);
    drop(first);
    assert_eq!(value(), 1);
    drop(second);
    assert_eq!(value(), 0);
}

#[test]
fn snapshot_includes_process_memory_and_registers_gauges_once() {
    REGISTERED_GAUGE.increment();
    REGISTERED_GAUGE.increment();
    let diagnostics = snapshot();
    let registered = diagnostics
        .gauges
        .iter()
        .filter(|gauge| gauge.name == "diagnostics.tests.registered")
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.process.id, std::process::id());
    assert_eq!(registered.len(), 1);
    assert_eq!(registered[0].value, 2);
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    assert!(
        diagnostics
            .process
            .resident_memory_bytes
            .is_some_and(|bytes| bytes > 0)
    );
    #[cfg(target_os = "macos")]
    assert!(
        diagnostics
            .process
            .physical_footprint_bytes
            .is_some_and(|bytes| bytes > 0)
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(diagnostics.process.physical_footprint_bytes, None);
}
