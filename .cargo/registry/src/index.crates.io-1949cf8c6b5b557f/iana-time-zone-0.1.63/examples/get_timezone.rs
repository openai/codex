use iana_time_zone::{get_timezone, GetTimezoneError};

fn main() -> Result<(), GetTimezoneError> {
    println!("{}", get_timezone()?);
    Ok(())
}
