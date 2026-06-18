use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use super::ChatWidget;
use super::plugins::ADD_MARKETPLACE_TAB_ID;
use super::plugins::ALL_PLUGINS_TAB_ID;
use super::plugins::PLUGINS_SELECTION_VIEW_ID;
use super::plugins::PluginsCacheState;
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
use crate::key_hint;
use crate::legacy_core::config::Config;
use crate::motion::MotionMode;
use crate::motion::shimmer_text;
use crate::onboarding::mark_url_hyperlink;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginShareContext;
use codex_app_server_protocol::PluginShareDiscoverability;
use codex_app_server_protocol::PluginSharePrincipal;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_core_plugins::is_openai_curated_marketplace_name;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
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

const INSTALLED_PLUGINS_TAB_ID: &str = "installed-plugins";
const MARKETPLACE_TAB_ID_PREFIX: &str = "marketplace:";
const OPENAI_CURATED_TAB_ID: &str = "marketplace:openai-curated";
const PLUGIN_ROW_PREFIX_WIDTH: usize = 6;
const LOADING_ANIMATION_DELAY: Duration = Duration::from_secs(1);
const LOADING_ANIMATION_INTERVAL: Duration = Duration::from_millis(100);
const APPS_HELP_ARTICLE_URL: &str = "https://help.openai.com/en/articles/11487775-apps-in-chatgpt";
const PERSONAL_MARKETPLACE_RELATIVE_PATH: &str = ".agents/plugins/marketplace.json";
const REMOTE_LOADING_TAB_ID_PREFIX: &str = "remote-loading:";
const REMOTE_EMPTY_TAB_ID_PREFIX: &str = "remote-empty:";
const REMOTE_ERROR_TAB_ID_PREFIX: &str = "remote-error:";
const WORKSPACE_SECTION_TAB_ORDER: u8 = 0;
const SHARED_WITH_ME_SECTION_TAB_ORDER: u8 = 1;
const SHARED_WITH_ME_LINK_SECTION_TAB_ORDER: u8 = 2;
const LOCAL_MARKETPLACE_TAB_ORDER: u8 = 3;
const OTHER_MARKETPLACE_TAB_ORDER: u8 = 4;

#[derive(Debug, Clone)]
struct PreferredLocalPluginSource {
    marketplace_path: AbsolutePathBuf,
    plugin_name: String,
    installed: bool,
}

#[derive(Debug, Clone, Copy)]
enum MarketplaceProduct {
    OpenAiCurated,
    Workspace,
    SharedWithMe,
    SharedWithMeLink,
    Local,
    Other,
}

impl MarketplaceProduct {
    fn from_marketplace(marketplace: &PluginMarketplaceEntry) -> Self {
        Self::from_marketplace_parts(&marketplace.name, marketplace.path.as_ref())
    }

    fn from_marketplace_parts(
        marketplace_name: &str,
        marketplace_path: Option<&AbsolutePathBuf>,
    ) -> Self {
        if marketplace_path.is_some_and(is_personal_marketplace_path) {
            return Self::Local;
        }

        Self::from_marketplace_name(marketplace_name)
    }

    fn from_marketplace_name(marketplace_name: &str) -> Self {
        if is_openai_curated_marketplace_name(marketplace_name)
            || marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME
        {
            return Self::OpenAiCurated;
        }

        match marketplace_name {
            REMOTE_WORKSPACE_MARKETPLACE_NAME => Self::Workspace,
            REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
            | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME => Self::SharedWithMe,
            REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME => Self::SharedWithMeLink,
            _ => Self::Other,
        }
    }

    fn label(self) -> Option<&'static str> {
        match self {
            Self::OpenAiCurated => Some("OpenAI Curated"),
            Self::Workspace => Some("Workspace"),
            Self::SharedWithMe => Some("Shared with me"),
            Self::SharedWithMeLink => Some("Shared with me (link)"),
            Self::Local => Some("Local"),
            Self::Other => None,
        }
    }

    fn tab_order(self) -> u8 {
        match self {
            Self::Workspace => WORKSPACE_SECTION_TAB_ORDER,
            Self::SharedWithMe => SHARED_WITH_ME_SECTION_TAB_ORDER,
            Self::SharedWithMeLink => SHARED_WITH_ME_LINK_SECTION_TAB_ORDER,
            Self::Local => LOCAL_MARKETPLACE_TAB_ORDER,
            Self::OpenAiCurated | Self::Other => OTHER_MARKETPLACE_TAB_ORDER,
        }
    }

    fn is_by_openai(self) -> bool {
        matches!(self, Self::OpenAiCurated)
    }
}

#[derive(Debug, Clone, Copy)]
struct RemoteMarketplaceSection {
    id: &'static str,
    label: &'static str,
    loading_tab_id: &'static str,
    marketplace_names: &'static [&'static str],
    empty_item_name: &'static str,
    empty_item_description: &'static str,
    tab_order: u8,
}

const REMOTE_MARKETPLACE_SECTIONS: [RemoteMarketplaceSection; 2] = [
    RemoteMarketplaceSection {
        id: "workspace",
        label: "Workspace",
        loading_tab_id: "workspace-loading",
        marketplace_names: &[REMOTE_WORKSPACE_MARKETPLACE_NAME],
        empty_item_name: "No workspace plugins available",
        empty_item_description: "No workspace directory plugins are available.",
        tab_order: WORKSPACE_SECTION_TAB_ORDER,
    },
    RemoteMarketplaceSection {
        id: "shared-with-me",
        label: "Shared with me",
        loading_tab_id: "shared-with-me-loading",
        marketplace_names: &[
            REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME,
            REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME,
            REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME,
        ],
        empty_item_name: "No shared plugins available",
        empty_item_description: "No plugins have been shared with you.",
        tab_order: SHARED_WITH_ME_SECTION_TAB_ORDER,
    },
];

impl RemoteMarketplaceSection {
    fn fallback_tab(
        self,
        marketplaces: &[PluginMarketplaceEntry],
        remote_sections_loading: bool,
        remote_sections_loaded: bool,
        section_errors: &[PluginRemoteSectionError],
    ) -> Option<(u8, SelectionTab)> {
        if marketplaces
            .iter()
            .any(|marketplace| self.contains_marketplace(&marketplace.name))
        {
            return None;
        }

        let tab = if remote_sections_loading {
            remote_section_loading_tab(self.loading_tab_id, self.label)
        } else if remote_sections_loaded {
            if let Some(section_error) = plugin_remote_section_error(section_errors, self.id) {
                remote_section_error_tab(section_error)
            } else {
                remote_section_empty_tab(
                    self.id,
                    self.label,
                    self.empty_item_name,
                    self.empty_item_description,
                )
            }
        } else {
            return None;
        };

        Some((self.tab_order, tab))
    }

    fn contains_marketplace(self, marketplace_name: &str) -> bool {
        self.marketplace_names.contains(&marketplace_name)
    }

    fn is_fallback_tab_id(self, tab_id: &str) -> bool {
        tab_id.strip_prefix(REMOTE_LOADING_TAB_ID_PREFIX) == Some(self.loading_tab_id)
            || tab_id.strip_prefix(REMOTE_EMPTY_TAB_ID_PREFIX) == Some(self.id)
            || tab_id.strip_prefix(REMOTE_ERROR_TAB_ID_PREFIX) == Some(self.id)
    }

    fn contains_tab_id(self, tab_id: &str) -> bool {
        self.is_fallback_tab_id(tab_id)
            || tab_id
                .strip_prefix(MARKETPLACE_TAB_ID_PREFIX)
                .is_some_and(|marketplace_name| self.contains_marketplace(marketplace_name))
    }
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

impl ChatWidget {
    pub(super) fn plugins_loading_popup_params(&self) -> SelectionViewParams {
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

    pub(super) fn marketplace_add_loading_popup_params(&self) -> SelectionViewParams {
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

    pub(super) fn marketplace_remove_confirmation_popup_params(
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

    pub(super) fn marketplace_remove_loading_popup_params(
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

    pub(super) fn marketplace_upgrade_loading_popup_params(
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

    pub(super) fn plugin_detail_loading_popup_params(
        &self,
        plugin_display_name: &str,
    ) -> SelectionViewParams {
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

    pub(super) fn plugin_install_loading_popup_params(
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

    pub(super) fn plugin_uninstall_loading_popup_params(
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

    pub(super) fn plugins_error_popup_params(&self, err: &str) -> SelectionViewParams {
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

    pub(super) fn marketplace_add_error_popup_params(&self) -> SelectionViewParams {
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

    pub(super) fn marketplace_remove_error_popup_params(
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

    pub(super) fn plugin_detail_error_popup_params(
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

    pub(super) fn plugins_popup_params(
        &self,
        response: &PluginListResponse,
        active_tab_id: Option<String>,
        initial_selected_idx: Option<usize>,
    ) -> SelectionViewParams {
        let marketplaces = &response.marketplaces;
        let preferred_local_sources = preferred_local_plugin_sources(marketplaces);

        let all_entries = plugin_entries_for_marketplaces(marketplaces);
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
        let all_items = self.plugin_selection_items(
            all_entries,
            &preferred_local_sources,
            /*include_marketplace_names*/ true,
            "No marketplace plugins available",
            "No plugins are available in the discovered marketplaces.",
        );

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

        let curated_entries =
            plugin_entries_for_marketplaces(marketplaces.iter().filter(|marketplace| {
                MarketplaceProduct::from_marketplace(marketplace).is_by_openai()
            }));
        let curated_total = curated_entries.len();
        let curated_installed = curated_entries
            .iter()
            .filter(|(_, plugin, _)| plugin.installed)
            .count();
        let curated_has_entries = !curated_entries.is_empty();
        let curated_loading = self.plugin_remote_sections_loading
            && self.plugins_fetch_state.vertical_section_requested;
        let by_openai_section_error =
            plugin_remote_section_error(&self.plugin_remote_section_errors, "vertical");
        let (curated_empty_name, curated_empty_description) =
            if curated_loading && !curated_has_entries {
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
        if curated_loading && curated_has_entries {
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
            .filter(|marketplace| !MarketplaceProduct::from_marketplace(marketplace).is_by_openai())
            .collect();
        additional_marketplaces.sort_by_cached_key(|marketplace| {
            let display_name = marketplace_display_name(marketplace);
            (
                MarketplaceProduct::from_marketplace(marketplace).tab_order(),
                display_name.to_ascii_lowercase(),
                display_name,
                marketplace.name.clone(),
            )
        });

        let mut additional_tabs = Vec::new();
        for section in REMOTE_MARKETPLACE_SECTIONS {
            if let Some(fallback_tab) = section.fallback_tab(
                marketplaces,
                self.plugin_remote_sections_loading,
                self.plugin_remote_sections_loaded,
                &self.plugin_remote_section_errors,
            ) {
                additional_tabs.push(fallback_tab);
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
                MarketplaceProduct::from_marketplace(marketplace).tab_order(),
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
            label: "Add Marketplace".to_string(),
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

    pub(super) fn plugin_detail_popup_params(
        &self,
        plugins_response: &PluginListResponse,
        plugin: &PluginDetail,
    ) -> SelectionViewParams {
        let marketplace_label = MarketplaceProduct::from_marketplace_parts(
            &plugin.marketplace_name,
            plugin.marketplace_path.as_ref(),
        )
        .label()
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
        preferred_local_sources: &HashMap<String, PreferredLocalPluginSource>,
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
            let disabled_by_admin = plugin.availability == PluginAvailability::DisabledByAdmin;
            let can_toggle_plugin = plugin.installed && !disabled_by_admin;
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
            } else if disabled_by_admin && can_view_details {
                format!("{selected_status_label}   Press Enter to view plugin details.")
            } else if can_view_details {
                format!("{selected_status_label}   Press Enter to install or view plugin details.")
            } else {
                format!("{selected_status_label}   Remote plugin details are not available yet.")
            };
            let search_value = format!(
                "{display_name} {} {} {}",
                plugin.id, plugin.name, marketplace_label
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

pub(super) fn plugin_detail_hint_line() -> Line<'static> {
    Line::from("Press esc to close.")
}

pub(super) fn plugins_header(subtitle: String, count_line: String) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Plugins".bold()));
    header.push(Line::from(subtitle.dim()));
    header.push(Line::from(count_line.dim()));
    Box::new(header)
}

fn dedupe_plugin_entries<'a>(
    entries: Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)>,
) -> Vec<(&'a PluginMarketplaceEntry, &'a PluginSummary, String)> {
    // App-server should eventually normalize local/remote duplicates. Keep this
    // display-only pass narrow so shared plugins do not appear twice meanwhile.
    let mut deduped: Vec<(&PluginMarketplaceEntry, &PluginSummary, String)> = Vec::new();
    let mut remote_entry_indexes = HashMap::new();
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
    if candidate.1.installed != existing.1.installed {
        return candidate.1.installed;
    }

    let candidate_is_local_share =
        candidate.1.share_context.is_some() && !matches!(&candidate.1.source, PluginSource::Remote);
    let existing_is_local_share =
        existing.1.share_context.is_some() && !matches!(&existing.1.source, PluginSource::Remote);
    if candidate_is_local_share != existing_is_local_share {
        return candidate_is_local_share;
    }

    !matches!(&candidate.1.source, PluginSource::Remote)
        && matches!(&existing.1.source, PluginSource::Remote)
}

fn preferred_local_plugin_sources(
    marketplaces: &[PluginMarketplaceEntry],
) -> HashMap<String, PreferredLocalPluginSource> {
    let mut sources = HashMap::new();
    for marketplace in marketplaces {
        let Some(marketplace_path) = marketplace.path.as_ref() else {
            continue;
        };
        for plugin in &marketplace.plugins {
            if matches!(&plugin.source, PluginSource::Remote) {
                continue;
            }
            let Some(share_context) = plugin.share_context.as_ref() else {
                continue;
            };
            sources
                .entry(share_context.remote_plugin_id.clone())
                .or_insert_with(|| PreferredLocalPluginSource {
                    marketplace_path: marketplace_path.clone(),
                    plugin_name: plugin.name.clone(),
                    installed: plugin.installed,
                });
        }
    }
    sources
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
            let marketplace_label =
                MarketplaceProduct::from_marketplace_name(&plugin.marketplace_name)
                    .label()
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

pub(super) fn marketplace_tab_id(marketplace: &PluginMarketplaceEntry) -> String {
    match marketplace.path.as_ref() {
        Some(path) => marketplace_tab_id_from_path(path.as_path()),
        None => format!("marketplace:{}", marketplace.name),
    }
}

pub(super) fn marketplace_tab_id_from_path(path: &Path) -> String {
    format!("{MARKETPLACE_TAB_ID_PREFIX}{}", path.display())
}

pub(super) fn marketplace_tab_id_matching_saved_id(
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
    let section = REMOTE_MARKETPLACE_SECTIONS
        .into_iter()
        .find(|section| section.is_fallback_tab_id(saved_tab_id))?;

    section
        .marketplace_names
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

    let section = REMOTE_MARKETPLACE_SECTIONS
        .into_iter()
        .find(|section| section.contains_tab_id(saved_tab_id))?;

    tabs.iter()
        .find(|tab| section.contains_tab_id(&tab.id))
        .map(|tab| tab.id.clone())
}

pub(super) fn merge_remote_marketplaces(
    response: &mut PluginListResponse,
    remote_marketplaces: Vec<PluginMarketplaceEntry>,
) {
    let remote_names = remote_marketplaces
        .iter()
        .map(|marketplace| marketplace.name.clone())
        .collect::<std::collections::HashSet<_>>();
    let remote_curated_present = remote_names.contains(REMOTE_GLOBAL_MARKETPLACE_NAME);
    response.marketplaces.retain(|marketplace| {
        if remote_curated_present
            && marketplace.path.is_some()
            && is_openai_curated_marketplace_name(&marketplace.name)
        {
            return false;
        }

        marketplace.path.is_some()
            || !REMOTE_MARKETPLACE_SECTIONS
                .into_iter()
                .any(|section| section.contains_marketplace(&marketplace.name))
                && !remote_names.contains(marketplace.name.as_str())
    });
    response.marketplaces.extend(remote_marketplaces);
}

fn is_personal_marketplace_path(marketplace_path: &AbsolutePathBuf) -> bool {
    dirs::home_dir()
        .and_then(|home| {
            AbsolutePathBuf::try_from(home.join(PERSONAL_MARKETPLACE_RELATIVE_PATH)).ok()
        })
        .is_some_and(|personal_path| personal_path.as_path() == marketplace_path.as_path())
}

fn remote_section_loading_item(label: &str) -> SelectionItem {
    SelectionItem {
        name: format!("Loading {label} plugins..."),
        description: Some("This section updates when app-server returns it.".to_string()),
        is_disabled: true,
        ..Default::default()
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

fn plugin_remote_section_error<'a>(
    section_errors: &'a [PluginRemoteSectionError],
    section_id: &str,
) -> Option<&'a PluginRemoteSectionError> {
    section_errors
        .iter()
        .find(|section_error| section_error.section_id == section_id)
}

fn remote_section_loading_tab(id: &str, label: &str) -> SelectionTab {
    SelectionTab {
        id: format!("{REMOTE_LOADING_TAB_ID_PREFIX}{id}"),
        label: label.to_string(),
        header: plugins_header(
            format!("Loading {label} plugins."),
            "Local plugin functionality is already available.".to_string(),
        ),
        items: vec![remote_section_loading_item(label)],
    }
}

fn remote_section_empty_tab(
    id: &str,
    label: &str,
    item_name: &str,
    item_description: &str,
) -> SelectionTab {
    SelectionTab {
        id: format!("{REMOTE_EMPTY_TAB_ID_PREFIX}{id}"),
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
        id: format!("{REMOTE_ERROR_TAB_ID_PREFIX}{}", section_error.section_id),
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

fn disambiguate_duplicate_tab_labels(labels: Vec<String>) -> Vec<String> {
    let mut counts = HashMap::new();
    for label in &labels {
        *counts.entry(label.clone()).or_insert(0) += 1;
    }

    let mut seen = HashMap::new();
    labels
        .into_iter()
        .map(|label| {
            let total = counts[&label];
            if total == 1 {
                return label;
            }

            let current = seen.entry(label.clone()).or_insert(0);
            *current += 1;
            format!("{label} ({current}/{total})")
        })
        .collect()
}

pub(super) fn marketplace_display_name(marketplace: &PluginMarketplaceEntry) -> String {
    if let Some(label) = MarketplaceProduct::from_marketplace(marketplace).label() {
        return label.to_string();
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

pub(super) fn marketplace_is_user_configured(config: &Config, marketplace_name: &str) -> bool {
    let Some(user_config) = config.config_layer_stack.effective_user_config() else {
        return false;
    };
    user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
        .is_some_and(|marketplaces| marketplaces.contains_key(marketplace_name))
}

pub(super) fn marketplace_is_user_configured_git(config: &Config, marketplace_name: &str) -> bool {
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
    preferred_local_sources: &HashMap<String, PreferredLocalPluginSource>,
) -> Option<(PluginLocation, String)> {
    if matches!(&plugin.source, PluginSource::Remote)
        && let Some(remote_plugin_id) = plugin_remote_identity(plugin)
        && let Some(preferred_source) = preferred_local_sources.get(remote_plugin_id)
        && preferred_source.installed == plugin.installed
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

fn plugin_request_name(plugin: &PluginSummary) -> String {
    if matches!(&plugin.source, PluginSource::Remote)
        && let Some(remote_plugin_id) = plugin_remote_identity(plugin)
    {
        return remote_plugin_id.to_string();
    }
    plugin.name.clone()
}

fn plugin_remote_identity(plugin: &PluginSummary) -> Option<&str> {
    plugin
        .share_context
        .as_ref()
        .map(|context| context.remote_plugin_id.as_str())
        .or(plugin.remote_plugin_id.as_deref())
}

fn plugin_uninstall_id(plugin: &PluginSummary) -> Option<String> {
    if matches!(&plugin.source, PluginSource::Remote) {
        return plugin_remote_identity(plugin).map(str::to_string);
    }
    Some(plugin.id.clone())
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
