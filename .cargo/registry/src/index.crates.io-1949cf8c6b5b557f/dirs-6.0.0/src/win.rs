extern crate dirs_sys;

use std::path::PathBuf;

pub fn home_dir()         -> Option<PathBuf> { dirs_sys::known_folder_profile() }

pub fn cache_dir()        -> Option<PathBuf> { data_local_dir() }
pub fn config_dir()       -> Option<PathBuf> { dirs_sys::known_folder_roaming_app_data() }
pub fn config_local_dir() -> Option<PathBuf> { dirs_sys::known_folder_local_app_data() }
pub fn data_dir()         -> Option<PathBuf> { dirs_sys::known_folder_roaming_app_data() }
pub fn data_local_dir()   -> Option<PathBuf> { dirs_sys::known_folder_local_app_data() }
pub fn executable_dir()   -> Option<PathBuf> { None }
pub fn preference_dir()   -> Option<PathBuf> { dirs_sys::known_folder_local_app_data() }
pub fn runtime_dir()      -> Option<PathBuf> { None }
pub fn state_dir()        -> Option<PathBuf> { None }

pub fn audio_dir()        -> Option<PathBuf> { dirs_sys::known_folder_music() }
pub fn desktop_dir()      -> Option<PathBuf> { dirs_sys::known_folder_desktop() }
pub fn document_dir()     -> Option<PathBuf> { dirs_sys::known_folder_documents() }
pub fn download_dir()     -> Option<PathBuf> { dirs_sys::known_folder_downloads() }
pub fn font_dir()         -> Option<PathBuf> { None }
pub fn picture_dir()      -> Option<PathBuf> { dirs_sys::known_folder_pictures() }
pub fn public_dir()       -> Option<PathBuf> { dirs_sys::known_folder_public()}
pub fn template_dir()     -> Option<PathBuf> { dirs_sys::known_folder_templates() }
pub fn video_dir()        -> Option<PathBuf> { dirs_sys::known_folder_videos() }
