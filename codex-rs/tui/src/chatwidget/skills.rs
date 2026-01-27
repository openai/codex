use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use super::ChatWidget;
use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::SkillsToggleItem;
use crate::bottom_pane::SkillsToggleView;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::skills_helpers::skill_description;
use crate::skills_helpers::skill_display_name;
use codex_core::protocol::ListSkillsResponseEvent;
use codex_core::protocol::SkillMetadata as ProtocolSkillMetadata;
use codex_core::protocol::SkillsListEntry;
use codex_core::skills::model::SkillDependencies;
use codex_core::skills::model::SkillInterface;
use codex_core::skills::model::SkillMetadata;
use codex_core::skills::model::SkillToolDependency;

impl ChatWidget {
    pub(crate) fn open_skills_list(&mut self) {
        self.insert_str("$");
    }

    pub(crate) fn open_skills_menu(&mut self) {
        let items = vec![
            SelectionItem {
                name: "List skills".to_string(),
                description: Some("Tip: press $ to open this list directly.".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenSkillsList);
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Enable/Disable Skills".to_string(),
                description: Some("Enable or disable skills.".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenManageSkillsPopup);
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Skills".to_string()),
            subtitle: Some("Choose an action".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_manage_skills_popup(&mut self) {
        if self.skills_all.is_empty() {
            self.add_info_message("No skills available.".to_string(), None);
            return;
        }

        let mut initial_state = HashMap::new();
        for skill in &self.skills_all {
            initial_state.insert(normalize_skill_config_path(&skill.path), skill.enabled);
        }
        self.skills_initial_state = Some(initial_state);

        let items: Vec<SkillsToggleItem> = self
            .skills_all
            .iter()
            .map(|skill| {
                let core_skill = protocol_skill_to_core(skill);
                let display_name = skill_display_name(&core_skill).to_string();
                let description = skill_description(&core_skill).to_string();
                let name = core_skill.name.clone();
                let path = core_skill.path;
                SkillsToggleItem {
                    name: display_name,
                    skill_name: name,
                    description,
                    enabled: skill.enabled,
                    path,
                }
            })
            .collect();

        let view = SkillsToggleView::new(items, self.app_event_tx.clone());
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn update_skill_enabled(&mut self, path: PathBuf, enabled: bool) {
        let target = normalize_skill_config_path(&path);
        for skill in &mut self.skills_all {
            if normalize_skill_config_path(&skill.path) == target {
                skill.enabled = enabled;
            }
        }
        self.set_skills(Some(enabled_skills_for_mentions(&self.skills_all)));
    }

    pub(crate) fn handle_manage_skills_closed(&mut self) {
        let Some(initial_state) = self.skills_initial_state.take() else {
            return;
        };
        let mut current_state = HashMap::new();
        for skill in &self.skills_all {
            current_state.insert(normalize_skill_config_path(&skill.path), skill.enabled);
        }

        let mut enabled_count = 0;
        let mut disabled_count = 0;
        for (path, was_enabled) in initial_state {
            let Some(is_enabled) = current_state.get(&path) else {
                continue;
            };
            if was_enabled != *is_enabled {
                if *is_enabled {
                    enabled_count += 1;
                } else {
                    disabled_count += 1;
                }
            }
        }

        if enabled_count == 0 && disabled_count == 0 {
            return;
        }
        self.add_info_message(
            format!("{enabled_count} skills enabled, {disabled_count} skills disabled"),
            None,
        );
    }

    pub(crate) fn set_skills_from_response(&mut self, response: &ListSkillsResponseEvent) {
        let skills = skills_for_cwd(&self.config.cwd, &response.skills);
        self.skills_all = skills;
        self.set_skills(Some(enabled_skills_for_mentions(&self.skills_all)));
    }
}

fn skills_for_cwd(cwd: &Path, skills_entries: &[SkillsListEntry]) -> Vec<ProtocolSkillMetadata> {
    skills_entries
        .iter()
        .find(|entry| entry.cwd.as_path() == cwd)
        .map(|entry| entry.skills.clone())
        .unwrap_or_default()
}

fn enabled_skills_for_mentions(skills: &[ProtocolSkillMetadata]) -> Vec<SkillMetadata> {
    skills
        .iter()
        .filter(|skill| skill.enabled)
        .map(protocol_skill_to_core)
        .collect()
}

fn protocol_skill_to_core(skill: &ProtocolSkillMetadata) -> SkillMetadata {
    SkillMetadata {
        name: skill.name.clone(),
        description: skill.description.clone(),
        short_description: skill.short_description.clone(),
        interface: skill.interface.clone().map(|interface| SkillInterface {
            display_name: interface.display_name,
            short_description: interface.short_description,
            icon_small: interface.icon_small,
            icon_large: interface.icon_large,
            brand_color: interface.brand_color,
            default_prompt: interface.default_prompt,
        }),
        dependencies: skill
            .dependencies
            .clone()
            .map(|dependencies| SkillDependencies {
                tools: dependencies
                    .tools
                    .into_iter()
                    .map(|tool| SkillToolDependency {
                        r#type: tool.r#type,
                        value: tool.value,
                        description: tool.description,
                        transport: tool.transport,
                        command: tool.command,
                        url: tool.url,
                    })
                    .collect(),
            }),
        path: skill.path.clone(),
        scope: skill.scope,
    }
}

pub(crate) fn find_skill_mentions(text: &str, skills: &[SkillMetadata]) -> Vec<SkillMetadata> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut matches: Vec<SkillMetadata> = Vec::new();
    for skill in skills {
        if seen.contains(&skill.name) {
            continue;
        }
        let needle = format!("${}", skill.name);
        if text.contains(&needle) {
            seen.insert(skill.name.clone());
            matches.push(skill.clone());
        }
    }
    matches
}

fn normalize_skill_config_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
