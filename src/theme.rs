use crate::config::CustomTheme;
use crate::{AppWindow, Theme, ThemeItem};
use slint::{Color, ComponentHandle, SharedString};

#[derive(Debug, Clone)]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    pub accent: String,
    pub accent_hover: String,
    pub accent_soft: String,
    pub bg_canvas: String,
    pub bg_sidebar: String,
    pub bg_elevated: String,
    pub bg_input: String,
    pub bg_transport: String,
    pub bg_row_alt: String,
    pub bg_hover: String,
    pub bg_selected: String,
    pub border_subtle: String,
    pub green_badge: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub is_custom: bool,
}

pub fn parse_hex_color(hex_str: &str) -> Color {
    let clean = hex_str.trim().trim_start_matches('#');
    match clean.len() {
        6 => {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
            Color::from_argb_u8(255, r, g, b)
        }
        8 => {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
            let a = u8::from_str_radix(&clean[6..8], 16).unwrap_or(255);
            Color::from_argb_u8(a, r, g, b)
        }
        _ => Color::from_argb_u8(255, 250, 45, 72), // fallback
    }
}

pub fn get_bundled_themes() -> Vec<ThemeDefinition> {
    vec![
        ThemeDefinition {
            id: "default".to_string(),
            name: "Apple Dark (Red)".to_string(),
            accent: "#fa2d48".to_string(),
            accent_hover: "#ff455d".to_string(),
            accent_soft: "#fa2d4826".to_string(),
            bg_canvas: "#101012".to_string(),
            bg_sidebar: "#161619".to_string(),
            bg_elevated: "#1d1d21".to_string(),
            bg_input: "#18181c".to_string(),
            bg_transport: "#17171a".to_string(),
            bg_row_alt: "#131316".to_string(),
            bg_hover: "#222227".to_string(),
            bg_selected: "#28161b".to_string(),
            border_subtle: "#26262b".to_string(),
            green_badge: "#30d158".to_string(),
            text_primary: "#f5f5f7".to_string(),
            text_secondary: "#98989f".to_string(),
            text_muted: "#636369".to_string(),
            is_custom: false,
        },
        ThemeDefinition {
            id: "emerald".to_string(),
            name: "Emerald (Green)".to_string(),
            accent: "#30d158".to_string(),
            accent_hover: "#3cd964".to_string(),
            accent_soft: "#30d15826".to_string(),
            bg_canvas: "#0c130e".to_string(),
            bg_sidebar: "#111a14".to_string(),
            bg_elevated: "#16221a".to_string(),
            bg_input: "#141f17".to_string(),
            bg_transport: "#121b15".to_string(),
            bg_row_alt: "#0e1611".to_string(),
            bg_hover: "#1c2c22".to_string(),
            bg_selected: "#183324".to_string(),
            border_subtle: "#1f3427".to_string(),
            green_badge: "#30d158".to_string(),
            text_primary: "#f2f8f4".to_string(),
            text_secondary: "#8ea698".to_string(),
            text_muted: "#5b7264".to_string(),
            is_custom: false,
        },
        ThemeDefinition {
            id: "ocean".to_string(),
            name: "Ocean (Blue)".to_string(),
            accent: "#0a84ff".to_string(),
            accent_hover: "#409cff".to_string(),
            accent_soft: "#0a84ff26".to_string(),
            bg_canvas: "#0b111a".to_string(),
            bg_sidebar: "#101724".to_string(),
            bg_elevated: "#162031".to_string(),
            bg_input: "#131c2b".to_string(),
            bg_transport: "#111927".to_string(),
            bg_row_alt: "#0d141f".to_string(),
            bg_hover: "#1b283d".to_string(),
            bg_selected: "#142d50".to_string(),
            border_subtle: "#1e2f47".to_string(),
            green_badge: "#30d158".to_string(),
            text_primary: "#f0f4f9".to_string(),
            text_secondary: "#8ea3bc".to_string(),
            text_muted: "#566b82".to_string(),
            is_custom: false,
        },
        ThemeDefinition {
            id: "amethyst".to_string(),
            name: "Amethyst (Purple)".to_string(),
            accent: "#bf5af2".to_string(),
            accent_hover: "#c974f4".to_string(),
            accent_soft: "#bf5af226".to_string(),
            bg_canvas: "#120e18".to_string(),
            bg_sidebar: "#181320".to_string(),
            bg_elevated: "#211a2c".to_string(),
            bg_input: "#1c1626".to_string(),
            bg_transport: "#191423".to_string(),
            bg_row_alt: "#140f1c".to_string(),
            bg_hover: "#2c223a".to_string(),
            bg_selected: "#361e4f".to_string(),
            border_subtle: "#302442".to_string(),
            green_badge: "#30d158".to_string(),
            text_primary: "#f7f2fa".to_string(),
            text_secondary: "#a695b5".to_string(),
            text_muted: "#6d5f7c".to_string(),
            is_custom: false,
        },
        ThemeDefinition {
            id: "amber".to_string(),
            name: "Amber (Gold)".to_string(),
            accent: "#ff9f0a".to_string(),
            accent_hover: "#ffb340".to_string(),
            accent_soft: "#ff9f0a26".to_string(),
            bg_canvas: "#14110b".to_string(),
            bg_sidebar: "#1a160e".to_string(),
            bg_elevated: "#241e14".to_string(),
            bg_input: "#201b12".to_string(),
            bg_transport: "#1b1710".to_string(),
            bg_row_alt: "#16130d".to_string(),
            bg_hover: "#2d2619".to_string(),
            bg_selected: "#3b2e15".to_string(),
            border_subtle: "#352b1b".to_string(),
            green_badge: "#30d158".to_string(),
            text_primary: "#faf6f0".to_string(),
            text_secondary: "#b0a18c".to_string(),
            text_muted: "#756956".to_string(),
            is_custom: false,
        },
        ThemeDefinition {
            id: "light".to_string(),
            name: "Apple Light".to_string(),
            accent: "#fa2d48".to_string(),
            accent_hover: "#e0223c".to_string(),
            accent_soft: "#fa2d4826".to_string(),
            bg_canvas: "#f5f5f7".to_string(),
            bg_sidebar: "#e8e8ed".to_string(),
            bg_elevated: "#ffffff".to_string(),
            bg_input: "#efeff4".to_string(),
            bg_transport: "#ececf1".to_string(),
            bg_row_alt: "#ebebf0".to_string(),
            bg_hover: "#dfdfeb".to_string(),
            bg_selected: "#fed7de".to_string(),
            border_subtle: "#d6d6de".to_string(),
            green_badge: "#30d158".to_string(),
            text_primary: "#1d1d1f".to_string(),
            text_secondary: "#6e6e73".to_string(),
            text_muted: "#86868b".to_string(),
            is_custom: false,
        },
        ThemeDefinition {
            id: "nord".to_string(),
            name: "Nordic Frost".to_string(),
            accent: "#88c0d0".to_string(),
            accent_hover: "#81a1c1".to_string(),
            accent_soft: "#88c0d026".to_string(),
            bg_canvas: "#242933".to_string(),
            bg_sidebar: "#2e3440".to_string(),
            bg_elevated: "#3b4252".to_string(),
            bg_input: "#353c4a".to_string(),
            bg_transport: "#2e3440".to_string(),
            bg_row_alt: "#272c38".to_string(),
            bg_hover: "#434c5e".to_string(),
            bg_selected: "#3b4b5e".to_string(),
            border_subtle: "#4c566a".to_string(),
            green_badge: "#a3be8c".to_string(),
            text_primary: "#eceff4".to_string(),
            text_secondary: "#d8dee9".to_string(),
            text_muted: "#9aa5b8".to_string(),
            is_custom: false,
        },
    ]
}

pub fn custom_to_definition(c: &CustomTheme) -> ThemeDefinition {
    let clean_accent = c.accent.trim().trim_start_matches('#');
    let accent_soft = format!("#{}26", clean_accent);

    ThemeDefinition {
        id: c.id.clone(),
        name: c.name.clone(),
        accent: c.accent.clone(),
        accent_hover: c.accent.clone(),
        accent_soft,
        bg_canvas: c.bg_canvas.clone(),
        bg_sidebar: c.bg_sidebar.clone(),
        bg_elevated: c.bg_elevated.clone(),
        bg_input: c.bg_sidebar.clone(),
        bg_transport: c.bg_sidebar.clone(),
        bg_row_alt: c.bg_canvas.clone(),
        bg_hover: if c.is_light { "#e4e4ea".to_string() } else { "#252530".to_string() },
        bg_selected: format!("#{}2a", clean_accent),
        border_subtle: if c.is_light { "#d6d6de".to_string() } else { "#2c2c36".to_string() },
        green_badge: "#30d158".to_string(),
        text_primary: c.text_primary.clone(),
        text_secondary: c.text_secondary.clone(),
        text_muted: if c.is_light { "#86868b".to_string() } else { "#6a6a75".to_string() },
        is_custom: true,
    }
}

pub fn get_theme(id: &str, custom_themes: &[CustomTheme]) -> ThemeDefinition {
    for c in custom_themes {
        if c.id == id {
            return custom_to_definition(c);
        }
    }
    for b in get_bundled_themes() {
        if b.id == id {
            return b;
        }
    }
    get_bundled_themes().into_iter().next().unwrap()
}

pub fn get_all_theme_items(custom_themes: &[CustomTheme]) -> Vec<ThemeItem> {
    let mut items = Vec::new();
    for b in get_bundled_themes() {
        items.push(ThemeItem {
            id: SharedString::from(b.id),
            name: SharedString::from(b.name),
            accent: SharedString::from(b.accent),
            bg_canvas: SharedString::from(b.bg_canvas),
            is_custom: false,
        });
    }
    for c in custom_themes {
        items.push(ThemeItem {
            id: SharedString::from(c.id.clone()),
            name: SharedString::from(c.name.clone()),
            accent: SharedString::from(c.accent.clone()),
            bg_canvas: SharedString::from(c.bg_canvas.clone()),
            is_custom: true,
        });
    }
    items
}

pub fn apply_theme(ui: &AppWindow, theme_def: &ThemeDefinition) {
    let theme = ui.global::<Theme>();
    theme.set_bg_canvas(parse_hex_color(&theme_def.bg_canvas));
    theme.set_bg_sidebar(parse_hex_color(&theme_def.bg_sidebar));
    theme.set_bg_elevated(parse_hex_color(&theme_def.bg_elevated));
    theme.set_bg_input(parse_hex_color(&theme_def.bg_input));
    theme.set_bg_transport(parse_hex_color(&theme_def.bg_transport));
    theme.set_bg_row_alt(parse_hex_color(&theme_def.bg_row_alt));
    theme.set_bg_hover(parse_hex_color(&theme_def.bg_hover));
    theme.set_bg_selected(parse_hex_color(&theme_def.bg_selected));
    theme.set_border_subtle(parse_hex_color(&theme_def.border_subtle));
    theme.set_accent(parse_hex_color(&theme_def.accent));
    theme.set_accent_hover(parse_hex_color(&theme_def.accent_hover));
    theme.set_accent_soft(parse_hex_color(&theme_def.accent_soft));
    theme.set_green_badge(parse_hex_color(&theme_def.green_badge));
    theme.set_text_primary(parse_hex_color(&theme_def.text_primary));
    theme.set_text_secondary(parse_hex_color(&theme_def.text_secondary));
    theme.set_text_muted(parse_hex_color(&theme_def.text_muted));

    ui.set_active_theme_id(SharedString::from(theme_def.id.clone()));
}
