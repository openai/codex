use std::io::prelude::*;

use libc::off_t;
use nix::sys::sendfile::*;
use tempfile::tempfile;

cfg_if! {
    if #[cfg(linux_android)] {
        use nix::unistd::{pipe, read};
    } else if #[cfg(any(freebsdlike, apple_targets))] {
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;
    } else if #[cfg(solarish)] {
        use std::net::Shutdown;
        use std::net::{TcpListener, TcpStream};
    }
}

#[cfg(linux_android)]
#[test]
fn test_sendfile_linux() {
    const CONTENTS: &[u8] = b"abcdef123456";
    let mut tmp = tempfile().unwrap();
    tmp.write_all(CONTENTS).unwrap();

    let (rd, wr) = pipe().unwrap();
    let mut offset: off_t = 5;
    let res = sendfile(&wr, &tmp, Some(&mut offset), 2).unwrap();

    assert_eq!(2, res);

    let mut buf = [0u8; 1024];
    assert_eq!(2, read(&rd, &mut buf).unwrap());
    assert_eq!(b"f1", &buf[0..2]);
    assert_eq!(7, offset);
}

#[cfg(target_os = "linux")]
#[test]
fn test_sendfile64_linux() {
    const CONTENTS: &[u8] = b"abcdef123456";
    let mut tmp = tempfile().unwrap();
    tmp.write_all(CONTENTS).unwrap();

    let (rd, wr) = pipe().unwrap();
    let mut offset: libc::off64_t = 5;
    let res = sendfile64(&wr, &tmp, Some(&mut offset), 2).unwrap();

    assert_eq!(2, res);

    let mut buf = [0u8; 1024];
    assert_eq!(2, read(&rd, &mut buf).unwrap());
    assert_eq!(b"f1", &buf[0..2]);
    assert_eq!(7, offset);
}

#[cfg(target_os = "freebsd")]
#[test]
fn test_sendfile_freebsd() {
    // Declare the content
    let header_strings =
        ["HTTP/1.1 200 OK\n", "Content-Type: text/plain\n", "\n"];
    let body = "Xabcdef123456";
    let body_offset = 1;
    let trailer_strings = ["\n", "Served by Make Believe\n"];

    // Write the body to a file
    let mut tmp = tempfile().unwrap();
    tmp.write_all(body.as_bytes()).unwrap();

    // Prepare headers and trailers for sendfile
    let headers: Vec<&[u8]> =
        header_strings.iter().map(|s| s.as_bytes()).collect();
    let trailers: Vec<&[u8]> =
        trailer_strings.iter().map(|s| s.as_bytes()).collect();

    // Prepare socket pair
    let (mut rd, wr) = UnixStream::pair().unwrap();

    // Call the test method
    let (res, bytes_written) = sendfile(
        &tmp,
        &wr,
        body_offset as off_t,
        None,
        Some(headers.as_slice()),
        Some(trailers.as_slice()),
        SfFlags::empty(),
        0,
    );
    assert!(res.is_ok());
    wr.shutdown(Shutdown::Both).unwrap();

    // Prepare the expected result
    let expected_string = header_strings.concat()
        + &body[body_offset..]
        + &trailer_strings.concat();

    // Verify the message that was sent
    assert_eq!(bytes_written as usize, expected_string.len());

    let mut read_string = String::new();
    let bytes_read = rd.read_to_string(&mut read_string).unwrap();
    assert_eq!(bytes_written as usize, bytes_read);
    assert_eq!(expected_string, read_string);
}

#[cfg(target_os = "dragonfly")]
#[test]
fn test_sendfile_dragonfly() {
    // Declare the content
    let header_strings =
        ["HTTP/1.1 200 OK\n", "Content-Type: text/plain\n", "\n"];
    let body = "Xabcdef123456";
    let body_offset = 1;
    let trailer_strings = ["\n", "Served by Make Believe\n"];

    // Write the body to a file
    let mut tmp = tempfile().unwrap();
    tmp.write_all(body.as_bytes()).unwrap();

    // Prepare headers and trailers for sendfile
    let headers: Vec<&[u8]> =
        header_strings.iter().map(|s| s.as_bytes()).collect();
    let trailers: Vec<&[u8]> =
        trailer_strings.iter().map(|s| s.as_bytes()).collect();

    // Prepare socket pair
    let (mut rd, wr) = UnixStream::pair().unwrap();

    // Call the test method
    let (res, bytes_written) = sendfile(
        &tmp,
        &wr,
        body_offset as off_t,
        None,
        Some(headers.as_slice()),
        Some(trailers.as_slice()),
    );
    assert!(res.is_ok());
    wr.shutdown(Shutdown::Both).unwrap();

    // Prepare the expected result
    let expected_string = header_strings.concat()
        + &body[body_offset..]
        + &trailer_strings.concat();

    // Verify the message that was sent
    assert_eq!(bytes_written as usize, expected_string.len());

    let mut read_string = String::new();
    let bytes_read = rd.read_to_string(&mut read_string).unwrap();
    assert_eq!(bytes_written as usize, bytes_read);
    assert_eq!(expected_string, read_string);
}

#[cfg(apple_targets)]
#[test]
fn test_sendfile_darwin() {
    // Declare the content
    let header_strings =
        vec!["HTTP/1.1 200 OK\n", "Content-Type: text/plain\n", "\n"];
    let body = "Xabcdef123456";
    let body_offset = 1;
    let trailer_strings = vec!["\n", "Served by Make Believe\n"];

    // Write the body to a file
    let mut tmp = tempfile().unwrap();
    tmp.write_all(body.as_bytes()).unwrap();

    // Prepare headers and trailers for sendfile
    let headers: Vec<&[u8]> =
        header_strings.iter().map(|s| s.as_bytes()).collect();
    let trailers: Vec<&[u8]> =
        trailer_strings.iter().map(|s| s.as_bytes()).collect();

    // Prepare socket pair
    let (mut rd, wr) = UnixStream::pair().unwrap();

    // Call the test method
    let (res, bytes_written) = sendfile(
        &tmp,
        &wr,
        body_offset as off_t,
        None,
        Some(headers.as_slice()),
        Some(trailers.as_slice()),
    );
    assert!(res.is_ok());
    wr.shutdown(Shutdown::Both).unwrap();

    // Prepare the expected result
    let expected_string = header_strings.concat()
        + &body[body_offset..]
        + &trailer_strings.concat();

    // Verify the message that was sent
    assert_eq!(bytes_written as usize, expected_string.as_bytes().len());

    let mut read_string = String::new();
    let bytes_read = rd.read_to_string(&mut read_string).unwrap();
    assert_eq!(bytes_written as usize, bytes_read);
    assert_eq!(expected_string, read_string);
}

#[cfg(solarish)]
#[test]
fn test_sendfilev() {
    use std::os::fd::AsFd;
    // Declare the content
    let header_strings =
        ["HTTP/1.1 200 OK\n", "Content-Type: text/plain\n", "\n"];
    let body = "Xabcdef123456";
    let body_offset = 1usize;
    let trailer_strings = ["\n", "Served by Make Believe\n"];

    // Write data to files
    let mut header_data = tempfile().unwrap();
    header_data
        .write_all(header_strings.concat().as_bytes())
        .unwrap();
    let mut body_data = tempfile().unwrap();
    body_data.write_all(body.as_bytes()).unwrap();
    let mut trailer_data = tempfile().unwrap();
    trailer_data
        .write_all(trailer_strings.concat().as_bytes())
        .unwrap();
    // Create a TCP socket pair (listener and client)
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mut rd = TcpStream::connect(addr).unwrap();
    let (wr, _) = listener.accept().unwrap();
    let vec: &[SendfileVec] = &[
        SendfileVec::new(
            header_data.as_fd(),
            0,
            header_strings.iter().map(|s| s.len()).sum(),
        ),
        SendfileVec::new(
            body_data.as_fd(),
            body_offset as off_t,
            body.len() - body_offset,
        ),
        SendfileVec::new(
            trailer_data.as_fd(),
            0,
            trailer_strings.iter().map(|s| s.len()).sum(),
        ),
    ];

    let (res, bytes_written) = sendfilev(&wr, vec);
    assert!(res.is_ok());
    wr.shutdown(Shutdown::Write).unwrap();

    // Prepare the expected result
    let expected_string = header_strings.concat()
        + &body[body_offset..]
        + &trailer_strings.concat();

    // Verify the message that was sent
    assert_eq!(bytes_written, expected_string.as_bytes().len());

    let mut read_string = String::new();
    let bytes_read = rd.read_to_string(&mut read_string).unwrap();
    assert_eq!(bytes_written, bytes_read);
    assert_eq!(expected_string, read_string);
}
