#[cfg(feature = "unstable-locales")]
mod localized {
    use pure_rust_locales::{Locale, locale_match};

    pub(crate) const fn default_locale() -> Locale {
        Locale::POSIX
    }

    pub(crate) const fn short_months(locale: Locale) -> &'static [&'static str] {
        locale_match!(locale => LC_TIME::ABMON)
    }

    pub(crate) const fn long_months(locale: Locale) -> &'static [&'static str] {
        locale_match!(locale => LC_TIME::MON)
    }

    pub(crate) const fn short_weekdays(locale: Locale) -> &'static [&'static str] {
        locale_match!(locale => LC_TIME::ABDAY)
    }

    pub(crate) const fn long_weekdays(locale: Locale) -> &'static [&'static str] {
        locale_match!(locale => LC_TIME::DAY)
    }

    pub(crate) const fn am_pm(locale: Locale) -> &'static [&'static str] {
        locale_match!(locale => LC_TIME::AM_PM)
    }

    pub(crate) const fn decimal_point(locale: Locale) -> &'static str {
        locale_match!(locale => LC_NUMERIC::DECIMAL_POINT)
    }

    pub(crate) const fn d_fmt(locale: Locale) -> &'static str {
        locale_match!(locale => LC_TIME::D_FMT)
    }

    pub(crate) const fn d_t_fmt(locale: Locale) -> &'static str {
        locale_match!(locale => LC_TIME::D_T_FMT)
    }

    pub(crate) const fn t_fmt(locale: Locale) -> &'static str {
        locale_match!(locale => LC_TIME::T_FMT)
    }

    pub(crate) const fn t_fmt_ampm(locale: Locale) -> &'static str {
        locale_match!(locale => LC_TIME::T_FMT_AMPM)
    }
}

#[cfg(feature = "unstable-locales")]
pub(crate) use localized::*;
#[cfg(feature = "unstable-locales")]
pub use pure_rust_locales::Locale;

#[cfg(not(feature = "unstable-locales"))]
mod unlocalized {
    #[derive(Copy, Clone, Debug)]
    pub(crate) struct Locale;

    pub(crate) const fn default_locale() -> Locale {
        Locale
    }

    pub(crate) const fn short_months(_locale: Locale) -> &'static [&'static str] {
        &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    }

    pub(crate) const fn long_months(_locale: Locale) -> &'static [&'static str] {
        &[
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]
    }

    pub(crate) const fn short_weekdays(_locale: Locale) -> &'static [&'static str] {
        &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    }

    pub(crate) const fn long_weekdays(_locale: Locale) -> &'static [&'static str] {
        &["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]
    }

    pub(crate) const fn am_pm(_locale: Locale) -> &'static [&'static str] {
        &["AM", "PM"]
    }

    pub(crate) const fn decimal_point(_locale: Locale) -> &'static str {
        "."
    }
}

#[cfg(not(feature = "unstable-locales"))]
pub(crate) use unlocalized::*;
