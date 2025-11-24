use std::io::IoSlice;
use std::ops::Deref;
use std::slice;

pub struct IoBufs<'a, 'b>(&'b mut [IoSlice<'a>]);

impl<'a, 'b> IoBufs<'a, 'b> {
    pub fn new(slices: &'b mut [IoSlice<'a>]) -> Self {
        IoBufs(slices)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn advance(mut self, n: usize) -> IoBufs<'a, 'b> {
        let mut to_remove = 0;
        let mut remaining_len = n;
        for slice in self.0.iter() {
            if remaining_len < slice.len() {
                break;
            } else {
                remaining_len -= slice.len();
                to_remove += 1;
            }
        }
        self.0 = self.0.split_at_mut(to_remove).1;
        if let Some(slice) = self.0.first_mut() {
            let tail = &slice[remaining_len..];
            // Safety: recasts slice to the original lifetime
            let tail = unsafe { slice::from_raw_parts(tail.as_ptr(), tail.len()) };
            *slice = IoSlice::new(tail);
        } else if remaining_len != 0 {
            panic!("advance past the end of the slice vector");
        }
        self
    }
}

impl<'a, 'b> Deref for IoBufs<'a, 'b> {
    type Target = [IoSlice<'a>];
    fn deref(&self) -> &[IoSlice<'a>] {
        self.0
    }
}
