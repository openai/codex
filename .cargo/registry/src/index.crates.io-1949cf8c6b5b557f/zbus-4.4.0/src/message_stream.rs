use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_broadcast::Receiver as ActiveReceiver;
use futures_core::stream;
use futures_util::stream::FusedStream;
use ordered_stream::{OrderedStream, PollResult};
use static_assertions::assert_impl_all;
use tracing::warn;

use crate::{
    connection::ConnectionInner,
    message::{Message, Sequence},
    AsyncDrop, Connection, MatchRule, OwnedMatchRule, Result,
};

/// A [`stream::Stream`] implementation that yields [`Message`] items.
///
/// You can convert a [`Connection`] to this type and back to [`Connection`].
///
/// **NOTE**: You must ensure a `MessageStream` is continuously polled or you will experience hangs.
/// If you don't need to continuously poll the `MessageStream` but need to keep it around for later
/// use, keep the connection around and convert it into a `MessageStream` when needed. The
/// conversion is not an expensive operation so you don't need to  worry about performance, unless
/// you do it very frequently. If you need to convert back and forth frequently, you may want to
/// consider keeping both a connection and stream around.
#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct MessageStream {
    inner: Inner,
}

assert_impl_all!(MessageStream: Send, Sync, Unpin);

impl MessageStream {
    /// Create a message stream for the given match rule.
    ///
    /// If `conn` is a bus connection and match rule is for a signal, the match rule will be
    /// registered with the bus and queued for deregistration when the stream is dropped. If you'd
    /// like immediate deregistration, use [`AsyncDrop::async_drop`]. The reason match rules are
    /// only registered with the bus for signals is that D-Bus specification only allows signals to
    /// be broadcasted and unicast messages are always sent to their destination (regardless of any
    /// match rules registered by the destination) by the bus. Hence there is no need to register
    /// match rules for non-signal messages with the bus.
    ///
    /// Having said that, stream created by this method can still very useful as it allows you to
    /// avoid needless task wakeups and simplify your stream consuming code.
    ///
    /// You can optionally also request the capacity of the underlying message queue through
    /// `max_queued`. If specified, the capacity is guaranteed to be at least `max_queued`. If not
    /// specified, the default of 64 is assumed. The capacity can also be changed later through
    /// [`MessageStream::set_max_queued`].
    ///
    /// # Example
    ///
    /// ```
    /// use async_io::Timer;
    /// use zbus::{AsyncDrop, Connection, MatchRule, MessageStream, fdo::NameOwnerChanged};
    /// use futures_util::{TryStreamExt, future::select, future::Either::{Left, Right}, pin_mut};
    ///
    /// # zbus::block_on(async {
    /// let conn = Connection::session().await?;
    /// let rule = MatchRule::builder()
    ///     .msg_type(zbus::message::Type::Signal)
    ///     .sender("org.freedesktop.DBus")?
    ///     .interface("org.freedesktop.DBus")?
    ///     .member("NameOwnerChanged")?
    ///     .add_arg("org.freedesktop.zbus.MatchRuleStreamTest42")?
    ///     .build();
    /// let mut stream = MessageStream::for_match_rule(
    ///     rule,
    ///     &conn,
    ///     // For such a specific match rule, we don't need a big queue.
    ///     Some(1),
    /// ).await?;
    ///
    /// let rule_str = "type='signal',sender='org.freedesktop.DBus',\
    ///                 interface='org.freedesktop.DBus',member='NameOwnerChanged',\
    ///                 arg0='org.freedesktop.zbus.MatchRuleStreamTest42'";
    /// assert_eq!(
    ///     stream.match_rule().map(|r| r.to_string()).as_deref(),
    ///     Some(rule_str),
    /// );
    ///
    /// // We register 2 names, starting with the uninteresting one. If `stream` wasn't filtering
    /// // messages based on the match rule, we'd receive method return call for each of these 2
    /// // calls first.
    /// //
    /// // Note that the `NameOwnerChanged` signal will not be sent by the bus  for the first name
    /// // we register since we setup an arg filter.
    /// conn.request_name("org.freedesktop.zbus.MatchRuleStreamTest44")
    ///     .await?;
    /// conn.request_name("org.freedesktop.zbus.MatchRuleStreamTest42")
    ///     .await?;
    ///
    /// let msg = stream.try_next().await?.unwrap();
    /// let signal = NameOwnerChanged::from_message(msg).unwrap();
    /// assert_eq!(signal.args()?.name(), "org.freedesktop.zbus.MatchRuleStreamTest42");
    /// stream.async_drop().await;
    ///
    /// // Ensure the match rule is deregistered and this connection doesn't receive
    /// // `NameOwnerChanged` signals.
    /// let stream = MessageStream::from(&conn).try_filter_map(|msg| async move {
    ///     Ok(NameOwnerChanged::from_message(msg))
    /// });
    /// conn.release_name("org.freedesktop.zbus.MatchRuleStreamTest42").await?;
    ///
    /// pin_mut!(stream);
    /// let next = stream.try_next();
    /// pin_mut!(next);
    /// let timeout = Timer::after(std::time::Duration::from_millis(50));
    /// pin_mut!(timeout);
    /// match select(next, timeout).await {
    ///    Left((msg, _)) => unreachable!("unexpected message: {:?}", msg),
    ///    Right((_, _)) => (),
    /// }
    ///
    /// # Ok::<(), zbus::Error>(())
    /// # }).unwrap();
    /// ```
    ///
    /// # Caveats
    ///
    /// Since this method relies on [`MatchRule::matches`], it inherits its caveats.
    pub async fn for_match_rule<R>(
        rule: R,
        conn: &Connection,
        max_queued: Option<usize>,
    ) -> Result<Self>
    where
        R: TryInto<OwnedMatchRule>,
        R::Error: Into<crate::Error>,
    {
        let rule = rule.try_into().map_err(Into::into)?;
        let msg_receiver = conn.add_match(rule.clone(), max_queued).await?;

        Ok(Self::for_subscription_channel(
            msg_receiver,
            Some(rule),
            conn,
        ))
    }

    /// The associated match rule, if any.
    pub fn match_rule(&self) -> Option<MatchRule<'_>> {
        self.inner.match_rule.as_deref().cloned()
    }

    /// The maximum number of messages to queue for this stream.
    pub fn max_queued(&self) -> usize {
        self.inner.msg_receiver.capacity()
    }

    /// Set maximum number of messages to queue for this stream.
    ///
    /// After this call, the capacity is guaranteed to be at least `max_queued`.
    pub fn set_max_queued(&mut self, max_queued: usize) {
        if max_queued <= self.max_queued() {
            return;
        }
        self.inner.msg_receiver.set_capacity(max_queued);
    }

    pub(crate) fn for_subscription_channel(
        msg_receiver: ActiveReceiver<Result<Message>>,
        rule: Option<OwnedMatchRule>,
        conn: &Connection,
    ) -> Self {
        let conn_inner = conn.inner.clone();

        Self {
            inner: Inner {
                conn_inner,
                msg_receiver,
                match_rule: rule,
            },
        }
    }
}

impl stream::Stream for MessageStream {
    type Item = Result<Message>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        Pin::new(&mut this.inner.msg_receiver).poll_next(cx)
    }
}

impl OrderedStream for MessageStream {
    type Data = Result<Message>;
    type Ordering = Sequence;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let this = self.get_mut();

        match stream::Stream::poll_next(Pin::new(this), cx) {
            Poll::Pending if before.is_some() => {
                // Assume the provided Sequence in before was obtained from a Message
                // associated with our Connection (because that's the only supported use case).
                // Because there is only one socket-reader task, any messages that would have been
                // ordered before that message would have already been sitting in the broadcast
                // queue (and we would have seen Ready in our poll).  Because we didn't, we can
                // guarantee that we won't ever produce a message whose sequence is before that
                // provided value, and so we can return NoneBefore.
                //
                // This ensures that ordered_stream::Join will never return Pending while it
                // has a message buffered.
                Poll::Ready(PollResult::NoneBefore)
            }
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(msg))) => Poll::Ready(PollResult::Item {
                ordering: msg.recv_position(),
                data: Ok(msg),
            }),
            Poll::Ready(Some(Err(e))) => Poll::Ready(PollResult::Item {
                ordering: Sequence::LAST,
                data: Err(e),
            }),
            Poll::Ready(None) => Poll::Ready(PollResult::Terminated),
        }
    }
}

impl FusedStream for MessageStream {
    fn is_terminated(&self) -> bool {
        self.inner.msg_receiver.is_terminated()
    }
}

impl From<Connection> for MessageStream {
    fn from(conn: Connection) -> Self {
        let conn_inner = conn.inner;
        let msg_receiver = conn_inner.msg_receiver.activate_cloned();

        Self {
            inner: Inner {
                conn_inner,
                msg_receiver,
                match_rule: None,
            },
        }
    }
}

impl From<&Connection> for MessageStream {
    fn from(conn: &Connection) -> Self {
        Self::from(conn.clone())
    }
}

impl From<MessageStream> for Connection {
    fn from(stream: MessageStream) -> Connection {
        Connection::from(&stream)
    }
}

impl From<&MessageStream> for Connection {
    fn from(stream: &MessageStream) -> Connection {
        Connection {
            inner: stream.inner.conn_inner.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct Inner {
    conn_inner: Arc<ConnectionInner>,
    msg_receiver: ActiveReceiver<Result<Message>>,
    match_rule: Option<OwnedMatchRule>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let conn = Connection {
            inner: self.conn_inner.clone(),
        };

        if let Some(rule) = self.match_rule.take() {
            conn.queue_remove_match(rule);
        }
    }
}

#[async_trait::async_trait]
impl AsyncDrop for MessageStream {
    async fn async_drop(mut self) {
        let conn = Connection {
            inner: self.inner.conn_inner.clone(),
        };

        if let Some(rule) = self.inner.match_rule.take() {
            if let Err(e) = conn.remove_match(rule).await {
                warn!("Failed to remove match rule: {}", e);
            }
        }
    }
}
