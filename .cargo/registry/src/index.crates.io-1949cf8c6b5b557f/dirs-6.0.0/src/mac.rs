extern crate dirs_sys;

use std::path::PathBuf;

pub fn home_dir()         -> Option<PathBuf> { dirs_sys::home_dir() }

fn app_support_dir()       -> Option<PathBuf> { home_dir().map(|h| h.join("Library/Application Support")) }

pub fn cache_dir()        -> Option<PathBuf> { home_dir().map(|h| h.join("Library/Caches")) }
pub fn config_dir()       -> Option<PathBuf> { app_support_dir() }
pub fn config_local_dir() -> Option<PathBuf> { app_support_dir() }
pub fn data_dir()         -> Option<PathBuf> { app_support_dir() }
pub fn data_local_dir()   -> Option<PathBuf> { app_support_dir() }
pub fn preference_dir()   -> Option<PathBuf> { home_dir().map(|h| h.join("Library/Preferences")) }
pub fn executable_dir()   -> Option<PathBuf> { None }
pub fn runtime_dir()      -> Option<PathBuf> { None }
pub fn state_dir()        -> Option<PathBuf> { None }

pub fn audio_dir()        -> Option<PathBuf> { home_dir().map(|h| h.join("Music")) }
pub fn desktop_dir()      -> Option<PathBuf> { home_dir().map(|h| h.join("Desktop")) }
pub fn document_dir()     -> Option<PathBuf> { home_dir().map(|h| h.join("Documents")) }
pub fn download_dir()     -> Option<PathBuf> { home_dir().map(|h| h.join("Downloads")) }
pub fn font_dir()         -> Option<PathBuf> { home_dir().map(|h| h.join("Library/Fonts")) }
pub fn picture_dir()      -> Option<PathBuf> { home_dir().map(|h| h.join("Pictures")) }
pub fn public_dir()       -> Option<PathBuf> { home_dir().map(|h| h.join("Public")) }
pub fn template_dir()     -> Option<PathBuf> { None }
pub fn video_dir()        -> Option<PathBuf> { home_dir().map(|h| h.join("Movies")) }
