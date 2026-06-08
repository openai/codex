use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::app_event::PluginLocation;
use crate::app_event::PluginRemoteSectionError;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionTab;
use crate::bottom_pane::SelectionToggle;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use crate::history_cell;
use crate::key_hint;
use crate::legacy_core::config::Config;
use crate::motion::MotionMode;
use crate::motion::shimmer_text;
use crate::onboarding::mark_url_hyperlink;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;
use codex_app_server_protocol::MarketplaceAddResponse;
use codex_app_server_protocol::MarketplaceLoadErrorInfo;
use codex_app_server_protocol::MarketplaceRemoveResponse;
use codex_app_server_protocol::MarketplaceUpgradeResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginShareContext;
use codex_app_server_protocol::PluginShareDiscoverability;
use codex_app_server_protocol::PluginSharePrincipal;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_core_plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthStr;

const PLUGINS_SELECTION_VIEW_ID: &str = "plugins-selection";
const ALL_PLUGINS_TAB_ID: &str = "all-plugins";
const INSTALLED_PLUGINS_TAB_ID: &str = "installed-plugins";
const MARKETPLACE_TAB_ID_PREFIX: &str = "marketplace:";
const OPENAI_CURATED_TAB_ID: &str = "marketplace:openai-curated";
const ADD_MARKETPLACE_TAB_ID: &str = "add-marketplace";
const PERSONAL_MARKETPLACE_RELATIVE_PATH: &str = ".agents/plugins/marketplace.json";
const WORKSPACE_SECTION_MARKETPLACE_NAMES: &[&str] = &[REMOTE_WORKSPACE_MARKETPLACE_NAME];
const SHARED_WITH_ME_SECTION_MARKETPLACE_NAMES: &[&str] = &[
    REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME,
    REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME,
    REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME,
];
const WORKSPACE_SECTION_TAB_IDS: &[&str] = &[
    "marketplace:workspace-directory",
    "remote-loading:workspace-loading",
    "remote-empty:workspace",
    "remote-error:workspace",
];
const SHARED_WITH_ME_SECTION_TAB_IDS: &[&str] = &[
    "marketplace:workspace-shared-with-me",
    "marketplace:workspace-shared-with-me-private",
    "marketplace:workspace-shared-with-me-unlisted",
    "remote-loading:shared-with-me-loading",
    "remote-empty:shared-with-me",
    "remote-error:shared-with-me",
];
const WORKSPACE_SECTION_TAB_ORDER: u8 = 0;
const SHARED_WITH_ME_SECTION_TAB_ORDER: u8 = 1;
const SHARED_WITH_ME_LINK_SECTION_TAB_ORDER: u8 = 2;
const LOCAL_MARKETPLACE_TAB_ORDER: u8 = 3;
const OTHER_MARKETPLACE_TAB_ORDER: u8 = 4;
const WORKSPACE_SECTION_FALLBACK_TAB_ORDER: u8 = 5;
const SHARED_WITH_ME_SECTION_FALLBACK_TAB_ORDER: u8 = 6;
const PLUGIN_ROW_PREFIX_WIDTH: usize = 6;
const LOADING_ANIMATION_DELAY: Duration = Duration::from_secs(1);
const LOADING_ANIMATION_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Default)]
pub(super) struct PluginListFetchState {
    pub(super) cache_cwd: Option<PathBuf>,
    pub(super) in_flight_cwd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(super) struct PluginInstallAuthFlowState {
    plugin_display_name: String,
    next_app_index: usize,
}

struct DelayedLoadingHeader {
    started_at: Instant,
    frame_requester: FrameRequester,
    animations_enabled: bool,
    loading_text: String,
    note: Option<String>,
}

impl DelayedLoadingHeader {
    fn new(
        frame_requester: FrameRequester,
        animations_enabled: bool,
        loading_text: String,
        note: Option<String>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            frame_requester,
            animations_enabled,
            loading_text,
            note,
        }
    }
}

impl Renderable for DelayedLoadingHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let mut lines = Vec::with_capacity(3);
        lines.push(Line::from("Plugins".bold()));

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.started_at);
        if elapsed < LOADING_ANIMATION_DELAY {
            self.frame_requester
                .schedule_frame_in(LOADING_ANIMATION_DELAY - elapsed);
            lines.push(Line::from(self.loading_text.as_str().dim()));
        } else if self.animations_enabled {
            self.frame_requester
                .schedule_frame_in(LOADING_ANIMATION_INTERVAL);
            lines.push(Line::from(shimmer_text(
                self.loading_text.as_str(),
                MotionMode::Animated,
            )));
        } else {
            lines.push(Line::from(self.loading_text.as_str().dim()));
        }

        if let Some(note) = &self.note {
            lines.push(Line::from(note.as_str().dim()));
        }

        Paragraph::new(lines).render_ref(area, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        2 + u16::from(self.note.is_some())
    }
}

const APPS_HELP_ARTICLE_URL: &str = "https://help.openai.com/en/articles/11487775-apps-in-chatgpt";

struct PluginDisclosureLine {
    line: Line<'static>,
}

impl Renderable for PluginDisclosureLine {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.line.clone())
            .wrap(Wrap { trim: false })
            .render(area, buf);
        mark_url_hyperlink(buf, area, APPS_HELP_ARTICLE_URL);
    }

    fn desired_height(&self, width: u16) -> u16 {
        Paragraph::new(self.line.clone())
            .wrap(Wrap { trim: false })
            .line_count(width)
            .try_into()
            .unwrap_or(u16::MAX)
    }
}

#[derive(Debug, Clone, Default)]
pub(super) enum PluginsCacheState {
    #[default]
    Uninitialized,
    Loading,
    Ready(PluginListResponse),
    Failed(String),
}

#[derive(Debug, Clone)]
struct PreferredLocalPluginSource {
    remote_plugin_id: String,
    marketplace_path: AbsolutePathBuf,
    plugin_name: String,
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
        if self.plugins_fetch_state.cache_cwd.as_deref() != Some(cwd.as_path()) {
            self.plugins_cache = PluginsCacheState::Loading;
        }
    }

    fn plugins_cache_for_current_cwd(&self) -> PluginsCacheState {
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
                    self.plugin_action_error_popup_params(
                        "Failed to install plugin.",
                        "Plugin install failed",
                        &err,
                        plugins_response.as_ref(),
                    ),
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
                self.plugins_fetch_state.cache_cwd = None;
                self.plugins_cache = PluginsCacheState::Loading;
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
                self.plugins_fetch_state.cache_cwd = None;
                self.plugins_cache = PluginsCacheState::Loading;
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
                self.return_to_cached_plugins_popup_if_open();
            }
            Err(err) => {
                let plugins_response = match self.plugins_cache_for_current_cwd() {
                    PluginsCacheState::Ready(response) => Some(response),
                    _ => None,
                };
                let _ = self.bottom_pane.replace_selection_view_if_active(
                    PLUGINS_SELECTION_VIEW_ID,
                    self.plugin_action_error_popup_params(
                        "Failed to uninstall plugin.",
                        "Plugin uninstall failed",
                        &err,
                        plugins_response.as_ref(),
                    ),
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

    fn return_to_cached_plugins_popup_if_open(&mut self) {
        if let PluginsCacheState::Ready(response) = self.plugins_cache_for_current_cwd() {
            self.refresh_plugins_popup_if_open(&response);
        }
    }

    fn plugins_loading_popup_params(&self) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(DelayedLoadingHeader::new(
                self.frame_requester.clone(),
                self.config.animations,
                "Loading available plugins...".to_string(),
                Some("This updates when the marketplace list is ready.".to_string()),
            )),
            items: vec![SelectionItem {
                name: "Loading plugins...".to_string(),
                description: Some("This updates when the marketplace list is ready.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn marketplace_add_loading_popup_params(&self) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(DelayedLoadingHeader::new(
                self.frame_requester.clone(),
                self.config.animations,
                "Adding marketplace...".to_string(),
                /*note*/ None,
            )),
            items: vec![SelectionItem {
                name: "Adding marketplace...".to_string(),
                description: Some(
                    "This updates when marketplace installation completes.".to_string(),
                ),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn marketplace_remove_confirmation_popup_params(
        &self,
        plugins_response: &PluginListResponse,
        marketplace_name: String,
        marketplace_display_name: String,
    ) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(
            format!("Remove {marketplace_display_name} marketplace?").dim(),
        ));
        header.push(Line::from(
            "This removes the configured marketplace from Codex.".dim(),
        ));

        let cwd_for_remove = self.config.cwd.to_path_buf();
        let cwd_for_cancel = self.config.cwd.to_path_buf();
        let cwd_for_on_cancel = self.config.cwd.to_path_buf();
        let plugins_response_for_cancel = plugins_response.clone();
        let plugins_response_for_on_cancel = plugins_response.clone();

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(Line::from(vec![
                Span::from(key_hint::plain(KeyCode::Enter)),
                " select".dim(),
                " · ".into(),
                "esc close".dim(),
            ])),
            items: vec![
                SelectionItem {
                    name: "Remove marketplace".to_string(),
                    description: Some(
                        "Remove this marketplace from the available plugin list.".to_string(),
                    ),
                    selected_description: Some(
                        "Remove this marketplace from the available plugin list.".to_string(),
                    ),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenMarketplaceRemoveLoading {
                            marketplace_display_name: marketplace_display_name.clone(),
                        });
                        tx.send(AppEvent::FetchMarketplaceRemove {
                            cwd: cwd_for_remove.clone(),
                            marketplace_name: marketplace_name.clone(),
                            marketplace_display_name: marketplace_display_name.clone(),
                        });
                    })],
                    ..Default::default()
                },
                SelectionItem {
                    name: "Back to plugins".to_string(),
                    description: Some("Keep this marketplace installed.".to_string()),
                    selected_description: Some("Keep this marketplace installed.".to_string()),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenPluginsList {
                            cwd: cwd_for_cancel.clone(),
                            response: plugins_response_for_cancel.clone(),
                        });
                    })],
                    ..Default::default()
                },
            ],
            on_cancel: Some(Box::new(move |tx| {
                tx.send(AppEvent::OpenPluginsList {
                    cwd: cwd_for_on_cancel.clone(),
                    response: plugins_response_for_on_cancel.clone(),
                });
            })),
            ..Default::default()
        }
    }

    fn marketplace_remove_loading_popup_params(
        &self,
        marketplace_display_name: &str,
    ) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(
            format!("Removing {marketplace_display_name}...").dim(),
        ));

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            items: vec![SelectionItem {
                name: "Removing marketplace...".to_string(),
                description: Some("This updates when marketplace removal completes.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn marketplace_upgrade_loading_popup_params(
        &self,
        marketplace_name: Option<&str>,
    ) -> SelectionViewParams {
        let loading_text = marketplace_name
            .map(|name| format!("Upgrading {name} marketplace..."))
            .unwrap_or_else(|| "Upgrading marketplaces...".to_string());
        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(DelayedLoadingHeader::new(
                self.frame_requester.clone(),
                self.config.animations,
                loading_text.clone(),
                /*note*/ None,
            )),
            items: vec![SelectionItem {
                name: loading_text,
                description: Some("This updates when marketplace upgrade completes.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn plugin_detail_loading_popup_params(&self, plugin_display_name: &str) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(DelayedLoadingHeader::new(
                self.frame_requester.clone(),
                self.config.animations,
                format!("Loading details for {plugin_display_name}..."),
                /*note*/ None,
            )),
            items: vec![SelectionItem {
                name: "Loading plugin details...".to_string(),
                description: Some("This updates when plugin details load.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn plugin_install_loading_popup_params(
        &self,
        plugin_display_name: &str,
    ) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(
            format!("Installing {plugin_display_name}...").dim(),
        ));

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            items: vec![SelectionItem {
                name: "Installing plugin...".to_string(),
                description: Some("This updates when plugin installation completes.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn plugin_uninstall_loading_popup_params(
        &self,
        plugin_display_name: &str,
    ) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(
            format!("Uninstalling {plugin_display_name}...").dim(),
        ));

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            items: vec![SelectionItem {
                name: "Uninstalling plugin...".to_string(),
                description: Some("This updates when the plugin removal completes.".to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn plugins_error_popup_params(&self, err: &str) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from("Failed to load plugins.".dim()));

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            items: vec![SelectionItem {
                name: "Plugin marketplace unavailable".to_string(),
                description: Some(err.to_string()),
                is_disabled: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn marketplace_add_error_popup_params(&self) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from("Failed to add marketplace.".dim()));

        let mut items = vec![
            SelectionItem {
                name: "Marketplace add failed".to_string(),
                description: Some(
                    "Failed to add marketplace from the provided source.".to_string(),
                ),
                is_disabled: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Try again".to_string(),
                description: Some("Enter a marketplace source.".to_string()),
                selected_description: Some("Enter a marketplace source.".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenMarketplaceAddPrompt);
                })],
                ..Default::default()
            },
        ];

        if let PluginsCacheState::Ready(plugins_response) = self.plugins_cache_for_current_cwd() {
            let cwd = self.config.cwd.to_path_buf();
            items.push(SelectionItem {
                name: "Back to plugins".to_string(),
                description: Some("Return to the plugin list.".to_string()),
                selected_description: Some("Return to the plugin list.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenPluginsList {
                        cwd: cwd.clone(),
                        response: plugins_response.clone(),
                    });
                })],
                ..Default::default()
            });
        }

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(plugin_detail_hint_line()),
            items,
            ..Default::default()
        }
    }

    fn marketplace_remove_error_popup_params(
        &self,
        marketplace_name: &str,
        marketplace_display_name: &str,
    ) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from("Failed to remove marketplace.".dim()));

        let marketplace_name = marketplace_name.to_string();
        let marketplace_display_name = marketplace_display_name.to_string();
        let mut items = vec![
            SelectionItem {
                name: "Marketplace removal failed".to_string(),
                description: Some("Failed to remove the selected marketplace.".to_string()),
                is_disabled: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Try again".to_string(),
                description: Some("Review the confirmation prompt again.".to_string()),
                selected_description: Some("Review the confirmation prompt again.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenMarketplaceRemoveConfirm {
                        marketplace_name: marketplace_name.clone(),
                        marketplace_display_name: marketplace_display_name.clone(),
                    });
                })],
                ..Default::default()
            },
        ];

        if let PluginsCacheState::Ready(plugins_response) = self.plugins_cache_for_current_cwd() {
            let cwd = self.config.cwd.to_path_buf();
            items.push(SelectionItem {
                name: "Back to plugins".to_string(),
                description: Some("Return to the plugin list.".to_string()),
                selected_description: Some("Return to the plugin list.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenPluginsList {
                        cwd: cwd.clone(),
                        response: plugins_response.clone(),
                    });
                })],
                ..Default::default()
            });
        }

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(plugin_detail_hint_line()),
            items,
            ..Default::default()
        }
    }

    fn plugin_detail_error_popup_params(
        &self,
        err: &str,
        plugins_response: Option<&PluginListResponse>,
    ) -> SelectionViewParams {
        self.plugin_action_error_popup_params(
            "Failed to load plugin details.",
            "Plugin detail unavailable",
            err,
            plugins_response,
        )
    }

    fn plugin_action_error_popup_params(
        &self,
        title: &str,
        item_name: &str,
        err: &str,
        plugins_response: Option<&PluginListResponse>,
    ) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(title.to_string().dim()));

        let mut items = vec![SelectionItem {
            name: item_name.to_string(),
            description: Some(Self::plugin_action_error_description(err)),
            is_disabled: true,
            ..Default::default()
        }];
        if let Some(plugins_response) = plugins_response.cloned() {
            let cwd = self.config.cwd.to_path_buf();
            items.push(SelectionItem {
                name: "Back to plugins".to_string(),
                description: Some("Return to the plugin list.".to_string()),
                selected_description: Some("Return to the plugin list.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenPluginsList {
                        cwd: cwd.clone(),
                        response: plugins_response.clone(),
                    });
                })],
                ..Default::default()
            });
        }

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(plugin_detail_hint_line()),
            items,
            ..Default::default()
        }
    }

    fn plugins_popup_params(
        &self,
        response: &PluginListResponse,
        active_tab_id: Option<String>,
        initial_selected_idx: Option<usize>,
    ) -> SelectionViewParams {
        let marketplaces: Vec<&PluginMarketplaceEntry> = response.marketplaces.iter().collect();
        let preferred_local_sources = preferred_local_plugin_sources(&marketplaces);

        let all_entries = plugin_entries_for_marketplaces(marketplaces.iter().copied());
        let total = all_entries.len();
        let installed = all_entries
            .iter()
            .filter(|(_, plugin, _)| plugin.installed)
            .count();
        let name_column_width = all_entries
            .iter()
            .map(|(_, _, display_name)| {
                PLUGIN_ROW_PREFIX_WIDTH + UnicodeWidthStr::width(display_name.as_str())
            })
            .chain([UnicodeWidthStr::width("Add marketplace")])
            .max();
        let installed_entries = all_entries
            .iter()
            .filter(|(_, plugin, _)| plugin.installed)
            .cloned()
            .collect();

        let mut tabs = Vec::new();
        let mut tab_footer_hints = Vec::new();
        let mut all_items = self.plugin_selection_items(
            all_entries,
            &preferred_local_sources,
            /*include_marketplace_names*/ true,
            "No marketplace plugins available",
            "No plugins are available in the discovered marketplaces.",
        );
        append_marketplace_load_error_items(&mut all_items, &response.marketplace_load_errors);

        tabs.push(SelectionTab {
            id: ALL_PLUGINS_TAB_ID.to_string(),
            label: "All Plugins".to_string(),
            header: plugins_header(
                "Browse plugins from available marketplaces.".to_string(),
                format!("Installed {installed} of {total} available plugins."),
            ),
            items: all_items,
        });

        tabs.push(SelectionTab {
            id: INSTALLED_PLUGINS_TAB_ID.to_string(),
            label: format!("Installed ({installed})"),
            header: plugins_header(
                "Installed plugins.".to_string(),
                format!("Showing {installed} installed plugins."),
            ),
            items: self.plugin_selection_items(
                installed_entries,
                &preferred_local_sources,
                /*include_marketplace_names*/ true,
                "No installed plugins",
                "No installed plugins.",
            ),
        });

        let by_openai_marketplaces = marketplaces
            .iter()
            .copied()
            .filter(|marketplace| marketplace_is_by_openai(marketplace))
            .collect::<Vec<_>>();
        let curated_entries = plugin_entries_for_marketplaces(by_openai_marketplaces);
        let curated_total = curated_entries.len();
        let curated_installed = curated_entries
            .iter()
            .filter(|(_, plugin, _)| plugin.installed)
            .count();
        let curated_has_entries = !curated_entries.is_empty();
        let by_openai_section_error = self
            .plugin_remote_section_errors
            .iter()
            .find(|section_error| section_error.section_id == "vertical");
        let (curated_empty_name, curated_empty_description) =
            if self.plugin_remote_sections_loading && !curated_has_entries {
                (
                    "Loading OpenAI Curated plugins...",
                    "This section updates when app-server returns it.",
                )
            } else if let Some(section_error) = by_openai_section_error
                && !curated_has_entries
            {
                ("OpenAI Curated unavailable", section_error.message.as_str())
            } else {
                (
                    "No OpenAI Curated plugins available",
                    "No OpenAI Curated plugins available.",
                )
            };
        let mut curated_items = self.plugin_selection_items(
            curated_entries,
            &preferred_local_sources,
            /*include_marketplace_names*/ false,
            curated_empty_name,
            curated_empty_description,
        );
        if self.plugin_remote_sections_loading && curated_has_entries {
            curated_items.push(remote_section_loading_item("OpenAI Curated"));
        }
        if let Some(section_error) = by_openai_section_error
            && curated_has_entries
        {
            curated_items.push(remote_section_error_item(
                &section_error.label,
                &section_error.message,
            ));
        }
        tabs.push(SelectionTab {
            id: OPENAI_CURATED_TAB_ID.to_string(),
            label: "OpenAI Curated".to_string(),
            header: plugins_header(
                "OpenAI Curated marketplace.".to_string(),
                format!("Installed {curated_installed} of {curated_total} OpenAI Curated plugins."),
            ),
            items: curated_items,
        });

        let mut additional_marketplaces: Vec<&PluginMarketplaceEntry> = marketplaces
            .iter()
            .copied()
            .filter(|marketplace| !marketplace_is_by_openai(marketplace))
            .collect();
        additional_marketplaces.sort_by_cached_key(|marketplace| {
            let display_name = marketplace_display_name(marketplace);
            (
                marketplace_product_tab_order(marketplace),
                display_name.to_ascii_lowercase(),
                display_name,
                marketplace.name.clone(),
            )
        });

        let has_workspace_tab = additional_marketplaces
            .iter()
            .any(|marketplace| marketplace.name == REMOTE_WORKSPACE_MARKETPLACE_NAME);
        let has_shared_with_me_tab = additional_marketplaces.iter().any(|marketplace| {
            matches!(
                marketplace.name.as_str(),
                REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
                    | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME
                    | REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME
            )
        });
        let workspace_section_error = self
            .plugin_remote_section_errors
            .iter()
            .find(|section_error| section_error.section_id == "workspace");
        let shared_with_me_section_error = self
            .plugin_remote_section_errors
            .iter()
            .find(|section_error| section_error.section_id == "shared-with-me");
        let mut additional_tabs = Vec::new();
        if !has_workspace_tab {
            if self.plugin_remote_sections_loading {
                additional_tabs.push((
                    WORKSPACE_SECTION_FALLBACK_TAB_ORDER,
                    remote_section_loading_tab("workspace-loading", "Workspace"),
                ));
            } else if self.plugin_remote_sections_loaded {
                let tab = if let Some(section_error) = workspace_section_error {
                    remote_section_error_tab(section_error)
                } else {
                    remote_section_empty_tab(
                        "workspace",
                        "Workspace",
                        "No workspace plugins available",
                        "No workspace directory plugins are available.",
                    )
                };
                additional_tabs.push((WORKSPACE_SECTION_FALLBACK_TAB_ORDER, tab));
            }
        }
        if !has_shared_with_me_tab {
            if self.plugin_remote_sections_loading {
                additional_tabs.push((
                    SHARED_WITH_ME_SECTION_FALLBACK_TAB_ORDER,
                    remote_section_loading_tab("shared-with-me-loading", "Shared with me"),
                ));
            } else if self.plugin_remote_sections_loaded {
                let tab = if let Some(section_error) = shared_with_me_section_error {
                    remote_section_error_tab(section_error)
                } else {
                    remote_section_empty_tab(
                        "shared-with-me",
                        "Shared with me",
                        "No shared plugins available",
                        "No plugins have been shared with you.",
                    )
                };
                additional_tabs.push((SHARED_WITH_ME_SECTION_FALLBACK_TAB_ORDER, tab));
            }
        }

        let labels = disambiguate_duplicate_tab_labels(
            additional_marketplaces
                .iter()
                .map(|marketplace| marketplace_display_name(marketplace))
                .collect(),
        );
        for (marketplace, label) in additional_marketplaces.into_iter().zip(labels) {
            let entries = plugin_entries_for_marketplaces([marketplace]);
            let marketplace_total = entries.len();
            let marketplace_installed = entries
                .iter()
                .filter(|(_, plugin, _)| plugin.installed)
                .count();
            let tab_id = marketplace_tab_id(marketplace);
            let can_remove_marketplace =
                marketplace_is_user_configured(&self.config, &marketplace.name);
            let can_upgrade_marketplace = marketplace.path.is_some()
                && marketplace_is_user_configured_git(&self.config, &marketplace.name);
            if can_remove_marketplace || can_upgrade_marketplace {
                tab_footer_hints.push((
                    tab_id.clone(),
                    plugins_popup_hint_line(
                        /*can_remove_marketplace*/ can_remove_marketplace,
                        /*can_upgrade_marketplace*/ can_upgrade_marketplace,
                    ),
                ));
            }
            let header = if self.newly_installed_marketplace_tab_id.as_deref() == Some(&tab_id) {
                plugins_header(
                    format!("{label} installed successfully."),
                    "Select the plugins you want to use and press Enter to install or view details."
                        .to_string(),
                )
            } else {
                plugins_header(
                    format!("{label}."),
                    format!(
                        "Installed {marketplace_installed} of {marketplace_total} {label} plugins."
                    ),
                )
            };
            additional_tabs.push((
                marketplace_product_tab_order(marketplace),
                SelectionTab {
                    id: tab_id,
                    label: label.clone(),
                    header,
                    items: self.plugin_selection_items(
                        entries,
                        &preferred_local_sources,
                        /*include_marketplace_names*/ false,
                        "No plugins available in this marketplace",
                        "No plugins available in this marketplace.",
                    ),
                },
            ));
        }
        additional_tabs.sort_by_key(|(tab_order, _)| *tab_order);
        tabs.extend(additional_tabs.into_iter().map(|(_, tab)| tab));

        tabs.push(self.marketplace_add_tab());
        let initial_tab_id =
            active_tab_id.and_then(|tab_id| plugin_tab_id_matching_saved_id(&tab_id, &tabs));

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(()),
            footer_hint: Some(plugins_popup_hint_line(
                /*can_remove_marketplace*/ false, /*can_upgrade_marketplace*/ false,
            )),
            tab_footer_hints,
            tabs,
            initial_tab_id,
            is_searchable: true,
            search_placeholder: Some("Type to search plugins".to_string()),
            col_width_mode: ColumnWidthMode::AutoAllRows,
            row_display: SelectionRowDisplay::SingleLine,
            name_column_width,
            initial_selected_idx,
            ..Default::default()
        }
    }

    fn marketplace_add_tab(&self) -> SelectionTab {
        SelectionTab {
            id: ADD_MARKETPLACE_TAB_ID.to_string(),
            label: "Add marketplace".to_string(),
            header: plugins_header(
                "Add a marketplace from a Git repo or local root.".to_string(),
                "Enter a source to make its plugins available in this menu.".to_string(),
            ),
            items: vec![SelectionItem {
                name: "Add marketplace".to_string(),
                description: Some(
                    "Enter owner/repo, a Git URL, or a local marketplace path.".to_string(),
                ),
                selected_description: Some(
                    "Press Enter to enter a marketplace source.".to_string(),
                ),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenMarketplaceAddPrompt);
                })],
                ..Default::default()
            }],
        }
    }

    fn plugin_action_error_description(err: &str) -> String {
        let next_step = Self::plugin_action_error_next_step(err);
        if next_step.is_empty() {
            err.to_string()
        } else {
            format!("{err} {next_step}")
        }
    }

    fn plugin_action_error_next_step(err: &str) -> &'static str {
        let err = err.to_ascii_lowercase();
        if err.contains("api key auth is not supported") {
            "Sign in with ChatGPT auth; API key auth cannot use remote plugins."
        } else if err.contains("authentication required")
            || err.contains("not signed in")
            || err.contains("not logged in")
        {
            "Sign in to ChatGPT, then try again."
        } else if err.contains("codex plugins are disabled")
            || err.contains("plugin sharing is disabled")
            || err.contains("plugin sharing is not enabled")
            || err.contains("feature disabled")
        {
            "Ask a workspace admin to enable Codex plugins or plugin sharing."
        } else if err.contains("workspace") && (err.contains("access") || err.contains("mismatch"))
        {
            "Switch to the matching workspace or ask the sharer for access."
        } else if err.contains("not found") || err.contains("status 404") {
            "Check that you are signed in to the correct workspace and still have access."
        } else if err.contains("disabled by admin") || err.contains("admin disabled") {
            "Ask a workspace admin to confirm plugin access."
        } else if err.contains("service unavailable")
            || err.contains("temporarily unavailable")
            || err.contains("status 503")
        {
            "Try again later; local plugin functionality is still available."
        } else if err.contains("not installable") || err.contains("not available") {
            "Choose a plugin that app-server reports as installable."
        } else if err.contains("invalid plugin") || err.contains("invalid manifest") {
            "Check the local plugin files, then try again."
        } else if err.contains("old build") || err.contains("update codex") || err.contains("stale")
        {
            "Update Codex, then try again."
        } else if err.contains("failed to send")
            || err.contains("request")
            || err.contains("status")
        {
            "Try again later; local plugin functionality is still available."
        } else {
            ""
        }
    }

    fn plugin_detail_popup_params(
        &self,
        plugins_response: &PluginListResponse,
        plugin: &PluginDetail,
    ) -> SelectionViewParams {
        let marketplace_label = marketplace_product_label_from_name(&plugin.marketplace_name)
            .map(str::to_string)
            .unwrap_or_else(|| plugin.marketplace_name.clone());
        let display_name = plugin_display_name(&plugin.summary);
        let detail_status_label = plugin_detail_status_label(&plugin.summary);
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from(
            format!("{display_name} · {detail_status_label} · {marketplace_label}").bold(),
        ));
        if !plugin.summary.installed {
            header.push(PluginDisclosureLine {
                line: Line::from(vec![
                    "Data shared with this app is subject to the app's ".into(),
                    "terms of service".bold(),
                    " and ".into(),
                    "privacy policy".bold(),
                    ". ".into(),
                    "Learn more".cyan().underlined(),
                    ".".into(),
                ]),
            });
        }
        if let Some(description) = plugin_detail_description(plugin) {
            header.push(Line::from(description.dim()));
        }

        let cwd = self.config.cwd.to_path_buf();
        let plugins_response = plugins_response.clone();
        let mut items = vec![SelectionItem {
            name: "Back to plugins".to_string(),
            description: Some("Return to the plugin list.".to_string()),
            selected_description: Some("Return to the plugin list.".to_string()),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenPluginsList {
                    cwd: cwd.clone(),
                    response: plugins_response.clone(),
                });
            })],
            ..Default::default()
        }];

        if plugin.summary.installed {
            if let Some(plugin_id) = plugin_uninstall_id(&plugin.summary) {
                let uninstall_cwd = self.config.cwd.to_path_buf();
                let plugin_display_name = display_name;
                items.push(SelectionItem {
                    name: "Uninstall plugin".to_string(),
                    description: Some("Remove this plugin now.".to_string()),
                    selected_description: Some("Remove this plugin now.".to_string()),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenPluginUninstallLoading {
                            plugin_display_name: plugin_display_name.clone(),
                        });
                        tx.send(AppEvent::FetchPluginUninstall {
                            cwd: uninstall_cwd.clone(),
                            plugin_id: plugin_id.clone(),
                            plugin_display_name: plugin_display_name.clone(),
                        });
                    })],
                    ..Default::default()
                });
            } else {
                items.push(SelectionItem {
                    name: "Uninstall plugin".to_string(),
                    description: Some(
                        "This remote plugin did not provide an uninstall identity.".to_string(),
                    ),
                    is_disabled: true,
                    ..Default::default()
                });
            }
        } else if plugin.summary.availability == PluginAvailability::DisabledByAdmin {
            items.push(SelectionItem {
                name: "Install plugin".to_string(),
                description: Some("This plugin is disabled by your workspace admin.".to_string()),
                is_disabled: true,
                ..Default::default()
            });
        } else if plugin.summary.install_policy == PluginInstallPolicy::NotAvailable {
            items.push(SelectionItem {
                name: "Install plugin".to_string(),
                description: Some(
                    "This plugin is not installable from this marketplace.".to_string(),
                ),
                is_disabled: true,
                ..Default::default()
            });
        } else if let Some(location) = plugin_detail_location(plugin) {
            let install_cwd = self.config.cwd.to_path_buf();
            let plugin_name = plugin_request_name(&plugin.summary);
            let plugin_display_name = display_name;
            items.push(SelectionItem {
                name: "Install plugin".to_string(),
                description: Some("Install this plugin now.".to_string()),
                selected_description: Some("Install this plugin now.".to_string()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenPluginInstallLoading {
                        plugin_display_name: plugin_display_name.clone(),
                    });
                    tx.send(AppEvent::FetchPluginInstall {
                        cwd: install_cwd.clone(),
                        location: location.clone(),
                        plugin_name: plugin_name.clone(),
                        plugin_display_name: plugin_display_name.clone(),
                    });
                })],
                ..Default::default()
            });
        } else {
            items.push(SelectionItem {
                name: "Install plugin".to_string(),
                description: Some("This plugin did not provide an install location.".to_string()),
                is_disabled: true,
                ..Default::default()
            });
        }

        items.extend(plugin_metadata_items(plugin));

        items.push(SelectionItem {
            name: "Skills".to_string(),
            description: Some(plugin_skill_summary(plugin)),
            is_disabled: true,
            ..Default::default()
        });
        items.push(SelectionItem {
            name: "Hooks".to_string(),
            description: Some(plugin_hook_summary(plugin)),
            is_disabled: true,
            ..Default::default()
        });
        items.push(SelectionItem {
            name: "Apps".to_string(),
            description: Some(plugin_app_summary(plugin)),
            is_disabled: true,
            ..Default::default()
        });
        items.push(SelectionItem {
            name: "MCP Servers".to_string(),
            description: Some(plugin_mcp_summary(plugin)),
            is_disabled: true,
            ..Default::default()
        });

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(header),
            footer_hint: Some(plugin_detail_hint_line()),
            items,
            col_width_mode: ColumnWidthMode::AutoAllRows,
            ..Default::default()
        }
    }

    fn plugin_selection_items<'a>(
        &self,
        mut plugin_entries: Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)>,
        preferred_local_sources: &[PreferredLocalPluginSource],
        include_marketplace_names: bool,
        empty_name: &str,
        empty_description: &str,
    ) -> Vec<SelectionItem> {
        sort_plugin_entries(&mut plugin_entries);
        let status_label_width = plugin_entries
            .iter()
            .map(|(_, plugin, _)| plugin_status_label(plugin).chars().count())
            .max()
            .unwrap_or(0);

        let mut items: Vec<SelectionItem> = Vec::new();
        for (marketplace, plugin, display_name) in plugin_entries {
            let marketplace_label = marketplace_display_name(marketplace);
            let status_label = plugin_status_label(plugin);
            let description = if include_marketplace_names {
                plugin_brief_description(plugin, &marketplace_label, status_label_width)
            } else {
                plugin_brief_description_without_marketplace(plugin, status_label_width)
            };
            let plugin_detail_request =
                plugin_detail_request_for_entry(marketplace, plugin, preferred_local_sources);
            let can_view_details = plugin_detail_request.is_some();
            let can_toggle_plugin =
                plugin.installed && plugin.availability != PluginAvailability::DisabledByAdmin;
            let selected_status_label = format!("{status_label:<status_label_width$}");
            let selected_description = if can_toggle_plugin {
                let toggle_action = if plugin.enabled { "disable" } else { "enable" };
                if can_view_details {
                    format!(
                        "{selected_status_label}   Space to {toggle_action}; Enter view details."
                    )
                } else {
                    format!("{selected_status_label}   Space to {toggle_action}.")
                }
            } else if plugin.installed && can_view_details {
                format!("{selected_status_label}   Press Enter to view plugin details.")
            } else if plugin.installed {
                format!("{selected_status_label}   Plugin details are unavailable.")
            } else if can_view_details {
                format!("{selected_status_label}   Press Enter to install or view plugin details.")
            } else {
                format!("{selected_status_label}   Remote plugin details are not available yet.")
            };
            let search_value = format!(
                "{display_name} {} {} {} {} {}",
                plugin.id,
                plugin.name,
                marketplace_label,
                plugin_description(plugin).unwrap_or_default(),
                plugin.keywords.join(" ")
            );
            let cwd = self.config.cwd.to_path_buf();
            let plugin_display_name = display_name.clone();
            let toggle_cwd = cwd.clone();
            let toggle_plugin_id = plugin.id.clone();
            let toggle = can_toggle_plugin.then(|| SelectionToggle {
                is_on: plugin.enabled,
                action: Box::new(move |enabled, tx| {
                    tx.send(AppEvent::SetPluginEnabled {
                        cwd: toggle_cwd.clone(),
                        plugin_id: toggle_plugin_id.clone(),
                        enabled,
                    });
                }),
            });
            let actions: Vec<SelectionAction> =
                if let Some((location, plugin_name)) = plugin_detail_request {
                    vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenPluginDetailLoading {
                            plugin_display_name: plugin_display_name.clone(),
                        });
                        let (marketplace_path, remote_marketplace_name) =
                            location.clone().into_request_params();
                        tx.send(AppEvent::FetchPluginDetail {
                            cwd: cwd.clone(),
                            params: codex_app_server_protocol::PluginReadParams {
                                marketplace_path,
                                remote_marketplace_name,
                                plugin_name: plugin_name.clone(),
                            },
                        });
                    })]
                } else {
                    Vec::new()
                };
            let is_disabled = !can_view_details && !plugin.installed;
            let disabled_reason = is_disabled.then(|| "plugin details are unavailable".to_string());

            items.push(SelectionItem {
                name: display_name,
                toggle,
                toggle_placeholder: (!can_toggle_plugin).then_some("[-] "),
                description: Some(description),
                selected_description: Some(selected_description),
                search_value: Some(search_value),
                actions,
                is_disabled,
                disabled_reason,
                ..Default::default()
            });
        }

        if items.is_empty() {
            items.push(SelectionItem {
                name: empty_name.to_string(),
                description: Some(empty_description.to_string()),
                is_disabled: true,
                ..Default::default()
            });
        }
        items
    }
}

fn plugins_popup_hint_line(
    can_remove_marketplace: bool,
    can_upgrade_marketplace: bool,
) -> Line<'static> {
    match (can_remove_marketplace, can_upgrade_marketplace) {
        (true, true) => Line::from(
            "ctrl + u upgrade · ctrl + r remove · space toggle · ←/→ tabs · enter details · esc close",
        ),
        (true, false) => {
            Line::from("ctrl + r remove · space toggle · ←/→ tabs · enter details · esc close")
        }
        (false, true) => {
            Line::from("ctrl + u upgrade · space toggle · ←/→ tabs · enter details · esc close")
        }
        (false, false) => Line::from(
            "space enable/disable · ←/→ select marketplace · enter view details · esc close",
        ),
    }
}

fn plugin_detail_hint_line() -> Line<'static> {
    Line::from("Press esc to close.")
}

fn plugins_header(subtitle: String, count_line: String) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Plugins".bold()));
    header.push(Line::from(subtitle.dim()));
    header.push(Line::from(count_line.dim()));
    Box::new(header)
}

fn remote_section_loading_tab(id: &str, label: &str) -> SelectionTab {
    SelectionTab {
        id: format!("remote-loading:{id}"),
        label: label.to_string(),
        header: plugins_header(
            format!("Loading {label} plugins."),
            "Local plugin functionality is already available.".to_string(),
        ),
        items: vec![remote_section_loading_item(label)],
    }
}

fn remote_section_loading_item(label: &str) -> SelectionItem {
    SelectionItem {
        name: format!("Loading {label} plugins..."),
        description: Some("This section updates when app-server returns it.".to_string()),
        is_disabled: true,
        ..Default::default()
    }
}

fn remote_section_empty_tab(
    id: &str,
    label: &str,
    item_name: &str,
    item_description: &str,
) -> SelectionTab {
    SelectionTab {
        id: format!("remote-empty:{id}"),
        label: label.to_string(),
        header: plugins_header(
            format!("{label}."),
            "This section loaded successfully.".to_string(),
        ),
        items: vec![SelectionItem {
            name: item_name.to_string(),
            description: Some(item_description.to_string()),
            is_disabled: true,
            ..Default::default()
        }],
    }
}

fn remote_section_error_tab(section_error: &PluginRemoteSectionError) -> SelectionTab {
    SelectionTab {
        id: format!("remote-error:{}", section_error.section_id),
        label: section_error.label.clone(),
        header: plugins_header(
            format!("{} unavailable.", section_error.label),
            "Local plugin functionality is still available.".to_string(),
        ),
        items: vec![remote_section_error_item(
            &section_error.label,
            &section_error.message,
        )],
    }
}

fn remote_section_error_item(label: &str, message: &str) -> SelectionItem {
    SelectionItem {
        name: format!("{label} unavailable"),
        description: Some(message.to_string()),
        is_disabled: true,
        ..Default::default()
    }
}

fn append_marketplace_load_error_items(
    items: &mut Vec<SelectionItem>,
    load_errors: &[MarketplaceLoadErrorInfo],
) {
    for load_error in load_errors {
        let marketplace_path = load_error.marketplace_path.as_path().display();
        let description = format!("{marketplace_path}: {}", load_error.message);
        items.push(SelectionItem {
            name: "Marketplace unavailable".to_string(),
            description: Some(description.clone()),
            selected_description: Some(description.clone()),
            search_value: Some(description),
            is_disabled: true,
            ..Default::default()
        });
    }
}

fn plugin_entries_for_marketplaces<'a>(
    marketplaces: impl IntoIterator<Item = &'a PluginMarketplaceEntry>,
) -> Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)> {
    let entries = marketplaces
        .into_iter()
        .flat_map(|marketplace| {
            marketplace
                .plugins
                .iter()
                .map(move |plugin| (marketplace, plugin, plugin_display_name(plugin)))
        })
        .collect::<Vec<_>>();
    dedupe_plugin_entries(entries)
}

fn dedupe_plugin_entries<'a>(
    entries: Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)>,
) -> Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)> {
    // App-server should eventually normalize local/remote duplicates. Keep this
    // display-only pass narrow so shared plugins do not appear twice meanwhile.
    let mut deduped: Vec<(&PluginMarketplaceEntry, &PluginSummary, String)> = Vec::new();
    let mut remote_entry_indexes = std::collections::HashMap::new();
    for entry in entries {
        let Some(remote_plugin_id) = plugin_remote_identity(entry.1) else {
            deduped.push(entry);
            continue;
        };
        if let Some(existing_index) = remote_entry_indexes.get(&remote_plugin_id).copied() {
            if plugin_entry_preferred(&entry, &deduped[existing_index]) {
                deduped[existing_index] = entry;
            }
        } else {
            remote_entry_indexes.insert(remote_plugin_id, deduped.len());
            deduped.push(entry);
        }
    }
    deduped
}

fn plugin_entry_preferred(
    candidate: &(&PluginMarketplaceEntry, &PluginSummary, String),
    existing: &(&PluginMarketplaceEntry, &PluginSummary, String),
) -> bool {
    let candidate_is_local_share =
        candidate.1.share_context.is_some() && !matches!(candidate.1.source, PluginSource::Remote);
    let existing_is_local_share =
        existing.1.share_context.is_some() && !matches!(existing.1.source, PluginSource::Remote);
    if candidate_is_local_share != existing_is_local_share {
        return candidate_is_local_share;
    }

    if candidate.1.installed != existing.1.installed {
        return candidate.1.installed;
    }

    !matches!(candidate.1.source, PluginSource::Remote)
        && matches!(existing.1.source, PluginSource::Remote)
}

fn sort_plugin_entries(entries: &mut [(&PluginMarketplaceEntry, &PluginSummary, String)]) {
    entries.sort_by(|left, right| {
        right
            .1
            .installed
            .cmp(&left.1.installed)
            .then_with(|| {
                left.2
                    .to_ascii_lowercase()
                    .cmp(&right.2.to_ascii_lowercase())
            })
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.1.name.cmp(&right.1.name))
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
}

fn marketplace_tab_id(marketplace: &PluginMarketplaceEntry) -> String {
    match marketplace.path.as_ref() {
        Some(path) => marketplace_tab_id_from_path(path.as_path()),
        None => format!("marketplace:{}", marketplace.name),
    }
}

fn marketplace_tab_id_from_path(path: &Path) -> String {
    format!("{MARKETPLACE_TAB_ID_PREFIX}{}", path.display())
}

fn marketplace_tab_id_matching_saved_id(
    saved_tab_id: &str,
    marketplaces: &[PluginMarketplaceEntry],
) -> Option<String> {
    if let Some(tab_id) = remote_section_marketplace_tab_id(saved_tab_id, marketplaces) {
        return Some(tab_id);
    }

    if let Some(tab_id) = marketplaces.iter().find_map(|marketplace| {
        let tab_id = marketplace_tab_id(marketplace);
        (tab_id == saved_tab_id).then_some(tab_id)
    }) {
        return Some(tab_id);
    }

    let root = saved_tab_id.strip_prefix(MARKETPLACE_TAB_ID_PREFIX)?;
    if root.is_empty() {
        return None;
    }
    let root = Path::new(root);
    marketplaces.iter().find_map(|marketplace| {
        marketplace
            .path
            .as_ref()
            .is_some_and(|path| path.as_path().starts_with(root))
            .then(|| marketplace_tab_id(marketplace))
    })
}

fn remote_section_marketplace_tab_id(
    saved_tab_id: &str,
    marketplaces: &[PluginMarketplaceEntry],
) -> Option<String> {
    let marketplace_name_matches = match saved_tab_id {
        "remote-loading:workspace-loading"
        | "remote-empty:workspace"
        | "remote-error:workspace" => WORKSPACE_SECTION_MARKETPLACE_NAMES,
        "remote-loading:shared-with-me-loading"
        | "remote-empty:shared-with-me"
        | "remote-error:shared-with-me" => SHARED_WITH_ME_SECTION_MARKETPLACE_NAMES,
        _ => return None,
    };

    marketplace_name_matches
        .iter()
        .find_map(|marketplace_name| {
            marketplaces
                .iter()
                .find(|marketplace| marketplace.name.as_str() == *marketplace_name)
                .map(marketplace_tab_id)
        })
}

fn plugin_tab_id_matching_saved_id(saved_tab_id: &str, tabs: &[SelectionTab]) -> Option<String> {
    if let Some(tab_id) = tabs
        .iter()
        .find(|tab| tab.id.as_str() == saved_tab_id)
        .map(|tab| tab.id.clone())
    {
        return Some(tab_id);
    }

    let candidate_tab_ids = match saved_tab_id {
        "remote-loading:workspace-loading"
        | "remote-empty:workspace"
        | "remote-error:workspace"
        | "marketplace:workspace-directory" => WORKSPACE_SECTION_TAB_IDS,
        "remote-loading:shared-with-me-loading"
        | "remote-empty:shared-with-me"
        | "remote-error:shared-with-me"
        | "marketplace:workspace-shared-with-me"
        | "marketplace:workspace-shared-with-me-private"
        | "marketplace:workspace-shared-with-me-unlisted" => SHARED_WITH_ME_SECTION_TAB_IDS,
        _ => return None,
    };

    tabs.iter()
        .find(|tab| candidate_tab_ids.contains(&tab.id.as_str()))
        .map(|tab| tab.id.clone())
}

fn merge_remote_marketplaces(
    response: &mut PluginListResponse,
    remote_marketplaces: Vec<PluginMarketplaceEntry>,
) {
    let remote_names = remote_marketplaces
        .iter()
        .map(|marketplace| marketplace.name.clone())
        .collect::<std::collections::HashSet<_>>();
    response.marketplaces.retain(|marketplace| {
        marketplace.path.is_some()
            || !remote_marketplace_is_remote_section(marketplace)
                && !remote_names.contains(marketplace.name.as_str())
    });
    response.marketplaces.extend(remote_marketplaces);
}

fn remote_marketplace_is_remote_section(marketplace: &PluginMarketplaceEntry) -> bool {
    matches!(
        marketplace.name.as_str(),
        REMOTE_WORKSPACE_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME
    )
}

fn disambiguate_duplicate_tab_labels(labels: Vec<String>) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for label in &labels {
        if let Some((_, count)) = counts.iter_mut().find(|(existing, _)| existing == label) {
            *count += 1;
        } else {
            counts.push((label.clone(), 1));
        }
    }

    let mut seen: Vec<(String, usize)> = Vec::new();
    labels
        .into_iter()
        .map(|label| {
            let total = counts
                .iter()
                .find(|(existing, _)| existing == &label)
                .map(|(_, count)| *count)
                .unwrap_or(1);
            if total == 1 {
                return label;
            }

            let current = if let Some((_, seen_count)) =
                seen.iter_mut().find(|(existing, _)| existing == &label)
            {
                *seen_count += 1;
                *seen_count
            } else {
                seen.push((label.clone(), 1));
                1
            };
            format!("{label} ({current}/{total})")
        })
        .collect()
}

fn marketplace_display_name(marketplace: &PluginMarketplaceEntry) -> String {
    if let Some(label) = marketplace_product_label_from_name(&marketplace.name) {
        return label.to_string();
    }
    if marketplace
        .path
        .as_ref()
        .is_some_and(is_personal_marketplace_path)
    {
        return "Local".to_string();
    }
    marketplace
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| marketplace.name.clone())
}

fn marketplace_product_tab_order(marketplace: &PluginMarketplaceEntry) -> u8 {
    match marketplace.name.as_str() {
        REMOTE_WORKSPACE_MARKETPLACE_NAME => WORKSPACE_SECTION_TAB_ORDER,
        REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
        | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME => {
            SHARED_WITH_ME_SECTION_TAB_ORDER
        }
        REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME => {
            SHARED_WITH_ME_LINK_SECTION_TAB_ORDER
        }
        _ if marketplace
            .path
            .as_ref()
            .is_some_and(is_personal_marketplace_path) =>
        {
            LOCAL_MARKETPLACE_TAB_ORDER
        }
        _ => OTHER_MARKETPLACE_TAB_ORDER,
    }
}

fn marketplace_product_label_from_name(marketplace_name: &str) -> Option<&str> {
    match marketplace_name {
        OPENAI_CURATED_MARKETPLACE_NAME | REMOTE_GLOBAL_MARKETPLACE_NAME => Some("OpenAI Curated"),
        REMOTE_WORKSPACE_MARKETPLACE_NAME => Some("Workspace"),
        REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
        | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME => Some("Shared with me"),
        REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME => Some("Shared with me (link)"),
        _ => None,
    }
}

fn marketplace_is_by_openai(marketplace: &PluginMarketplaceEntry) -> bool {
    marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME
        || marketplace.name == REMOTE_GLOBAL_MARKETPLACE_NAME
}

fn marketplace_is_user_configured(config: &Config, marketplace_name: &str) -> bool {
    let Some(user_config) = config.config_layer_stack.effective_user_config() else {
        return false;
    };
    user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
        .is_some_and(|marketplaces| marketplaces.contains_key(marketplace_name))
}

fn marketplace_is_user_configured_git(config: &Config, marketplace_name: &str) -> bool {
    config
        .config_layer_stack
        .get_active_user_layer()
        .and_then(|user_layer| user_layer.config.get("marketplaces"))
        .and_then(toml::Value::as_table)
        .and_then(|marketplaces| marketplaces.get(marketplace_name))
        .and_then(toml::Value::as_table)
        .and_then(|marketplace| marketplace.get("source_type"))
        .and_then(toml::Value::as_str)
        .is_some_and(|source_type| source_type == "git")
}

fn plugin_display_name(plugin: &PluginSummary) -> String {
    plugin
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| plugin.name.clone())
}

fn plugin_brief_description(
    plugin: &PluginSummary,
    marketplace_label: &str,
    status_label_width: usize,
) -> String {
    let status_label = plugin_status_label(plugin);
    let status_label = format!("{status_label:<status_label_width$}");
    match plugin_description(plugin) {
        Some(description) => format!("{status_label} · {marketplace_label} · {description}"),
        None => format!("{status_label} · {marketplace_label}"),
    }
}

fn plugin_brief_description_without_marketplace(
    plugin: &PluginSummary,
    status_label_width: usize,
) -> String {
    let status_label = plugin_status_label(plugin);
    let status_label = format!("{status_label:<status_label_width$}");
    match plugin_description(plugin) {
        Some(description) => format!("{status_label} · {description}"),
        None => status_label,
    }
}

fn plugin_status_label(plugin: &PluginSummary) -> &'static str {
    if plugin.availability == PluginAvailability::DisabledByAdmin {
        return "Disabled by admin";
    }
    if plugin.installed {
        if plugin.enabled {
            "Installed"
        } else {
            "Disabled"
        }
    } else {
        match plugin.install_policy {
            PluginInstallPolicy::NotAvailable => "Not installable",
            PluginInstallPolicy::Available => "Available",
            PluginInstallPolicy::InstalledByDefault => "Available by default",
        }
    }
}

fn plugin_detail_status_label(plugin: &PluginSummary) -> &'static str {
    if plugin.availability == PluginAvailability::DisabledByAdmin {
        return "Disabled by admin";
    }
    if plugin.installed {
        if plugin.enabled {
            "Installed"
        } else {
            "Disabled"
        }
    } else {
        match plugin.install_policy {
            PluginInstallPolicy::NotAvailable => "Not installable",
            PluginInstallPolicy::Available => "Can be installed",
            PluginInstallPolicy::InstalledByDefault => "Available by default",
        }
    }
}

fn plugin_location_for_marketplace(
    marketplace: &PluginMarketplaceEntry,
    plugin: &PluginSummary,
) -> Option<PluginLocation> {
    if let Some(marketplace_path) = marketplace.path.clone() {
        return Some(PluginLocation::Local { marketplace_path });
    }
    plugin_remote_identity(plugin).map(|_| PluginLocation::Remote {
        marketplace_name: marketplace.name.clone(),
    })
}

fn plugin_detail_location(plugin: &PluginDetail) -> Option<PluginLocation> {
    if let Some(marketplace_path) = plugin.marketplace_path.clone() {
        return Some(PluginLocation::Local { marketplace_path });
    }
    plugin_remote_identity(&plugin.summary).map(|_| PluginLocation::Remote {
        marketplace_name: plugin.marketplace_name.clone(),
    })
}

fn plugin_detail_request_for_entry(
    marketplace: &PluginMarketplaceEntry,
    plugin: &PluginSummary,
    preferred_local_sources: &[PreferredLocalPluginSource],
) -> Option<(PluginLocation, String)> {
    if matches!(plugin.source, PluginSource::Remote)
        && let Some(remote_plugin_id) = plugin_remote_identity(plugin)
        && let Some(preferred_source) = preferred_local_sources
            .iter()
            .find(|source| source.remote_plugin_id == remote_plugin_id)
    {
        return Some((
            PluginLocation::Local {
                marketplace_path: preferred_source.marketplace_path.clone(),
            },
            preferred_source.plugin_name.clone(),
        ));
    }

    plugin_location_for_marketplace(marketplace, plugin)
        .map(|location| (location, plugin_request_name(plugin)))
}

fn preferred_local_plugin_sources(
    marketplaces: &[&PluginMarketplaceEntry],
) -> Vec<PreferredLocalPluginSource> {
    let mut sources: Vec<PreferredLocalPluginSource> = Vec::new();
    let mut seen_remote_plugin_ids = std::collections::HashSet::new();
    for marketplace in marketplaces {
        let Some(marketplace_path) = marketplace.path.clone() else {
            continue;
        };
        for plugin in &marketplace.plugins {
            if matches!(plugin.source, PluginSource::Remote) {
                continue;
            }
            let Some(remote_plugin_id) = plugin
                .share_context
                .as_ref()
                .map(|context| context.remote_plugin_id.clone())
            else {
                continue;
            };
            if !seen_remote_plugin_ids.insert(remote_plugin_id.clone()) {
                continue;
            }
            sources.push(PreferredLocalPluginSource {
                remote_plugin_id,
                marketplace_path: marketplace_path.clone(),
                plugin_name: plugin.name.clone(),
            });
        }
    }
    sources
}

fn plugin_request_name(plugin: &PluginSummary) -> String {
    if matches!(plugin.source, PluginSource::Remote)
        && let Some(remote_plugin_id) = plugin_remote_identity(plugin)
    {
        return remote_plugin_id;
    }
    plugin.name.clone()
}

fn plugin_remote_identity(plugin: &PluginSummary) -> Option<String> {
    plugin
        .share_context
        .as_ref()
        .map(|context| context.remote_plugin_id.clone())
        .or_else(|| plugin.remote_plugin_id.clone())
}

fn plugin_uninstall_id(plugin: &PluginSummary) -> Option<String> {
    if matches!(plugin.source, PluginSource::Remote) {
        return plugin_remote_identity(plugin);
    }
    Some(plugin.id.clone())
}

fn is_personal_marketplace_path(marketplace_path: &AbsolutePathBuf) -> bool {
    dirs::home_dir()
        .and_then(|home| personal_marketplace_path_from_home(home.as_path()))
        .is_some_and(|personal_path| personal_path.as_path() == marketplace_path.as_path())
}

fn personal_marketplace_path_from_home(home: &Path) -> Option<AbsolutePathBuf> {
    AbsolutePathBuf::try_from(home.join(PERSONAL_MARKETPLACE_RELATIVE_PATH)).ok()
}

fn plugin_metadata_items(plugin: &PluginDetail) -> Vec<SelectionItem> {
    let mut items = Vec::new();
    items.push(SelectionItem {
        name: "Source".to_string(),
        description: Some(plugin_source_summary(plugin)),
        is_disabled: true,
        ..Default::default()
    });
    items.push(SelectionItem {
        name: "Auth".to_string(),
        description: Some(plugin_auth_policy_summary(plugin.summary.auth_policy)),
        is_disabled: true,
        ..Default::default()
    });
    if let Some(version) = plugin_version_summary(&plugin.summary) {
        items.push(SelectionItem {
            name: "Version".to_string(),
            description: Some(version),
            is_disabled: true,
            ..Default::default()
        });
    }
    if let Some(share_context) = &plugin.summary.share_context {
        items.push(SelectionItem {
            name: "Sharing".to_string(),
            description: Some(plugin_share_context_summary(share_context)),
            is_disabled: true,
            ..Default::default()
        });
    }
    items
}

fn plugin_source_summary(plugin: &PluginDetail) -> String {
    match &plugin.summary.source {
        PluginSource::Local { .. } => "Local".to_string(),
        PluginSource::Git { url, ref_name, .. } => match ref_name {
            Some(ref_name) => format!("Git · {url}@{ref_name}"),
            None => format!("Git · {url}"),
        },
        PluginSource::Remote => {
            let marketplace_label = marketplace_product_label_from_name(&plugin.marketplace_name)
                .unwrap_or(plugin.marketplace_name.as_str());
            format!("Remote · {marketplace_label}")
        }
    }
}

fn plugin_auth_policy_summary(auth_policy: PluginAuthPolicy) -> String {
    match auth_policy {
        PluginAuthPolicy::OnInstall => "Auth on install".to_string(),
        PluginAuthPolicy::OnUse => "Auth on use".to_string(),
    }
}

fn plugin_version_summary(plugin: &PluginSummary) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(local_version) = plugin.local_version.as_deref() {
        parts.push(format!("local {local_version}"));
    }
    if let Some(remote_version) = plugin
        .share_context
        .as_ref()
        .and_then(|context| context.remote_version.as_deref())
    {
        parts.push(format!("remote {remote_version}"));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn plugin_share_context_summary(context: &PluginShareContext) -> String {
    let mut parts = Vec::new();
    if let Some(discoverability) = context.discoverability {
        parts.push(plugin_share_discoverability_label(discoverability).to_string());
    }
    if let Some(creator_summary) = plugin_share_creator_summary(context) {
        parts.push(creator_summary);
    }
    if let Some(principals) = context.share_principals.as_ref() {
        parts.push(plugin_share_principals_summary(principals));
    }
    if let Some(share_url) = context
        .share_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    {
        parts.push(share_url.to_string());
    }
    if parts.is_empty() {
        format!("Remote ID {}", context.remote_plugin_id)
    } else {
        parts.join(" · ")
    }
}

fn plugin_share_discoverability_label(discoverability: PluginShareDiscoverability) -> &'static str {
    match discoverability {
        PluginShareDiscoverability::Listed => "Listed",
        PluginShareDiscoverability::Unlisted => "Workspace link",
        PluginShareDiscoverability::Private => "Private",
    }
}

fn plugin_share_creator_summary(context: &PluginShareContext) -> Option<String> {
    match (
        context.creator_name.as_deref(),
        context.creator_account_user_id.as_deref(),
    ) {
        (Some(name), Some(account_id)) => Some(format!("creator {name} ({account_id})")),
        (Some(name), None) => Some(format!("creator {name}")),
        (None, Some(account_id)) => Some(format!("creator account {account_id}")),
        (None, None) => None,
    }
}

fn plugin_share_principals_summary(principals: &[PluginSharePrincipal]) -> String {
    match principals.len() {
        0 => "No explicit principals".to_string(),
        1 => format!("1 principal: {}", principals[0].name),
        count => format!("{count} principals"),
    }
}

fn plugin_description(plugin: &PluginSummary) -> Option<String> {
    plugin
        .interface
        .as_ref()
        .and_then(|interface| {
            interface
                .short_description
                .as_deref()
                .or(interface.long_description.as_deref())
        })
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
}

fn plugin_detail_description(plugin: &PluginDetail) -> Option<String> {
    plugin
        .description
        .as_deref()
        .or_else(|| {
            plugin
                .summary
                .interface
                .as_ref()
                .and_then(|interface| interface.long_description.as_deref())
        })
        .or_else(|| {
            plugin
                .summary
                .interface
                .as_ref()
                .and_then(|interface| interface.short_description.as_deref())
        })
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
}

fn plugin_skill_summary(plugin: &PluginDetail) -> String {
    if plugin.skills.is_empty() {
        "No plugin skills.".to_string()
    } else {
        plugin
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn plugin_app_summary(plugin: &PluginDetail) -> String {
    if plugin.apps.is_empty() {
        "No plugin apps.".to_string()
    } else {
        plugin
            .apps
            .iter()
            .map(|app| app.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn plugin_hook_summary(plugin: &PluginDetail) -> String {
    if plugin.hooks.is_empty() {
        "No plugin hooks.".to_string()
    } else {
        let mut event_counts = Vec::<(codex_app_server_protocol::HookEventName, usize)>::new();
        for hook in &plugin.hooks {
            if let Some((_, handler_count)) = event_counts
                .iter_mut()
                .find(|(event_name, _)| *event_name == hook.event_name)
            {
                *handler_count += 1;
            } else {
                event_counts.push((hook.event_name, 1));
            }
        }
        event_counts
            .into_iter()
            .map(|(event_name, handler_count)| format!("{event_name:?} ({handler_count})"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn plugin_mcp_summary(plugin: &PluginDetail) -> String {
    if plugin.mcp_servers.is_empty() {
        "No plugin MCP servers.".to_string()
    } else {
        plugin.mcp_servers.join(", ")
    }
}
