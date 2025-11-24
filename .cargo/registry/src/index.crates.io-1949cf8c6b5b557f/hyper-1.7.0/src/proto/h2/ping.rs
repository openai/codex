//! HTTP2 Ping usage
//!
//! hyper uses HTTP2 pings for two purposes:
//!
//! 1. Adaptive flow control using BDP
//! 2. Connection keep-alive
//!
//! Both cases are optional.
//!
//! # BDP Algorithm
//!
//! 1. When receiving a DATA frame, if a BDP ping isn't outstanding:
//!    1a. Record current time.
//!    1b. Send a BDP ping.
//! 2. Increment the number of received bytes.
//! 3. When the BDP ping ack is received:
//!    3a. Record duration from sent time.
//!    3b. Merge RTT with a running average.
//!    3c. Calculate bdp as bytes/rtt.
//!    3d. If bdp is over 2/3 max, set new max to bdp and update windows.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{self, Poll};
use std::time::{Duration, Instant};

use h2::{Ping, PingPong};

use crate::common::time::Time;
use crate::rt::Sleep;

type WindowSize = u32;

pub(super) fn disabled() -> Recorder {
    Recorder { shared: None }
}

pub(super) fn channel(ping_pong: PingPong, config: Config, __timer: Time) -> (Recorder, Ponger) {
    debug_assert!(
        config.is_enabled(),
        "ping channel requires bdp or keep-alive config",
    );

    let bdp = config.bdp_initial_window.map(|wnd| Bdp {
        bdp: wnd,
        max_bandwidth: 0.0,
        rtt: 0.0,
        ping_delay: Duration::from_millis(100),
        stable_count: 0,
    });

    let (bytes, next_bdp_at) = if bdp.is_some() {
        (Some(0), Some(Instant::now()))
    } else {
        (None, None)
    };

    let keep_alive = config.keep_alive_interval.map(|interval| KeepAlive {
        interval,
        timeout: config.keep_alive_timeout,
        while_idle: config.keep_alive_while_idle,
        sleep: __timer.sleep(interval),
        state: KeepAliveState::Init,
        timer: __timer,
    });

    let last_read_at = keep_alive.as_ref().map(|_| Instant::now());

    let shared = Arc::new(Mutex::new(Shared {
        bytes,
        last_read_at,
        is_keep_alive_timed_out: false,
        ping_pong,
        ping_sent_at: None,
        next_bdp_at,
    }));

    (
        Recorder {
            shared: Some(shared.clone()),
        },
        Ponger {
            bdp,
            keep_alive,
            shared,
        },
    )
}

#[derive(Clone)]
pub(super) struct Config {
    pub(super) bdp_initial_window: Option<WindowSize>,
    /// If no frames are received in this amount of time, a PING frame is sent.
    pub(super) keep_alive_interval: Option<Duration>,
    /// After sending a keepalive PING, the connection will be closed if
    /// a pong is not received in this amount of time.
    pub(super) keep_alive_timeout: Duration,
    /// If true, sends pings even when there are no active streams.
    pub(super) keep_alive_while_idle: bool,
}

#[derive(Clone)]
pub(crate) struct Recorder {
    shared: Option<Arc<Mutex<Shared>>>,
}

pub(super) struct Ponger {
    bdp: Option<Bdp>,
    keep_alive: Option<KeepAlive>,
    shared: Arc<Mutex<Shared>>,
}

struct Shared {
    ping_pong: PingPong,
    ping_sent_at: Option<Instant>,

    // bdp
    /// If `Some`, bdp is enabled, and this tracks how many bytes have been
    /// read during the current sample.
    bytes: Option<usize>,
    /// We delay a variable amount of time between BDP pings. This allows us
    /// to send less pings as the bandwidth stabilizes.
    next_bdp_at: Option<Instant>,

    // keep-alive
    /// If `Some`, keep-alive is enabled, and the Instant is how long ago
    /// the connection read the last frame.
    last_read_at: Option<Instant>,

    is_keep_alive_timed_out: bool,
}

struct Bdp {
    /// Current BDP in bytes
    bdp: u32,
    /// Largest bandwidth we've seen so far.
    max_bandwidth: f64,
    /// Round trip time in seconds
    rtt: f64,
    /// Delay the next ping by this amount.
    ///
    /// This will change depending on how stable the current bandwidth is.
    ping_delay: Duration,
    /// The count of ping round trips where BDP has stayed the same.
    stable_count: u32,
}

struct KeepAlive {
    /// If no frames are received in this amount of time, a PING frame is sent.
    interval: Duration,
    /// After sending a keepalive PING, the connection will be closed if
    /// a pong is not received in this amount of time.
    timeout: Duration,
    /// If true, sends pings even when there are no active streams.
    while_idle: bool,
    state: KeepAliveState,
    sleep: Pin<Box<dyn Sleep>>,
    timer: Time,
}

enum KeepAliveState {
    Init,
    Scheduled(Instant),
    PingSent,
}

pub(super) enum Ponged {
    SizeUpdate(WindowSize),
    KeepAliveTimedOut,
}

#[derive(Debug)]
pub(super) struct KeepAliveTimedOut;

// ===== impl Config =====

impl Config {
    pub(super) fn is_enabled(&self) -> bool {
        self.bdp_initial_window.is_some() || self.keep_alive_interval.is_some()
    }
}

// ===== impl Recorder =====

impl Recorder {
    pub(crate) fn record_data(&self, len: usize) {
        let shared = if let Some(ref shared) = self.shared {
            shared
        } else {
            return;
        };

        let mut locked = shared.lock().unwrap();

        locked.update_last_read_at();

        // are we ready to send another bdp ping?
        // if not, we don't need to record bytes either

        if let Some(ref next_bdp_at) = locked.next_bdp_at {
            if Instant::now() < *next_bdp_at {
                return;
            } else {
                locked.next_bdp_at = None;
            }
        }

        if let Some(ref mut bytes) = locked.bytes {
            *bytes += len;
        } else {
            // no need to send bdp ping if bdp is disabled
            return;
        }

        if !locked.is_ping_sent() {
            locked.send_ping();
        }
    }

    pub(crate) fn record_non_data(&self) {
        let shared = if let Some(ref shared) = self.shared {
            shared
        } else {
            return;
        };

        let mut locked = shared.lock().unwrap();

        locked.update_last_read_at();
    }

    /// If the incoming stream is already closed, convert self into
    /// a disabled reporter.
    #[cfg(feature = "client")]
    pub(super) fn for_stream(self, stream: &h2::RecvStream) -> Self {
        if stream.is_end_stream() {
            disabled()
        } else {
            self
        }
    }

    pub(super) fn ensure_not_timed_out(&self) -> crate::Result<()> {
        if let Some(ref shared) = self.shared {
            let locked = shared.lock().unwrap();
            if locked.is_keep_alive_timed_out {
                return Err(KeepAliveTimedOut.crate_error());
            }
        }

        // else
        Ok(())
    }
}

// ===== impl Ponger =====

impl Ponger {
    pub(super) fn poll(&mut self, cx: &mut task::Context<'_>) -> Poll<Ponged> {
        let now = Instant::now();
        let mut locked = self.shared.lock().unwrap();
        let is_idle = self.is_idle();

        if let Some(ref mut ka) = self.keep_alive {
            ka.maybe_schedule(is_idle, &locked);
            ka.maybe_ping(cx, is_idle, &mut locked);
        }

        if !locked.is_ping_sent() {
            // XXX: this doesn't register a waker...?
            return Poll::Pending;
        }

        match locked.ping_pong.poll_pong(cx) {
            Poll::Ready(Ok(_pong)) => {
                let start = locked
                    .ping_sent_at
                    .expect("pong received implies ping_sent_at");
                locked.ping_sent_at = None;
                let rtt = now - start;
                trace!("recv pong");

                if let Some(ref mut ka) = self.keep_alive {
                    locked.update_last_read_at();
                    ka.maybe_schedule(is_idle, &locked);
                    ka.maybe_ping(cx, is_idle, &mut locked);
                }

                if let Some(ref mut bdp) = self.bdp {
                    let bytes = locked.bytes.expect("bdp enabled implies bytes");
                    locked.bytes = Some(0); // reset
                    trace!("received BDP ack; bytes = {}, rtt = {:?}", bytes, rtt);

                    let update = bdp.calculate(bytes, rtt);
                    locked.next_bdp_at = Some(now + bdp.ping_delay);
                    if let Some(update) = update {
                        return Poll::Ready(Ponged::SizeUpdate(update));
                    }
                }
            }
            Poll::Ready(Err(_e)) => {
                debug!("pong error: {}", _e);
            }
            Poll::Pending => {
                if let Some(ref mut ka) = self.keep_alive {
                    if let Err(KeepAliveTimedOut) = ka.maybe_timeout(cx) {
                        self.keep_alive = None;
                        locked.is_keep_alive_timed_out = true;
                        return Poll::Ready(Ponged::KeepAliveTimedOut);
                    }
                }
            }
        }

        // XXX: this doesn't register a waker...?
        Poll::Pending
    }

    fn is_idle(&self) -> bool {
        Arc::strong_count(&self.shared) <= 2
    }
}

// ===== impl Shared =====

impl Shared {
    fn send_ping(&mut self) {
        match self.ping_pong.send_ping(Ping::opaque()) {
            Ok(()) => {
                self.ping_sent_at = Some(Instant::now());
                trace!("sent ping");
            }
            Err(_err) => {
                debug!("error sending ping: {}", _err);
            }
        }
    }

    fn is_ping_sent(&self) -> bool {
        self.ping_sent_at.is_some()
    }

    fn update_last_read_at(&mut self) {
        if self.last_read_at.is_some() {
            self.last_read_at = Some(Instant::now());
        }
    }

    fn last_read_at(&self) -> Instant {
        self.last_read_at.expect("keep_alive expects last_read_at")
    }
}

// ===== impl Bdp =====

/// Any higher than this likely will be hitting the TCP flow control.
const BDP_LIMIT: usize = 1024 * 1024 * 16;

impl Bdp {
    fn calculate(&mut self, bytes: usize, rtt: Duration) -> Option<WindowSize> {
        // No need to do any math if we're at the limit.
        if self.bdp as usize == BDP_LIMIT {
            self.stabilize_delay();
            return None;
        }

        // average the rtt
        let rtt = seconds(rtt);
        if self.rtt == 0.0 {
            // First sample means rtt is first rtt.
            self.rtt = rtt;
        } else {
            // Weigh this rtt as 1/8 for a moving average.
            self.rtt += (rtt - self.rtt) * 0.125;
        }

        // calculate the current bandwidth
        let bw = (bytes as f64) / (self.rtt * 1.5);
        trace!("current bandwidth = {:.1}B/s", bw);

        if bw < self.max_bandwidth {
            // not a faster bandwidth, so don't update
            self.stabilize_delay();
            return None;
        } else {
            self.max_bandwidth = bw;
        }

        // if the current `bytes` sample is at least 2/3 the previous
        // bdp, increase to double the current sample.
        if bytes >= self.bdp as usize * 2 / 3 {
            self.bdp = (bytes * 2).min(BDP_LIMIT) as WindowSize;
            trace!("BDP increased to {}", self.bdp);

            self.stable_count = 0;
            self.ping_delay /= 2;
            Some(self.bdp)
        } else {
            self.stabilize_delay();
            None
        }
    }

    fn stabilize_delay(&mut self) {
        if self.ping_delay < Duration::from_secs(10) {
            self.stable_count += 1;

            if self.stable_count >= 2 {
                self.ping_delay *= 4;
                self.stable_count = 0;
            }
        }
    }
}

fn seconds(dur: Duration) -> f64 {
    const NANOS_PER_SEC: f64 = 1_000_000_000.0;
    let secs = dur.as_secs() as f64;
    secs + (dur.subsec_nanos() as f64) / NANOS_PER_SEC
}

// ===== impl KeepAlive =====

impl KeepAlive {
    fn maybe_schedule(&mut self, is_idle: bool, shared: &Shared) {
        match self.state {
            KeepAliveState::Init => {
                if !self.while_idle && is_idle {
                    return;
                }

                self.schedule(shared);
            }
            KeepAliveState::PingSent => {
                if shared.is_ping_sent() {
                    return;
                }
                self.schedule(shared);
            }
            KeepAliveState::Scheduled(..) => (),
        }
    }

    fn schedule(&mut self, shared: &Shared) {
        let interval = shared.last_read_at() + self.interval;
        self.state = KeepAliveState::Scheduled(interval);
        self.timer.reset(&mut self.sleep, interval);
    }

    fn maybe_ping(&mut self, cx: &mut task::Context<'_>, is_idle: bool, shared: &mut Shared) {
        match self.state {
            KeepAliveState::Scheduled(at) => {
                if Pin::new(&mut self.sleep).poll(cx).is_pending() {
                    return;
                }
                // check if we've received a frame while we were scheduled
                if shared.last_read_at() + self.interval > at {
                    self.state = KeepAliveState::Init;
                    cx.waker().wake_by_ref(); // schedule us again
                    return;
                }
                if !self.while_idle && is_idle {
                    trace!("keep-alive no need to ping when idle and while_idle=false");
                    return;
                }
                trace!("keep-alive interval ({:?}) reached", self.interval);
                shared.send_ping();
                self.state = KeepAliveState::PingSent;
                let timeout = Instant::now() + self.timeout;
                self.timer.reset(&mut self.sleep, timeout);
            }
            KeepAliveState::Init | KeepAliveState::PingSent => (),
        }
    }

    fn maybe_timeout(&mut self, cx: &mut task::Context<'_>) -> Result<(), KeepAliveTimedOut> {
        match self.state {
            KeepAliveState::PingSent => {
                if Pin::new(&mut self.sleep).poll(cx).is_pending() {
                    return Ok(());
                }
                trace!("keep-alive timeout ({:?}) reached", self.timeout);
                Err(KeepAliveTimedOut)
            }
            KeepAliveState::Init | KeepAliveState::Scheduled(..) => Ok(()),
        }
    }
}

// ===== impl KeepAliveTimedOut =====

impl KeepAliveTimedOut {
    pub(super) fn crate_error(self) -> crate::Error {
        crate::Error::new(crate::error::Kind::Http2).with(self)
    }
}

impl fmt::Display for KeepAliveTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("keep-alive timed out")
    }
}

impl std::error::Error for KeepAliveTimedOut {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&crate::error::TimedOut)
    }
}
