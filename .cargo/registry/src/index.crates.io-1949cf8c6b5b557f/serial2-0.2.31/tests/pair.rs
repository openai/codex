#![cfg(unix)]

use assert2::{assert, let_assert};
use serial2::SerialPort;

#[test]
fn open_pair() {
	let_assert!(Ok((a, b)) = SerialPort::pair());
	assert!(let Ok(()) = a.write_all(b"Hello!"));
	let mut buffer = [0; 6];
	assert!(let Ok(()) = b.read_exact(&mut buffer));
	assert!(&buffer == b"Hello!");

	assert!(let Ok(()) = b.write_all(b"Goodbye!"));
	let mut buffer = [0; 8];
	assert!(let Ok(()) = a.read_exact(&mut buffer));
	assert!(&buffer == b"Goodbye!");
}
