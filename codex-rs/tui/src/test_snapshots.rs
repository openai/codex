pub(crate) fn snapshot_settings() -> insta::Settings {
    insta::Settings::clone_current()
}

pub(crate) fn with_snapshot_settings<F: FnOnce()>(callback: F) {
    snapshot_settings().bind(callback);
}
