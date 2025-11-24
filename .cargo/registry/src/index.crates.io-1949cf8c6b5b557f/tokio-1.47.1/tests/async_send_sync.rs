#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![allow(clippy::type_complexity, clippy::diverging_sub_expression)]

use std::cell::Cell;
use std::future::Future;
use std::io::SeekFrom;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use tokio::net::TcpStream;
use tokio::time::{Duration, Instant};

// The names of these structs behaves better when sorted.
// Send: Yes, Sync: Yes
#[derive(Clone)]
#[allow(unused)]
struct YY {}

// Send: Yes, Sync: No
#[derive(Clone)]
#[allow(unused)]
struct YN {
    _value: Cell<u8>,
}

// Send: No, Sync: No
#[derive(Clone)]
#[allow(unused)]
struct NN {
    _value: Rc<u8>,
}

#[allow(dead_code)]
type BoxFutureSync<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + Sync>>;
#[allow(dead_code)]
type BoxFutureSend<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;
#[allow(dead_code)]
type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T>>>;

#[allow(dead_code)]
type BoxAsyncRead = std::pin::Pin<Box<dyn tokio::io::AsyncBufRead + Send + Sync>>;
#[allow(dead_code)]
type BoxAsyncSeek = std::pin::Pin<Box<dyn tokio::io::AsyncSeek + Send + Sync>>;
#[allow(dead_code)]
type BoxAsyncWrite = std::pin::Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>;

#[allow(dead_code)]
fn require_send<T: Send>(_t: &T) {}
#[allow(dead_code)]
fn require_sync<T: Sync>(_t: &T) {}
#[allow(dead_code)]
fn require_unpin<T: Unpin>(_t: &T) {}

#[allow(dead_code)]
struct Invalid;

#[allow(unused)]
trait AmbiguousIfSend<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
impl<T: ?Sized + Send> AmbiguousIfSend<Invalid> for T {}

#[allow(unused)]
trait AmbiguousIfSync<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfSync<()> for T {}
impl<T: ?Sized + Sync> AmbiguousIfSync<Invalid> for T {}

#[allow(unused)]
trait AmbiguousIfUnpin<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfUnpin<()> for T {}
impl<T: ?Sized + Unpin> AmbiguousIfUnpin<Invalid> for T {}

macro_rules! into_todo {
    ($typ:ty) => {{
        let x: $typ = todo!();
        x
    }};
}

macro_rules! async_assert_fn_send {
    (Send & $(!)?Sync & $(!)?Unpin, $value:expr) => {
        require_send(&$value);
    };
    (!Send & $(!)?Sync & $(!)?Unpin, $value:expr) => {
        AmbiguousIfSend::some_item(&$value);
    };
}
macro_rules! async_assert_fn_sync {
    ($(!)?Send & Sync & $(!)?Unpin, $value:expr) => {
        require_sync(&$value);
    };
    ($(!)?Send & !Sync & $(!)?Unpin, $value:expr) => {
        AmbiguousIfSync::some_item(&$value);
    };
}
macro_rules! async_assert_fn_unpin {
    ($(!)?Send & $(!)?Sync & Unpin, $value:expr) => {
        require_unpin(&$value);
    };
    ($(!)?Send & $(!)?Sync & !Unpin, $value:expr) => {
        AmbiguousIfUnpin::some_item(&$value);
    };
}

macro_rules! async_assert_fn {
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): $($tok:tt)*) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            async_assert_fn_send!($($tok)*, f);
            async_assert_fn_sync!($($tok)*, f);
            async_assert_fn_unpin!($($tok)*, f);
        };
    };
}
macro_rules! assert_value {
    ($type:ty: $($tok:tt)*) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f: $type = todo!();
            async_assert_fn_send!($($tok)*, f);
            async_assert_fn_sync!($($tok)*, f);
            async_assert_fn_unpin!($($tok)*, f);
        };
    };
}

macro_rules! cfg_not_wasi {
    ($($item:item)*) => {
        $(
            #[cfg(not(target_os = "wasi"))]
            $item
        )*
    }
}

// Manually re-implementation of `async_assert_fn` for `poll_fn`. The macro
// doesn't work for this particular case because constructing the closure
// is too complicated.
const _: fn() = || {
    let pinned = std::marker::PhantomPinned;
    let f = tokio::macros::support::poll_fn(move |_| {
        // Use `pinned` to take ownership of it.
        let _ = &pinned;
        std::task::Poll::Pending::<()>
    });
    require_send(&f);
    require_sync(&f);
    AmbiguousIfUnpin::some_item(&f);
};

cfg_not_wasi! {
    mod fs {
        use super::*;
        assert_value!(tokio::fs::DirBuilder: Send & Sync & Unpin);
        assert_value!(tokio::fs::DirEntry: Send & Sync & Unpin);
        assert_value!(tokio::fs::File: Send & Sync & Unpin);
        assert_value!(tokio::fs::OpenOptions: Send & Sync & Unpin);
        assert_value!(tokio::fs::ReadDir: Send & Sync & Unpin);

        async_assert_fn!(tokio::fs::canonicalize(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::copy(&str, &str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::create_dir(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::create_dir_all(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::hard_link(&str, &str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::metadata(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::read(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::read_dir(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::read_link(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::read_to_string(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::remove_dir(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::remove_dir_all(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::remove_file(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::rename(&str, &str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::set_permissions(&str, std::fs::Permissions): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::symlink_metadata(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::write(&str, Vec<u8>): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::ReadDir::next_entry(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::OpenOptions::open(_, &str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::DirBuilder::create(_, &str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::DirEntry::metadata(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::DirEntry::file_type(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::open(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::create(&str): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::sync_all(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::sync_data(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::set_len(_, u64): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::metadata(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::try_clone(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::fs::File::into_std(_): Send & Sync & !Unpin);
        async_assert_fn!(
            tokio::fs::File::set_permissions(_, std::fs::Permissions): Send & Sync & !Unpin
        );
    }
}

cfg_not_wasi! {
    assert_value!(tokio::net::TcpSocket: Send & Sync & Unpin);
    async_assert_fn!(tokio::net::TcpListener::bind(SocketAddr): Send & Sync & !Unpin);
    async_assert_fn!(tokio::net::TcpStream::connect(SocketAddr): Send & Sync & !Unpin);
}

assert_value!(tokio::net::TcpListener: Send & Sync & Unpin);
assert_value!(tokio::net::TcpStream: Send & Sync & Unpin);
assert_value!(tokio::net::tcp::OwnedReadHalf: Send & Sync & Unpin);
assert_value!(tokio::net::tcp::OwnedWriteHalf: Send & Sync & Unpin);
assert_value!(tokio::net::tcp::ReadHalf<'_>: Send & Sync & Unpin);
assert_value!(tokio::net::tcp::ReuniteError: Send & Sync & Unpin);
assert_value!(tokio::net::tcp::WriteHalf<'_>: Send & Sync & Unpin);
async_assert_fn!(tokio::net::TcpListener::accept(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::net::TcpStream::peek(_, &mut [u8]): Send & Sync & !Unpin);
async_assert_fn!(tokio::net::TcpStream::readable(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::net::TcpStream::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
async_assert_fn!(tokio::net::TcpStream::writable(_): Send & Sync & !Unpin);

// Wasi does not support UDP
cfg_not_wasi! {
    mod udp_socket {
        use super::*;
        assert_value!(tokio::net::UdpSocket: Send & Sync & Unpin);
        async_assert_fn!(tokio::net::UdpSocket::bind(SocketAddr): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::connect(_, SocketAddr): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::peek_from(_, &mut [u8]): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::readable(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::recv(_, &mut [u8]): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::recv_from(_, &mut [u8]): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::send(_, &[u8]): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::send_to(_, &[u8], SocketAddr): Send & Sync & !Unpin);
        async_assert_fn!(tokio::net::UdpSocket::writable(_): Send & Sync & !Unpin);
    }
}
async_assert_fn!(tokio::net::lookup_host(SocketAddr): Send & Sync & !Unpin);
async_assert_fn!(tokio::net::tcp::ReadHalf::peek(_, &mut [u8]): Send & Sync & !Unpin);

#[cfg(unix)]
mod unix_datagram {
    use super::*;
    use tokio::net::*;
    assert_value!(UnixDatagram: Send & Sync & Unpin);
    assert_value!(UnixListener: Send & Sync & Unpin);
    assert_value!(UnixStream: Send & Sync & Unpin);
    assert_value!(unix::OwnedReadHalf: Send & Sync & Unpin);
    assert_value!(unix::OwnedWriteHalf: Send & Sync & Unpin);
    assert_value!(unix::ReadHalf<'_>: Send & Sync & Unpin);
    assert_value!(unix::ReuniteError: Send & Sync & Unpin);
    assert_value!(unix::SocketAddr: Send & Sync & Unpin);
    assert_value!(unix::UCred: Send & Sync & Unpin);
    assert_value!(unix::WriteHalf<'_>: Send & Sync & Unpin);
    async_assert_fn!(UnixDatagram::readable(_): Send & Sync & !Unpin);
    async_assert_fn!(UnixDatagram::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
    async_assert_fn!(UnixDatagram::recv(_, &mut [u8]): Send & Sync & !Unpin);
    async_assert_fn!(UnixDatagram::recv_from(_, &mut [u8]): Send & Sync & !Unpin);
    async_assert_fn!(UnixDatagram::send(_, &[u8]): Send & Sync & !Unpin);
    async_assert_fn!(UnixDatagram::send_to(_, &[u8], &str): Send & Sync & !Unpin);
    async_assert_fn!(UnixDatagram::writable(_): Send & Sync & !Unpin);
    async_assert_fn!(UnixListener::accept(_): Send & Sync & !Unpin);
    async_assert_fn!(UnixStream::connect(&str): Send & Sync & !Unpin);
    async_assert_fn!(UnixStream::readable(_): Send & Sync & !Unpin);
    async_assert_fn!(UnixStream::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
    async_assert_fn!(UnixStream::writable(_): Send & Sync & !Unpin);
}

#[cfg(unix)]
mod unix_pipe {
    use super::*;
    use tokio::net::unix::pipe::*;
    assert_value!(OpenOptions: Send & Sync & Unpin);
    assert_value!(Receiver: Send & Sync & Unpin);
    assert_value!(Sender: Send & Sync & Unpin);
    async_assert_fn!(Receiver::readable(_): Send & Sync & !Unpin);
    async_assert_fn!(Receiver::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
    async_assert_fn!(Sender::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
    async_assert_fn!(Sender::writable(_): Send & Sync & !Unpin);
}

#[cfg(windows)]
mod windows_named_pipe {
    use super::*;
    use tokio::net::windows::named_pipe::*;
    assert_value!(ClientOptions: Send & Sync & Unpin);
    assert_value!(NamedPipeClient: Send & Sync & Unpin);
    assert_value!(NamedPipeServer: Send & Sync & Unpin);
    assert_value!(PipeEnd: Send & Sync & Unpin);
    assert_value!(PipeInfo: Send & Sync & Unpin);
    assert_value!(PipeMode: Send & Sync & Unpin);
    assert_value!(ServerOptions: Send & Sync & Unpin);
    async_assert_fn!(NamedPipeClient::readable(_): Send & Sync & !Unpin);
    async_assert_fn!(NamedPipeClient::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
    async_assert_fn!(NamedPipeClient::writable(_): Send & Sync & !Unpin);
    async_assert_fn!(NamedPipeServer::connect(_): Send & Sync & !Unpin);
    async_assert_fn!(NamedPipeServer::readable(_): Send & Sync & !Unpin);
    async_assert_fn!(NamedPipeServer::ready(_, tokio::io::Interest): Send & Sync & !Unpin);
    async_assert_fn!(NamedPipeServer::writable(_): Send & Sync & !Unpin);
}

cfg_not_wasi! {
    mod test_process {
        use super::*;
        assert_value!(tokio::process::Child: Send & Sync & Unpin);
        assert_value!(tokio::process::ChildStderr: Send & Sync & Unpin);
        assert_value!(tokio::process::ChildStdin: Send & Sync & Unpin);
        assert_value!(tokio::process::ChildStdout: Send & Sync & Unpin);
        assert_value!(tokio::process::Command: Send & Sync & Unpin);
        async_assert_fn!(tokio::process::Child::kill(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::process::Child::wait(_): Send & Sync & !Unpin);
        async_assert_fn!(tokio::process::Child::wait_with_output(_): Send & Sync & !Unpin);
    }

    async_assert_fn!(tokio::signal::ctrl_c(): Send & Sync & !Unpin);
}

#[cfg(unix)]
mod unix_signal {
    use super::*;
    assert_value!(tokio::signal::unix::Signal: Send & Sync & Unpin);
    assert_value!(tokio::signal::unix::SignalKind: Send & Sync & Unpin);
    async_assert_fn!(tokio::signal::unix::Signal::recv(_): Send & Sync & !Unpin);
}
#[cfg(windows)]
mod windows_signal {
    use super::*;
    assert_value!(tokio::signal::windows::CtrlC: Send & Sync & Unpin);
    assert_value!(tokio::signal::windows::CtrlBreak: Send & Sync & Unpin);
    async_assert_fn!(tokio::signal::windows::CtrlC::recv(_): Send & Sync & !Unpin);
    async_assert_fn!(tokio::signal::windows::CtrlBreak::recv(_): Send & Sync & !Unpin);
}

assert_value!(tokio::sync::AcquireError: Send & Sync & Unpin);
assert_value!(tokio::sync::Barrier: Send & Sync & Unpin);
assert_value!(tokio::sync::BarrierWaitResult: Send & Sync & Unpin);
assert_value!(tokio::sync::MappedMutexGuard<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::MappedMutexGuard<'_, YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::MappedMutexGuard<'_, YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::Mutex<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::Mutex<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::Mutex<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::MutexGuard<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::MutexGuard<'_, YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::MutexGuard<'_, YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::Notify: Send & Sync & Unpin);
assert_value!(tokio::sync::OnceCell<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OnceCell<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::OnceCell<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::SetOnce<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::SetOnce<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::SetOnce<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::OwnedMutexGuard<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMutexGuard<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMutexGuard<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<NN,NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<NN,YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<NN,YY>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<YN,NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<YN,YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<YN,YY>: Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<YY,NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<YY,YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedMappedMutexGuard<YY,YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockMappedWriteGuard<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockMappedWriteGuard<YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockMappedWriteGuard<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockReadGuard<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockReadGuard<YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockReadGuard<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockWriteGuard<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockWriteGuard<YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::OwnedRwLockWriteGuard<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::OwnedSemaphorePermit: Send & Sync & Unpin);
assert_value!(tokio::sync::RwLock<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLock<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLock<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::RwLockMappedWriteGuard<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLockMappedWriteGuard<'_, YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLockMappedWriteGuard<'_, YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::RwLockReadGuard<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLockReadGuard<'_, YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLockReadGuard<'_, YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::RwLockWriteGuard<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLockWriteGuard<'_, YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::RwLockWriteGuard<'_, YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::Semaphore: Send & Sync & Unpin);
assert_value!(tokio::sync::SemaphorePermit<'_>: Send & Sync & Unpin);
assert_value!(tokio::sync::TryAcquireError: Send & Sync & Unpin);
assert_value!(tokio::sync::TryLockError: Send & Sync & Unpin);
assert_value!(tokio::sync::broadcast::Receiver<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::broadcast::Receiver<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::broadcast::Receiver<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::broadcast::Sender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::broadcast::Sender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::broadcast::Sender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::broadcast::WeakSender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::broadcast::WeakSender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::broadcast::WeakSender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::futures::Notified<'_>: Send & Sync & !Unpin);
assert_value!(tokio::sync::futures::OwnedNotified: Send & Sync & !Unpin);
assert_value!(tokio::sync::mpsc::OwnedPermit<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::OwnedPermit<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::OwnedPermit<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::Permit<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::Permit<'_, YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::Permit<'_, YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::Receiver<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::Receiver<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::Receiver<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::Sender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::Sender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::Sender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::UnboundedReceiver<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::UnboundedReceiver<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::UnboundedReceiver<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::UnboundedSender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::UnboundedSender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::UnboundedSender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::WeakSender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::WeakSender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::WeakSender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::WeakUnboundedSender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::WeakUnboundedSender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::WeakUnboundedSender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::SendError<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::SendError<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::SendError<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::SendTimeoutError<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::SendTimeoutError<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::SendTimeoutError<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::TrySendError<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::TrySendError<YN>: Send & !Sync & Unpin);
assert_value!(tokio::sync::mpsc::error::TrySendError<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::oneshot::Receiver<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::oneshot::Receiver<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::oneshot::Receiver<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::oneshot::Sender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::oneshot::Sender<YN>: Send & Sync & Unpin);
assert_value!(tokio::sync::oneshot::Sender<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::watch::Receiver<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::watch::Receiver<YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::watch::Receiver<YY>: Send & Sync & Unpin);
assert_value!(tokio::sync::watch::Ref<'_, NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::watch::Ref<'_, YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::watch::Ref<'_, YY>: !Send & Sync & Unpin);
assert_value!(tokio::sync::watch::Sender<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::watch::Sender<YN>: !Send & !Sync & Unpin);
assert_value!(tokio::sync::watch::Sender<YY>: Send & Sync & Unpin);
assert_value!(tokio::task::JoinError: Send & Sync & Unpin);
assert_value!(tokio::task::JoinHandle<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::task::JoinHandle<YN>: Send & Sync & Unpin);
assert_value!(tokio::task::JoinHandle<YY>: Send & Sync & Unpin);
assert_value!(tokio::task::JoinSet<NN>: !Send & !Sync & Unpin);
assert_value!(tokio::task::JoinSet<YN>: Send & Sync & Unpin);
assert_value!(tokio::task::JoinSet<YY>: Send & Sync & Unpin);
assert_value!(tokio::task::LocalSet: !Send & !Sync & Unpin);
assert_value!(tokio::task::coop::RestoreOnPending: !Send & !Sync & Unpin);
async_assert_fn!(tokio::sync::Barrier::wait(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Mutex<NN>::lock(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::Mutex<NN>::lock_owned(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::Mutex<YN>::lock(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Mutex<YN>::lock_owned(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Mutex<YY>::lock(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Mutex<YY>::lock_owned(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Notify::notified(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<NN>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = NN> + Send + Sync>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<NN>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = NN> + Send>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<NN>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = NN>>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<NN>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<NN>> + Send + Sync>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<NN>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<NN>> + Send>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<NN>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<NN>>>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YN>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = YN> + Send + Sync>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YN>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = YN> + Send>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YN>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = YN>>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YN>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<YN>> + Send + Sync>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YN>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<YN>> + Send>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YN>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<YN>>>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YY>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = YY> + Send + Sync>>): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YY>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = YY> + Send>>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YY>::get_or_init( _, fn() -> Pin<Box<dyn Future<Output = YY>>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YY>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<YY>> + Send + Sync>>): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YY>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<YY>> + Send>>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::OnceCell<YY>::get_or_try_init( _, fn() -> Pin<Box<dyn Future<Output = std::io::Result<YY>>>>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::SetOnce<NN>::wait(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::SetOnce<YN>::wait(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::SetOnce<YY>::wait(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::RwLock<NN>::read(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::RwLock<NN>::write(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::RwLock<YN>::read(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::RwLock<YN>::write(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::RwLock<YY>::read(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::RwLock<YY>::write(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Semaphore::acquire(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Semaphore::acquire_many(_, u32): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Semaphore::acquire_many_owned(_, u32): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::Semaphore::acquire_owned(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::broadcast::Receiver<NN>::recv(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::broadcast::Receiver<YN>::recv(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::broadcast::Receiver<YY>::recv(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Receiver<NN>::recv(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Receiver<YN>::recv(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Receiver<YY>::recv(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<NN>::closed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<NN>::reserve(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<NN>::reserve_owned(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<NN>::send(_, NN): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<NN>::send_timeout(_, NN, Duration): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YN>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YN>::reserve(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YN>::reserve_owned(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YN>::send(_, YN): Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YN>::send_timeout(_, YN, Duration): Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YY>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YY>::reserve(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YY>::reserve_owned(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YY>::send(_, YY): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::Sender<YY>::send_timeout(_, YY, Duration): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::UnboundedReceiver<NN>::recv(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::UnboundedReceiver<YN>::recv(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::UnboundedReceiver<YY>::recv(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::UnboundedSender<NN>::closed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::UnboundedSender<YN>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::mpsc::UnboundedSender<YY>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::oneshot::Sender<NN>::closed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::oneshot::Sender<YN>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::oneshot::Sender<YY>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::watch::Receiver<NN>::changed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::watch::Receiver<YN>::changed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::watch::Receiver<YY>::changed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::sync::watch::Sender<NN>::closed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::watch::Sender<YN>::closed(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::sync::watch::Sender<YY>::closed(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::task::JoinSet<Cell<u32>>::join_next(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::task::JoinSet<Cell<u32>>::shutdown(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::task::JoinSet<Rc<u32>>::join_next(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::JoinSet<Rc<u32>>::shutdown(_): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::JoinSet<u32>::join_next(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::task::JoinSet<u32>::shutdown(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<Cell<u32>>::scope(_, Cell<u32>, BoxFuture<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<Cell<u32>>::scope(_, Cell<u32>, BoxFutureSend<()>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<Cell<u32>>::scope(_, Cell<u32>, BoxFutureSync<()>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<Rc<u32>>::scope(_, Rc<u32>, BoxFuture<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<Rc<u32>>::scope(_, Rc<u32>, BoxFutureSend<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<Rc<u32>>::scope(_, Rc<u32>, BoxFutureSync<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<u32>::scope(_, u32, BoxFuture<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<u32>::scope(_, u32, BoxFutureSend<()>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::LocalKey<u32>::scope(_, u32, BoxFutureSync<()>): Send & Sync & !Unpin);
async_assert_fn!(tokio::task::LocalSet::run_until(_, BoxFutureSync<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::task::unconstrained(BoxFuture<()>): !Send & !Sync & Unpin);
async_assert_fn!(tokio::task::unconstrained(BoxFutureSend<()>): Send & !Sync & Unpin);
async_assert_fn!(tokio::task::unconstrained(BoxFutureSync<()>): Send & Sync & Unpin);

assert_value!(tokio::runtime::Builder: Send & Sync & Unpin);
assert_value!(tokio::runtime::EnterGuard<'_>: !Send & Sync & Unpin);
assert_value!(tokio::runtime::Handle: Send & Sync & Unpin);
assert_value!(tokio::runtime::Runtime: Send & Sync & Unpin);

assert_value!(tokio::time::Interval: Send & Sync & Unpin);
assert_value!(tokio::time::Instant: Send & Sync & Unpin);
assert_value!(tokio::time::Sleep: Send & Sync & !Unpin);
assert_value!(tokio::time::Timeout<BoxFutureSync<()>>: Send & Sync & !Unpin);
assert_value!(tokio::time::Timeout<BoxFutureSend<()>>: Send & !Sync & !Unpin);
assert_value!(tokio::time::Timeout<BoxFuture<()>>: !Send & !Sync & !Unpin);
assert_value!(tokio::time::error::Elapsed: Send & Sync & Unpin);
assert_value!(tokio::time::error::Error: Send & Sync & Unpin);
async_assert_fn!(tokio::time::advance(Duration): Send & Sync & !Unpin);
async_assert_fn!(tokio::time::sleep(Duration): Send & Sync & !Unpin);
async_assert_fn!(tokio::time::sleep_until(Instant): Send & Sync & !Unpin);
async_assert_fn!(tokio::time::timeout(Duration, BoxFutureSync<()>): Send & Sync & !Unpin);
async_assert_fn!(tokio::time::timeout(Duration, BoxFutureSend<()>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::time::timeout(Duration, BoxFuture<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::time::timeout_at(Instant, BoxFutureSync<()>): Send & Sync & !Unpin);
async_assert_fn!(tokio::time::timeout_at(Instant, BoxFutureSend<()>): Send & !Sync & !Unpin);
async_assert_fn!(tokio::time::timeout_at(Instant, BoxFuture<()>): !Send & !Sync & !Unpin);
async_assert_fn!(tokio::time::Interval::tick(_): Send & Sync & !Unpin);

assert_value!(tokio::io::BufReader<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::BufStream<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::BufWriter<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::DuplexStream: Send & Sync & Unpin);
assert_value!(tokio::io::Empty: Send & Sync & Unpin);
assert_value!(tokio::io::Interest: Send & Sync & Unpin);
assert_value!(tokio::io::Lines<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::ReadBuf<'_>: Send & Sync & Unpin);
assert_value!(tokio::io::ReadHalf<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::Ready: Send & Sync & Unpin);
assert_value!(tokio::io::Repeat: Send & Sync & Unpin);
assert_value!(tokio::io::Sink: Send & Sync & Unpin);
assert_value!(tokio::io::Split<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::Stderr: Send & Sync & Unpin);
assert_value!(tokio::io::Stdin: Send & Sync & Unpin);
assert_value!(tokio::io::Stdout: Send & Sync & Unpin);
assert_value!(tokio::io::Take<TcpStream>: Send & Sync & Unpin);
assert_value!(tokio::io::WriteHalf<TcpStream>: Send & Sync & Unpin);
async_assert_fn!(tokio::io::copy(&mut TcpStream, &mut TcpStream): Send & Sync & !Unpin);
async_assert_fn!(
    tokio::io::copy_bidirectional(&mut TcpStream, &mut TcpStream): Send & Sync & !Unpin
);
async_assert_fn!(tokio::io::copy_buf(&mut tokio::io::BufReader<TcpStream>, &mut TcpStream): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::empty(): Send & Sync & Unpin);
async_assert_fn!(tokio::io::repeat(u8): Send & Sync & Unpin);
async_assert_fn!(tokio::io::sink(): Send & Sync & Unpin);
async_assert_fn!(tokio::io::split(TcpStream): Send & Sync & Unpin);
async_assert_fn!(tokio::io::stderr(): Send & Sync & Unpin);
async_assert_fn!(tokio::io::stdin(): Send & Sync & Unpin);
async_assert_fn!(tokio::io::stdout(): Send & Sync & Unpin);
async_assert_fn!(tokio::io::Split<tokio::io::BufReader<TcpStream>>::next_segment(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::Lines<tokio::io::BufReader<TcpStream>>::next_line(_): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncBufReadExt::read_until(&mut BoxAsyncRead, u8, &mut Vec<u8>): Send & Sync & !Unpin);
async_assert_fn!(
    tokio::io::AsyncBufReadExt::read_line(&mut BoxAsyncRead, &mut String): Send & Sync & !Unpin
);
async_assert_fn!(tokio::io::AsyncBufReadExt::fill_buf(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read(&mut BoxAsyncRead, &mut [u8]): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_buf(&mut BoxAsyncRead, &mut Vec<u8>): Send & Sync & !Unpin);
async_assert_fn!(
    tokio::io::AsyncReadExt::read_exact(&mut BoxAsyncRead, &mut [u8]): Send & Sync & !Unpin
);
async_assert_fn!(tokio::io::AsyncReadExt::read_u8(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i8(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u16(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i16(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u32(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i32(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u64(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i64(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u128(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i128(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_f32(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_f64(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u16_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i16_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u32_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i32_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u64_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i64_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_u128_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_i128_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_f32_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_f64_le(&mut BoxAsyncRead): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncReadExt::read_to_end(&mut BoxAsyncRead, &mut Vec<u8>): Send & Sync & !Unpin);
async_assert_fn!(
    tokio::io::AsyncReadExt::read_to_string(&mut BoxAsyncRead, &mut String): Send & Sync & !Unpin
);
async_assert_fn!(tokio::io::AsyncSeekExt::seek(&mut BoxAsyncSeek, SeekFrom): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncSeekExt::stream_position(&mut BoxAsyncSeek): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncWriteExt::write(&mut BoxAsyncWrite, &[u8]): Send & Sync & !Unpin);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_vectored(&mut BoxAsyncWrite, _): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_buf(&mut BoxAsyncWrite, &mut bytes::Bytes): Send
        & Sync
        & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_all_buf(&mut BoxAsyncWrite, &mut bytes::Bytes): Send
        & Sync
        & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_all(&mut BoxAsyncWrite, &[u8]): Send & Sync & !Unpin
);
async_assert_fn!(tokio::io::AsyncWriteExt::write_u8(&mut BoxAsyncWrite, u8): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncWriteExt::write_i8(&mut BoxAsyncWrite, i8): Send & Sync & !Unpin);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u16(&mut BoxAsyncWrite, u16): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i16(&mut BoxAsyncWrite, i16): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u32(&mut BoxAsyncWrite, u32): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i32(&mut BoxAsyncWrite, i32): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u64(&mut BoxAsyncWrite, u64): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i64(&mut BoxAsyncWrite, i64): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u128(&mut BoxAsyncWrite, u128): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i128(&mut BoxAsyncWrite, i128): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_f32(&mut BoxAsyncWrite, f32): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_f64(&mut BoxAsyncWrite, f64): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u16_le(&mut BoxAsyncWrite, u16): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i16_le(&mut BoxAsyncWrite, i16): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u32_le(&mut BoxAsyncWrite, u32): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i32_le(&mut BoxAsyncWrite, i32): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u64_le(&mut BoxAsyncWrite, u64): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i64_le(&mut BoxAsyncWrite, i64): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_u128_le(&mut BoxAsyncWrite, u128): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_i128_le(&mut BoxAsyncWrite, i128): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_f32_le(&mut BoxAsyncWrite, f32): Send & Sync & !Unpin
);
async_assert_fn!(
    tokio::io::AsyncWriteExt::write_f64_le(&mut BoxAsyncWrite, f64): Send & Sync & !Unpin
);
async_assert_fn!(tokio::io::AsyncWriteExt::flush(&mut BoxAsyncWrite): Send & Sync & !Unpin);
async_assert_fn!(tokio::io::AsyncWriteExt::shutdown(&mut BoxAsyncWrite): Send & Sync & !Unpin);

#[cfg(unix)]
mod unix_asyncfd {
    use super::*;
    use tokio::io::unix::*;

    #[allow(unused)]
    struct ImplsFd<T> {
        _t: T,
    }
    impl<T> std::os::unix::io::AsRawFd for ImplsFd<T> {
        fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
            unreachable!()
        }
    }

    assert_value!(AsyncFd<ImplsFd<YY>>: Send & Sync & Unpin);
    assert_value!(AsyncFd<ImplsFd<YN>>: Send & !Sync & Unpin);
    assert_value!(AsyncFd<ImplsFd<NN>>: !Send & !Sync & Unpin);
    assert_value!(AsyncFdReadyGuard<'_, ImplsFd<YY>>: Send & Sync & Unpin);
    assert_value!(AsyncFdReadyGuard<'_, ImplsFd<YN>>: !Send & !Sync & Unpin);
    assert_value!(AsyncFdReadyGuard<'_, ImplsFd<NN>>: !Send & !Sync & Unpin);
    assert_value!(AsyncFdReadyMutGuard<'_, ImplsFd<YY>>: Send & Sync & Unpin);
    assert_value!(AsyncFdReadyMutGuard<'_, ImplsFd<YN>>: Send & !Sync & Unpin);
    assert_value!(AsyncFdReadyMutGuard<'_, ImplsFd<NN>>: !Send & !Sync & Unpin);
    assert_value!(TryIoError: Send & Sync & Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YY>>::readable(_): Send & Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YY>>::readable_mut(_): Send & Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YY>>::writable(_): Send & Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YY>>::writable_mut(_): Send & Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YN>>::readable(_): !Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YN>>::readable_mut(_): Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YN>>::writable(_): !Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<YN>>::writable_mut(_): Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<NN>>::readable(_): !Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<NN>>::readable_mut(_): !Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<NN>>::writable(_): !Send & !Sync & !Unpin);
    async_assert_fn!(AsyncFd<ImplsFd<NN>>::writable_mut(_): !Send & !Sync & !Unpin);
}

#[cfg(tokio_unstable)]
mod unstable {
    use super::*;

    assert_value!(tokio::runtime::LocalRuntime: !Send & !Sync & Unpin);
    assert_value!(tokio::runtime::LocalOptions: !Send & !Sync & Unpin);
}
