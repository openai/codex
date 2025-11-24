// spell-checker:ignore uname

use std::{fs::File, io::Read};

use log::{error, trace};

use crate::{Bitness, Info, Type, Version};

const UNAME_FILE: &str = "sys:uname";

pub fn current_platform() -> Info {
    trace!("redox::current_platform is called");

    let version = get_version()
        .map(Version::from_string)
        .unwrap_or_else(|| Version::Unknown);
    let info = Info {
        os_type: Type::Redox,
        version,
        bitness: Bitness::Unknown,
        ..Default::default()
    };
    trace!("Returning {:?}", info);
    info
}

fn get_version() -> Option<String> {
    let mut file = match File::open(UNAME_FILE) {
        Ok(file) => file,
        Err(e) => {
            error!("Unable to open {} file: {:?}", UNAME_FILE, e);
            return None;
        }
    };

    let mut version = String::new();
    if let Err(e) = file.read_to_string(&mut version) {
        error!("Unable to read {} file: {:?}", UNAME_FILE, e);
        return None;
    }
    Some(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn os_type() {
        let version = current_platform();
        assert_eq!(Type::Redox, version.os_type());
    }
}
