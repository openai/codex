use futures_core::stream::Stream;
use std::{io, pin::Pin};
use tokio_test::{assert_ready, io::Builder, task};
use tokio_util::codec::{BytesCodec, FramedRead};

macro_rules! pin {
    ($id:ident) => {
        Pin::new(&mut $id)
    };
}

macro_rules! assert_read {
    ($e:expr, $n:expr) => {{
        let val = assert_ready!($e);
        assert_eq!(val.unwrap().unwrap(), $n);
    }};
}

#[tokio::test]
async fn return_none_after_error() {
    let mut io = FramedRead::new(
        Builder::new()
            .read(b"abcdef")
            .read_error(io::Error::new(io::ErrorKind::Other, "Resource errored out"))
            .read(b"more data")
            .build(),
        BytesCodec::new(),
    );

    let mut task = task::spawn(());

    task.enter(|cx, _| {
        assert_read!(pin!(io).poll_next(cx), b"abcdef".to_vec());
        assert!(assert_ready!(pin!(io).poll_next(cx)).unwrap().is_err());
        assert!(assert_ready!(pin!(io).poll_next(cx)).is_none());
        assert_read!(pin!(io).poll_next(cx), b"more data".to_vec());
    })
}
