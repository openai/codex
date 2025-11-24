use std::fmt;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Pack {
    mask: usize,
    shift: u32,
}

impl Pack {
    /// Value is packed in the `width` least-significant bits.
    pub(crate) const fn least_significant(width: u32) -> Pack {
        let mask = mask_for(width);

        Pack { mask, shift: 0 }
    }

    /// Value is packed in the `width` more-significant bits.
    pub(crate) const fn then(&self, width: u32) -> Pack {
        let shift = usize::BITS - self.mask.leading_zeros();
        let mask = mask_for(width) << shift;

        Pack { mask, shift }
    }

    /// Width, in bits, dedicated to storing the value.
    pub(crate) const fn width(&self) -> u32 {
        usize::BITS - (self.mask >> self.shift).leading_zeros()
    }

    /// Max representable value.
    pub(crate) const fn max_value(&self) -> usize {
        (1 << self.width()) - 1
    }

    pub(crate) fn pack(&self, value: usize, base: usize) -> usize {
        assert!(value <= self.max_value());
        (base & !self.mask) | (value << self.shift)
    }

    pub(crate) fn unpack(&self, src: usize) -> usize {
        unpack(src, self.mask, self.shift)
    }
}

impl fmt::Debug for Pack {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            "Pack {{ mask: {:b}, shift: {} }}",
            self.mask, self.shift
        )
    }
}

/// Returns a `usize` with the right-most `n` bits set.
pub(crate) const fn mask_for(n: u32) -> usize {
    let shift = 1usize.wrapping_shl(n - 1);
    shift | (shift - 1)
}

/// Unpacks a value using a mask & shift.
pub(crate) const fn unpack(src: usize, mask: usize, shift: u32) -> usize {
    (src & mask) >> shift
}
