fn main() {
    match codex_v8_poc::smoke_value() {
        Ok(3) => {}
        Ok(value) => {
            eprintln!("expected smoke value 3, got {value}");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("smoke evaluation failed: {err}");
            std::process::exit(1);
        }
    }
}
