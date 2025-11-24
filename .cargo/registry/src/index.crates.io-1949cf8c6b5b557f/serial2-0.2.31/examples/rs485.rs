use serial2::rs4xx::Rs485Config;
use serial2::{SerialPort, Settings};
use std::time::Duration;

fn main() -> std::io::Result<()> {
	let port_name = "/dev/ttyS5";
	let serial_port = SerialPort::open(port_name, |mut settings: Settings| {
		settings.set_raw();
		settings.set_baud_rate(115200)?;
		Ok(settings)
	})?;

	let mut rs485_config = Rs485Config::new();
	rs485_config.set_bus_termination(true);
	rs485_config.set_full_duplex(true);
	serial_port.set_rs4xx_mode(rs485_config)?;

	loop {
		serial_port.write(b"test").unwrap();
		std::thread::sleep(Duration::from_millis(500));
	}
}
