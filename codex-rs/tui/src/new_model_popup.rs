use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_core::config::SWIFTFOX_MODEL_DISPLAY_NAME;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use rand::Rng as _;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::time::Duration;
use tokio_stream::StreamExt;

// Embed animation frames for each variant at compile time.
macro_rules! frames_for {
    ($dir:literal) => {
        [
            include_str!(concat!("../frames/", $dir, "/frame_000.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_004.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_008.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_012.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_016.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_020.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_024.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_028.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_032.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_036.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_040.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_044.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_048.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_052.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_056.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_060.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_064.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_068.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_072.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_076.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_080.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_084.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_088.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_092.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_096.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_100.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_104.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_108.txt")),
            include_str!(concat!("../frames/", $dir, "/frame_112.txt")),
        ]
    };
}

const FRAMES_DEFAULT: [&str; 29] = frames_for!("default");
const FRAMES_CODEX: [&str; 29] = frames_for!("codex");
const FRAMES_OPENAI: [&str; 29] = frames_for!("openai");
const FRAMES_BLOCKS: [&str; 29] = frames_for!("blocks");
const FRAMES_DOTS: [&str; 29] = frames_for!("dots");
const FRAMES_HASH: [&str; 29] = frames_for!("hash");
const FRAMES_HBARS: [&str; 29] = frames_for!("hbars");
const FRAMES_VBARS: [&str; 29] = frames_for!("vbars");
const FRAMES_SHAPES: [&str; 29] = frames_for!("shapes");
const FRAMES_SLUG: [&str; 29] = frames_for!("slug");

const VARIANTS: &[&[&str]] = &[
    &FRAMES_DEFAULT,
    &FRAMES_CODEX,
    &FRAMES_OPENAI,
    &FRAMES_BLOCKS,
    &FRAMES_DOTS,
    &FRAMES_HASH,
    &FRAMES_HBARS,
    &FRAMES_VBARS,
    &FRAMES_SHAPES,
    &FRAMES_SLUG,
];

const FRAME_TICK: Duration = Duration::from_millis(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelUpgradeDecision {
    Switch,
    KeepCurrent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelUpgradeOption {
    TryNewModel,
    KeepCurrent,
}

struct ModelUpgradePopup {
    highlighted: ModelUpgradeOption,
    decision: Option<ModelUpgradeDecision>,
    request_frame: FrameRequester,
    frame_idx: usize,
    variant_idx: usize,
}

impl ModelUpgradePopup {
    fn new(request_frame: FrameRequester) -> Self {
        Self {
            highlighted: ModelUpgradeOption::TryNewModel,
            decision: None,
            request_frame,
            frame_idx: 0,
            variant_idx: 0,
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.highlight(ModelUpgradeOption::TryNewModel),
            KeyCode::Down | KeyCode::Char('j') => self.highlight(ModelUpgradeOption::KeepCurrent),
            KeyCode::Char('1') => self.select(ModelUpgradeOption::TryNewModel),
            KeyCode::Char('2') => self.select(ModelUpgradeOption::KeepCurrent),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(ModelUpgradeOption::KeepCurrent),
            KeyCode::Char('.') => {
                if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.pick_random_variant();
                }
            }
            _ => {}
        }
    }

    fn highlight(&mut self, option: ModelUpgradeOption) {
        if self.highlighted != option {
            self.highlighted = option;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, option: ModelUpgradeOption) {
        self.decision = Some(option.into());
        self.request_frame.schedule_frame();
    }

    fn advance_animation(&mut self) {
        let len = self.frames().len();
        self.frame_idx = (self.frame_idx + 1) % len;
        self.request_frame.schedule_frame_in(FRAME_TICK);
    }

    fn frames(&self) -> &'static [&'static str] {
        VARIANTS[self.variant_idx]
    }

    fn pick_random_variant(&mut self) {
        let total = VARIANTS.len();
        if total <= 1 {
            return;
        }
        let mut rng = rand::rng();
        let mut next = self.variant_idx;
        while next == self.variant_idx {
            next = rng.random_range(0..total);
        }
        self.variant_idx = next;
        self.request_frame.schedule_frame();
    }
}

impl From<ModelUpgradeOption> for ModelUpgradeDecision {
    fn from(option: ModelUpgradeOption) -> Self {
        match option {
            ModelUpgradeOption::TryNewModel => ModelUpgradeDecision::Switch,
            ModelUpgradeOption::KeepCurrent => ModelUpgradeDecision::KeepCurrent,
        }
    }
}

impl WidgetRef for &ModelUpgradePopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let mut lines: Vec<Line> = self.frames()[self.frame_idx]
            .lines()
            .map(|l| l.to_string().into())
            .collect();

        // Spacer between animation and text content.
        lines.push("".into());

        lines.push(
            format!(
                "   Codex is now powered by {SWIFTFOX_MODEL_DISPLAY_NAME}, a new model that is"
            )
            .into(),
        );
        lines.push(Line::from(vec![
            "   ".into(),
            "faster, a better collaborator, ".bold(),
            "and ".into(),
            "more steerable.".bold(),
        ]));
        lines.push("".into());

        let create_option =
            |index: usize, option: ModelUpgradeOption, text: &str| -> Line<'static> {
                if self.highlighted == option {
                    Line::from(vec![
                        format!("> {}. ", index + 1).cyan(),
                        text.to_owned().cyan(),
                    ])
                } else {
                    format!("  {}. {text}", index + 1).into()
                }
            };

        lines.push(create_option(
            0,
            ModelUpgradeOption::TryNewModel,
            &format!("Yes, switch me to {SWIFTFOX_MODEL_DISPLAY_NAME}"),
        ));
        lines.push(create_option(
            1,
            ModelUpgradeOption::KeepCurrent,
            "Not right now",
        ));
        lines.push("".into());
        lines.push(
            "  Press Enter to confirm or Esc to keep your current model"
                .dim()
                .into(),
        );

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

pub(crate) async fn run_model_upgrade_popup(tui: &mut Tui) -> Result<ModelUpgradeDecision> {
    let mut popup = ModelUpgradePopup::new(tui.frame_requester());

    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&popup, frame.area());
    })?;

    popup.advance_animation();

    let events = tui.event_stream();
    tokio::pin!(events);
    while popup.decision.is_none() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => popup.handle_key_event(key_event),
                TuiEvent::Draw => {
                    popup.advance_animation();
                    let _ = tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&popup, frame.area());
                    });
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    Ok(popup.decision.unwrap_or(ModelUpgradeDecision::KeepCurrent))
}
