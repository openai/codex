use codex_core::features::FEATURES;
use codex_protocol::account::PlanType;
use codex_protocol::openai_models::ModelAvailabilityNux;
use codex_protocol::openai_models::ModelPreset;
use lazy_static::lazy_static;
use rand::Rng;
use std::collections::BTreeMap;

const ANNOUNCEMENT_TIP_URL: &str =
    "https://raw.githubusercontent.com/openai/codex/main/announcement_tip.toml";

const IS_MACOS: bool = cfg!(target_os = "macos");

const PAID_TOOLTIP: &str = "*New* Try the **Codex App** with 2x rate limits until *April 2nd*. Run 'codex app' or visit https://chatgpt.com/codex?app-landing-page=true";
const PAID_TOOLTIP_NON_MAC: &str = "*New* 2x rate limits until *April 2nd*.";
const OTHER_TOOLTIP: &str = "*New* Build faster with the **Codex App**. Run 'codex app' or visit https://chatgpt.com/codex?app-landing-page=true";
const OTHER_TOOLTIP_NON_MAC: &str = "*New* Build faster with Codex.";
const FREE_GO_TOOLTIP: &str =
    "*New* Codex is included in your plan for free through *March 2nd* – let’s build together.";

const RAW_TOOLTIPS: &str = include_str!("../tooltips.txt");

lazy_static! {
    static ref TOOLTIPS: Vec<&'static str> = RAW_TOOLTIPS
        .lines()
        .map(str::trim)
        .filter(|line| {
            if line.is_empty() || line.starts_with('#') {
                return false;
            }
            if !IS_MACOS && line.contains("codex app") {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StartupTip {
    Generic(String),
    AvailabilityNux { model: String, message: String },
}

impl StartupTip {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Generic(message) | Self::AvailabilityNux { message, .. } => message.clone(),
        }
    }

    pub(crate) fn availability_nux_model(&self) -> Option<&str> {
        match self {
            Self::Generic(_) => None,
            Self::AvailabilityNux { model, .. } => Some(model.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct StartupTips {
    pub(crate) first_session_tips: Vec<StartupTip>,
    pub(crate) selected_tip: Option<StartupTip>,
}

pub(crate) fn get_startup_tips(
    models: &[ModelPreset],
    availability_nux_display_counts: &BTreeMap<String, u32>,
    plan: Option<PlanType>,
    is_first_session: bool,
) -> StartupTips {
    let mut rng = rand::rng();
    get_startup_tips_with_rng(
        models,
        availability_nux_display_counts,
        plan,
        is_first_session,
        announcement::fetch_announcement_tip(),
        &mut rng,
    )
}

fn get_startup_tips_with_rng<R: Rng + ?Sized>(
    models: &[ModelPreset],
    availability_nux_display_counts: &BTreeMap<String, u32>,
    plan: Option<PlanType>,
    is_first_session: bool,
    announcement_tip: Option<String>,
    rng: &mut R,
) -> StartupTips {
    let availability_nux_tips = availability_nux_tips(models, availability_nux_display_counts);

    if is_first_session {
        return StartupTips {
            first_session_tips: availability_nux_tips,
            selected_tip: None,
        };
    }

    if availability_nux_tips.is_empty() {
        return StartupTips {
            first_session_tips: Vec::new(),
            selected_tip: get_generic_tooltip_with_rng(plan, announcement_tip, rng)
                .map(StartupTip::Generic),
        };
    }

    let mut weighted_candidates = Vec::new();
    for availability_nux_tip in &availability_nux_tips {
        weighted_candidates.push(availability_nux_tip.clone());
        weighted_candidates.push(availability_nux_tip.clone());
        weighted_candidates.push(availability_nux_tip.clone());
        weighted_candidates.push(availability_nux_tip.clone());
    }
    if let Some(announcement_tip) = announcement_tip {
        weighted_candidates.push(StartupTip::Generic(announcement_tip));
    }
    if let Some(plan_tip) = plan_tooltip(plan) {
        weighted_candidates.push(StartupTip::Generic(plan_tip.to_string()));
    }
    if let Some(random_tip) = pick_tooltip(rng) {
        weighted_candidates.push(StartupTip::Generic(random_tip.to_string()));
    }

    StartupTips {
        first_session_tips: Vec::new(),
        selected_tip: weighted_candidates
            .get(rng.random_range(0..weighted_candidates.len()))
            .cloned(),
    }
}

fn get_generic_tooltip_with_rng<R: Rng + ?Sized>(
    plan: Option<PlanType>,
    announcement_tip: Option<String>,
    rng: &mut R,
) -> Option<String> {
    if let Some(announcement) = announcement_tip {
        return Some(announcement);
    }

    // Leave small chance for a random tooltip to be shown.
    if rng.random_ratio(8, 10)
        && let Some(tooltip) = plan_tooltip(plan)
    {
        return Some(tooltip.to_string());
    }

    pick_tooltip(rng).map(str::to_string)
}

fn plan_tooltip(plan: Option<PlanType>) -> Option<&'static str> {
    match plan {
        Some(PlanType::Plus)
        | Some(PlanType::Business)
        | Some(PlanType::Team)
        | Some(PlanType::Enterprise)
        | Some(PlanType::Pro)
        | Some(PlanType::Edu) => Some(if IS_MACOS {
            PAID_TOOLTIP
        } else {
            PAID_TOOLTIP_NON_MAC
        }),
        Some(PlanType::Go) | Some(PlanType::Free) => Some(FREE_GO_TOOLTIP),
        Some(PlanType::Unknown) | None => Some(if IS_MACOS {
            OTHER_TOOLTIP
        } else {
            OTHER_TOOLTIP_NON_MAC
        }),
    }
}

fn availability_nux_tips(
    models: &[ModelPreset],
    availability_nux_display_counts: &BTreeMap<String, u32>,
) -> Vec<StartupTip> {
    models
        .iter()
        .filter_map(|preset| {
            preset
                .availability_nux
                .as_ref()
                .map(|availability_nux| (preset.model.as_str(), availability_nux))
        })
        .filter(|(model, _)| {
            availability_nux_display_counts
                .get(*model)
                .copied()
                .unwrap_or(0)
                < 4
        })
        .map(|(model, availability_nux)| startup_tip_from_availability_nux(model, availability_nux))
        .collect()
}

fn startup_tip_from_availability_nux(
    model: &str,
    availability_nux: &ModelAvailabilityNux,
) -> StartupTip {
    StartupTip::AvailabilityNux {
        model: model.to_string(),
        message: availability_nux.message.clone(),
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
    use regex_lite::Regex;
    use serde::Deserialize;
    use std::sync::OnceLock;
    use std::thread;
    use std::time::Duration;

    static ANNOUNCEMENT_TIP: OnceLock<Option<String>> = OnceLock::new();

    /// Prewarm the cache of the announcement tip.
    pub(crate) fn prewarm() {
        let _ = thread::spawn(|| ANNOUNCEMENT_TIP.get_or_init(init_announcement_tip_in_thread));
    }

    /// Fetch the announcement tip, return None if the prewarm is not done yet.
    pub(crate) fn fetch_announcement_tip() -> Option<String> {
        ANNOUNCEMENT_TIP
            .get()
            .cloned()
            .flatten()
            .and_then(|raw| parse_announcement_tip_toml(&raw))
    }

    #[derive(Debug, Deserialize)]
    struct AnnouncementTipRaw {
        content: String,
        from_date: Option<String>,
        to_date: Option<String>,
        version_regex: Option<String>,
        target_app: Option<String>,
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

    pub(crate) fn parse_announcement_tip_toml(text: &str) -> Option<String> {
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
            if tip.version_matches(CODEX_CLI_VERSION)
                && tip.date_matches(today)
                && tip.target_app == "cli"
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

            Some(Self {
                content: content.to_string(),
                from_date,
                to_date,
                version_regex,
                target_app: raw.target_app.unwrap_or("cli".to_string()).to_lowercase(),
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

    fn model_preset(
        model: &str,
        display_name: &str,
        availability_nux: Option<&str>,
        description: &str,
    ) -> ModelPreset {
        ModelPreset {
            id: model.to_string(),
            model: model.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            default_reasoning_effort: codex_protocol::openai_models::ReasoningEffort::Medium,
            supported_reasoning_efforts: vec![],
            supports_personality: false,
            is_default: false,
            upgrade: None,
            show_in_picker: true,
            availability_nux: availability_nux.map(|message| ModelAvailabilityNux {
                message: message.to_string(),
            }),
            supported_in_api: true,
            input_modalities: codex_protocol::openai_models::default_input_modalities(),
        }
    }

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
            parse_announcement_tip_toml(toml)
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
            parse_announcement_tip_toml(toml)
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

        assert_eq!(None, parse_announcement_tip_toml(toml));
    }

    #[test]
    fn announcement_tip_toml_bad_deserialization() {
        let toml = r#"
[[announcements]]
content = 123
from_date = "2000-01-01"
        "#;

        assert_eq!(None, parse_announcement_tip_toml(toml));
    }

    #[test]
    fn announcement_tip_toml_parse_comments() {
        let toml = r#"
# Example announcement tips for Codex TUI.
# Each [[announcements]] entry is evaluated in order; the last matching one is shown.
# Dates are UTC, formatted as YYYY-MM-DD. The from_date is inclusive and the to_date is exclusive.
# version_regex matches against the CLI version (env!("CARGO_PKG_VERSION")); omit to apply to all versions.
# target_app specify which app should display the announcement (cli, vsce, ...).

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
            parse_announcement_tip_toml(toml)
        );
    }

    #[test]
    fn first_session_eligible_model_returns_all_availability_nux_tips() {
        let preset = model_preset(
            "gpt-test",
            "GPT Test",
            Some("*New* Spark is now available to you."),
            "Fast, high-reliability coding model.",
        );
        let mut rng = StdRng::seed_from_u64(1);

        let tips = get_startup_tips_with_rng(
            &[preset],
            &BTreeMap::new(),
            Some(PlanType::Plus),
            true,
            Some("announcement".to_string()),
            &mut rng,
        );

        assert_eq!(
            StartupTips {
                first_session_tips: vec![StartupTip::AvailabilityNux {
                    model: "gpt-test".to_string(),
                    message: "*New* Spark is now available to you.".to_string(),
                }],
                selected_tip: None,
            },
            tips
        );
    }

    #[test]
    fn first_session_ineligible_model_skips_tip() {
        let preset = model_preset("gpt-test", "GPT Test", None, "");
        let mut rng = StdRng::seed_from_u64(1);

        let tips = get_startup_tips_with_rng(
            &[preset],
            &BTreeMap::new(),
            Some(PlanType::Plus),
            true,
            Some("announcement".to_string()),
            &mut rng,
        );

        assert_eq!(StartupTips::default(), tips);
    }

    #[test]
    fn first_session_returns_multiple_availability_nuxes() {
        let mut rng = StdRng::seed_from_u64(1);
        let models = vec![
            model_preset(
                "spark",
                "Spark",
                Some("*New* Spark is now available to you."),
                "",
            ),
            model_preset(
                "canvas",
                "Canvas",
                Some("*New* Canvas is now available to you."),
                "",
            ),
        ];

        let tips = get_startup_tips_with_rng(
            &models,
            &BTreeMap::new(),
            Some(PlanType::Plus),
            true,
            Some("announcement".to_string()),
            &mut rng,
        );

        assert_eq!(
            StartupTips {
                first_session_tips: vec![
                    StartupTip::AvailabilityNux {
                        model: "spark".to_string(),
                        message: "*New* Spark is now available to you.".to_string(),
                    },
                    StartupTip::AvailabilityNux {
                        model: "canvas".to_string(),
                        message: "*New* Canvas is now available to you.".to_string(),
                    },
                ],
                selected_tip: None,
            },
            tips
        );
    }

    #[test]
    fn later_session_can_select_availability_nux_from_weighted_pool() {
        let preset = model_preset(
            "gpt-test",
            "GPT Test",
            Some("*New* Spark is now available to you."),
            "",
        );
        let mut rng = StdRng::seed_from_u64(5);

        let tips = get_startup_tips_with_rng(
            &[preset],
            &BTreeMap::new(),
            Some(PlanType::Plus),
            false,
            Some("announcement".to_string()),
            &mut rng,
        );

        assert_eq!(
            StartupTips {
                first_session_tips: Vec::new(),
                selected_tip: Some(StartupTip::AvailabilityNux {
                    model: "gpt-test".to_string(),
                    message: "*New* Spark is now available to you.".to_string(),
                }),
            },
            tips
        );
    }

    #[test]
    fn later_session_count_limit_disables_availability_nux() {
        let preset = model_preset(
            "gpt-test",
            "GPT Test",
            Some("*New* Spark is now available to you."),
            "",
        );
        let counts = BTreeMap::from([("gpt-test".to_string(), 4)]);
        let mut rng = StdRng::seed_from_u64(5);

        let tips = get_startup_tips_with_rng(
            &[preset],
            &counts,
            Some(PlanType::Plus),
            false,
            Some("announcement".to_string()),
            &mut rng,
        );

        assert_eq!(
            StartupTips {
                first_session_tips: Vec::new(),
                selected_tip: Some(StartupTip::Generic("announcement".to_string())),
            },
            tips
        );
    }

    #[test]
    fn later_session_eligible_model_includes_announcement_and_generic_candidates() {
        let preset = model_preset(
            "gpt-test",
            "GPT Test",
            Some("*New* Spark is now available to you."),
            "",
        );
        let mut saw_announcement = false;
        let mut saw_plan_tip = false;
        let mut saw_random_tip = false;
        let mut saw_availability_nux_tip = false;

        for seed in 0..64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let tip = get_startup_tips_with_rng(
                std::slice::from_ref(&preset),
                &BTreeMap::new(),
                Some(PlanType::Plus),
                false,
                Some("announcement".to_string()),
                &mut rng,
            )
            .selected_tip
            .expect("tip");

            match tip {
                StartupTip::Generic(message) if message == "announcement" => {
                    saw_announcement = true;
                }
                StartupTip::Generic(message)
                    if message == plan_tooltip(Some(PlanType::Plus)).unwrap() =>
                {
                    saw_plan_tip = true;
                }
                StartupTip::Generic(_) => {
                    saw_random_tip = true;
                }
                StartupTip::AvailabilityNux { .. } => {
                    saw_availability_nux_tip = true;
                }
            }
        }

        assert!(saw_announcement);
        assert!(saw_plan_tip);
        assert!(saw_random_tip);
        assert!(saw_availability_nux_tip);
    }
}
