
use httparse::{Error, Request, Status, EMPTY_HEADER};

const NUM_OF_HEADERS: usize = 4;

macro_rules! req {
    ($name:ident, $buf:expr, |$arg:ident| $body:expr) => (
        req! {$name, $buf, Ok(Status::Complete($buf.len())), |$arg| $body }
    );
    ($name:ident, $buf:expr, $len:expr, |$arg:ident| $body:expr) => (
    #[test]
    fn $name() {
        let mut headers = [EMPTY_HEADER; NUM_OF_HEADERS];
        let mut req = Request::new(&mut headers[..]);
        let status = req.parse($buf.as_ref());
        assert_eq!(status, $len);
        closure(req);

        fn closure($arg: Request<'_, '_>) {
            $body
        }
    }
    )
}

req! {
    urltest_001,
    b"GET /bar;par?b HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/bar;par?b");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}


req! {
    urltest_002,
    b"GET /x HTTP/1.1\r\nHost: test\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"test");
    }
}


req! {
    urltest_003,
    b"GET /x HTTP/1.1\r\nHost: test\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"test");
    }
}


req! {
    urltest_004,
    b"GET /foo/foo.com HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/foo.com");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_005,
    b"GET /foo/:foo.com HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:foo.com");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_006,
    b"GET /foo/foo.com HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/foo.com");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_007,
    b"GET  foo.com HTTP/1.1\r\nHost: \r\n\r\n",
    Err(Error::Token),
    |_r| {}
}


req! {
    urltest_008,
    b"GET /%20b%20?%20d%20 HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%20b%20?%20d%20");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}


req! {
    urltest_009,
    b"GET x x HTTP/1.1\r\nHost: \r\n\r\n",
    Err(Error::Version),
    |_r| {}
}


req! {
    urltest_010,
    b"GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}


req! {
    urltest_011,
    b"GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}


req! {
    urltest_012,
    b"GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}


req! {
    urltest_013,
    b"GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}


req! {
    urltest_014,
    b"GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}


req! {
    urltest_015,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_016,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_017,
    b"GET /foo/:foo.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:foo.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_018,
    b"GET /foo/:foo.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:foo.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_019,
    b"GET /foo/: HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_020,
    b"GET /foo/:a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_021,
    b"GET /foo/:/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_022,
    b"GET /foo/:/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_023,
    b"GET /foo/: HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_024,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_025,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_026,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_027,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_028,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_029,
    b"GET /foo/:23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:23");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_030,
    b"GET /:23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/:23");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_031,
    b"GET /foo/:: HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/::");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_032,
    b"GET /foo/::23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/::23");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_033,
    b"GET /d HTTP/1.1\r\nHost: c\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/d");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"c");
    }
}


req! {
    urltest_034,
    b"GET /foo/:@c:29 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/:@c:29");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_035,
    b"GET //@ HTTP/1.1\r\nHost: foo.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "//@");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo.com");
    }
}


req! {
    urltest_036,
    b"GET /b:c/d@foo.com/ HTTP/1.1\r\nHost: a\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/b:c/d@foo.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"a");
    }
}


req! {
    urltest_037,
    b"GET /bar.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/bar.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_038,
    b"GET /////// HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "///////");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_039,
    b"GET ///////bar.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "///////bar.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_040,
    b"GET //:///// HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "//://///");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_041,
    b"GET /foo HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_042,
    b"GET /bar HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}


req! {
    urltest_043,
    b"GET /path;a??e HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/path;a??e");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}


req! {
    urltest_044,
    b"GET /abcd?efgh?ijkl HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/abcd?efgh?ijkl");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}


req! {
    urltest_045,
    b"GET /abcd HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/abcd");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}


req! {
    urltest_046,
    b"GET /foo/[61:24:74]:98 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/[61:24:74]:98");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_047,
    b"GET /foo/[61:27]/:foo HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/[61:27]/:foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_048,
    b"GET /example.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_049,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_050,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_051,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_052,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_053,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_054,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_055,
    b"GET /foo/example.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_056,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_057,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_058,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_059,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_060,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_061,
    b"GET /a/b/c HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/a/b/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_062,
    b"GET /a/%20/c HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/a/%20/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_063,
    b"GET /a%2fc HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/a%2fc");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_064,
    b"GET /a/%2f/c HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/a/%2f/c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_065,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_066,
    b"GET text/html,test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "text/html,test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_067,
    b"GET 1234567890 HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "1234567890");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_068,
    b"GET /c:/foo/bar.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c:/foo/bar.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_069,
    b"GET /c:////foo/bar.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c:////foo/bar.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_070,
    b"GET /C:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_071,
    b"GET /C:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_072,
    b"GET /C:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_073,
    b"GET /file HTTP/1.1\r\nHost: server\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/file");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"server");
    }
}


req! {
    urltest_074,
    b"GET /file HTTP/1.1\r\nHost: server\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/file");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"server");
    }
}


req! {
    urltest_075,
    b"GET /file HTTP/1.1\r\nHost: server\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/file");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"server");
    }
}


req! {
    urltest_076,
    b"GET /foo/bar.txt HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_077,
    b"GET /home/me HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/home/me");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_078,
    b"GET /test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_079,
    b"GET /test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_080,
    b"GET /tmp/mock/test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/tmp/mock/test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_081,
    b"GET /tmp/mock/test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/tmp/mock/test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_082,
    b"GET /foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_083,
    b"GET /.foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/.foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_084,
    b"GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_085,
    b"GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_086,
    b"GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_087,
    b"GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_088,
    b"GET /foo/..bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/..bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_089,
    b"GET /foo/ton HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/ton");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_090,
    b"GET /a HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_091,
    b"GET /ton HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/ton");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_092,
    b"GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_093,
    b"GET /foo/%2e%2 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/%2e%2");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_094,
    b"GET /%2e.bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%2e.bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_095,
    b"GET // HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "//");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_096,
    b"GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_097,
    b"GET /foo/bar/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_098,
    b"GET /foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_099,
    b"GET /%20foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%20foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_100,
    b"GET /foo% HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_101,
    b"GET /foo%2 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%2");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_102,
    b"GET /foo%2zbar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%2zbar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_103,
    b"GET /foo%2%C3%82%C2%A9zbar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%2%C3%82%C2%A9zbar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_104,
    b"GET /foo%41%7a HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%41%7a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_105,
    b"GET /foo%C2%91%91 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%C2%91%91");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_106,
    b"GET /foo%00%51 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%00%51");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_107,
    b"GET /(%28:%3A%29) HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/(%28:%3A%29)");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_108,
    b"GET /%3A%3a%3C%3c HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%3A%3a%3C%3c");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_109,
    b"GET /foobar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foobar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_110,
    b"GET //foo//bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "//foo//bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_111,
    b"GET /%7Ffp3%3Eju%3Dduvgw%3Dd HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%7Ffp3%3Eju%3Dduvgw%3Dd");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_112,
    b"GET /@asdf%40 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/@asdf%40");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_113,
    b"GET /%E4%BD%A0%E5%A5%BD%E4%BD%A0%E5%A5%BD HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%E4%BD%A0%E5%A5%BD%E4%BD%A0%E5%A5%BD");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_114,
    b"GET /%E2%80%A5/foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%E2%80%A5/foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_115,
    b"GET /%EF%BB%BF/foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%EF%BB%BF/foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_116,
    b"GET /%E2%80%AE/foo/%E2%80%AD/bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%E2%80%AE/foo/%E2%80%AD/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_117,
    b"GET /foo?bar=baz HTTP/1.1\r\nHost: www.google.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo?bar=baz");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.google.com");
    }
}


req! {
    urltest_118,
    b"GET /foo?bar=baz HTTP/1.1\r\nHost: www.google.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo?bar=baz");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.google.com");
    }
}


req! {
    urltest_119,
    b"GET test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_120,
    b"GET /foo%2Ehtml HTTP/1.1\r\nHost: www\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo%2Ehtml");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www");
    }
}


req! {
    urltest_121,
    b"GET /foo/html HTTP/1.1\r\nHost: www\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www");
    }
}


req! {
    urltest_122,
    b"GET /foo HTTP/1.1\r\nHost: www.google.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.google.com");
    }
}


req! {
    urltest_123,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_124,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_125,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_126,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_127,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_128,
    b"GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_129,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_130,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_131,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_132,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_133,
    b"GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "example.com/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_134,
    b"GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_135,
    b"GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_136,
    b"GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_137,
    b"GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_138,
    b"GET /aaa/test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/aaa/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_139,
    b"GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_140,
    b"GET /%E4%B8%AD/test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%E4%B8%AD/test.txt");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}


req! {
    urltest_141,
    b"GET /... HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/...");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_142,
    b"GET /a HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_143,
    b"GET /%EF%BF%BD?%EF%BF%BD HTTP/1.1\r\nHost: x\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%EF%BF%BD?%EF%BF%BD");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"x");
    }
}


req! {
    urltest_144,
    b"GET /bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}


req! {
    urltest_145,
    b"GET test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_146,
    b"GET x@x.com HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "x@x.com");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_147,
    b"GET , HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), ",");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_148,
    b"GET blank HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "blank");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_149,
    b"GET test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "test?test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_150,
    b"GET /%60%7B%7D?`{} HTTP/1.1\r\nHost: h\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/%60%7B%7D?`{}");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"h");
    }

}


req! {
    urltest_151,
    b"GET /?%27 HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/?%27");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}


req! {
    urltest_152,
    b"GET /?' HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/?'");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}


req! {
    urltest_153,
    b"GET /some/path HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/some/path");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_154,
    b"GET /smth HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/smth");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_155,
    b"GET /some/path HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/some/path");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_156,
    b"GET /pa/i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_157,
    b"GET /i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}


req! {
    urltest_158,
    b"GET /pa/i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_159,
    b"GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_160,
    b"GET /i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}


req! {
    urltest_161,
    b"GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_162,
    b"GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_163,
    b"GET /i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}


req! {
    urltest_164,
    b"GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_165,
    b"GET /pa/pa?i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa/pa?i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_166,
    b"GET /pa?i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa?i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}


req! {
    urltest_167,
    b"GET /pa/pa?i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa/pa?i");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_168,
    b"GET sd HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "sd");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_169,
    b"GET sd/sd HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "sd/sd");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_170,
    b"GET /pa/pa HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa/pa");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_171,
    b"GET /pa HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}


req! {
    urltest_172,
    b"GET /pa/pa HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pa/pa");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_173,
    b"GET /x HTTP/1.1\r\nHost: %C3%B1\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"%C3%B1");
    }
}


req! {
    urltest_174,
    b"GET \\.\\./ HTTP/1.1\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "\\.\\./");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 0);
    }
}


req! {
    urltest_175,
    b"GET :a@example.net HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), ":a@example.net");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_176,
    b"GET %NBD HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "%NBD");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_177,
    b"GET %1G HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "%1G");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_178,
    b"GET /relative_import.html HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/relative_import.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"127.0.0.1");
    }
}


req! {
    urltest_179,
    b"GET /?foo=%7B%22abc%22 HTTP/1.1\r\nHost: facebook.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/?foo=%7B%22abc%22");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"facebook.com");
    }
}


req! {
    urltest_180,
    b"GET /jqueryui@1.2.3 HTTP/1.1\r\nHost: localhost\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/jqueryui@1.2.3");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"localhost");
    }
}


req! {
    urltest_181,
    b"GET /path?query HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/path?query");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}


req! {
    urltest_182,
    b"GET /foo/bar?a=b&c=d HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar?a=b&c=d");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_183,
    b"GET /foo/bar??a=b&c=d HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar??a=b&c=d");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_184,
    b"GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_185,
    b"GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/baz?qux");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}


req! {
    urltest_186,
    b"GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/baz?qux");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}


req! {
    urltest_187,
    b"GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/baz?qux");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}


req! {
    urltest_188,
    b"GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/baz?qux");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}


req! {
    urltest_189,
    b"GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/baz?qux");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}


req! {
    urltest_190,
    b"GET /C%3A/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C%3A/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_191,
    b"GET /C%7C/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C%7C/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_192,
    b"GET /C:/Users/Domenic/Dropbox/GitHub/tmpvar/jsdom/test/level2/html/files/pix/submit.gif HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/Users/Domenic/Dropbox/GitHub/tmpvar/jsdom/test/level2/html/files/pix/submit.gif");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_193,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_194,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_195,
    b"GET /d: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/d:");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_196,
    b"GET /d:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/d:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_197,
    b"GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_198,
    b"GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_199,
    b"GET /test?x HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_200,
    b"GET /test?x HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_201,
    b"GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_202,
    b"GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_203,
    b"GET /?fox HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/?fox");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_204,
    b"GET /localhost//cat HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/localhost//cat");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_205,
    b"GET /localhost//cat HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/localhost//cat");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_206,
    b"GET /mouse HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/mouse");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_207,
    b"GET /pig HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pig");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_208,
    b"GET /pig HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pig");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_209,
    b"GET /pig HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/pig");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_210,
    b"GET /localhost//pig HTTP/1.1\r\nHost: lion\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/localhost//pig");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"lion");
    }
}


req! {
    urltest_211,
    b"GET /rooibos HTTP/1.1\r\nHost: tea\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/rooibos");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"tea");
    }
}


req! {
    urltest_212,
    b"GET /?chai HTTP/1.1\r\nHost: tea\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/?chai");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"tea");
    }
}


req! {
    urltest_213,
    b"GET /C: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_214,
    b"GET /C: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_215,
    b"GET /C: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_216,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_217,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_218,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_219,
    b"GET /dir/C HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/dir/C");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}


req! {
    urltest_220,
    b"GET /dir/C|a HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/dir/C|a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}


req! {
    urltest_221,
    b"GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_222,
    b"GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_223,
    b"GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_224,
    b"GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/c:/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_225,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_226,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_227,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_228,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_229,
    b"GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/C:/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_230,
    b"GET /?q=v HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/?q=v");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_231,
    b"GET ?x HTTP/1.1\r\nHost: %C3%B1\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "?x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"%C3%B1");
    }
}


req! {
    urltest_232,
    b"GET ?x HTTP/1.1\r\nHost: %C3%B1\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "?x");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"%C3%B1");
    }
}


req! {
    urltest_233,
    b"GET // HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "//");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_234,
    b"GET //x/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "//x/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_235,
    b"GET /someconfig;mode=netascii HTTP/1.1\r\nHost: foobar.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/someconfig;mode=netascii");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"foobar.com");
    }
}


req! {
    urltest_236,
    b"GET /Index.ut2 HTTP/1.1\r\nHost: 10.10.10.10\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/Index.ut2");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"10.10.10.10");
    }
}


req! {
    urltest_237,
    b"GET /0?baz=bam&qux=baz HTTP/1.1\r\nHost: somehost\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/0?baz=bam&qux=baz");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"somehost");
    }
}


req! {
    urltest_238,
    b"GET /sup HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/sup");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}


req! {
    urltest_239,
    b"GET /foo/bar.git HTTP/1.1\r\nHost: github.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar.git");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"github.com");
    }
}


req! {
    urltest_240,
    b"GET /channel?passwd HTTP/1.1\r\nHost: myserver.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/channel?passwd");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"myserver.com");
    }
}


req! {
    urltest_241,
    b"GET /foo.bar.org?type=TXT HTTP/1.1\r\nHost: fw.example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo.bar.org?type=TXT");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"fw.example.org");
    }
}


req! {
    urltest_242,
    b"GET /ou=People,o=JNDITutorial HTTP/1.1\r\nHost: localhost\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/ou=People,o=JNDITutorial");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"localhost");
    }
}


req! {
    urltest_243,
    b"GET /foo/bar HTTP/1.1\r\nHost: github.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"github.com");
    }
}


req! {
    urltest_244,
    b"GET ietf:rfc:2648 HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "ietf:rfc:2648");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_245,
    b"GET joe@example.org,2001:foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "joe@example.org,2001:foo/bar");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_246,
    b"GET /path HTTP/1.1\r\nHost: H%4fSt\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/path");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"H%4fSt");
    }
}


req! {
    urltest_247,
    b"GET https://example.com:443/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "https://example.com:443/");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_248,
    b"GET d3958f5c-0777-0845-9dcf-2cb28783acaf HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "d3958f5c-0777-0845-9dcf-2cb28783acaf");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_249,
    b"GET /test?%22 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?%22");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_250,
    b"GET /test HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_251,
    b"GET /test?%3C HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?%3C");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_252,
    b"GET /test?%3E HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?%3E");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_253,
    b"GET /test?%E2%8C%A3 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?%E2%8C%A3");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_254,
    b"GET /test?%23%23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?%23%23");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_255,
    b"GET /test?%GH HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?%GH");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_256,
    b"GET /test?a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_257,
    b"GET /test?a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_258,
    b"GET /test-a-colon-slash.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test-a-colon-slash.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_259,
    b"GET /test-a-colon-slash-slash.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test-a-colon-slash-slash.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_260,
    b"GET /test-a-colon-slash-b.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test-a-colon-slash-b.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}


req! {
    urltest_261,
    b"GET /test-a-colon-slash-slash-b.html HTTP/1.1\r\nHost: b\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test-a-colon-slash-slash-b.html");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"b");
    }
}


req! {
    urltest_262,
    b"GET /test?a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/test?a");
        assert_eq!(req.version.unwrap(), 1);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}


req! {
    urltest_nvidia,
    b"GET /nvidia_web_services/controller.gfeclientcontent.php/com.nvidia.services.GFEClientContent.getShieldReady/{\"gcV\":\"2.2.2.0\",\"dID\":\"1341\",\"osC\":\"6.20\",\"is6\":\"1\",\"lg\":\"1033\",\"GFPV\":\"389.08\",\"isO\":\"1\",\"sM\":\"16777216\"} HTTP/1.0\r\nHost: gfwsl.geforce.com\r\n\r\n",
    |req| {
        assert_eq!(req.method.unwrap(), "GET");
        assert_eq!(req.path.unwrap(), "/nvidia_web_services/controller.gfeclientcontent.php/com.nvidia.services.GFEClientContent.getShieldReady/{\"gcV\":\"2.2.2.0\",\"dID\":\"1341\",\"osC\":\"6.20\",\"is6\":\"1\",\"lg\":\"1033\",\"GFPV\":\"389.08\",\"isO\":\"1\",\"sM\":\"16777216\"}");
        assert_eq!(req.version.unwrap(), 0);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].name, "Host");
        assert_eq!(req.headers[0].value, b"gfwsl.geforce.com");
    }
}
