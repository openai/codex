use http_body::{Body, Frame, SizeHint};
use std::pin::Pin;
use std::task::{Context, Poll};

struct Mock {
    size_hint: SizeHint,
}

impl Body for Mock {
    type Data = ::std::io::Cursor<Vec<u8>>;
    type Error = ();

    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(None)
    }

    fn size_hint(&self) -> SizeHint {
        self.size_hint.clone()
    }
}

#[test]
fn is_end_stream_true() {
    let combos = [
        (None, None, false),
        (Some(123), None, false),
        (Some(0), Some(123), false),
        (Some(123), Some(123), false),
        (Some(0), Some(0), false),
    ];

    for &(lower, upper, is_end_stream) in &combos {
        let mut size_hint = SizeHint::new();
        assert_eq!(0, size_hint.lower());
        assert!(size_hint.upper().is_none());

        if let Some(lower) = lower {
            size_hint.set_lower(lower);
        }

        if let Some(upper) = upper {
            size_hint.set_upper(upper);
        }

        let mut mock = Mock { size_hint };

        assert_eq!(
            is_end_stream,
            Pin::new(&mut mock).is_end_stream(),
            "size_hint = {:?}",
            mock.size_hint.clone()
        );
    }
}

#[test]
fn is_end_stream_default_false() {
    let mut mock = Mock {
        size_hint: SizeHint::default(),
    };

    assert!(
        !Pin::new(&mut mock).is_end_stream(),
        "size_hint = {:?}",
        mock.size_hint.clone()
    );
}
