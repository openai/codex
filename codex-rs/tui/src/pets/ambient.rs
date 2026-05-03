//! Ambient terminal rendering for the Codex companion.
//!
//! Ambient pets reuse the same extracted image frames as the full-screen viewer. The surrounding
//! TUI still owns the notification text and layout slot; the sprite itself is emitted through the
//! terminal image protocol after ratatui finishes drawing the frame.

use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::tui::FrameRequester;

use super::DEFAULT_PET_ID;
use super::frames;
use super::image_protocol::ImageProtocol;
use super::image_protocol::ProtocolSelection;
use super::model::Animation;
use super::model::Pet;

const PET_TARGET_HEIGHT_PX: u16 = 75;

const RUNNING_LIFETIME: Duration = Duration::from_secs(3 * 60);
const FAILED_LIFETIME: Duration = Duration::from_secs(60 * 60);
const WAITING_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);
const REVIEW_LIFETIME: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PetNotificationKind {
    Running,
    Waiting,
    Review,
    Failed,
}

impl PetNotificationKind {
    fn animation_name(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Review => "review",
            Self::Failed => "failed",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Waiting => "Needs input",
            Self::Review => "Ready",
            Self::Failed => "Blocked",
        }
    }

    fn fallback_body(self) -> &'static str {
        match self {
            Self::Running => "Thinking",
            Self::Waiting => "Needs input",
            Self::Review => "Ready",
            Self::Failed => "Blocked",
        }
    }

    fn lifetime(self) -> Duration {
        match self {
            Self::Running => RUNNING_LIFETIME,
            Self::Waiting => WAITING_LIFETIME,
            Self::Review => REVIEW_LIFETIME,
            Self::Failed => FAILED_LIFETIME,
        }
    }
}

#[derive(Debug, Clone)]
struct PetNotification {
    kind: PetNotificationKind,
    body: String,
    updated_at: Instant,
}

impl PetNotification {
    fn new(kind: PetNotificationKind, body: Option<String>) -> Self {
        Self {
            kind,
            body: body.unwrap_or_else(|| kind.fallback_body().to_string()),
            updated_at: Instant::now(),
        }
    }

    fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.updated_at) >= self.kind.lifetime()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AmbientPetDraw {
    pub(crate) frame: PathBuf,
    pub(crate) protocol: ImageProtocol,
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) height_px: u16,
    pub(crate) sixel_dir: PathBuf,
}

pub(crate) struct AmbientPet {
    pet: Pet,
    protocol: Option<ImageProtocol>,
    frames: Vec<PathBuf>,
    sixel_dir: PathBuf,
    frame_requester: FrameRequester,
    notification: Option<PetNotification>,
    animation_started_at: Instant,
}

impl AmbientPet {
    pub(crate) fn load(
        selected_pet: Option<&str>,
        codex_home: &std::path::Path,
        frame_requester: FrameRequester,
    ) -> Result<Self> {
        let pet =
            Pet::load_with_codex_home(selected_pet.unwrap_or(DEFAULT_PET_ID), Some(codex_home))
                .with_context(|| "load ambient pet")?;
        let cache_dir = frames::cache_dir().join("tui-pets").join(&pet.id);
        let frame_dir = cache_dir.join("frames");
        let sixel_dir = cache_dir.join("sixel");
        let frames = frames::prepare_png_frames(&pet, &frame_dir)?;
        Ok(Self {
            pet,
            protocol: ProtocolSelection::Auto.resolve(),
            frames,
            sixel_dir,
            frame_requester,
            notification: None,
            animation_started_at: Instant::now(),
        })
    }

    pub(crate) fn set_notification(&mut self, kind: PetNotificationKind, body: Option<String>) {
        self.notification = Some(PetNotification::new(kind, body));
        self.animation_started_at = Instant::now();
    }

    pub(crate) fn image_enabled(&self) -> bool {
        self.protocol.is_some()
    }

    pub(crate) fn schedule_next_frame(&self) {
        if self.protocol.is_none() {
            return;
        }

        let animation = self.current_animation();
        if animation.frames.len() <= 1 {
            return;
        }
        let frame_duration = Duration::from_secs_f64(1.0 / animation.fps.max(0.1));
        let elapsed = self.animation_started_at.elapsed();
        let rem = elapsed.as_nanos() % frame_duration.as_nanos();
        let delay = if rem == 0 {
            frame_duration
        } else {
            frame_duration.saturating_sub(Duration::from_nanos(rem as u64))
        };
        self.frame_requester.schedule_frame_in(delay);
    }

    pub(crate) fn draw_request(&self, area: Rect, footer_height: u16) -> Option<AmbientPetDraw> {
        let protocol = self.protocol?;
        let size = self.image_size();
        let notification = self.visible_notification(Instant::now());
        let notification_height = notification.map_or(0, notification_height);
        let notification_width = notification.map_or(0, notification_width);
        let required_height = size.rows.saturating_add(notification_height);
        if area.height < required_height.saturating_add(footer_height)
            || area.width < size.columns.max(notification_width)
        {
            return None;
        }

        let x = area.x + area.width.saturating_sub(size.columns);
        let y = area
            .bottom()
            .saturating_sub(footer_height)
            .saturating_sub(size.rows);
        Some(AmbientPetDraw {
            frame: self.current_frame_path(),
            protocol,
            x,
            y,
            columns: size.columns,
            rows: size.rows,
            height_px: size.height_px,
            sixel_dir: self.sixel_dir.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn selected_pet_id(&self) -> &str {
        &self.pet.id
    }

    pub(crate) fn render_overlay(&self, area: Rect, footer_height: u16, buf: &mut Buffer) {
        let notification = self.visible_notification(Instant::now());
        let size = self.protocol.map(|_| self.image_size());
        let notification_height = notification.map_or(0, notification_height);
        let notification_width = notification.map_or(0, notification_width);
        let image_columns = size.map_or(0, |size| size.columns);
        let image_rows = size.map_or(0, |size| size.rows);
        let required_height = image_rows.saturating_add(notification_height);
        if area.height < required_height.saturating_add(footer_height)
            || area.width < image_columns.max(notification_width)
        {
            return;
        }

        if let Some(notification) = notification {
            let x = area.x
                + area
                    .width
                    .saturating_sub(notification_width.max(image_columns));
            let y = area
                .bottom()
                .saturating_sub(footer_height)
                .saturating_sub(image_rows + notification_height);
            render_notification(notification, x, y, buf);
        }
    }

    fn visible_notification(&self, now: Instant) -> Option<&PetNotification> {
        self.notification
            .as_ref()
            .filter(|notification| !notification.is_expired(now))
    }

    fn current_animation(&self) -> &Animation {
        let animation_name = self
            .visible_notification(Instant::now())
            .map_or("idle", |notification| notification.kind.animation_name());
        let Some(animation) = self
            .pet
            .animations
            .get(animation_name)
            .or_else(|| self.pet.animations.get("idle"))
        else {
            unreachable!("ambient pets always have an idle animation");
        };
        if !animation.loop_animation {
            let elapsed_frames = (self.animation_started_at.elapsed().as_secs_f64()
                * animation.fps.max(0.1))
            .floor() as usize;
            if elapsed_frames >= animation.frames.len()
                && let Some(fallback) = self.pet.animations.get(&animation.fallback)
            {
                return fallback;
            }
        }
        animation
    }

    fn current_frame_path(&self) -> PathBuf {
        let animation = self.current_animation();
        let frame_index = current_animation_frame(animation, self.animation_started_at.elapsed());
        let sprite_index = animation.frames[frame_index];
        self.frames[sprite_index.min(self.frames.len().saturating_sub(1))].clone()
    }

    fn image_size(&self) -> ImageSize {
        let rows = ((f64::from(PET_TARGET_HEIGHT_PX) / 15.0).round() as u16).max(1);
        let aspect = f64::from(self.pet.frame_height) / f64::from(self.pet.frame_width) * 0.52;
        let columns = (f64::from(rows) / aspect).round() as u16;
        ImageSize {
            columns: columns.max(1),
            rows,
            height_px: PET_TARGET_HEIGHT_PX,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ImageSize {
    columns: u16,
    rows: u16,
    height_px: u16,
}

fn current_animation_frame(animation: &Animation, elapsed: Duration) -> usize {
    if animation.frames.len() <= 1 {
        return 0;
    }
    let elapsed_frames = (elapsed.as_secs_f64() * animation.fps.max(0.1)).floor() as usize;
    if animation.loop_animation {
        elapsed_frames % animation.frames.len()
    } else {
        elapsed_frames.min(animation.frames.len() - 1)
    }
}

fn notification_height(notification: &PetNotification) -> u16 {
    if notification.body == notification.kind.label() {
        1
    } else {
        2
    }
}

fn notification_width(notification: &PetNotification) -> u16 {
    notification
        .kind
        .label()
        .len()
        .max(notification.body.len())
        .try_into()
        .unwrap_or(u16::MAX)
}

fn render_notification(notification: &PetNotification, x: u16, y: u16, buf: &mut Buffer) {
    let width = buf.area.right().saturating_sub(x);
    let mut lines = vec![notification.kind.label()];
    if notification.body != notification.kind.label() {
        lines.push(notification.body.as_str());
    }
    for (offset, line) in lines.into_iter().enumerate() {
        buf.set_stringn(
            x,
            y + offset as u16,
            line,
            width as usize,
            ratatui::style::Style::default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_labels_match_codex_app_vocabulary() {
        assert_eq!(PetNotificationKind::Running.label(), "Running");
        assert_eq!(PetNotificationKind::Waiting.label(), "Needs input");
        assert_eq!(PetNotificationKind::Review.label(), "Ready");
        assert_eq!(PetNotificationKind::Failed.label(), "Blocked");
    }
}
