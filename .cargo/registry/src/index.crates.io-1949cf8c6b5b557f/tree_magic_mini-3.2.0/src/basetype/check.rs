use crate::{read_bytes, Mime};
use std::collections::HashMap;
use std::fs::File;

pub(crate) struct BaseType;

impl crate::Checker for BaseType {
    fn match_bytes(&self, bytes: &[u8], mimetype: &str) -> bool {
        if mimetype == "application/octet-stream" || mimetype == "all/allfiles" {
            // Both of these are the case if we have a bytestream at all
            return true;
        }
        if mimetype == "text/plain" {
            is_text_plain_from_u8(bytes)
        } else {
            // ...how did we get bytes for this?
            false
        }
    }

    fn match_file(&self, file: &File, mimetype: &str) -> bool {
        // Being bad with error handling here,
        // but if you can't open it it's probably not a file.
        let Ok(meta) = file.metadata() else {
            return false;
        };

        match mimetype {
            "all/all" => true,
            "all/allfiles" | "application/octet-stream" => meta.is_file(),
            "inode/directory" => meta.is_dir(),
            "text/plain" => is_text_plain_from_file(file),
            _ => false,
        }
    }

    fn get_supported(&self) -> Vec<Mime> {
        super::init::get_supported()
    }

    fn get_subclasses(&self) -> Vec<(Mime, Mime)> {
        super::init::get_subclasses()
    }

    fn get_aliaslist(&self) -> HashMap<Mime, Mime> {
        super::init::get_aliaslist()
    }
}

/// If there are any null bytes, return False. Otherwise return True.
fn is_text_plain_from_u8(bytes: &[u8]) -> bool {
    memchr::memchr(0, bytes).is_none()
}

// TODO: Hoist the main logic here somewhere else. This'll get redundant fast!
fn is_text_plain_from_file(file: &File) -> bool {
    let Ok(bytes) = read_bytes(file, 512) else {
        return false;
    };
    is_text_plain_from_u8(&bytes)
}
