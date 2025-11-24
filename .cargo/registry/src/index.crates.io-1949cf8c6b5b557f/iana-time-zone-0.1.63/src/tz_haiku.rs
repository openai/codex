pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    iana_time_zone_haiku::get_timezone().ok_or(crate::GetTimezoneError::OsError)
}
