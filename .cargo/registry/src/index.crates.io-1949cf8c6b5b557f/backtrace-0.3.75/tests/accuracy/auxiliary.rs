#[inline(never)]
pub fn callback<F>(f: F)
where
    F: FnOnce((&'static str, u32)),
{
    f((file!(), line!()))
}

#[inline(always)]
pub fn callback_inlined<F>(f: F)
where
    F: FnOnce((&'static str, u32)),
{
    f((file!(), line!()))
}
