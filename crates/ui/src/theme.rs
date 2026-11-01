//! Central theme data. All widgets take their colors from [`ThemePalette`].

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePalette {
    pub dark: bool,
    pub window_bg: Color32,
    pub panel_bg: Color32,
    pub toolbar_bg: Color32,
    pub toolbar_border: Color32,
    pub table_bg: Color32,
    pub table_alt_bg: Color32,
    pub header_bg: Color32,
    pub header_bg_bottom: Color32,
    pub header_border: Color32,
    pub hover_bg: Color32,
    pub selected_bg: Color32,
    pub selected_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub border: Color32,
    pub input_bg: Color32,
    pub input_border: Color32,
    pub accent: Color32,
    pub accent_top: Color32,
    pub accent_bottom: Color32,
    pub accent_border: Color32,
    pub accent_text: Color32,
    pub stop_top: Color32,
    pub stop_bottom: Color32,
    pub stop_border: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub tab_active_bg: Color32,
    pub tab_inactive_bg: Color32,
    pub icon_fg: Color32,
    pub icon_screen: Color32,
    pub icon_screen_dim: Color32,
    pub link: Color32,
    pub progress: Color32,
}

impl ThemePalette {
    /// Classic Windows look of the reference screenshot.
    pub const LIGHT: ThemePalette = ThemePalette {
        dark: false,
        window_bg: Color32::from_rgb(0xF0, 0xF0, 0xF0),
        panel_bg: Color32::from_rgb(0xF0, 0xF0, 0xF0),
        toolbar_bg: Color32::from_rgb(0xF5, 0xF6, 0xF7),
        toolbar_border: Color32::from_rgb(0xDA, 0xDB, 0xDC),
        table_bg: Color32::WHITE,
        table_alt_bg: Color32::from_rgb(0xF5, 0xF7, 0xFA),
        header_bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        header_bg_bottom: Color32::from_rgb(0xF2, 0xF2, 0xF2),
        header_border: Color32::from_rgb(0xE5, 0xE5, 0xE5),
        hover_bg: Color32::from_rgb(0xE5, 0xF3, 0xFF),
        selected_bg: Color32::from_rgb(0xCC, 0xE8, 0xFF),
        selected_border: Color32::from_rgb(0x99, 0xD1, 0xFF),
        text_primary: Color32::from_rgb(0x00, 0x00, 0x00),
        text_secondary: Color32::from_rgb(0x6D, 0x6D, 0x6D),
        text_disabled: Color32::from_rgb(0xA0, 0xA0, 0xA0),
        border: Color32::from_rgb(0xAB, 0xAD, 0xB3),
        input_bg: Color32::WHITE,
        input_border: Color32::from_rgb(0xAB, 0xAD, 0xB3),
        accent: Color32::from_rgb(0x4C, 0xA8, 0x36),
        accent_top: Color32::from_rgb(0x6C, 0xC2, 0x4A),
        accent_bottom: Color32::from_rgb(0x46, 0x9B, 0x2C),
        accent_border: Color32::from_rgb(0x3A, 0x86, 0x24),
        accent_text: Color32::WHITE,
        stop_top: Color32::from_rgb(0xE4, 0x6A, 0x5A),
        stop_bottom: Color32::from_rgb(0xC2, 0x43, 0x34),
        stop_border: Color32::from_rgb(0xA8, 0x36, 0x29),
        error: Color32::from_rgb(0xC4, 0x2B, 0x1C),
        warning: Color32::from_rgb(0xC7, 0x7C, 0x02),
        tab_active_bg: Color32::WHITE,
        tab_inactive_bg: Color32::from_rgb(0xE6, 0xE6, 0xE6),
        icon_fg: Color32::from_rgb(0x4A, 0x4F, 0x55),
        icon_screen: Color32::from_rgb(0x2F, 0x8F, 0xE0),
        icon_screen_dim: Color32::from_rgb(0xB8, 0xBC, 0xC2),
        link: Color32::from_rgb(0x00, 0x66, 0xCC),
        progress: Color32::from_rgb(0x06, 0xB0, 0x25),
    };

    /// Modernised dark variant with the same information architecture.
    pub const DARK: ThemePalette = ThemePalette {
        dark: true,
        window_bg: Color32::from_rgb(0x1B, 0x1C, 0x1F),
        panel_bg: Color32::from_rgb(0x24, 0x26, 0x2A),
        toolbar_bg: Color32::from_rgb(0x2A, 0x2C, 0x31),
        toolbar_border: Color32::from_rgb(0x3A, 0x3D, 0x43),
        table_bg: Color32::from_rgb(0x1F, 0x20, 0x24),
        table_alt_bg: Color32::from_rgb(0x24, 0x26, 0x2A),
        header_bg: Color32::from_rgb(0x33, 0x35, 0x3B),
        header_bg_bottom: Color32::from_rgb(0x2C, 0x2E, 0x33),
        header_border: Color32::from_rgb(0x40, 0x43, 0x4A),
        hover_bg: Color32::from_rgb(0x2C, 0x36, 0x42),
        selected_bg: Color32::from_rgb(0x26, 0x4F, 0x78),
        selected_border: Color32::from_rgb(0x3A, 0x72, 0xA8),
        text_primary: Color32::from_rgb(0xE8, 0xE9, 0xEB),
        text_secondary: Color32::from_rgb(0x9A, 0x9F, 0xA6),
        text_disabled: Color32::from_rgb(0x62, 0x66, 0x6D),
        border: Color32::from_rgb(0x45, 0x48, 0x4F),
        input_bg: Color32::from_rgb(0x18, 0x19, 0x1C),
        input_border: Color32::from_rgb(0x4A, 0x4E, 0x55),
        accent: Color32::from_rgb(0x4C, 0xAF, 0x50),
        accent_top: Color32::from_rgb(0x5A, 0xB5, 0x4A),
        accent_bottom: Color32::from_rgb(0x3B, 0x8C, 0x31),
        accent_border: Color32::from_rgb(0x2F, 0x73, 0x27),
        accent_text: Color32::WHITE,
        stop_top: Color32::from_rgb(0xC0, 0x5B, 0x50),
        stop_bottom: Color32::from_rgb(0x9C, 0x40, 0x37),
        stop_border: Color32::from_rgb(0x80, 0x33, 0x2C),
        error: Color32::from_rgb(0xD0, 0x6A, 0x63),
        warning: Color32::from_rgb(0xE0, 0xA8, 0x45),
        tab_active_bg: Color32::from_rgb(0x1F, 0x20, 0x24),
        tab_inactive_bg: Color32::from_rgb(0x2E, 0x30, 0x35),
        icon_fg: Color32::from_rgb(0xB4, 0xB8, 0xBE),
        icon_screen: Color32::from_rgb(0x4C, 0x9E, 0xEB),
        icon_screen_dim: Color32::from_rgb(0x55, 0x59, 0x60),
        link: Color32::from_rgb(0x6C, 0xB4, 0xFF),
        progress: Color32::from_rgb(0x4C, 0xAF, 0x50),
    };

    pub fn get(dark: bool) -> ThemePalette {
        if dark {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

/// Base font size of the reference UI (Segoe UI 9pt = 12 px).
pub const BASE_FONT: f32 = 12.0;

/// Left/right inset of every window section (menu, toolbar, inputs, table, status bar).
pub const SIDE_MARGIN: f32 = 10.0;

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Apply palette and spacing to egui's style.
pub fn apply_theme(ctx: &egui::Context, p: &ThemePalette) {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.panel_bg;
    v.window_fill = p.panel_bg;
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.window_corner_radius = CornerRadius::same(3);
    v.menu_corner_radius = CornerRadius::same(2);
    v.extreme_bg_color = p.input_bg;
    v.faint_bg_color = p.table_alt_bg;
    v.code_bg_color = p.table_alt_bg;
    v.override_text_color = Some(p.text_primary);
    v.hyperlink_color = p.link;
    v.error_fg_color = p.error;
    v.warn_fg_color = p.warning;
    v.selection.bg_fill = p.selected_bg;
    v.selection.stroke = Stroke::new(1.0_f32, p.text_primary);
    v.window_shadow = egui::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(if p.dark { 120 } else { 40 }) };
    v.popup_shadow = egui::Shadow { offset: [1, 2], blur: 6, spread: 0, color: Color32::from_black_alpha(if p.dark { 120 } else { 50 }) };
    let r = CornerRadius::same(2);
    let btn_bg = if p.dark { Color32::from_rgb(0x33, 0x36, 0x3C) } else { Color32::from_rgb(0xE1, 0xE1, 0xE1) };
    let btn_hover = if p.dark { Color32::from_rgb(0x3D, 0x41, 0x48) } else { Color32::from_rgb(0xE5, 0xF1, 0xFB) };
    let btn_active = if p.dark { Color32::from_rgb(0x46, 0x4B, 0x53) } else { Color32::from_rgb(0xCC, 0xE4, 0xF7) };
    v.widgets.noninteractive.bg_fill = p.panel_bg;
    v.widgets.noninteractive.weak_bg_fill = p.panel_bg;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = btn_bg;
    v.widgets.inactive.weak_bg_fill = btn_bg;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.input_border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = btn_hover;
    v.widgets.hovered.weak_bg_fill = btn_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, if p.dark { p.selected_border } else { Color32::from_rgb(0x00, 0x78, 0xD7) });
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = btn_active;
    v.widgets.active.weak_bg_fill = btn_active;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, if p.dark { p.selected_border } else { Color32::from_rgb(0x00, 0x54, 0x99) });
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open = v.widgets.active;
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size = egui::vec2(24.0, 20.0);
        s.spacing.menu_margin = egui::Margin::symmetric(2, 3);
        s.spacing.window_margin = egui::Margin::same(10);
        s.spacing.combo_height = 300.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        s.text_styles.insert(TextStyle::Body, font(BASE_FONT));
        s.text_styles.insert(TextStyle::Button, font(BASE_FONT));
        s.text_styles.insert(TextStyle::Small, font(10.5));
        s.text_styles.insert(TextStyle::Heading, font(15.0));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace));
        s.interaction.tooltip_delay = 0.4;
    });
}

/// Use a system UI font close to the original (Segoe UI / Tahoma) when
/// available; egui's bundled fonts remain as fallback for symbols.
pub fn install_fonts(ctx: &egui::Context) {
    let candidates: &[(&str, u32)] = &[
        ("C:\\Windows\\Fonts\\segoeui.ttf", 0),
        ("C:\\Windows\\Fonts\\tahoma.ttf", 0),
        ("/System/Library/Fonts/Supplemental/Tahoma.ttf", 0),
        ("/System/Library/Fonts/Supplemental/Arial.ttf", 0),
        ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 0),
        ("/usr/share/fonts/noto/NotoSans-Regular.ttf", 0),
        ("/usr/share/fonts/google-noto/NotoSans-Regular.ttf", 0),
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
        ("/usr/share/fonts/TTF/DejaVuSans.ttf", 0),
        ("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf", 0),
        ("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf", 0),
    ];
    let mut fonts = FontDefinitions::default();
    for (path, index) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut data = FontData::from_owned(bytes);
            data.index = *index;
            fonts.font_data.insert("system-ui".into(), Arc::new(data));
            if let Some(f) = fonts.families.get_mut(&FontFamily::Proportional) {
                f.insert(0, "system-ui".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
}
