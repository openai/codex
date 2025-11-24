use std::fmt;

/// Release date including year, month, and day.
// Internal storage is: y[31..9] | m[8..5] | d[5...0].
#[derive(Debug, PartialEq, Eq, Copy, Clone, PartialOrd, Ord)]
pub struct Date(u32);

impl Date {
    /// Reads the release date of the running compiler. If it cannot be
    /// determined (see the [top-level documentation](crate)), returns `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Date;
    ///
    /// match Date::read() {
    ///     Some(d) => format!("The release date is: {}", d),
    ///     None => format!("Failed to read the release date.")
    /// };
    /// ```
    pub fn read() -> Option<Date> {
        ::get_version_and_date()
            .and_then(|(_, date)| date)
            .and_then(|date| Date::parse(&date))
    }

    /// Parse a release date of the form `%Y-%m-%d`. Returns `None` if `date` is
    /// not in `%Y-%m-%d` format.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Date;
    ///
    /// let date = Date::parse("2016-04-20").unwrap();
    ///
    /// assert!(date.at_least("2016-01-10"));
    /// assert!(date.at_most("2016-04-20"));
    /// assert!(date.exactly("2016-04-20"));
    ///
    /// assert!(Date::parse("2021-12-31").unwrap().exactly("2021-12-31"));
    ///
    /// assert!(Date::parse("March 13, 2018").is_none());
    /// assert!(Date::parse("1-2-3-4-5").is_none());
    /// assert!(Date::parse("2020-300-23120").is_none());
    /// assert!(Date::parse("2020-12-12 1").is_none());
    /// assert!(Date::parse("2020-10").is_none());
    /// assert!(Date::parse("2020").is_none());
    /// ```
    pub fn parse(date: &str) -> Option<Date> {
        let mut ymd = [0u16; 3];
        for (i, split) in date.split('-').map(|s| s.parse::<u16>()).enumerate() {
            ymd[i] = match (i, split) {
                (3, _) | (_, Err(_)) => return None,
                (_, Ok(v)) => v,
            };
        }

        let (year, month, day) = (ymd[0], ymd[1], ymd[2]);
        if year == 0 || month == 0 || month > 12 || day == 0 || day > 31 {
            return None;
        }

        Some(Date::from_ymd(year, month as u8, day as u8))
    }

    /// Creates a `Date` from `(year, month, day)` date components.
    ///
    /// Does not check the validity of `year`, `month`, or `day`, but `year` is
    /// truncated to 23 bits (% 8,388,608), `month` to 4 bits (% 16), and `day`
    /// to 5 bits (% 32).
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Date;
    ///
    /// assert!(Date::from_ymd(2021, 7, 30).exactly("2021-07-30"));
    /// assert!(Date::from_ymd(2010, 3, 23).exactly("2010-03-23"));
    /// assert!(Date::from_ymd(2090, 1, 31).exactly("2090-01-31"));
    ///
    /// // Truncation: 33 % 32 == 0x21 & 0x1F == 1.
    /// assert!(Date::from_ymd(2090, 1, 33).exactly("2090-01-01"));
    /// ```
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Date {
        let year = (year as u32) << 9;
        let month = ((month as u32) & 0xF) << 5;
        let day = (day as u32) & 0x1F;
        Date(year | month | day)
    }

    /// Return the original (YYYY, MM, DD).
    fn to_ymd(&self) -> (u16, u8, u8) {
        let y = self.0 >> 9;
        let m = (self.0 >> 5) & 0xF;
        let d = self.0 & 0x1F;
        (y as u16, m as u8, d as u8)
    }

    /// Returns `true` if `self` occurs on or after `date`.
    ///
    /// If `date` occurs before `self`, or if `date` is not in `%Y-%m-%d`
    /// format, returns `false`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Date;
    ///
    /// let date = Date::parse("2020-01-01").unwrap();
    ///
    /// assert!(date.at_least("2019-12-31"));
    /// assert!(date.at_least("2020-01-01"));
    /// assert!(date.at_least("2014-04-31"));
    ///
    /// assert!(!date.at_least("2020-01-02"));
    /// assert!(!date.at_least("2024-08-18"));
    /// ```
    pub fn at_least(&self, date: &str) -> bool {
        Date::parse(date)
            .map(|date| self >= &date)
            .unwrap_or(false)
    }

    /// Returns `true` if `self` occurs on or before `date`.
    ///
    /// If `date` occurs after `self`, or if `date` is not in `%Y-%m-%d`
    /// format, returns `false`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Date;
    ///
    /// let date = Date::parse("2020-01-01").unwrap();
    ///
    /// assert!(date.at_most("2020-01-01"));
    /// assert!(date.at_most("2020-01-02"));
    /// assert!(date.at_most("2024-08-18"));
    ///
    /// assert!(!date.at_most("2019-12-31"));
    /// assert!(!date.at_most("2014-04-31"));
    /// ```
    pub fn at_most(&self, date: &str) -> bool {
        Date::parse(date)
            .map(|date| self <= &date)
            .unwrap_or(false)
    }

    /// Returns `true` if `self` occurs exactly on `date`.
    ///
    /// If `date` is not exactly `self`, or if `date` is not in `%Y-%m-%d`
    /// format, returns `false`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Date;
    ///
    /// let date = Date::parse("2020-01-01").unwrap();
    ///
    /// assert!(date.exactly("2020-01-01"));
    ///
    /// assert!(!date.exactly("2019-12-31"));
    /// assert!(!date.exactly("2014-04-31"));
    /// assert!(!date.exactly("2020-01-02"));
    /// assert!(!date.exactly("2024-08-18"));
    /// ```
    pub fn exactly(&self, date: &str) -> bool {
        Date::parse(date)
            .map(|date| self == &date)
            .unwrap_or(false)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (y, m, d) = self.to_ymd();
        write!(f, "{}-{:02}-{:02}", y, m, d)
    }
}

#[cfg(test)]
mod tests {
    use super::Date;

    macro_rules! reflexive_display {
        ($string:expr) => (
            assert_eq!(Date::parse($string).unwrap().to_string(), $string);
        )
    }

    #[test]
    fn display() {
        reflexive_display!("2019-05-08");
        reflexive_display!("2000-01-01");
        reflexive_display!("2000-12-31");
        reflexive_display!("2090-12-31");
        reflexive_display!("1999-02-19");
        reflexive_display!("9999-12-31");
    }
}
