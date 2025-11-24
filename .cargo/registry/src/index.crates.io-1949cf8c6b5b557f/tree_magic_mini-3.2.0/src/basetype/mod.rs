//! Handles "base types" such as inode/* and text/plain
const TYPES: [&str; 5] = [
    "all/all",
    "all/allfiles",
    "inode/directory",
    "text/plain",
    "application/octet-stream",
];

pub mod check;
pub mod init;
