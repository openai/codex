//! Translate a TUN worker’s TCP destination to the original hostname or literal IP
//! for the managed SOCKS5 bridge.

use crate::managed_tcp_dns::Names;
use crate::proxy_routing::proxy_bidirectional;
use socks5_protocol::Address;
use socks5_protocol::AuthMethod;
use socks5_protocol::AuthRequest;
use socks5_protocol::AuthResponse;
use socks5_protocol::Command as SocksCommand;
use socks5_protocol::CommandReply;
use socks5_protocol::CommandRequest;
use socks5_protocol::CommandResponse;
use socks5_protocol::Version;
use socks5_protocol::sync::FromIO;
use std::io;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

fn relay(
    mut client: TcpStream,
    proxy: SocketAddr,
    names: &Mutex<Names>,
) -> socks5_protocol::Result<()> {
    client.set_read_timeout(Some(TIMEOUT))?;
    client.set_write_timeout(Some(TIMEOUT))?;
    Version::read_from(&mut client)?;
    let offered = AuthRequest::read_from(&mut client)?;
    if !offered.0.contains(&AuthMethod::Noauth) {
        return Err(io::Error::other("unsupported SOCKS greeting").into());
    }
    Version::V5.write_to(&mut client)?;
    AuthResponse::new(AuthMethod::Noauth).write_to(&mut client)?;

    let upstream = match connect(&mut client, proxy, names) {
        Ok(stream) => stream,
        Err(err) => {
            let denied = CommandResponse::reply_error(CommandReply::ConnectionNotAllowedByRuleset);
            let _ = write_frame(&mut client, &denied);
            return Err(err);
        }
    };
    write_frame(&mut client, &CommandResponse::success(Address::default()))?;
    client.set_read_timeout(None)?;
    client.set_write_timeout(None)?;
    upstream.set_read_timeout(None)?;
    upstream.set_write_timeout(None)?;
    Ok(proxy_bidirectional(upstream, client)?)
}

fn connect(
    client: &mut impl io::Read,
    proxy: SocketAddr,
    names: &Mutex<Names>,
) -> socks5_protocol::Result<TcpStream> {
    let request = CommandRequest::read_from(client)?;
    let (SocksCommand::Connect, Address::SocketAddr(target)) = (request.command, request.address)
    else {
        return Err(io::Error::other("expected TCP CONNECT to a TUN destination IP").into());
    };
    let destination = match names
        .lock()
        .map_err(|_| io::Error::other("DNS name table is unavailable"))?
        .lookup(target.ip())?
    {
        Some(host) => Address::Domain(host, target.port()),
        None => Address::SocketAddr(target),
    };

    let mut upstream = TcpStream::connect_timeout(&proxy, TIMEOUT)?;
    upstream.set_read_timeout(Some(TIMEOUT))?;
    upstream.set_write_timeout(Some(TIMEOUT))?;
    Version::V5.write_to(&mut upstream)?;
    AuthRequest::new([AuthMethod::Noauth]).write_to(&mut upstream)?;
    Version::read_from(&mut upstream)?;
    if AuthResponse::read_from(&mut upstream)?.method() != AuthMethod::Noauth {
        return Err(io::Error::other("managed SOCKS5 handshake failed").into());
    }
    write_frame(&mut upstream, &CommandRequest::connect(destination))?;
    // The trusted proxy owns the approval deadline. The relay's handshake
    // timeout applies to framing and negotiation, not the human decision.
    upstream.set_read_timeout(None)?;
    let reply = CommandResponse::read_from(&mut upstream)?;
    if reply.reply != CommandReply::Succeeded
        || matches!(reply.address, Address::Domain(ref name, _) if name.is_empty())
    {
        return Err(io::Error::other("invalid managed SOCKS5 success reply").into());
    }
    Ok(upstream)
}

// Buffer the codec's field-by-field output before calling write_all. A denial
// reply is best effort; the authorization decision does not depend on its delivery.
fn write_frame(stream: &mut TcpStream, message: &impl FromIO) -> socks5_protocol::Result<()> {
    let mut bytes = Vec::new();
    message.write_to(&mut bytes)?;
    Ok(stream.write_all(&bytes)?)
}

#[cfg(test)]
#[path = "managed_tcp_relay_tests.rs"]
mod tests;
