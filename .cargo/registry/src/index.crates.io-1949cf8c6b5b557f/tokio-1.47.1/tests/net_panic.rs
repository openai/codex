#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))]
#![cfg(panic = "unwind")]

use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::{Builder, Runtime};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn udp_socket_from_std_panic_caller() -> Result<(), Box<dyn Error>> {
    use std::net::SocketAddr;
    use tokio::net::UdpSocket;

    let addr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
    let std_sock = std::net::UdpSocket::bind(addr).unwrap();
    std_sock.set_nonblocking(true).unwrap();

    let panic_location_file = test_panic(|| {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _sock = UdpSocket::from_std(std_sock);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn tcp_listener_from_std_panic_caller() -> Result<(), Box<dyn Error>> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();

    let panic_location_file = test_panic(|| {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _ = TcpListener::from_std(std_listener);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn tcp_stream_from_std_panic_caller() -> Result<(), Box<dyn Error>> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();

    let std_stream = std::net::TcpStream::connect(std_listener.local_addr().unwrap()).unwrap();
    std_stream.set_nonblocking(true).unwrap();

    let panic_location_file = test_panic(|| {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _ = TcpStream::from_std(std_stream);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn unix_listener_bind_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::net::UnixListener;

    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("socket");

    let panic_location_file = test_panic(|| {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _ = UnixListener::bind(&sock_path);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn unix_listener_from_std_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::net::UnixListener;

    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("socket");
    let std_listener = std::os::unix::net::UnixListener::bind(sock_path).unwrap();

    let panic_location_file = test_panic(|| {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _ = UnixListener::from_std(std_listener);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn unix_stream_from_std_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::net::UnixStream;

    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("socket");
    let _std_listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
    let std_stream = std::os::unix::net::UnixStream::connect(&sock_path).unwrap();

    let panic_location_file = test_panic(|| {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _ = UnixStream::from_std(std_stream);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(unix)]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn unix_datagram_from_std_panic_caller() -> Result<(), Box<dyn Error>> {
    use std::os::unix::net::UnixDatagram as StdUDS;
    use tokio::net::UnixDatagram;

    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("socket");

    // Bind the socket to a filesystem path
    // /let socket_path = tmp.path().join("socket");
    let std_socket = StdUDS::bind(sock_path).unwrap();
    std_socket.set_nonblocking(true).unwrap();

    let panic_location_file = test_panic(move || {
        let rt = runtime_without_io();
        rt.block_on(async {
            let _ = UnixDatagram::from_std(std_socket);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg(windows)]
fn server_options_max_instances_panic_caller() -> Result<(), Box<dyn Error>> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let panic_location_file = test_panic(move || {
        let rt = runtime_without_io();
        rt.block_on(async {
            let mut options = ServerOptions::new();
            options.max_instances(255);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

// Runtime without `enable_io` so it has no IO driver set.
fn runtime_without_io() -> Runtime {
    Builder::new_current_thread().build().unwrap()
}
