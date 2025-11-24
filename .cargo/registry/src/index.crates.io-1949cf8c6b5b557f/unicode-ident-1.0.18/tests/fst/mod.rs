#![allow(clippy::module_name_repetitions)]

pub fn xid_start_fst() -> fst::Set<&'static [u8]> {
    let data = include_bytes!("xid_start.fst");
    fst::Set::from(fst::raw::Fst::new(data.as_slice()).unwrap())
}

pub fn xid_continue_fst() -> fst::Set<&'static [u8]> {
    let data = include_bytes!("xid_continue.fst");
    fst::Set::from(fst::raw::Fst::new(data.as_slice()).unwrap())
}
