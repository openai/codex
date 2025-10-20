use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

pub fn to_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
    let mut v: Vec<u16> = s.as_ref().encode_wide().collect();
    v.push(0);
    v
}
