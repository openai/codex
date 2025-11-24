use crate::io::{AsyncRead, ReadBuf};

use bytes::Buf;
use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::io::ErrorKind::UnexpectedEof;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

macro_rules! reader {
    ($name:ident, $ty:ty, $reader:ident) => {
        reader!($name, $ty, $reader, std::mem::size_of::<$ty>());
    };
    ($name:ident, $ty:ty, $reader:ident, $bytes:expr) => {
        pin_project! {
            #[doc(hidden)]
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            pub struct $name<R> {
                #[pin]
                src: R,
                buf: [u8; $bytes],
                read: u8,
                // Make this future `!Unpin` for compatibility with async trait methods.
                #[pin]
                _pin: PhantomPinned,
            }
        }

        impl<R> $name<R> {
            pub(crate) fn new(src: R) -> Self {
                $name {
                    src,
                    buf: [0; $bytes],
                    read: 0,
                    _pin: PhantomPinned,
                }
            }
        }

        impl<R> Future for $name<R>
        where
            R: AsyncRead,
        {
            type Output = io::Result<$ty>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut me = self.project();

                if *me.read == $bytes as u8 {
                    return Poll::Ready(Ok(Buf::$reader(&mut &me.buf[..])));
                }

                while *me.read < $bytes as u8 {
                    let mut buf = ReadBuf::new(&mut me.buf[*me.read as usize..]);

                    *me.read += match me.src.as_mut().poll_read(cx, &mut buf) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(())) => {
                            let n = buf.filled().len();
                            if n == 0 {
                                return Poll::Ready(Err(UnexpectedEof.into()));
                            }

                            n as u8
                        }
                    };
                }

                let num = Buf::$reader(&mut &me.buf[..]);

                Poll::Ready(Ok(num))
            }
        }
    };
}

macro_rules! reader8 {
    ($name:ident, $ty:ty) => {
        pin_project! {
            /// Future returned from `read_u8`
            #[doc(hidden)]
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            pub struct $name<R> {
                #[pin]
                reader: R,
                // Make this future `!Unpin` for compatibility with async trait methods.
                #[pin]
                _pin: PhantomPinned,
            }
        }

        impl<R> $name<R> {
            pub(crate) fn new(reader: R) -> $name<R> {
                $name {
                    reader,
                    _pin: PhantomPinned,
                }
            }
        }

        impl<R> Future for $name<R>
        where
            R: AsyncRead,
        {
            type Output = io::Result<$ty>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let me = self.project();

                let mut buf = [0; 1];
                let mut buf = ReadBuf::new(&mut buf);
                match me.reader.poll_read(cx, &mut buf) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
                    Poll::Ready(Ok(())) => {
                        if buf.filled().len() == 0 {
                            return Poll::Ready(Err(UnexpectedEof.into()));
                        }

                        Poll::Ready(Ok(buf.filled()[0] as $ty))
                    }
                }
            }
        }
    };
}

reader8!(ReadU8, u8);
reader8!(ReadI8, i8);

reader!(ReadU16, u16, get_u16);
reader!(ReadU32, u32, get_u32);
reader!(ReadU64, u64, get_u64);
reader!(ReadU128, u128, get_u128);

reader!(ReadI16, i16, get_i16);
reader!(ReadI32, i32, get_i32);
reader!(ReadI64, i64, get_i64);
reader!(ReadI128, i128, get_i128);

reader!(ReadF32, f32, get_f32);
reader!(ReadF64, f64, get_f64);

reader!(ReadU16Le, u16, get_u16_le);
reader!(ReadU32Le, u32, get_u32_le);
reader!(ReadU64Le, u64, get_u64_le);
reader!(ReadU128Le, u128, get_u128_le);

reader!(ReadI16Le, i16, get_i16_le);
reader!(ReadI32Le, i32, get_i32_le);
reader!(ReadI64Le, i64, get_i64_le);
reader!(ReadI128Le, i128, get_i128_le);

reader!(ReadF32Le, f32, get_f32_le);
reader!(ReadF64Le, f64, get_f64_le);
