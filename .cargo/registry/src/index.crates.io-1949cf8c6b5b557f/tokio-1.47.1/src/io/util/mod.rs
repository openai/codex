#![allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411

cfg_io_util! {
    mod async_buf_read_ext;
    pub use async_buf_read_ext::AsyncBufReadExt;

    mod async_read_ext;
    pub use async_read_ext::AsyncReadExt;

    mod async_seek_ext;
    pub use async_seek_ext::AsyncSeekExt;

    mod async_write_ext;
    pub use async_write_ext::AsyncWriteExt;

    mod buf_reader;
    pub use buf_reader::BufReader;

    mod buf_stream;
    pub use buf_stream::BufStream;

    mod buf_writer;
    pub use buf_writer::BufWriter;

    mod chain;

    mod copy;
    pub use copy::copy;

    mod copy_bidirectional;
    pub use copy_bidirectional::{copy_bidirectional, copy_bidirectional_with_sizes};

    mod copy_buf;
    pub use copy_buf::copy_buf;

    mod empty;
    pub use empty::{empty, Empty};

    mod flush;

    mod lines;
    pub use lines::Lines;

    mod mem;
    pub use mem::{duplex, simplex, DuplexStream, SimplexStream};

    mod read;
    mod read_buf;
    mod read_exact;
    mod read_int;
    mod read_line;
    mod fill_buf;

    mod read_to_end;
    mod vec_with_initialized;
    cfg_process! {
        pub(crate) use read_to_end::read_to_end;
    }

    mod read_to_string;
    mod read_until;

    mod repeat;
    pub use repeat::{repeat, Repeat};

    mod shutdown;

    mod sink;
    pub use sink::{sink, Sink};

    mod split;
    pub use split::Split;

    mod take;
    pub use take::Take;

    mod write;
    mod write_vectored;
    mod write_all;
    mod write_buf;
    mod write_all_buf;
    mod write_int;


    // used by `BufReader` and `BufWriter`
    // https://github.com/rust-lang/rust/blob/master/library/std/src/sys_common/io.rs#L1
    const DEFAULT_BUF_SIZE: usize = 8 * 1024;

    cfg_coop! {
        fn poll_proceed_and_make_progress(cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
            let coop = std::task::ready!(crate::task::coop::poll_proceed(cx));
            coop.made_progress();
            std::task::Poll::Ready(())
        }
    }

    cfg_not_coop! {
        fn poll_proceed_and_make_progress(_: &mut std::task::Context<'_>) -> std::task::Poll<()> {
            std::task::Poll::Ready(())
        }
    }
}

cfg_not_io_util! {
    cfg_process! {
        mod vec_with_initialized;
        mod read_to_end;
        // Used by process
        pub(crate) use read_to_end::read_to_end;
    }
}
