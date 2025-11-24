use std::{collections::HashMap, sync::Arc};

use event_listener::Event;
use tracing::{debug, instrument, trace};

use crate::{
    async_lock::Mutex, connection::MsgBroadcaster, Executor, Message, OwnedMatchRule, Task,
};

use super::socket::ReadHalf;

#[derive(Debug)]
pub(crate) struct SocketReader {
    socket: Box<dyn ReadHalf>,
    senders: Arc<Mutex<HashMap<Option<OwnedMatchRule>, MsgBroadcaster>>>,
    already_received_bytes: Vec<u8>,
    #[cfg(unix)]
    already_received_fds: Vec<std::os::fd::OwnedFd>,
    prev_seq: u64,
    activity_event: Arc<Event>,
}

impl SocketReader {
    pub fn new(
        socket: Box<dyn ReadHalf>,
        senders: Arc<Mutex<HashMap<Option<OwnedMatchRule>, MsgBroadcaster>>>,
        already_received_bytes: Vec<u8>,
        #[cfg(unix)] already_received_fds: Vec<std::os::fd::OwnedFd>,
        activity_event: Arc<Event>,
    ) -> Self {
        Self {
            socket,
            senders,
            already_received_bytes,
            #[cfg(unix)]
            already_received_fds,
            prev_seq: 0,
            activity_event,
        }
    }

    pub fn spawn(self, executor: &Executor<'_>) -> Task<()> {
        executor.spawn(self.receive_msg(), "socket reader")
    }

    // Keep receiving messages and put them on the queue.
    #[instrument(name = "socket reader", skip(self))]
    async fn receive_msg(mut self) {
        loop {
            trace!("Waiting for message on the socket..");
            let msg = self.read_socket().await;
            match &msg {
                Ok(msg) => trace!("Message received on the socket: {:?}", msg),
                Err(e) => trace!("Error reading from the socket: {:?}", e),
            };

            let mut senders = self.senders.lock().await;
            for (rule, sender) in &*senders {
                if let Ok(msg) = &msg {
                    if let Some(rule) = rule.as_ref() {
                        match rule.matches(msg) {
                            Ok(true) => (),
                            Ok(false) => continue,
                            Err(e) => {
                                debug!("Error matching message against rule: {:?}", e);

                                continue;
                            }
                        }
                    }
                }

                if let Err(e) = sender.broadcast_direct(msg.clone()).await {
                    // An error would be due to either of these:
                    //
                    // 1. the channel is closed.
                    // 2. No active receivers.
                    //
                    // In either case, just log it.
                    trace!(
                        "Error broadcasting message to stream for `{:?}`: {:?}",
                        rule,
                        e
                    );
                }
            }
            trace!("Broadcasted to all streams: {:?}", msg);

            if msg.is_err() {
                senders.clear();
                trace!("Socket reading task stopped");

                return;
            }
        }
    }

    #[instrument]
    async fn read_socket(&mut self) -> crate::Result<Message> {
        self.activity_event.notify(usize::MAX);
        let seq = self.prev_seq + 1;
        let msg = self
            .socket
            .receive_message(
                seq,
                &mut self.already_received_bytes,
                #[cfg(unix)]
                &mut self.already_received_fds,
            )
            .await?;
        self.prev_seq = seq;

        Ok(msg)
    }
}
