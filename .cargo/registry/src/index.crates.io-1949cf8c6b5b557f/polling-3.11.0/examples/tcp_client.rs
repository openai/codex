use std::{io, net};

use polling::Event;
use socket2::Type;

fn main() -> io::Result<()> {
    let socket = socket2::Socket::new(socket2::Domain::IPV4, Type::STREAM, None)?;
    let poller = polling::Poller::new()?;
    unsafe {
        poller.add(&socket, Event::new(0, true, true))?;
    }
    let addr = net::SocketAddr::new(net::Ipv4Addr::LOCALHOST.into(), 8080);
    socket.set_nonblocking(true)?;
    let _ = socket.connect(&addr.into());

    let mut events = polling::Events::new();

    events.clear();
    poller.wait(&mut events, None)?;

    let event = events.iter().next();
    let event = match event {
        Some(event) => event,
        None => {
            println!("no event");
            return Ok(());
        }
    };

    println!("event: {event:?}");
    if event.is_err().unwrap_or(false) {
        println!("connect failed");
    }

    Ok(())
}
