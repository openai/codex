use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    // https://illumos.org/man/5/TIMEZONE
    // https://docs.oracle.com/cd/E23824_01/html/821-1473/uc-timezone-4.html

    let file = OpenOptions::new().read(true).open("/etc/default/init")?;
    let mut reader = BufReader::with_capacity(1536, file);
    let mut line = String::with_capacity(80);
    loop {
        line.clear();
        let count = reader.read_line(&mut line)?;
        if count == 0 {
            return Err(crate::GetTimezoneError::FailedParsingString);
        } else if line.starts_with("TZ=") {
            line.truncate(line.trim_end().len());
            line.replace_range(..3, "");
            return Ok(line);
        }
    }
}
