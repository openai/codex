use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use fluent_bundle::FluentArgs;
use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use thiserror::Error;
use unic_langid::LanguageIdentifier;

const LOCALES_DIR: &str = "src/locales";
pub const DEFAULT_LOCALE: &str = "en";
const CANONICAL_LOCALE_CODES: &[&str] = &[
    DEFAULT_LOCALE,
    "am",
    "ar",
    "bg",
    "bn",
    "bs",
    "ca",
    "cs",
    "da",
    "de",
    "el",
    "es",
    "es-419",
    "et",
    "fa",
    "fi",
    "fr",
    "fr-CA",
    "gu",
    "hi",
    "hr",
    "hu",
    "hy",
    "id",
    "is",
    "it",
    "ja",
    "ka",
    "kk",
    "kn",
    "ko",
    "lt",
    "lv",
    "mk",
    "ml",
    "mn",
    "mr",
    "ms",
    "my",
    "nb",
    "nl",
    "pa",
    "pl",
    "pt",
    "pt-PT",
    "ro",
    "ru",
    "sk",
    "sl",
    "so",
    "sq",
    "sr",
    "sv",
    "sw",
    "ta",
    "te",
    "th",
    "tl",
    "tr",
    "uk",
    "ur",
    "vi",
    "zh",
    "zh-HK",
    "zh-Hant",
];

type Bundle = FluentBundle<FluentResource>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocaleError {
    #[error("invalid locale `{0}`")]
    InvalidLocale(String),
}

struct LocaleResources {
    supported_locales: Vec<String>,
    bundles: HashMap<String, Bundle>,
}

static LOCALE_RESOURCES: LazyLock<Option<LocaleResources>> =
    LazyLock::new(|| load_locale_resources().ok());

pub fn resolve_locale(config_locale: Option<&str>) -> Result<String, LocaleError> {
    resolve_locale_with_system(config_locale, sys_locale::get_locale().as_deref())
}

pub fn resolve_locale_with_system(
    config_locale: Option<&str>,
    system_locale: Option<&str>,
) -> Result<String, LocaleError> {
    if let Some(config_locale) = config_locale {
        let requested = parse_locale(config_locale)
            .ok_or_else(|| LocaleError::InvalidLocale(config_locale.trim().to_string()))?;
        return Ok(negotiate_locale(Some(&requested)));
    }

    let requested = system_locale.and_then(parse_locale);
    Ok(negotiate_locale(requested.as_ref()))
}

pub fn format_message(locale: &str, message_id: &str, args: &[(&str, &str)]) -> String {
    let locale = parse_locale(locale).as_ref().map_or_else(
        || DEFAULT_LOCALE.to_string(),
        |requested| negotiate_locale(Some(requested)),
    );
    let rendered = LOCALE_RESOURCES.as_ref().and_then(|locale_resources| {
        locale_resources
            .bundles
            .get(&locale)
            .and_then(|bundle| render_message(bundle, message_id, args))
            .or_else(|| {
                (locale != DEFAULT_LOCALE)
                    .then_some(())
                    .and_then(|()| locale_resources.bundles.get(DEFAULT_LOCALE))
                    .and_then(|bundle| render_message(bundle, message_id, args))
            })
    });

    rendered.unwrap_or_else(|| message_id.to_string())
}

fn parse_locale(value: &str) -> Option<LanguageIdentifier> {
    normalize_locale_candidate(value)?.parse().ok()
}

fn normalize_locale_candidate(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_modifier = trimmed.split('@').next()?;
    let without_encoding = without_modifier.split('.').next()?;
    let normalized = without_encoding.replace('_', "-");
    (!normalized.is_empty()).then_some(normalized)
}

fn negotiate_locale(requested: Option<&LanguageIdentifier>) -> String {
    let Some(requested) = requested else {
        return DEFAULT_LOCALE.to_string();
    };

    for candidate in canonical_locale_candidates(requested) {
        if supported_locales()
            .iter()
            .any(|locale| locale == &candidate)
        {
            return candidate;
        }
    }

    DEFAULT_LOCALE.to_string()
}

fn canonical_locale_candidates(requested: &LanguageIdentifier) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut push_candidate = |candidate: String| {
        if CANONICAL_LOCALE_CODES.contains(&candidate.as_str()) && !candidates.contains(&candidate)
        {
            candidates.push(candidate);
        }
    };

    push_candidate(requested.to_string());

    let language = requested.language.as_str();

    if let Some(region) = requested.region.as_ref() {
        push_candidate(format!("{language}-{region}"));
    }

    if let Some(script) = requested.script.as_ref() {
        push_candidate(format!("{language}-{script}"));
    }

    push_candidate(language.to_string());
    candidates
}

fn supported_locales() -> &'static [String] {
    LOCALE_RESOURCES
        .as_ref()
        .map(|locale_resources| locale_resources.supported_locales.as_slice())
        .unwrap_or(&[])
}

fn load_locale_resources() -> io::Result<LocaleResources> {
    let locales_dir = resolve_locales_dir()?;
    let mut supported_locales = Vec::new();
    let mut bundles = HashMap::new();

    for entry in sorted_dir_entries(&locales_dir)? {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let locale_name = entry.file_name().to_string_lossy().to_string();
        let Some(language_identifier) = parse_locale(&locale_name) else {
            continue;
        };
        if !CANONICAL_LOCALE_CODES.contains(&locale_name.as_str()) {
            continue;
        }

        let bundle = load_bundle(&entry.path(), &locale_name, language_identifier)?;
        supported_locales.push(locale_name.clone());
        bundles.insert(locale_name, bundle);
    }

    supported_locales.sort();

    Ok(LocaleResources {
        supported_locales,
        bundles,
    })
}

fn resolve_locales_dir() -> io::Result<PathBuf> {
    let runfile_path = codex_utils_cargo_bin::find_resource!(LOCALES_DIR);
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(LOCALES_DIR);

    match runfile_path {
        Ok(path) if path.exists() => Ok(path),
        Ok(_) if source_path.exists() => Ok(source_path),
        Ok(path) => Ok(path),
        Err(_) if source_path.exists() => Ok(source_path),
        Err(err) => Err(err),
    }
}

fn load_bundle(
    locale_dir: &Path,
    locale_name: &str,
    language_identifier: LanguageIdentifier,
) -> io::Result<Bundle> {
    let mut bundle = FluentBundle::new_concurrent(vec![language_identifier]);
    bundle.set_use_isolating(false);

    for entry in sorted_dir_entries(locale_dir)? {
        if !entry.file_type()?.is_file() || entry.path().extension() != Some(OsStr::new("ftl")) {
            continue;
        }

        let resource_path = entry.path();
        let contents = fs::read_to_string(&resource_path)?;
        let resource = FluentResource::try_new(contents).map_err(|(_, errors)| {
            io::Error::other(format!(
                "invalid Fluent resource `{}`: {errors:?}",
                resource_path.display()
            ))
        })?;
        if bundle.add_resource(resource).is_err() {
            return Err(io::Error::other(format!(
                "locale `{locale_name}` defines duplicate messages in `{}`",
                resource_path.display()
            )));
        }
    }

    Ok(bundle)
}

fn sorted_dir_entries(dir: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn render_message(bundle: &Bundle, message_id: &str, args: &[(&str, &str)]) -> Option<String> {
    let message = bundle.get_message(message_id)?;
    let pattern = message.value()?;
    let mut errors = Vec::new();
    let fluent_args = (!args.is_empty()).then(|| {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, *value);
        }
        fluent_args
    });

    Some(
        bundle
            .format_pattern(pattern, fluent_args.as_ref(), &mut errors)
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn explicit_locale_overrides_system_locale() {
        assert_eq!(
            resolve_locale_with_system(Some("zh-CN"), Some("en-US")).expect("resolve locale"),
            "zh"
        );
    }

    #[test]
    fn unsupported_explicit_locale_falls_back_to_default() {
        assert_eq!(
            resolve_locale_with_system(Some("zu-ZA"), Some("zh-CN")).expect("resolve locale"),
            DEFAULT_LOCALE
        );
    }

    #[test]
    fn supported_system_locale_is_used_when_config_is_missing() {
        assert_eq!(
            resolve_locale_with_system(None, Some("zh")).expect("resolve locale"),
            "zh"
        );
    }

    #[test]
    fn specific_locale_falls_back_to_generic_locale() {
        assert_eq!(
            resolve_locale_with_system(None, Some("zh-HK")).expect("resolve locale"),
            "zh"
        );
    }

    #[test]
    fn unsupported_system_locale_falls_back_to_default() {
        assert_eq!(
            resolve_locale_with_system(None, Some("zu-ZA")).expect("resolve locale"),
            DEFAULT_LOCALE
        );
    }

    #[test]
    fn invalid_explicit_locale_is_rejected() {
        assert_eq!(
            resolve_locale_with_system(Some("not a locale"), None),
            Err(LocaleError::InvalidLocale("not a locale".to_string()))
        );
    }

    #[test]
    fn formats_message_with_interpolation() {
        let rendered = format_message(
            "en",
            "approval-question-google-docs-create-document",
            &[("connector_name", "Google Docs")],
        );

        assert_eq!(rendered.contains("Google Docs"), true);
        assert_eq!(rendered.is_empty(), false);
    }

    #[test]
    fn different_locales_can_render_same_message() {
        let english = format_message(
            "en",
            "approval-question-google-docs-create-document",
            &[("connector_name", "Google Docs")],
        );
        let chinese = format_message(
            "zh-HK",
            "approval-question-google-docs-create-document",
            &[("connector_name", "Google Docs")],
        );

        assert_eq!(english.contains("Google Docs"), true);
        assert_eq!(chinese.contains("Google Docs"), true);
        assert_ne!(english, chinese);
    }

    #[test]
    fn format_message_falls_back_to_english_message() {
        let english = format_message("en", "approval-option-allow", &[]);

        assert_eq!(
            format_message("zu-ZA", "approval-option-allow", &[]),
            english
        );
    }

    #[test]
    fn format_message_returns_message_id_when_message_is_missing() {
        assert_eq!(
            format_message("zu-ZA", "approval-option-missing", &[]),
            "approval-option-missing".to_string()
        );
    }
}
