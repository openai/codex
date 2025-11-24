//! The _dirs_ crate is
//!
//! - a tiny library with a minimal API (18 public functions)
//! - that provides the platform-specific, user-accessible locations
//! - for finding and storing configuration, cache and other data
//! - on Linux, Redox, Windows (≥ Vista) and macOS.
//!
//! The library provides the location of these directories by leveraging the mechanisms defined by
//!
//! - the [XDG base directory](https://standards.freedesktop.org/basedir-spec/basedir-spec-latest.html) and the [XDG user directory](https://www.freedesktop.org/wiki/Software/xdg-user-dirs/) specifications on Linux,
//! - the [Known Folder](https://msdn.microsoft.com/en-us/library/windows/desktop/bb776911(v=vs.85).aspx) system on Windows, and
//! - the [Standard Directories](https://developer.apple.com/library/content/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html#//apple_ref/doc/uid/TP40010672-CH2-SW6) on macOS.

#![deny(missing_docs)]

use std::path::PathBuf;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win as sys;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod mac;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use mac as sys;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
use wasm as sys;

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos", target_os = "ios",
    target_arch = "wasm32"
)))]
mod lin;
#[cfg(not(any(
    target_os = "windows",
    target_os = "macos", target_os = "ios",
    target_arch = "wasm32"
)))]
use lin as sys;

/// Returns the path to the user's home directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                | Example        |
/// | ------- | -------------------- | -------------- |
/// | Linux   | `$HOME`              | /home/alice    |
/// | macOS   | `$HOME`              | /Users/Alice   |
/// | Windows | `{FOLDERID_Profile}` | C:\Users\Alice |
///
/// ### Linux and macOS:
///
/// - Use `$HOME` if it is set and not empty.
/// - If `$HOME` is not set or empty, then the function `getpwuid_r` is used to determine
///   the home directory of the current user.
/// - If `getpwuid_r` lacks an entry for the current user id or the home directory field is empty,
///   then the function returns `None`.
///
/// ### Windows:
///
/// This function retrieves the user profile folder using `SHGetKnownFolderPath`.
///
/// All the examples on this page mentioning `$HOME` use this behavior.
///
/// _Note:_ This function's behavior differs from [`std::env::home_dir`],
/// which works incorrectly on Linux, macOS and Windows.
///
/// [`std::env::home_dir`]: https://doc.rust-lang.org/std/env/fn.home_dir.html
pub fn home_dir() -> Option<PathBuf> {
    sys::home_dir()
}
/// Returns the path to the user's cache directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                               | Example                      |
/// | ------- | ----------------------------------- | ---------------------------- |
/// | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
/// | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
/// | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
pub fn cache_dir() -> Option<PathBuf> {
    sys::cache_dir()
}
/// Returns the path to the user's config directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                 | Example                                  |
/// | ------- | ------------------------------------- | ---------------------------------------- |
/// | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config                      |
/// | macOS   | `$HOME`/Library/Application Support   | /Users/Alice/Library/Application Support |
/// | Windows | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming           |
pub fn config_dir() -> Option<PathBuf> {
    sys::config_dir()
}
/// Returns the path to the user's local config directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                 | Example                                  |
/// | ------- | ------------------------------------- | ---------------------------------------- |
/// | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config                      |
/// | macOS   | `$HOME`/Library/Application Support   | /Users/Alice/Library/Application Support |
/// | Windows | `{FOLDERID_LocalAppData}`             | C:\Users\Alice\AppData\Local             |
pub fn config_local_dir() -> Option<PathBuf> {
    sys::config_local_dir()
}
/// Returns the path to the user's data directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                    | Example                                  |
/// | ------- | ---------------------------------------- | ---------------------------------------- |
/// | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
/// | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
/// | Windows | `{FOLDERID_RoamingAppData}`              | C:\Users\Alice\AppData\Roaming           |
pub fn data_dir() -> Option<PathBuf> {
    sys::data_dir()
}
/// Returns the path to the user's local data directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                    | Example                                  |
/// | ------- | ---------------------------------------- | ---------------------------------------- |
/// | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
/// | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
/// | Windows | `{FOLDERID_LocalAppData}`                | C:\Users\Alice\AppData\Local             |
pub fn data_local_dir() -> Option<PathBuf> {
    sys::data_local_dir()
}
/// Returns the path to the user's executable directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                                            | Example                |
/// | ------- | ---------------------------------------------------------------- | ---------------------- |
/// | Linux   | `$XDG_BIN_HOME` or `$XDG_DATA_HOME`/../bin or `$HOME`/.local/bin | /home/alice/.local/bin |
/// | macOS   | –                                                                | –                      |
/// | Windows | –                                                                | –                      |
pub fn executable_dir() -> Option<PathBuf> {
    sys::executable_dir()
}
/// Returns the path to the user's preference directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                 | Example                          |
/// | ------- | ------------------------------------- | -------------------------------- |
/// | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config              |
/// | macOS   | `$HOME`/Library/Preferences           | /Users/Alice/Library/Preferences |
/// | Windows | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming   |
pub fn preference_dir() -> Option<PathBuf> {
    sys::preference_dir()
}
/// Returns the path to the user's runtime directory.
///
/// The runtime directory contains transient, non-essential data (like sockets or named pipes) that
/// is expected to be cleared when the user's session ends.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value              | Example         |
/// | ------- | ------------------ | --------------- |
/// | Linux   | `$XDG_RUNTIME_DIR` | /run/user/1001/ |
/// | macOS   | –                  | –               |
/// | Windows | –                  | –               |
pub fn runtime_dir() -> Option<PathBuf> {
    sys::runtime_dir()
}
/// Returns the path to the user's state directory.
///
/// The state directory contains data that should be retained between sessions (unlike the runtime
/// directory), but may not be important/portable enough to be synchronized across machines (unlike
/// the config/preferences/data directories).
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                     | Example                  |
/// | ------- | ----------------------------------------- | ------------------------ |
/// | Linux   | `$XDG_STATE_HOME` or `$HOME`/.local/state | /home/alice/.local/state |
/// | macOS   | –                                         | –                        |
/// | Windows | –                                         | –                        |
pub fn state_dir() -> Option<PathBuf> {
    sys::state_dir()
}

/// Returns the path to the user's audio directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value              | Example              |
/// | ------- | ------------------ | -------------------- |
/// | Linux   | `XDG_MUSIC_DIR`    | /home/alice/Music    |
/// | macOS   | `$HOME`/Music      | /Users/Alice/Music   |
/// | Windows | `{FOLDERID_Music}` | C:\Users\Alice\Music |
pub fn audio_dir() -> Option<PathBuf> {
    sys::audio_dir()
}
/// Returns the path to the user's desktop directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                | Example                |
/// | ------- | -------------------- | ---------------------- |
/// | Linux   | `XDG_DESKTOP_DIR`    | /home/alice/Desktop    |
/// | macOS   | `$HOME`/Desktop      | /Users/Alice/Desktop   |
/// | Windows | `{FOLDERID_Desktop}` | C:\Users\Alice\Desktop |
pub fn desktop_dir() -> Option<PathBuf> {
    sys::desktop_dir()
}
/// Returns the path to the user's document directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                  | Example                  |
/// | ------- | ---------------------- | ------------------------ |
/// | Linux   | `XDG_DOCUMENTS_DIR`    | /home/alice/Documents    |
/// | macOS   | `$HOME`/Documents      | /Users/Alice/Documents   |
/// | Windows | `{FOLDERID_Documents}` | C:\Users\Alice\Documents |
pub fn document_dir() -> Option<PathBuf> {
    sys::document_dir()
}
/// Returns the path to the user's download directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                  | Example                  |
/// | ------- | ---------------------- | ------------------------ |
/// | Linux   | `XDG_DOWNLOAD_DIR`     | /home/alice/Downloads    |
/// | macOS   | `$HOME`/Downloads      | /Users/Alice/Downloads   |
/// | Windows | `{FOLDERID_Downloads}` | C:\Users\Alice\Downloads |
pub fn download_dir() -> Option<PathBuf> {
    sys::download_dir()
}
/// Returns the path to the user's font directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                                                | Example                        |
/// | ------- | ---------------------------------------------------- | ------------------------------ |
/// | Linux   | `$XDG_DATA_HOME`/fonts or `$HOME`/.local/share/fonts | /home/alice/.local/share/fonts |
/// | macOS   | `$HOME/Library/Fonts`                                | /Users/Alice/Library/Fonts     |
/// | Windows | –                                                    | –                              |
pub fn font_dir() -> Option<PathBuf> {
    sys::font_dir()
}
/// Returns the path to the user's picture directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                 | Example                 |
/// | ------- | --------------------- | ----------------------- |
/// | Linux   | `XDG_PICTURES_DIR`    | /home/alice/Pictures    |
/// | macOS   | `$HOME`/Pictures      | /Users/Alice/Pictures   |
/// | Windows | `{FOLDERID_Pictures}` | C:\Users\Alice\Pictures |
pub fn picture_dir() -> Option<PathBuf> {
    sys::picture_dir()
}
/// Returns the path to the user's public directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                 | Example             |
/// | ------- | --------------------- | ------------------- |
/// | Linux   | `XDG_PUBLICSHARE_DIR` | /home/alice/Public  |
/// | macOS   | `$HOME`/Public        | /Users/Alice/Public |
/// | Windows | `{FOLDERID_Public}`   | C:\Users\Public     |
pub fn public_dir() -> Option<PathBuf> {
    sys::public_dir()
}
/// Returns the path to the user's template directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value                  | Example                                                    |
/// | ------- | ---------------------- | ---------------------------------------------------------- |
/// | Linux   | `XDG_TEMPLATES_DIR`    | /home/alice/Templates                                      |
/// | macOS   | –                      | –                                                          |
/// | Windows | `{FOLDERID_Templates}` | C:\Users\Alice\AppData\Roaming\Microsoft\Windows\Templates |
pub fn template_dir() -> Option<PathBuf> {
    sys::template_dir()
}

/// Returns the path to the user's video directory.
///
/// The returned value depends on the operating system and is either a `Some`, containing a value from the following table, or a `None`.
///
/// |Platform | Value               | Example               |
/// | ------- | ------------------- | --------------------- |
/// | Linux   | `XDG_VIDEOS_DIR`    | /home/alice/Videos    |
/// | macOS   | `$HOME`/Movies      | /Users/Alice/Movies   |
/// | Windows | `{FOLDERID_Videos}` | C:\Users\Alice\Videos |
pub fn video_dir() -> Option<PathBuf> {
    sys::video_dir()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_dirs() {
        println!("home_dir:       {:?}", ::home_dir());
        println!();
        println!("cache_dir:      {:?}", ::cache_dir());
        println!("config_dir:     {:?}", ::config_dir());
        println!("data_dir:       {:?}", ::data_dir());
        println!("data_local_dir: {:?}", ::data_local_dir());
        println!("executable_dir: {:?}", ::executable_dir());
        println!("preference_dir: {:?}", ::preference_dir());
        println!("runtime_dir:    {:?}", ::runtime_dir());
        println!("state_dir:      {:?}", ::state_dir());
        println!();
        println!("audio_dir:      {:?}", ::audio_dir());
        println!("desktop_dir:    {:?}", ::desktop_dir());
        println!("document_dir:   {:?}", ::document_dir());
        println!("download_dir:   {:?}", ::download_dir());
        println!("font_dir:       {:?}", ::font_dir());
        println!("picture_dir:    {:?}", ::picture_dir());
        println!("public_dir:     {:?}", ::public_dir());
        println!("template_dir:   {:?}", ::template_dir());
        println!("video_dir:      {:?}", ::video_dir());
    }
}
