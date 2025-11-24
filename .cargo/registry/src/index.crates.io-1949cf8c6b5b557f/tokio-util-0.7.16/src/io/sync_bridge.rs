use std::io::{BufRead, Read, Seek, Write};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite,
    AsyncWriteExt,
};

/// Use a [`tokio::io::AsyncRead`] synchronously as a [`std::io::Read`] or
/// a [`tokio::io::AsyncWrite`] synchronously as a [`std::io::Write`].
///
/// # Alternatives
///
/// In many cases, there are better alternatives to using `SyncIoBridge`, especially
/// if you want to avoid blocking the async runtime. Consider the following scenarios:
///
/// When hashing data, using `SyncIoBridge` can lead to suboptimal performance and
/// might not fully leverage the async capabilities of the system.
///
/// ### Why It Matters:
///
/// `SyncIoBridge` allows you to use asynchronous I/O operations in an synchronous
/// context by blocking the current thread. However, this can be inefficient because:
/// - **Inefficient Resource Usage**: `SyncIoBridge` takes up an entire OS thread,
///   which is inefficient compared to asynchronous code that can multiplex many
///   tasks on a single thread.
/// - **Thread Pool Saturation**: Excessive use of `SyncIoBridge` can exhaust the
///   async runtime's thread pool, reducing the number of threads available for
///   other tasks and impacting overall performance.
/// - **Missed Concurrency Benefits**: By using synchronous operations with
///   `SyncIoBridge`, you lose the ability to interleave tasks efficiently,
///   which is a key advantage of asynchronous programming.
///
/// ## Example 1: Hashing Data
///
/// The use of `SyncIoBridge` is unnecessary when hashing data. Instead, you can
/// process the data asynchronously by reading it into memory, which avoids blocking
/// the async runtime.
///
/// There are two strategies for avoiding `SyncIoBridge` when hashing data. When
/// the data fits into memory, the easiest is to read the data into a `Vec<u8>`
/// and hash it:
///
/// Explanation: This example demonstrates how to asynchronously read data from a
/// reader into memory and hash it using a synchronous hashing function. The
/// `SyncIoBridge` is avoided, ensuring that the async runtime is not blocked.
/// ```rust
/// use tokio::io::AsyncReadExt;
/// use tokio::io::AsyncRead;
/// use std::io::Cursor;
/// # mod blake3 { pub fn hash(_: &[u8]) {} }
///
/// async fn hash_contents(mut reader: impl AsyncRead + Unpin) -> Result<(), std::io::Error> {
///    // Read all data from the reader into a Vec<u8>.
///    let mut data = Vec::new();
///    reader.read_to_end(&mut data).await?;
///
///    // Hash the data using the blake3 hashing function.
///    let hash = blake3::hash(&data);
///
///    Ok(hash)
///}
///
/// #[tokio::main]
/// async fn main() -> Result<(), std::io::Error> {
///     // Example: In-memory data.
///     let data = b"Hello, world!"; // A byte slice.
///     let reader = Cursor::new(data); // Create an in-memory AsyncRead.
///     hash_contents(reader).await
/// }
/// ```
///
/// When the data doesn't fit into memory, the hashing library will usually
/// provide a `hasher` that you can repeatedly call `update` on to hash the data
/// one chunk at the time.
///
/// Explanation: This example demonstrates how to asynchronously stream data in
/// chunks for hashing. Each chunk is read asynchronously, and the hash is updated
/// incrementally. This avoids blocking and improves performance over using
/// `SyncIoBridge`.
///
/// ```rust
/// use tokio::io::AsyncReadExt;
/// use tokio::io::AsyncRead;
/// use std::io::Cursor;
/// # struct Hasher;
/// # impl Hasher { pub fn update(&mut self, _: &[u8]) {} pub fn finalize(&self) {} }
///
/// /// Asynchronously streams data from an async reader, processes it in chunks,
/// /// and hashes the data incrementally.
/// async fn hash_stream(mut reader: impl AsyncRead + Unpin, mut hasher: Hasher) -> Result<(), std::io::Error> {
///    // Create a buffer to read data into, sized for performance.
///    let mut data = vec![0; 64 * 1024];
///    loop {
///        // Read data from the reader into the buffer.
///        let len = reader.read(&mut data).await?;
///        if len == 0 { break; } // Exit loop if no more data.
///
///        // Update the hash with the data read.
///        hasher.update(&data[..len]);
///    }
///
///    // Finalize the hash after all data has been processed.
///    let hash = hasher.finalize();
///
///    Ok(hash)
///}
///
/// #[tokio::main]
/// async fn main() -> Result<(), std::io::Error> {
///     // Example: In-memory data.
///     let data = b"Hello, world!"; // A byte slice.
///     let reader = Cursor::new(data); // Create an in-memory AsyncRead.
///     let hasher = Hasher;
///     hash_stream(reader, hasher).await
/// }
/// ```
///
///
/// ## Example 2: Compressing Data
///
/// When compressing data, the use of `SyncIoBridge` is unnecessary as it introduces
/// blocking and inefficient code. Instead, you can utilize an async compression library
/// such as the [`async-compression`](https://docs.rs/async-compression/latest/async_compression/)
/// crate, which is built to handle asynchronous data streams efficiently.
///
/// Explanation: This example shows how to asynchronously compress data using an
/// async compression library. By reading and writing asynchronously, it avoids
/// blocking and is more efficient than using `SyncIoBridge` with a non-async
/// compression library.
///
/// ```ignore
/// use async_compression::tokio::write::GzipEncoder;
/// use std::io::Cursor;
/// use tokio::io::AsyncRead;
///
/// /// Asynchronously compresses data from an async reader using Gzip and an async encoder.
/// async fn compress_data(mut reader: impl AsyncRead + Unpin) -> Result<(), std::io::Error> {
///    let writer = tokio::io::sink();
///
///    // Create a Gzip encoder that wraps the writer.
///    let mut encoder = GzipEncoder::new(writer);
///
///    // Copy data from the reader to the encoder, compressing it.
///    tokio::io::copy(&mut reader, &mut encoder).await?;
///
///    Ok(())
///}
///
/// #[tokio::main]
/// async fn main() -> Result<(), std::io::Error> {
///     // Example: In-memory data.
///     let data = b"Hello, world!"; // A byte slice.
///     let reader = Cursor::new(data); // Create an in-memory AsyncRead.
///     compress_data(reader).await?;
///
///   Ok(())
/// }
/// ```
///
///
/// ## Example 3: Parsing Data Formats
///
///
/// `SyncIoBridge` is not ideal when parsing data formats such as `JSON`, as it
/// blocks async operations. A more efficient approach is to read data asynchronously
/// into memory and then `deserialize` it, avoiding unnecessary synchronization overhead.
///
/// Explanation: This example shows how to asynchronously read data into memory
/// and then parse it as `JSON`. By avoiding `SyncIoBridge`, the asynchronous runtime
/// remains unblocked, leading to better performance when working with asynchronous
/// I/O streams.
///
/// ```rust,no_run
/// use tokio::io::AsyncRead;
/// use tokio::io::AsyncReadExt;
/// use std::io::Cursor;
/// # mod serde {
/// #     pub trait DeserializeOwned: 'static {}
/// #     impl<T: 'static> DeserializeOwned for T {}
/// # }
/// # mod serde_json {
/// #     use super::serde::DeserializeOwned;
/// #     pub fn from_slice<T: DeserializeOwned>(_: &[u8]) -> Result<T, std::io::Error> {
/// #         unimplemented!()
/// #     }
/// # }
/// # #[derive(Debug)] struct MyStruct;
///
///
/// async fn parse_json(mut reader: impl AsyncRead + Unpin) -> Result<MyStruct, std::io::Error> {
///    // Read all data from the reader into a Vec<u8>.
///    let mut data = Vec::new();
///    reader.read_to_end(&mut data).await?;
///
///    // Deserialize the data from the Vec<u8> into a MyStruct instance.
///    let value: MyStruct = serde_json::from_slice(&data)?;
///
///    Ok(value)
///}
///
/// #[tokio::main]
/// async fn main() -> Result<(), std::io::Error> {
///     // Example: In-memory data.
///     let data = b"Hello, world!"; // A byte slice.
///     let reader = Cursor::new(data); // Create an in-memory AsyncRead.
///     parse_json(reader).await?;
///     Ok(())
/// }
/// ```
///
/// ## Correct Usage of `SyncIoBridge` inside `spawn_blocking`
///
/// `SyncIoBridge` is mainly useful when you need to interface with synchronous
/// libraries from an asynchronous context.
///
/// Explanation: This example shows how to use `SyncIoBridge` inside a `spawn_blocking`
/// task to safely perform synchronous I/O without blocking the async runtime. The
/// `spawn_blocking` ensures that the synchronous code is offloaded to a dedicated
/// thread pool, preventing it from interfering with the async tasks.
///
/// ```rust
/// use tokio::task::spawn_blocking;
/// use tokio_util::io::SyncIoBridge;
/// use tokio::io::AsyncRead;
/// use std::marker::Unpin;
/// use std::io::Cursor;
///
/// /// Wraps an async reader with `SyncIoBridge` and performs synchronous I/O operations in a blocking task.
/// async fn process_sync_io(reader: impl AsyncRead + Unpin + Send + 'static) -> Result<Vec<u8>, std::io::Error> {
///    // Wrap the async reader with `SyncIoBridge` to allow synchronous reading.
///    let mut sync_reader = SyncIoBridge::new(reader);
///
///    // Spawn a blocking task to perform synchronous I/O operations.
///    let result = spawn_blocking(move || {
///        // Create an in-memory buffer to hold the copied data.
///        let mut buffer = Vec::new();
///        // Copy data from the sync_reader to the buffer.
///        std::io::copy(&mut sync_reader, &mut buffer)?;
///        // Return the buffer containing the copied data.
///        Ok::<_, std::io::Error>(buffer)
///    })
///    .await??;
///
///    // Return the result from the blocking task.
///    Ok(result)
///}
///
/// #[tokio::main]
/// async fn main() -> Result<(), std::io::Error> {
///     // Example: In-memory data.
///     let data = b"Hello, world!"; // A byte slice.
///     let reader = Cursor::new(data); // Create an in-memory AsyncRead.
///     let result = process_sync_io(reader).await?;
///
///     // You can use `result` here as needed.
///
///     Ok(())
/// }
/// ```
///
#[derive(Debug)]
pub struct SyncIoBridge<T> {
    src: T,
    rt: tokio::runtime::Handle,
}

impl<T: AsyncBufRead + Unpin> BufRead for SyncIoBridge<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let src = &mut self.src;
        self.rt.block_on(AsyncBufReadExt::fill_buf(src))
    }

    fn consume(&mut self, amt: usize) {
        let src = &mut self.src;
        AsyncBufReadExt::consume(src, amt)
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt
            .block_on(AsyncBufReadExt::read_until(src, byte, buf))
    }
    fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt.block_on(AsyncBufReadExt::read_line(src, buf))
    }
}

impl<T: AsyncRead + Unpin> Read for SyncIoBridge<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt.block_on(AsyncReadExt::read(src, buf))
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt.block_on(src.read_to_end(buf))
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt.block_on(src.read_to_string(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        let src = &mut self.src;
        // The AsyncRead trait returns the count, synchronous doesn't.
        let _n = self.rt.block_on(src.read_exact(buf))?;
        Ok(())
    }
}

impl<T: AsyncWrite + Unpin> Write for SyncIoBridge<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt.block_on(src.write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let src = &mut self.src;
        self.rt.block_on(src.flush())
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let src = &mut self.src;
        self.rt.block_on(src.write_all(buf))
    }

    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        let src = &mut self.src;
        self.rt.block_on(src.write_vectored(bufs))
    }
}

impl<T: AsyncSeek + Unpin> Seek for SyncIoBridge<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let src = &mut self.src;
        self.rt.block_on(AsyncSeekExt::seek(src, pos))
    }
}

// Because https://doc.rust-lang.org/std/io/trait.Write.html#method.is_write_vectored is at the time
// of this writing still unstable, we expose this as part of a standalone method.
impl<T: AsyncWrite> SyncIoBridge<T> {
    /// Determines if the underlying [`tokio::io::AsyncWrite`] target supports efficient vectored writes.
    ///
    /// See [`tokio::io::AsyncWrite::is_write_vectored`].
    pub fn is_write_vectored(&self) -> bool {
        self.src.is_write_vectored()
    }
}

impl<T: AsyncWrite + Unpin> SyncIoBridge<T> {
    /// Shutdown this writer. This method provides a way to call the [`AsyncWriteExt::shutdown`]
    /// function of the inner [`tokio::io::AsyncWrite`] instance.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`AsyncWriteExt::shutdown`].
    ///
    /// [`AsyncWriteExt::shutdown`]: tokio::io::AsyncWriteExt::shutdown
    pub fn shutdown(&mut self) -> std::io::Result<()> {
        let src = &mut self.src;
        self.rt.block_on(src.shutdown())
    }
}

impl<T: Unpin> SyncIoBridge<T> {
    /// Use a [`tokio::io::AsyncRead`] synchronously as a [`std::io::Read`] or
    /// a [`tokio::io::AsyncWrite`] as a [`std::io::Write`].
    ///
    /// When this struct is created, it captures a handle to the current thread's runtime with [`tokio::runtime::Handle::current`].
    /// It is hence OK to move this struct into a separate thread outside the runtime, as created
    /// by e.g. [`tokio::task::spawn_blocking`].
    ///
    /// Stated even more strongly: to make use of this bridge, you *must* move
    /// it into a separate thread outside the runtime.  The synchronous I/O will use the
    /// underlying handle to block on the backing asynchronous source, via
    /// [`tokio::runtime::Handle::block_on`].  As noted in the documentation for that
    /// function, an attempt to `block_on` from an asynchronous execution context
    /// will panic.
    ///
    /// # Wrapping `!Unpin` types
    ///
    /// Use e.g. `SyncIoBridge::new(Box::pin(src))`.
    ///
    /// # Panics
    ///
    /// This will panic if called outside the context of a Tokio runtime.
    #[track_caller]
    pub fn new(src: T) -> Self {
        Self::new_with_handle(src, tokio::runtime::Handle::current())
    }

    /// Use a [`tokio::io::AsyncRead`] synchronously as a [`std::io::Read`] or
    /// a [`tokio::io::AsyncWrite`] as a [`std::io::Write`].
    ///
    /// This is the same as [`SyncIoBridge::new`], but allows passing an arbitrary handle and hence may
    /// be initially invoked outside of an asynchronous context.
    pub fn new_with_handle(src: T, rt: tokio::runtime::Handle) -> Self {
        Self { src, rt }
    }

    /// Consume this bridge, returning the underlying stream.
    pub fn into_inner(self) -> T {
        self.src
    }
}

impl<T> AsMut<T> for SyncIoBridge<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.src
    }
}

impl<T> AsRef<T> for SyncIoBridge<T> {
    fn as_ref(&self) -> &T {
        &self.src
    }
}
