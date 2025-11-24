use crate::{Error, MaxDepthExceeded, Result};

// We take the limits from the D-Bus specification for gvariant as well.
//
// The GVariant specification removed all the limits, from the D-Bus specification but that turned
// out to be a [mistake]. Although glib went for a higher limit (128) but we'll stick to the D-Bus
// limits and expand if/when needed.
//
// [mistake]: https://gitlab.gnome.org/GNOME/glib/-/commit/7c4e6e9fbe473de0401c778c6b0c4aad27d5145a
const MAX_STRUCT_DEPTH: u8 = 32;
const MAX_ARRAY_DEPTH: u8 = 32;
const MAX_TOTAL_DEPTH: u8 = 64;

// Represents the current depth of all container being (de)serialized.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ContainerDepths {
    structure: u8,
    array: u8,
    variant: u8,
    #[cfg(feature = "gvariant")]
    maybe: u8,
}

impl ContainerDepths {
    pub fn inc_structure(mut self) -> Result<Self> {
        self.structure += 1;
        self.check()
    }

    pub fn dec_structure(mut self) -> Self {
        self.structure -= 1;
        self
    }

    pub fn inc_array(mut self) -> Result<Self> {
        self.array += 1;
        self.check()
    }

    pub fn dec_array(mut self) -> Self {
        self.array -= 1;
        self
    }

    pub fn inc_variant(mut self) -> Result<Self> {
        self.variant += 1;
        self.check()
    }

    #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
    pub fn inc_maybe(mut self) -> Result<Self> {
        self.maybe += 1;
        self.check()
    }

    #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
    pub fn dec_maybe(mut self) -> Self {
        self.maybe -= 1;
        self
    }

    fn check(self) -> Result<Self> {
        if self.structure > MAX_STRUCT_DEPTH {
            return Err(Error::MaxDepthExceeded(MaxDepthExceeded::Structure));
        }

        if self.array > MAX_ARRAY_DEPTH {
            return Err(Error::MaxDepthExceeded(MaxDepthExceeded::Array));
        }

        #[cfg(not(feature = "gvariant"))]
        let total = self.structure + self.array + self.variant;
        #[cfg(feature = "gvariant")]
        let total = self.structure + self.array + self.variant + self.maybe;

        if total > MAX_TOTAL_DEPTH {
            return Err(Error::MaxDepthExceeded(MaxDepthExceeded::Container));
        }

        Ok(self)
    }
}
