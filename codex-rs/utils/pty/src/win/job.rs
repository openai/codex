use std::io;
use std::mem;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::IntoRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::io::RawHandle;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use anyhow::Context;
use winapi::shared::minwindef::FALSE;
use winapi::um::handleapi::CloseHandle;
use winapi::um::jobapi2::AssignProcessToJobObject;
use winapi::um::jobapi2::CreateJobObjectW;
use winapi::um::jobapi2::SetInformationJobObject;
use winapi::um::jobapi2::TerminateJobObject;
use winapi::um::processthreadsapi::ResumeThread;
use winapi::um::processthreadsapi::TerminateProcess;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;
use winapi::um::winnt::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use winapi::um::winnt::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use winapi::um::winnt::JobObjectExtendedLimitInformation;

/// Injectable Win32 operations for the create/configure/assign/resume state
/// machine. Production uses [`Win32JobObjectApi`]; tests replace individual
/// stages so every failure path can be exercised deterministically.
trait JobObjectApi {
    fn create_job(&self) -> io::Result<OwnedHandle>;
    fn configure_kill_on_close(&self, job: RawHandle) -> io::Result<()>;
    fn assign_process(&self, job: RawHandle, process: RawHandle) -> io::Result<()>;
    fn resume_thread(&self, primary_thread: RawHandle) -> io::Result<u32>;
}

struct Win32JobObjectApi;

impl JobObjectApi for Win32JobObjectApi {
    fn create_job(&self) -> io::Result<OwnedHandle> {
        let raw_handle = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
        if raw_handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedHandle::from_raw_handle(raw_handle.cast()) })
        }
    }

    fn configure_kill_on_close(&self, job: RawHandle) -> io::Result<()> {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let result = unsafe {
            SetInformationJobObject(
                job as HANDLE,
                JobObjectExtendedLimitInformation,
                ptr::addr_of_mut!(limits).cast(),
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if result == FALSE {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn assign_process(&self, job: RawHandle, process: RawHandle) -> io::Result<()> {
        let result = unsafe { AssignProcessToJobObject(job as HANDLE, process as HANDLE) };
        if result == FALSE {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn resume_thread(&self, primary_thread: RawHandle) -> io::Result<u32> {
        let previous_suspend_count = unsafe { ResumeThread(primary_thread as HANDLE) };
        if previous_suspend_count == u32::MAX {
            Err(io::Error::last_os_error())
        } else {
            Ok(previous_suspend_count)
        }
    }
}

fn io_error_with_context(error: io::Error, context: &'static str) -> io::Error {
    let kind = error.kind();
    io::Error::new(kind, anyhow::Error::new(error).context(context))
}

/// Shared controller for a Windows job that terminates all members when closed.
///
/// Clones share one underlying job handle. Calling [`Self::close`] from any
/// clone closes that handle exactly once, so process wait and cancellation paths
/// can race safely without keeping the job alive through duplicated OS handles.
#[derive(Clone, Debug)]
pub struct KillOnCloseJob {
    handle: Arc<Mutex<Option<OwnedHandle>>>,
}

impl KillOnCloseJob {
    /// Create an unnamed, non-inheritable job configured to kill all members
    /// when its sole operating-system handle is closed.
    pub fn new() -> io::Result<Self> {
        Self::new_with_api(&Win32JobObjectApi)
    }

    fn new_with_api(api: &impl JobObjectApi) -> io::Result<Self> {
        let handle = api
            .create_job()
            .map_err(|err| io_error_with_context(err, "failed to create job object"))?;
        api.configure_kill_on_close(handle.as_raw_handle())
            .map_err(|err| {
                io_error_with_context(err, "failed to configure kill-on-close job object")
            })?;
        Ok(Self {
            handle: Arc::new(Mutex::new(Some(handle))),
        })
    }

    fn assign_process_with_api(
        &self,
        process: RawHandle,
        api: &impl JobObjectApi,
    ) -> io::Result<()> {
        let guard = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(job) = guard.as_ref() else {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "job handle is already closed",
            ));
        };
        api.assign_process(job.as_raw_handle(), process)
    }

    /// Close the shared job handle, terminating all processes in the job.
    pub fn close(&self) -> io::Result<()> {
        self.take_and_close(/*exit_code*/ None)
    }

    /// Terminate all job members with `exit_code`, then close the shared job
    /// handle even if explicit termination reports an error.
    pub fn terminate_and_close(&self, exit_code: u32) -> io::Result<()> {
        self.take_and_close(Some(exit_code))
    }

    fn take_and_close(&self, exit_code: Option<u32>) -> io::Result<()> {
        let mut guard = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(handle) = guard.take() else {
            return Ok(());
        };
        let raw_handle = handle.into_raw_handle();

        let terminate_result = exit_code.map(|exit_code| {
            let result = unsafe { TerminateJobObject(raw_handle.cast(), exit_code) };
            if result == FALSE {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
        let close_result = unsafe { CloseHandle(raw_handle.cast()) };
        let close_result = if close_result == FALSE {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        };

        terminate_result.unwrap_or(Ok(())).and(close_result)
    }
}

/// Owns a newly created process and its suspended primary thread until the
/// process has been assigned to a kill-on-close job and resumed.
pub struct SuspendedProcess {
    process: Option<OwnedHandle>,
    primary_thread: Option<OwnedHandle>,
    process_id: u32,
}

impl SuspendedProcess {
    /// Take ownership of raw handles returned by a successful `CreateProcess*`
    /// call made with `CREATE_SUSPENDED`.
    ///
    /// # Safety
    ///
    /// `process` and `primary_thread` must be valid, owned handles for the same
    /// newly created process. The primary thread must still have its initial
    /// suspension count, and ownership of both handles transfers to this value.
    pub unsafe fn from_raw_handles(
        process: RawHandle,
        primary_thread: RawHandle,
        process_id: u32,
    ) -> Self {
        Self {
            process: Some(unsafe { OwnedHandle::from_raw_handle(process) }),
            primary_thread: Some(unsafe { OwnedHandle::from_raw_handle(primary_thread) }),
            process_id,
        }
    }

    /// Assign the suspended process to `job`, then resume its primary thread.
    /// Any failure leaves this guard armed, so the process is terminated and
    /// waited before the error is returned.
    pub fn assign_and_resume(self, job: KillOnCloseJob) -> anyhow::Result<JobProcess> {
        self.assign_and_resume_with_api(job, &Win32JobObjectApi)
    }

    fn assign_and_resume_with_api(
        mut self,
        job: KillOnCloseJob,
        api: &impl JobObjectApi,
    ) -> anyhow::Result<JobProcess> {
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| io::Error::other("suspended process is missing its process handle"))?;
        job.assign_process_with_api(process.as_raw_handle(), api)
            .context("failed to assign suspended process to job")?;

        let primary_thread = self.primary_thread.as_ref().ok_or_else(|| {
            io::Error::other("suspended process is missing its primary thread handle")
        })?;
        let previous_suspend_count = api
            .resume_thread(primary_thread.as_raw_handle())
            .context("failed to resume suspended process")?;
        if previous_suspend_count != 1 {
            return Err(io::Error::other(format!(
                "expected suspended process thread count 1, got {previous_suspend_count}"
            )))
            .context("failed to resume suspended process");
        }

        drop(self.primary_thread.take());
        let process = self
            .process
            .take()
            .ok_or_else(|| io::Error::other("suspended process is missing its process handle"))?;
        Ok(JobProcess {
            process,
            process_id: self.process_id,
            controller: job,
        })
    }
}

impl Drop for SuspendedProcess {
    fn drop(&mut self) {
        let Some(process) = self.process.as_ref() else {
            return;
        };
        unsafe {
            let _ = TerminateProcess(process.as_raw_handle().cast(), /*uExitCode*/ 1);
            let _ = WaitForSingleObject(process.as_raw_handle().cast(), INFINITE);
        }
    }
}

/// A running process contained by a kill-on-close Windows job.
#[derive(Debug)]
pub struct JobProcess {
    process: OwnedHandle,
    process_id: u32,
    controller: KillOnCloseJob,
}

impl JobProcess {
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    pub fn as_raw_handle(&self) -> RawHandle {
        self.process.as_raw_handle()
    }

    pub fn controller(&self) -> KillOnCloseJob {
        self.controller.clone()
    }
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
