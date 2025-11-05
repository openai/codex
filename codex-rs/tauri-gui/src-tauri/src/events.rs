// Event types for the Tauri application
// These will be used in v1.3.0 for real-time event communication

#![allow(dead_code)] // These types are defined for future use

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangedEvent {
    pub file_path: String,
    pub change_type: String,
    pub lines_added: i32,
    pub lines_removed: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub title: String,
    pub body: String,
    pub level: String, // "info", "warning", "error"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintProgressEvent {
    pub blueprint_id: String,
    pub status: String,
    pub progress: f32,
    pub message: String,
}

/// Event names used throughout the application
pub mod event_names {
    pub const FILE_CHANGED: &str = "file:changed";
    pub const NOTIFICATION: &str = "notification";
    pub const BLUEPRINT_PROGRESS: &str = "blueprint:progress";
    pub const NAVIGATE: &str = "navigate";
}
