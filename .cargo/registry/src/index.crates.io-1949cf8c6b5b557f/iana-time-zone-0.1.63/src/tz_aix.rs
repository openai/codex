use std::env;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    env::var("TZ").map_err(|_| crate::GetTimezoneError::OsError)
}
