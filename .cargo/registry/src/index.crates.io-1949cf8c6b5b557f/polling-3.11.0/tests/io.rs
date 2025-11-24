use polling::{Event, Events, Poller};
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn basic_io() {
    let poller = Poller::new().unwrap();
    let (read, mut write) = tcp_pair().unwrap();
    unsafe {
        poller.add(&read, Event::readable(1)).unwrap();
    }

    // Nothing should be available at first.
    let mut events = Events::new();
    assert_eq!(
        poller
            .wait(&mut events, Some(Duration::from_secs(0)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // After a write, the event should be available now.
    write.write_all(&[1]).unwrap();
    assert_eq!(
        poller
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );

    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(1)
    );
    poller.delete(&read).unwrap();
}

#[test]
fn insert_twice() {
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;
    #[cfg(windows)]
    use std::os::windows::io::AsRawSocket;

    let (read, mut write) = tcp_pair().unwrap();
    let read = Arc::new(read);

    let poller = Poller::new().unwrap();
    unsafe {
        #[cfg(unix)]
        let read = read.as_raw_fd();
        #[cfg(windows)]
        let read = read.as_raw_socket();

        poller.add(read, Event::readable(1)).unwrap();
        assert_eq!(
            poller.add(read, Event::readable(1)).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    write.write_all(&[1]).unwrap();
    let mut events = Events::new();
    assert_eq!(
        poller
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );

    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(1)
    );

    poller.delete(&read).unwrap();
}

/// Test that calling `wait` appends events, as [documented], rather than
/// overwriting them.
///
/// [documented]: https://docs.rs/polling/latest/polling/struct.Poller.html#method.wait
#[test]
fn append_events() {
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;
    #[cfg(windows)]
    use std::os::windows::io::AsRawSocket;

    // Create a few sockets.
    let mut pairs = Vec::new();
    for _ in 0..4 {
        let (read, write) = tcp_pair().unwrap();
        pairs.push((read, write));
    }

    // Add the sockets to the poller.
    let poller = Poller::new().unwrap();
    unsafe {
        for (read, _write) in &pairs {
            #[cfg(unix)]
            let read = read.as_raw_fd();
            #[cfg(windows)]
            let read = read.as_raw_socket();

            poller.add(read, Event::readable(1)).unwrap();
        }
    }

    // Trigger read events on the sockets and reuse the event list to test
    // that events are appended.
    let mut events = Events::new();

    for (index, (_read, ref mut write)) in pairs.iter_mut().enumerate() {
        // Write to the socket prompting a reader readiness event.
        write.write_all(&[1]).unwrap();
        assert_eq!(
            poller
                .wait(&mut events, Some(Duration::from_secs(1)))
                .unwrap(),
            index + 1
        );
        assert_eq!(events.len(), index + 1);
        for event in events.iter() {
            assert_eq!(event.with_no_extra(), Event::readable(1));
        }
    }

    for (read, _write) in &pairs {
        poller.delete(read).unwrap();
    }
}

fn tcp_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let a = TcpStream::connect(listener.local_addr()?)?;
    let (b, _) = listener.accept()?;
    Ok((a, b))
}
