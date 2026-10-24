use filedescriptor::OwnedHandle;
use std::collections::HashMap;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle as ProcessHandle;
use std::os::windows::io::RawHandle;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tokio::process::Child;
use tokio::process::Command;
use winapi::shared::ntdef::NT_SUCCESS;
use winapi::shared::ntdef::NTSTATUS;
use winapi::shared::winerror::ERROR_INVALID_PARAMETER;
use winapi::shared::winerror::ERROR_MORE_DATA;
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::jobapi::IsProcessInJob;
use winapi::um::jobapi2::AssignProcessToJobObject;
use winapi::um::jobapi2::CreateJobObjectW;
use winapi::um::jobapi2::QueryInformationJobObject;
use winapi::um::jobapi2::SetInformationJobObject;
use winapi::um::jobapi2::TerminateJobObject;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::processthreadsapi::TerminateProcess;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::CREATE_NO_WINDOW;
use winapi::um::winbase::CREATE_SUSPENDED;
use winapi::um::winbase::WAIT_OBJECT_0;
use winapi::um::winnt::HANDLE;
use winapi::um::winnt::JOB_OBJECT_LIMIT_BREAKAWAY_OK;
use winapi::um::winnt::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use winapi::um::winnt::JOBOBJECT_BASIC_PROCESS_ID_LIST;
use winapi::um::winnt::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use winapi::um::winnt::JobObjectBasicProcessIdList;
use winapi::um::winnt::JobObjectExtendedLimitInformation;
use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
use winapi::um::winnt::PROCESS_SET_QUOTA;
use winapi::um::winnt::PROCESS_SUSPEND_RESUME;
use winapi::um::winnt::PROCESS_TERMINATE;
use winapi::um::winnt::SYNCHRONIZE;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtResumeProcess(process_handle: HANDLE) -> NTSTATUS;
}

/// Owns a Windows Job Object used to terminate a spawned process tree.
#[derive(Debug)]
pub struct JobObject {
    handle: OwnedHandle,
    // A mutex makes the state check, Job Object API call, and state update
    // atomic with respect to concurrent preserve and terminate requests.
    preserve_descendants: Mutex<bool>,
}

// A failed restart must still stop any remaining members.
struct TerminateOnDrop<'a>(Option<&'a JobObject>);

impl Drop for TerminateOnDrop<'_> {
    fn drop(&mut self) {
        if let Some(job) = self.0
            && let Err(error) = job.terminate()
        {
            log::warn!("Failed to terminate Windows job after an interrupted restart: {error}");
        }
    }
}

impl JobObject {
    /// Creates a Job Object configured to terminate all members when its last handle closes.
    pub fn create() -> io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(handle.cast()) };

        Self::set_limit_flags(
            &handle,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        )?;

        Ok(Self {
            handle,
            preserve_descendants: Mutex::new(false),
        })
    }

    /// Creates a Job Object whose owned descendants cannot explicitly break away.
    pub fn create_without_breakaway() -> io::Result<Self> {
        let job = Self::create()?;
        Self::set_limit_flags(&job.handle, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)?;
        Ok(job)
    }

    /// Captures an owned process handle before its numeric identifier can be reused.
    pub fn open_process_handle(process_id: u32) -> io::Result<ProcessHandle> {
        Self::open_process(process_id, PROCESS_TERMINATE)
    }

    fn open_process(process_id: u32, access: u32) -> io::Result<ProcessHandle> {
        let handle = unsafe {
            OpenProcess(access, /*bInheritHandle*/ 0, process_id)
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }

        Ok(unsafe { ProcessHandle::from_raw_handle(handle.cast()) })
    }

    /// Terminates the exact process identified by a previously captured handle.
    pub fn terminate_process_handle(handle: &ProcessHandle) -> io::Result<()> {
        let terminated = unsafe {
            TerminateProcess(handle.as_raw_handle().cast(), /*uExitCode*/ 1)
        };
        if terminated == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn set_limit_flags(handle: &OwnedHandle, flags: u32) -> io::Result<()> {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = flags;
        let configured = unsafe {
            SetInformationJobObject(
                handle.as_raw_handle().cast(),
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of_mut!(limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Assigns a running process to this job.
    ///
    /// Assignment is not retroactive: descendants created before this call
    /// completes are not guaranteed to become members of the job.
    pub(crate) fn assign_process(&self, process_handle: RawHandle) -> io::Result<()> {
        let assigned = unsafe {
            AssignProcessToJobObject(self.handle.as_raw_handle().cast(), process_handle.cast())
        };
        if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Prepares a background child to be assigned to this job before it runs.
    ///
    /// Replaces all creation flags with `CREATE_SUSPENDED | CREATE_NO_WINDOW`.
    /// Interactive children that need to inherit a console require an explicit launch.
    pub fn prepare_suspended_spawn(&self, command: &mut Command) {
        command
            .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
            .kill_on_drop(true);
    }

    /// Assigns and resumes a suspended child, returning its handle if containment succeeded.
    ///
    /// Nested jobs can reject assignment. Such a child is resumed without
    /// containment so callers can preserve their existing compatibility fallback.
    pub fn assign_and_resume_process(&self, process_id: u32) -> io::Result<Option<ProcessHandle>> {
        let process = Self::open_process(
            process_id,
            PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_SUSPEND_RESUME | SYNCHRONIZE,
        )?;
        let assignment = self.assign_process(process.as_raw_handle());

        let status = unsafe { NtResumeProcess(process.as_raw_handle().cast()) };
        if !NT_SUCCESS(status) {
            unsafe {
                TerminateProcess(process.as_raw_handle().cast(), /*uExitCode*/ 1)
            };
            return Err(io::Error::other(format!(
                "failed to resume suspended process: NTSTATUS {status:#x}"
            )));
        }

        match assignment {
            Ok(()) => Ok(Some(process)),
            Err(error) => {
                log::warn!(
                    "Windows process job assignment unavailable for pid {process_id}: {error}"
                );
                Ok(None)
            }
        }
    }

    /// Starts a console-free background process, falling back when containment is unavailable.
    /// The returned job owns the contained process tree when assignment succeeds.
    pub fn spawn_background(command: &mut Command) -> io::Result<(Child, Option<Self>)> {
        Self::spawn_background_with_job(command, Self::create())
    }

    fn spawn_background_with_job(
        command: &mut Command,
        job: io::Result<Self>,
    ) -> io::Result<(Child, Option<Self>)> {
        match job.and_then(|job| job.spawn_contained(command).map(|child| (child, job))) {
            Ok((child, job)) => Ok((child, Some(job))),
            Err(_) => {
                // Remove suspension before retrying, including after failed assignment.
                command.creation_flags(CREATE_NO_WINDOW).kill_on_drop(true);
                command.spawn().map(|child| (child, None))
            }
        }
    }

    /// Starts a background child without a console, assigning it to this job before it runs.
    ///
    /// Replaces existing creation flags as in [`Self::prepare_suspended_spawn`].
    pub fn spawn_contained(&self, command: &mut Command) -> io::Result<Child> {
        self.prepare_suspended_spawn(command);
        let child = command.spawn()?;
        let process_handle = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("missing child process handle"))?;
        self.assign_process(process_handle)?;

        let status = unsafe { NtResumeProcess(process_handle.cast()) };
        if !NT_SUCCESS(status) {
            return Err(io::Error::other(format!(
                "failed to resume contained process: NTSTATUS {status:#x}"
            )));
        }

        Ok(child)
    }

    /// Allows contained descendants to keep running after the root exits normally.
    ///
    /// This disables both explicit job termination and kill-on-close for this
    /// object. Calls race safely with [`Self::terminate`]: whichever operation
    /// acquires the state lock first determines whether the process tree is
    /// preserved or terminated.
    pub fn preserve_descendants(&self) -> io::Result<()> {
        let mut preserve_descendants = self
            .preserve_descendants
            .lock()
            .map_err(|_| io::Error::other("job state lock poisoned"))?;
        if *preserve_descendants {
            return Ok(());
        }

        Self::set_limit_flags(&self.handle, JOB_OBJECT_LIMIT_BREAKAWAY_OK)?;
        *preserve_descendants = true;
        Ok(())
    }

    /// Terminates every process currently assigned to the job.
    pub fn terminate(&self) -> io::Result<()> {
        let preserve_descendants = self
            .preserve_descendants
            .lock()
            .map_err(|_| io::Error::other("job state lock poisoned"))?;
        if *preserve_descendants {
            return Ok(());
        }

        let terminated = unsafe {
            TerminateJobObject(self.handle.as_raw_handle().cast(), /*uExitCode*/ 1)
        };
        if terminated == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Stops a root from [`Self::assign_and_resume_process`] and the remaining members of its job.
    pub fn terminate_and_wait(&self, root: &ProcessHandle, grace: Duration) -> io::Result<()> {
        let mut cleanup = TerminateOnDrop(Some(self));
        let deadline = Instant::now() + grace;
        Self::request_stop(root)?;
        Self::wait_for_exit(root, deadline)?;
        // TerminateJobObject can empty the job list before members release their handles.
        // Stop by process handle, then rescan for children born during the previous pass.
        let mut list = vec![0usize; 16];
        let mut stopped = HashMap::new();
        loop {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let pids = self.member_ids(&mut list)?;
            if pids.iter().all(|pid| stopped.contains_key(pid)) {
                self.terminate()?;
                cleanup.0 = None;
                return Ok(());
            }
            for &pid in pids {
                if stopped.contains_key(&pid) {
                    continue;
                }
                let member = match Self::open_process(
                    pid as u32,
                    PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE,
                ) {
                    Ok(member) => member,
                    Err(error) if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) => {
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                // A listed PID can be reused before OpenProcess. Only signal confirmed job members.
                let mut in_job = 0;
                if unsafe {
                    IsProcessInJob(
                        member.as_raw_handle().cast(),
                        self.handle.as_raw_handle().cast(),
                        &mut in_job,
                    )
                } == 0
                {
                    return Err(io::Error::last_os_error());
                }
                if in_job != 0 {
                    Self::request_stop(&member)?;
                    stopped.insert(pid, member);
                }
            }
            for member in stopped.values() {
                Self::wait_for_exit(member, deadline)?;
            }
        }
    }

    fn member_ids<'a>(&self, list: &'a mut Vec<usize>) -> io::Result<&'a [usize]> {
        const HEADER_WORDS: usize =
            std::mem::offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList)
                / std::mem::size_of::<usize>();
        loop {
            let listed = unsafe {
                QueryInformationJobObject(
                    self.handle.as_raw_handle().cast(),
                    JobObjectBasicProcessIdList,
                    list.as_mut_ptr().cast(),
                    u32::try_from(std::mem::size_of_val(list.as_slice()))
                        .map_err(io::Error::other)?,
                    std::ptr::null_mut(),
                )
            };
            if listed != 0 {
                let header = unsafe { &*list.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>() };
                if header.NumberOfProcessIdsInList == header.NumberOfAssignedProcesses {
                    return Ok(&list[HEADER_WORDS..][..header.NumberOfProcessIdsInList as usize]);
                }
            } else {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_MORE_DATA as i32) {
                    return Err(error);
                }
            }
            list.resize(list.len() * 2, 0);
        }
    }

    fn request_stop(process: &ProcessHandle) -> io::Result<()> {
        // Windows also returns access denied when the process has already exited. Its handle
        // must still signal before a replacement can start.
        match Self::terminate_process_handle(process) {
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(()),
            result => result,
        }
    }

    fn wait_for_exit(process: &ProcessHandle, deadline: Instant) -> io::Result<()> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        // The largest DWORD denotes INFINITE, so cap this bounded wait below it.
        let millis = remaining.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        match unsafe { WaitForSingleObject(process.as_raw_handle().cast(), millis) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(io::ErrorKind::TimedOut.into()),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

impl AsRawHandle for JobObject {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
