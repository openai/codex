use std::{process::Command, str};

use log::{error, trace};

use crate::{bitness, uname::uname, Info, Type, Version};

pub fn current_platform() -> Info {
    trace!("aix::current_platform is called");

    let version = get_version()
        .map(Version::from_string)
        .unwrap_or_else(|| Version::Unknown);

    let info = Info {
        os_type: get_os(),
        version,
        bitness: bitness::get(),
        ..Default::default()
    };

    trace!("Returning {:?}", info);
    info
}

fn get_version() -> Option<String> {
    let major = uname("-v")?;
    let minor = uname("-r").unwrap_or(String::from("0"));
    Some(format!("{}.{}", major, minor))
}

fn get_os() -> Type {
    match uname("-s").as_deref() {
        Some("AIX") => Type::AIX,
        _ => Type::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_type() {
        let version = current_platform();
        assert_eq!(Type::AIX, version.os_type());
    }
}
