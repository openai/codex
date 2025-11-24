use std::mem::{align_of, size_of};
use std::str::FromStr;

use debugid::DebugId;
use uuid::Uuid;

#[test]
fn test_is_nil() {
    assert!(DebugId::default().is_nil());
}

#[test]
fn test_parse_zero() {
    assert_eq!(
        DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0,
        )
    );
}

#[test]
fn test_parse_short() {
    assert_eq!(
        DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-a").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0xa,
        )
    );
}

#[test]
fn test_parse_long() {
    assert_eq!(
        DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-feedface").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0xfeed_face,
        )
    );
}

#[test]
fn test_parse_compact() {
    assert_eq!(
        DebugId::from_str("dfb8e43af2423d73a453aeb6a777ef75feedface").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0xfeed_face,
        )
    );
}

#[test]
fn test_parse_upper() {
    assert_eq!(
        DebugId::from_str("DFB8E43A-F242-3D73-A453-AEB6A777EF75-FEEDFACE").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0xfeed_face,
        )
    );
}

#[test]
fn test_parse_ignores_tail() {
    assert_eq!(
        DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-feedface-1-2-3").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0xfeed_face,
        )
    );
}

#[test]
fn test_to_string_zero() {
    let id = DebugId::from_parts(
        Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
        0,
    );

    assert_eq!(id.to_string(), "dfb8e43a-f242-3d73-a453-aeb6a777ef75");
}

#[test]
fn test_to_string_short() {
    let id = DebugId::from_parts(
        Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
        10,
    );

    assert_eq!(id.to_string(), "dfb8e43a-f242-3d73-a453-aeb6a777ef75-a");
}

#[test]
fn test_to_string_long() {
    let id = DebugId::from_parts(
        Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
        0xfeed_face,
    );

    assert_eq!(
        id.to_string(),
        "dfb8e43a-f242-3d73-a453-aeb6a777ef75-feedface"
    );
}

#[test]
fn test_parse_error_short() {
    assert!(DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef7").is_err());
}

#[test]
fn test_parse_error_trailing_dash() {
    assert!(DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-").is_err());
}

#[test]
fn test_parse_error_unicode() {
    assert!(DebugId::from_str("아이쿱 조합원 앱카드").is_err());
}

#[test]
fn test_from_guid_age() {
    let guid = [
        0x98, 0xd1, 0xef, 0xe8, 0x6e, 0xf8, 0xfe, 0x45, 0x9d, 0xdb, 0xe1, 0x13, 0x82, 0xb5, 0xd1,
        0xc9,
    ];

    assert_eq!(
        DebugId::from_guid_age(&guid[..], 1).unwrap(),
        DebugId::from_str("e8efd198-f86e-45fe-9ddb-e11382b5d1c9-1").unwrap()
    )
}

#[test]
fn test_parse_breakpad_zero() {
    assert_eq!(
        DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF750").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
            0,
        )
    );
}

#[test]
fn test_parse_breakpad_short() {
    assert_eq!(
        DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF75a").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
            10,
        )
    );
}

#[test]
fn test_parse_breakpad_long() {
    assert_eq!(
        DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF75feedface").unwrap(),
        DebugId::from_parts(
            Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
            0xfeed_face,
        )
    );
}

#[test]
fn test_parse_breakpad_error_tail() {
    assert!(DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF75feedface123").is_err());
}

#[test]
fn test_parse_breakpad_error_missing_age() {
    assert!(DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF75").is_err());
}

#[test]
fn test_parse_breakpad_error_short() {
    assert!(DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF7").is_err());
}

#[test]
fn test_parse_breakpad_error_dashes() {
    assert!(DebugId::from_breakpad("e8efd198-f86e-45fe-9ddb-e11382b5d1c9-1").is_err());
}

#[test]
fn test_to_string_breakpad_zero() {
    let id = DebugId::from_parts(
        Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
        0,
    );

    assert_eq!(
        id.breakpad().to_string(),
        "DFB8E43AF2423D73A453AEB6A777EF750"
    );
}

#[test]
fn test_to_string_breakpad_short() {
    let id = DebugId::from_parts(
        Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
        10,
    );

    assert_eq!(
        id.breakpad().to_string(),
        "DFB8E43AF2423D73A453AEB6A777EF75a"
    );
}

#[test]
fn test_to_string_breakpad_long() {
    let id = DebugId::from_parts(
        Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
        0xfeed_face,
    );

    assert_eq!(
        id.breakpad().to_string(),
        "DFB8E43AF2423D73A453AEB6A777EF75feedface"
    );
}

#[test]
fn test_debug_id_debug() {
    let id = DebugId::from_parts(
        Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
        10,
    );

    assert_eq!(
        format!("{:?}", id),
        "DebugId { uuid: \"dfb8e43a-f242-3d73-a453-aeb6a777ef75\", appendix: 10 }"
    );
}

#[test]
#[cfg(feature = "with_serde")]
fn test_serde_serialize() {
    let id = DebugId::from_parts(
        Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
        10,
    );

    assert_eq!(
        serde_json::to_string(&id).expect("could not serialize"),
        "\"dfb8e43a-f242-3d73-a453-aeb6a777ef75-a\""
    )
}

#[test]
#[cfg(feature = "with_serde")]
fn test_serde_deserialize() {
    let id: DebugId = serde_json::from_str("\"dfb8e43a-f242-3d73-a453-aeb6a777ef75-a\"")
        .expect("could not deserialize");

    assert_eq!(
        id,
        DebugId::from_parts(
            Uuid::parse_str("DFB8E43AF2423D73A453AEB6A777EF75").unwrap(),
            10,
        )
    );
}

#[test]
fn test_pdb20() {
    let timestamp: u32 = 0x418e89c3;
    let age: u32 = 1;
    let debug_id = DebugId::from_pdb20(timestamp, age);

    assert!(debug_id.is_pdb20());
    assert_eq!(
        debug_id.uuid(),
        Uuid::parse_str("418e89c3-0000-0000-0000-000000000000").unwrap()
    );
}

#[test]
fn test_pdb20_format() {
    let timestamp: u32 = 0x418e89c3;
    let age: u32 = 1;
    let debug_id = DebugId::from_pdb20(timestamp, age);

    assert_eq!(debug_id.to_string(), "418E89C3-1".to_string());
    assert_eq!(debug_id.breakpad().to_string(), "418E89C31");
}

#[test]
fn test_pdb20_parse() {
    let timestamp: u32 = 0x418e89c3;
    let age: u32 = 1;
    let debug_id = DebugId::from_pdb20(timestamp, age);

    let s = "418E89C3-1";
    let parsed = DebugId::from_str(s).unwrap();
    assert_eq!(parsed, debug_id);

    let s = "418E89C31";
    let parsed = DebugId::from_str(s).unwrap();
    assert_eq!(parsed, debug_id);

    let s = "418E89C31";
    let parsed = DebugId::from_breakpad(s).unwrap();
    assert_eq!(parsed, debug_id);

    let s = "418E89C3-1";
    assert!(DebugId::from_breakpad(s).is_err());
}

/// The version of `DebugId` up to 0.7.2.
#[repr(C, packed)]
struct OldDebugId {
    uuid: Uuid,
    appendix: u32,
    _padding: [u8; 12],
}

#[test]
fn test_mem() {
    // The size of this struct needs to be exactly aligned and can not change for backwards
    // compatibility.
    assert_eq!(size_of::<OldDebugId>(), 32);
    assert_eq!(size_of::<OldDebugId>(), size_of::<DebugId>());
    assert_eq!(align_of::<DebugId>(), 1);
}

#[test]
fn test_to_from_raw() {
    let debug_id = DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-a").unwrap();

    // Create &[u8] from DebugId.
    let slice = &[debug_id];
    let ptr = slice.as_ptr() as *const u8;
    let len = std::mem::size_of_val(slice);
    let buf: &[u8] = unsafe { std::slice::from_raw_parts(ptr, len) };

    // Copy bytes to new location.
    let mut new_buf: Vec<u8> = Vec::new();
    std::io::copy(&mut std::io::Cursor::new(buf), &mut new_buf).unwrap();

    // Create DebugId from &[u8].
    let ptr = new_buf.as_ptr() as *const DebugId;
    let new_debug_id = unsafe { &*ptr };

    assert_eq!(*new_debug_id, debug_id);
}

#[test]
fn test_from_old_raw() {
    // Ensures we can still read previous in-memory representations into the new format.
    let debug_id = DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-a").unwrap();
    let old_debug_id = OldDebugId {
        uuid: debug_id.uuid(),
        appendix: debug_id.appendix(),
        _padding: [0; 12],
    };

    // Create &[u8] from OldDebugId.
    let slice = &[old_debug_id];
    let ptr = slice.as_ptr() as *const u8;
    let len = std::mem::size_of_val(slice);
    let buf: &[u8] = unsafe { std::slice::from_raw_parts(ptr, len) };

    // Copy bytes to new location.
    let mut new_buf: Vec<u8> = Vec::new();
    std::io::copy(&mut std::io::Cursor::new(buf), &mut new_buf).unwrap();

    // Create DebugId from &[u8].
    let ptr = new_buf.as_ptr() as *const DebugId;
    let new_debug_id = unsafe { &*ptr };

    assert_eq!(*new_debug_id, debug_id);
}

#[test]
fn test_default() {
    let debug_id = DebugId::default();
    assert_eq!(debug_id.uuid(), Uuid::nil());
    assert_eq!(debug_id.appendix(), 0);
}
