use log::trace;

use crate::{Bitness, Info, Type};

pub fn current_platform() -> Info {
    trace!("android::current_platform is called");

    let info = Info::with_type(Type::Android);
    trace!("Returning {:?}", info);
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn os_type() {
        let version = current_platform();
        assert_eq!(Type::Android, version.os_type());
    }
}
