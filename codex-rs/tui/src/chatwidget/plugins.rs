use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionTab;
use crate::bottom_pane::SelectionToggle;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::custom_prompt_view::CustomPromptView;
use crate::history_cell;
use crate::legacy_core::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::onboarding::mark_url_hyperlink;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;
use codex_app_server_protocol::MarketplaceAddResponse;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
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
const PLUGIN_MENU_ROW_PREFIX_WIDTH: usize = 6;
const LOADING_ANIMATION_DELAY: Duration = Duration::from_secs(1);
const LOADING_ANIMATION_INTERVAL: Duration = Duration::from_millis(100);

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
            lines.push(Line::from(shimmer_spans(self.loading_text.as_str())));
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
        if self.plugins_fetch_state.in_flight_cwd.as_deref() == Some(cwd.as_path()) {
            self.plugins_fetch_state.in_flight_cwd = None;
        }

        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        let auth_flow_active = self.plugin_install_auth_flow.is_some();

        match result {
            Ok(response) => {
                self.plugins_fetch_state.cache_cwd = Some(cwd);
                let active_tab_id = self
                    .plugins_active_tab_id
                    .as_deref()
                    .and_then(|tab_id| {
                        marketplace_tab_id_matching_saved_id(tab_id, &response.marketplaces)
                    })
                    .or_else(|| self.plugins_active_tab_id.clone());
                self.plugins_active_tab_id = active_tab_id;
                self.plugins_cache = PluginsCacheState::Ready(response.clone());
                if !auth_flow_active {
                    self.refresh_plugins_popup_if_open(&response);
                }
            }
            Err(err) => {
                if !auth_flow_active {
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

    fn prefetch_plugins(&mut self) {
        let cwd = self.config.cwd.to_path_buf();
        if self.plugins_fetch_state.in_flight_cwd.as_deref() == Some(cwd.as_path()) {
            return;
        }

        self.plugins_fetch_state.in_flight_cwd = Some(cwd.clone());
        if self.plugins_fetch_state.cache_cwd.as_deref() != Some(cwd.as_path()) {
            self.plugins_cache = PluginsCacheState::Loading;
        }

        self.app_event_tx.send(AppEvent::FetchPluginsList { cwd });
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

    pub(crate) fn open_marketplace_add_loading_popup(&mut self, source: &str) {
        self.plugins_active_tab_id = Some(ADD_MARKETPLACE_TAB_ID.to_string());
        let params = self.marketplace_add_loading_popup_params(source);
        if !self
            .bottom_pane
            .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
        {
            self.bottom_pane
                .show_selection_view(self.marketplace_add_loading_popup_params(source));
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

    pub(crate) fn on_marketplace_add_loaded(
        &mut self,
        cwd: PathBuf,
        source: String,
        result: Result<MarketplaceAddResponse, String>,
    ) {
        if self.config.cwd.as_path() != cwd.as_path() {
            return;
        }

        match result {
            Ok(response) => {
                self.plugins_active_tab_id =
                    Some(marketplace_tab_id_from_path(&response.installed_root));
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
            Err(err) => {
                self.plugins_active_tab_id = Some(ADD_MARKETPLACE_TAB_ID.to_string());
                let params = self.marketplace_add_error_popup_params(&source, &err);
                if !self
                    .bottom_pane
                    .replace_selection_view_if_active(PLUGINS_SELECTION_VIEW_ID, params)
                {
                    self.bottom_pane.show_selection_view(
                        self.marketplace_add_error_popup_params(&source, &err),
                    );
                }
            }
        }
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
        _marketplace_path: AbsolutePathBuf,
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
                    self.plugin_install_auth_flow = Some(super::PluginInstallAuthFlowState {
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

    fn marketplace_add_loading_popup_params(&self, source: &str) -> SelectionViewParams {
        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(DelayedLoadingHeader::new(
                self.frame_requester.clone(),
                self.config.animations,
                "Adding marketplace...".to_string(),
                Some(source.to_string()),
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

    fn marketplace_add_error_popup_params(&self, source: &str, err: &str) -> SelectionViewParams {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from("Failed to add marketplace.".dim()));

        let mut items = vec![
            SelectionItem {
                name: "Marketplace add failed".to_string(),
                description: Some(err.to_string()),
                is_disabled: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Try again".to_string(),
                description: Some(format!("Enter a marketplace source. Last source: {source}")),
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
                    tx.send(AppEvent::PluginsLoaded {
                        cwd: cwd.clone(),
                        result: Ok(plugins_response.clone()),
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
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Plugins".bold()));
        header.push(Line::from("Failed to load plugin details.".dim()));

        let mut items = vec![SelectionItem {
            name: "Plugin detail unavailable".to_string(),
            description: Some(err.to_string()),
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
                    tx.send(AppEvent::PluginsLoaded {
                        cwd: cwd.clone(),
                        result: Ok(plugins_response.clone()),
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

        let total: usize = marketplaces
            .iter()
            .map(|marketplace| marketplace.plugins.len())
            .sum();
        let installed = marketplaces
            .iter()
            .flat_map(|marketplace| marketplace.plugins.iter())
            .filter(|plugin| plugin.installed)
            .count();

        let all_entries = plugin_entries_for_marketplaces(marketplaces.iter().copied());
        let name_column_width = all_entries
            .iter()
            .map(|(_, _, display_name)| {
                PLUGIN_MENU_ROW_PREFIX_WIDTH + UnicodeWidthStr::width(display_name.as_str())
            })
            .chain([UnicodeWidthStr::width("Add marketplace")])
            .max();
        let installed_entries = all_entries
            .iter()
            .filter(|(_, plugin, _)| plugin.installed)
            .cloned()
            .collect();

        let mut tabs = Vec::new();
        tabs.push(SelectionTab {
            id: ALL_PLUGINS_TAB_ID.to_string(),
            label: "All Plugins".to_string(),
            header: plugins_header(
                "Browse plugins from available marketplaces.".to_string(),
                format!("Installed {installed} of {total} available plugins."),
                response.remote_sync_error.as_deref(),
            ),
            items: self.plugin_selection_items(
                all_entries,
                /*include_marketplace_names*/ true,
                "No marketplace plugins available",
                "No plugins are available in the discovered marketplaces.",
            ),
        });

        tabs.push(SelectionTab {
            id: INSTALLED_PLUGINS_TAB_ID.to_string(),
            label: format!("Installed ({installed})"),
            header: plugins_header(
                "Installed plugins.".to_string(),
                format!("Showing {installed} installed plugins."),
                response.remote_sync_error.as_deref(),
            ),
            items: self.plugin_selection_items(
                installed_entries,
                /*include_marketplace_names*/ true,
                "No installed plugins",
                "No installed plugins.",
            ),
        });

        let curated_marketplace = marketplaces
            .iter()
            .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
            .copied();
        let curated_entries = curated_marketplace
            .map(|marketplace| plugin_entries_for_marketplaces([marketplace]))
            .unwrap_or_default();
        let curated_total = curated_entries.len();
        let curated_installed = curated_entries
            .iter()
            .filter(|(_, plugin, _)| plugin.installed)
            .count();
        tabs.push(SelectionTab {
            id: OPENAI_CURATED_TAB_ID.to_string(),
            label: "OpenAI Curated".to_string(),
            header: plugins_header(
                "OpenAI Curated marketplace.".to_string(),
                format!("Installed {curated_installed} of {curated_total} OpenAI Curated plugins."),
                response.remote_sync_error.as_deref(),
            ),
            items: self.plugin_selection_items(
                curated_entries,
                /*include_marketplace_names*/ false,
                "No OpenAI Curated plugins available",
                "No OpenAI Curated plugins available.",
            ),
        });

        let mut additional_marketplaces: Vec<&PluginMarketplaceEntry> = marketplaces
            .iter()
            .copied()
            .filter(|marketplace| marketplace.name != OPENAI_CURATED_MARKETPLACE_NAME)
            .collect();
        additional_marketplaces.sort_by(|left, right| {
            marketplace_display_name(left)
                .to_ascii_lowercase()
                .cmp(&marketplace_display_name(right).to_ascii_lowercase())
                .then_with(|| marketplace_display_name(left).cmp(&marketplace_display_name(right)))
                .then_with(|| left.name.cmp(&right.name))
        });

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
            tabs.push(SelectionTab {
                id: marketplace_tab_id(marketplace),
                label: label.clone(),
                header: plugins_header(
                    format!("{label}."),
                    format!(
                        "Installed {marketplace_installed} of {marketplace_total} {label} plugins."
                    ),
                    response.remote_sync_error.as_deref(),
                ),
                items: self.plugin_selection_items(
                    entries,
                    /*include_marketplace_names*/ false,
                    "No plugins available in this marketplace",
                    "No plugins available in this marketplace.",
                ),
            });
        }

        tabs.push(self.marketplace_add_tab());

        SelectionViewParams {
            view_id: Some(PLUGINS_SELECTION_VIEW_ID),
            header: Box::new(()),
            footer_hint: Some(plugins_popup_hint_line()),
            tabs,
            initial_tab_id: active_tab_id,
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
            label: "Add Marketplace".to_string(),
            header: plugins_header(
                "Add a marketplace from a Git repo or local root.".to_string(),
                "Enter a source to make its plugins available in this menu.".to_string(),
                /*remote_sync_error*/ None,
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

    fn plugin_detail_popup_params(
        &self,
        plugins_response: &PluginListResponse,
        plugin: &PluginDetail,
    ) -> SelectionViewParams {
        let marketplace_label = plugin.marketplace_name.clone();
        let display_name = plugin_display_name(&plugin.summary);
        let detail_status_label = if plugin.summary.installed {
            if plugin.summary.enabled {
                "Installed"
            } else {
                "Disabled"
            }
        } else {
            match plugin.summary.install_policy {
                PluginInstallPolicy::NotAvailable => "Not installable",
                PluginInstallPolicy::Available => "Can be installed",
                PluginInstallPolicy::InstalledByDefault => "Available by default",
            }
        };
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
                tx.send(AppEvent::PluginsLoaded {
                    cwd: cwd.clone(),
                    result: Ok(plugins_response.clone()),
                });
            })],
            ..Default::default()
        }];

        if plugin.summary.installed {
            let uninstall_cwd = self.config.cwd.to_path_buf();
            let plugin_id = plugin.summary.id.clone();
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
        } else if plugin.summary.install_policy == PluginInstallPolicy::NotAvailable {
            items.push(SelectionItem {
                name: "Install plugin".to_string(),
                description: Some(
                    "This plugin is not installable from this marketplace.".to_string(),
                ),
                is_disabled: true,
                ..Default::default()
            });
        } else {
            let install_cwd = self.config.cwd.to_path_buf();
            let marketplace_path = plugin.marketplace_path.clone();
            let plugin_name = plugin.summary.name.clone();
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
                        marketplace_path: marketplace_path.clone(),
                        plugin_name: plugin_name.clone(),
                        plugin_display_name: plugin_display_name.clone(),
                    });
                })],
                ..Default::default()
            });
        }

        items.push(SelectionItem {
            name: "Skills".to_string(),
            description: Some(plugin_skill_summary(plugin)),
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
            let selected_status_label = format!("{status_label:<status_label_width$}");
            let selected_description = if plugin.installed {
                let toggle_action = if plugin.enabled { "disable" } else { "enable" };
                format!("{selected_status_label}   Space to {toggle_action}; Enter view details.")
            } else {
                format!("{selected_status_label}   Press Enter to view plugin details.")
            };
            let search_value = format!(
                "{display_name} {} {} {}",
                plugin.id, plugin.name, marketplace_label
            );
            let cwd = self.config.cwd.to_path_buf();
            let plugin_display_name = display_name.clone();
            let marketplace_path = marketplace.path.clone();
            let plugin_name = plugin.name.clone();
            let toggle_cwd = cwd.clone();
            let toggle_plugin_id = plugin.id.clone();
            let toggle = plugin.installed.then(|| SelectionToggle {
                is_on: plugin.enabled,
                action: Box::new(move |enabled, tx| {
                    tx.send(AppEvent::SetPluginEnabled {
                        cwd: toggle_cwd.clone(),
                        plugin_id: toggle_plugin_id.clone(),
                        enabled,
                    });
                }),
            });

            items.push(SelectionItem {
                name: display_name,
                toggle,
                toggle_placeholder: (!plugin.installed).then_some("[-] "),
                description: Some(description),
                selected_description: Some(selected_description),
                search_value: Some(search_value),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenPluginDetailLoading {
                        plugin_display_name: plugin_display_name.clone(),
                    });
                    tx.send(AppEvent::FetchPluginDetail {
                        cwd: cwd.clone(),
                        params: codex_app_server_protocol::PluginReadParams {
                            marketplace_path: marketplace_path.clone(),
                            plugin_name: plugin_name.clone(),
                        },
                    });
                })],
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

fn plugins_popup_hint_line() -> Line<'static> {
    Line::from("space enable/disable · ←/→ select marketplace · enter view details · esc close")
}

fn plugin_detail_hint_line() -> Line<'static> {
    Line::from("Press esc to close.")
}

fn plugins_header(
    subtitle: String,
    count_line: String,
    remote_sync_error: Option<&str>,
) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Plugins".bold()));
    header.push(Line::from(subtitle.dim()));
    header.push(Line::from(count_line.dim()));
    if let Some(remote_sync_error) = remote_sync_error {
        header.push(Line::from(
            format!("Using cached marketplace data: {remote_sync_error}").dim(),
        ));
    }
    Box::new(header)
}

fn plugin_entries_for_marketplaces<'a>(
    marketplaces: impl IntoIterator<Item = &'a PluginMarketplaceEntry>,
) -> Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)> {
    marketplaces
        .into_iter()
        .flat_map(|marketplace| {
            marketplace
                .plugins
                .iter()
                .map(move |plugin| (marketplace, plugin, plugin_display_name(plugin)))
        })
        .collect()
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
    marketplace_tab_id_from_path(&marketplace.path)
}

fn marketplace_tab_id_from_path(path: &AbsolutePathBuf) -> String {
    format!("{MARKETPLACE_TAB_ID_PREFIX}{}", path.display())
}

fn marketplace_tab_id_matching_saved_id(
    saved_tab_id: &str,
    marketplaces: &[PluginMarketplaceEntry],
) -> Option<String> {
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
            .as_path()
            .starts_with(root)
            .then(|| marketplace_tab_id(marketplace))
    })
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
    marketplace
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| marketplace.name.clone())
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
            PluginInstallPolicy::InstalledByDefault => "Available",
        }
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

fn plugin_mcp_summary(plugin: &PluginDetail) -> String {
    if plugin.mcp_servers.is_empty() {
        "No plugin MCP servers.".to_string()
    } else {
        plugin.mcp_servers.join(", ")
    }
}
