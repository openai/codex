//! One session-lived worker owns blocking clipboard operations and native leases.
//! Setup has a five-second budget, but abandoning it cannot interrupt an OS call.
//! There is no backlog: the worker stays busy until that call returns. Once delivery
//! starts it must finish. Text reads have the same budget for the whole operation; late
//! results are discarded. Terminal escape sequences are emitted by the UI.

use super::ClipboardLease;
use super::CopyFormat;
use super::CopyStatus;
use crate::tui::FrameRequester;
use std::cell::Cell;
use std::sync::Arc;
use std::sync::Mutex;
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

enum Request {
    Copy {
        text: Arc<str>,
        format: CopyFormat,
        setup: Arc<CopySetup>,
    },
    Read {
        deadline: Instant,
        response: mpsc::Sender<Result<String, String>>,
    },
}

struct PendingRead {
    frames: FrameRequester,
    deadline: Instant,
    response: mpsc::Receiver<Result<String, String>>,
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
    pending_read: Option<PendingRead>,
    read_result: Option<(Instant, Result<String, String>)>,
}

impl ClipboardWorker {
    pub(crate) fn is_busy(&self) -> bool {
        self.pending.is_some() || self.pending_read.is_some()
    }

    pub(crate) fn copy(
        &mut self,
        text: Arc<str>,
        format: CopyFormat,
        frames: FrameRequester,
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
        });
        self.requests
            .as_ref()
            .ok_or("clipboard worker stopped")?
            .send(Request::Copy {
                text,
                format,
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

    /// Start one text read. A timed-out native call keeps the shared worker occupied.
    pub(crate) fn read_text(&mut self, frames: FrameRequester) -> Result<bool, String> {
        if self.is_busy() {
            return Ok(false);
        }
        self.ensure_started(frames.clone())?;
        let deadline = Instant::now() + SETUP_TIMEOUT;
        let (response, receiver) = mpsc::channel();
        self.requests
            .as_ref()
            .ok_or("clipboard worker stopped")?
            .send(Request::Read { deadline, response })
            .map_err(|_| "clipboard worker stopped".to_string())?;
        self.pending_read = Some(PendingRead {
            frames: frames.clone(),
            deadline,
            response: receiver,
            expired: false,
        });
        self.read_result = None;
        frames.schedule_frame_in(SETUP_TIMEOUT);
        Ok(true)
    }

    pub(crate) fn take_text_result(&mut self) -> Option<Result<String, String>> {
        self.read_result.take().map(|(deadline, result)| {
            if Instant::now() >= deadline {
                Err("clipboard read timed out".into())
            } else {
                result
            }
        })
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
                while let Ok(request) = incoming.recv() {
                    let (outcome, terminal_text) = match request {
                        Request::Copy {
                            text,
                            format,
                            setup,
                        } => copy(&text, format, &setup),
                        Request::Read { deadline, response } => {
                            let _ = response.send(read(deadline));
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
                drop(lease);
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
            if !read.expired && Instant::now() >= read.deadline {
                read.expired = true;
                self.read_result = Some((read.deadline, Err("clipboard read timed out".into())));
            }
            match read.response.try_recv() {
                Ok(result) => {
                    if !read.expired {
                        self.read_result = Some((read.deadline, result));
                    }
                    self.pending_read = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !read.expired {
                        self.read_result =
                            Some((read.deadline, Err("clipboard worker stopped".into())));
                    }
                    self.pending_read = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
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
