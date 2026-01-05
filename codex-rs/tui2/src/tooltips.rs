use lazy_static::lazy_static;
use rand::Rng;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::task;

const ANNOUNCEMENT_TIP_URL: &str =
    "https://raw.githubusercontent.com/openai/codex/main/announcement_tip";
static ANNOUNCEMENT_TIP: OnceLock<Option<String>> = OnceLock::new();
const RAW_TOOLTIPS: &str = include_str!("../tooltips.txt");

lazy_static! {
    static ref TOOLTIPS: Vec<&'static str> = RAW_TOOLTIPS
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
}

pub(crate) fn random_tooltip() -> Option<String> {
    if let Some(announcement) = fetch_announcement_tip() {
        return Some(announcement);
    }
    let mut rng = rand::rng();
    pick_tooltip(&mut rng).map(str::to_string)
}

fn pick_tooltip<R: Rng + ?Sized>(rng: &mut R) -> Option<&'static str> {
    if TOOLTIPS.is_empty() {
        None
    } else {
        TOOLTIPS.get(rng.random_range(0..TOOLTIPS.len())).copied()
    }
}

fn fetch_announcement_tip() -> Option<String> {
    let tip_ref = ANNOUNCEMENT_TIP.get_or_init(|| {
        let handle = Handle::try_current().ok()?;
        let text = task::block_in_place(|| {
            handle.block_on(async {
                let response = reqwest::Client::new()
                    .get(ANNOUNCEMENT_TIP_URL)
                    .timeout(Duration::from_millis(100))
                    .send()
                    .await
                    .ok()?;
                let text = response.error_for_status().ok()?.text().await.ok()?;
                Some(text)
            })
        })?;

        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    tip_ref.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
