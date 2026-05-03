//! Builds the /pets picker dialog for the TUI.

use std::fs;
use std::path::Path;

use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;

use super::DEFAULT_PET_ID;
use super::model::Pet;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PetPickerEntry {
    selector: String,
    display_name: String,
    description: Option<String>,
}

pub(crate) fn build_pet_picker_params(
    current_pet: Option<&str>,
    codex_home: &Path,
) -> SelectionViewParams {
    let current_pet = current_pet.unwrap_or(DEFAULT_PET_ID);
    let mut entries = available_pet_entries(codex_home);
    entries.sort_by(|left, right| left.display_name.cmp(&right.display_name));

    let mut initial_selected_idx = None;
    let items = entries
        .into_iter()
        .enumerate()
        .map(|(idx, entry)| {
            let is_current = current_pet == entry.selector;
            if is_current {
                initial_selected_idx = Some(idx);
            }
            let pet_id = entry.selector.clone();
            SelectionItem {
                name: entry.display_name,
                description: entry.description,
                is_current,
                dismiss_on_select: true,
                search_value: Some(entry.selector),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::PetSelected {
                        pet_id: pet_id.clone(),
                    });
                })],
                ..Default::default()
            }
        })
        .collect();

    SelectionViewParams {
        title: Some("Select Pet".to_string()),
        subtitle: Some("Choose a pet to wake in the terminal.".to_string()),
        footer_hint: Some(standard_popup_hint_line()),
        items,
        is_searchable: true,
        search_placeholder: Some("Type to filter pets...".to_string()),
        initial_selected_idx,
        ..Default::default()
    }
}

fn available_pet_entries(codex_home: &Path) -> Vec<PetPickerEntry> {
    let mut entries = vec![pet_picker_entry(DEFAULT_PET_ID), pet_picker_entry("boba")];
    let pets_dir = codex_home.join("pets");
    let Ok(children) = fs::read_dir(pets_dir) else {
        return entries;
    };

    for child in children.flatten() {
        let path = child.path();
        if !path.join("pet.json").is_file() {
            continue;
        }
        let Some(selector) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        entries.push(pet_picker_entry_from_path(selector, &path));
    }
    entries
}

fn pet_picker_entry(selector: &str) -> PetPickerEntry {
    match Pet::load(selector) {
        Ok(pet) => PetPickerEntry {
            selector: selector.to_string(),
            display_name: pet.display_name,
            description: (!pet.description.is_empty()).then_some(pet.description),
        },
        Err(_) => PetPickerEntry {
            selector: selector.to_string(),
            display_name: selector.to_string(),
            description: None,
        },
    }
}

fn pet_picker_entry_from_path(selector: &str, path: &Path) -> PetPickerEntry {
    match Pet::load(path.to_string_lossy().as_ref()) {
        Ok(pet) => PetPickerEntry {
            selector: selector.to_string(),
            display_name: pet.display_name,
            description: (!pet.description.is_empty()).then_some(pet.description),
        },
        Err(_) => PetPickerEntry {
            selector: selector.to_string(),
            display_name: selector.to_string(),
            description: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn write_pet(dir: &Path, folder_name: &str, display_name: &str) {
        let pet_dir = dir.join("pets").join(folder_name);
        fs::create_dir_all(&pet_dir).unwrap();
        fs::write(
            pet_dir.join("pet.json"),
            format!(
                r#"{{
                    "id": "{folder_name}",
                    "displayName": "{display_name}",
                    "description": "custom pet",
                    "spritesheetPath": "spritesheet.webp"
                }}"#
            ),
        )
        .unwrap();
        fs::File::create(pet_dir.join("spritesheet.webp"))
            .unwrap()
            .write_all(b"not-used-by-loader")
            .unwrap();
    }

    #[test]
    fn picker_lists_bundled_and_installed_pets() {
        let codex_home = tempfile::tempdir().unwrap();
        write_pet(codex_home.path(), "chefito", "Chefito");

        let params = build_pet_picker_params(Some("chefito"), codex_home.path());

        assert_eq!(
            params
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Boba", "Chefito", "Codex"],
        );
        assert_eq!(params.initial_selected_idx, Some(1));
    }

    #[test]
    fn picker_defaults_to_codex_when_no_pet_is_configured() {
        let codex_home = tempfile::tempdir().unwrap();
        let params = build_pet_picker_params(/*current_pet*/ None, codex_home.path());

        assert_eq!(params.initial_selected_idx, Some(1));
        assert_eq!(params.items[1].name, "Codex");
        assert!(params.items[1].is_current);
    }
}
