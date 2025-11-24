// Common routines for all fdo_magic parsers

pub mod builtin;

#[derive(Debug, Clone)]
pub struct MagicRule<'a> {
    indent_level: u32,
    start_off: u32,
    val: &'a [u8],
    mask: Option<&'a [u8]>,
    region_len: u32,
}

impl MagicRule<'_> {
    fn scan_len(&self) -> usize {
        self.start_off as usize + self.val.len() + self.region_len as usize
    }
}

mod check;
mod ruleset;
