//! Numbered reusable inputs with one HUD and pickers for editing, saving, and deleting.
//! Saving leaves the draft untouched; recall preserves drafts and obeys normal command gates.
//! The shared-leader submenu respects popup ownership, cancellation, and held-key repeats.

use super::*;
use crate::bottom_pane::ShortcutHud;
use crate::wrapping::word_wrap_lines;
use codex_config::types::QuickDial;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;

struct QuickDialConfirmationHeader(Vec<Line<'static>>);

impl Renderable for QuickDialConfirmationHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(
            &Paragraph::new(word_wrap_lines(&self.0, usize::from(area.width))),
            area,
            buf,
        );
    }
    fn desired_height(&self, width: u16) -> u16 {
        word_wrap_lines(&self.0, usize::from(width)).len() as u16
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuickDialPickerMode {
    Save,
    Edit,
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuickDialEditorTarget {
    PickSlot,
    Slot(u8),
}

fn quick_dial_label(dial: &QuickDial) -> String {
    dial.label
        .as_deref()
        .filter(|label| !label.trim().is_empty())
        .unwrap_or(&dial.input)
        .trim_start()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

impl ChatWidget {
    pub(crate) fn quick_dial_menu_open(&self) -> bool {
        self.quick_dial_menu
    }

    pub(crate) fn quick_dial_hud(&self) -> ShortcutHud {
        let mut entries: Vec<_> = (1..=9)
            .chain(std::iter::once(/*value*/ 0))
            .filter_map(|slot| {
                let key = slot.to_string();
                self.local_settings
                    .tui
                    .quick_dials
                    .get(&key)
                    .map(|dial| ShortcutHud::item(key, quick_dial_label(dial).into()))
            })
            .collect();
        if entries.is_empty() {
            entries.push("No presets saved. Press n to create one.".dim().into());
        }
        let commands = [
            ("n", "new"),
            ("s", "from settings"),
            ("e", "edit"),
            ("d", "delete"),
            ("esc", "close"),
        ]
        .into_iter()
        .map(|(key, label)| ShortcutHud::item(key, label.into()))
        .collect();
        ShortcutHud::new("Presets", entries, commands)
    }

    fn composer_quick_dial_text(&mut self) -> String {
        if !self.bottom_pane.composer_local_images().is_empty()
            || !self.bottom_pane.remote_image_urls().is_empty()
        {
            self.add_info_message(
                "Images cannot be copied into a preset. Starting with empty text.".into(),
                /*hint*/ None,
            );
            return String::new();
        }
        if !self
            .bottom_pane
            .composer_draft_snapshot()
            .mention_bindings
            .is_empty()
        {
            self.add_info_message(
                "Linked mentions cannot be copied into a preset. Starting with empty text.".into(),
                /*hint*/ None,
            );
            return String::new();
        }
        self.bottom_pane.composer_text_with_pending()
    }

    pub(super) fn current_model_quick_dial(&self) -> Result<QuickDial, &'static str> {
        let Some(model) = self
            .model_catalog
            .models
            .iter()
            .find(|model| model.model == self.current_model() && model.show_in_picker)
            .filter(|_| !self.restrict_model_picker_to_luna_reserve())
        else {
            return Err("The current model cannot be recalled with /model.");
        };
        let mut input = format!("/model {}", self.current_model());
        if let Some(effort) = self.current_reasoning_effort() {
            let supported = if model.supported_reasoning_efforts.is_empty() {
                effort == model.default_reasoning_effort
            } else {
                model
                    .supported_reasoning_efforts
                    .iter()
                    .any(|option| option.effort == effort)
            };
            if !supported {
                return Err("Choose an available reasoning effort with /model before saving.");
            }
            input.push_str(&format!(" --reasoning={effort}"));
        }
        let speed = self
            .current_service_tier()
            .filter(|tier| *tier != SERVICE_TIER_DEFAULT_REQUEST_VALUE)
            .unwrap_or("standard");
        if !matches!(speed, "standard" | "flex")
            && !model
                .service_tiers
                .iter()
                .any(|tier| tier.id == speed && self.config.features.service_tier_enabled(&tier.id))
        {
            return Err("Choose an available speed with /model before saving.");
        }
        input.push_str(&format!(" --speed={speed}"));
        let dial = QuickDial { input, label: None };
        dial.validate()?;
        Ok(dial)
    }

    pub(super) fn current_terminal_title_quick_dial(&self) -> Result<QuickDial, &'static str> {
        status_surface_quick_dial::<TerminalTitleItem>(
            "/title",
            self.configured_terminal_title_items(),
            /*suffix*/ None,
            "Choose available terminal-title items with /title before saving.",
        )
    }

    pub(super) fn current_status_line_quick_dial(&self) -> Result<QuickDial, &'static str> {
        let colors = if self.local_settings.tui.status_line_use_colors {
            "--colors"
        } else {
            "--no-colors"
        };
        status_surface_quick_dial::<StatusLineItem>(
            "/statusline",
            self.configured_status_line_items(),
            /*suffix*/ Some(colors),
            "Choose available status-line items with /statusline before saving.",
        )
    }

    fn quick_dials_available(&mut self) -> bool {
        if self.blocks_direct_input
            || self.active_side_conversation
            || !self.bottom_pane.composer_input_enabled()
        {
            self.add_error_message("Presets are unavailable in this conversation.".into());
            return false;
        }
        if !self.is_session_configured() {
            self.add_info_message(
                "Presets are available after startup completes.".into(),
                /*hint*/ None,
            );
            return false;
        }
        true
    }

    pub(crate) fn recall_quick_dial(&mut self, slot: u8) {
        if !self.quick_dials_available() {
            return;
        }
        let Some(dial) = self
            .local_settings
            .tui
            .quick_dials
            .get(&slot.to_string())
            .cloned()
        else {
            return;
        };
        if let Err(reason) = dial.validate() {
            self.add_error_message(format!("Preset {slot}: {reason}"));
            return;
        }
        self.app_event_tx.send(AppEvent::FollowTranscript);
        self.queue_user_message_with_options(
            dial.input.into(),
            QueuedInputAction::ParseSlash,
            Vec::new(),
        );
    }

    fn open_quick_dials_picker(&mut self, mode: QuickDialPickerMode) {
        if !self.quick_dials_available() {
            return;
        }
        self.show_quick_dials_picker(mode, /*current*/ None);
    }

    pub(crate) fn open_quick_dial_save_picker(&mut self, dial: QuickDial) {
        if let Err(reason) = dial.validate() {
            self.add_error_message(reason.into());
            return;
        }
        self.show_quick_dials_picker(QuickDialPickerMode::Save, Some(dial));
    }

    fn show_quick_dials_picker(&mut self, mode: QuickDialPickerMode, current: Option<QuickDial>) {
        let mut items = Vec::new();
        for slot in (1..=9).chain(std::iter::once(/*value*/ 0)) {
            let saved = self.local_settings.tui.quick_dials.get(&slot.to_string());
            if mode != QuickDialPickerMode::Save && saved.is_none() {
                continue;
            }
            let dial = current.clone();
            let saved_for_edit = saved.cloned();
            let has_saved = saved.is_some();
            let (is_disabled, dismiss_on_select, dismiss_parent_on_child_accept) = match mode {
                QuickDialPickerMode::Save => (dial.is_none(), !has_saved, has_saved),
                QuickDialPickerMode::Edit | QuickDialPickerMode::Clear => (false, false, true),
            };
            items.push(SelectionItem {
                name: saved.map(quick_dial_label).unwrap_or_default(),
                name_prefix_spans: if saved.is_none() {
                    vec!["empty".dim()]
                } else {
                    Vec::new()
                },
                description: saved
                    .filter(|dial| {
                        dial.label
                            .as_deref()
                            .is_some_and(|label| !label.trim().is_empty())
                    })
                    .map(|dial| dial.input.clone()),
                display_shortcut: Some(
                    key_hint::plain(KeyCode::Char(char::from(b'0' + slot))).into(),
                ),
                is_disabled,
                actions: vec![Box::new(move |tx| {
                    let event = match mode {
                        QuickDialPickerMode::Save if has_saved => AppEvent::ConfirmQuickDial {
                            slot,
                            dial: dial.clone(),
                        },
                        QuickDialPickerMode::Save => AppEvent::SaveQuickDial {
                            slot,
                            dial: dial.clone(),
                        },
                        QuickDialPickerMode::Edit => AppEvent::OpenQuickDialEditor {
                            target: QuickDialEditorTarget::Slot(slot),
                            dial: saved_for_edit.clone(),
                        },
                        QuickDialPickerMode::Clear => {
                            AppEvent::ConfirmQuickDial { slot, dial: None }
                        }
                    };
                    tx.send(event);
                })],
                dismiss_on_select,
                dismiss_parent_on_child_accept,
                ..Default::default()
            });
        }
        let title = match mode {
            QuickDialPickerMode::Save => "Save preset to slot",
            QuickDialPickerMode::Edit => "Edit a preset",
            QuickDialPickerMode::Clear => "Delete a preset",
        };
        let initial_selected_idx = (mode == QuickDialPickerMode::Save).then(|| {
            (1..=9)
                .chain(std::iter::once(/*value*/ 0))
                .position(|slot| {
                    !self
                        .local_settings
                        .tui
                        .quick_dials
                        .contains_key(&slot.to_string())
                })
                .unwrap_or(/*default*/ 0)
        });
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some("quick-dials"),
            auto_number_shortcuts: false,
            initial_selected_idx,
            max_visible_rows: 10,
            title: Some(title.into()),
            subtitle: (mode == QuickDialPickerMode::Save)
                .then(|| current.as_ref().map(quick_dial_label))
                .flatten(),
            items,
            ..SelectionViewParams::picker()
        });
        self.defer_input_until_settings_applied();
    }

    pub(crate) fn open_quick_dial_editor(
        &mut self,
        target: QuickDialEditorTarget,
        dial: Option<QuickDial>,
    ) {
        if !self.quick_dials_available() {
            return;
        }
        let slot = match target {
            QuickDialEditorTarget::PickSlot => None,
            QuickDialEditorTarget::Slot(slot) => Some(slot),
        };
        let dial = dial.unwrap_or_else(|| QuickDial {
            input: self.composer_quick_dial_text(),
            label: None,
        });
        let title = slot
            .filter(|slot| {
                self.local_settings
                    .tui
                    .quick_dials
                    .contains_key(&slot.to_string())
            })
            .map_or_else(
                || "New preset".to_string(),
                |slot| format!("Edit preset {slot}"),
            );
        let submit_label = if slot.is_some() {
            "review changes"
        } else {
            "choose slot"
        };
        let tx = self.app_event_tx.clone();
        let label = dial.label;
        let view = CustomPromptView::new(
            title,
            "Write a prompt or slash command".to_string(),
            dial.input,
            /*context_label*/ None,
            Box::new(move |input| {
                let dial = QuickDial {
                    input,
                    label: label.clone(),
                };
                tx.send(match target {
                    QuickDialEditorTarget::PickSlot => AppEvent::OpenQuickDialSavePicker(dial),
                    QuickDialEditorTarget::Slot(slot) => AppEvent::ConfirmQuickDial {
                        slot,
                        dial: Some(dial),
                    },
                });
            }),
        )
        .with_child_flow(submit_label)
        .preserve_whitespace();
        self.bottom_pane.show_text_prompt(view);
        self.defer_input_until_settings_applied();
    }

    pub(super) fn open_quick_dial_prefills(&mut self) {
        if !self.quick_dials_available() {
            return;
        }
        let permissions = self
            .config
            .permissions
            .active_permission_profile()
            .map(|profile| -> Result<QuickDial, &'static str> {
                let approval_policy =
                    AskForApproval::from(self.config.permissions.approval_policy.value());
                let approval_policy = match approval_policy {
                    AskForApproval::UnlessTrusted => "untrusted".to_string(),
                    AskForApproval::OnRequest => "on-request".to_string(),
                    AskForApproval::Never => "never".to_string(),
                    granular @ AskForApproval::Granular { .. } => serde_json::to_string(&granular)
                        .map_err(|_| "The approval policy cannot be recalled.")?,
                };
                let profile_id = shlex::try_quote(&profile.id)
                    .map_err(|_| "The permission profile name cannot be recalled.")?;
                let approval_policy = shlex::try_quote(&approval_policy)
                    .map_err(|_| "The approval policy cannot be recalled.")?;
                let input = format!(
                    "/permissions {profile_id} --approval-policy={approval_policy} --reviewer={}",
                    self.config.approvals_reviewer
                );
                let dial = QuickDial { input, label: None };
                dial.validate()?;
                Ok(dial)
            })
            .unwrap_or(Err(
                "Select a named permission profile with /permissions first.",
            ));
        let terminal_title = self.current_terminal_title_quick_dial();
        let status_line = self.current_status_line_quick_dial();
        let items = [
            ("Current model", self.current_model_quick_dial()),
            ("Current permissions", permissions),
            ("Current terminal title", terminal_title),
            ("Current status line", status_line),
        ]
        .into_iter()
        .map(|(name, dial)| {
            let (description, is_disabled, actions) = match dial {
                Ok(dial) => {
                    let description = Some(dial.input.clone());
                    let actions = vec![Box::new(move |tx: &AppEventSender| {
                        tx.send(AppEvent::OpenQuickDialEditor {
                            target: QuickDialEditorTarget::PickSlot,
                            dial: Some(dial.clone()),
                        })
                    }) as SelectionAction];
                    (description, false, actions)
                }
                Err(reason) => (Some(reason.into()), true, Vec::new()),
            };
            SelectionItem {
                name: name.into(),
                description,
                is_disabled,
                actions,
                dismiss_parent_on_child_accept: true,
                ..Default::default()
            }
        })
        .collect();
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some("quick-dial-prefills"),
            max_visible_rows: 4,
            title: Some("Start preset from current settings".into()),
            items,
            ..SelectionViewParams::picker()
        });
        self.defer_input_until_settings_applied();
    }

    pub(crate) fn confirm_quick_dial(&mut self, slot: u8, dial: Option<QuickDial>) {
        if let Some(dial) = &dial
            && let Err(reason) = dial.validate()
        {
            self.add_error_message(reason.into());
            return;
        }
        let saved = self.local_settings.tui.quick_dials.get(&slot.to_string());
        let (action, description) = match (saved, &dial) {
            (Some(old), Some(new)) => (
                "Replace",
                format!("{}\n→ {}", old.input.trim(), new.input.trim()),
            ),
            (Some(old), None) => ("Delete", quick_dial_label(old)),
            (None, Some(new)) => ("Save", new.input.trim().to_string()),
            (None, None) => return,
        };
        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(QuickDialConfirmationHeader(
                std::iter::once(Line::from(format!("{action} preset {slot}?").bold()))
                    .chain(
                        description
                            .lines()
                            .map(|line| Line::from(line.to_string().dim())),
                    )
                    .collect(),
            )),
            items: vec![SelectionItem {
                name: format!("{action} preset {slot}"),
                require_explicit_confirmation: true,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SaveQuickDial {
                        slot,
                        dial: dial.clone(),
                    })
                })],
                dismiss_on_select: true,
                ..Default::default()
            }],
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn cancel_quick_dial_menu(&mut self) {
        if self.quick_dial_menu {
            self.set_footer_hint_override(/*items*/ None);
        }
    }

    pub(crate) fn open_quick_dial_menu(&mut self) {
        if self.quick_dials_available() {
            self.bottom_pane.flush_pending_composer_input();
            if !self.bottom_pane.no_modal_or_popup_active() {
                return;
            }
            if !self.local_settings.workspace_enabled(&self.config) {
                self.set_footer_hint_override(Some(Vec::new()));
            }
            self.quick_dial_menu = true;
            self.request_redraw();
        }
    }

    pub(crate) fn handle_quick_dial_menu(&mut self, key: KeyEvent) -> bool {
        if let Some(guard) = self.quick_dial_menu_key_guard.as_mut() {
            if guard.consume(KeyBinding::from_event(key), key.kind) {
                return true;
            }
            if !guard.is_active() {
                self.quick_dial_menu_key_guard = None;
            }
        }
        if !self.quick_dial_menu {
            return false;
        }
        if !self.bottom_pane.no_modal_or_popup_active() {
            self.cancel_quick_dial_menu();
            return false;
        }
        if key.kind != KeyEventKind::Press || key.code == KeyCode::Enter {
            return true;
        }
        self.cancel_quick_dial_menu();
        if self.chat_keymap.chord_hints.leader.is_pressed(key)
            || self.chat_keymap.open_presets.is_pressed(key)
        {
            return true;
        }
        if key.modifiers != KeyModifiers::NONE {
            return false;
        }
        let consumed = match key.code {
            KeyCode::Esc => true,
            KeyCode::Char('n') => {
                self.open_quick_dial_editor(QuickDialEditorTarget::PickSlot, /*dial*/ None);
                true
            }
            KeyCode::Char('s') => {
                self.open_quick_dial_prefills();
                true
            }
            KeyCode::Char('e') => {
                self.open_quick_dials_picker(QuickDialPickerMode::Edit);
                true
            }
            KeyCode::Char('d') => {
                self.open_quick_dials_picker(QuickDialPickerMode::Clear);
                true
            }
            KeyCode::Char(c @ '0'..='9') => {
                if self.quick_dials_available() {
                    let slot = c as u8 - b'0';
                    if self
                        .local_settings
                        .tui
                        .quick_dials
                        .contains_key(&slot.to_string())
                    {
                        self.input_queue.suppress_queue_autosend = true;
                        self.app_event_tx.send(AppEvent::RecallQuickDial {
                            slot,
                            thread_id: self.thread_id(),
                        });
                        self.on_modal_or_popup_closed();
                    } else {
                        self.open_quick_dial_editor(
                            QuickDialEditorTarget::Slot(slot),
                            /*dial*/ None,
                        );
                    }
                }
                true
            }
            _ => false,
        };
        if consumed {
            self.quick_dial_menu_key_guard =
                Some(crate::activation_key_guard::ActivationKeyGuard::new_at(
                    KeyBinding::from_event(key),
                    Instant::now(),
                    /*reported_repeats_only*/ false,
                ));
        }
        consumed
    }
}

fn status_surface_quick_dial<T>(
    command: &str,
    item_ids: Vec<String>,
    suffix: Option<&str>,
    invalid_items_error: &'static str,
) -> Result<QuickDial, &'static str>
where
    T: std::str::FromStr + ToString,
{
    let items = item_ids
        .into_iter()
        .map(|id| {
            id.parse::<T>()
                .map(|item| item.to_string())
                .map_err(|_| invalid_items_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut input = command.to_string();
    if items.is_empty() {
        input.push_str(" --clear");
    } else {
        for item in items {
            input.push(' ');
            input.push_str(&item);
        }
    }
    if let Some(suffix) = suffix {
        input.push(' ');
        input.push_str(suffix);
    }
    let dial = QuickDial { input, label: None };
    dial.validate()?;
    Ok(dial)
}
