#![cfg(test)]

use super::*;
use crate::history_cell::HistoryCell;

#[test]
fn nfs_migration_warning() {
    let cell = StartupWarningsCell::new(vec![NFS_MIGRATION_WARNING.to_string()]);
    let lines = cell.transcript_lines(/*width*/ 80);
    let text = lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(text);
}
