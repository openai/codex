use std::process::Command;

use log::trace;

use crate::{bitness, uname::uname, Bitness, Info, Type, Version};

pub fn current_platform() -> Info {
    trace!("dragonfly::current_platform is called");

    let version = uname("-r")
        .map(Version::from_string)
        .unwrap_or_else(|| Version::Unknown);

    let info = Info {
        os_type: Type::DragonFly,
        version,
        bitness: bitness::get(),
        ..Default::default()
    };

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
        assert_eq!(Type::DragonFly, version.os_type());
    }
}
