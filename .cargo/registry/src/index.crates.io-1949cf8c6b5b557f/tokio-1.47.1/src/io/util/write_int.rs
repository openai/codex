use crate::io::AsyncWrite;

use bytes::BufMut;
use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

macro_rules! writer {
    ($name:ident, $ty:ty, $writer:ident) => {
        writer!($name, $ty, $writer, std::mem::size_of::<$ty>());
    };
    ($name:ident, $ty:ty, $writer:ident, $bytes:expr) => {
        pin_project! {
            #[doc(hidden)]
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            pub struct $name<W> {
                #[pin]
                dst: W,
                buf: [u8; $bytes],
                written: u8,
                // Make this future `!Unpin` for compatibility with async trait methods.
                #[pin]
                _pin: PhantomPinned,
            }
        }

        impl<W> $name<W> {
            pub(crate) fn new(w: W, value: $ty) -> Self {
                let mut writer = Self {
                    buf: [0; $bytes],
                    written: 0,
                    dst: w,
                    _pin: PhantomPinned,
                };
                BufMut::$writer(&mut &mut writer.buf[..], value);
                writer
            }
        }

        impl<W> Future for $name<W>
        where
            W: AsyncWrite,
        {
            type Output = io::Result<()>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut me = self.project();

                if *me.written == $bytes as u8 {
                    return Poll::Ready(Ok(()));
                }

                while *me.written < $bytes as u8 {
                    *me.written += match me
                        .dst
                        .as_mut()
                        .poll_write(cx, &me.buf[*me.written as usize..])
                    {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(0)) => {
                            return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                        }
                        Poll::Ready(Ok(n)) => n as u8,
                    };
                }
                Poll::Ready(Ok(()))
            }
        }
    };
}

macro_rules! writer8 {
    ($name:ident, $ty:ty) => {
        pin_project! {
            #[doc(hidden)]
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            pub struct $name<W> {
                #[pin]
                dst: W,
                byte: $ty,
                // Make this future `!Unpin` for compatibility with async trait methods.
                #[pin]
                _pin: PhantomPinned,
            }
        }

        impl<W> $name<W> {
            pub(crate) fn new(dst: W, byte: $ty) -> Self {
                Self {
                    dst,
                    byte,
                    _pin: PhantomPinned,
                }
            }
        }

        impl<W> Future for $name<W>
        where
            W: AsyncWrite,
        {
            type Output = io::Result<()>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let me = self.project();

                let buf = [*me.byte as u8];

                match me.dst.poll_write(cx, &buf[..]) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
                    Poll::Ready(Ok(0)) => Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                    Poll::Ready(Ok(1)) => Poll::Ready(Ok(())),
                    Poll::Ready(Ok(_)) => unreachable!(),
                }
            }
        }
    };
}

writer8!(WriteU8, u8);
writer8!(WriteI8, i8);

writer!(WriteU16, u16, put_u16);
writer!(WriteU32, u32, put_u32);
writer!(WriteU64, u64, put_u64);
writer!(WriteU128, u128, put_u128);

writer!(WriteI16, i16, put_i16);
writer!(WriteI32, i32, put_i32);
writer!(WriteI64, i64, put_i64);
writer!(WriteI128, i128, put_i128);

writer!(WriteF32, f32, put_f32);
writer!(WriteF64, f64, put_f64);

writer!(WriteU16Le, u16, put_u16_le);
writer!(WriteU32Le, u32, put_u32_le);
writer!(WriteU64Le, u64, put_u64_le);
writer!(WriteU128Le, u128, put_u128_le);

writer!(WriteI16Le, i16, put_i16_le);
writer!(WriteI32Le, i32, put_i32_le);
writer!(WriteI64Le, i64, put_i64_le);
writer!(WriteI128Le, i128, put_i128_le);

writer!(WriteF32Le, f32, put_f32_le);
writer!(WriteF64Le, f64, put_f64_le);
