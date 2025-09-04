#[cfg(test)]
const ALT_PREFIX: &str = "⌥";
#[cfg(all(not(test), target_os = "macos"))]
const ALT_PREFIX: &str = "⌥";
#[cfg(all(not(test), not(target_os = "macos")))]
const ALT_PREFIX: &str = "Alt+";

#[cfg(test)]
const CTRL_PREFIX: &str = "⌃";
#[cfg(all(not(test), target_os = "macos"))]
const CTRL_PREFIX: &str = "⌃";
#[cfg(all(not(test), not(target_os = "macos")))]
const CTRL_PREFIX: &str = "Ctrl+";

#[cfg(test)]
const SHIFT_PREFIX: &str = "⇧";
#[cfg(all(not(test), target_os = "macos"))]
const SHIFT_PREFIX: &str = "⇧";
#[cfg(all(not(test), not(target_os = "macos")))]
const SHIFT_PREFIX: &str = "Shift+";

pub(crate) fn alt_prefix() -> &'static str {
    ALT_PREFIX
}

pub(crate) fn ctrl_prefix() -> &'static str {
    CTRL_PREFIX
}

pub(crate) fn shift_prefix() -> &'static str {
    SHIFT_PREFIX
}

pub(crate) fn shortcut(prefix: &str, key: &str) -> String {
    format!("{prefix}{key}")
}
