use std::fs::{read_link, read_to_string};

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    etc_localtime()
        .or_else(|_| etc_timezone())
        .or_else(|_| openwrt::etc_config_system())
}

fn etc_timezone() -> Result<String, crate::GetTimezoneError> {
    // see https://stackoverflow.com/a/12523283
    let mut contents = read_to_string("/etc/timezone")?;
    // Trim to the correct length without allocating.
    contents.truncate(contents.trim_end().len());
    Ok(contents)
}

fn etc_localtime() -> Result<String, crate::GetTimezoneError> {
    // Per <https://www.man7.org/linux/man-pages/man5/localtime.5.html>:
    // “ The /etc/localtime file configures the system-wide timezone of the local system that is
    //   used by applications for presentation to the user. It should be an absolute or relative
    //   symbolic link pointing to /usr/share/zoneinfo/, followed by a timezone identifier such as
    //   "Europe/Berlin" or "Etc/UTC". The resulting link should lead to the corresponding binary
    //   tzfile(5) timezone data for the configured timezone. ”

    // Systemd does not canonicalize the link, but only checks if it is prefixed by
    // "/usr/share/zoneinfo/" or "../usr/share/zoneinfo/". So we do the same.
    // <https://github.com/systemd/systemd/blob/9102c625a673a3246d7e73d8737f3494446bad4e/src/basic/time-util.c#L1493>

    const PREFIXES: &[&str] = &[
        "/usr/share/zoneinfo/",   // absolute path
        "../usr/share/zoneinfo/", // relative path
        "/etc/zoneinfo/",         // absolute path for NixOS
        "../etc/zoneinfo/",       // relative path for NixOS
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

mod openwrt {
    use std::io::BufRead;
    use std::{fs, io, iter};

    pub(crate) fn etc_config_system() -> Result<String, crate::GetTimezoneError> {
        let f = fs::OpenOptions::new()
            .read(true)
            .open("/etc/config/system")?;
        let mut f = io::BufReader::new(f);
        let mut in_system_section = false;
        let mut line = String::with_capacity(80);

        // prefer option "zonename" (IANA time zone) over option "timezone" (POSIX time zone)
        let mut timezone = None;
        loop {
            line.clear();
            f.read_line(&mut line)?;
            if line.is_empty() {
                break;
            }

            let mut iter = IterWords(&line);
            let mut next = || iter.next().transpose();

            if let Some(keyword) = next()? {
                if keyword == "config" {
                    in_system_section = next()? == Some("system") && next()?.is_none();
                } else if in_system_section && keyword == "option" {
                    if let Some(key) = next()? {
                        if key == "zonename" {
                            if let (Some(zonename), None) = (next()?, next()?) {
                                return Ok(zonename.to_owned());
                            }
                        } else if key == "timezone" {
                            if let (Some(value), None) = (next()?, next()?) {
                                timezone = Some(value.to_owned());
                            }
                        }
                    }
                }
            }
        }

        timezone.ok_or(crate::GetTimezoneError::OsError)
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    struct BrokenQuote;

    impl From<BrokenQuote> for crate::GetTimezoneError {
        fn from(_: BrokenQuote) -> Self {
            crate::GetTimezoneError::FailedParsingString
        }
    }

    /// Iterated over all words in a OpenWRT config line.
    struct IterWords<'a>(&'a str);

    impl<'a> Iterator for IterWords<'a> {
        type Item = Result<&'a str, BrokenQuote>;

        fn next(&mut self) -> Option<Self::Item> {
            match read_word(self.0) {
                Ok(Some((item, tail))) => {
                    self.0 = tail;
                    Some(Ok(item))
                }
                Ok(None) => {
                    self.0 = "";
                    None
                }
                Err(err) => {
                    self.0 = "";
                    Some(Err(err))
                }
            }
        }
    }

    impl iter::FusedIterator for IterWords<'_> {}

    /// Read the next word in a OpenWRT config line. Strip any surrounding quotation marks.
    ///
    /// Returns
    ///
    ///  * a tuple `Some((word, remaining_line))` if found,
    ///  * `None` if the line is exhausted, or
    ///  * `Err(BrokenQuote)` if the line could not be parsed.
    #[allow(clippy::manual_strip)] // needs to be compatile to 1.36
    fn read_word(s: &str) -> Result<Option<(&str, &str)>, BrokenQuote> {
        let s = s.trim_start();
        if s.is_empty() || s.starts_with('#') {
            Ok(None)
        } else if s.starts_with('\'') {
            let mut iter = s[1..].splitn(2, '\'');
            match (iter.next(), iter.next()) {
                (Some(item), Some(tail)) => Ok(Some((item, tail))),
                _ => Err(BrokenQuote),
            }
        } else if s.starts_with('"') {
            let mut iter = s[1..].splitn(2, '"');
            match (iter.next(), iter.next()) {
                (Some(item), Some(tail)) => Ok(Some((item, tail))),
                _ => Err(BrokenQuote),
            }
        } else {
            let mut iter = s.splitn(2, |c: char| c.is_whitespace());
            match (iter.next(), iter.next()) {
                (Some(item), Some(tail)) => Ok(Some((item, tail))),
                _ => Ok(Some((s, ""))),
            }
        }
    }

    #[cfg(test)]
    #[test]
    fn test_read_word() {
        assert_eq!(
            read_word("       option timezone 'CST-8'\n").unwrap(),
            Some(("option", "timezone 'CST-8'\n")),
        );
        assert_eq!(
            read_word("timezone 'CST-8'\n").unwrap(),
            Some(("timezone", "'CST-8'\n")),
        );
        assert_eq!(read_word("'CST-8'\n").unwrap(), Some(("CST-8", "\n")));
        assert_eq!(read_word("\n").unwrap(), None);

        assert_eq!(
            read_word(r#""time 'Zone'""#).unwrap(),
            Some(("time 'Zone'", "")),
        );

        assert_eq!(read_word("'CST-8").unwrap_err(), BrokenQuote);
    }
}
