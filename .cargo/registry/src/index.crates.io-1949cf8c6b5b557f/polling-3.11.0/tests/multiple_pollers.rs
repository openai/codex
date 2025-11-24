//! Test registering one source into multiple pollers.

use polling::{Event, Events, PollMode, Poller};

use std::io::{self, prelude::*};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

#[test]
fn level_triggered() {
    let poller1 = Poller::new().unwrap();
    let poller2 = Poller::new().unwrap();
    let mut events = Events::new();

    if !poller1.supports_level() || !poller2.supports_level() {
        return;
    }

    // Register the source into both pollers.
    let (mut reader, mut writer) = tcp_pair().unwrap();
    unsafe {
        poller1
            .add_with_mode(&reader, Event::readable(1), PollMode::Level)
            .unwrap();
        poller2
            .add_with_mode(&reader, Event::readable(2), PollMode::Level)
            .unwrap();
    }

    // Neither poller should have any events.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // Write to the source.
    writer.write_all(&[1]).unwrap();

    // At least one poller should have an event.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(1)
    );

    events.clear();
    // poller2 should have zero or one events.
    match poller2.wait(&mut events, Some(Duration::from_secs(1))) {
        Ok(1) => {
            assert_eq!(events.len(), 1);
            assert_eq!(
                events.iter().next().unwrap().with_no_extra(),
                Event::readable(2)
            );
        }
        Ok(0) => assert!(events.is_empty()),
        _ => panic!("unexpected error"),
    }

    // Writing more data should cause the same event.
    writer.write_all(&[1]).unwrap();
    events.clear();
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(1)
    );

    // poller2 should have zero or one events.
    events.clear();
    match poller2.wait(&mut events, Some(Duration::from_secs(1))) {
        Ok(1) => {
            assert_eq!(events.len(), 1);
            assert_eq!(
                events.iter().next().unwrap().with_no_extra(),
                Event::readable(2)
            );
        }
        Ok(0) => assert!(events.is_empty()),
        _ => panic!("unexpected error"),
    }

    // Read from the source.
    reader.read_exact(&mut [0; 2]).unwrap();

    // Both pollers should not have any events.
    events.clear();
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // Dereference the pollers.
    poller1.delete(&reader).unwrap();
    poller2.delete(&reader).unwrap();
}

#[test]
fn edge_triggered() {
    let poller1 = Poller::new().unwrap();
    let poller2 = Poller::new().unwrap();
    let mut events = Events::new();

    if !poller1.supports_edge() || !poller2.supports_edge() {
        return;
    }

    // Register the source into both pollers.
    let (mut reader, mut writer) = tcp_pair().unwrap();
    unsafe {
        poller1
            .add_with_mode(&reader, Event::readable(1), PollMode::Edge)
            .unwrap();
        poller2
            .add_with_mode(&reader, Event::readable(2), PollMode::Edge)
            .unwrap();
    }

    // Neither poller should have any events.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // Write to the source.
    writer.write_all(&[1]).unwrap();

    // Both pollers should have an event.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(1)
    );

    events.clear();
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(2)
    );

    // Writing to the poller again should cause an event.
    writer.write_all(&[1]).unwrap();

    // Both pollers should have one event.
    events.clear();
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(1)
    );

    events.clear();
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        1
    );
    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(2)
    );

    // Read from the source.
    reader.read_exact(&mut [0; 2]).unwrap();

    // Both pollers should not have any events.
    events.clear();
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // Dereference the pollers.
    poller1.delete(&reader).unwrap();
    poller2.delete(&reader).unwrap();
}

#[test]
fn oneshot_triggered() {
    let poller1 = Poller::new().unwrap();
    let poller2 = Poller::new().unwrap();
    let mut events = Events::new();

    // Register the source into both pollers.
    let (mut reader, mut writer) = tcp_pair().unwrap();
    unsafe {
        poller1
            .add_with_mode(&reader, Event::readable(1), PollMode::Oneshot)
            .unwrap();
        poller2
            .add_with_mode(&reader, Event::readable(2), PollMode::Oneshot)
            .unwrap();
    }

    // Neither poller should have any events.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // Write to the source.
    writer.write_all(&[1]).unwrap();

    // Sources should have either one or no events.
    match poller1.wait(&mut events, Some(Duration::from_secs(1))) {
        Ok(1) => {
            assert_eq!(events.len(), 1);
            assert_eq!(
                events.iter().next().unwrap().with_no_extra(),
                Event::readable(1)
            );
        }
        Ok(0) => assert!(events.is_empty()),
        _ => panic!("unexpected error"),
    }
    events.clear();

    match poller2.wait(&mut events, Some(Duration::from_secs(1))) {
        Ok(1) => {
            assert_eq!(events.len(), 1);
            assert_eq!(
                events.iter().next().unwrap().with_no_extra(),
                Event::readable(2)
            );
        }
        Ok(0) => assert!(events.is_empty()),
        _ => panic!("unexpected error"),
    }
    events.clear();

    // Writing more data should not cause an event.
    writer.write_all(&[1]).unwrap();

    // Sources should have no events.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());

    // Read from the source.
    reader.read_exact(&mut [0; 2]).unwrap();

    // Sources should have no events.
    assert_eq!(
        poller1
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
    assert_eq!(
        poller2
            .wait(&mut events, Some(Duration::from_secs(1)))
            .unwrap(),
        0
    );
    assert!(events.is_empty());
}

fn tcp_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let a = TcpStream::connect(listener.local_addr()?)?;
    let (b, _) = listener.accept()?;
    Ok((a, b))
}
