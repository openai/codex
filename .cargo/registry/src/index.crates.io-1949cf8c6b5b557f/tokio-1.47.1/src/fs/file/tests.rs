use super::*;
use crate::{
    fs::mocks::*,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};
use mockall::{predicate::eq, Sequence};
use tokio_test::{assert_pending, assert_ready_err, assert_ready_ok, task};

const HELLO: &[u8] = b"hello world...";
const FOO: &[u8] = b"foo bar baz...";

#[test]
fn open_read() {
    let mut file = MockFile::default();
    file.expect_inner_read().once().returning(|buf| {
        buf[0..HELLO.len()].copy_from_slice(HELLO);
        Ok(HELLO.len())
    });
    let mut file = File::from_std(file);

    let mut buf = [0; 1024];
    let mut t = task::spawn(file.read(&mut buf));

    assert_eq!(0, pool::len());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());

    pool::run_one();

    assert!(t.is_woken());

    let n = assert_ready_ok!(t.poll());
    assert_eq!(n, HELLO.len());
    assert_eq!(&buf[..n], HELLO);
}

#[test]
fn read_twice_before_dispatch() {
    let mut file = MockFile::default();
    file.expect_inner_read().once().returning(|buf| {
        buf[0..HELLO.len()].copy_from_slice(HELLO);
        Ok(HELLO.len())
    });
    let mut file = File::from_std(file);

    let mut buf = [0; 1024];
    let mut t = task::spawn(file.read(&mut buf));

    assert_pending!(t.poll());
    assert_pending!(t.poll());

    assert_eq!(pool::len(), 1);
    pool::run_one();

    assert!(t.is_woken());

    let n = assert_ready_ok!(t.poll());
    assert_eq!(&buf[..n], HELLO);
}

#[test]
fn read_with_smaller_buf() {
    let mut file = MockFile::default();
    file.expect_inner_read().once().returning(|buf| {
        buf[0..HELLO.len()].copy_from_slice(HELLO);
        Ok(HELLO.len())
    });

    let mut file = File::from_std(file);

    {
        let mut buf = [0; 32];
        let mut t = task::spawn(file.read(&mut buf));
        assert_pending!(t.poll());
    }

    pool::run_one();

    {
        let mut buf = [0; 4];
        let mut t = task::spawn(file.read(&mut buf));
        let n = assert_ready_ok!(t.poll());
        assert_eq!(n, 4);
        assert_eq!(&buf[..], &HELLO[..n]);
    }

    // Calling again immediately succeeds with the rest of the buffer
    let mut buf = [0; 32];
    let mut t = task::spawn(file.read(&mut buf));
    let n = assert_ready_ok!(t.poll());
    assert_eq!(n, 10);
    assert_eq!(&buf[..n], &HELLO[4..]);

    assert_eq!(0, pool::len());
}

#[test]
fn read_with_bigger_buf() {
    let mut seq = Sequence::new();
    let mut file = MockFile::default();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..4].copy_from_slice(&HELLO[..4]);
            Ok(4)
        });
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len() - 4].copy_from_slice(&HELLO[4..]);
            Ok(HELLO.len() - 4)
        });

    let mut file = File::from_std(file);

    {
        let mut buf = [0; 4];
        let mut t = task::spawn(file.read(&mut buf));
        assert_pending!(t.poll());
    }

    pool::run_one();

    {
        let mut buf = [0; 32];
        let mut t = task::spawn(file.read(&mut buf));
        let n = assert_ready_ok!(t.poll());
        assert_eq!(n, 4);
        assert_eq!(&buf[..n], &HELLO[..n]);
    }

    // Calling again immediately succeeds with the rest of the buffer
    let mut buf = [0; 32];
    let mut t = task::spawn(file.read(&mut buf));

    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());

    let n = assert_ready_ok!(t.poll());
    assert_eq!(n, 10);
    assert_eq!(&buf[..n], &HELLO[4..]);

    assert_eq!(0, pool::len());
}

#[test]
fn read_err_then_read_success() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|_| Err(io::ErrorKind::Other.into()));
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });

    let mut file = File::from_std(file);

    {
        let mut buf = [0; 32];
        let mut t = task::spawn(file.read(&mut buf));
        assert_pending!(t.poll());

        pool::run_one();

        assert_ready_err!(t.poll());
    }

    {
        let mut buf = [0; 32];
        let mut t = task::spawn(file.read(&mut buf));
        assert_pending!(t.poll());

        pool::run_one();

        let n = assert_ready_ok!(t.poll());

        assert_eq!(n, HELLO.len());
        assert_eq!(&buf[..n], HELLO);
    }
}

#[test]
fn open_write() {
    let mut file = MockFile::default();
    file.expect_inner_write()
        .once()
        .with(eq(HELLO))
        .returning(|buf| Ok(buf.len()));

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));

    assert_eq!(0, pool::len());
    assert_ready_ok!(t.poll());

    assert_eq!(1, pool::len());

    pool::run_one();

    assert!(!t.is_woken());

    let mut t = task::spawn(file.flush());
    assert_ready_ok!(t.poll());
}

#[test]
fn flush_while_idle() {
    let file = MockFile::default();

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.flush());
    assert_ready_ok!(t.poll());
}

#[test]
#[cfg_attr(miri, ignore)] // takes a really long time with miri
fn read_with_buffer_larger_than_max() {
    // Chunks
    let chunk_a = crate::io::blocking::DEFAULT_MAX_BUF_SIZE;
    let chunk_b = chunk_a * 2;
    let chunk_c = chunk_a * 3;
    let chunk_d = chunk_a * 4;

    assert_eq!(chunk_d / 1024 / 1024, 8);

    let mut data = vec![];
    for i in 0..(chunk_d - 1) {
        data.push((i % 151) as u8);
    }
    let data = Arc::new(data);
    let d0 = data.clone();
    let d1 = data.clone();
    let d2 = data.clone();
    let d3 = data.clone();

    let mut seq = Sequence::new();
    let mut file = MockFile::default();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(move |buf| {
            buf[0..chunk_a].copy_from_slice(&d0[0..chunk_a]);
            Ok(chunk_a)
        });
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(move |buf| {
            buf[..chunk_a].copy_from_slice(&d1[chunk_a..chunk_b]);
            Ok(chunk_b - chunk_a)
        });
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(move |buf| {
            buf[..chunk_a].copy_from_slice(&d2[chunk_b..chunk_c]);
            Ok(chunk_c - chunk_b)
        });
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(move |buf| {
            buf[..chunk_a - 1].copy_from_slice(&d3[chunk_c..]);
            Ok(chunk_a - 1)
        });
    let mut file = File::from_std(file);

    let mut actual = vec![0; chunk_d];
    let mut pos = 0;

    while pos < data.len() {
        let mut t = task::spawn(file.read(&mut actual[pos..]));

        assert_pending!(t.poll());
        pool::run_one();
        assert!(t.is_woken());

        let n = assert_ready_ok!(t.poll());
        assert!(n <= chunk_a);

        pos += n;
    }

    assert_eq!(&data[..], &actual[..data.len()]);
}

#[test]
#[cfg_attr(miri, ignore)] // takes a really long time with miri
fn write_with_buffer_larger_than_max() {
    // Chunks
    let chunk_a = crate::io::blocking::DEFAULT_MAX_BUF_SIZE;
    let chunk_b = chunk_a * 2;
    let chunk_c = chunk_a * 3;
    let chunk_d = chunk_a * 4;

    assert_eq!(chunk_d / 1024 / 1024, 8);

    let mut data = vec![];
    for i in 0..(chunk_d - 1) {
        data.push((i % 151) as u8);
    }
    let data = Arc::new(data);
    let d0 = data.clone();
    let d1 = data.clone();
    let d2 = data.clone();
    let d3 = data.clone();

    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .withf(move |buf| buf == &d0[0..chunk_a])
        .returning(|buf| Ok(buf.len()));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .withf(move |buf| buf == &d1[chunk_a..chunk_b])
        .returning(|buf| Ok(buf.len()));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .withf(move |buf| buf == &d2[chunk_b..chunk_c])
        .returning(|buf| Ok(buf.len()));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .withf(move |buf| buf == &d3[chunk_c..chunk_d - 1])
        .returning(|buf| Ok(buf.len()));

    let mut file = File::from_std(file);

    let mut rem = &data[..];

    let mut first = true;

    while !rem.is_empty() {
        let mut task = task::spawn(file.write(rem));

        if !first {
            assert_pending!(task.poll());
            pool::run_one();
            assert!(task.is_woken());
        }

        first = false;

        let n = assert_ready_ok!(task.poll());

        rem = &rem[n..];
    }

    pool::run_one();
}

#[test]
fn write_twice_before_dispatch() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(HELLO))
        .returning(|buf| Ok(buf.len()));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(FOO))
        .returning(|buf| Ok(buf.len()));

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.write(FOO));
    assert_pending!(t.poll());

    assert_eq!(pool::len(), 1);
    pool::run_one();

    assert!(t.is_woken());

    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.flush());
    assert_pending!(t.poll());

    assert_eq!(pool::len(), 1);
    pool::run_one();

    assert!(t.is_woken());
    assert_ready_ok!(t.poll());
}

#[test]
fn incomplete_read_followed_by_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });
    file.expect_inner_seek()
        .once()
        .with(eq(SeekFrom::Current(-(HELLO.len() as i64))))
        .in_sequence(&mut seq)
        .returning(|_| Ok(0));
    file.expect_inner_write()
        .once()
        .with(eq(FOO))
        .returning(|_| Ok(FOO.len()));

    let mut file = File::from_std(file);

    let mut buf = [0; 32];

    let mut t = task::spawn(file.read(&mut buf));
    assert_pending!(t.poll());

    pool::run_one();

    let mut t = task::spawn(file.write(FOO));
    assert_ready_ok!(t.poll());

    assert_eq!(pool::len(), 1);
    pool::run_one();

    let mut t = task::spawn(file.flush());
    assert_ready_ok!(t.poll());
}

#[test]
fn incomplete_partial_read_followed_by_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });
    file.expect_inner_seek()
        .once()
        .in_sequence(&mut seq)
        .with(eq(SeekFrom::Current(-10)))
        .returning(|_| Ok(0));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(FOO))
        .returning(|_| Ok(FOO.len()));

    let mut file = File::from_std(file);

    let mut buf = [0; 32];
    let mut t = task::spawn(file.read(&mut buf));
    assert_pending!(t.poll());

    pool::run_one();

    let mut buf = [0; 4];
    let mut t = task::spawn(file.read(&mut buf));
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.write(FOO));
    assert_ready_ok!(t.poll());

    assert_eq!(pool::len(), 1);
    pool::run_one();

    let mut t = task::spawn(file.flush());
    assert_ready_ok!(t.poll());
}

#[test]
fn incomplete_read_followed_by_flush() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });
    file.expect_inner_seek()
        .once()
        .in_sequence(&mut seq)
        .with(eq(SeekFrom::Current(-(HELLO.len() as i64))))
        .returning(|_| Ok(0));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(FOO))
        .returning(|_| Ok(FOO.len()));

    let mut file = File::from_std(file);

    let mut buf = [0; 32];

    let mut t = task::spawn(file.read(&mut buf));
    assert_pending!(t.poll());

    pool::run_one();

    let mut t = task::spawn(file.flush());
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.write(FOO));
    assert_ready_ok!(t.poll());

    pool::run_one();
}

#[test]
fn incomplete_flush_followed_by_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(HELLO))
        .returning(|_| Ok(HELLO.len()));
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(FOO))
        .returning(|_| Ok(FOO.len()));

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    let n = assert_ready_ok!(t.poll());
    assert_eq!(n, HELLO.len());

    let mut t = task::spawn(file.flush());
    assert_pending!(t.poll());

    // TODO: Move under write
    pool::run_one();

    let mut t = task::spawn(file.write(FOO));
    assert_ready_ok!(t.poll());

    pool::run_one();

    let mut t = task::spawn(file.flush());
    assert_ready_ok!(t.poll());
}

#[test]
fn read_err() {
    let mut file = MockFile::default();
    file.expect_inner_read()
        .once()
        .returning(|_| Err(io::ErrorKind::Other.into()));

    let mut file = File::from_std(file);

    let mut buf = [0; 1024];
    let mut t = task::spawn(file.read(&mut buf));

    assert_pending!(t.poll());

    pool::run_one();
    assert!(t.is_woken());

    assert_ready_err!(t.poll());
}

#[test]
fn write_write_err() {
    let mut file = MockFile::default();
    file.expect_inner_write()
        .once()
        .returning(|_| Err(io::ErrorKind::Other.into()));

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    pool::run_one();

    let mut t = task::spawn(file.write(FOO));
    assert_ready_err!(t.poll());
}

#[test]
fn write_read_write_err() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .returning(|_| Err(io::ErrorKind::Other.into()));
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    pool::run_one();

    let mut buf = [0; 1024];
    let mut t = task::spawn(file.read(&mut buf));

    assert_pending!(t.poll());

    pool::run_one();

    let mut t = task::spawn(file.write(FOO));
    assert_ready_err!(t.poll());
}

#[test]
fn write_read_flush_err() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .returning(|_| Err(io::ErrorKind::Other.into()));
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    pool::run_one();

    let mut buf = [0; 1024];
    let mut t = task::spawn(file.read(&mut buf));

    assert_pending!(t.poll());

    pool::run_one();

    let mut t = task::spawn(file.flush());
    assert_ready_err!(t.poll());
}

#[test]
fn write_seek_write_err() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .returning(|_| Err(io::ErrorKind::Other.into()));
    file.expect_inner_seek()
        .once()
        .with(eq(SeekFrom::Start(0)))
        .in_sequence(&mut seq)
        .returning(|_| Ok(0));

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    pool::run_one();

    {
        let mut t = task::spawn(file.seek(SeekFrom::Start(0)));
        assert_pending!(t.poll());
    }

    pool::run_one();

    let mut t = task::spawn(file.write(FOO));
    assert_ready_err!(t.poll());
}

#[test]
fn write_seek_flush_err() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .returning(|_| Err(io::ErrorKind::Other.into()));
    file.expect_inner_seek()
        .once()
        .with(eq(SeekFrom::Start(0)))
        .in_sequence(&mut seq)
        .returning(|_| Ok(0));

    let mut file = File::from_std(file);

    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    pool::run_one();

    {
        let mut t = task::spawn(file.seek(SeekFrom::Start(0)));
        assert_pending!(t.poll());
    }

    pool::run_one();

    let mut t = task::spawn(file.flush());
    assert_ready_err!(t.poll());
}

#[test]
fn sync_all_ordered_after_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(HELLO))
        .returning(|_| Ok(HELLO.len()));
    file.expect_sync_all().once().returning(|| Ok(()));

    let mut file = File::from_std(file);
    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.sync_all());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_ready_ok!(t.poll());
}

#[test]
fn sync_all_err_ordered_after_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(HELLO))
        .returning(|_| Ok(HELLO.len()));
    file.expect_sync_all()
        .once()
        .returning(|| Err(io::ErrorKind::Other.into()));

    let mut file = File::from_std(file);
    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.sync_all());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_ready_err!(t.poll());
}

#[test]
fn sync_data_ordered_after_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(HELLO))
        .returning(|_| Ok(HELLO.len()));
    file.expect_sync_data().once().returning(|| Ok(()));

    let mut file = File::from_std(file);
    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.sync_data());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_ready_ok!(t.poll());
}

#[test]
fn sync_data_err_ordered_after_write() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .with(eq(HELLO))
        .returning(|_| Ok(HELLO.len()));
    file.expect_sync_data()
        .once()
        .returning(|| Err(io::ErrorKind::Other.into()));

    let mut file = File::from_std(file);
    let mut t = task::spawn(file.write(HELLO));
    assert_ready_ok!(t.poll());

    let mut t = task::spawn(file.sync_data());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_pending!(t.poll());

    assert_eq!(1, pool::len());
    pool::run_one();

    assert!(t.is_woken());
    assert_ready_err!(t.poll());
}

#[test]
fn open_set_len_ok() {
    let mut file = MockFile::default();
    file.expect_set_len().with(eq(123)).returning(|_| Ok(()));

    let file = File::from_std(file);
    let mut t = task::spawn(file.set_len(123));

    assert_pending!(t.poll());

    pool::run_one();

    assert!(t.is_woken());
    assert_ready_ok!(t.poll());
}

#[test]
fn open_set_len_err() {
    let mut file = MockFile::default();
    file.expect_set_len()
        .with(eq(123))
        .returning(|_| Err(io::ErrorKind::Other.into()));

    let file = File::from_std(file);
    let mut t = task::spawn(file.set_len(123));

    assert_pending!(t.poll());

    pool::run_one();

    assert!(t.is_woken());
    assert_ready_err!(t.poll());
}

#[test]
fn partial_read_set_len_ok() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..HELLO.len()].copy_from_slice(HELLO);
            Ok(HELLO.len())
        });
    file.expect_inner_seek()
        .once()
        .with(eq(SeekFrom::Current(-(HELLO.len() as i64))))
        .in_sequence(&mut seq)
        .returning(|_| Ok(0));
    file.expect_set_len()
        .once()
        .in_sequence(&mut seq)
        .with(eq(123))
        .returning(|_| Ok(()));
    file.expect_inner_read()
        .once()
        .in_sequence(&mut seq)
        .returning(|buf| {
            buf[0..FOO.len()].copy_from_slice(FOO);
            Ok(FOO.len())
        });

    let mut buf = [0; 32];
    let mut file = File::from_std(file);

    {
        let mut t = task::spawn(file.read(&mut buf));
        assert_pending!(t.poll());
    }

    pool::run_one();

    {
        let mut t = task::spawn(file.set_len(123));

        assert_pending!(t.poll());
        pool::run_one();
        assert_ready_ok!(t.poll());
    }

    let mut t = task::spawn(file.read(&mut buf));
    assert_pending!(t.poll());
    pool::run_one();
    let n = assert_ready_ok!(t.poll());

    assert_eq!(n, FOO.len());
    assert_eq!(&buf[..n], FOO);
}

#[test]
fn busy_file_seek_error() {
    let mut file = MockFile::default();
    let mut seq = Sequence::new();
    file.expect_inner_write()
        .once()
        .in_sequence(&mut seq)
        .returning(|_| Err(io::ErrorKind::Other.into()));

    let mut file = crate::io::BufReader::new(File::from_std(file));
    {
        let mut t = task::spawn(file.write(HELLO));
        assert_ready_ok!(t.poll());
    }

    pool::run_one();

    let mut t = task::spawn(file.seek(SeekFrom::Start(0)));
    assert_ready_err!(t.poll());
}
