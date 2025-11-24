use futures_util::StreamExt;
use static_assertions::assert_impl_all;

use crate::{
    blocking::Connection, message::Message, utils::block_on, MatchRule, OwnedMatchRule, Result,
};

/// A blocking wrapper of [`crate::MessageStream`].
///
/// Just like [`crate::MessageStream`] must be continuously polled, you must continuously iterate
/// over this type until it's consumed or dropped.
#[derive(Debug, Clone)]
pub struct MessageIterator {
    // Wrap it in an `Option` to ensure the stream is dropped in a `block_on` call. This is needed
    // for tokio because the proxy spawns a task in its `Drop` impl and that needs a runtime
    // context in case of tokio. Moreover, we want to use `AsyncDrop::async_drop` to drop the
    // stream to ensure any associated match rule is deregistered before the iterator is
    // dropped.
    pub(crate) azync: Option<crate::MessageStream>,
}

assert_impl_all!(MessageIterator: Send, Sync, Unpin);

impl MessageIterator {
    /// Get a reference to the underlying async message stream.
    pub fn inner(&self) -> &crate::MessageStream {
        self.azync.as_ref().expect("Inner stream is `None`")
    }

    /// Get the underlying async message stream, consuming `self`.
    pub fn into_inner(mut self) -> crate::MessageStream {
        self.azync.take().expect("Inner stream is `None`")
    }

    /// Create a message iterator for the given match rule.
    ///
    /// This is a wrapper around [`crate::MessageStream::for_match_rule`]. Unlike the underlying
    /// `MessageStream`, the match rule is immediately deregistered when the iterator is dropped.
    ///
    /// # Example
    ///
    /// ```
    /// use zbus::{blocking::{Connection, MessageIterator}, MatchRule, fdo::NameOwnerChanged};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let conn = Connection::session()?;
    /// let rule = MatchRule::builder()
    ///     .msg_type(zbus::message::Type::Signal)
    ///     .sender("org.freedesktop.DBus")?
    ///     .interface("org.freedesktop.DBus")?
    ///     .member("NameOwnerChanged")?
    ///     .add_arg("org.freedesktop.zbus.MatchRuleIteratorTest42")?
    ///     .build();
    /// let mut iter = MessageIterator::for_match_rule(
    ///     rule,
    ///     &conn,
    ///     // For such a specific match rule, we don't need a big queue.
    ///     Some(1),
    /// )?;
    ///
    /// let rule_str = "type='signal',sender='org.freedesktop.DBus',\
    ///                 interface='org.freedesktop.DBus',member='NameOwnerChanged',\
    ///                 arg0='org.freedesktop.zbus.MatchRuleIteratorTest42'";
    /// assert_eq!(
    ///     iter.match_rule().map(|r| r.to_string()).as_deref(),
    ///     Some(rule_str),
    /// );
    ///
    /// // We register 2 names, starting with the uninteresting one. If `iter` wasn't filtering
    /// // messages based on the match rule, we'd receive method return call for each of these 2
    /// // calls first.
    /// //
    /// // Note that the `NameOwnerChanged` signal will not be sent by the bus  for the first name
    /// // we register since we setup an arg filter.
    /// conn.request_name("org.freedesktop.zbus.MatchRuleIteratorTest44")?;
    /// conn.request_name("org.freedesktop.zbus.MatchRuleIteratorTest42")?;
    ///
    /// let msg = iter.next().unwrap()?;
    /// let signal = NameOwnerChanged::from_message(msg).unwrap();
    /// assert_eq!(signal.args()?.name(), "org.freedesktop.zbus.MatchRuleIteratorTest42");
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Caveats
    ///
    /// Since this method relies on [`MatchRule::matches`], it inherits its caveats.
    pub fn for_match_rule<R>(rule: R, conn: &Connection, max_queued: Option<usize>) -> Result<Self>
    where
        R: TryInto<OwnedMatchRule>,
        R::Error: Into<crate::Error>,
    {
        block_on(crate::MessageStream::for_match_rule(
            rule,
            conn.inner(),
            max_queued,
        ))
        .map(Some)
        .map(|s| Self { azync: s })
    }

    /// The associated match rule, if any.
    pub fn match_rule(&self) -> Option<MatchRule<'_>> {
        self.azync
            .as_ref()
            .expect("Inner stream is `None`")
            .match_rule()
    }
}

impl Iterator for MessageIterator {
    type Item = Result<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.azync.as_mut().expect("Inner stream is `None`").next())
    }
}

impl From<Connection> for MessageIterator {
    fn from(conn: Connection) -> Self {
        let azync = crate::MessageStream::from(conn.into_inner());

        Self { azync: Some(azync) }
    }
}

impl From<&Connection> for MessageIterator {
    fn from(conn: &Connection) -> Self {
        Self::from(conn.clone())
    }
}

impl From<MessageIterator> for Connection {
    fn from(mut iter: MessageIterator) -> Connection {
        Connection::from(crate::Connection::from(
            iter.azync.take().expect("Inner stream is `None`"),
        ))
    }
}

impl From<&MessageIterator> for Connection {
    fn from(iter: &MessageIterator) -> Connection {
        Connection::from(crate::Connection::from(
            iter.azync.as_ref().expect("Inner stream is `None`"),
        ))
    }
}

impl std::ops::Drop for MessageIterator {
    fn drop(&mut self) {
        block_on(async {
            if let Some(azync) = self.azync.take() {
                crate::AsyncDrop::async_drop(azync).await;
            }
        });
    }
}
