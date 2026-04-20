use codex_features::FEATURES;
use codex_protocol::account::PlanType;
use lazy_static::lazy_static;
use rand::Rng;

const ANNOUNCEMENT_TIP_URL: &str =
    "https://raw.githubusercontent.com/openai/codex/main/announcement_tip.toml";

const IS_MACOS: bool = cfg!(target_os = "macos");
const IS_WINDOWS: bool = cfg!(target_os = "windows");

const APP_TOOLTIP: &str = "Try the **Codex App**. Run 'codex app' or visit https://chatgpt.com/codex?app-landing-page=true";
const FAST_TOOLTIP: &str =
    "*New* Use **/fast** to enable our fastest inference with increased plan usage.";
const OTHER_TOOLTIP: &str = "*New* Build faster with the **Codex App**. Run 'codex app' or visit https://chatgpt.com/codex?app-landing-page=true";
const OTHER_TOOLTIP_NON_MAC: &str = "*New* Build faster with Codex.";
const FREE_GO_TOOLTIP: &str =
    "*New* For a limited time, Codex is included in your plan for free – let’s build together.";

const RAW_TOOLTIPS: &str = include_str!("../tooltips.txt");

lazy_static! {
    static ref TOOLTIPS: Vec<&'static str> = RAW_TOOLTIPS
        .lines()
        .map(str::trim)
        .filter(|line| {
            if line.is_empty() || line.starts_with('#') {
                return false;
            }
            if !IS_MACOS && !IS_WINDOWS && line.contains("codex app") {
                return false;
            }
            true
        })
        .collect();
    static ref ALL_TOOLTIPS: Vec<&'static str> = {
        let mut tips = Vec::new();
        tips.extend(TOOLTIPS.iter().copied());
        tips.extend(experimental_tooltips());
        tips
    };
}

fn experimental_tooltips() -> Vec<&'static str> {
    FEATURES
        .iter()
        .filter_map(|spec| spec.stage.experimental_announcement())
        .collect()
}

/// Pick a random tooltip to show to the user when starting Codex.
pub(crate) fn get_tooltip(plan: Option<PlanType>, fast_mode_enabled: bool) -> Option<String> {
    let mut rng = rand::rng();

    if let Some(announcement) = announcement::fetch_announcement_tip(plan) {
        return Some(announcement);
    }

    // Leave small chance for a random tooltip to be shown.
    if rng.random_ratio(8, 10) {
        match plan {
            Some(plan_type)
                if matches!(
                    plan_type,
                    PlanType::Plus | PlanType::Enterprise | PlanType::Pro | PlanType::ProLite
                ) || plan_type.is_team_like()
                    || plan_type.is_business_like() =>
            {
                if let Some(tooltip) = pick_paid_tooltip(&mut rng, fast_mode_enabled) {
                    return Some(tooltip.to_string());
                }
            }
            Some(PlanType::Go) | Some(PlanType::Free) => {
                return Some(FREE_GO_TOOLTIP.to_string());
            }
            _ => {
                let tooltip = if IS_MACOS {
                    OTHER_TOOLTIP
                } else {
                    OTHER_TOOLTIP_NON_MAC
                };
                return Some(tooltip.to_string());
            }
        }
    }

    pick_tooltip(&mut rng).map(str::to_string)
}

fn paid_app_tooltip() -> Option<&'static str> {
    if IS_MACOS || IS_WINDOWS {
        Some(APP_TOOLTIP)
    } else {
        None
    }
}

/// Paid users spend most startup sessions in a dedicated promo slot rather than the
/// generic random tip pool. Keep this business logic explicit: we currently split
/// that slot between the app promo and Fast mode, but suppress the Fast promo once
/// the user already has Fast mode enabled.
fn pick_paid_tooltip<R: Rng + ?Sized>(
    rng: &mut R,
    fast_mode_enabled: bool,
) -> Option<&'static str> {
    if fast_mode_enabled || rng.random_bool(0.5) {
        paid_app_tooltip()
    } else {
        Some(FAST_TOOLTIP)
    }
}

fn pick_tooltip<R: Rng + ?Sized>(rng: &mut R) -> Option<&'static str> {
    if ALL_TOOLTIPS.is_empty() {
        None
    } else {
        ALL_TOOLTIPS
            .get(rng.random_range(0..ALL_TOOLTIPS.len()))
            .copied()
    }
}

pub(crate) mod announcement {
    use crate::tooltips::ANNOUNCEMENT_TIP_URL;
    use crate::version::CODEX_CLI_VERSION;
    use chrono::NaiveDate;
    use chrono::Utc;
    use codex_protocol::account::PlanType;
    use regex_lite::Regex;
    use serde::Deserialize;
    use std::sync::OnceLock;
    use std::thread;
    use std::time::Duration;

    static ANNOUNCEMENT_TIP: OnceLock<Option<String>> = OnceLock::new();
    const CURRENT_OS: TargetOs = TargetOs::current();

    /// Prewarm the cache of the announcement tip.
    pub(crate) fn prewarm() {
        let _ = thread::spawn(|| ANNOUNCEMENT_TIP.get_or_init(init_announcement_tip_in_thread));
    }

    /// Fetch the announcement tip, return None if the prewarm is not done yet.
    pub(crate) fn fetch_announcement_tip(plan: Option<PlanType>) -> Option<String> {
        ANNOUNCEMENT_TIP
            .get()
            .cloned()
            .flatten()
            .and_then(|raw| parse_announcement_tip_toml(&raw, plan))
    }

    #[derive(Debug, Deserialize)]
    struct AnnouncementTipRaw {
        content: String,
        from_date: Option<String>,
        to_date: Option<String>,
        version_regex: Option<String>,
        target_app: Option<String>,
        target_plan_types: Option<Vec<PlanType>>,
        target_oses: Option<Vec<TargetOs>>,
    }

    #[derive(Debug, Deserialize)]
    struct AnnouncementTipDocument {
        announcements: Vec<AnnouncementTipRaw>,
    }

    #[derive(Debug)]
    struct AnnouncementTip {
        content: String,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
        version_regex: Option<Regex>,
        target_app: String,
        target_plan_types: Option<Vec<PlanType>>,
        target_oses: Option<Vec<TargetOs>>,
    }

    #[derive(Debug, Deserialize, Copy, Clone, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    enum TargetOs {
        Linux,
        Macos,
        Windows,
        #[serde(other)]
        Unknown,
    }

    impl TargetOs {
        const fn current() -> Self {
            if cfg!(target_os = "macos") {
                Self::Macos
            } else if cfg!(target_os = "windows") {
                Self::Windows
            } else {
                // Codex currently publishes CLI builds for macOS, Windows, and Linux.
                Self::Linux
            }
        }
    }

    fn init_announcement_tip_in_thread() -> Option<String> {
        thread::spawn(blocking_init_announcement_tip)
            .join()
            .ok()
            .flatten()
    }

    fn blocking_init_announcement_tip() -> Option<String> {
        // Avoid system proxy detection to prevent macOS system-configuration panics (#8912).
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .ok()?;
        let response = client
            .get(ANNOUNCEMENT_TIP_URL)
            .timeout(Duration::from_millis(2000))
            .send()
            .ok()?;
        response.error_for_status().ok()?.text().ok()
    }

    pub(crate) fn parse_announcement_tip_toml(
        text: &str,
        plan: Option<PlanType>,
    ) -> Option<String> {
        let announcements = toml::from_str::<AnnouncementTipDocument>(text)
            .map(|doc| doc.announcements)
            .or_else(|_| toml::from_str::<Vec<AnnouncementTipRaw>>(text))
            .ok()?;

        let mut latest_match = None;
        let today = Utc::now().date_naive();
        for raw in announcements {
            let Some(tip) = AnnouncementTip::from_raw(raw) else {
                continue;
            };
            let plan_matches = tip
                .target_plan_types
                .as_ref()
                .is_none_or(|target_plans| plan.is_some_and(|plan| target_plans.contains(&plan)));
            let os_matches = tip
                .target_oses
                .as_ref()
                .is_none_or(|target_oses| target_oses.contains(&CURRENT_OS));
            if tip.version_matches(CODEX_CLI_VERSION)
                && tip.date_matches(today)
                && tip.target_app == "cli"
                && plan_matches
                && os_matches
            {
                latest_match = Some(tip.content);
            }
        }
        latest_match
    }

    impl AnnouncementTip {
        fn from_raw(raw: AnnouncementTipRaw) -> Option<Self> {
            let content = raw.content.trim();
            if content.is_empty() {
                return None;
            }

            let from_date = match raw.from_date {
                Some(date) => Some(NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?),
                None => None,
            };
            let to_date = match raw.to_date {
                Some(date) => Some(NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?),
                None => None,
            };
            let version_regex = match raw.version_regex {
                Some(pattern) => Some(Regex::new(&pattern).ok()?),
                None => None,
            };
            let target_plan_types = raw.target_plan_types;
            if target_plan_types
                .as_ref()
                .is_some_and(|plans| plans.contains(&PlanType::Unknown))
            {
                return None;
            }
            let target_oses = raw.target_oses;
            if target_oses
                .as_ref()
                .is_some_and(|oses| oses.contains(&TargetOs::Unknown))
            {
                return None;
            }

            Some(Self {
                content: content.to_string(),
                from_date,
                to_date,
                version_regex,
                target_app: raw.target_app.unwrap_or("cli".to_string()).to_lowercase(),
                target_plan_types,
                target_oses,
            })
        }

        fn version_matches(&self, version: &str) -> bool {
            self.version_regex
                .as_ref()
                .is_none_or(|regex| regex.is_match(version))
        }

        fn date_matches(&self, today: NaiveDate) -> bool {
            if let Some(from) = self.from_date
                && today < from
            {
                return false;
            }
            if let Some(to) = self.to_date
                && today >= to
            {
                return false;
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tooltips::announcement::parse_announcement_tip_toml;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn random_tooltip_returns_some_tip_when_available() {
        let mut rng = StdRng::seed_from_u64(42);
        assert!(pick_tooltip(&mut rng).is_some());
    }

    #[test]
    fn random_tooltip_is_reproducible_with_seed() {
        let expected = {
            let mut rng = StdRng::seed_from_u64(7);
            pick_tooltip(&mut rng)
        };

        let mut rng = StdRng::seed_from_u64(7);
        assert_eq!(expected, pick_tooltip(&mut rng));
    }

    #[test]
    fn paid_tooltip_pool_rotates_between_promos() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..32 {
            let mut rng = StdRng::seed_from_u64(seed);
            seen.insert(pick_paid_tooltip(
                &mut rng, /*fast_mode_enabled*/ false,
            ));
        }

        let expected = std::collections::BTreeSet::from([paid_app_tooltip(), Some(FAST_TOOLTIP)]);
        assert_eq!(seen, expected);
    }

    #[test]
    fn paid_tooltip_pool_skips_fast_when_fast_mode_is_enabled() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..8 {
            let mut rng = StdRng::seed_from_u64(seed);
            seen.insert(pick_paid_tooltip(&mut rng, /*fast_mode_enabled*/ true));
        }

        let expected = std::collections::BTreeSet::from([paid_app_tooltip()]);
        assert_eq!(seen, expected);
        assert!(!seen.contains(&Some(FAST_TOOLTIP)));
    }

    #[test]
    fn announcement_tip_toml_picks_last_matching() {
        let toml = r#"
[[announcements]]
content = "first"
from_date = "2000-01-01"

[[announcements]]
content = "latest match"
version_regex = ".*"
target_app = "cli"

[[announcements]]
content = "should not match"
to_date = "2000-01-01"
        "#;

        assert_eq!(
            Some("latest match".to_string()),
            parse_announcement_tip_toml(toml, /*plan*/ None)
        );

        let toml = r#"
[[announcements]]
content = "first"
from_date = "2000-01-01"
target_app = "cli"

[[announcements]]
content = "latest match"
version_regex = ".*"

[[announcements]]
content = "should not match"
to_date = "2000-01-01"
        "#;

        assert_eq!(
            Some("latest match".to_string()),
            parse_announcement_tip_toml(toml, /*plan*/ None)
        );
    }

    #[test]
    fn announcement_tip_toml_picks_no_match() {
        let toml = r#"
[[announcements]]
content = "first"
from_date = "2000-01-01"
to_date = "2000-01-05"

[[announcements]]
content = "latest match"
version_regex = "invalid_version_name"

[[announcements]]
content = "should not match either "
target_app = "vsce"
        "#;

        assert_eq!(None, parse_announcement_tip_toml(toml, /*plan*/ None));
    }

    #[test]
    fn announcement_tip_toml_bad_deserialization() {
        let toml = r#"
[[announcements]]
content = 123
from_date = "2000-01-01"
        "#;

        assert_eq!(None, parse_announcement_tip_toml(toml, /*plan*/ None));
    }

    #[test]
    fn announcement_tip_toml_parse_comments() {
        let toml = r#"
# Example announcement tips for Codex TUI.
# Each [[announcements]] entry is evaluated in order; the last matching one is shown.
# Dates are UTC, formatted as YYYY-MM-DD. The from_date is inclusive and the to_date is exclusive.
# version_regex matches against the CLI version (env!("CARGO_PKG_VERSION")); omit to apply to all versions.
# target_app specify which app should display the announcement (cli, vsce, ...).
# target_plan_types optionally restricts the announcement to plan types like ["plus", "pro"].
# target_oses optionally restricts the announcement to operating systems like ["macos", "windows"].

[[announcements]]
content = "Welcome to Codex! Check out the new onboarding flow."
from_date = "2024-10-01"
to_date = "2024-10-15"
target_app = "cli"
version_regex = "^0\\.0\\.0$"

[[announcements]]
content = "This is a test announcement"
        "#;

        assert_eq!(
            Some("This is a test announcement".to_string()),
            parse_announcement_tip_toml(toml, /*plan*/ None)
        );
    }

    #[test]
    fn announcement_tip_toml_matches_target_plan_type() {
        let toml = r#"
[[announcements]]
content = "all plans"

[[announcements]]
content = "pro announcement"
target_plan_types = ["pro", "enterprise"]

[[announcements]]
content = "free announcement"
target_plan_types = ["free"]
        "#;

        assert_eq!(
            Some("pro announcement".to_string()),
            parse_announcement_tip_toml(toml, Some(PlanType::Pro))
        );
        assert_eq!(
            Some("free announcement".to_string()),
            parse_announcement_tip_toml(toml, Some(PlanType::Free))
        );
        assert_eq!(
            Some("all plans".to_string()),
            parse_announcement_tip_toml(toml, Some(PlanType::Plus))
        );
        assert_eq!(
            Some("all plans".to_string()),
            parse_announcement_tip_toml(toml, /*plan*/ None)
        );
    }

    #[test]
    fn announcement_tip_toml_rejects_unknown_target_plan_type() {
        let toml = r#"
[[announcements]]
content = "all plans"

[[announcements]]
content = "typo announcement"
target_plan_types = ["prp"]
        "#;

        assert_eq!(
            Some("all plans".to_string()),
            parse_announcement_tip_toml(toml, Some(PlanType::Unknown))
        );
    }

    #[test]
    fn announcement_tip_toml_matches_target_os() {
        let toml = r#"
[[announcements]]
content = "linux announcement"
target_oses = ["linux"]

[[announcements]]
content = "macos announcement"
target_oses = ["macos"]

[[announcements]]
content = "windows announcement"
target_oses = ["windows"]
        "#;

        let expected = if cfg!(target_os = "macos") {
            "macos announcement"
        } else if cfg!(target_os = "windows") {
            "windows announcement"
        } else {
            "linux announcement"
        };
        assert_eq!(
            Some(expected.to_string()),
            parse_announcement_tip_toml(toml, /*plan*/ None)
        );
    }

    #[test]
    fn announcement_tip_toml_rejects_unknown_target_os() {
        let toml = r#"
[[announcements]]
content = "all operating systems"

[[announcements]]
content = "typo announcement"
target_oses = ["amiga"]
        "#;

        assert_eq!(
            Some("all operating systems".to_string()),
            parse_announcement_tip_toml(toml, /*plan*/ None)
        );
    }
}
