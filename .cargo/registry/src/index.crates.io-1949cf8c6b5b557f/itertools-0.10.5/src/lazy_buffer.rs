use std::ops::Index;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct LazyBuffer<I: Iterator> {
    pub it: I,
    done: bool,
    buffer: Vec<I::Item>,
}

impl<I> LazyBuffer<I>
where
    I: Iterator,
{
    pub fn new(it: I) -> LazyBuffer<I> {
        LazyBuffer {
            it,
            done: false,
            buffer: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn get_next(&mut self) -> bool {
        if self.done {
            return false;
        }
        if let Some(x) = self.it.next() {
            self.buffer.push(x);
            true
        } else {
            self.done = true;
            false
        }
    }

    pub fn prefill(&mut self, len: usize) {
        let buffer_len = self.buffer.len();

        if !self.done && len > buffer_len {
            let delta = len - buffer_len;

            self.buffer.extend(self.it.by_ref().take(delta));
            self.done = self.buffer.len() < len;
        }
    }
}

impl<I, J> Index<J> for LazyBuffer<I>
where
    I: Iterator,
    I::Item: Sized,
    Vec<I::Item>: Index<J>
{
    type Output = <Vec<I::Item> as Index<J>>::Output;

    fn index(&self, index: J) -> &Self::Output {
        self.buffer.index(index)
    }
}
