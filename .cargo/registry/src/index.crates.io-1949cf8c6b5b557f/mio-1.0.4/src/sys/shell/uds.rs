pub(crate) mod datagram {
    use std::io;
    use std::os::unix::net::{self, SocketAddr};

    pub(crate) fn bind_addr(_: &SocketAddr) -> io::Result<net::UnixDatagram> {
        os_required!()
    }

    pub(crate) fn unbound() -> io::Result<net::UnixDatagram> {
        os_required!()
    }

    pub(crate) fn pair() -> io::Result<(net::UnixDatagram, net::UnixDatagram)> {
        os_required!()
    }
}

pub(crate) mod listener {
    use std::io;
    use std::os::unix::net::{self, SocketAddr};

    use crate::net::UnixStream;

    pub(crate) fn bind_addr(_: &SocketAddr) -> io::Result<net::UnixListener> {
        os_required!()
    }

    pub(crate) fn accept(_: &net::UnixListener) -> io::Result<(UnixStream, SocketAddr)> {
        os_required!()
    }
}

pub(crate) mod stream {
    use std::io;
    use std::os::unix::net::{self, SocketAddr};

    pub(crate) fn connect_addr(_: &SocketAddr) -> io::Result<net::UnixStream> {
        os_required!()
    }

    pub(crate) fn pair() -> io::Result<(net::UnixStream, net::UnixStream)> {
        os_required!()
    }
}
