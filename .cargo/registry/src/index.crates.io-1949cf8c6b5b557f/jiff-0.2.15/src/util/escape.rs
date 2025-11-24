/*!
Provides convenience routines for escaping raw bytes.

This was copied from `regex-automata` with a few light edits.
*/

// These were originally defined here, but they got moved to
// shared since they're needed there. We re-export them here
// because this is really where they should live, but they're
// in shared because `jiff-tzdb-static` needs it.
pub(crate) use crate::shared::util::escape::{Byte, Bytes};
