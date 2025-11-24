use self::ChainState::*;
use crate::StdError;

#[cfg(any(feature = "std", not(anyhow_no_core_error)))]
use alloc::vec::{self, Vec};

#[cfg(any(feature = "std", not(anyhow_no_core_error)))]
pub(crate) use crate::Chain;

#[cfg(all(not(feature = "std"), anyhow_no_core_error))]
pub(crate) struct Chain<'a> {
    state: ChainState<'a>,
}

#[derive(Clone)]
pub(crate) enum ChainState<'a> {
    Linked {
        next: Option<&'a (dyn StdError + 'static)>,
    },
    #[cfg(any(feature = "std", not(anyhow_no_core_error)))]
    Buffered {
        rest: vec::IntoIter<&'a (dyn StdError + 'static)>,
    },
}

impl<'a> Chain<'a> {
    #[cold]
    pub fn new(head: &'a (dyn StdError + 'static)) -> Self {
        Chain {
            state: ChainState::Linked { next: Some(head) },
        }
    }
}

impl<'a> Iterator for Chain<'a> {
    type Item = &'a (dyn StdError + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            Linked { next } => {
                let error = (*next)?;
                *next = error.source();
                Some(error)
            }
            #[cfg(any(feature = "std", not(anyhow_no_core_error)))]
            Buffered { rest } => rest.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

#[cfg(any(feature = "std", not(anyhow_no_core_error)))]
impl DoubleEndedIterator for Chain<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            Linked { mut next } => {
                let mut rest = Vec::new();
                while let Some(cause) = next {
                    next = cause.source();
                    rest.push(cause);
                }
                let mut rest = rest.into_iter();
                let last = rest.next_back();
                self.state = Buffered { rest };
                last
            }
            Buffered { rest } => rest.next_back(),
        }
    }
}

impl ExactSizeIterator for Chain<'_> {
    fn len(&self) -> usize {
        match &self.state {
            Linked { mut next } => {
                let mut len = 0;
                while let Some(cause) = next {
                    next = cause.source();
                    len += 1;
                }
                len
            }
            #[cfg(any(feature = "std", not(anyhow_no_core_error)))]
            Buffered { rest } => rest.len(),
        }
    }
}

#[cfg(any(feature = "std", not(anyhow_no_core_error)))]
impl Default for Chain<'_> {
    fn default() -> Self {
        Chain {
            state: ChainState::Buffered {
                rest: Vec::new().into_iter(),
            },
        }
    }
}
