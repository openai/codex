//! Mandatory pathname and network protections for privileged daemon sockets.
//! Emit these after all grants, including implicit platform access.

use super::SeatbeltPreparationError;
use std::path::Path;

pub(super) fn protection_policy(directory: &Path) -> Result<String, SeatbeltPreparationError> {
    let quoted = serde_json::to_string(&directory.to_string_lossy())
        .map_err(|error| SeatbeltPreparationError::FileSystem(error.to_string()))?;
    let mut rules = vec![format!(
        "(deny file-read* file-write* (literal {quoted}) (subpath {quoted}))\n\
         (deny network-outbound (remote unix-socket (subpath {quoted})))"
    )];
    // Moving an ancestor would relocate the entire protected subtree beyond
    // both pathname rules. Only unlink is denied; sibling writes still work.
    for ancestor in directory.ancestors().skip(/*n*/ 1) {
        let quoted = serde_json::to_string(&ancestor.to_string_lossy())
            .map_err(|error| SeatbeltPreparationError::FileSystem(error.to_string()))?;
        rules.push(format!(
            "(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal {quoted})))"
        ));
    }
    Ok(rules.join("\n"))
}
