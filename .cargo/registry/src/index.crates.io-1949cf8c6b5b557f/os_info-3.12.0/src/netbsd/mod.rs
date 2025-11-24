use std::process::Command;

use log::{error, trace};

use crate::{architecture, bitness, uname::uname, Info, Type, Version};

pub fn current_platform() -> Info {
    trace!("netbsd::current_platform is called");

    let version = uname("-s")
        .map(Version::from_string)
        .unwrap_or_else(|| Version::Unknown);

    let info = Info {
        os_type: Type::NetBSD,
        version,
        bitness: bitness::get(),
        architecture: architecture::get(),
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
        assert_eq!(Type::NetBSD, version.os_type());
    }
}
