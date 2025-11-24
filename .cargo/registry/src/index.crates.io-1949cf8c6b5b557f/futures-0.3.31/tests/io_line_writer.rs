use futures::executor::block_on;
use futures::io::{AsyncWriteExt, LineWriter};
use std::io;

#[test]
fn line_writer() {
    let mut writer = LineWriter::new(Vec::new());

    block_on(writer.write(&[0])).unwrap();
    assert_eq!(*writer.get_ref(), []);

    block_on(writer.write(&[1])).unwrap();
    assert_eq!(*writer.get_ref(), []);

    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1]);

    block_on(writer.write(&[0, b'\n', 1, b'\n', 2])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n']);

    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2]);

    block_on(writer.write(&[3, b'\n'])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2, 3, b'\n']);
}

#[test]
fn line_vectored() {
    let mut line_writer = LineWriter::new(Vec::new());
    assert_eq!(
        block_on(line_writer.write_vectored(&[
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"\n"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"a"),
        ]))
        .unwrap(),
        2
    );
    assert_eq!(line_writer.get_ref(), b"\n");

    assert_eq!(
        block_on(line_writer.write_vectored(&[
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"b"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"a"),
            io::IoSlice::new(&[]),
            io::IoSlice::new(b"c"),
        ]))
        .unwrap(),
        3
    );
    assert_eq!(line_writer.get_ref(), b"\n");
    block_on(line_writer.flush()).unwrap();
    assert_eq!(line_writer.get_ref(), b"\nabac");
    assert_eq!(block_on(line_writer.write_vectored(&[])).unwrap(), 0);

    assert_eq!(
        block_on(line_writer.write_vectored(&[
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
            io::IoSlice::new(&[]),
        ]))
        .unwrap(),
        0
    );

    assert_eq!(block_on(line_writer.write_vectored(&[io::IoSlice::new(b"a\nb")])).unwrap(), 3);
    assert_eq!(line_writer.get_ref(), b"\nabaca\nb");
}
