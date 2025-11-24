//! Helpers for working with `~/.Xauthority`.

#![cfg(feature = "std")]

use alloc::string::ToString;
use alloc::vec::Vec;
use std::io::Error;

use crate::protocol::xproto::Family as X11Family;

const MIT_MAGIC_COOKIE_1: &[u8] = b"MIT-MAGIC-COOKIE-1";

/// A family describes how to interpret some bytes as an address in an `AuthEntry`.
///
/// Compared to [`super::protocol::xproto::Family`], this is a `u16` and not an `u8` since
/// that's what is used in `~/.Xauthority` files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Family(u16);

impl Family {
    /// IPv4 connection to the server
    pub const INTERNET: Self = Self(0);
    /// DECnet
    pub const DEC_NET: Self = Self(1);
    /// Chaosnet connection
    pub const CHAOS: Self = Self(2);
    /// Family without predefined meaning, but interpreted by the server, for example a user name
    pub const SERVER_INTERPRETED: Self = Self(5);
    /// IPv6 connection to the server
    pub const INTERNET6: Self = Self(6);
    /// Wildcard matching any protocol family
    pub const WILD: Self = Self(65535);
    /// For local non-net authentication
    pub const LOCAL: Self = Self(256);
    /// TODO: No idea what this means exactly
    pub const NETNAME: Self = Self(254);
    /// Kerberos 5 principal name
    pub const KRB5_PRINCIPAL: Self = Self(253);
    /// For local non-net authentication
    pub const LOCAL_HOST: Self = Self(252);
}

impl From<X11Family> for Family {
    fn from(value: X11Family) -> Self {
        Self(value.into())
    }
}

impl From<u16> for Family {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

/// A single entry of an `.Xauthority` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthEntry {
    /// The protocol family to which the entry applies
    family: Family,
    /// The address of the peer in a family-specific format
    address: Vec<u8>,
    /// The display number
    number: Vec<u8>,
    /// The name of the authentication method to use for the X11 server described by the previous
    /// fields.
    name: Vec<u8>,
    /// Extra data for the authentication method.
    data: Vec<u8>,
}

mod file {
    //! Code for actually reading `~/.Xauthority`.

    use alloc::{vec, vec::Vec};
    use std::env::var_os;
    use std::fs::File;
    use std::io::{BufReader, Error, ErrorKind, Read};
    use std::path::PathBuf;

    use super::AuthEntry;

    /// Read a single `u16` from an `~/.Xauthority` file.
    ///
    /// The file stores these entries in big endian.
    fn read_u16<R: Read>(read: &mut R) -> Result<u16, Error> {
        let mut buffer = [0; 2];
        read.read_exact(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }

    /// Read a single "byte array" from an `~/.Xauthority` file.
    ///
    /// The file stores these as a length field followed by a number of bytes that contain the
    /// actual data.
    fn read_string<R: Read>(read: &mut R) -> Result<Vec<u8>, Error> {
        let length = read_u16(read)?;
        let mut result = vec![0; length.into()];
        read.read_exact(&mut result[..])?;
        Ok(result)
    }

    /// Read a single entry from an `~/.Xauthority` file.
    ///
    /// This function tries to return `Ok(None)` when the end of the file is reached. However, the
    /// code also treats a single byte as 'end of file', because things were simpler to implement
    /// like this.
    fn read_entry<R: Read>(read: &mut R) -> Result<Option<AuthEntry>, Error> {
        let family = match read_u16(read) {
            Ok(family) => family,
            Err(ref e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        .into();
        let address = read_string(read)?;
        let number = read_string(read)?;
        let name = read_string(read)?;
        let data = read_string(read)?;
        Ok(Some(AuthEntry {
            family,
            address,
            number,
            name,
            data,
        }))
    }

    /// Get the file name for `~/.Xauthority` based on environment variables.
    ///
    /// The code in libXau contains a special case for Windows (looks like cygwin) that is not
    /// handled here (yet?).
    fn get_xauthority_file_name() -> Option<PathBuf> {
        if let Some(name) = var_os("XAUTHORITY") {
            return Some(name.into());
        }
        var_os("HOME").map(|prefix| {
            let mut result = PathBuf::new();
            result.push(prefix);
            result.push(".Xauthority");
            result
        })
    }

    /// An iterator over the entries of an `.Xauthority` file
    #[derive(Debug)]
    pub(crate) struct XAuthorityEntries(BufReader<File>);

    impl XAuthorityEntries {
        /// Open `~/.Xauthority` for reading.
        ///
        /// This function returns `Ok(None)` when the location of the `.Xauthority` file could not
        /// be determined. If opening the file failed (for example, because it does not exist),
        /// that error is returned.
        pub(crate) fn new() -> Result<Option<XAuthorityEntries>, Error> {
            get_xauthority_file_name()
                .map(File::open)
                .transpose()?
                // At this point we have Option<File> and errors while opening the file were
                // returned to the caller.
                .map(|file| Ok(XAuthorityEntries(BufReader::new(file))))
                .transpose()
        }
    }

    impl Iterator for XAuthorityEntries {
        type Item = Result<AuthEntry, Error>;

        fn next(&mut self) -> Option<Self::Item> {
            read_entry(&mut self.0).transpose()
        }
    }

    #[cfg(test)]
    mod test {
        use super::super::{AuthEntry, Family};
        use super::read_entry;
        use alloc::vec;
        use std::io::Cursor;

        #[test]
        fn test_read() {
            // Data generated via xauth -f /tmp/file add :1 bar deadbeef
            let data = [
                0x01, 0x00, 0x00, 0x07, 0x5a, 0x77, 0x65, 0x69, 0x4c, 0x45, 0x44, 0x00, 0x01, 0x31,
                0x00, 0x03, 0x62, 0x61, 0x72, 0x00, 0x04, 0xde, 0xad, 0xbe, 0xef,
            ];
            let mut cursor = Cursor::new(&data[..]);
            let entry = read_entry(&mut cursor).unwrap();
            assert_eq!(
                entry,
                Some(AuthEntry {
                    family: Family::LOCAL,
                    address: b"ZweiLED".to_vec(),
                    number: b"1".to_vec(),
                    name: b"bar".to_vec(),
                    data: u32::to_be_bytes(0xdead_beef).to_vec(),
                })
            );
        }

        #[test]
        fn test_read_iterate() {
            // Data generated via:
            //   xauth -f /tmp/file add :1 bar deadbeef
            //   xauth -f /tmp/file add 1.2.3.4:2 baz aabbccdd
            let data = [
                0x01, 0x00, 0x00, 0x07, 0x5a, 0x77, 0x65, 0x69, 0x4c, 0x45, 0x44, 0x00, 0x01, 0x31,
                0x00, 0x03, 0x62, 0x61, 0x72, 0x00, 0x04, 0xde, 0xad, 0xbe, 0xef, 0x00, 0x00, 0x00,
                0x04, 0x01, 0x02, 0x03, 0x04, 0x00, 0x01, 0x32, 0x00, 0x03, 0x62, 0x61, 0x7a, 0x00,
                0x04, 0xaa, 0xbb, 0xcc, 0xdd,
            ];
            let mut cursor = Cursor::new(&data[..]);
            for expected in &[
                AuthEntry {
                    family: Family::LOCAL,
                    address: b"ZweiLED".to_vec(),
                    number: b"1".to_vec(),
                    name: b"bar".to_vec(),
                    data: u32::to_be_bytes(0xdead_beef).to_vec(),
                },
                AuthEntry {
                    family: Family::INTERNET,
                    address: vec![1, 2, 3, 4],
                    number: b"2".to_vec(),
                    name: b"baz".to_vec(),
                    data: u32::to_be_bytes(0xaabb_ccdd).to_vec(),
                },
            ] {
                let entry = read_entry(&mut cursor).unwrap();
                assert_eq!(entry.as_ref(), Some(expected));
            }
            let entry = read_entry(&mut cursor).unwrap();
            assert_eq!(entry, None);
        }
    }
}

pub(crate) type AuthInfo = (Vec<u8>, Vec<u8>);

/// Get the authentication information necessary for connecting to the given display.
///
/// - `family` is the protocol family that is used for connecting; this describes how to interpret
///   the `address`.
/// - `address` is the raw bytes describing the address that is being connected to.
/// - `display` is the display number.
///
/// If successful, this function returns that can be written to the X11 server as authorization
/// protocol name and data, respectively.
pub fn get_auth(family: Family, address: &[u8], display: u16) -> Result<Option<AuthInfo>, Error> {
    match file::XAuthorityEntries::new()? {
        None => Ok(None),
        Some(entries) => get_auth_impl(entries, family, address, display),
    }
}

fn get_auth_impl(
    entries: impl Iterator<Item = Result<AuthEntry, Error>>,
    family: Family,
    address: &[u8],
    display: u16,
) -> Result<Option<AuthInfo>, Error> {
    fn address_matches(
        (family1, address1): (Family, &[u8]),
        (family2, address2): (Family, &[u8]),
    ) -> bool {
        if family1 == Family::WILD || family2 == Family::WILD {
            true
        } else if family1 != family2 {
            false
        } else {
            address1 == address2
        }
    }

    fn display_number_matches(entry_number: &[u8], display_number: &[u8]) -> bool {
        debug_assert!(!display_number.is_empty()); // This case is not handled here and would be a match
        entry_number.is_empty() || entry_number == display_number
    }

    let display = display.to_string();
    let display = display.as_bytes();

    for entry in entries {
        let entry = entry?;

        if address_matches((family, address), (entry.family, &entry.address))
            && display_number_matches(&entry.number, display)
            && entry.name == MIT_MAGIC_COOKIE_1
        {
            return Ok(Some((entry.name, entry.data)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod test {
    use super::{get_auth_impl, AuthEntry, Family, MIT_MAGIC_COOKIE_1};
    use alloc::vec;

    // Call the given function on a matching auth entry. The function can change the entry.
    // Afterwards, it should still be a match.
    fn expect_match<F>(f: F)
    where
        F: FnOnce(&mut AuthEntry),
    {
        let mut entry = AuthEntry {
            family: Family::LOCAL,
            address: b"whatever".to_vec(),
            number: b"42".to_vec(),
            name: MIT_MAGIC_COOKIE_1.to_vec(),
            data: b"1234".to_vec(),
        };
        f(&mut entry);
        let entries = vec![Ok(entry)];
        assert_eq!(
            get_auth_impl(entries.into_iter(), Family::LOCAL, b"whatever", 42)
                .unwrap()
                .unwrap(),
            (MIT_MAGIC_COOKIE_1.to_vec(), b"1234".to_vec())
        );
    }

    // Call the given function on a matching auth entry. The function can change the entry.
    // Afterwards, it should no longer match.
    fn expect_mismatch<F>(f: F)
    where
        F: FnOnce(&mut AuthEntry),
    {
        let mut entry = AuthEntry {
            family: Family::LOCAL,
            address: b"whatever".to_vec(),
            number: b"42".to_vec(),
            name: MIT_MAGIC_COOKIE_1.to_vec(),
            data: b"1234".to_vec(),
        };
        f(&mut entry);
        let entries = vec![Ok(entry)];
        assert_eq!(
            get_auth_impl(entries.into_iter(), Family::LOCAL, b"whatever", 42).unwrap(),
            None
        );
    }

    #[test]
    fn direct_match() {
        // This checks that an auth entry where all members match, really matches
        expect_match(|_| {});
    }

    #[test]
    fn display_wildcard() {
        expect_match(|entry| entry.number = vec![]);
    }

    #[test]
    fn address_wildcard_match1() {
        expect_match(|entry| entry.family = Family::WILD);
    }

    #[test]
    fn address_wildcard_match2() {
        let entry = AuthEntry {
            family: Family::LOCAL,
            address: b"whatever".to_vec(),
            number: b"42".to_vec(),
            name: MIT_MAGIC_COOKIE_1.to_vec(),
            data: b"1234".to_vec(),
        };
        let entries = vec![Ok(entry)];
        assert_eq!(
            get_auth_impl(entries.into_iter(), Family::WILD, &[], 42)
                .unwrap()
                .unwrap(),
            (MIT_MAGIC_COOKIE_1.to_vec(), b"1234".to_vec())
        );
    }

    #[test]
    fn family_mismatch() {
        expect_mismatch(|entry| entry.family = Family::KRB5_PRINCIPAL);
    }

    #[test]
    fn address_mismatch() {
        expect_mismatch(|entry| entry.address = b"something else".to_vec());
    }

    #[test]
    fn number_mismatch() {
        expect_mismatch(|entry| entry.number = b"1337".to_vec());
    }

    #[test]
    fn protocol_mismatch() {
        expect_mismatch(|entry| entry.name = b"XDM-AUTHORIZATION-1".to_vec());
    }
}
