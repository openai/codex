//! Windows API support functions.

use std::{
	io::{Error, Result},
	ops::ControlFlow,
	time::Duration,
};

#[cfg(feature = "tracing")]
use tracing::{debug, instrument};
use windows::Win32::{
	Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
	System::{
		Diagnostics::ToolHelp::{
			CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
		},
		JobObjects::{
			AssignProcessToJobObject, CreateJobObjectW,
			JobObjectAssociateCompletionPortInformation, JobObjectExtendedLimitInformation,
			SetInformationJobObject, TerminateJobObject, JOBOBJECT_ASSOCIATE_COMPLETION_PORT,
			JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
		},
		Threading::{GetProcessId, OpenThread, ResumeThread, INFINITE, THREAD_SUSPEND_RESUME},
		IO::{CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED},
	},
};

#[derive(Clone, Copy, Debug)]
pub struct JobHandle(pub HANDLE);

unsafe impl Send for JobHandle {}
unsafe impl Sync for JobHandle {}

#[derive(Clone, Copy, Debug)]
pub struct PortHandle(pub HANDLE);

unsafe impl Send for PortHandle {}
unsafe impl Sync for PortHandle {}

/// A JobObject and its associated completion port.
///
/// This struct closes the handles when dropped.
#[derive(Debug)]
pub(crate) struct JobPort {
	pub job: JobHandle,
	pub completion_port: PortHandle,
}

impl Drop for JobPort {
	fn drop(&mut self) {
		unsafe { CloseHandle(self.job.0) }.ok();
		unsafe { CloseHandle(self.completion_port.0) }.ok();
	}
}

/// Create a JobObject and an associated completion port.
///
/// If `kill_on_drop` is true, we opt into the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag, which
/// essentially implements the "reap children" feature of Unix systems directly in Win32.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn make_job_object(process_handle: HANDLE, kill_on_drop: bool) -> Result<JobPort> {
	let job = JobHandle(unsafe { CreateJobObjectW(None, None) }.map_err(Error::other)?);
	#[cfg(feature = "tracing")]
	debug!(?job, "done CreateJobObjectW");

	let completion_port =
		PortHandle(unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 1) }?);
	#[cfg(feature = "tracing")]
	debug!(?completion_port, "done CreateIoCompletionPort");

	let associate_completion = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
		CompletionKey: job.0 .0 as _,
		CompletionPort: completion_port.0,
	};

	unsafe {
		SetInformationJobObject(
			job.0,
			JobObjectAssociateCompletionPortInformation,
			(&associate_completion) as *const _ as _,
			std::mem::size_of_val(&associate_completion)
				.try_into()
				.expect("cannot safely cast to DWORD"),
		)
	}?;
	#[cfg(feature = "tracing")]
	debug!(
		?associate_completion,
		"done SetInformationJobObject(completion)"
	);

	let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();

	if kill_on_drop {
		info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
	}

	unsafe {
		SetInformationJobObject(
			job.0,
			JobObjectExtendedLimitInformation,
			&info as *const _ as _,
			std::mem::size_of_val(&info)
				.try_into()
				.expect("cannot safely cast to DWORD"),
		)
	}?;
	#[cfg(feature = "tracing")]
	debug!(?info, "done SetInformationJobObject(limit)");

	unsafe { AssignProcessToJobObject(job.0, process_handle) }?;
	#[cfg(feature = "tracing")]
	debug!(?job, ?process_handle, "done AssignProcessToJobObject");

	Ok(JobPort {
		job,
		completion_port,
	})
}

/// Resume all threads in the process (ie resume the process).
///
/// This is a pretty terrible hack, but it's either this or we
/// re-implement all of Rust's std::process just to get access!
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn resume_threads(child_process: HANDLE) -> Result<()> {
	#[inline]
	unsafe fn inner(pid: u32, tool_handle: HANDLE) -> Result<()> {
		let mut entry = THREADENTRY32 {
			dwSize: 28,
			cntUsage: 0,
			th32ThreadID: 0,
			th32OwnerProcessID: 0,
			tpBasePri: 0,
			tpDeltaPri: 0,
			dwFlags: 0,
		};

		let mut res = unsafe { Thread32First(tool_handle, &mut entry) };
		while res.is_ok() {
			if entry.th32OwnerProcessID == pid {
				let thread_handle =
					unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }?;
				if unsafe { ResumeThread(thread_handle) } == u32::MAX {
					unsafe { CloseHandle(thread_handle) }?;
					return Err(Error::last_os_error());
				}
				unsafe { CloseHandle(thread_handle) }?;
			}

			res = unsafe { Thread32Next(tool_handle, &mut entry) };
		}

		Ok(())
	}

	let child_id = unsafe { GetProcessId(child_process) };
	let tool_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }?;
	let ret = unsafe { inner(child_id, tool_handle) };
	unsafe { CloseHandle(tool_handle) }.map_err(Error::other)?;
	ret
}

/// Terminate a job object without waiting for the processes to exit.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn terminate_job(job: JobHandle, exit_code: u32) -> Result<()> {
	unsafe { TerminateJobObject(job.0, exit_code) }.map_err(Error::other)
}

/// Wait for a job to complete.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn wait_on_job(
	completion_port: PortHandle,
	timeout: Option<Duration>,
) -> Result<ControlFlow<()>> {
	let mut code: u32 = 0;
	let mut key: usize = 0;
	let mut overlapped = OVERLAPPED::default();
	let mut lp_overlapped = &mut overlapped as *mut OVERLAPPED;

	let result = unsafe {
		GetQueuedCompletionStatus(
			completion_port.0,
			&mut code,
			&mut key,
			&mut lp_overlapped as *mut _,
			timeout.map_or(INFINITE, |d| d.as_millis().try_into().unwrap_or(INFINITE)),
		)
	};

	// ignore timing out errors unless the timeout was specified to INFINITE
	// https://docs.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-getqueuedcompletionstatus
	if timeout.is_some() && result.is_err() && lp_overlapped.is_null() {
		return Ok(ControlFlow::Continue(()));
	}

	Ok(ControlFlow::Break(()))
}
