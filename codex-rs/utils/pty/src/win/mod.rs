#![allow(clippy::unwrap_used)]

// This file is copied from https://github.com/wezterm/wezterm (MIT license).
// Copyright (c) 2018-Present Wez Furlong
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// Local modifications:
// - Keep each Windows PTY process in a kill-on-close job so terminating the
//   session also terminates descendants.

use anyhow::Context as _;
use portable_pty::Child;
use portable_pty::ChildKiller;
use portable_pty::ExitStatus;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::os::windows::io::AsRawHandle;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use winapi::shared::minwindef::DWORD;
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::processthreadsapi::*;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;
use winapi::um::winbase::WAIT_OBJECT_0;

pub(crate) mod conpty;
mod job;
mod procthreadattr;
mod psuedocon;

pub use conpty::ConPtySystem;
pub use job::JobProcess;
pub use job::KillOnCloseJob;
pub use job::SuspendedProcess;
pub use psuedocon::PsuedoCon;
pub use psuedocon::conpty_supported;

#[derive(Debug)]
pub struct WinChild {
    process: Mutex<JobProcess>,
}

impl WinChild {
    fn is_complete(&mut self) -> IoResult<Option<ExitStatus>> {
        let process = self.process.lock().unwrap();
        let proc = process.try_clone_process_handle()?;
        let controller = process.controller();
        drop(process);

        match unsafe {
            WaitForSingleObject(proc.as_raw_handle() as _, /*dwMilliseconds*/ 0)
        } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut status: DWORD = 0;
                let res = unsafe { GetExitCodeProcess(proc.as_raw_handle() as _, &mut status) };
                let status = if res != 0 {
                    Ok(Some(ExitStatus::with_exit_code(status)))
                } else {
                    Err(IoError::last_os_error())
                };
                controller.close()?;
                status
            }
            _ => {
                let err = IoError::last_os_error();
                let _ = controller.terminate_and_close(/*exit_code*/ 1);
                Err(err)
            }
        }
    }

    fn do_kill(&mut self) -> IoResult<()> {
        self.process
            .lock()
            .unwrap()
            .controller()
            .terminate_and_close(/*exit_code*/ 1)
    }
}

impl ChildKiller for WinChild {
    fn kill(&mut self) -> IoResult<()> {
        self.do_kill()
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        let controller = self.process.lock().unwrap().controller();
        Box::new(WinChildKiller { controller })
    }
}

#[derive(Debug)]
pub struct WinChildKiller {
    controller: KillOnCloseJob,
}

impl ChildKiller for WinChildKiller {
    fn kill(&mut self) -> IoResult<()> {
        self.controller.terminate_and_close(/*exit_code*/ 1)
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(WinChildKiller {
            controller: self.controller.clone(),
        })
    }
}

impl Child for WinChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        self.is_complete()
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        if let Some(status) = self.try_wait()? {
            return Ok(status);
        }
        let process = self.process.lock().unwrap();
        let proc = process.try_clone_process_handle()?;
        let controller = process.controller();
        drop(process);
        let wait_result = unsafe { WaitForSingleObject(proc.as_raw_handle() as _, INFINITE) };
        if wait_result != WAIT_OBJECT_0 {
            let err = IoError::last_os_error();
            let _ = controller.terminate_and_close(/*exit_code*/ 1);
            return Err(err);
        }
        let mut status: DWORD = 0;
        let res = unsafe { GetExitCodeProcess(proc.as_raw_handle() as _, &mut status) };
        let status = if res != 0 {
            Ok(ExitStatus::with_exit_code(status))
        } else {
            Err(IoError::last_os_error())
        };
        controller.close()?;
        status
    }

    fn process_id(&self) -> Option<u32> {
        Some(self.process.lock().unwrap().process_id())
    }

    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        Some(self.process.lock().unwrap().as_raw_handle())
    }
}

impl std::future::Future for WinChild {
    type Output = anyhow::Result<ExitStatus>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<anyhow::Result<ExitStatus>> {
        match self.is_complete() {
            Ok(Some(status)) => Poll::Ready(Ok(status)),
            Err(err) => Poll::Ready(Err(err).context("Failed to retrieve process exit status")),
            Ok(None) => {
                let proc = self.process.lock().unwrap().try_clone_process_handle()?;
                let waker = cx.waker().clone();
                std::thread::spawn(move || {
                    unsafe {
                        WaitForSingleObject(proc.as_raw_handle() as _, INFINITE);
                    }
                    waker.wake();
                });
                Poll::Pending
            }
        }
    }
}
