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
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_core_plugins::is_openai_curated_marketplace_name;
use codex_core_plugins::remote::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME;
use codex_core_plugins::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME;
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
                        tx.send(AppEvent::PluginsLoaded {
                            cwd: cwd_for_cancel.clone(),
                            result: Ok(plugins_response_for_cancel.clone()),
                        });
                    })],
                    ..Default::default()
                },
            ],
            on_cancel: Some(Box::new(move |tx| {
                tx.send(AppEvent::PluginsLoaded {
                    cwd: cwd_for_on_cancel.clone(),
                    result: Ok(plugins_response_for_on_cancel.clone()),
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

    pub(super) fn plugins_popup_params(
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
        tabs.push(SelectionTab {
            id: ALL_PLUGINS_TAB_ID.to_string(),
            label: "All Plugins".to_string(),
            header: plugins_header(
                "Browse plugins from available marketplaces.".to_string(),
                format!("Installed {installed} of {total} available plugins."),
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
            .find(|marketplace| is_openai_curated_marketplace_name(&marketplace.name))
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
            .filter(|marketplace| !is_openai_curated_marketplace_name(&marketplace.name))
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
            tabs.push(SelectionTab {
                id: tab_id,
                label: label.clone(),
                header,
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
            footer_hint: Some(plugins_popup_hint_line(
                /*can_remove_marketplace*/ false, /*can_upgrade_marketplace*/ false,
            )),
            tab_footer_hints,
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
        let marketplace_label = plugin.marketplace_name.clone();
        let display_name = plugin_display_name(&plugin.summary);
        let detail_status_label =
            if plugin.summary.availability == PluginAvailability::DisabledByAdmin {
                "Disabled by admin"
            } else if plugin.summary.installed {
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
            let plugin_detail_request = plugin_detail_request_for_entry(marketplace, plugin);
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

fn plugins_header(subtitle: String, count_line: String) -> Box<dyn Renderable> {
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Plugins".bold()));
    header.push(Line::from(subtitle.dim()));
    header.push(Line::from(count_line.dim()));
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

pub(super) fn merge_remote_marketplaces(
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

pub(super) fn marketplace_display_name(marketplace: &PluginMarketplaceEntry) -> String {
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
            PluginInstallPolicy::InstalledByDefault => "Available",
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
) -> Option<(PluginLocation, String)> {
    plugin_location_for_marketplace(marketplace, plugin)
        .map(|location| (location, plugin_request_name(plugin)))
}

fn plugin_request_name(plugin: &PluginSummary) -> String {
    if matches!(&plugin.source, PluginSource::Remote)
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
    if matches!(&plugin.source, PluginSource::Remote) {
        return plugin_remote_identity(plugin);
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
