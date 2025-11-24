#[cfg(feature = "resource_manager")]
mod test {
    use std::fs;
    use std::io::IoSlice;
    use std::path::{Path, PathBuf};

    use x11rb::connection::{
        BufWithFds, Connection, DiscardMode, RawEventAndSeqNumber, ReplyOrError, RequestConnection,
        RequestKind,
    };
    use x11rb::cookie::{Cookie, CookieWithFds, VoidCookie};
    use x11rb::errors::{ConnectionError, ParseError, ReplyOrIdError};
    use x11rb::protocol::xproto::{Screen, Setup};
    use x11rb::protocol::Event;
    use x11rb::resource_manager::{new_from_default, Database};
    use x11rb::utils::RawFdContainer;
    use x11rb::x11_utils::{ExtensionInformation, Serialize, TryParse, TryParseFd, X11Error};
    use x11rb_protocol::SequenceNumber;

    // Most tests in here are based on [1], which is: Copyright © 2016 Ingo Bürk
    // [1]: https://github.com/Airblader/xcb-util-xrm/blob/master/tests/tests_database.c

    /// Get the path to the `target` directory for the current build.
    ///
    /// This is inspired by cargo:
    /// https://github.com/rust-lang/cargo/blob/7302186d7beb2b11bf7366c0aa66269313ebe18f/crates/cargo-test-support/src/paths.rs#L18-L31
    fn get_temporary_dir(unique_name: &str) -> PathBuf {
        let mut path = std::env::current_exe().unwrap();
        while path.file_name().and_then(|n| n.to_str()) != Some("target") {
            path.pop();
        }
        path.push("test_data");
        path.push(unique_name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_file(base: impl AsRef<Path>, path: impl AsRef<Path>, content: &[u8]) -> PathBuf {
        let mut file_path = PathBuf::from(base.as_ref());
        file_path.push(path.as_ref());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, content).unwrap();
        file_path
    }

    fn database_from_file(file: impl AsRef<Path>) -> Database {
        // I decided against adding something like this to the API...
        let file = file.as_ref();
        let mut include = Vec::new();
        include.extend(b"#include \"");
        include.extend(file.file_name().unwrap().to_str().unwrap().bytes());
        include.extend(b"\"\n");
        Database::new_from_data_with_base_directory(&include, file.parent().unwrap())
    }

    fn check_db(db: &Database, queries: &[(&str, &[u8])]) {
        let mut errors = 0;
        for (query, expected) in queries {
            let result = db.get_bytes(query, "");
            if result != Some(expected) {
                eprintln!(
                    "Queried database for \"{}\" and expected {:?}, but got {:?}",
                    query, expected, result
                );
                errors += 1;
            }
        }
        assert_eq!(errors, 0, "Had {} errors", errors);
    }

    #[test]
    fn relative_include() {
        let dir = get_temporary_dir("relative_include");
        let file = write_file(
            &dir,
            "Xresources1",
            b"First: 1\n\n#include \"xresources2\"\n",
        );
        write_file(
            &dir,
            "xresources2",
            b"#include \"sub/xresources3\"\nSecond: 2\n",
        );
        write_file(&dir, "sub/xresources3", b"Third: 3\n");

        let db = database_from_file(file);
        check_db(&db, &[("First", b"1"), ("Second", b"2"), ("Third", b"3")]);
    }

    #[test]
    fn include_loop() {
        let dir = get_temporary_dir("include_loop");
        let file = write_file(
            dir,
            "loop.xresources",
            b"First: 1\n! Provoke an endless chain of self-inclusion\n#include \"loop.xresources\"\nSecond: 2\n",
        );
        let db = database_from_file(file);
        check_db(&db, &[("First", b"1")]);
    }

    #[test]
    fn home_resolution() {
        let mut dir = get_temporary_dir("home_resolution");
        write_file(&dir, ".Xresources", b"First: 1\n");
        write_file(&dir, "xenvironment", b"Second: 2\n");

        std::env::set_var("HOME", &dir);

        dir.push("xenvironment");
        std::env::set_var("XENVIRONMENT", dir);

        let conn = mock_connection(None);

        let db = new_from_default(&conn).unwrap();
        check_db(&db, &[("First", b"1"), ("Second", b"2")]);
    }

    #[test]
    fn from_resource_manager() {
        let conn = mock_connection(Some(b"First: 1\n*Second: 2\n"));
        let db = new_from_default(&conn).unwrap();
        check_db(&db, &[("First", b"1"), ("Second", b"2")]);
    }

    /// Create a mock connection that produces the given answer for queries of the RESOURCE_MANAGER
    /// property.
    fn mock_connection(resource_manager: Option<&[u8]>) -> impl Connection {
        // Ugly way to get a default screen: Parse enough zero bytes
        let screen = Screen::try_parse(&[0; 100]).unwrap().0;
        let mut setup = Setup::try_parse(&[0; 100]).unwrap().0;
        setup.roots.push(screen);

        MockConnection(setup, resource_manager.map(|v| v.to_vec()))
    }

    struct MockConnection(Setup, Option<Vec<u8>>);

    impl RequestConnection for MockConnection {
        type Buf = Vec<u8>;

        fn send_request_with_reply<R>(
            &self,
            _: &[IoSlice<'_>],
            _: Vec<RawFdContainer>,
        ) -> Result<Cookie<'_, Self, R>, ConnectionError>
        where
            R: TryParse,
        {
            Ok(Cookie::new(self, 42))
        }

        fn send_request_with_reply_with_fds<R>(
            &self,
            _: &[IoSlice<'_>],
            _: Vec<RawFdContainer>,
        ) -> Result<CookieWithFds<'_, Self, R>, ConnectionError>
        where
            R: TryParseFd,
        {
            unimplemented!()
        }

        fn send_request_without_reply(
            &self,
            _: &[IoSlice<'_>],
            _: Vec<RawFdContainer>,
        ) -> Result<VoidCookie<'_, Self>, ConnectionError> {
            unimplemented!()
        }

        fn discard_reply(&self, _: SequenceNumber, _: RequestKind, _: DiscardMode) {
            unimplemented!()
        }

        fn prefetch_extension_information(&self, _: &'static str) -> Result<(), ConnectionError> {
            unimplemented!()
        }

        fn extension_information(
            &self,
            _: &'static str,
        ) -> Result<Option<ExtensionInformation>, ConnectionError> {
            unimplemented!()
        }

        fn wait_for_reply_or_raw_error(
            &self,
            sequence: SequenceNumber,
        ) -> Result<ReplyOrError<Self::Buf>, ConnectionError> {
            let value = self.1.as_ref().map(|v| &v[..]).unwrap_or(&[]);
            // response type
            let mut reply = vec![1];
            // format
            if value.is_empty() {
                reply.push(0);
            } else {
                reply.push(8);
            }
            // sequence
            (sequence as u16).serialize_into(&mut reply);
            // length
            0u32.serialize_into(&mut reply);
            // type (STRING)
            31u32.serialize_into(&mut reply);
            // bytes_after
            0u32.serialize_into(&mut reply);
            (value.len() as u32).serialize_into(&mut reply);
            // padding
            reply.extend([0; 12].iter().copied());
            reply.extend(value);
            Ok(ReplyOrError::Reply(reply))
        }

        fn wait_for_reply(&self, _: SequenceNumber) -> Result<Option<Self::Buf>, ConnectionError> {
            unimplemented!()
        }

        fn wait_for_reply_with_fds_raw(
            &self,
            _: SequenceNumber,
        ) -> Result<ReplyOrError<BufWithFds<Self::Buf>, Self::Buf>, ConnectionError> {
            unimplemented!()
        }

        fn check_for_raw_error(
            &self,
            _: SequenceNumber,
        ) -> Result<Option<Self::Buf>, ConnectionError> {
            unimplemented!()
        }

        fn prefetch_maximum_request_bytes(&self) {
            unimplemented!()
        }

        fn maximum_request_bytes(&self) -> usize {
            unimplemented!()
        }

        fn parse_error(&self, _: &[u8]) -> Result<X11Error, ParseError> {
            unimplemented!()
        }

        fn parse_event(&self, _: &[u8]) -> Result<Event, ParseError> {
            unimplemented!()
        }
    }

    impl Connection for MockConnection {
        fn wait_for_raw_event_with_sequence(
            &self,
        ) -> Result<RawEventAndSeqNumber<Self::Buf>, ConnectionError> {
            unimplemented!()
        }

        fn poll_for_raw_event_with_sequence(
            &self,
        ) -> Result<Option<RawEventAndSeqNumber<Self::Buf>>, ConnectionError> {
            unimplemented!()
        }

        fn flush(&self) -> Result<(), ConnectionError> {
            unimplemented!()
        }

        fn setup(&self) -> &Setup {
            &self.0
        }

        fn generate_id(&self) -> Result<u32, ReplyOrIdError> {
            unimplemented!()
        }
    }
}
