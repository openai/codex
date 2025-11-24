use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use easy_parallel::Parallel;
use polling::{Event, Events, Poller};

#[test]
fn concurrent_add() -> io::Result<()> {
    let (reader, mut writer) = tcp_pair()?;
    let poller = Poller::new()?;

    let mut events = Events::new();

    let result = Parallel::new()
        .add(|| {
            poller.wait(&mut events, None)?;
            Ok(())
        })
        .add(|| {
            thread::sleep(Duration::from_millis(100));
            unsafe {
                poller.add(&reader, Event::readable(0))?;
            }
            writer.write_all(&[1])?;
            Ok(())
        })
        .run()
        .into_iter()
        .collect::<io::Result<()>>();

    poller.delete(&reader)?;
    result?;

    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(0)
    );

    Ok(())
}

#[test]
fn concurrent_modify() -> io::Result<()> {
    let (reader, mut writer) = tcp_pair()?;
    let poller = Poller::new()?;
    unsafe {
        poller.add(&reader, Event::none(0))?;
    }

    let mut events = Events::new();

    Parallel::new()
        .add(|| {
            poller.wait(&mut events, Some(Duration::from_secs(10)))?;
            Ok(())
        })
        .add(|| {
            thread::sleep(Duration::from_millis(100));
            poller.modify(&reader, Event::readable(0))?;
            writer.write_all(&[1])?;
            Ok(())
        })
        .run()
        .into_iter()
        .collect::<io::Result<()>>()?;

    assert_eq!(events.len(), 1);
    assert_eq!(
        events.iter().next().unwrap().with_no_extra(),
        Event::readable(0)
    );

    Ok(())
}

#[cfg(all(unix, not(target_os = "vita")))]
#[test]
fn concurrent_interruption() -> io::Result<()> {
    struct MakeItSend<T>(T);
    unsafe impl<T> Send for MakeItSend<T> {}

    let (reader, _writer) = tcp_pair()?;
    let poller = Poller::new()?;
    unsafe {
        poller.add(&reader, Event::none(0))?;
    }

    let mut events = Events::new();
    let events_borrow = &mut events;
    let (sender, receiver) = std::sync::mpsc::channel();

    Parallel::new()
        .add(move || {
            // Register a signal handler so that the syscall is actually interrupted. A signal that
            // is ignored by default does not cause an interrupted syscall.
            signal_hook::flag::register(signal_hook::consts::signal::SIGURG, Default::default())?;

            // Signal to the other thread how to send a signal to us
            sender
                .send(MakeItSend(unsafe { libc::pthread_self() }))
                .unwrap();

            poller.wait(events_borrow, Some(Duration::from_secs(1)))?;
            Ok(())
        })
        .add(move || {
            let MakeItSend(target_thread) = receiver.recv().unwrap();
            thread::sleep(Duration::from_millis(100));
            assert_eq!(0, unsafe {
                libc::pthread_kill(target_thread, libc::SIGURG)
            });
            Ok(())
        })
        .run()
        .into_iter()
        .collect::<io::Result<()>>()?;

    assert_eq!(events.len(), 0);

    Ok(())
}

fn tcp_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let a = TcpStream::connect(listener.local_addr()?)?;
    let (b, _) = listener.accept()?;
    Ok((a, b))
}
