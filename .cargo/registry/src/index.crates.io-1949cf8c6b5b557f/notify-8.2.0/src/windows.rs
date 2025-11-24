#![allow(missing_docs)]
//! Watcher implementation for Windows' directory management APIs
//!
//! For more information see the [ReadDirectoryChangesW reference][ref].
//!
//! [ref]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa363950(v=vs.85).aspx

use crate::{bounded, unbounded, BoundSender, Config, Receiver, Sender};
use crate::{event::*, WatcherKind};
use crate::{Error, EventHandler, RecursiveMode, Result, Watcher};
use std::alloc;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::os::raw::c_void;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex};
use std::thread;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_OPERATION_ABORTED, ERROR_SUCCESS, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_ACTION_ADDED, FILE_ACTION_MODIFIED,
    FILE_ACTION_REMOVED, FILE_ACTION_RENAMED_NEW_NAME, FILE_ACTION_RENAMED_OLD_NAME,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_CREATION, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SECURITY,
    FILE_NOTIFY_CHANGE_SIZE, FILE_NOTIFY_INFORMATION, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Threading::{
    CreateSemaphoreW, ReleaseSemaphore, WaitForSingleObjectEx, INFINITE,
};
use windows_sys::Win32::System::IO::{CancelIo, OVERLAPPED};

const BUF_SIZE: u32 = 16384;

#[derive(Clone)]
struct ReadData {
    dir: PathBuf,          // directory that is being watched
    file: Option<PathBuf>, // if a file is being watched, this is its full path
    complete_sem: HANDLE,
    is_recursive: bool,
}

struct ReadDirectoryRequest {
    event_handler: Arc<Mutex<dyn EventHandler>>,
    buffer: [u8; BUF_SIZE as usize],
    handle: HANDLE,
    data: ReadData,
    action_tx: Sender<Action>,
}

impl ReadDirectoryRequest {
    fn unwatch(&self) {
        let _ = self.action_tx.send(Action::Unwatch(self.data.dir.clone()));
    }
}

enum Action {
    Watch(PathBuf, RecursiveMode),
    Unwatch(PathBuf),
    Stop,
    Configure(Config, BoundSender<Result<bool>>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MetaEvent {
    SingleWatchComplete,
    WatcherAwakened,
}

struct WatchState {
    dir_handle: HANDLE,
    complete_sem: HANDLE,
}

struct ReadDirectoryChangesServer {
    tx: Sender<Action>,
    rx: Receiver<Action>,
    event_handler: Arc<Mutex<dyn EventHandler>>,
    meta_tx: Sender<MetaEvent>,
    cmd_tx: Sender<Result<PathBuf>>,
    watches: HashMap<PathBuf, WatchState>,
    wakeup_sem: HANDLE,
}

impl ReadDirectoryChangesServer {
    fn start(
        event_handler: Arc<Mutex<dyn EventHandler>>,
        meta_tx: Sender<MetaEvent>,
        cmd_tx: Sender<Result<PathBuf>>,
        wakeup_sem: HANDLE,
    ) -> Sender<Action> {
        let (action_tx, action_rx) = unbounded();
        // it is, in fact, ok to send the semaphore across threads
        let sem_temp = wakeup_sem as u64;
        let _ = thread::Builder::new()
            .name("notify-rs windows loop".to_string())
            .spawn({
                let tx = action_tx.clone();
                move || {
                    let wakeup_sem = sem_temp as HANDLE;
                    let server = ReadDirectoryChangesServer {
                        tx,
                        rx: action_rx,
                        event_handler,
                        meta_tx,
                        cmd_tx,
                        watches: HashMap::new(),
                        wakeup_sem,
                    };
                    server.run();
                }
            });
        action_tx
    }

    fn run(mut self) {
        loop {
            // process all available actions first
            let mut stopped = false;

            while let Ok(action) = self.rx.try_recv() {
                match action {
                    Action::Watch(path, recursive_mode) => {
                        let res = self.add_watch(path, recursive_mode.is_recursive());
                        let _ = self.cmd_tx.send(res);
                    }
                    Action::Unwatch(path) => self.remove_watch(path),
                    Action::Stop => {
                        stopped = true;
                        for ws in self.watches.values() {
                            stop_watch(ws, &self.meta_tx);
                        }
                        break;
                    }
                    Action::Configure(config, tx) => {
                        self.configure_raw_mode(config, tx);
                    }
                }
            }

            if stopped {
                break;
            }

            unsafe {
                // wait with alertable flag so that the completion routine fires
                let waitres = WaitForSingleObjectEx(self.wakeup_sem, 100, 1);
                if waitres == WAIT_OBJECT_0 {
                    let _ = self.meta_tx.send(MetaEvent::WatcherAwakened);
                }
            }
        }

        // we have to clean this up, since the watcher may be long gone
        unsafe {
            CloseHandle(self.wakeup_sem);
        }
    }

    fn add_watch(&mut self, path: PathBuf, is_recursive: bool) -> Result<PathBuf> {
        // path must exist and be either a file or directory
        if !path.is_dir() && !path.is_file() {
            return Err(
                Error::generic("Input watch path is neither a file nor a directory.")
                    .add_path(path),
            );
        }

        let (watching_file, dir_target) = {
            if path.is_dir() {
                (false, path.clone())
            } else {
                // emulate file watching by watching the parent directory
                (true, path.parent().unwrap().to_path_buf())
            }
        };

        let encoded_path: Vec<u16> = dir_target
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let handle;
        unsafe {
            handle = CreateFileW(
                encoded_path.as_ptr(),
                FILE_LIST_DIRECTORY,
                FILE_SHARE_READ | FILE_SHARE_DELETE | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            );

            if handle == INVALID_HANDLE_VALUE {
                return Err(if watching_file {
                    Error::generic(
                        "You attempted to watch a single file, but parent \
                         directory could not be opened.",
                    )
                    .add_path(path)
                } else {
                    // TODO: Call GetLastError for better error info?
                    Error::path_not_found().add_path(path)
                });
            }
        }
        let wf = if watching_file {
            Some(path.clone())
        } else {
            None
        };
        // every watcher gets its own semaphore to signal completion
        let semaphore = unsafe { CreateSemaphoreW(ptr::null_mut(), 0, 1, ptr::null_mut()) };
        if semaphore.is_null() || semaphore == INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(handle);
            }
            return Err(Error::generic("Failed to create semaphore for watch.").add_path(path));
        }
        let rd = ReadData {
            dir: dir_target,
            file: wf,
            complete_sem: semaphore,
            is_recursive,
        };
        let ws = WatchState {
            dir_handle: handle,
            complete_sem: semaphore,
        };
        self.watches.insert(path.clone(), ws);
        start_read(&rd, self.event_handler.clone(), handle, self.tx.clone());
        Ok(path)
    }

    fn remove_watch(&mut self, path: PathBuf) {
        if let Some(ws) = self.watches.remove(&path) {
            stop_watch(&ws, &self.meta_tx);
        }
    }

    fn configure_raw_mode(&mut self, _config: Config, tx: BoundSender<Result<bool>>) {
        tx.send(Ok(false))
            .expect("configuration channel disconnect");
    }
}

fn stop_watch(ws: &WatchState, meta_tx: &Sender<MetaEvent>) {
    unsafe {
        let cio = CancelIo(ws.dir_handle);
        let ch = CloseHandle(ws.dir_handle);
        // have to wait for it, otherwise we leak the memory allocated for there read request
        if cio != 0 && ch != 0 {
            while WaitForSingleObjectEx(ws.complete_sem, INFINITE, 1) != WAIT_OBJECT_0 {
                // drain the apc queue, fix for https://github.com/notify-rs/notify/issues/287#issuecomment-801465550
            }
        }
        CloseHandle(ws.complete_sem);
    }
    let _ = meta_tx.send(MetaEvent::SingleWatchComplete);
}

fn start_read(
    rd: &ReadData,
    event_handler: Arc<Mutex<dyn EventHandler>>,
    handle: HANDLE,
    action_tx: Sender<Action>,
) {
    let request = Box::new(ReadDirectoryRequest {
        event_handler,
        handle,
        buffer: [0u8; BUF_SIZE as usize],
        data: rd.clone(),
        action_tx,
    });

    let flags = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_ATTRIBUTES
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_CREATION
        | FILE_NOTIFY_CHANGE_SECURITY;

    let monitor_subdir = if request.data.file.is_none() && request.data.is_recursive {
        1
    } else {
        0
    };

    unsafe {
        let overlapped = alloc::alloc_zeroed(alloc::Layout::new::<OVERLAPPED>()) as *mut OVERLAPPED;
        // When using callback based async requests, we are allowed to use the hEvent member
        // for our own purposes

        let request = Box::leak(request);
        (*overlapped).hEvent = request as *mut _ as _;

        // This is using an asynchronous call with a completion routine for receiving notifications
        // An I/O completion port would probably be more performant
        let ret = ReadDirectoryChangesW(
            handle,
            request.buffer.as_mut_ptr() as *mut c_void,
            BUF_SIZE,
            monitor_subdir,
            flags,
            &mut 0u32 as *mut u32, // not used for async reqs
            overlapped,
            Some(handle_event),
        );

        if ret == 0 {
            // error reading. retransmute request memory to allow drop.
            // Because of the error, ownership of the `overlapped` alloc was not passed
            // over to `ReadDirectoryChangesW`.
            // So we can claim ownership back.
            let _overlapped = Box::from_raw(overlapped);
            let request = Box::from_raw(request);
            ReleaseSemaphore(request.data.complete_sem, 1, ptr::null_mut());
        }
    }
}

unsafe extern "system" fn handle_event(
    error_code: u32,
    _bytes_written: u32,
    overlapped: *mut OVERLAPPED,
) {
    let overlapped: Box<OVERLAPPED> = Box::from_raw(overlapped);
    let request: Box<ReadDirectoryRequest> = Box::from_raw(overlapped.hEvent as *mut _);

    match error_code {
        ERROR_OPERATION_ABORTED => {
            // received when dir is unwatched or watcher is shutdown; return and let overlapped/request get drop-cleaned
            ReleaseSemaphore(request.data.complete_sem, 1, ptr::null_mut());
            return;
        }
        ERROR_ACCESS_DENIED => {
            // This could happen when the watched directory is deleted or trashed, first check if it's the case.
            // If so, unwatch the directory and return, otherwise, continue to handle the event.
            if !request.data.dir.exists() {
                request.unwatch();
                ReleaseSemaphore(request.data.complete_sem, 1, ptr::null_mut());
                return;
            }
        }
        ERROR_SUCCESS => {
            // Success, continue to handle the event
        }
        _ => {
            // Some unidentified error occurred, log and unwatch the directory, then return.
            log::error!(
                "unknown error in ReadDirectoryChangesW for directory {}: {}",
                request.data.dir.display(),
                error_code
            );
            request.unwatch();
            ReleaseSemaphore(request.data.complete_sem, 1, ptr::null_mut());
            return;
        }
    }

    // Get the next request queued up as soon as possible
    start_read(
        &request.data,
        request.event_handler.clone(),
        request.handle,
        request.action_tx,
    );

    // The FILE_NOTIFY_INFORMATION struct has a variable length due to the variable length
    // string as its last member. Each struct contains an offset for getting the next entry in
    // the buffer.
    let mut cur_offset: *const u8 = request.buffer.as_ptr();
    // In Wine, FILE_NOTIFY_INFORMATION structs are packed placed in the buffer;
    // they are aligned to 16bit (WCHAR) boundary instead of 32bit required by FILE_NOTIFY_INFORMATION.
    // Hence, we need to use `read_unaligned` here to avoid UB.
    let mut cur_entry = ptr::read_unaligned(cur_offset as *const FILE_NOTIFY_INFORMATION);
    loop {
        // filename length is size in bytes, so / 2
        let len = cur_entry.FileNameLength as usize / 2;
        let encoded_path: &[u16] = slice::from_raw_parts(
            cur_offset.offset(std::mem::offset_of!(FILE_NOTIFY_INFORMATION, FileName) as isize)
                as _,
            len,
        );
        // prepend root to get a full path
        let path = request
            .data
            .dir
            .join(PathBuf::from(OsString::from_wide(encoded_path)));

        // if we are watching a single file, ignore the event unless the path is exactly
        // the watched file
        let skip = match request.data.file {
            None => false,
            Some(ref watch_path) => *watch_path != path,
        };

        if !skip {
            log::trace!(
                "Event: path = `{}`, action = {:?}",
                path.display(),
                cur_entry.Action
            );

            let newe = Event::new(EventKind::Any).add_path(path);

            fn emit_event(event_handler: &Mutex<dyn EventHandler>, res: Result<Event>) {
                if let Ok(mut guard) = event_handler.lock() {
                    let f: &mut dyn EventHandler = &mut *guard;
                    f.handle_event(res);
                }
            }

            let event_handler = |res| emit_event(&request.event_handler, res);

            if cur_entry.Action == FILE_ACTION_RENAMED_OLD_NAME {
                let mode = RenameMode::From;
                let kind = ModifyKind::Name(mode);
                let kind = EventKind::Modify(kind);
                let ev = newe.set_kind(kind);
                event_handler(Ok(ev))
            } else {
                match cur_entry.Action {
                    FILE_ACTION_RENAMED_NEW_NAME => {
                        let kind = EventKind::Modify(ModifyKind::Name(RenameMode::To));
                        let ev = newe.set_kind(kind);
                        event_handler(Ok(ev));
                    }
                    FILE_ACTION_ADDED => {
                        let kind = EventKind::Create(CreateKind::Any);
                        let ev = newe.set_kind(kind);
                        event_handler(Ok(ev));
                    }
                    FILE_ACTION_REMOVED => {
                        let kind = EventKind::Remove(RemoveKind::Any);
                        let ev = newe.set_kind(kind);
                        event_handler(Ok(ev));
                    }
                    FILE_ACTION_MODIFIED => {
                        let kind = EventKind::Modify(ModifyKind::Any);
                        let ev = newe.set_kind(kind);
                        event_handler(Ok(ev));
                    }
                    _ => (),
                };
            }
        }

        if cur_entry.NextEntryOffset == 0 {
            break;
        }
        cur_offset = cur_offset.offset(cur_entry.NextEntryOffset as isize);
        cur_entry = ptr::read_unaligned(cur_offset as *const FILE_NOTIFY_INFORMATION);
    }
}

/// Watcher implementation based on ReadDirectoryChanges
#[derive(Debug)]
pub struct ReadDirectoryChangesWatcher {
    tx: Sender<Action>,
    cmd_rx: Receiver<Result<PathBuf>>,
    wakeup_sem: HANDLE,
}

impl ReadDirectoryChangesWatcher {
    pub fn create(
        event_handler: Arc<Mutex<dyn EventHandler>>,
        meta_tx: Sender<MetaEvent>,
    ) -> Result<ReadDirectoryChangesWatcher> {
        let (cmd_tx, cmd_rx) = unbounded();

        let wakeup_sem = unsafe { CreateSemaphoreW(ptr::null_mut(), 0, 1, ptr::null_mut()) };
        if wakeup_sem.is_null() || wakeup_sem == INVALID_HANDLE_VALUE {
            return Err(Error::generic("Failed to create wakeup semaphore."));
        }

        let action_tx =
            ReadDirectoryChangesServer::start(event_handler, meta_tx, cmd_tx, wakeup_sem);

        Ok(ReadDirectoryChangesWatcher {
            tx: action_tx,
            cmd_rx,
            wakeup_sem,
        })
    }

    fn wakeup_server(&mut self) {
        // breaks the server out of its wait state.  right now this is really just an optimization,
        // so that if you add a watch you don't block for 100ms in watch() while the
        // server sleeps.
        unsafe {
            ReleaseSemaphore(self.wakeup_sem, 1, ptr::null_mut());
        }
    }

    fn send_action_require_ack(&mut self, action: Action, pb: &PathBuf) -> Result<()> {
        self.tx
            .send(action)
            .map_err(|_| Error::generic("Error sending to internal channel"))?;

        // wake 'em up, we don't want to wait around for the ack
        self.wakeup_server();

        let ack_pb = self
            .cmd_rx
            .recv()
            .map_err(|_| Error::generic("Error receiving from command channel"))?
            .map_err(|e| Error::generic(&format!("Error in watcher: {:?}", e)))?;

        if pb.as_path() != ack_pb.as_path() {
            Err(Error::generic(&format!(
                "Expected ack for {:?} but got \
                 ack for {:?}",
                pb, ack_pb
            )))
        } else {
            Ok(())
        }
    }

    fn watch_inner(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        let pb = if path.is_absolute() {
            path.to_owned()
        } else {
            let p = env::current_dir().map_err(Error::io)?;
            p.join(path)
        };
        // path must exist and be either a file or directory
        if !pb.is_dir() && !pb.is_file() {
            return Err(Error::generic(
                "Input watch path is neither a file nor a directory.",
            ));
        }
        self.send_action_require_ack(Action::Watch(pb.clone(), recursive_mode), &pb)
    }

    fn unwatch_inner(&mut self, path: &Path) -> Result<()> {
        let pb = if path.is_absolute() {
            path.to_owned()
        } else {
            let p = env::current_dir().map_err(Error::io)?;
            p.join(path)
        };
        let res = self
            .tx
            .send(Action::Unwatch(pb))
            .map_err(|_| Error::generic("Error sending to internal channel"));
        self.wakeup_server();
        res
    }
}

impl Watcher for ReadDirectoryChangesWatcher {
    fn new<F: EventHandler>(event_handler: F, _config: Config) -> Result<Self> {
        // create dummy channel for meta event
        // TODO: determine the original purpose of this - can we remove it?
        let (meta_tx, _) = unbounded();
        let event_handler = Arc::new(Mutex::new(event_handler));
        Self::create(event_handler, meta_tx)
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        self.watch_inner(path, recursive_mode)
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.unwatch_inner(path)
    }

    fn configure(&mut self, config: Config) -> Result<bool> {
        let (tx, rx) = bounded(1);
        self.tx.send(Action::Configure(config, tx))?;
        rx.recv()?
    }

    fn kind() -> crate::WatcherKind {
        WatcherKind::ReadDirectoryChangesWatcher
    }
}

impl Drop for ReadDirectoryChangesWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Action::Stop);
        // better wake it up
        self.wakeup_server();
    }
}

// `ReadDirectoryChangesWatcher` is not Send/Sync because of the semaphore Handle.
// As said elsewhere it's perfectly safe to send it across threads.
unsafe impl Send for ReadDirectoryChangesWatcher {}
// Because all public methods are `&mut self` it's also perfectly safe to share references.
unsafe impl Sync for ReadDirectoryChangesWatcher {}
