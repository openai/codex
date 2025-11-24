use super::{ReadHalf, Socket, WriteHalf};

/// A pair of socket read and write halves.
#[derive(Debug)]
pub struct Split<R: ReadHalf, W: WriteHalf> {
    pub(super) read: R,
    pub(super) write: W,
}

impl<R: ReadHalf, W: WriteHalf> Split<R, W> {
    /// Reference to the read half.
    pub fn read(&self) -> &R {
        &self.read
    }

    /// Mutable reference to the read half.
    pub fn read_mut(&mut self) -> &mut R {
        &mut self.read
    }

    /// Reference to the write half.
    pub fn write(&self) -> &W {
        &self.write
    }

    /// Mutable reference to the write half.
    pub fn write_mut(&mut self) -> &mut W {
        &mut self.write
    }

    /// Take the read and write halves.
    pub fn take(self) -> (R, W) {
        (self.read, self.write)
    }
}

/// A boxed `Split`.
pub type BoxedSplit = Split<Box<dyn ReadHalf>, Box<dyn WriteHalf>>;

impl<S: Socket> From<S> for BoxedSplit {
    fn from(socket: S) -> Self {
        let split = socket.split();

        Split {
            read: Box::new(split.read),
            write: Box::new(split.write),
        }
    }
}
