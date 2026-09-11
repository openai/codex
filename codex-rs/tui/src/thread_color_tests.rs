use super::*;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use syntect::highlighting::Color as ThemeColor;
use syntect::highlighting::StyleModifier;
use syntect::highlighting::ThemeItem;
use two_face::theme::EmbeddedThemeName;

#[test]
fn assignments_reach_all_fourteen_default_accents() {
    let themes = two_face::theme::extra();
    for name in [
        EmbeddedThemeName::CatppuccinLatte,
        EmbeddedThemeName::CatppuccinMocha,
    ] {
        let colors = theme_accents(themes.get(name));
        let assigned: HashSet<_> = (0..512)
            .map(|id| color_for_id(ThreadId::from_u128(id), &colors))
            .collect();
        assert_eq!(assigned.len(), 14);
        assert_eq!(assigned, colors.iter().copied().collect());
    }
}

#[test]
fn theme_colors_are_preserved_without_a_replacement_palette() {
    let foreground = ThemeColor {
        r: 120,
        g: 120,
        b: 120,
        a: 255,
    };
    let accent = ThemeColor {
        r: 121,
        ..foreground
    };
    let mut theme = Theme::default();
    theme.settings.foreground = Some(foreground);
    theme.scopes = [foreground, accent, accent]
        .into_iter()
        .map(|color| ThemeItem {
            scope: "keyword".parse().unwrap(),
            style: StyleModifier {
                foreground: Some(color),
                ..StyleModifier::default()
            },
        })
        .collect();
    theme.scopes.push(ThemeItem {
        scope: "invalid".parse().unwrap(),
        style: StyleModifier {
            foreground: Some(ThemeColor {
                r: 122,
                ..foreground
            }),
            background: Some(accent),
            ..StyleModifier::default()
        },
    });
    let colors = theme_accents(&theme);
    assert_eq!(colors, vec![convert_syntect_color(accent).unwrap()]);
    assert_eq!(
        color_for_id(ThreadId::from_u128(/*value*/ 1), &colors),
        colors[0]
    );
    theme.scopes.clear();
    assert_eq!(
        color_for_id(ThreadId::from_u128(/*value*/ 1), &theme_accents(&theme)),
        Color::Reset
    );
}

#[test]
fn theme_revision_refreshes_the_cached_palette() {
    let id = ThreadId::from_u128(/*value*/ 1);
    let revision = syntax_theme_revision();
    PALETTE.set(Some((revision.wrapping_sub(1), Vec::new())));
    assert_eq!(
        thread_color(id),
        color_for_id(id, &theme_accents(&current_syntax_theme()))
    );
    PALETTE.with_borrow(|cached| assert_eq!(cached.as_ref().unwrap().0, revision));
}
