//! Watcher implementation for the kqueue API
//!
//! The kqueue() system call provides a generic method of notifying the user
//! when an event happens or a condition holds, based on the results of small
//! pieces of kernel code termed filters.

use super::event::*;
use super::{Config, Error, EventHandler, RecursiveMode, Result, Watcher};
use crate::{unbounded, Receiver, Sender};
use kqueue::{EventData, EventFilter, FilterFlag, Ident};
use std::collections::HashMap;
use std::env;
use std::fs::metadata;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use walkdir::WalkDir;

const KQUEUE: mio::Token = mio::Token(0);
const MESSAGE: mio::Token = mio::Token(1);

// The EventLoop will set up a mio::Poll and use it to wait for the following:
//
// -  messages telling it what to do
//
// -  events telling it that something has happened on one of the watched files.
struct EventLoop {
    running: bool,
    poll: mio::Poll,
    event_loop_waker: Arc<mio::Waker>,
    event_loop_tx: Sender<EventLoopMsg>,
    event_loop_rx: Receiver<EventLoopMsg>,
    kqueue: kqueue::Watcher,
    event_handler: Box<dyn EventHandler>,
    watches: HashMap<PathBuf, bool>,
    follow_symlinks: bool,
}

/// Watcher implementation based on inotify
#[derive(Debug)]
pub struct KqueueWatcher {
    channel: Sender<EventLoopMsg>,
    waker: Arc<mio::Waker>,
}

enum EventLoopMsg {
    AddWatch(PathBuf, RecursiveMode, Sender<Result<()>>),
    RemoveWatch(PathBuf, Sender<Result<()>>),
    Shutdown,
}

impl EventLoop {
    pub fn new(
        kqueue: kqueue::Watcher,
        event_handler: Box<dyn EventHandler>,
        follow_symlinks: bool,
    ) -> Result<Self> {
        let (event_loop_tx, event_loop_rx) = unbounded::<EventLoopMsg>();
        let poll = mio::Poll::new()?;

        let event_loop_waker = Arc::new(mio::Waker::new(poll.registry(), MESSAGE)?);

        let kqueue_fd = kqueue.as_raw_fd();
        let mut evented_kqueue = mio::unix::SourceFd(&kqueue_fd);
        poll.registry()
            .register(&mut evented_kqueue, KQUEUE, mio::Interest::READABLE)?;

        let event_loop = EventLoop {
            running: true,
            poll,
            event_loop_waker,
            event_loop_tx,
            event_loop_rx,
            kqueue,
            event_handler,
            watches: HashMap::new(),
            follow_symlinks,
        };
        Ok(event_loop)
    }

    // Run the event loop.
    pub fn run(self) {
        let _ = thread::Builder::new()
            .name("notify-rs kqueue loop".to_string())
            .spawn(|| self.event_loop_thread());
    }

    fn event_loop_thread(mut self) {
        let mut events = mio::Events::with_capacity(16);
        loop {
            // Wait for something to happen.
            match self.poll.poll(&mut events, None) {
                Err(ref e) if matches!(e.kind(), std::io::ErrorKind::Interrupted) => {
                    // System call was interrupted, we will retry
                    // TODO: Not covered by tests (to reproduce likely need to setup signal handlers)
                }
                Err(e) => panic!("poll failed: {}", e),
                Ok(()) => {}
            }

            // Process whatever happened.
            for event in &events {
                self.handle_event(event);
            }

            // Stop, if we're done.
            if !self.running {
                break;
            }
        }
    }

    // Handle a single event.
    fn handle_event(&mut self, event: &mio::event::Event) {
        match event.token() {
            MESSAGE => {
                // The channel is readable - handle messages.
                self.handle_messages()
            }
            KQUEUE => {
                // inotify has something to tell us.
                self.handle_kqueue()
            }
            _ => unreachable!(),
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.event_loop_rx.try_recv() {
            match msg {
                EventLoopMsg::AddWatch(path, recursive_mode, tx) => {
                    let _ = tx.send(self.add_watch(path, recursive_mode.is_recursive()));
                }
                EventLoopMsg::RemoveWatch(path, tx) => {
                    let _ = tx.send(self.remove_watch(path, false));
                }
                EventLoopMsg::Shutdown => {
                    self.running = false;
                    break;
                }
            }
        }
    }

    fn handle_kqueue(&mut self) {
        let mut add_watches = Vec::new();
        let mut remove_watches = Vec::new();

        while let Some(event) = self.kqueue.poll(None) {
            log::trace!("kqueue event: {event:?}");

            match event {
                kqueue::Event {
                    data: EventData::Vnode(data),
                    ident: Ident::Filename(_, path),
                } => {
                    let path = PathBuf::from(path);
                    let event = match data {
                        /*
                        TODO: Differentiate folders and files
                        kqueue doesn't tell us if this was a file or a dir, so we
                        could only emulate this inotify behavior if we keep track of
                        all files and directories internally and then perform a
                        lookup.
                        */
                        kqueue::Vnode::Delete => {
                            remove_watches.push(path.clone());
                            Ok(Event::new(EventKind::Remove(RemoveKind::Any)).add_path(path))
                        }

                        // a write to a directory means that a new file was created in it, let's
                        // figure out which file this was
                        kqueue::Vnode::Write if path.is_dir() => {
                            // find which file is new in the directory by comparing it with our
                            // list of known watches
                            std::fs::read_dir(&path)
                                .map(|dir| {
                                    dir.filter_map(std::result::Result::ok)
                                        .map(|f| f.path())
                                        .find(|f| !self.watches.contains_key(f))
                                })
                                .map(|file| {
                                    if let Some(file) = file {
                                        // watch this new file
                                        add_watches.push(file.clone());

                                        Event::new(EventKind::Create(if file.is_dir() {
                                            CreateKind::Folder
                                        } else if file.is_file() {
                                            CreateKind::File
                                        } else {
                                            CreateKind::Other
                                        }))
                                        .add_path(file)
                                    } else {
                                        Event::new(EventKind::Modify(ModifyKind::Data(
                                            DataChange::Any,
                                        )))
                                        .add_path(path)
                                    }
                                })
                                .map_err(Into::into)
                        }

                        // data was written to this file
                        kqueue::Vnode::Write => Ok(Event::new(EventKind::Modify(
                            ModifyKind::Data(DataChange::Any),
                        ))
                        .add_path(path)),

                        /*
                        Extend and Truncate are just different names for the same
                        operation, extend is only used on FreeBSD, truncate everywhere
                        else
                        */
                        kqueue::Vnode::Extend | kqueue::Vnode::Truncate => Ok(Event::new(
                            EventKind::Modify(ModifyKind::Data(DataChange::Size)),
                        )
                        .add_path(path)),

                        /*
                        this kevent has the same problem as the delete kevent. The
                        only way i can think of providing "better" event with more
                        information is to do the diff our self, while this maybe do
                        able of delete. In this case it would somewhat expensive to
                        keep track and compare ever peace of metadata for every file
                        */
                        kqueue::Vnode::Attrib => Ok(Event::new(EventKind::Modify(
                            ModifyKind::Metadata(MetadataKind::Any),
                        ))
                        .add_path(path)),

                        /*
                        The link count on a file changed => subdirectory created or
                        delete.
                        */
                        kqueue::Vnode::Link => {
                            // As we currently don't have a solution that would allow us
                            // to only add/remove the new/delete directory and that dosn't include a
                            // possible race condition. On possible solution would be to
                            // create a `HashMap<PathBuf, Vec<PathBuf>>` which would
                            // include every directory and this content add the time of
                            // adding it to kqueue. While this should allow us to do the
                            // diff and only add/remove the files necessary. This would
                            // also introduce a race condition, where multiple files could
                            // all ready be remove from the directory, and we could get out
                            // of sync.
                            // So for now, until we find a better solution, let remove and
                            // readd the whole directory.
                            // This is a expensive operation, as we recursive through all
                            // subdirectories.
                            remove_watches.push(path.clone());
                            add_watches.push(path.clone());
                            Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path))
                        }

                        // Kqueue not provide us with the information necessary to provide
                        // the new file name to the event.
                        kqueue::Vnode::Rename => {
                            remove_watches.push(path.clone());
                            Ok(
                                Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Any)))
                                    .add_path(path),
                            )
                        }

                        // Access to the file was revoked via revoke(2) or the underlying file system was unmounted.
                        kqueue::Vnode::Revoke => {
                            remove_watches.push(path.clone());
                            Ok(Event::new(EventKind::Remove(RemoveKind::Any)).add_path(path))
                        }

                        // On different BSD variants, different extra events may be present
                        #[allow(unreachable_patterns)]
                        _ => Ok(Event::new(EventKind::Other)),
                    };
                    self.event_handler.handle_event(event);
                }
                // as we don't add any other EVFILTER to kqueue we should never get here
                kqueue::Event { ident: _, data: _ } => unreachable!(),
            }
        }

        for path in remove_watches {
            self.remove_watch(path, true).ok();
        }

        for path in add_watches {
            self.add_watch(path, true).ok();
        }
    }

    fn add_watch(&mut self, path: PathBuf, is_recursive: bool) -> Result<()> {
        // If the watch is not recursive, or if we determine (by stat'ing the path to get its
        // metadata) that the watched path is not a directory, add a single path watch.
        if !is_recursive || !metadata(&path).map_err(Error::io)?.is_dir() {
            self.add_single_watch(path, false)?;
        } else {
            for entry in WalkDir::new(path)
                .follow_links(self.follow_symlinks)
                .into_iter()
            {
                let entry = entry.map_err(map_walkdir_error)?;
                self.add_single_watch(entry.into_path(), is_recursive)?;
            }
        }

        // Only make a single `kevent` syscall to add all the watches.
        self.kqueue.watch()?;

        Ok(())
    }

    /// Adds a single watch to the kqueue.
    ///
    /// The caller of this function must call `self.kqueue.watch()` afterwards to register the new watch.
    fn add_single_watch(&mut self, path: PathBuf, is_recursive: bool) -> Result<()> {
        let event_filter = EventFilter::EVFILT_VNODE;
        let filter_flags = FilterFlag::NOTE_DELETE
            | FilterFlag::NOTE_WRITE
            | FilterFlag::NOTE_EXTEND
            | FilterFlag::NOTE_ATTRIB
            | FilterFlag::NOTE_LINK
            | FilterFlag::NOTE_RENAME
            | FilterFlag::NOTE_REVOKE;

        log::trace!("adding kqueue watch: {}", path.display());

        self.kqueue
            .add_filename(&path, event_filter, filter_flags)
            .map_err(|e| Error::io(e).add_path(path.clone()))?;
        self.watches.insert(path, is_recursive);

        Ok(())
    }

    fn remove_watch(&mut self, path: PathBuf, remove_recursive: bool) -> Result<()> {
        log::trace!("removing kqueue watch: {}", path.display());

        match self.watches.remove(&path) {
            None => return Err(Error::watch_not_found()),
            Some(is_recursive) => {
                if is_recursive || remove_recursive {
                    for entry in WalkDir::new(path)
                        .follow_links(self.follow_symlinks)
                        .into_iter()
                    {
                        let p = entry.map_err(map_walkdir_error)?.into_path();
                        self.kqueue
                            .remove_filename(&p, EventFilter::EVFILT_VNODE)
                            .map_err(|e| Error::io(e).add_path(p))?;
                    }
                } else {
                    self.kqueue
                        .remove_filename(&path, EventFilter::EVFILT_VNODE)
                        .map_err(|e| Error::io(e).add_path(path.clone()))?;
                }

                self.kqueue.watch()?;
            }
        }
        Ok(())
    }
}

fn map_walkdir_error(e: walkdir::Error) -> Error {
    if e.io_error().is_some() {
        // save to unwrap otherwise we whouldn't be in this branch
        Error::io(e.into_io_error().unwrap())
    } else {
        Error::generic(&e.to_string())
    }
}

impl KqueueWatcher {
    fn from_event_handler(
        event_handler: Box<dyn EventHandler>,
        follow_symlinks: bool,
    ) -> Result<Self> {
        let kqueue = kqueue::Watcher::new()?;
        let event_loop = EventLoop::new(kqueue, event_handler, follow_symlinks)?;
        let channel = event_loop.event_loop_tx.clone();
        let waker = event_loop.event_loop_waker.clone();
        event_loop.run();
        Ok(KqueueWatcher { channel, waker })
    }

    fn watch_inner(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        let pb = if path.is_absolute() {
            path.to_owned()
        } else {
            let p = env::current_dir().map_err(Error::io)?;
            p.join(path)
        };
        let (tx, rx) = unbounded();
        let msg = EventLoopMsg::AddWatch(pb, recursive_mode, tx);

        self.channel
            .send(msg)
            .map_err(|e| Error::generic(&e.to_string()))?;
        self.waker
            .wake()
            .map_err(|e| Error::generic(&e.to_string()))?;
        rx.recv()
            .unwrap()
            .map_err(|e| Error::generic(&e.to_string()))
    }

    fn unwatch_inner(&mut self, path: &Path) -> Result<()> {
        let pb = if path.is_absolute() {
            path.to_owned()
        } else {
            let p = env::current_dir().map_err(Error::io)?;
            p.join(path)
        };
        let (tx, rx) = unbounded();
        let msg = EventLoopMsg::RemoveWatch(pb, tx);

        self.channel
            .send(msg)
            .map_err(|e| Error::generic(&e.to_string()))?;
        self.waker
            .wake()
            .map_err(|e| Error::generic(&e.to_string()))?;
        rx.recv()
            .unwrap()
            .map_err(|e| Error::generic(&e.to_string()))
    }
}

impl Watcher for KqueueWatcher {
    /// Create a new watcher.
    fn new<F: EventHandler>(event_handler: F, config: Config) -> Result<Self> {
        Self::from_event_handler(Box::new(event_handler), config.follow_symlinks())
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()> {
        self.watch_inner(path, recursive_mode)
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.unwatch_inner(path)
    }

    fn kind() -> crate::WatcherKind {
        crate::WatcherKind::Kqueue
    }
}

impl Drop for KqueueWatcher {
    fn drop(&mut self) {
        // we expect the event loop to live => unwrap must not panic
        self.channel.send(EventLoopMsg::Shutdown).unwrap();
        self.waker.wake().unwrap();
    }
}

mod tests {
    use super::{Config, KqueueWatcher, RecursiveMode};
    use crate::Watcher;
    use std::error::Error;
    use std::path::PathBuf;
    use std::result::Result;

    #[test]
    fn test_remove_recursive() -> Result<(), Box<dyn Error>> {
        let path = PathBuf::from("src");

        let mut watcher = KqueueWatcher::new(|event| println!("{:?}", event), Config::default())?;
        watcher.watch(&path, RecursiveMode::Recursive)?;
        let result = watcher.unwatch(&path);
        assert!(
            result.is_ok(),
            "unwatch yielded error: {}",
            result.unwrap_err()
        );
        Ok(())
    }
}
