extern crate dirs_sys;

use std::env;
use std::path::PathBuf;

pub fn home_dir()         -> Option<PathBuf> { dirs_sys::home_dir() }

pub fn cache_dir()        -> Option<PathBuf> { env::var_os("XDG_CACHE_HOME") .and_then(dirs_sys::is_absolute_path).or_else(|| home_dir().map(|h| h.join(".cache"))) }
pub fn config_dir()       -> Option<PathBuf> { env::var_os("XDG_CONFIG_HOME").and_then(dirs_sys::is_absolute_path).or_else(|| home_dir().map(|h| h.join(".config"))) }
pub fn config_local_dir() -> Option<PathBuf> { config_dir() }
pub fn data_dir()         -> Option<PathBuf> { env::var_os("XDG_DATA_HOME")  .and_then(dirs_sys::is_absolute_path).or_else(|| home_dir().map(|h| h.join(".local/share"))) }
pub fn data_local_dir()   -> Option<PathBuf> { data_dir() }
pub fn preference_dir()   -> Option<PathBuf> { config_dir() }
pub fn runtime_dir()      -> Option<PathBuf> { env::var_os("XDG_RUNTIME_DIR").and_then(dirs_sys::is_absolute_path) }
pub fn state_dir()        -> Option<PathBuf> { env::var_os("XDG_STATE_HOME") .and_then(dirs_sys::is_absolute_path).or_else(|| home_dir().map(|h| h.join(".local/state"))) }
pub fn executable_dir()   -> Option<PathBuf> { env::var_os("XDG_BIN_HOME")   .and_then(dirs_sys::is_absolute_path).or_else(|| home_dir().map(|h| h.join(".local/bin"))) }

pub fn audio_dir()        -> Option<PathBuf> { dirs_sys::user_dir("MUSIC") }
pub fn desktop_dir()      -> Option<PathBuf> { dirs_sys::user_dir("DESKTOP") }
pub fn document_dir()     -> Option<PathBuf> { dirs_sys::user_dir("DOCUMENTS") }
pub fn download_dir()     -> Option<PathBuf> { dirs_sys::user_dir("DOWNLOAD") }
pub fn font_dir()         -> Option<PathBuf> { data_dir().map(|d| d.join("fonts")) }
pub fn picture_dir()      -> Option<PathBuf> { dirs_sys::user_dir("PICTURES") }
pub fn public_dir()       -> Option<PathBuf> { dirs_sys::user_dir("PUBLICSHARE") }
pub fn template_dir()     -> Option<PathBuf> { dirs_sys::user_dir("TEMPLATES") }
pub fn video_dir()        -> Option<PathBuf> { dirs_sys::user_dir("VIDEOS") }

#[cfg(test)]
mod tests {
    #[test]
    fn test_file_user_dirs_exists() {
        let user_dirs_file = ::config_dir().unwrap().join("user-dirs.dirs");
        println!("{:?} exists: {:?}", user_dirs_file, user_dirs_file.exists());
    }
}
