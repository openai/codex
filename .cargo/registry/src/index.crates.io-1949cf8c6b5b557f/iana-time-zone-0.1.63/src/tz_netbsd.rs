use std::fs::read_link;

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    // see https://www.cyberciti.biz/faq/openbsd-time-zone-howto/

    // This is a backport of the Linux implementation.
    // NetBSDs is less than thorough how the softlink should be set up.

    const PREFIXES: &[&str] = &[
        "/usr/share/zoneinfo/",   // absolute path
        "../usr/share/zoneinfo/", // relative path
    ];
    let mut s = read_link("/etc/localtime")?
        .into_os_string()
        .into_string()
        .map_err(|_| crate::GetTimezoneError::FailedParsingString)?;
    for &prefix in PREFIXES {
        if s.starts_with(prefix) {
            // Trim to the correct length without allocating.
            s.replace_range(..prefix.len(), "");
            return Ok(s);
        }
    }
    Err(crate::GetTimezoneError::FailedParsingString)
}
