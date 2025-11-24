use std::borrow::Cow;

use crate::protocol::{get_request_name, request_name};
use crate::x11_utils::{ExtInfoProvider, ExtensionInformation};

struct ExtInfo<'a> {
    extension: Option<&'a str>,
    major_opcode: u8,
}

impl ExtInfoProvider for ExtInfo<'_> {
    fn get_from_major_opcode(&self, major_opcode: u8) -> Option<(&str, ExtensionInformation)> {
        assert_eq!(self.major_opcode, major_opcode);
        self.extension.map(|ext_name| {
            (
                ext_name,
                ExtensionInformation {
                    major_opcode,
                    first_event: 0,
                    first_error: 0,
                },
            )
        })
    }

    fn get_from_event_code(&self, _event_code: u8) -> Option<(&str, ExtensionInformation)> {
        unimplemented!()
    }

    fn get_from_error_code(&self, _error_code: u8) -> Option<(&str, ExtensionInformation)> {
        unimplemented!()
    }
}

/// Test whether request_name() and get_request_name() provide the correct information for the
/// given request.
fn get_request_names(
    extension: Option<&str>,
    major_opcode: u8,
    minor_opcode: u8,
) -> (Option<&'static str>, Cow<'static, str>) {
    let ext_info = ExtInfo {
        extension,
        major_opcode,
    };

    let (ext_name, name) = request_name(&ext_info, major_opcode, minor_opcode.into());
    let get_name = get_request_name(&ext_info, major_opcode, minor_opcode);
    assert_eq!(ext_name.as_deref(), extension);
    (name, get_name)
}

#[test]
fn test_core_request_names() {
    assert_eq!(
        get_request_names(None, 1, 0),
        (Some("CreateWindow"), Cow::from("CreateWindow"))
    );
    // Test that the minor opcode (which is actually the depth in CreateWindow) does not influence
    // the result
    assert_eq!(
        get_request_names(None, 1, 1),
        (Some("CreateWindow"), Cow::from("CreateWindow"))
    );
    assert_eq!(
        get_request_names(None, 1, 255),
        (Some("CreateWindow"), Cow::from("CreateWindow"))
    );
    // Special case: minor_opcode does not fit into u8.
    assert_eq!(
        request_name(
            &ExtInfo {
                extension: None,
                major_opcode: 42
            },
            1,
            54321
        ),
        (None, Some("CreateWindow"))
    );

    assert_eq!(
        get_request_names(None, 100, 0),
        (
            Some("ChangeKeyboardMapping"),
            Cow::from("ChangeKeyboardMapping")
        )
    );
    assert_eq!(
        get_request_names(None, 120, 0),
        (None, Cow::from("xproto::opcode 120"))
    );
    assert_eq!(
        get_request_names(None, 127, 0),
        (Some("NoOperation"), Cow::from("NoOperation"))
    );
}

#[test]
fn test_bigreq_request_names() {
    assert_eq!(
        get_request_names(Some("BIG-REQUESTS"), 130, 0),
        (Some("Enable"), Cow::from("BigRequests::Enable"))
    );
    assert_eq!(
        get_request_names(Some("BIG-REQUESTS"), 200, 0),
        (Some("Enable"), Cow::from("BigRequests::Enable"))
    );

    assert_eq!(
        get_request_names(Some("BIG-REQUESTS"), 130, 1),
        (None, Cow::from("BigRequests::opcode 1"))
    );
    assert_eq!(
        get_request_names(Some("BIG-REQUESTS"), 130, 255),
        (None, Cow::from("BigRequests::opcode 255"))
    );
}

#[test]
fn test_xc_misc_request_names() {
    assert_eq!(
        get_request_names(Some("XC-MISC"), 130, 0),
        (Some("GetVersion"), Cow::from("XCMisc::GetVersion"))
    );

    assert_eq!(
        get_request_names(Some("XC-MISC"), 200, 0),
        (Some("GetVersion"), Cow::from("XCMisc::GetVersion"))
    );
    assert_eq!(
        get_request_names(Some("XC-MISC"), 200, 1),
        (Some("GetXIDRange"), Cow::from("XCMisc::GetXIDRange"))
    );
    assert_eq!(
        get_request_names(Some("XC-MISC"), 200, 2),
        (Some("GetXIDList"), Cow::from("XCMisc::GetXIDList"))
    );
    assert_eq!(
        get_request_names(Some("XC-MISC"), 200, 3),
        (None, Cow::from("XCMisc::opcode 3"))
    );
    assert_eq!(
        get_request_names(Some("XC-MISC"), 200, 100),
        (None, Cow::from("XCMisc::opcode 100"))
    );
}

#[test]
fn test_unknown_extension_request_names() {
    assert_eq!(
        get_request_names(None, 210, 123),
        (None, Cow::from("ext 210::opcode 123"))
    );
    assert_eq!(
        get_request_names(Some("FOO-BAR"), 210, 123),
        (None, Cow::from("ext FOO-BAR::opcode 123"))
    );
}
