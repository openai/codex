//! `ChatWidget` integration points for the `/keymap` picker flow.
//!
//! The picker model, capture view, and edit semantics live in [`crate::keymap_setup`]. This module
//! keeps only the `ChatWidget`-owned responsibilities: opening those views in the bottom pane,
//! routing users back to the right picker row after an edit, and synchronizing the committed
//! keymap config back into the live widget state. Keeping these methods outside `chatwidget.rs`
//! keeps the main transcript/event surface from also owning the `/keymap` navigation details.
//!
//! The important invariant is that any accepted keymap edit must update three places together:
//! the stored `Config.tui_keymap`, the cached copy-response binding used by app-level shortcuts,
//! and the bottom pane's runtime keymap bindings. Updating only one of those would make the UI
//! appear to accept a remap while some handlers still respond to the old keys.

use codex_config::types::TuiKeymap;

use super::ChatWidget;
use crate::app_event::KeymapEditIntent;
use crate::keymap::RuntimeKeymap;
use crate::keymap_setup;

impl ChatWidget {
    /// Opens the root `/keymap` picker using the current `tui.keymap` configuration.
    ///
    /// This validates the persisted keymap before building picker rows because every subsequent
    /// picker screen needs the effective runtime bindings, including preset defaults and user
    /// overrides. If the config is invalid, the user sees the parse error instead of a partial
    /// picker that could commit edits against stale runtime state.
    pub(crate) fn open_keymap_picker(&mut self) {
        match RuntimeKeymap::from_config(&self.config.tui_keymap) {
            Ok(runtime_keymap) => {
                let params = keymap_setup::build_keymap_picker_params(
                    &runtime_keymap,
                    &self.config.tui_keymap,
                );
                self.bottom_pane.show_selection_view(params);
            }
            Err(err) => {
                self.add_error_message(format!("Invalid `tui.keymap` configuration: {err}"));
            }
        }
    }

    /// Opens the per-action menu for one keymap action.
    ///
    /// Callers pass the already-resolved runtime keymap from the app event that selected the
    /// action. Recomputing it here would risk showing a menu for a different config if another
    /// keymap edit was applied between the picker event and this handler.
    pub(crate) fn open_keymap_action_menu(
        &mut self,
        context: String,
        action: String,
        runtime_keymap: &RuntimeKeymap,
    ) {
        let params = keymap_setup::build_keymap_action_menu_params(
            context,
            action,
            runtime_keymap,
            &self.config.tui_keymap,
        );
        self.bottom_pane.show_selection_view(params);
    }

    /// Opens the key-capture view for a set, replace, or alternate-binding edit.
    ///
    /// The capture view owns raw key interpretation, but `ChatWidget` supplies the event sender so
    /// the captured key can come back through the same app-event path as menu selections. Bypassing
    /// that path would skip config persistence and leave the runtime keymap cache unchanged.
    pub(crate) fn open_keymap_capture(
        &mut self,
        context: String,
        action: String,
        intent: KeymapEditIntent,
        runtime_keymap: &RuntimeKeymap,
    ) {
        let view = keymap_setup::build_keymap_capture_view(
            context,
            action,
            intent,
            runtime_keymap,
            self.app_event_tx.clone(),
        );
        self.bottom_pane.show_view(Box::new(view));
        self.request_redraw();
    }

    /// Opens the menu that lets the user choose which existing binding to replace.
    ///
    /// This is only used for actions with multiple effective bindings. The chosen binding is
    /// carried through the subsequent capture intent so replacement edits do not accidentally
    /// collapse alternate bindings that should remain available.
    pub(crate) fn open_keymap_replace_binding_menu(
        &mut self,
        context: String,
        action: String,
        runtime_keymap: &RuntimeKeymap,
    ) {
        let params =
            keymap_setup::build_keymap_replace_binding_menu_params(context, action, runtime_keymap);
        self.bottom_pane.show_selection_view(params);
    }

    /// Returns to the root picker with the edited action selected.
    ///
    /// The preferred path replaces any active keymap picker submenu in place so the bottom-pane
    /// back stack does not accumulate obsolete menus after each edit. If the expected view stack is
    /// no longer active, this falls back to showing a fresh picker rather than dropping the user on
    /// a stale screen.
    pub(crate) fn return_to_keymap_picker(
        &mut self,
        context: &str,
        action: &str,
        runtime_keymap: &RuntimeKeymap,
    ) {
        let params = keymap_setup::build_keymap_picker_params_for_selected_action(
            runtime_keymap,
            &self.config.tui_keymap,
            context,
            action,
        );
        let replaced = self.bottom_pane.replace_active_views_with_selection_view(
            &[
                keymap_setup::KEYMAP_PICKER_VIEW_ID,
                keymap_setup::KEYMAP_ACTION_MENU_VIEW_ID,
                keymap_setup::KEYMAP_REPLACE_BINDING_MENU_VIEW_ID,
            ],
            params,
        );
        if !replaced {
            let params = keymap_setup::build_keymap_picker_params_for_selected_action(
                runtime_keymap,
                &self.config.tui_keymap,
                context,
                action,
            );
            self.bottom_pane.show_selection_view(params);
        }
        self.request_redraw();
    }

    /// Applies a committed keymap edit to the live chat widget.
    ///
    /// The caller is responsible for persisting the config file before invoking this method. This
    /// method updates the in-memory config, app-level copy binding cache, and bottom-pane keymap
    /// bindings as one unit; callers that update only `self.config.tui_keymap` would leave visible
    /// picker state and active key handlers disagreeing until the next restart.
    pub(crate) fn apply_keymap_update(
        &mut self,
        keymap_config: TuiKeymap,
        runtime_keymap: &RuntimeKeymap,
    ) {
        self.config.tui_keymap = keymap_config;
        self.copy_last_response_binding = runtime_keymap.app.copy.clone();
        self.bottom_pane.set_keymap_bindings(runtime_keymap);
        self.request_redraw();
    }
}
