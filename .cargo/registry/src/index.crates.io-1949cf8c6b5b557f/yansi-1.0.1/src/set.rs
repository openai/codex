use core::fmt;
use core::marker::PhantomData;

pub struct Set<T>(PhantomData<T>, pub(crate) u16);

pub trait SetMember: Copy + fmt::Debug {
    const MAX_VALUE: u8;

    fn bit_mask(self) -> u16;
    fn from_bit_mask(value: u16) -> Option<Self>;
}

impl<T: SetMember> Set<T> {
    pub const EMPTY: Self = Set(PhantomData, 0);

    pub fn contains(self, value: T) -> bool {
        (value.bit_mask() & self.1) == value.bit_mask()
    }

    pub const fn iter(self) -> Iter<T> {
        Iter { index: 0, set: self }
    }
}

pub struct Iter<T> {
    index: u8,
    set: Set<T>,
}

impl<T: SetMember> Iterator for Iter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index <= T::MAX_VALUE {
            let mask: u16 = 1 << self.index;

            self.index += 1;
            if let Some(v) = T::from_bit_mask(mask) {
                if self.set.contains(v) {
                    return Some(v);
                }
            }
        }

        None
    }
}

impl<T: SetMember> Default for Set<T> {
    fn default() -> Self {
        Set::EMPTY
    }
}

impl<T: SetMember> fmt::Debug for Set<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T> Copy for Set<T> { }

impl<T> Clone for Set<T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<T> PartialEq for Set<T> {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl<T> Eq for Set<T> { }

impl<T> PartialOrd for Set<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.1.partial_cmp(&other.1)
    }
}

impl<T> Ord for Set<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.1.cmp(&other.1)
    }
}

impl<T> core::hash::Hash for Set<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.1.hash(state);
    }
}
