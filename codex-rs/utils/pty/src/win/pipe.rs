use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::mem;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;
use anyhow::Result;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use winapi::shared::minwindef::FALSE;
use winapi::shared::minwindef::TRUE;
use winapi::um::handleapi::SetHandleInformation;
use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
use winapi::um::namedpipeapi::CreatePipe;
use winapi::um::processthreadsapi::CreateProcessW;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::processthreadsapi::PROCESS_INFORMATION;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::CREATE_SUSPENDED;
use winapi::um::winbase::CREATE_UNICODE_ENVIRONMENT;
use winapi::um::winbase::EXTENDED_STARTUPINFO_PRESENT;
use winapi::um::winbase::HANDLE_FLAG_INHERIT;
use winapi::um::winbase::INFINITE;
use winapi::um::winbase::STARTF_USESTDHANDLES;
use winapi::um::winbase::STARTUPINFOEXW;
use winapi::um::winbase::WAIT_OBJECT_0;
use winapi::um::winnt::HANDLE;

use super::KillOnCloseJob;
use super::SuspendedProcess;
use super::command::prepare_command;
use super::procthreadattr::ProcThreadAttributeList;
use crate::process::ChildTerminator;
use crate::process::ProcessHandle;
use crate::process::ProcessSignal;
use crate::process::SpawnedProcess;

struct JobTerminator {
    controller: KillOnCloseJob,
}

impl ChildTerminator for JobTerminator {
    fn signal(&mut self, signal: ProcessSignal) -> io::Result<()> {
        Err(crate::process::unsupported_signal(signal))
    }

    fn kill(&mut self) -> io::Result<()> {
        self.controller.terminate_and_close(/*exit_code*/ 1)
    }
}

#[derive(Clone, Copy)]
enum StdinMode {
    Piped,
    Null,
}

enum ParentPipeEnd {
    Reads,
    Writes,
}

struct PipeEnds {
    parent: OwnedHandle,
    child: OwnedHandle,
}

pub(crate) async fn spawn_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
) -> Result<SpawnedProcess> {
    spawn_process_with_stdin_mode(program, args, cwd, env, StdinMode::Piped).await
}

pub(crate) async fn spawn_process_no_stdin(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
) -> Result<SpawnedProcess> {
    spawn_process_with_stdin_mode(program, args, cwd, env, StdinMode::Null).await
}

async fn spawn_process_with_stdin_mode(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    stdin_mode: StdinMode,
) -> Result<SpawnedProcess> {
    let mut command = prepare_command(program, args, cwd, env)?;
    let stdin = create_pipe(ParentPipeEnd::Writes).context("failed to create stdin pipe")?;
    let stdout = create_pipe(ParentPipeEnd::Reads).context("failed to create stdout pipe")?;
    let stderr = create_pipe(ParentPipeEnd::Reads).context("failed to create stderr pipe")?;

    let mut startup: STARTUPINFOEXW = unsafe { mem::zeroed() };
    startup.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin.child.as_raw_handle().cast();
    startup.StartupInfo.hStdOutput = stdout.child.as_raw_handle().cast();
    startup.StartupInfo.hStdError = stderr.child.as_raw_handle().cast();

    let mut child_handles = [
        stdin.child.as_raw_handle().cast(),
        stdout.child.as_raw_handle().cast(),
        stderr.child.as_raw_handle().cast(),
    ];
    let mut attributes = ProcThreadAttributeList::with_capacity(/*num_attributes*/ 1)?;
    attributes.set_handle_list(&mut child_handles)?;
    startup.lpAttributeList = attributes.as_mut_ptr();

    let job = KillOnCloseJob::new().context("failed to create process job")?;
    let mut process_information: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    let created = unsafe {
        CreateProcessW(
            command.application.as_ptr(),
            command.command_line.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            TRUE,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            command.environment.as_mut_ptr().cast(),
            command.current_directory.as_ptr(),
            &mut startup.StartupInfo,
            &mut process_information,
        )
    };
    if created == FALSE {
        return Err(io::Error::last_os_error()).context("failed to create pipe child process");
    }

    let suspended = unsafe {
        SuspendedProcess::from_raw_handles(
            process_information.hProcess.cast(),
            process_information.hThread.cast(),
            process_information.dwProcessId,
        )
    };
    drop(stdin.child);
    drop(stdout.child);
    drop(stderr.child);
    let stdin_parent = match stdin_mode {
        StdinMode::Piped => Some(stdin.parent),
        StdinMode::Null => {
            drop(stdin.parent);
            None
        }
    };
    let process = suspended
        .assign_and_resume(job)
        .context("failed to contain and resume pipe child process")?;
    let controller = process.controller();

    let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stderr_tx, stderr_rx) = mpsc::channel::<Vec<u8>>(128);

    let writer_handle = match stdin_parent {
        Some(stdin_parent) => spawn_writer(File::from(stdin_parent), writer_rx),
        None => {
            drop(writer_rx);
            tokio::spawn(async {})
        }
    };
    let stdout_reader = spawn_reader(File::from(stdout.parent), stdout_tx);
    let stderr_reader = spawn_reader(File::from(stderr.parent), stderr_tx);
    let reader_abort_handles = vec![stdout_reader.abort_handle(), stderr_reader.abort_handle()];
    let reader_handle = tokio::spawn(async move {
        let _ = stdout_reader.await;
        let _ = stderr_reader.await;
    });

    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let exit_code = Arc::new(StdMutex::new(None));
    let wait_exit_code = Arc::clone(&exit_code);
    let wait_handle: JoinHandle<()> = tokio::task::spawn_blocking(move || {
        let code = wait_for_process(&process).unwrap_or(-1);
        let _ = process.controller().close();
        wait_exit_status.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut guard) = wait_exit_code.lock() {
            *guard = Some(code);
        }
        let _ = exit_tx.send(code);
    });

    let handle = ProcessHandle::new(
        writer_tx,
        Box::new(JobTerminator { controller }),
        reader_handle,
        reader_abort_handles,
        writer_handle,
        wait_handle,
        exit_status,
        exit_code,
        /*pty_handles*/ None,
        /*resizer*/ None,
    );

    Ok(SpawnedProcess {
        session: handle,
        stdout_rx,
        stderr_rx,
        exit_rx,
    })
}

fn create_pipe(parent_end: ParentPipeEnd) -> io::Result<PipeEnds> {
    let mut attributes: SECURITY_ATTRIBUTES = unsafe { mem::zeroed() };
    attributes.nLength = mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
    attributes.bInheritHandle = TRUE;
    let mut read_handle: HANDLE = ptr::null_mut();
    let mut write_handle: HANDLE = ptr::null_mut();
    let created = unsafe {
        CreatePipe(
            &mut read_handle,
            &mut write_handle,
            &mut attributes,
            /*nSize*/ 0,
        )
    };
    if created == FALSE {
        return Err(io::Error::last_os_error());
    }

    let read_handle = unsafe { OwnedHandle::from_raw_handle(read_handle.cast()) };
    let write_handle = unsafe { OwnedHandle::from_raw_handle(write_handle.cast()) };
    let (parent, child) = match parent_end {
        ParentPipeEnd::Reads => (read_handle, write_handle),
        ParentPipeEnd::Writes => (write_handle, read_handle),
    };
    let result = unsafe {
        SetHandleInformation(
            parent.as_raw_handle().cast(),
            HANDLE_FLAG_INHERIT,
            /*dwFlags*/ 0,
        )
    };
    if result == FALSE {
        return Err(io::Error::last_os_error());
    }

    Ok(PipeEnds { parent, child })
}

fn spawn_writer(mut writer: File, mut input_rx: mpsc::Receiver<Vec<u8>>) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        while let Some(bytes) = input_rx.blocking_recv() {
            if writer
                .write_all(&bytes)
                .and_then(|()| writer.flush())
                .is_err()
            {
                break;
            }
        }
    })
}

fn spawn_reader(mut reader: File, output_tx: mpsc::Sender<Vec<u8>>) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut buffer = vec![0u8; 8_192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if output_tx
                        .blocking_send(buffer[..bytes_read].to_vec())
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    })
}

fn wait_for_process(process: &super::JobProcess) -> io::Result<i32> {
    let wait_result = unsafe { WaitForSingleObject(process.as_raw_handle().cast(), INFINITE) };
    if wait_result != WAIT_OBJECT_0 {
        return Err(io::Error::last_os_error());
    }

    let mut exit_code = 0;
    let result = unsafe { GetExitCodeProcess(process.as_raw_handle().cast(), &mut exit_code) };
    if result == FALSE {
        Err(io::Error::last_os_error())
    } else {
        Ok(exit_code as i32)
    }
}
