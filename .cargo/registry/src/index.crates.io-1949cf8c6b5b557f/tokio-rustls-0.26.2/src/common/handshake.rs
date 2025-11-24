use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, mem};

use rustls::server::AcceptedAlert;
use rustls::{ConnectionCommon, SideData};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::common::{Stream, SyncWriteAdapter, TlsState};

pub(crate) trait IoSession {
    type Io;
    type Session;

    fn skip_handshake(&self) -> bool;
    fn get_mut(&mut self) -> (&mut TlsState, &mut Self::Io, &mut Self::Session);
    fn into_io(self) -> Self::Io;
}

pub(crate) enum MidHandshake<IS: IoSession> {
    Handshaking(IS),
    End,
    SendAlert {
        io: IS::Io,
        alert: AcceptedAlert,
        error: io::Error,
    },
    Error {
        io: IS::Io,
        error: io::Error,
    },
}

impl<IS, SD> Future for MidHandshake<IS>
where
    IS: IoSession + Unpin,
    IS::Io: AsyncRead + AsyncWrite + Unpin,
    IS::Session: DerefMut + Deref<Target = ConnectionCommon<SD>> + Unpin,
    SD: SideData,
{
    type Output = Result<IS, (io::Error, IS::Io)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        let mut stream = match mem::replace(this, MidHandshake::End) {
            MidHandshake::Handshaking(stream) => stream,
            MidHandshake::SendAlert {
                mut io,
                mut alert,
                error,
            } => loop {
                match alert.write(&mut SyncWriteAdapter { io: &mut io, cx }) {
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        *this = MidHandshake::SendAlert { io, error, alert };
                        return Poll::Pending;
                    }
                    Err(_) | Ok(0) => return Poll::Ready(Err((error, io))),
                    Ok(_) => {}
                };
            },
            // Starting the handshake returned an error; fail the future immediately.
            MidHandshake::Error { io, error } => return Poll::Ready(Err((error, io))),
            _ => panic!("unexpected polling after handshake"),
        };

        if !stream.skip_handshake() {
            let (state, io, session) = stream.get_mut();
            let mut tls_stream = Stream::new(io, session).set_eof(!state.readable());

            macro_rules! try_poll {
                ( $e:expr ) => {
                    match $e {
                        Poll::Ready(Ok(_)) => (),
                        Poll::Ready(Err(err)) => return Poll::Ready(Err((err, stream.into_io()))),
                        Poll::Pending => {
                            *this = MidHandshake::Handshaking(stream);
                            return Poll::Pending;
                        }
                    }
                };
            }

            while tls_stream.session.is_handshaking() {
                try_poll!(tls_stream.handshake(cx));
            }

            try_poll!(Pin::new(&mut tls_stream).poll_flush(cx));
        }

        Poll::Ready(Ok(stream))
    }
}
