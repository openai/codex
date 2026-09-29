//! One session-lived worker owns blocking clipboard operations and native leases.
//! Setup has a five-second budget, but abandoning it cannot interrupt an OS call.
//! There is no general backlog: the worker stays busy until that call returns. Only the
//! latest selection publication and one PRIMARY read can wait, with publication ordered
//! first. Their UI owners cancel by dropping their scopes. Once delivery starts it must
//! finish. Text reads have the same budget for the whole operation; late results are
//! discarded. Terminal escape sequences are emitted by the UI.

use super::ClipboardLease;
use super::CopyFormat;
use super::CopyStatus;
use crate::tui::FrameRequester;
use std::cell::Cell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

const SETUP_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
const SETUP_TIMEOUT_MESSAGE: &str = "clipboard setup timed out; copy abandoned";

enum Setup {
    Pending(Instant),
    Delivering,
    Abandoned,
}

impl Setup {
    fn timed_out(&mut self, now: Instant) -> bool {
        if matches!(self, Self::Pending(deadline) if now >= *deadline) {
            *self = Self::Abandoned;
        }
        matches!(self, Self::Abandoned)
    }
}

/// The UI deadline and worker delivery decision share one short, non-I/O critical section.
struct CopySetup {
    phase: Mutex<Setup>,
    frames: FrameRequester,
    owner: Option<Weak<()>>,
}

impl CopySetup {
    fn begin_delivery(&self) -> Result<(), String> {
        let mut setup = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if setup.timed_out(Instant::now()) {
            return Err(SETUP_TIMEOUT_MESSAGE.into());
        }
        if matches!(*setup, Setup::Pending(_))
            && self
                .owner
                .as_ref()
                .is_some_and(|owner| owner.strong_count() == 0)
        {
            return Err("selection ended before clipboard delivery".into());
        }
        *setup = Setup::Delivering;
        Ok(())
    }

    fn timed_out(&self) -> bool {
        let mut setup = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let timed_out = setup.timed_out(Instant::now());
        if let Setup::Pending(deadline) = *setup {
            // Earlier draws consume scheduled frames, so re-arm after every UI poll.
            self.frames
                .schedule_frame_in(deadline.saturating_duration_since(Instant::now()));
        }
        timed_out
    }
}

pub(crate) type CopyResult = Result<CopyStatus, String>;

/// A mouse selection may also retain the configured automatic clipboard copy.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum CopyDestination {
    Clipboard,
    Primary,
    ClipboardAndPrimary(Arc<str>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteSource {
    Clipboard,
    Primary,
}

enum Request {
    Copy {
        text: Arc<str>,
        format: CopyFormat,
        destination: CopyDestination,
        setup: Arc<CopySetup>,
    },
    Read {
        source: PasteSource,
        deadline: Instant,
        response: mpsc::Sender<Result<String, String>>,
    },
}

struct PendingRead {
    frames: FrameRequester,
    deadline: Instant,
    source: PasteSource,
    owner: Weak<()>,
    response: Option<mpsc::Receiver<Result<String, String>>>,
    expired: bool,
}

struct Response {
    result: CopyResult,
    terminal_text: Option<String>,
}

#[derive(Default)]
pub(crate) struct ClipboardWorker {
    requests: Option<mpsc::Sender<Request>>,
    responses: Option<mpsc::Receiver<Response>>,
    pending: Option<(u64, Arc<CopySetup>)>,
    next_id: u64,
    completed: Option<(u64, CopyResult)>,
    primary: Option<(Arc<str>, Weak<()>)>,
    pending_read: Option<PendingRead>,
    read_result: Option<(Instant, Weak<()>, Result<String, String>)>,
}

impl ClipboardWorker {
    pub(crate) fn is_busy(&self) -> bool {
        self.pending.is_some()
            || self
                .pending_read
                .as_ref()
                .is_some_and(|read| read.response.is_some())
    }

    pub(crate) fn copy(
        &mut self,
        text: Arc<str>,
        format: CopyFormat,
        frames: FrameRequester,
    ) -> CopyResult {
        self.copy_to(
            text,
            format,
            CopyDestination::Clipboard,
            frames,
            /*owner*/ None,
        )
    }

    /// On a busy automatic copy, retain only PRIMARY, never defer a CLIPBOARD copy.
    pub(crate) fn select(
        &mut self,
        text: Arc<str>,
        format: CopyFormat,
        destination: CopyDestination,
        frames: FrameRequester,
    ) -> (CopyResult, Arc<()>) {
        let owner = Arc::new(());
        let primary = match &destination {
            CopyDestination::Clipboard => None,
            CopyDestination::Primary => Some(Arc::clone(&text)),
            CopyDestination::ClipboardAndPrimary(plain_text) => Some(Arc::clone(plain_text)),
        };
        self.primary = None;
        let scope = primary.as_ref().map(|_| Arc::downgrade(&owner));
        let result = self.copy_to(text, format, destination, frames, scope);
        if result == Ok(CopyStatus::Busy) {
            self.primary = primary.map(|text| (text, Arc::downgrade(&owner)));
        }
        (result, owner)
    }

    fn copy_to(
        &mut self,
        text: Arc<str>,
        format: CopyFormat,
        destination: CopyDestination,
        frames: FrameRequester,
        owner: Option<Weak<()>>,
    ) -> CopyResult {
        if self.is_busy() {
            return Ok(CopyStatus::Busy);
        }
        if text.is_empty() {
            return Err("nothing to copy: the selected content is empty".into());
        }
        self.ensure_started(frames.clone())?;
        self.next_id += 1;
        let id = self.next_id;
        let setup = Arc::new(CopySetup {
            phase: Mutex::new(Setup::Pending(Instant::now() + SETUP_TIMEOUT)),
            frames: frames.clone(),
            owner,
        });
        self.requests
            .as_ref()
            .ok_or("clipboard worker stopped")?
            .send(Request::Copy {
                text,
                format,
                destination,
                setup: Arc::clone(&setup),
            })
            .map_err(|_| "clipboard worker stopped".to_string())?;
        self.pending = Some((id, setup));
        // Retain the last completion while a newer request runs so a temporarily hidden
        // view can still finish its feedback. Consumers already match request IDs.
        frames.schedule_frame_in(SETUP_TIMEOUT);
        Ok(CopyStatus::Pending(id))
    }

    fn ensure_started(&mut self, frames: FrameRequester) -> Result<(), String> {
        if self.requests.is_none() {
            self.start(
                frames,
                |text, format, setup| {
                    let terminal_text = Cell::new(/*value*/ None);
                    let result = super::copy_to_clipboard(
                        text,
                        format,
                        || setup.begin_delivery(),
                        |text| {
                            // Validate the limit before accepting a deferred terminal send.
                            super::osc52_sequence(text, std::env::var_os("TMUX").is_some())?;
                            terminal_text.set(Some(text.to_owned()));
                            Ok(())
                        },
                    );
                    (result, terminal_text.into_inner())
                },
                crate::clipboard_paste::text::read,
            )?;
        }
        Ok(())
    }

    /// Accept one text read. PRIMARY may wait behind its publication; CLIPBOARD never queues.
    /// The caller must retain the scope until it consumes or cancels the read.
    pub(crate) fn read_text(
        &mut self,
        source: PasteSource,
        frames: FrameRequester,
    ) -> Result<Option<Arc<()>>, String> {
        if self.pending_read.is_some() || (source == PasteSource::Clipboard && self.is_busy()) {
            return Ok(None);
        }
        self.ensure_started(frames.clone())?;
        let deadline = Instant::now() + SETUP_TIMEOUT;
        let owner = Arc::new(());
        let wait = self.is_busy() || (source == PasteSource::Primary && self.primary.is_some());
        self.pending_read = Some(PendingRead {
            frames: frames.clone(),
            deadline,
            source,
            owner: Arc::downgrade(&owner),
            response: None,
            expired: false,
        });
        self.read_result = None;
        if !wait {
            self.start_read()?;
        }
        frames.schedule_frame_in(SETUP_TIMEOUT);
        Ok(Some(owner))
    }

    fn start_read(&mut self) -> Result<(), String> {
        let read = self
            .pending_read
            .as_mut()
            .ok_or("no clipboard read pending")?;
        let (response, receiver) = mpsc::channel();
        self.requests
            .as_ref()
            .ok_or("clipboard worker stopped")?
            .send(Request::Read {
                source: read.source,
                deadline: read.deadline,
                response,
            })
            .map_err(|_| "clipboard worker stopped".to_string())?;
        read.response = Some(receiver);
        Ok(())
    }

    pub(crate) fn take_text_result(&mut self) -> Option<Result<String, String>> {
        let (deadline, owner, result) = self.read_result.take()?;
        (owner.strong_count() > 0).then(|| {
            if Instant::now() >= deadline {
                Err("clipboard read timed out".into())
            } else {
                result
            }
        })
    }

    /// Run deferred work only at an idle draw boundary, after the UI validates its owners.
    /// Polling a completion before a key must never preempt that key with an automatic copy.
    pub(crate) fn advance(&mut self, frames: FrameRequester) {
        if self.is_busy() {
            return;
        }
        // Target validation may have dropped the scope after this draw's poll.
        if self
            .pending_read
            .as_ref()
            .is_some_and(|read| read.owner.strong_count() == 0)
        {
            self.pending_read = None;
        }
        if let Some((text, owner)) = self.primary.take()
            && owner.strong_count() > 0
        {
            let _ = self.copy_to(
                text,
                CopyFormat::PlainText,
                CopyDestination::Primary,
                frames,
                Some(owner),
            );
        }
        if !self.is_busy()
            && self.pending_read.is_some()
            && let Err(error) = self.start_read()
            && let Some(read) = self.pending_read.take()
        {
            self.read_result = Some((read.deadline, read.owner, Err(error)));
        }
    }

    fn start(
        &mut self,
        frames: FrameRequester,
        mut copy: impl FnMut(
            &str,
            CopyFormat,
            &CopySetup,
        ) -> (Result<super::CopyOutcome, String>, Option<String>)
        + Send
        + 'static,
        mut read: impl FnMut(Instant) -> Result<String, String> + Send + 'static,
    ) -> Result<(), String> {
        let (requests, incoming) = mpsc::channel::<Request>();
        let (outgoing, responses) = mpsc::channel();
        // Native calls can wait indefinitely; shutdown waits only for a bounded handoff.
        std::thread::Builder::new()
            .name("clipboard-copy".into())
            .spawn(move || {
                let mut lease: Option<ClipboardLease> = None;
                // An ordinary copy must not relinquish PRIMARY, or vice versa.
                let mut primary_lease: Option<ClipboardLease> = None;
                while let Ok(request) = incoming.recv() {
                    let (outcome, terminal_text) = match request {
                        Request::Copy {
                            text,
                            format,
                            destination,
                            setup,
                        } => {
                            let plain_text = match &destination {
                                CopyDestination::Clipboard => None,
                                CopyDestination::Primary => Some(&text),
                                CopyDestination::ClipboardAndPrimary(plain_text) => {
                                    Some(plain_text)
                                }
                            };
                            let primary = plain_text.map(|text| {
                                super::primary::copy(text, || setup.begin_delivery())
                                    .map(|outcome| outcome.store(&mut primary_lease))
                            });
                            match primary {
                                Some(result) if destination == CopyDestination::Primary => {
                                    (result.map(|_| super::CopyOutcome::Requested), None)
                                }
                                // Unavailable PRIMARY must not break configured CLIPBOARD copy.
                                _ => copy(&text, format, &setup),
                            }
                        }
                        Request::Read {
                            source,
                            deadline,
                            response,
                        } => {
                            let result = match source {
                                PasteSource::Clipboard => read(deadline),
                                PasteSource::Primary => super::primary::read(deadline),
                            };
                            let _ = response.send(result);
                            frames.schedule_frame();
                            continue;
                        }
                    };
                    let result = outcome.map(|outcome| outcome.store(&mut lease));
                    if outgoing
                        .send(Response {
                            result,
                            terminal_text,
                        })
                        .is_err()
                    {
                        break;
                    }
                    frames.schedule_frame();
                }
                // Finish Linux clipboard-manager handoff before disconnecting the response channel.
                #[cfg(target_os = "linux")]
                drop((lease, primary_lease));
                drop(outgoing);
            })
            .map_err(|error| format!("could not start clipboard worker: {error}"))?;
        self.requests = Some(requests);
        self.responses = Some(responses);
        Ok(())
    }

    /// Poll only while the UI owns the terminal. Retain one result for its original consumer.
    pub(crate) fn poll(&mut self) -> Option<&(u64, CopyResult)> {
        if let Some(read) = &mut self.pending_read {
            if read.owner.strong_count() == 0 {
                read.expired = true;
            }
            if !read.expired && Instant::now() >= read.deadline {
                read.expired = true;
                self.read_result = Some((
                    read.deadline,
                    read.owner.clone(),
                    Err("clipboard read timed out".into()),
                ));
            }
            match read.response.as_ref().map(mpsc::Receiver::try_recv) {
                Some(Ok(result)) => {
                    if !read.expired {
                        self.read_result = Some((read.deadline, read.owner.clone(), result));
                    }
                    self.pending_read = None;
                }
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    if !read.expired {
                        self.read_result = Some((
                            read.deadline,
                            read.owner.clone(),
                            Err("clipboard worker stopped".into()),
                        ));
                    }
                    self.pending_read = None;
                }
                None if read.expired => self.pending_read = None,
                Some(Err(mpsc::TryRecvError::Empty)) | None => {
                    if !read.expired {
                        read.frames.schedule_frame_in(
                            read.deadline.saturating_duration_since(Instant::now()),
                        );
                    }
                }
            }
        }
        if let Some((id, setup)) = &self.pending {
            let id = *id;
            if !matches!(self.completed, Some((completed_id, _)) if completed_id == id)
                && setup.timed_out()
            {
                self.completed = Some((id, Err(SETUP_TIMEOUT_MESSAGE.into())));
            }
            match self.responses.as_ref()?.try_recv() {
                Ok(response) => {
                    // A timeout completes the request, not the blocked worker. Drain its
                    // eventual response without replacing failure or emitting terminal output.
                    if matches!(self.completed, Some((completed_id, _)) if completed_id == id) {
                        self.pending = None;
                        return self.completed.as_ref();
                    }
                    let mut result = response.result;
                    if let Some(text) = response.terminal_text
                        && let Err(error) = super::osc52_copy(&text)
                        && result != Ok(CopyStatus::Confirmed)
                    {
                        result = Err(error);
                    }
                    self.completed = Some((id, result));
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !matches!(self.completed, Some((completed_id, _)) if completed_id == id) {
                        self.completed = Some((id, Err("clipboard worker stopped".into())));
                    }
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        self.completed.as_ref()
    }
}

impl Drop for ClipboardWorker {
    fn drop(&mut self) {
        self.requests.take();
        // A confirmed Linux copy needs time to hand ownership to the clipboard manager.
        // Keep this bounded: even dropping a native clipboard can stall on an X11 server.
        if let Some(responses) = &self.responses {
            let deadline = Instant::now() + Duration::from_millis(/*millis*/ 250);
            // A queued result does not mean the lease has finished handing off. Only
            // channel disconnection follows that cleanup, including unpolled copies.
            while responses
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .is_ok()
            {}
        }
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
