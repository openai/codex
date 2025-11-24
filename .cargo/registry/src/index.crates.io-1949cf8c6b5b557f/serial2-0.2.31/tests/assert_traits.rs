use serial2::SerialPort;
use std::io::{Read, Write};

fn assert_read<T: Read>() {}
fn assert_write<T: Write>() {}

#[test]
fn assert_read_is_implemented() {
	assert_read::<SerialPort>();
	assert_read::<&SerialPort>();
	assert_read::<&mut SerialPort>();
	assert_read::<&mut &SerialPort>();
}

#[test]
fn assert_write_is_implemented() {
	assert_write::<SerialPort>();
	assert_write::<&SerialPort>();
	assert_write::<&mut SerialPort>();
	assert_write::<&mut &SerialPort>();
}
