/*
use std::error::Error;

pub(crate) fn find<'a, E: Error + 'static>(top: &'a (dyn Error + 'static)) -> Option<&'a E> {
    let mut err = Some(top);
    while let Some(src) = err {
        if src.is::<E>() {
            return src.downcast_ref();
        }
        err = src.source();
    }
    None
}
*/
