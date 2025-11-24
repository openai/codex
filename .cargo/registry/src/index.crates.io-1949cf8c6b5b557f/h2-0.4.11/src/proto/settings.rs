use crate::codec::UserError;
use crate::error::Reason;
use crate::proto::*;
use std::task::{Context, Poll};

#[derive(Debug)]
pub(crate) struct Settings {
    /// Our local SETTINGS sync state with the remote.
    local: Local,
    /// Received SETTINGS frame pending processing. The ACK must be written to
    /// the socket first then the settings applied **before** receiving any
    /// further frames.
    remote: Option<frame::Settings>,
    /// Whether the connection has received the initial SETTINGS frame from the
    /// remote peer.
    has_received_remote_initial_settings: bool,
}

#[derive(Debug)]
enum Local {
    /// We want to send these SETTINGS to the remote when the socket is ready.
    ToSend(frame::Settings),
    /// We have sent these SETTINGS and are waiting for the remote to ACK
    /// before we apply them.
    WaitingAck(frame::Settings),
    /// Our local settings are in sync with the remote.
    Synced,
}

impl Settings {
    pub(crate) fn new(local: frame::Settings) -> Self {
        Settings {
            // We assume the initial local SETTINGS were flushed during
            // the handshake process.
            local: Local::WaitingAck(local),
            remote: None,
            has_received_remote_initial_settings: false,
        }
    }

    pub(crate) fn recv_settings<T, B, C, P>(
        &mut self,
        frame: frame::Settings,
        codec: &mut Codec<T, B>,
        streams: &mut Streams<C, P>,
    ) -> Result<(), Error>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
        C: Buf,
        P: Peer,
    {
        if frame.is_ack() {
            match &self.local {
                Local::WaitingAck(local) => {
                    tracing::debug!("received settings ACK; applying {:?}", local);

                    if let Some(max) = local.max_frame_size() {
                        codec.set_max_recv_frame_size(max as usize);
                    }

                    if let Some(max) = local.max_header_list_size() {
                        codec.set_max_recv_header_list_size(max as usize);
                    }

                    if let Some(val) = local.header_table_size() {
                        codec.set_recv_header_table_size(val as usize);
                    }

                    streams.apply_local_settings(local)?;
                    self.local = Local::Synced;
                    Ok(())
                }
                Local::ToSend(..) | Local::Synced => {
                    // We haven't sent any SETTINGS frames to be ACKed, so
                    // this is very bizarre! Remote is either buggy or malicious.
                    proto_err!(conn: "received unexpected settings ack");
                    Err(Error::library_go_away(Reason::PROTOCOL_ERROR))
                }
            }
        } else {
            // We always ACK before reading more frames, so `remote` should
            // always be none!
            assert!(self.remote.is_none());
            self.remote = Some(frame);
            Ok(())
        }
    }

    pub(crate) fn send_settings(&mut self, frame: frame::Settings) -> Result<(), UserError> {
        assert!(!frame.is_ack());
        match &self.local {
            Local::ToSend(..) | Local::WaitingAck(..) => Err(UserError::SendSettingsWhilePending),
            Local::Synced => {
                tracing::trace!("queue to send local settings: {:?}", frame);
                self.local = Local::ToSend(frame);
                Ok(())
            }
        }
    }

    /// Sets `true` to `self.has_received_remote_initial_settings`.
    /// Returns `true` if this method is called for the first time.
    /// (i.e. it is the initial SETTINGS frame from the remote peer)
    fn mark_remote_initial_settings_as_received(&mut self) -> bool {
        let has_received = self.has_received_remote_initial_settings;
        self.has_received_remote_initial_settings = true;
        !has_received
    }

    pub(crate) fn poll_send<T, B, C, P>(
        &mut self,
        cx: &mut Context,
        dst: &mut Codec<T, B>,
        streams: &mut Streams<C, P>,
    ) -> Poll<Result<(), Error>>
    where
        T: AsyncWrite + Unpin,
        B: Buf,
        C: Buf,
        P: Peer,
    {
        if let Some(settings) = self.remote.clone() {
            if !dst.poll_ready(cx)?.is_ready() {
                return Poll::Pending;
            }

            // Create an ACK settings frame
            let frame = frame::Settings::ack();

            // Buffer the settings frame
            dst.buffer(frame.into()).expect("invalid settings frame");

            tracing::trace!("ACK sent; applying settings");

            let is_initial = self.mark_remote_initial_settings_as_received();
            streams.apply_remote_settings(&settings, is_initial)?;

            if let Some(val) = settings.header_table_size() {
                dst.set_send_header_table_size(val as usize);
            }

            if let Some(val) = settings.max_frame_size() {
                dst.set_max_send_frame_size(val as usize);
            }
        }

        self.remote = None;

        match &self.local {
            Local::ToSend(settings) => {
                if !dst.poll_ready(cx)?.is_ready() {
                    return Poll::Pending;
                }

                // Buffer the settings frame
                dst.buffer(settings.clone().into())
                    .expect("invalid settings frame");
                tracing::trace!("local settings sent; waiting for ack: {:?}", settings);

                self.local = Local::WaitingAck(settings.clone());
            }
            Local::WaitingAck(..) | Local::Synced => {}
        }

        Poll::Ready(Ok(()))
    }
}
