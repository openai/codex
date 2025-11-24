pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    // see https://gitlab.gnome.org/GNOME/evolution-data-server/-/issues/19
    let mut contents = std::fs::read_to_string("/var/db/zoneinfo")?;
    // Trim to the correct length without allocating.
    contents.truncate(contents.trim_end().len());
    Ok(contents)
}
