use std::{env, ffi::OsStr};

const LANGUAGE: &str = "LANGUAGE";
const LC_ALL: &str = "LC_ALL";
const LC_MESSAGES: &str = "LC_MESSAGES";
const LANG: &str = "LANG";

/// Environment variable access abstraction to allow testing without
/// mutating env variables.
///
/// Use [StdEnv] to query [std::env]
trait EnvAccess {
    /// See also [std::env::var]
    fn get(&self, key: impl AsRef<OsStr>) -> Option<String>;
}

/// Proxy to [std::env]
struct StdEnv;
impl EnvAccess for StdEnv {
    fn get(&self, key: impl AsRef<OsStr>) -> Option<String> {
        env::var(key).ok()
    }
}

pub(crate) fn get() -> impl Iterator<Item = String> {
    _get(&StdEnv)
}

/// Retrieves a list of unique locales by checking specific environment variables
/// in a predefined order: LANGUAGE, LC_ALL, LC_MESSAGES, and LANG.
///
/// The function first checks the `LANGUAGE` environment variable, which can contain
/// one or more locales separated by a colon (`:`). It then splits these values,
/// converts them from [POSIX](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap08.html)
/// to [BCP 47](https://www.ietf.org/rfc/bcp/bcp47.html) format, and adds them to the list of locales
/// if they are not already included.
///
/// Next, the function checks the `LC_ALL`, `LC_MESSAGES`, and `LANG` environment
/// variables. Each of these variables contains a single locale. If a locale is found,
/// and it's not empty, it is converted to BCP 47 format and added to the list if
/// it is not already included.
///
/// For more information check this issue: https://github.com/1Password/sys-locale/issues/14.
///
/// The function ensures that locales are returned in the order of precedence
/// and without duplicates. The final list of locales is returned as an iterator.
///
/// # Returns
///
/// An iterator over the unique locales found in the environment variables.
///
/// # Environment Variables Checked
///
/// 1. `LANGUAGE` - Can contain multiple locales, each separated by a colon (`:`), highest priority.
/// 2. `LC_ALL` - Contains a single locale, high priority.
/// 3. `LC_MESSAGES` - Contains a single locale, medium priority.
/// 4. `LANG` - Contains a single locale, low priority.
///
/// # Example
///
/// ```ignore
/// let locales: Vec<String> = _get(&env).collect();
/// for locale in locales {
///     println!("User's preferred locales: {}", locale);
/// }
/// ```
fn _get(env: &impl EnvAccess) -> impl Iterator<Item = String> {
    let mut locales = Vec::new();

    // LANGUAGE contains one or multiple locales separated by colon (':')
    if let Some(val) = env.get(LANGUAGE).filter(|val| !val.is_empty()) {
        for part in val.split(':') {
            let locale = posix_to_bcp47(part);
            if !locales.contains(&locale) {
                locales.push(locale);
            }
        }
    }

    // LC_ALL, LC_MESSAGES and LANG contain one locale
    for variable in [LC_ALL, LC_MESSAGES, LANG] {
        if let Some(val) = env.get(variable).filter(|val| !val.is_empty()) {
            let locale = posix_to_bcp47(&val);
            if !locales.contains(&locale) {
                locales.push(locale);
            }
        }
    }

    locales.into_iter()
}

/// Converts a POSIX locale string to a BCP 47 locale string.
///
/// This function processes the input `code` by removing any character encoding
/// (the part after the `.` character) and any modifiers (the part after the `@` character).
/// It replaces underscores (`_`) with hyphens (`-`) to conform to BCP 47 formatting.
///
/// If the locale is already in the BCP 47 format, no changes are made.
///
/// Useful links:
/// - [The Open Group Base Specifications Issue 8 - 7. Locale](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap07.html)
/// - [The Open Group Base Specifications Issue 8 - 8. Environment Variables](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap08.html)
/// - [BCP 47 specification](https://www.ietf.org/rfc/bcp/bcp47.html)
///
/// # Examples
///
/// ```ignore
/// let bcp47 = posix_to_bcp47("en-US"); // already BCP 47
/// assert_eq!(bcp47, "en-US"); // no changes
///
/// let bcp47 = posix_to_bcp47("en_US");
/// assert_eq!(bcp47, "en-US");
///
/// let bcp47 = posix_to_bcp47("ru_RU.UTF-8");
/// assert_eq!(bcp47, "ru-RU");
///
/// let bcp47 = posix_to_bcp47("fr_FR@dict");
/// assert_eq!(bcp47, "fr-FR");
///
/// let bcp47 = posix_to_bcp47("de_DE.UTF-8@euro");
/// assert_eq!(bcp47, "de-DE");
/// ```
///
/// # TODO
///
/// 1. Implement POSIX to BCP 47 modifier conversion (see https://github.com/1Password/sys-locale/issues/32).
/// 2. Optimize to avoid creating a new buffer (see https://github.com/1Password/sys-locale/pull/33).
fn posix_to_bcp47(locale: &str) -> String {
    locale
        .chars()
        .take_while(|&c| c != '.' && c != '@')
        .map(|c| if c == '_' { '-' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{EnvAccess, _get, posix_to_bcp47, LANG, LANGUAGE, LC_ALL, LC_MESSAGES};
    use std::{
        collections::HashMap,
        ffi::{OsStr, OsString},
    };

    type MockEnv = HashMap<OsString, String>;
    impl EnvAccess for MockEnv {
        fn get(&self, key: impl AsRef<OsStr>) -> Option<String> {
            self.get(key.as_ref()).cloned()
        }
    }

    const BCP_47: &str = "fr-FR";
    const POSIX: &str = "fr_FR";
    const POSIX_ENC: &str = "fr_FR.UTF-8";
    const POSIX_MOD: &str = "fr_FR@euro";
    const POSIX_ENC_MOD: &str = "fr_FR.UTF-8@euro";

    #[test]
    fn parse_identifier() {
        assert_eq!(posix_to_bcp47(BCP_47), BCP_47);
        assert_eq!(posix_to_bcp47(POSIX), BCP_47);
        assert_eq!(posix_to_bcp47(POSIX_ENC), BCP_47);
        assert_eq!(posix_to_bcp47(POSIX_MOD), BCP_47);
        assert_eq!(posix_to_bcp47(POSIX_ENC_MOD), BCP_47);
    }

    #[test]
    fn env_get() {
        fn case(
            env: &mut MockEnv,
            language: impl Into<String>,
            lc_all: impl Into<String>,
            lc_messages: impl Into<String>,
            lang: impl Into<String>,
            expected: impl IntoIterator<Item = impl Into<String>>,
        ) {
            env.insert(LANGUAGE.into(), language.into());
            env.insert(LC_ALL.into(), lc_all.into());
            env.insert(LC_MESSAGES.into(), lc_messages.into());
            env.insert(LANG.into(), lang.into());
            assert!(_get(env).eq(expected.into_iter().map(|s| s.into())));
        }

        let mut env = MockEnv::new();
        assert_eq!(_get(&env).next(), None);

        // Empty
        case(&mut env, "", "", "", "", &[] as &[String]);

        // Constants
        case(
            &mut env,
            POSIX_ENC_MOD,
            POSIX_ENC,
            POSIX_MOD,
            POSIX,
            [BCP_47],
        );

        // Only one variable
        case(&mut env, "en_US", "", "", "", ["en-US"]);
        case(&mut env, "", "en_US", "", "", ["en-US"]);
        case(&mut env, "", "", "en_US", "", ["en-US"]);
        case(&mut env, "", "", "", "en_US", ["en-US"]);

        // Duplicates
        case(&mut env, "en_US", "en_US", "en_US", "en_US", ["en-US"]);
        case(
            &mut env,
            "en_US",
            "en_US",
            "ru_RU",
            "en_US",
            ["en-US", "ru-RU"],
        );
        case(
            &mut env,
            "en_US",
            "ru_RU",
            "ru_RU",
            "en_US",
            ["en-US", "ru-RU"],
        );
        case(
            &mut env,
            "en_US",
            "es_ES",
            "ru_RU",
            "en_US",
            ["en-US", "es-ES", "ru-RU"],
        );
        case(
            &mut env,
            "en_US:ru_RU:es_ES:en_US",
            "es_ES",
            "ru_RU",
            "en_US",
            ["en-US", "ru-RU", "es-ES"],
        );

        // Duplicates with different case
        case(
            &mut env,
            "en_US:fr_fr",
            "EN_US",
            "fR_Fr",
            "En_US",
            ["en-US", "fr-fr", "EN-US", "fR-Fr", "En-US"],
        );

        // More complicated cases
        case(
            &mut env,
            "ru_RU:ru:en_US:en",
            "ru_RU.UTF-8",
            "ru_RU.UTF-8",
            "ru_RU.UTF-8",
            ["ru-RU", "ru", "en-US", "en"],
        );
        case(
            &mut env,
            "fr_FR.UTF-8@euro:fr_FR.UTF-8:fr_FR:fr:en_US.UTF-8:en_US:en",
            "es_ES.UTF-8@euro",
            "fr_FR.UTF-8@euro",
            "fr_FR.UTF-8@euro",
            ["fr-FR", "fr", "en-US", "en", "es-ES"],
        );
        case(
            &mut env,
            "",
            "es_ES.UTF-8@euro",
            "fr_FR.UTF-8@euro",
            "fr_FR.UTF-8@euro",
            ["es-ES", "fr-FR"],
        );
        case(
            &mut env,
            "fr_FR@euro",
            "fr_FR.UTF-8",
            "en_US.UTF-8",
            "en_US.UTF-8@dict",
            ["fr-FR", "en-US"],
        );

        // Already BCP 47
        case(&mut env, BCP_47, BCP_47, BCP_47, POSIX, [BCP_47]);
        case(
            &mut env,
            "fr-FR",
            "es-ES",
            "de-DE",
            "en-US",
            ["fr-FR", "es-ES", "de-DE", "en-US"],
        );
    }
}
