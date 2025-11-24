mod winapi;

use log::trace;

use crate::Info;

pub fn current_platform() -> Info {
    trace!("windows::current_platform is called");
    let info = winapi::get();
    trace!("Returning {:?}", info);
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Type;
    use pretty_assertions::assert_eq;

    #[test]
    fn os_type() {
        let version = current_platform();
        assert_eq!(Type::Windows, version.os_type());
        assert!(version.edition().is_some());
    }
}
