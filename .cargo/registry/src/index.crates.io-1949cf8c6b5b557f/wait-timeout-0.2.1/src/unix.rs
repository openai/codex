//! Unix implementation of waiting for children with timeouts
//!
//! On unix, wait() and its friends have no timeout parameters, so there is
//! no way to time out a thread in wait(). From some googling and some
//! thinking, it appears that there are a few ways to handle timeouts in
//! wait(), but the only real reasonable one for a multi-threaded program is
//! to listen for SIGCHLD.
//!
//! With this in mind, the waiting mechanism with a timeout only uses
//! waitpid() with WNOHANG, but otherwise all the necessary blocking is done by
//! waiting for a SIGCHLD to arrive (and that blocking has a timeout). Note,
//! however, that waitpid() is still used to actually reap the child.
//!
//! Signal handling is super tricky in general, and this is no exception. Due
//! to the async nature of SIGCHLD, we use the self-pipe trick to transmit
//! data out of the signal handler to the rest of the application.

#![allow(bad_style)]

use std::cmp;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::mem;
use std::os::unix::net::UnixStream;
use std::os::unix::prelude::*;
use std::process::{Child, ExitStatus};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use libc::{self, c_int};

static INIT: Once = Once::new();
static mut STATE: *mut State = 0 as *mut _;

struct State {
    prev: libc::sigaction,
    write: UnixStream,
    read: UnixStream,
    map: Mutex<StateMap>,
}

type StateMap = HashMap<*mut Child, (UnixStream, Option<ExitStatus>)>;

pub fn wait_timeout(child: &mut Child, dur: Duration) -> io::Result<Option<ExitStatus>> {
    INIT.call_once(State::init);
    unsafe { (*STATE).wait_timeout(child, dur) }
}

impl State {
    #[allow(unused_assignments)]
    fn init() {
        unsafe {
            // Create our "self pipe" and then set both ends to nonblocking
            // mode.
            let (read, write) = UnixStream::pair().unwrap();
            read.set_nonblocking(true).unwrap();
            write.set_nonblocking(true).unwrap();

            let state = Box::new(State {
                prev: mem::zeroed(),
                write: write,
                read: read,
                map: Mutex::new(HashMap::new()),
            });

            // Register our sigchld handler
            let mut new: libc::sigaction = mem::zeroed();
            new.sa_sigaction = sigchld_handler as usize;
            new.sa_flags = libc::SA_NOCLDSTOP | libc::SA_RESTART | libc::SA_SIGINFO;

            STATE = Box::into_raw(state);

            assert_eq!(libc::sigaction(libc::SIGCHLD, &new, &mut (*STATE).prev), 0);
        }
    }

    fn wait_timeout(&self, child: &mut Child, dur: Duration) -> io::Result<Option<ExitStatus>> {
        // First up, prep our notification pipe which will tell us when our
        // child has been reaped (other threads may signal this pipe).
        let (read, write) = UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;

        // Next, take a lock on the map of children currently waiting. Right
        // after this, **before** we add ourselves to the map, we check to see
        // if our child has actually already exited via a `try_wait`. If the
        // child has exited then we return immediately as we'll never otherwise
        // receive a SIGCHLD notification.
        //
        // If the wait reports the child is still running, however, we add
        // ourselves to the map and then block in `select` waiting for something
        // to happen.
        let mut map = self.map.lock().unwrap();
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        assert!(map.insert(child, (write, None)).is_none());
        drop(map);

        // Make sure that no matter what when we exit our pointer is removed
        // from the map.
        struct Remove<'a> {
            state: &'a State,
            child: &'a mut Child,
        }
        impl<'a> Drop for Remove<'a> {
            fn drop(&mut self) {
                let mut map = self.state.map.lock().unwrap();
                drop(map.remove(&(self.child as *mut Child)));
            }
        }
        let remove = Remove { state: self, child };

        // Alright, we're guaranteed that we'll eventually get a SIGCHLD due
        // to our `try_wait` failing, and we're also guaranteed that we'll
        // get notified about this because we're in the map. Next up wait
        // for an event.
        //
        // Note that this happens in a loop for two reasons; we could
        // receive EINTR or we could pick up a SIGCHLD for other threads but not
        // actually be ready oureslves.
        let start = Instant::now();
        let mut fds = [
            libc::pollfd {
                fd: self.read.as_raw_fd(),
                events: libc::POLLIN,

                revents: 0,
            },
            libc::pollfd {
                fd: read.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        loop {
            let elapsed = start.elapsed();
            if elapsed >= dur {
                break;
            }
            let timeout = dur - elapsed;
            let timeout = timeout
                .as_secs()
                .checked_mul(1_000)
                .and_then(|amt| amt.checked_add(timeout.subsec_nanos() as u64 / 1_000_000))
                .unwrap_or(u64::max_value());
            let timeout = cmp::min(<c_int>::max_value() as u64, timeout) as c_int;
            let r = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout) };
            let timeout = match r {
                0 => true,
                n if n > 0 => false,
                n => {
                    let err = io::Error::last_os_error();
                    if err.kind() == io::ErrorKind::Interrupted {
                        continue;
                    } else {
                        panic!("error in select = {}: {}", n, err)
                    }
                }
            };

            // Now that something has happened, we need to process what actually
            // happened. There's are three reasons we could have woken up:
            //
            // 1. The file descriptor in our SIGCHLD handler was written to.
            //    This means that a SIGCHLD was received and we need to poll the
            //    entire list of waiting processes to figure out which ones
            //    actually exited.
            // 2. Our file descriptor was written to. This means that another
            //    thread reaped our child and listed the exit status in the
            //    local map.
            // 3. We timed out. This means we need to remove ourselves from the
            //    map and simply carry on.
            //
            // In the case that a SIGCHLD signal was received, we do that
            // processing and keep going. If our fd was written to or a timeout
            // was received then we break out of the loop and return from this
            // call.
            let mut map = self.map.lock().unwrap();
            if drain(&self.read) {
                self.process_sigchlds(&mut map);
            }

            if drain(&read) || timeout {
                break;
            }
        }

        let mut map = self.map.lock().unwrap();
        let (_write, ret) = map.remove(&(remove.child as *mut Child)).unwrap();
        drop(map);
        Ok(ret)
    }

    fn process_sigchlds(&self, map: &mut StateMap) {
        for (&k, &mut (ref write, ref mut status)) in map {
            // Already reaped, nothing to do here
            if status.is_some() {
                continue;
            }

            *status = unsafe { (*k).try_wait().unwrap() };
            if status.is_some() {
                notify(write);
            }
        }
    }
}

fn drain(mut file: &UnixStream) -> bool {
    let mut ret = false;
    let mut buf = [0u8; 16];
    loop {
        match file.read(&mut buf) {
            Ok(0) => return true, // EOF == something happened
            Ok(..) => ret = true, // data read, but keep draining
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return ret;
                } else {
                    panic!("bad read: {}", e)
                }
            }
        }
    }
}

fn notify(mut file: &UnixStream) {
    match file.write(&[1]) {
        Ok(..) => {}
        Err(e) => {
            if e.kind() != io::ErrorKind::WouldBlock {
                panic!("bad error on write fd: {}", e)
            }
        }
    }
}

// Signal handler for SIGCHLD signals, must be async-signal-safe!
//
// This function will write to the writing half of the "self pipe" to wake
// up the helper thread if it's waiting. Note that this write must be
// nonblocking because if it blocks and the reader is the thread we
// interrupted, then we'll deadlock.
//
// When writing, if the write returns EWOULDBLOCK then we choose to ignore
// it. At that point we're guaranteed that there's something in the pipe
// which will wake up the other end at some point, so we just allow this
// signal to be coalesced with the pending signals on the pipe.
#[allow(unused_assignments)]
extern "C" fn sigchld_handler(signum: c_int, info: *mut libc::siginfo_t, ptr: *mut libc::c_void) {
    type FnSigaction = extern "C" fn(c_int, *mut libc::siginfo_t, *mut libc::c_void);
    type FnHandler = extern "C" fn(c_int);

    unsafe {
        let state = &*STATE;
        notify(&state.write);

        let fnptr = state.prev.sa_sigaction;
        if fnptr == 0 {
            return;
        }
        if state.prev.sa_flags & libc::SA_SIGINFO == 0 {
            let action = mem::transmute::<usize, FnHandler>(fnptr);
            action(signum)
        } else {
            let action = mem::transmute::<usize, FnSigaction>(fnptr);
            action(signum, info, ptr)
        }
    }
}
