use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub root: Style,
    pub content: Style,
    pub app_title: Style,
    pub tabs: Style,
    pub tabs_selected: Style,
    pub borders: Style,
    pub description: Style,
    pub description_title: Style,
    pub key_binding: KeyBinding,
    pub logo: Logo,
    pub email: Email,
    pub traceroute: Traceroute,
    pub recipe: Recipe,
}

pub struct KeyBinding {
    pub key: Style,
    pub description: Style,
}

pub struct Logo {
    pub rat_eye: Color,
    pub rat_eye_alt: Color,
}

pub struct Email {
    pub tabs: Style,
    pub tabs_selected: Style,
    pub inbox: Style,
    pub item: Style,
    pub selected_item: Style,
    pub header: Style,
    pub header_value: Style,
    pub body: Style,
}

pub struct Traceroute {
    pub header: Style,
    pub selected: Style,
    pub ping: Style,
    pub map: Map,
}

pub struct Map {
    pub style: Style,
    pub color: Color,
    pub path: Color,
    pub source: Color,
    pub destination: Color,
    pub background_color: Color,
}

pub struct Recipe {
    pub ingredients: Style,
    pub ingredients_header: Style,
}

pub const THEME: Theme = Theme {
    root: Style::new().bg(DARK_BLUE),
    content: Style::new().bg(DARK_BLUE).fg(LIGHT_GRAY),
    app_title: Style::new()
        .fg(WHITE)
        .bg(DARK_BLUE)
        .add_modifier(Modifier::BOLD),
    tabs: Style::new().fg(MID_GRAY).bg(DARK_BLUE),
    tabs_selected: Style::new()
        .fg(WHITE)
        .bg(DARK_BLUE)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::REVERSED),
    borders: Style::new().fg(LIGHT_GRAY),
    description: Style::new().fg(LIGHT_GRAY).bg(DARK_BLUE),
    description_title: Style::new().fg(LIGHT_GRAY).add_modifier(Modifier::BOLD),
    logo: Logo {
        rat_eye: BLACK,
        rat_eye_alt: RED,
    },
    key_binding: KeyBinding {
        key: Style::new().fg(BLACK).bg(DARK_GRAY),
        description: Style::new().fg(DARK_GRAY).bg(BLACK),
    },
    email: Email {
        tabs: Style::new().fg(MID_GRAY).bg(DARK_BLUE),
        tabs_selected: Style::new()
            .fg(WHITE)
            .bg(DARK_BLUE)
            .add_modifier(Modifier::BOLD),
        inbox: Style::new().bg(DARK_BLUE).fg(LIGHT_GRAY),
        item: Style::new().fg(LIGHT_GRAY),
        selected_item: Style::new().fg(LIGHT_YELLOW),
        header: Style::new().add_modifier(Modifier::BOLD),
        header_value: Style::new().fg(LIGHT_GRAY),
        body: Style::new().bg(DARK_BLUE).fg(LIGHT_GRAY),
    },
    traceroute: Traceroute {
        header: Style::new()
            .bg(DARK_BLUE)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
        selected: Style::new().fg(LIGHT_YELLOW),
        ping: Style::new().fg(WHITE),
        map: Map {
            style: Style::new().bg(DARK_BLUE),
            background_color: DARK_BLUE,
            color: LIGHT_GRAY,
            path: LIGHT_BLUE,
            source: LIGHT_GREEN,
            destination: LIGHT_RED,
        },
    },
    recipe: Recipe {
        ingredients: Style::new().bg(DARK_BLUE).fg(LIGHT_GRAY),
        ingredients_header: Style::new()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    },
};

const DARK_BLUE: Color = Color::Rgb(16, 24, 48);
const LIGHT_BLUE: Color = Color::Rgb(64, 96, 192);
const LIGHT_YELLOW: Color = Color::Rgb(192, 192, 96);
const LIGHT_GREEN: Color = Color::Rgb(64, 192, 96);
const LIGHT_RED: Color = Color::Rgb(192, 96, 96);
const RED: Color = Color::Rgb(215, 0, 0);
const BLACK: Color = Color::Rgb(8, 8, 8); // not really black, often #080808
const DARK_GRAY: Color = Color::Rgb(68, 68, 68);
const MID_GRAY: Color = Color::Rgb(128, 128, 128);
const LIGHT_GRAY: Color = Color::Rgb(188, 188, 188);
const WHITE: Color = Color::Rgb(238, 238, 238); // not really white, often #eeeeee
