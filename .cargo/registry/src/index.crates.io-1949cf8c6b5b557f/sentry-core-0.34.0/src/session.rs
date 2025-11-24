//! Release Health Sessions
//!
//! <https://develop.sentry.dev/sdk/sessions/>

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use crate::client::TransportArc;
use crate::clientoptions::SessionMode;
use crate::protocol::{
    EnvelopeItem, Event, Level, SessionAggregateItem, SessionAggregates, SessionAttributes,
    SessionStatus, SessionUpdate,
};
use crate::scope::StackLayer;
use crate::types::random_uuid;
use crate::{Client, Envelope};

#[derive(Clone, Debug)]
pub struct Session {
    client: Arc<Client>,
    session_update: SessionUpdate<'static>,
    started: Instant,
    dirty: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close(SessionStatus::Exited);
        if self.dirty {
            self.client.enqueue_session(self.session_update.clone());
        }
    }
}

impl Session {
    pub fn from_stack(stack: &StackLayer) -> Option<Self> {
        let client = stack.client.as_ref()?;
        let options = client.options();
        let user = stack.scope.user.as_deref();
        let distinct_id = user
            .and_then(|user| {
                user.id
                    .as_ref()
                    .or(user.email.as_ref())
                    .or(user.username.as_ref())
            })
            .cloned();
        Some(Self {
            client: client.clone(),
            session_update: SessionUpdate {
                session_id: random_uuid(),
                distinct_id,
                sequence: None,
                timestamp: None,
                started: SystemTime::now(),
                init: true,
                duration: None,
                status: SessionStatus::Ok,
                errors: 0,
                attributes: SessionAttributes {
                    release: options.release.clone()?,
                    environment: options.environment.clone(),
                    ip_address: None,
                    user_agent: None,
                },
            },
            started: Instant::now(),
            dirty: true,
        })
    }

    pub(crate) fn update_from_event(&mut self, event: &Event<'static>) {
        if self.session_update.status != SessionStatus::Ok {
            // a session that has already transitioned to a "terminal" state
            // should not receive any more updates
            return;
        }
        let mut has_error = event.level >= Level::Error;
        let mut is_crash = false;
        for exc in &event.exception.values {
            has_error = true;
            if let Some(mechanism) = &exc.mechanism {
                if let Some(false) = mechanism.handled {
                    is_crash = true;
                    break;
                }
            }
        }

        if is_crash {
            self.session_update.status = SessionStatus::Crashed;
        }
        if has_error {
            self.session_update.errors += 1;
            self.dirty = true;
        }
    }

    pub(crate) fn close(&mut self, status: SessionStatus) {
        if self.session_update.status == SessionStatus::Ok {
            let status = match status {
                SessionStatus::Ok => SessionStatus::Exited,
                s => s,
            };
            self.session_update.duration = Some(self.started.elapsed().as_secs_f64());
            self.session_update.status = status;
            self.dirty = true;
        }
    }

    pub(crate) fn create_envelope_item(&mut self) -> Option<EnvelopeItem> {
        if self.dirty {
            let item = self.session_update.clone().into();
            self.session_update.init = false;
            self.dirty = false;
            return Some(item);
        }
        None
    }
}

// as defined here: https://develop.sentry.dev/sdk/envelopes/#size-limits
const MAX_SESSION_ITEMS: usize = 100;
const FLUSH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
struct SessionQueue {
    individual: Vec<SessionUpdate<'static>>,
    aggregated: Option<AggregatedSessions>,
}

#[derive(Debug)]
struct AggregatedSessions {
    buckets: HashMap<AggregationKey, AggregationCounts>,
    attributes: SessionAttributes<'static>,
}

impl From<AggregatedSessions> for EnvelopeItem {
    fn from(sessions: AggregatedSessions) -> Self {
        let aggregates = sessions
            .buckets
            .into_iter()
            .map(|(key, counts)| SessionAggregateItem {
                started: key.started,
                distinct_id: key.distinct_id,
                exited: counts.exited,
                errored: counts.errored,
                abnormal: counts.abnormal,
                crashed: counts.crashed,
            })
            .collect();

        SessionAggregates {
            aggregates,
            attributes: sessions.attributes,
        }
        .into()
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct AggregationKey {
    started: SystemTime,
    distinct_id: Option<String>,
}

#[derive(Debug, Default)]
struct AggregationCounts {
    exited: u32,
    errored: u32,
    abnormal: u32,
    crashed: u32,
}

/// Background Session Flusher
///
/// The background flusher queues session updates for delayed batched sending.
/// It has its own background thread that will flush its queue once every
/// `FLUSH_INTERVAL`.
pub(crate) struct SessionFlusher {
    transport: TransportArc,
    mode: SessionMode,
    queue: Arc<Mutex<SessionQueue>>,
    shutdown: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl SessionFlusher {
    /// Creates a new Flusher that will submit envelopes to the given `transport`.
    pub fn new(transport: TransportArc, mode: SessionMode) -> Self {
        let queue = Arc::new(Mutex::new(Default::default()));
        #[allow(clippy::mutex_atomic)]
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let worker_transport = transport.clone();
        let worker_queue = queue.clone();
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("sentry-session-flusher".into())
            .spawn(move || {
                let (lock, cvar) = worker_shutdown.as_ref();
                let mut shutdown = lock.lock().unwrap();
                // check this immediately, in case the main thread is already shutting down
                if *shutdown {
                    return;
                }
                let mut last_flush = Instant::now();
                loop {
                    let timeout = FLUSH_INTERVAL
                        .checked_sub(last_flush.elapsed())
                        .unwrap_or_else(|| Duration::from_secs(0));
                    shutdown = cvar.wait_timeout(shutdown, timeout).unwrap().0;
                    if *shutdown {
                        return;
                    }
                    if last_flush.elapsed() < FLUSH_INTERVAL {
                        continue;
                    }
                    SessionFlusher::flush_queue_internal(
                        worker_queue.lock().unwrap(),
                        &worker_transport,
                    );
                    last_flush = Instant::now();
                }
            })
            .unwrap();

        Self {
            transport,
            mode,
            queue,
            shutdown,
            worker: Some(worker),
        }
    }

    /// Enqueues a session update for delayed sending.
    ///
    /// This will aggregate session counts in request mode, for all sessions
    /// that were not yet partially sent.
    pub fn enqueue(&self, session_update: SessionUpdate<'static>) {
        let mut queue = self.queue.lock().unwrap();
        if self.mode == SessionMode::Application || !session_update.init {
            queue.individual.push(session_update);
            if queue.individual.len() >= MAX_SESSION_ITEMS {
                SessionFlusher::flush_queue_internal(queue, &self.transport);
            }
            return;
        }

        let aggregate = queue.aggregated.get_or_insert_with(|| AggregatedSessions {
            buckets: HashMap::with_capacity(1),
            attributes: session_update.attributes.clone(),
        });

        let duration = session_update
            .started
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let duration = (duration.as_secs() / 60) * 60;
        let started = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(duration))
            .unwrap();

        let key = AggregationKey {
            started,
            distinct_id: session_update.distinct_id,
        };

        let bucket = aggregate.buckets.entry(key).or_default();

        match session_update.status {
            SessionStatus::Exited => {
                if session_update.errors > 0 {
                    bucket.errored += 1;
                } else {
                    bucket.exited += 1;
                }
            }
            SessionStatus::Crashed => {
                bucket.crashed += 1;
            }
            SessionStatus::Abnormal => {
                bucket.abnormal += 1;
            }
            SessionStatus::Ok => {
                sentry_debug!("unreachable: only closed sessions will be enqueued");
            }
        }
    }

    /// Flushes the queue to the transport.
    pub fn flush(&self) {
        let queue = self.queue.lock().unwrap();
        SessionFlusher::flush_queue_internal(queue, &self.transport);
    }

    /// Flushes the queue to the transport.
    ///
    /// This is a static method as it will be called from both the background
    /// thread and the main thread on drop.
    fn flush_queue_internal(mut queue_lock: MutexGuard<SessionQueue>, transport: &TransportArc) {
        let queue = std::mem::take(&mut queue_lock.individual);
        let aggregate = queue_lock.aggregated.take();
        drop(queue_lock);

        // send aggregates
        if let Some(aggregate) = aggregate {
            if let Some(ref transport) = *transport.read().unwrap() {
                let mut envelope = Envelope::new();
                envelope.add_item(aggregate);
                transport.send_envelope(envelope);
            }
        }

        // send individual items
        if queue.is_empty() {
            return;
        }

        let mut envelope = Envelope::new();
        let mut items = 0;

        for session_update in queue {
            if items >= MAX_SESSION_ITEMS {
                if let Some(ref transport) = *transport.read().unwrap() {
                    transport.send_envelope(envelope);
                }
                envelope = Envelope::new();
                items = 0;
            }

            envelope.add_item(session_update);
            items += 1;
        }

        if let Some(ref transport) = *transport.read().unwrap() {
            transport.send_envelope(envelope);
        }
    }
}

impl Drop for SessionFlusher {
    fn drop(&mut self) {
        let (lock, cvar) = self.shutdown.as_ref();
        *lock.lock().unwrap() = true;
        cvar.notify_one();

        if let Some(worker) = self.worker.take() {
            worker.join().ok();
        }
        SessionFlusher::flush_queue_internal(self.queue.lock().unwrap(), &self.transport);
    }
}

#[cfg(all(test, feature = "test"))]
mod tests {
    use std::cmp::Ordering;

    use super::*;
    use crate as sentry;
    use crate::protocol::{Envelope, EnvelopeItem, SessionStatus};

    fn capture_envelopes<F>(f: F) -> Vec<Envelope>
    where
        F: FnOnce(),
    {
        crate::test::with_captured_envelopes_options(
            f,
            crate::ClientOptions {
                release: Some("some-release".into()),
                ..Default::default()
            },
        )
    }

    #[test]
    fn test_session_startstop() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();
            std::thread::sleep(std::time::Duration::from_millis(10));
        });
        assert_eq!(envelopes.len(), 1);

        let mut items = envelopes[0].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert!(session.duration.unwrap() > 0.01);
            assert_eq!(session.errors, 0);
            assert_eq!(session.attributes.release, "some-release");
            assert!(session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    #[test]
    fn test_session_batching() {
        let envelopes = capture_envelopes(|| {
            for _ in 0..(MAX_SESSION_ITEMS * 2) {
                sentry::start_session();
            }
        });
        // we only want *two* envelope for all the sessions
        assert_eq!(envelopes.len(), 2);

        let items = envelopes[0].items().chain(envelopes[1].items());
        assert_eq!(items.clone().count(), MAX_SESSION_ITEMS * 2);
        for item in items {
            assert!(matches!(item, EnvelopeItem::SessionUpdate(_)));
        }
    }

    #[test]
    fn test_session_aggregation() {
        let envelopes = crate::test::with_captured_envelopes_options(
            || {
                sentry::start_session();
                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);

                for _ in 0..50 {
                    sentry::start_session();
                }
                sentry::end_session();

                sentry::configure_scope(|scope| {
                    scope.set_user(Some(sentry::User {
                        id: Some("foo-bar".into()),
                        ..Default::default()
                    }));
                    scope.add_event_processor(Box::new(|_| None));
                });

                for _ in 0..50 {
                    sentry::start_session();
                }

                // This error will be discarded because of the event processor,
                // and session will not be updated.
                // Only events dropped due to sampling should update the session.
                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);
            },
            crate::ClientOptions {
                release: Some("some-release".into()),
                session_mode: SessionMode::Request,
                ..Default::default()
            },
        );
        assert_eq!(envelopes.len(), 2);

        let mut items = envelopes[0].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        assert_eq!(items.next(), None);

        let mut items = envelopes[1].items();
        if let Some(EnvelopeItem::SessionAggregates(aggregate)) = items.next() {
            let mut aggregates = aggregate.aggregates.clone();
            assert_eq!(aggregates.len(), 2);
            // the order depends on a hashmap and is not stable otherwise
            aggregates.sort_by(|a, b| {
                a.distinct_id
                    .partial_cmp(&b.distinct_id)
                    .unwrap_or(Ordering::Less)
            });

            assert_eq!(aggregates[0].distinct_id, None);
            assert_eq!(aggregates[0].exited, 50);

            assert_eq!(aggregates[1].errored, 0);
            assert_eq!(aggregates[1].distinct_id, Some("foo-bar".into()));
            assert_eq!(aggregates[1].exited, 50);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    #[test]
    fn test_session_error() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);
        });
        assert_eq!(envelopes.len(), 2);

        let mut items = envelopes[0].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Ok);
            assert_eq!(session.errors, 1);
            assert_eq!(session.attributes.release, "some-release");
            assert!(session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);

        let mut items = envelopes[1].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 1);
            assert!(!session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    #[test]
    fn test_session_abnormal() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();
            sentry::end_session_with_status(SessionStatus::Abnormal);
        });
        assert_eq!(envelopes.len(), 1);

        let mut items = envelopes[0].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Abnormal);
            assert!(session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    #[test]
    fn test_session_sampled_errors() {
        let mut envelopes = crate::test::with_captured_envelopes_options(
            || {
                sentry::start_session();

                for _ in 0..100 {
                    let err = "NaN".parse::<usize>().unwrap_err();
                    sentry::capture_error(&err);
                }
            },
            crate::ClientOptions {
                release: Some("some-release".into()),
                sample_rate: 0.5,
                ..Default::default()
            },
        );
        assert!(envelopes.len() > 25);
        assert!(envelopes.len() < 75);

        let envelope = envelopes.pop().unwrap();
        let mut items = envelope.items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 100);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    /// For _user-mode_ sessions, we want to inherit the session for any _new_
    /// Hub that is spawned from the main thread Hub which already has a session
    /// attached
    #[test]
    fn test_inherit_session_from_top() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);

            // create a new Hub which should have the same session
            let hub = std::sync::Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));

            sentry::Hub::run(hub, || {
                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);

                sentry::with_scope(
                    |_| {},
                    || {
                        let err = "NaN".parse::<usize>().unwrap_err();
                        sentry::capture_error(&err);
                    },
                );
            });
        });

        assert_eq!(envelopes.len(), 4); // 3 errors and one session end

        let mut items = envelopes[3].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 3);
            assert!(!session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    /// We want to forward-inherit sessions as the previous test asserted, but
    /// not *backwards*. So any new session created in a derived Hub and scope
    /// will only get updates from that particular scope.
    #[test]
    fn test_dont_inherit_session_backwards() {
        let envelopes = capture_envelopes(|| {
            let hub = std::sync::Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));

            sentry::Hub::run(hub, || {
                sentry::with_scope(
                    |_| {},
                    || {
                        sentry::start_session();

                        let err = "NaN".parse::<usize>().unwrap_err();
                        sentry::capture_error(&err);
                    },
                );

                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);
            });

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);
        });

        assert_eq!(envelopes.len(), 4); // 3 errors and one session end

        let mut items = envelopes[0].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Ok);
            assert_eq!(session.errors, 1);
            assert!(session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);

        // the other two events should not have session updates
        let mut items = envelopes[1].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        assert_eq!(items.next(), None);

        let mut items = envelopes[2].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        assert_eq!(items.next(), None);

        // the session end is sent last as it is possibly batched
        let mut items = envelopes[3].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 1);
            assert!(!session.init);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }
}
