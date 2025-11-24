use assert2::{assert, let_assert};

#[test]
fn test_convert_char_size() {
	assert!(serial2::CharSize::Bits5.as_u8() == 5);
	assert!(serial2::CharSize::Bits6.as_u8() == 6);
	assert!(serial2::CharSize::Bits7.as_u8() == 7);
	assert!(serial2::CharSize::Bits8.as_u8() == 8);

	assert!(let Ok(serial2::CharSize::Bits5) = serial2::CharSize::try_from(5));
	assert!(let Ok(serial2::CharSize::Bits6) = serial2::CharSize::try_from(6));
	assert!(let Ok(serial2::CharSize::Bits7) = serial2::CharSize::try_from(7));
	assert!(let Ok(serial2::CharSize::Bits8) = serial2::CharSize::try_from(8));

	let_assert!(Err(e) = serial2::CharSize::try_from(-8i8));
	assert!(e.to_string() == "invalid value: -8, expected the number 5, 6, 7 or 8");

	let_assert!(Err(e) = serial2::CharSize::try_from(-1i16));
	assert!(e.to_string() == "invalid value: -1, expected the number 5, 6, 7 or 8");

	let_assert!(Err(e) = serial2::CharSize::try_from(0u8));
	assert!(e.to_string() == "invalid value: 0, expected the number 5, 6, 7 or 8");

	let_assert!(Err(e) = serial2::CharSize::try_from(4u16));
	assert!(e.to_string() == "invalid value: 4, expected the number 5, 6, 7 or 8");

	let_assert!(Err(e) = serial2::CharSize::try_from(9usize));
	assert!(e.to_string() == "invalid value: 9, expected the number 5, 6, 7 or 8");
}

#[test]
fn test_convert_stop_bits() {
	assert!(serial2::StopBits::One.as_u8() == 1);
	assert!(serial2::StopBits::Two.as_u8() == 2);

	assert!(let Ok(serial2::StopBits::One) = serial2::StopBits::try_from(1));
	assert!(let Ok(serial2::StopBits::Two) = serial2::StopBits::try_from(2));

	let_assert!(Err(e) = serial2::StopBits::try_from(0u8));
	assert!(e.to_string() == "invalid value: 0, expected the number 1 or 2");

	let_assert!(Err(e) = serial2::StopBits::try_from(3u16));
	assert!(e.to_string() == "invalid value: 3, expected the number 1 or 2");

	let_assert!(Err(e) = serial2::StopBits::try_from(-8isize));
	assert!(e.to_string() == "invalid value: -8, expected the number 1 or 2");
}

#[test]
fn test_serde_parity() {
	assert!(serial2::Parity::None.as_str() == "none");
	assert!(serial2::Parity::Odd.as_str() == "odd");
	assert!(serial2::Parity::Even.as_str() == "even");

	assert!(let Ok(serial2::Parity::None) = serial2::Parity::from_str("none"));
	assert!(let Ok(serial2::Parity::Odd) = serial2::Parity::from_str("odd"));
	assert!(let Ok(serial2::Parity::Even) = serial2::Parity::from_str("even"));

	let_assert!(Err(e) = serial2::Parity::from_str("even-then-odd"));
	assert!(e.to_string() == "invalid value: \"even-then-odd\", expected the string \"none\", \"odd\" or \"even\"");
}

#[test]
fn test_serde_flow_control() {
	assert!(serial2::FlowControl::None.as_str() == "none");
	assert!(serial2::FlowControl::XonXoff.as_str() == "xon/xoff");
	assert!(serial2::FlowControl::RtsCts.as_str() == "rts/cts");

	assert!(let Ok(serial2::FlowControl::None) = serial2::FlowControl::from_str("none"));
	assert!(let Ok(serial2::FlowControl::XonXoff) = serial2::FlowControl::from_str("xon/xoff"));
	assert!(let Ok(serial2::FlowControl::RtsCts) = serial2::FlowControl::from_str("rts/cts"));

	let_assert!(Err(e) = serial2::FlowControl::from_str("plug-in/plug-out"));
	assert!(e.to_string() == "invalid value: \"plug-in/plug-out\", expected the string \"none\", \"xon/xoff\" or \"rts/cts\"");
}
