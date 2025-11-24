use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::num::NonZeroUsize;

// https://www.ibm.com/docs/en/aix/7.3?topic=files-proc-file
//
// The 'status' file is defined as binary format below
//   uint32_t pr_flag;  /* process flags from proc struct p_flag */
//   uint32_t pr_flag2; /* process flags from proc struct p_flag2 */
//   uint32_t pr_flags; /* /proc flags */
//   uint32_t pr_nlwp;  /* number of threads in the process */
//   ...                /* Omitted */
pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    let pid = std::process::id();
    let mut file = File::open(format!("/proc/{}/status", pid)).ok()?;
    let mut buffer: [u8; 16] = [0; 16];
    // Read 4 bytes after initial 12 bytes and convert into 32-byte uint.
    file.read_exact(&mut buffer).ok()?;
    let nlwp_bytes = <[u8; 4]>::try_from(&buffer[12..16]).ok()?;
    let nlwp = unsafe { std::mem::transmute::<[u8; 4], u32>(nlwp_bytes) };
    NonZeroUsize::new(nlwp as usize)
}
