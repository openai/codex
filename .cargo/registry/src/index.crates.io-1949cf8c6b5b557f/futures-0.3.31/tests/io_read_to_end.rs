use futures::{
    executor::block_on,
    io::{self, AsyncRead, AsyncReadExt},
    task::{Context, Poll},
};
use std::pin::Pin;

#[test]
#[should_panic(expected = "assertion failed: n <= buf.len()")]
fn issue2310() {
    struct MyRead {
        first: bool,
    }

    impl MyRead {
        fn new() -> Self {
            Self { first: false }
        }
    }

    impl AsyncRead for MyRead {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(if !self.first {
                self.first = true;
                // First iteration: return more than the buffer size
                Ok(64)
            } else {
                // Second iteration: indicate that we are done
                Ok(0)
            })
        }
    }

    struct VecWrapper {
        inner: Vec<u8>,
    }

    impl VecWrapper {
        fn new() -> Self {
            Self { inner: Vec::new() }
        }
    }

    impl Drop for VecWrapper {
        fn drop(&mut self) {
            // Observe uninitialized bytes
            println!("{:?}", &self.inner);
            // Overwrite heap contents
            for b in &mut self.inner {
                *b = 0x90;
            }
        }
    }

    block_on(async {
        let mut vec = VecWrapper::new();
        let mut read = MyRead::new();

        read.read_to_end(&mut vec.inner).await.unwrap();
    })
}
