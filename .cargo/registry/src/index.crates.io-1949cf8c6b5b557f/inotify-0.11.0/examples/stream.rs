use std::{
    fs::File,
    io,
    thread,
    time::Duration,
};

use futures_util::StreamExt;
use inotify::{
    Inotify,
    WatchMask,
};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let inotify = Inotify::init()
        .expect("Failed to initialize inotify");

    let dir = TempDir::new()?;

    inotify.watches().add(dir.path(), WatchMask::CREATE | WatchMask::MODIFY)?;

    thread::spawn::<_, Result<(), io::Error>>(move || {
        loop {
            File::create(dir.path().join("file"))?;
            thread::sleep(Duration::from_millis(500));
        }
    });

    let mut buffer = [0; 1024];
    let mut stream = inotify.into_event_stream(&mut buffer)?;

    while let Some(event_or_error) = stream.next().await {
        println!("event: {:?}", event_or_error?);
    }

    Ok(())
}
