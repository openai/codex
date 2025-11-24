use std::io;
use std::mem::MaybeUninit;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Read data from an `AsyncRead` into an `Arc`.
///
/// This uses `Arc::new_uninit_slice` and reads into the resulting uninitialized `Arc`.
///
/// # Example
///
/// ```
/// # #[tokio::main]
/// # async fn main() -> std::io::Result<()> {
/// use tokio_util::io::read_exact_arc;
///
/// let read = tokio::io::repeat(42);
///
/// let arc = read_exact_arc(read, 4).await?;
///
/// assert_eq!(&arc[..], &[42; 4]);
/// # Ok(())
/// # }
/// ```
pub async fn read_exact_arc<R: AsyncRead>(read: R, len: usize) -> io::Result<Arc<[u8]>> {
    tokio::pin!(read);
    // TODO(MSRV 1.82): When bumping MSRV, switch to `Arc::new_uninit_slice(len)`. The following is
    // equivalent, and generates the same assembly, but works without requiring MSRV 1.82.
    let arc: Arc<[MaybeUninit<u8>]> = (0..len).map(|_| MaybeUninit::uninit()).collect();
    // TODO(MSRV future): Use `Arc::get_mut_unchecked` once it's stabilized.
    // SAFETY: We're the only owner of the `Arc`, and we keep the `Arc` valid throughout this loop
    // as we write through this reference.
    let mut buf = unsafe { &mut *(Arc::as_ptr(&arc) as *mut [MaybeUninit<u8>]) };
    while !buf.is_empty() {
        if read.read_buf(&mut buf).await? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "early eof"));
        }
    }
    // TODO(MSRV 1.82): When bumping MSRV, switch to `arc.assume_init()`. The following is
    // equivalent, and generates the same assembly, but works without requiring MSRV 1.82.
    // SAFETY: This changes `[MaybeUninit<u8>]` to `[u8]`, and we've initialized all the bytes in
    // the loop above.
    Ok(unsafe { Arc::from_raw(Arc::into_raw(arc) as *const [u8]) })
}
