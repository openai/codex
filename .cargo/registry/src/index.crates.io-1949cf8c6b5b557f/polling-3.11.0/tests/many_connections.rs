//! Tests to ensure more than 32 connections can be polled at once.

// Doesn't work on OpenBSD.
#![cfg(not(target_os = "openbsd"))]

use std::io::{self, prelude::*};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use polling::Events;

#[test]
fn many_connections() {
    // Create 100 connections.
    let mut connections = Vec::new();
    for i in 0..100 {
        let (reader, writer) = tcp_pair().unwrap();
        connections.push((i, reader, writer));
    }

    // Create a poller and add all the connections.
    let poller = polling::Poller::new().unwrap();

    for (i, reader, _) in connections.iter() {
        unsafe {
            poller.add(reader, polling::Event::readable(*i)).unwrap();
        }
    }

    let mut events = Events::new();
    while !connections.is_empty() {
        // Choose a random connection to write to.
        let i = fastrand::usize(..connections.len());
        let (id, mut reader, mut writer) = connections.remove(i);

        // Write a byte to the connection.
        writer.write_all(&[1]).unwrap();

        // Wait for the connection to become readable.
        poller
            .wait(&mut events, Some(Duration::from_secs(10)))
            .unwrap();

        // Check that the connection is readable.
        let current_events = events.iter().collect::<Vec<_>>();
        assert_eq!(current_events.len(), 1, "events: {current_events:?}");
        assert_eq!(
            current_events[0].with_no_extra(),
            polling::Event::readable(id)
        );

        // Read the byte from the connection.
        let mut buf = [0];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [1]);
        poller.delete(&reader).unwrap();
        events.clear();
    }
}

fn tcp_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let a = TcpStream::connect(listener.local_addr()?)?;
    let (b, _) = listener.accept()?;
    Ok((a, b))
}
