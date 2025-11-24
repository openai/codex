use std::io::{Read, Result};
use std::time::{Duration, Instant};

pub struct Progress<R> {
    bytes: usize,
    tick: Instant,
    stream: R,
}

impl<R> Progress<R> {
    pub fn new(stream: R) -> Self {
        Progress {
            bytes: 0,
            tick: Instant::now() + Duration::from_millis(2000),
            stream,
        }
    }
}

impl<R: Read> Read for Progress<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let num = self.stream.read(buf)?;
        self.bytes += num;
        let now = Instant::now();
        if now > self.tick {
            self.tick = now + Duration::from_millis(500);
            errorf!("downloading... {} bytes\n", self.bytes);
        }
        Ok(num)
    }
}

impl<R> Drop for Progress<R> {
    fn drop(&mut self) {
        errorf!("done ({} bytes)\n", self.bytes);
    }
}
