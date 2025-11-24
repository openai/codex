use crate::Instant;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) enum Timer {
    /// When to send an ack-eliciting probe packet or declare unacked packets lost
    LossDetection = 0,
    /// When to close the connection after no activity
    Idle = 1,
    /// When the close timer expires, the connection has been gracefully terminated.
    Close = 2,
    /// When keys are discarded because they should not be needed anymore
    KeyDiscard = 3,
    /// When to give up on validating a new path to the peer
    PathValidation = 4,
    /// When to send a `PING` frame to keep the connection alive
    KeepAlive = 5,
    /// When pacing will allow us to send a packet
    Pacing = 6,
    /// When to invalidate old CID and proactively push new one via NEW_CONNECTION_ID frame
    PushNewCid = 7,
    /// When to send an immediate ACK if there are unacked ack-eliciting packets of the peer
    MaxAckDelay = 8,
}

impl Timer {
    pub(crate) const VALUES: [Self; 9] = [
        Self::LossDetection,
        Self::Idle,
        Self::Close,
        Self::KeyDiscard,
        Self::PathValidation,
        Self::KeepAlive,
        Self::Pacing,
        Self::PushNewCid,
        Self::MaxAckDelay,
    ];
}

/// A table of data associated with each distinct kind of `Timer`
#[derive(Debug, Copy, Clone, Default)]
pub(crate) struct TimerTable {
    data: [Option<Instant>; 10],
}

impl TimerTable {
    pub(super) fn set(&mut self, timer: Timer, time: Instant) {
        self.data[timer as usize] = Some(time);
    }

    pub(super) fn get(&self, timer: Timer) -> Option<Instant> {
        self.data[timer as usize]
    }

    pub(super) fn stop(&mut self, timer: Timer) {
        self.data[timer as usize] = None;
    }

    pub(super) fn next_timeout(&self) -> Option<Instant> {
        self.data.iter().filter_map(|&x| x).min()
    }

    pub(super) fn is_expired(&self, timer: Timer, after: Instant) -> bool {
        self.data[timer as usize].is_some_and(|x| x <= after)
    }
}
