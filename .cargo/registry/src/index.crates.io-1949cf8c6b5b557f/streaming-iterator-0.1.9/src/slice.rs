use crate::{
    DoubleEndedStreamingIterator, DoubleEndedStreamingIteratorMut, StreamingIterator,
    StreamingIteratorMut,
};

use core::mem;
use core::num::NonZeroUsize;

/// Creates an iterator over all contiguous windows of length `size` in a mutable `slice`.
///
/// The windows overlap and may be mutated via `StreamingIteratorMut`.
/// If the `slice` is shorter than `size`, the iterator returns no values.
///
/// # Panics
///
/// Panics if `size` is 0.
pub fn windows_mut<T>(slice: &mut [T], size: usize) -> WindowsMut<'_, T> {
    WindowsMut {
        slice,
        size: NonZeroUsize::new(size).expect("size is zero"),
        position: Position::Init,
    }
}

/// A streaming iterator which returns overlapping mutable subslices of length `size`.
///
/// This struct is created by the [`windows_mut`] function.
pub struct WindowsMut<'a, T> {
    slice: &'a mut [T],
    size: NonZeroUsize,
    position: Position,
}

enum Position {
    Init,
    Front,
    Back,
}

impl<T> WindowsMut<'_, T> {
    fn consume(&mut self) {
        match self.position {
            Position::Init => {}
            Position::Front => {
                let slice = mem::take(&mut self.slice);
                if let Some((_, tail)) = slice.split_first_mut() {
                    self.slice = tail;
                }
            }
            Position::Back => {
                let slice = mem::take(&mut self.slice);
                if let Some((_, head)) = slice.split_last_mut() {
                    self.slice = head;
                }
            }
        }
    }

    fn get_front(&self) -> Option<&[T]> {
        self.slice.get(..self.size.get())
    }

    fn get_front_mut(&mut self) -> Option<&mut [T]> {
        self.slice.get_mut(..self.size.get())
    }

    fn get_back(&self) -> Option<&[T]> {
        let start = self.slice.len().checked_sub(self.size.get())?;
        self.slice.get(start..)
    }

    fn get_back_mut(&mut self) -> Option<&mut [T]> {
        let start = self.slice.len().checked_sub(self.size.get())?;
        self.slice.get_mut(start..)
    }

    fn len(&self) -> usize {
        let len = match self.position {
            Position::Init => self.slice.len(),
            _ => self.slice.len().saturating_sub(1),
        };
        len.saturating_sub(self.size.get() - 1)
    }
}

impl<T> StreamingIterator for WindowsMut<'_, T> {
    type Item = [T];

    fn advance(&mut self) {
        self.consume();
        self.position = Position::Front;
    }

    fn get(&self) -> Option<&Self::Item> {
        match self.position {
            Position::Init => None,
            Position::Front => self.get_front(),
            Position::Back => self.get_back(),
        }
    }

    fn next(&mut self) -> Option<&Self::Item> {
        self.advance();
        self.get_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    fn is_done(&self) -> bool {
        self.slice.len() < self.size.get()
    }

    fn count(self) -> usize {
        self.len()
    }
}

impl<T> StreamingIteratorMut for WindowsMut<'_, T> {
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        match self.position {
            Position::Init => None,
            Position::Front => self.get_front_mut(),
            Position::Back => self.get_back_mut(),
        }
    }

    fn next_mut(&mut self) -> Option<&mut Self::Item> {
        self.advance();
        self.get_front_mut()
    }
}

impl<T> DoubleEndedStreamingIterator for WindowsMut<'_, T> {
    fn advance_back(&mut self) {
        self.consume();
        self.position = Position::Back;
    }

    fn next_back(&mut self) -> Option<&Self::Item> {
        self.advance_back();
        self.get_back()
    }
}

impl<T> DoubleEndedStreamingIteratorMut for WindowsMut<'_, T> {
    fn next_back_mut(&mut self) -> Option<&mut Self::Item> {
        self.advance_back();
        self.get_back_mut()
    }
}

#[test]
fn test_windows_mut() {
    let slice: &mut [_] = &mut [0; 6];

    windows_mut(slice, 3).fold_mut(0, |i, win| {
        win.copy_from_slice(&[i; 3]);
        i + 1
    });
    assert_eq!(slice, &[0, 1, 2, 3, 3, 3]);

    windows_mut(slice, 2).rfold_mut(0, |i, win| {
        win.copy_from_slice(&[i; 2]);
        i + 1
    });
    assert_eq!(slice, &[4, 4, 3, 2, 1, 0]);

    let mut i = 0;
    let mut iter = windows_mut(slice, 1);
    while let Some(win) = iter.next_mut() {
        win.copy_from_slice(&[i]);
        i += 1;

        if let Some(win) = iter.next_back_mut() {
            win.copy_from_slice(&[i]);
            i += 1;
        }
    }
    assert_eq!(slice, &[0, 2, 4, 5, 3, 1]);
}

#[test]
fn test_windows_mut_count() {
    let slice: &mut [_] = &mut [0; 6];

    assert_eq!(windows_mut(slice, 3).count(), 4);
    assert_eq!(windows_mut(slice, 6).count(), 1);
    assert_eq!(windows_mut(slice, 9).count(), 0);

    let mut iter = windows_mut(slice, 3);
    assert_eq!(iter.size_hint(), (4, Some(4)));
    iter.advance();
    assert_eq!(iter.count(), 3);
}

#[test]
#[should_panic]
fn test_windows_mut_0() {
    let _: WindowsMut<'_, i32> = windows_mut(&mut [], 0);
}
