// You can run this example from the root of the mio repo:
// cargo run --example udp_server --features="os-poll net"
use log::warn;
use mio::{Events, Interest, Poll, Token};
use std::io;

// A token to allow us to identify which event is for the `UdpSocket`.
const UDP_SOCKET: Token = Token(0);

#[cfg(not(target_os = "wasi"))]
fn main() -> io::Result<()> {
    use mio::net::UdpSocket;

    env_logger::init();

    // Create a poll instance.
    let mut poll = Poll::new()?;
    // Create storage for events. Since we will only register a single socket, a
    // capacity of 1 will do.
    let mut events = Events::with_capacity(1);

    // Setup the UDP socket.
    let addr = "127.0.0.1:9000".parse().unwrap();

    let mut socket = UdpSocket::bind(addr)?;

    // Register our socket with the token defined above and an interest in being
    // `READABLE`.
    poll.registry()
        .register(&mut socket, UDP_SOCKET, Interest::READABLE)?;

    println!("You can connect to the server using `nc`:");
    println!(" $ nc -u 127.0.0.1 9000");
    println!("Anything you type will be echoed back to you.");

    // Initialize a buffer for the UDP packet. We use the maximum size of a UDP
    // packet, which is the maximum value of 16 a bit integer.
    let mut buf = [0; 1 << 16];

    // Our event loop.
    loop {
        // Poll to check if we have events waiting for us.
        if let Err(err) = poll.poll(&mut events, None) {
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }

        // Process each event.
        for event in events.iter() {
            // Validate the token we registered our socket with,
            // in this example it will only ever be one but we
            // make sure it's valid none the less.
            match event.token() {
                UDP_SOCKET => loop {
                    // In this loop we receive all packets queued for the socket.
                    match socket.recv_from(&mut buf) {
                        Ok((packet_size, source_address)) => {
                            // Echo the data.
                            socket.send_to(&buf[..packet_size], source_address)?;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // If we get a `WouldBlock` error we know our socket
                            // has no more packets queued, so we can return to
                            // polling and wait for some more.
                            break;
                        }
                        Err(e) => {
                            // If it was any other kind of error, something went
                            // wrong and we terminate with an error.
                            return Err(e);
                        }
                    }
                },
                _ => {
                    // This should never happen as we only registered our
                    // `UdpSocket` using the `UDP_SOCKET` token, but if it ever
                    // does we'll log it.
                    warn!("Got event for unexpected token: {:?}", event);
                }
            }
        }
    }
}

#[cfg(target_os = "wasi")]
fn main() {
    panic!("can't bind to an address with wasi")
}
