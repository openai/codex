#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::{AsyncWrite, AsyncWriteExt};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[tokio::test]
async fn write_int_should_err_if_write_count_0() {
    struct Wr {}

    impl AsyncWrite for Wr {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Ok(0).into()
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }
    }

    let mut wr = Wr {};

    // should be ok just to test these 2, other cases actually expanded by same macro.
    assert!(wr.write_i8(0).await.is_err());
    assert!(wr.write_i32(12).await.is_err());
}
