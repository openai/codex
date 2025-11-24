pub fn parse_cmd_args() -> Option<u16> {
    let mut sleep_ms: Option<u16> = None;
    let mut skip_next = false;

    for (i, arg) in std::env::args().skip(1).enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        match &arg[..] {
            "-s" | "--sleep" => {
                sleep_ms = std::env::args()
                    .nth(i + 2) // next is +2 because .skip(1)
                    .unwrap_or(String::from("5000u16"))
                    .parse::<u16>()
                    .ok()
                    .and_then(|parsed| {
                        skip_next = true;
                        Some(parsed)
                    });
            }
            _ => {}
        }
    }

    sleep_ms
}

pub fn sleep(sleep_ms: Option<u16>) {
    if let Some(sleep_ms) = sleep_ms {
        let sleep_ms = std::time::Duration::from_millis(sleep_ms as u64);
        std::thread::sleep(sleep_ms);
    }
}
