mod prelude {
	pub use std::{io::Result, process::Stdio, time::Duration};

	pub use process_wrap::tokio::*;
	pub use tokio::{
		io::{AsyncReadExt, AsyncWriteExt},
		time::sleep,
	};

	pub const DIE_TIME: Duration = Duration::from_millis(1000);
}

mod id_same_as_inner;
mod inner_read_stdout;
mod into_inner_write_stdin;
mod kill_and_try_wait;
mod try_wait_after_die;
mod wait_after_die;
mod wait_twice;
mod wait_with_output;
