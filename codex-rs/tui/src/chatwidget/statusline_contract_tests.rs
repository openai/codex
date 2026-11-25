#![cfg(test)]
//! Generate the TypeScript reference and check the exports installed with the authoring skill.

use super::StatuslineState;
use crate::bottom_pane::StatusLineItem;
use crate::ui_tree::UiTree;
use codex_protocol::protocol::RateLimitWindow;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fmt::Write as _;
use std::path::Path;
use strum::IntoEnumIterator;
use ts_rs::TS;

#[test]
fn statusline_typescript_contract_matches_the_bundled_reference() -> color_eyre::Result<()> {
    let output = tempfile::tempdir()?;
    if let Some(destination) = std::env::var_os("UPDATE_STATUSLINE_TYPES") {
        std::fs::write(destination, generate_reference(output.path())?)?;
        return Ok(());
    }

    let home = AbsolutePathBuf::try_from(output.path().to_path_buf())?;
    codex_skills::install_system_skills(&home)?;
    let reference = codex_skills::system_cache_root_dir(&home)
        .join("create-statusline/references/statusline.d.ts");
    let reference = std::fs::read_to_string(reference.as_path())?;
    for name in [
        "StatuslineState",
        "UiTree",
        "RowElement",
        "ColElement",
        "BoxElement",
        "BadgeElement",
        "SpacerElement",
        "CodeElement",
        "StatuslineItemId",
        "Color",
    ] {
        assert!(
            reference.lines().any(|line| {
                line.strip_prefix("export type ")
                    .and_then(|declaration| declaration.split_once('='))
                    .is_some_and(|(declared_name, _)| declared_name.trim_end() == name)
            }),
            "missing export {name} in statusline.d.ts"
        );
    }
    Ok(())
}

fn generate_reference(output: &Path) -> color_eyre::Result<String> {
    StatuslineState::export_all_to(output)?;
    UiTree::export_all_to(output)?;
    let mut generated = std::fs::read_to_string(output.join("statusline.d.ts"))?;
    generated.push('\n');

    // Item IDs use the same display serialization as the native field editor.
    let items = StatusLineItem::iter()
        .map(|item| serde_json::to_string(&item))
        .collect::<Result<Vec<_>, _>>()?;
    writeln!(
        generated,
        "export type StatuslineItemId = {};",
        items.join(" | ")
    )?;
    for name in [
        "Text",
        "Bold",
        "Italic",
        "Underline",
        "Link",
        "Row",
        "Col",
        "Box",
        "Badge",
        "Spacer",
        "Code",
    ] {
        let tag = name.to_ascii_lowercase();
        writeln!(
            generated,
            "export type {name}Element = Extract<Element, {{ type: \"{tag}\" }}>;"
        )?;
    }

    // Column boxes must exclude row-only props even when direction is dynamic.
    generated.push_str("type BoxColumnProps = ColProps & { [K in Exclude<keyof RowProps, keyof ColProps>]?: never };\n");
    writeln!(generated, "export {}", RateLimitWindow::decl())?;
    generated.push_str(
        "export type ThemeColor = Extract<Color, \"primary\" | \"surface\" | \"accent\">;\n\
         export type AnsiColor = Exclude<Color, ThemeColor | StatusColor | HexRgb>;\n",
    );

    let mut generated = generated
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    generated.push('\n');
    Ok(generated)
}
