//! This module implements a serial port based tty.
//! This is a bit different from the other implementations in that
//! we cannot explicitly spawn a process into the serial connection,
//! so we can only use a `CommandBuilder::new_default_prog` with the
//! `openpty` method.
//! On most (all?) systems, attempting to open multiple instances of
//! the same serial port will fail.
use crate::{
    Child, ChildKiller, CommandBuilder, ExitStatus, MasterPty, PtyPair, PtySize, PtySystem,
    SlavePty,
};
use anyhow::{ensure, Context};
use filedescriptor::FileDescriptor;
use serial2::{CharSize, FlowControl, Parity, SerialPort, StopBits};
use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Result as IoResult, Write};
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

type Handle = Arc<SerialPort>;

pub struct SerialTty {
    port: OsString,
    baud: u32,
    char_size: CharSize,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
}

impl SerialTty {
    pub fn new<T: AsRef<OsStr> + ?Sized>(port: &T) -> Self {
        Self {
            port: port.as_ref().to_owned(),
            baud: 9600,
            char_size: CharSize::Bits8,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow_control: FlowControl::XonXoff,
        }
    }

    pub fn set_baud_rate(&mut self, baud: u32) {
        self.baud = baud;
    }

    pub fn set_char_size(&mut self, char_size: CharSize) {
        self.char_size = char_size;
    }

    pub fn set_parity(&mut self, parity: Parity) {
        self.parity = parity;
    }

    pub fn set_stop_bits(&mut self, stop_bits: StopBits) {
        self.stop_bits = stop_bits;
    }

    pub fn set_flow_control(&mut self, flow_control: FlowControl) {
        self.flow_control = flow_control;
    }
}

impl PtySystem for SerialTty {
    fn openpty(&self, _size: PtySize) -> anyhow::Result<PtyPair> {
        let mut port = SerialPort::open(&self.port, self.baud)
            .with_context(|| format!("openpty on serial port {:?}", self.port))?;

        let mut settings = port.get_configuration()?;
        settings.set_raw();
        settings.set_baud_rate(self.baud)?;
        settings.set_char_size(self.char_size);
        settings.set_flow_control(self.flow_control);
        settings.set_parity(self.parity);
        settings.set_stop_bits(self.stop_bits);
        log::debug!("serial settings: {:#?}", port.get_configuration());
        port.set_configuration(&settings)?;

        // The timeout needs to be rather short because, at least on Windows,
        // a read with a long timeout will block a concurrent write from
        // happening.  In wezterm we tend to have a thread looping on read
        // while writes happen occasionally from the gui thread, and if we
        // make this timeout too long we can block the gui thread.
        port.set_read_timeout(Duration::from_millis(50))?;
        port.set_write_timeout(Duration::from_millis(50))?;

        let port: Handle = Arc::new(port);

        Ok(PtyPair {
            slave: Box::new(Slave {
                port: Arc::clone(&port),
            }),
            master: Box::new(Master {
                port,
                took_writer: RefCell::new(false),
            }),
        })
    }
}

struct Slave {
    port: Handle,
}

impl SlavePty for Slave {
    fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<Box<dyn Child + Send + Sync>> {
        ensure!(
            cmd.is_default_prog(),
            "can only use default prog commands with serial tty implementations"
        );
        Ok(Box::new(SerialChild {
            port: Arc::clone(&self.port),
        }))
    }
}

/// There isn't really a child process on the end of the serial connection,
/// so all of the Child trait impls are NOP
struct SerialChild {
    port: Handle,
}

// An anemic impl of Debug to satisfy some indirect trait bounds
impl std::fmt::Debug for SerialChild {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("SerialChild").finish()
    }
}

impl Child for SerialChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        Ok(None)
    }

    fn wait(&mut self) -> IoResult<ExitStatus> {
        // There isn't really a child process to wait for,
        // as the serial connection never really "dies",
        // however, for something like a USB serial port,
        // if it is unplugged then it logically is terminated.
        // We read the CD (carrier detect) signal periodically
        // to see if the device has gone away: we actually discard
        // the CD value itself and just look for an error state.
        // We could potentially also decide to call CD==false the
        // same thing as the "child" completing.
        loop {
            std::thread::sleep(Duration::from_secs(5));

            let port = &self.port;
            if let Err(err) = port.read_cd() {
                log::error!("Error reading carrier detect: {:#}", err);
                return Ok(ExitStatus::with_exit_code(1));
            }
        }
    }

    fn process_id(&self) -> Option<u32> {
        None
    }

    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
    }
}

impl ChildKiller for SerialChild {
    fn kill(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(SerialChildKiller)
    }
}

#[derive(Debug)]
struct SerialChildKiller;

impl ChildKiller for SerialChildKiller {
    fn kill(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(SerialChildKiller)
    }
}

struct Master {
    port: Handle,
    took_writer: RefCell<bool>,
}

struct MasterWriter {
    port: Handle,
}

impl Write for MasterWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.port.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.port.flush()
    }
}

impl MasterPty for Master {
    fn resize(&self, _size: PtySize) -> anyhow::Result<()> {
        // Serial ports have no concept of size
        Ok(())
    }

    fn get_size(&self) -> anyhow::Result<PtySize> {
        // Serial ports have no concept of size
        Ok(PtySize::default())
    }

    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        // We rely on the fact that SystemPort implements the traits
        // that expose the underlying file descriptor, and that direct
        // reads from that return the raw data that we want
        let fd = FileDescriptor::dup(&*self.port)?;
        Ok(Box::new(Reader { fd }))
    }

    fn take_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send>> {
        if *self.took_writer.borrow() {
            anyhow::bail!("cannot take writer more than once");
        }
        *self.took_writer.borrow_mut() = true;
        let port = Arc::clone(&self.port);
        Ok(Box::new(MasterWriter { port }))
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<libc::pid_t> {
        // N/A: there is no local process
        None
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<crate::unix::RawFd> {
        None
    }

    #[cfg(unix)]
    fn tty_name(&self) -> Option<PathBuf> {
        None
    }
}

struct Reader {
    fd: FileDescriptor,
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        // On windows, this self.fd.read will block for up to the time we set
        // as the timeout when we set up the port, but on unix it will
        // never block.
        loop {
            #[cfg(unix)]
            {
                use filedescriptor::{poll, pollfd, AsRawSocketDescriptor, POLLIN};
                // The serial crate puts the serial port in non-blocking mode,
                // so we must explicitly poll for ourselves here to avoid a
                // busy loop.
                let mut poll_array = [pollfd {
                    fd: self.fd.as_socket_descriptor(),
                    events: POLLIN,
                    revents: 0,
                }];
                let _ = poll(&mut poll_array, None);
            }

            match self.fd.read(buf) {
                Ok(0) => {
                    if cfg!(windows) {
                        // Read timeout with no data available yet;
                        // loop and try again.
                        continue;
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "EOF on serial port",
                    ));
                }
                Ok(size) => {
                    return Ok(size);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    log::error!("serial read error: {}", e);
                    return Err(e);
                }
            }
        }
    }
}
