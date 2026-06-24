use std::path::PathBuf;

use super::ChatWidget;
use super::plugin_catalog::marketplace_display_name;
use super::plugin_catalog::marketplace_is_user_configured;
use super::plugin_catalog::marketplace_is_user_configured_git;
use super::plugin_catalog::marketplace_tab_id;
use super::plugin_catalog::marketplace_tab_id_from_path;
use super::plugin_catalog::marketplace_tab_id_matching_saved_id;
use super::plugin_catalog::merge_remote_marketplaces;
use super::plugin_catalog::plugin_detail_hint_line;
use crate::app_event::AppEvent;
use crate::app_event::PluginLocation;
use crate::app_event::PluginRemoteSectionError;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use crate::history_cell;
use crate::key_hint;
use crate::render::renderable::ColumnRenderable;
use codex_app_server_protocol::MarketplaceAddResponse;
use codex_app_server_protocol::MarketplaceRemoveResponse;
use codex_app_server_protocol::MarketplaceUpgradeResponse;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_features::Feature;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::style::Stylize;
use ratatui::text::Line;

pub(super) const PLUGINS_SELECTION_VIEW_ID: &str = "plugins-selection";
pub(super) const ALL_PLUGINS_TAB_ID: &str = "all-plugins";
pub(super) const ADD_MARKETPLACE_TAB_ID: &str = "add-marketplace";

#[derive(Debug, Clone, Default)]
pub(super) struct PluginListFetchState {
    pub(super) cache_cwd: Option<PathBuf>,
    pub(super) in_flight_cwd: Option<PathBuf>,
    pub(super) vertical_section_requested: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PluginInstallAuthFlowState {
    plugin_display_name: String,
    next_app_index: usize,
}

#[derive(Debug, Clone, Default)]
pub(super) enum PluginsCacheState {
    #[default]
    Uninitialized,
    Loading,
    Ready(PluginListResponse),
    Failed(String),
}

impl ChatWidget {
    pub(crate) fn add_plugins_output(&mut self) {
        if !self.config.features.enabled(Feature::Plugins) {
            self.add_info_message(
                "Plugins are disabled.".to_string(),
                Some("Enable the plugins feature to use /plugins.".to_string()),
            );
            return;
        }

        self.plugins_active_tab_id = Some(ALL_PLUGINS_TAB_ID.to_string());
        self.prefetch_plugins();

        match self.plugins_cache_for_current_cwd() {
            PluginsCacheState::Ready(response) => {
                self.open_plugins_popup(&response);
            }
            PluginsCacheState::Failed(err) => {
                self.add_to_history(history_cell::new_error_event(err));
            }
            PluginsCacheState::Loading | PluginsCacheState::Uninitialized => {
                self.open_plugins_loading_popup();
            }
        }
        self.request_redraw();
    }

    pub(crate) fn on_plugins_loaded(
        &mut self,
        cwd: PathBuf,
        result: Result<PluginListResponse, String>,
    ) {
        let request_was_in_flight =
            self.plugins_fetch_state.in_flight_cwd.as_deref() == Some(cwd.as_path());
        if request_was_in_flight {
            self.plugins_fetch_state.in_flight_cwd = None;
        }

        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        let auth_flow_active = self.plugin_install_auth_flow.is_some();
        let should_refresh_plugins_popup = !auth_flow_active
            && (self
                .bottom_pane
                .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
                .is_some()
                || self
                    .bottom_pane
                    .selected_index_for_active_view(PLUGINS_SELECTION_VIEW_ID)
                    .is_some()
                || !matches!(
                    self.plugins_cache_for_current_cwd(),
                    PluginsCacheState::Ready(_)
                ));

        match result {
            Ok(response) => {
                self.plugins_fetch_state.cache_cwd = Some(cwd);
                self.plugin_remote_sections_loading = request_was_in_flight;
                if request_was_in_flight {
                    self.plugin_remote_sections_loaded = false;
                }
                self.plugin_remote_section_errors.clear();
                let active_tab_id = self
                    .plugins_active_tab_id
                    .as_deref()
                    .and_then(|tab_id| {
                        marketplace_tab_id_matching_saved_id(tab_id, &response.marketplaces)
                    })
                    .or_else(|| self.plugins_active_tab_id.clone());
                self.newly_installed_marketplace_tab_id = self
                    .newly_installed_marketplace_tab_id
                    .as_deref()
                    .and_then(|tab_id| {
                        marketplace_tab_id_matching_saved_id(tab_id, &response.marketplaces)
                    });
                self.plugins_active_tab_id = active_tab_id;
                self.plugins_cache = PluginsCacheState::Ready(response.clone());
                if should_refresh_plugins_popup {
                    self.refresh_plugins_popup_if_open(&response);
                }
                self.newly_installed_marketplace_tab_id = None;
            }
            Err(err) => {
                self.plugin_remote_sections_loading = false;
                self.plugin_remote_sections_loaded = false;
                self.plugins_fetch_state.vertical_section_requested = false;
                if should_refresh_plugins_popup {
                    self.plugins_fetch_state.cache_cwd = None;
                    self.plugins_cache = PluginsCacheState::Failed(err.clone());
                    let _ = self.bottom_pane.replace_selection_view_if_active(
                        PLUGINS_SELECTION_VIEW_ID,
                        self.plugins_error_popup_params(&err),
                    );
                }
            }
        }
    }

    pub(crate) fn on_plugin_remote_sections_loaded(
        &mut self,
        cwd: PathBuf,
        marketplaces: Vec<PluginMarketplaceEntry>,
        section_errors: Vec<PluginRemoteSectionError>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        let should_refresh_plugins_popup = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
            .is_some();
        self.plugin_remote_sections_loading = false;
        self.plugin_remote_sections_loaded = true;
        self.plugins_fetch_state.vertical_section_requested = false;
        let refreshed_response = match &mut self.plugins_cache {
            PluginsCacheState::Ready(response)
                if self.plugins_fetch_state.cache_cwd.as_deref() == Some(cwd.as_path()) =>
            {
                merge_remote_marketplaces(response, marketplaces);
                self.plugin_remote_section_errors = section_errors;
                Some(response.clone())
            }
            _ => {
                self.plugin_remote_section_errors = section_errors;
                None
            }
        };

        if let Some(response) = refreshed_response
            && should_refresh_plugins_popup
        {
            self.refresh_plugins_popup_if_open(&response);
        }
    }

    fn prefetch_plugins(&mut self) {
        let cwd = self.config.cwd.to_path_buf();
        if self.plugins_fetch_state.in_flight_cwd.as_deref() == Some(cwd.as_path()) {
            return;
        }

        self.on_plugins_list_fetch_started(cwd.clone());
        self.app_event_tx.send(AppEvent::FetchPluginsList { cwd });
    }

    pub(crate) fn on_plugins_list_fetch_started(&mut self, cwd: PathBuf) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        self.plugins_fetch_state.in_flight_cwd = Some(cwd.clone());
        self.plugins_fetch_state.vertical_section_requested =
            !self.config.features.enabled(Feature::RemotePlugin);
        if self.plugins_fetch_state.cache_cwd.as_deref() != Some(cwd.as_path()) {
            self.plugins_cache = PluginsCacheState::Loading;
        }
    }

    pub(super) fn plugins_cache_for_current_cwd(&self) -> PluginsCacheState {
        if self.plugins_fetch_state.cache_cwd.as_deref() == Some(self.config.cwd.as_path()) {
            self.plugins_cache.clone()
        } else {
            PluginsCacheState::Uninitialized
        }
    }

    fn open_plugins_loading_popup(&mut self) {
        if !self.bottom_pane.replace_selection_view_if_active(
            PLUGINS_SELECTION_VIEW_ID,
            self.plugins_loading_popup_params(),
        ) {
            self.bottom_pane
                .show_selection_view(self.plugins_loading_popup_params());
        }
    }

    fn open_plugins_popup(&mut self, response: &PluginListResponse) {
        self.plugins_active_tab_id = Some(ALL_PLUGINS_TAB_ID.to_string());
        self.bottom_pane
            .show_selection_view(self.plugins_popup_params(
                response,
                self.plugins_active_tab_id.clone(),
                /*initial_selected_idx*/ None,
            ));
    }

    pub(crate) fn open_plugins_list(&mut self, cwd: PathBuf, response: PluginListResponse) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        let response = match self.plugins_cache_for_current_cwd() {
            PluginsCacheState::Ready(current_response) => current_response,
            PluginsCacheState::Uninitialized
            | PluginsCacheState::Loading
            | PluginsCacheState::Failed(_) => response,
        };
        self.plugins_fetch_state.cache_cwd = Some(cwd);
        self.plugins_cache = PluginsCacheState::Ready(response.clone());
        let active_tab_id = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
            .map(str::to_string)
            .or_else(|| self.plugins_active_tab_id.clone())
            .or_else(|| Some(ALL_PLUGINS_TAB_ID.to_string()));
        self.plugins_active_tab_id = active_tab_id.clone();
        let params =
            self.plugins_popup_params(&response, active_tab_id, /*initial_selected_idx*/ None);
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
        {
            self.open_plugins_popup(&response);
        }
    }

    pub(crate) fn open_marketplace_add_prompt(&mut self) {
        self.plugins_active_tab_id = Some(ADD_MARKETPLACE_TAB_ID.to_string());
        let tx = self.app_event_tx.clone();
        let cwd = self.config.cwd.to_path_buf();
        let view = CustomPromptView::new(
            "Add marketplace".to_string(),
            "owner/repo, git URL, or local marketplace path".to_string(),
            String::new(),
            Some("Examples: owner/repo, git URL, ./marketplace".to_string()),
            Box::new(move |source: String| {
                let source = source.trim().to_string();
                if source.is_empty() {
                    return;
                }
                tx.send(AppEvent::OpenMarketplaceAddLoading {
                    source: source.clone(),
                });
                tx.send(AppEvent::FetchMarketplaceAdd {
                    cwd: cwd.clone(),
                    source,
                });
            }),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn open_marketplace_add_loading_popup(&mut self, _source: &str) {
        self.plugins_active_tab_id = Some(ADD_MARKETPLACE_TAB_ID.to_string());
        let params = self.marketplace_add_loading_popup_params();
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
        {
            self.bottom_pane
                .show_selection_view(self.marketplace_add_loading_popup_params());
        }
    }

    pub(crate) fn open_marketplace_upgrade_loading_popup(
        &mut self,
        marketplace_name: Option<&str>,
    ) {
        self.plugins_active_tab_id = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
            .map(str::to_string)
            .or_else(|| self.plugins_active_tab_id.clone());
        let params = self.marketplace_upgrade_loading_popup_params(marketplace_name);
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
        {
            self.bottom_pane.show_selection_view(
                self.marketplace_upgrade_loading_popup_params(marketplace_name),
            );
        }
    }

    pub(crate) fn open_marketplace_remove_confirmation(
        &mut self,
        marketplace_name: String,
        marketplace_display_name: String,
    ) {
        self.plugins_active_tab_id = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
            .map(str::to_string)
            .or_else(|| self.plugins_active_tab_id.clone());

        let PluginsCacheState::Ready(plugins_response) = self.plugins_cache_for_current_cwd()
        else {
            return;
        };

        let params = self.marketplace_remove_confirmation_popup_params(
            &plugins_response,
            marketplace_name.clone(),
            marketplace_display_name.clone(),
        );
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
        {
            self.bottom_pane.show_selection_view(
                self.marketplace_remove_confirmation_popup_params(
                    &plugins_response,
                    marketplace_name,
                    marketplace_display_name,
                ),
            );
        }
    }

    pub(crate) fn open_marketplace_remove_loading_popup(&mut self, marketplace_display_name: &str) {
        let params = self.marketplace_remove_loading_popup_params(marketplace_display_name);
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
        {
            self.bottom_pane.show_selection_view(
                self.marketplace_remove_loading_popup_params(marketplace_display_name),
            );
        }
    }

    pub(crate) fn open_plugin_detail_loading_popup(&mut self, plugin_display_name: &str) {
        self.plugins_active_tab_id = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
            .map(str::to_string)
            .or_else(|| self.plugins_active_tab_id.clone());
        let params = self.plugin_detail_loading_popup_params(plugin_display_name);
        let _ = self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params);
    }

    pub(crate) fn open_plugin_install_loading_popup(&mut self, plugin_display_name: &str) {
        let params = self.plugin_install_loading_popup_params(plugin_display_name);
        let _ = self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params);
    }

    pub(crate) fn open_plugin_uninstall_loading_popup(&mut self, plugin_display_name: &str) {
        let params = self.plugin_uninstall_loading_popup_params(plugin_display_name);
        let _ = self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params);
    }

    pub(crate) fn on_plugin_detail_loaded(
        &mut self,
        cwd: PathBuf,
        result: Result<PluginReadResponse, String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        let plugins_response = match self.plugins_cache_for_current_cwd() {
            PluginsCacheState::Ready(response) => Some(response),
            _ => None,
        };

        match result {
            Ok(response) => {
                if let Some(plugins_response) = plugins_response {
                    let _ = self.bottom_pane.replace_selection_view_if_active(
                        PLUGINS_SELECTION_VIEW_ID,
                        self.plugin_detail_popup_params(&plugins_response, &response.plugin),
                    );
                }
            }
            Err(err) => {
                let _ = self.bottom_pane.replace_selection_view_if_active(
                    PLUGINS_SELECTION_VIEW_ID,
                    self.plugin_detail_error_popup_params(&err, plugins_response.as_ref()),
                );
            }
        }
    }

    pub(crate) fn on_plugin_install_loaded(
        &mut self,
        cwd: PathBuf,
        _location: PluginLocation,
        _plugin_name: String,
        plugin_display_name: String,
        result: Result<PluginInstallResponse, String>,
    ) -> bool {
        if self.config.cwd.as_path() != cwd.as_path() {
            return true;
        }

        match result {
            Ok(response) => {
                self.plugin_install_apps_needing_auth = response.apps_needing_auth;
                self.plugin_install_auth_flow = None;
                if self.plugin_install_apps_needing_auth.is_empty() {
                    self.add_info_message(
                        format!("Installed {plugin_display_name} plugin."),
                        Some("No additional app authentication is required.".to_string()),
                    );
                    true
                } else {
                    let app_names = self
                        .plugin_install_apps_needing_auth
                        .iter()
                        .map(|app| app.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.add_info_message(
                        format!("Installed {plugin_display_name} plugin."),
                        Some(format!(
                            "{} app(s) still need authentication: {app_names}",
                            self.plugin_install_apps_needing_auth.len()
                        )),
                    );
                    self.plugin_install_auth_flow = Some(PluginInstallAuthFlowState {
                        plugin_display_name,
                        next_app_index: 0,
                    });
                    self.open_plugin_install_auth_popup();
                    false
                }
            }
            Err(err) => {
                self.plugin_install_apps_needing_auth.clear();
                self.plugin_install_auth_flow = None;
                let plugins_response = match self.plugins_cache_for_current_cwd() {
                    PluginsCacheState::Ready(response) => Some(response),
                    _ => None,
                };
                let _ = self.bottom_pane.replace_selection_view_if_active(
                    PLUGINS_SELECTION_VIEW_ID,
                    self.plugin_detail_error_popup_params(&err, plugins_response.as_ref()),
                );
                true
            }
        }
    }

    pub(crate) fn on_marketplace_add_loaded(
        &mut self,
        cwd: PathBuf,
        _source: String,
        result: Result<MarketplaceAddResponse, String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        match result {
            Ok(response) => {
                let marketplace_tab_id = marketplace_tab_id_from_path(&response.installed_root);
                self.plugins_active_tab_id = Some(marketplace_tab_id.clone());
                self.newly_installed_marketplace_tab_id =
                    (!response.already_added).then_some(marketplace_tab_id);
                let message = if response.already_added {
                    format!(
                        "Marketplace {} is already added.",
                        response.marketplace_name
                    )
                } else {
                    format!("Added marketplace {}.", response.marketplace_name)
                };
                self.add_info_message(
                    message,
                    Some(format!(
                        "Marketplace root: {}",
                        response.installed_root.as_path().display()
                    )),
                );
            }
            Err(_) => {
                self.plugins_active_tab_id = Some(ADD_MARKETPLACE_TAB_ID.to_string());
                let params = self.marketplace_add_error_popup_params();
                if !self
                    .bottom_pane
                    .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
                {
                    self.bottom_pane
                        .show_selection_view(self.marketplace_add_error_popup_params());
                }
            }
        }
    }

    pub(crate) fn on_marketplace_remove_loaded(
        &mut self,
        cwd: PathBuf,
        marketplace_name: String,
        marketplace_display_name: String,
        result: Result<MarketplaceRemoveResponse, String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        match result {
            Ok(response) => {
                self.plugins_active_tab_id = Some(ALL_PLUGINS_TAB_ID.to_string());
                self.add_info_message(
                    format!("Removed marketplace {marketplace_display_name}."),
                    Some(match response.installed_root {
                        Some(installed_root) => {
                            format!("Marketplace root: {}", installed_root.as_path().display())
                        }
                        None => format!(
                            "Removed marketplace config for {}.",
                            response.marketplace_name
                        ),
                    }),
                );
            }
            Err(_) => {
                let params = self.marketplace_remove_error_popup_params(
                    &marketplace_name,
                    &marketplace_display_name,
                );
                if !self
                    .bottom_pane
                    .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
                {
                    self.bottom_pane.show_selection_view(
                        self.marketplace_remove_error_popup_params(
                            &marketplace_name,
                            &marketplace_display_name,
                        ),
                    );
                }
            }
        }
    }

    pub(crate) fn on_marketplace_upgrade_loaded(
        &mut self,
        cwd: PathBuf,
        result: Result<MarketplaceUpgradeResponse, String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        match result {
            Ok(response) => {
                if response.upgraded_roots.len() == 1 {
                    self.plugins_active_tab_id =
                        Some(marketplace_tab_id_from_path(&response.upgraded_roots[0]));
                }

                let selected_count = response.selected_marketplaces.len();
                let upgraded_count = response.upgraded_roots.len();
                let error_count = response.errors.len();
                if selected_count == 0 {
                    self.add_info_message(
                        "No configured Git marketplaces to upgrade.".to_string(),
                        Some("Only configured Git marketplaces can be upgraded.".to_string()),
                    );
                    return;
                }

                if upgraded_count == 0 && error_count == 0 {
                    let message = if selected_count == 1 {
                        format!(
                            "Marketplace {} is already up to date.",
                            response.selected_marketplaces[0]
                        )
                    } else {
                        format!(
                            "Checked {selected_count} marketplaces; all are already up to date."
                        )
                    };
                    self.add_info_message(
                        message,
                        Some(format!(
                            "Checked: {}",
                            response.selected_marketplaces.join(", ")
                        )),
                    );
                    return;
                }

                if upgraded_count > 0 {
                    let noun = if upgraded_count == 1 {
                        "marketplace"
                    } else {
                        "marketplaces"
                    };
                    self.add_info_message(
                        format!("Upgraded {upgraded_count} {noun}."),
                        Some(format!(
                            "Updated roots: {}",
                            response
                                .upgraded_roots
                                .iter()
                                .map(|root| root.as_path().display().to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                    );
                }

                if error_count > 0 {
                    let noun = if error_count == 1 {
                        "marketplace"
                    } else {
                        "marketplaces"
                    };
                    self.add_error_message(format!(
                        "Failed to upgrade {error_count} {noun}: {}",
                        response
                            .errors
                            .iter()
                            .map(|err| format!("{}: {}", err.marketplace_name, err.message))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ));
                }
            }
            Err(err) => {
                self.add_error_message(err);
            }
        }
    }

    pub(crate) fn handle_plugins_popup_key_event(&mut self, key_event: KeyEvent) -> bool {
        let remove_marketplace = key_hint::ctrl(KeyCode::Char('r')).is_press(key_event);
        let upgrade_marketplace = key_hint::ctrl(KeyCode::Char('u')).is_press(key_event);
        if !remove_marketplace && !upgrade_marketplace {
            return false;
        }

        let Some(active_tab_id) = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
        else {
            return false;
        };
        let PluginsCacheState::Ready(plugins_response) = self.plugins_cache_for_current_cwd()
        else {
            return false;
        };
        let Some(marketplace) = plugins_response.marketplaces.iter().find(|marketplace| {
            marketplace_tab_id(marketplace) == active_tab_id
                && marketplace_is_user_configured(&self.config, &marketplace.name)
        }) else {
            return false;
        };

        if remove_marketplace {
            self.open_marketplace_remove_confirmation(
                marketplace.name.clone(),
                marketplace_display_name(marketplace),
            );
            return true;
        }
        if marketplace.path.is_none()
            || !marketplace_is_user_configured_git(&self.config, &marketplace.name)
        {
            return false;
        }
        if key_event.kind != KeyEventKind::Press {
            return true;
        }

        let cwd = self.config.cwd.to_path_buf();
        let marketplace_name = Some(marketplace.name.clone());
        self.open_marketplace_upgrade_loading_popup(marketplace_name.as_deref());
        self.app_event_tx
            .send(AppEvent::OpenMarketplaceUpgradeLoading {
                marketplace_name: marketplace_name.clone(),
            });
        self.app_event_tx.send(AppEvent::FetchMarketplaceUpgrade {
            cwd,
            marketplace_name,
        });
        true
    }

    pub(crate) fn on_plugin_enabled_set(
        &mut self,
        cwd: PathBuf,
        plugin_id: String,
        enabled: bool,
        result: Result<(), String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        if let Err(err) = result {
            self.add_error_message(format!(
                "Failed to update plugin config for {plugin_id}: {err}"
            ));
            if let PluginsCacheState::Ready(response) = self.plugins_cache_for_current_cwd() {
                self.refresh_plugins_popup_if_open(&response);
            }
            return;
        }

        let refreshed_response = match &mut self.plugins_cache {
            PluginsCacheState::Ready(response)
                if self.plugins_fetch_state.cache_cwd.as_deref() == Some(cwd.as_path()) =>
            {
                for plugin in response
                    .marketplaces
                    .iter_mut()
                    .flat_map(|marketplace| marketplace.plugins.iter_mut())
                    .filter(|plugin| plugin.id == plugin_id)
                {
                    plugin.enabled = enabled;
                }
                Some(response.clone())
            }
            _ => None,
        };

        if let Some(response) = refreshed_response {
            self.refresh_plugins_popup_if_open(&response);
        }
    }

    pub(crate) fn on_plugin_uninstall_loaded(
        &mut self,
        cwd: PathBuf,
        plugin_display_name: String,
        result: Result<PluginUninstallResponse, String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        match result {
            Ok(_response) => {
                self.plugin_install_apps_needing_auth.clear();
                self.plugin_install_auth_flow = None;
                self.add_info_message(
                    format!("Uninstalled {plugin_display_name} plugin."),
                    Some("Bundled apps remain installed.".to_string()),
                );
            }
            Err(err) => {
                let plugins_response = match self.plugins_cache_for_current_cwd() {
                    PluginsCacheState::Ready(response) => Some(response),
                    _ => None,
                };
                let _ = self.bottom_pane.replace_selection_view_if_active(
                    PLUGINS_SELECTION_VIEW_ID,
                    self.plugin_detail_error_popup_params(&err, plugins_response.as_ref()),
                );
            }
        }
    }

    pub(crate) fn advance_plugin_install_auth_flow(&mut self) {
        let should_finish = {
            let Some(flow) = self.plugin_install_auth_flow.as_mut() else {
                return;
            };
            flow.next_app_index += 1;
            flow.next_app_index >= self.plugin_install_apps_needing_auth.len()
        };

        if should_finish {
            self.finish_plugin_install_auth_flow(/*abandoned*/ false);
            return;
        }

        self.open_plugin_install_auth_popup();
    }

    pub(crate) fn abandon_plugin_install_auth_flow(&mut self) {
        self.finish_plugin_install_auth_flow(/*abandoned*/ true);
    }

    fn open_plugin_install_auth_popup(&mut self) {
        let Some(params) = self.plugin_install_auth_popup_params() else {
            self.finish_plugin_install_auth_flow(/*abandoned*/ false);
            return;
        };
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
            && let Some(params) = self.plugin_install_auth_popup_params()
        {
            self.bottom_pane.show_selection_view(params);
        }
    }

    fn plugin_install_auth_popup_params(&self) -> Option<SelectionViewParams> {
        let flow = self.plugin_install_auth_flow.as_ref()?;
        let app = self
            .plugin_install_apps_needing_auth
            .get(flow.next_app_index)?;
        let total = self.plugin_install_apps_needing_auth.len();
        let current = flow.next_app_index + 1;
        let is_installed = self.plugin_install_auth_app_is_installed(app.id.as_str());
        let status_label = if is_installed {
            "Already installed in this session."
        } else {
            "Install the required Apps in ChatGPT to continue:"
        };
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(
            format!("{} plugin installed.", flow.plugin_display_name).bold(),
        ));
        header.push(Line::from(
            format!("App setup {current}/{total}: {}", app.name).dim(),
        ));
        header.push(Line::from(status_label.dim()));

        let mut items = Vec::new();

        if let Some(install_url) = app.install_url.clone() {
            let install_label = if is_installed {
                "Manage on ChatGPT"
            } else {
                "Install on ChatGPT"
            };
            items.push(SelectionItem {
                name: install_label.to_string(),
                description: Some("Open the ChatGPT app management page".to_string()),
                selected_description: Some("Open the app page in your browser.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenUrlInBrowser {
                        url: install_url.clone(),
                    });
                })],
                ..Default::default()
            });
        } else {
            items.push(SelectionItem {
                name: "ChatGPT apps link unavailable".to_string(),
                description: Some("This app did not provide an install/manage URL.".to_string()),
                is_disabled: true,
                ..Default::default()
            });
        }

        if is_installed {
            items.push(SelectionItem {
                name: "Continue".to_string(),
                description: Some("This app is already installed.".to_string()),
                selected_description: Some("Advance to the next app.".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::PluginInstallAuthAdvance {
                        refresh_connectors: false,
                    });
                })],
                ..Default::default()
            });
        } else {
            items.push(SelectionItem {
                name: "I've installed it".to_string(),
                description: Some(
                    "Trust your confirmation and continue to the next app.".to_string(),
                ),
                selected_description: Some(
                    "Continue without waiting for refresh to complete.".to_string(),
                ),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::PluginInstallAuthAdvance {
                        refresh_connectors: true,
                    });
                })],
                ..Default::default()
            });
        }

        items.push(SelectionItem {
            name: "Skip remaining app setup".to_string(),
            description: Some("Stop this follow-up flow for this plugin.".to_string()),
            selected_description: Some("Abandon remaining required app setup.".to_string()),
            actions: vec![Box::new(|tx| {
                tx.send(AppEvent::PluginInstallAuthAbandon);
            })],
            ..Default::default()
        });

        Some(SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(plugin_detail_hint_line()),
            items,
            col_width_mode: ColumnWidthMode::AutoAllRows,
            ..Default::default()
        })
    }

    fn plugin_install_auth_app_is_installed(&self, app_id: &str) -> bool {
        self.connectors_for_mentions().is_some_and(|connectors| {
            connectors
                .iter()
                .any(|connector| connector.id == app_id && connector.is_accessible)
        })
    }

    fn finish_plugin_install_auth_flow(&mut self, abandoned: bool) {
        let Some(flow) = self.plugin_install_auth_flow.take() else {
            return;
        };
        self.plugin_install_apps_needing_auth.clear();
        if abandoned {
            self.add_info_message(
                format!(
                    "Skipped remaining app setup for {} plugin.",
                    flow.plugin_display_name
                ),
                Some("The plugin may not be usable until required apps are installed.".to_string()),
            );
        } else {
            self.add_info_message(
                format!(
                    "Completed app setup flow for {} plugin.",
                    flow.plugin_display_name
                ),
                Some("You can now continue managing plugins from /plugins.".to_string()),
            );
        }

        let plugins_response = match self.plugins_cache_for_current_cwd() {
            PluginsCacheState::Ready(response) => Some(response),
            _ => None,
        };
        if let Some(plugins_response) = plugins_response {
            let tab_id = self.plugins_active_tab_id.clone();
            let _ = self.bottom_pane.replace_selection_view_if_active(
                PLUGINS_SELECTION_VIEW_ID,
                self.plugins_popup_params(
                    &plugins_response,
                    tab_id,
                    /*initial_selected_idx*/ None,
                ),
            );
        }
    }

    fn refresh_plugins_popup_if_open(&mut self, response: &PluginListResponse) {
        let active_tab_id = self
            .bottom_pane
            .active_tab_id_for_active_view(PLUGINS_SELECTION_VIEW_ID)
            .map(str::to_string)
            .or_else(|| self.plugins_active_tab_id.clone());
        let selected_idx = self
            .bottom_pane
            .selected_index_for_active_view(PLUGINS_SELECTION_VIEW_ID);
        self.plugins_active_tab_id = active_tab_id.clone();
        let _ = self.bottom_pane.replace_selection_view_if_active(
            PLUGINS_SELECTION_VIEW_ID,
            self.plugins_popup_params(response, active_tab_id, selected_idx),
        );
    }
}
