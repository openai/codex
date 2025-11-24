use ::std::{
    collections::HashMap, io::IoSlice, option::Option as RealOption, result::Result as RealResult,
    vec::Vec as RealVec,
};
use ::x11rb::{
    atom_manager,
    connection::{
        BufWithFds, DiscardMode, ReplyOrError, RequestConnection, RequestKind, SequenceNumber,
    },
    cookie::{Cookie, CookieWithFds, VoidCookie},
    errors::{ConnectionError, ParseError, ReplyError},
    protocol::xproto::{InternAtomReply, INTERN_ATOM_REQUEST},
    utils::RawFdContainer,
    x11_utils::{ExtensionInformation, Serialize, TryParse, TryParseFd},
};

// Just to be annoying, overwrite some standard types.
// We want to make sure atom_manager! does not use these directly.
mod std {}
mod x11rb {}
#[allow(dead_code)]
type Vec = ();
#[allow(dead_code)]
type Result = ();
#[allow(dead_code)]
type PhantomData = ();
#[allow(dead_code)]
type Option = ();

atom_manager! {
    Atoms: AtomsCookies {
        FIRST,
        SECOND,
        THIRD: b"3rd",
        LAST,
    }
}

struct AtomFakeConnection {
    // Mapping from byte string to the corresponding atom value and sequence number.
    // If no entry is found, sending the InternAtom request fails.
    // If the entry does not fit into u16, fetching the reply fails.
    atoms_and_cookies: HashMap<RealVec<u8>, u32>,
}

impl RequestConnection for AtomFakeConnection {
    type Buf = RealVec<u8>;

    fn send_request_with_reply<R>(
        &self,
        bufs: &[IoSlice],
        _fds: RealVec<RawFdContainer>,
    ) -> RealResult<Cookie<Self, R>, ConnectionError>
    where
        R: TryParse,
    {
        let bytes: RealVec<u8> = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        // request type and only-if-exists flag
        assert_eq!(bytes[..2], [INTERN_ATOM_REQUEST, 0]);
        // We ignore the length field
        let name_len = usize::from(u16::from_ne_bytes([bytes[4], bytes[5]]));
        let name = &bytes[8..(8 + name_len)];

        match self.atoms_and_cookies.get(name) {
            // We recycle the atom as the sequence number
            Some(&atom) => Ok(Cookie::new(self, atom.into())),
            None => Err(ConnectionError::UnsupportedExtension),
        }
    }

    fn send_request_with_reply_with_fds<R>(
        &self,
        _bufs: &[IoSlice],
        _fds: RealVec<RawFdContainer>,
    ) -> RealResult<CookieWithFds<Self, R>, ConnectionError>
    where
        R: TryParseFd,
    {
        unimplemented!()
    }

    fn send_request_without_reply(
        &self,
        _bufs: &[IoSlice],
        _fds: RealVec<RawFdContainer>,
    ) -> RealResult<VoidCookie<Self>, ConnectionError> {
        unimplemented!()
    }

    fn discard_reply(&self, _sequence: SequenceNumber, _kind: RequestKind, _mode: DiscardMode) {
        // Just ignore this
    }

    fn prefetch_extension_information(
        &self,
        _extension_name: &'static str,
    ) -> RealResult<(), ConnectionError> {
        unimplemented!();
    }

    fn extension_information(
        &self,
        _extension_name: &'static str,
    ) -> RealResult<RealOption<ExtensionInformation>, ConnectionError> {
        unimplemented!()
    }

    fn wait_for_reply_or_raw_error(
        &self,
        sequence: SequenceNumber,
    ) -> RealResult<ReplyOrError<RealVec<u8>>, ConnectionError> {
        let sequence_u16 = match u16::try_from(sequence) {
            Ok(value) => value,
            Err(_) => return Err(ConnectionError::UnsupportedExtension),
        };
        let reply = InternAtomReply {
            length: 0,
            sequence: sequence_u16,
            atom: sequence.try_into().unwrap(),
        };
        let mut data = reply.serialize().to_vec();
        data.extend([0; 32]);
        Ok(ReplyOrError::Reply(data))
    }

    fn wait_for_reply(
        &self,
        _sequence: SequenceNumber,
    ) -> RealResult<RealOption<RealVec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn wait_for_reply_with_fds_raw(
        &self,
        _sequence: SequenceNumber,
    ) -> RealResult<ReplyOrError<BufWithFds<RealVec<u8>>, RealVec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn check_for_raw_error(
        &self,
        _sequence: SequenceNumber,
    ) -> RealResult<RealOption<RealVec<u8>>, ConnectionError> {
        unimplemented!()
    }

    fn maximum_request_bytes(&self) -> usize {
        2usize.pow(16)
    }

    fn prefetch_maximum_request_bytes(&self) {
        unimplemented!()
    }

    fn parse_error(&self, _error: &[u8]) -> RealResult<::x11rb::x11_utils::X11Error, ParseError> {
        unimplemented!()
    }

    fn parse_event(&self, _event: &[u8]) -> RealResult<::x11rb::protocol::Event, ParseError> {
        unimplemented!()
    }
}

#[test]
fn test_atom_manager_success() {
    let conn = AtomFakeConnection {
        atoms_and_cookies: [
            (b"FIRST".to_vec(), 42),
            (b"SECOND".to_vec(), 50),
            (b"3rd".to_vec(), 100),
            (b"LAST".to_vec(), 200),
        ]
        .into(),
    };
    let atoms = Atoms::new(&conn).unwrap();
    let atoms = atoms.reply().unwrap();

    assert_eq!(atoms.FIRST, 42);
    assert_eq!(atoms.SECOND, 50);
    assert_eq!(atoms.THIRD, 100);
    assert_eq!(atoms.LAST, 200);
}

#[test]
fn test_atom_manager_sending_fails() {
    let conn = AtomFakeConnection {
        atoms_and_cookies: [
            (b"FIRST".to_vec(), 42),
            (b"3rd".to_vec(), 100),
            (b"LAST".to_vec(), 200),
        ]
        .into(),
    };
    match Atoms::new(&conn) {
        // This error is produced in send_request_with_reply() above
        Err(ConnectionError::UnsupportedExtension) => {}
        Err(err) => panic!("Unexpected error: {err:?}"),
        Ok(_) => unreachable!(),
    };
}

#[test]
fn test_atom_manager_receiving_fails() {
    // A value that does not fit into u16 indicates to AtomFakeConnection that receiving fails
    let too_large = u32::from(u16::MAX) * 3;
    let conn = AtomFakeConnection {
        atoms_and_cookies: [
            (b"FIRST".to_vec(), 42),
            (b"SECOND".to_vec(), too_large),
            (b"3rd".to_vec(), 100),
            (b"LAST".to_vec(), 200),
        ]
        .into(),
    };
    let atoms = Atoms::new(&conn).unwrap();
    match atoms.reply() {
        // This error is produced in wait_for_reply_or_raw_error() above
        Err(ReplyError::ConnectionError(ConnectionError::UnsupportedExtension)) => {}
        result => panic!("Unexpected result: {result:?}"),
    }
}
