//! A mechanism for allocating XIDs.

use crate::errors::ConnectError;
use crate::protocol::xc_misc::GetXIDRangeReply;

#[cfg(feature = "std")]
use std::error::Error;

use core::fmt;

/// An allocator for X11 IDs.
///
/// This struct handles the client-side generation of X11 IDs. The ID allocation is based on a
/// range of IDs that the server assigned us. This range is described by a base and a mask. From
/// the X11 protocol reference manual:
///
/// > The resource-id-mask contains a single contiguous set of bits (at least 18). The client
/// > allocates resource IDs [..] by choosing a value with only some subset of these bits set and
/// > ORing it with resource-id-base.
#[derive(Debug, Clone, Copy)]
pub struct IdAllocator {
    next_id: u32,
    max_id: u32,
    increment: u32,
}

impl IdAllocator {
    /// Create a new instance of an ID allocator.
    ///
    /// The arguments should be the `resource_id_base` and `resource_id_mask` values that the X11
    /// server sent in a `Setup` response.
    pub fn new(id_base: u32, id_mask: u32) -> Result<Self, ConnectError> {
        if id_mask == 0 {
            return Err(ConnectError::ZeroIdMask);
        }
        // Find the right-most set bit in id_mask, e.g. for 0b110, this results in 0b010.
        let increment = id_mask & (1 + !id_mask);
        Ok(Self {
            next_id: id_base,
            max_id: id_base | id_mask,
            increment,
        })
    }

    /// Update the available range of IDs based on a GetXIDRangeReply
    pub fn update_xid_range(&mut self, xidrange: &GetXIDRangeReply) -> Result<(), IdsExhausted> {
        let (start, count) = (xidrange.start_id, xidrange.count);
        // Apparently (0, 1) is how the server signals "I am out of IDs".
        // The second case avoids an underflow below and should never happen.
        if (start, count) == (0, 1) || count == 0 {
            return Err(IdsExhausted);
        }
        self.next_id = start;
        self.max_id = start + (count - 1) * self.increment;
        Ok(())
    }

    /// Generate the next ID.
    pub fn generate_id(&mut self) -> Option<u32> {
        if self.next_id > self.max_id {
            None
        } else {
            let id = self.next_id;
            self.next_id += self.increment;
            Some(id)
        }
    }
}

/// The XID range has been exhausted.
#[derive(Debug, Copy, Clone)]
pub struct IdsExhausted;

impl fmt::Display for IdsExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "XID range has been exhausted")
    }
}

#[cfg(feature = "std")]
impl Error for IdsExhausted {}

#[cfg(test)]
mod test {
    use super::{GetXIDRangeReply, IdAllocator, IdsExhausted};

    #[test]
    fn exhaustive() {
        let mut allocator = IdAllocator::new(0x2800, 0x1ff).unwrap();
        for expected in 0x2800..=0x29ff {
            assert_eq!(Some(expected), allocator.generate_id());
        }
        assert_eq!(None, allocator.generate_id());
    }

    #[test]
    fn increment() {
        let mut allocator = IdAllocator::new(0, 0b1100).unwrap();
        assert_eq!(Some(0b0000), allocator.generate_id());
        assert_eq!(Some(0b0100), allocator.generate_id());
        assert_eq!(Some(0b1000), allocator.generate_id());
        assert_eq!(Some(0b1100), allocator.generate_id());
        assert_eq!(None, allocator.generate_id());
    }

    #[test]
    fn new_range() {
        let mut allocator = IdAllocator::new(0x420, 2).unwrap();
        assert_eq!(Some(0x420), allocator.generate_id());
        assert_eq!(Some(0x422), allocator.generate_id());
        // At this point the range is exhausted and a GetXIDRange request needs to be sent
        assert_eq!(None, allocator.generate_id());
        allocator
            .update_xid_range(&generate_get_xid_range_reply(0x13370, 3))
            .unwrap();
        assert_eq!(Some(0x13370), allocator.generate_id());
        assert_eq!(Some(0x13372), allocator.generate_id());
        assert_eq!(Some(0x13374), allocator.generate_id());
        // At this point the range is exhausted and a GetXIDRange request needs to be sent
        assert_eq!(None, allocator.generate_id());
        allocator
            .update_xid_range(&generate_get_xid_range_reply(0x13370, 3))
            .unwrap();
        assert_eq!(Some(0x13370), allocator.generate_id());
    }

    #[test]
    fn invalid_new_arg() {
        let err = IdAllocator::new(1234, 0).unwrap_err();
        if let super::ConnectError::ZeroIdMask = err {
        } else {
            panic!("Wrong error: {:?}", err);
        }
    }

    #[test]
    fn invalid_update_arg() {
        fn check_ids_exhausted(arg: &Result<(), IdsExhausted>) {
            if let Err(IdsExhausted) = arg {
            } else {
                panic!("Expected IdsExhausted, got {:?}", arg);
            }
        }

        let mut allocator = IdAllocator::new(0x420, 2).unwrap();
        check_ids_exhausted(&allocator.update_xid_range(&generate_get_xid_range_reply(0, 1)));
        check_ids_exhausted(&allocator.update_xid_range(&generate_get_xid_range_reply(1, 0)));
    }

    fn generate_get_xid_range_reply(start_id: u32, count: u32) -> GetXIDRangeReply {
        GetXIDRangeReply {
            sequence: 0,
            length: 0,
            start_id,
            count,
        }
    }
}
