use assert2::{assert, let_assert};

#[test]
fn test_serde_char_size() {
	assert!(let Ok("5") = serde_json::to_string(&serial2::CharSize::Bits5).as_deref());
	assert!(let Ok("6") = serde_json::to_string(&serial2::CharSize::Bits6).as_deref());
	assert!(let Ok("7") = serde_json::to_string(&serial2::CharSize::Bits7).as_deref());
	assert!(let Ok("8") = serde_json::to_string(&serial2::CharSize::Bits8).as_deref());

	assert!(let Ok(serial2::CharSize::Bits5) = serde_json::from_str::<serial2::CharSize>("5"));
	assert!(let Ok(serial2::CharSize::Bits6) = serde_json::from_str::<serial2::CharSize>("6"));
	assert!(let Ok(serial2::CharSize::Bits7) = serde_json::from_str::<serial2::CharSize>("7"));
	assert!(let Ok(serial2::CharSize::Bits8) = serde_json::from_str::<serial2::CharSize>("8"));

	let_assert!(Err(e) = serde_json::from_str::<serial2::CharSize>("4"));
	assert!(e.to_string() == "invalid value: integer `4`, expected the number 5, 6, 7 or 8 at line 1 column 1");

	let_assert!(Err(e) = serde_json::from_str::<serial2::CharSize>("9"));
	assert!(e.to_string() == "invalid value: integer `9`, expected the number 5, 6, 7 or 8 at line 1 column 1");

	let_assert!(Err(e) = serde_json::from_str::<serial2::CharSize>("\"5\""));
	assert!(e.to_string() == "invalid type: string \"5\", expected the number 5, 6, 7 or 8 at line 1 column 3");
}

#[test]
fn test_serde_stop_bits() {
	assert!(let Ok("1") = serde_json::to_string(&serial2::StopBits::One).as_deref());
	assert!(let Ok("2") = serde_json::to_string(&serial2::StopBits::Two).as_deref());

	assert!(let Ok(serial2::StopBits::One) = serde_json::from_str::<serial2::StopBits>("1"));
	assert!(let Ok(serial2::StopBits::Two) = serde_json::from_str::<serial2::StopBits>("2"));

	let_assert!(Err(e) = serde_json::from_str::<serial2::StopBits>("0"));
	assert!(e.to_string() == "invalid value: integer `0`, expected the number 1 or 2 at line 1 column 1");

	let_assert!(Err(e) = serde_json::from_str::<serial2::StopBits>("3"));
	assert!(e.to_string() == "invalid value: integer `3`, expected the number 1 or 2 at line 1 column 1");

	let_assert!(Err(e) = serde_json::from_str::<serial2::StopBits>("\"1\""));
	assert!(e.to_string() == "invalid type: string \"1\", expected the number 1 or 2 at line 1 column 3");
}

#[test]
fn test_serde_parity() {
	assert!(let Ok("\"none\"") = serde_json::to_string(&serial2::Parity::None).as_deref());
	assert!(let Ok("\"even\"") = serde_json::to_string(&serial2::Parity::Even).as_deref());
	assert!(let Ok("\"odd\"") = serde_json::to_string(&serial2::Parity::Odd).as_deref());

	assert!(let Ok(serial2::Parity::None) = serde_json::from_str::<serial2::Parity>("\"none\""));
	assert!(let Ok(serial2::Parity::Even) = serde_json::from_str::<serial2::Parity>("\"even\""));
	assert!(let Ok(serial2::Parity::Odd) = serde_json::from_str::<serial2::Parity>("\"odd\""));

	let_assert!(Err(e) = serde_json::from_str::<serial2::Parity>("\"even-then-odd\""));
	assert!(e.to_string() == "invalid value: string \"even-then-odd\", expected the string \"none\", \"odd\" or \"even\" at line 1 column 15");
}

#[test]
fn test_serde_flow_control() {
	assert!(let Ok("\"none\"") = serde_json::to_string(&serial2::FlowControl::None).as_deref());
	assert!(let Ok("\"xon/xoff\"") = serde_json::to_string(&serial2::FlowControl::XonXoff).as_deref());
	assert!(let Ok("\"rts/cts\"") = serde_json::to_string(&serial2::FlowControl::RtsCts).as_deref());

	assert!(let Ok(serial2::FlowControl::None) = serde_json::from_str::<serial2::FlowControl>("\"none\""));
	assert!(let Ok(serial2::FlowControl::XonXoff) = serde_json::from_str::<serial2::FlowControl>("\"xon/xoff\""));
	assert!(let Ok(serial2::FlowControl::RtsCts) = serde_json::from_str::<serial2::FlowControl>("\"rts/cts\""));

	let_assert!(Err(e) = serde_json::from_str::<serial2::FlowControl>("\"plug-in/plug-out\""));
	assert!(e.to_string() == "invalid value: string \"plug-in/plug-out\", expected the string \"none\", \"xon/xoff\" or \"rts/cts\" at line 1 column 18");
}
