//! Colour themes for a run's IDE. Every colour on screen comes from one [`Palette`], built from
//! a theme in `data/themes.ron`, so switching theme recolours the whole IDE at once.

use ninsei_core::data::{ThemeDef, rgb};

use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Shadow, Stroke, TextStyle, Theme, Visuals,
};

/// One theme, six intensities, from unlit glass to full glow.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Deepest background: text fields, the network map.
    pub bg: Color32,
    /// Panel background.
    pub panel: Color32,
    /// Barely lit: borders, empty bar cells, selection.
    pub faint: Color32,
    /// Secondary text, cleared nodes.
    pub dim: Color32,
    /// Normal text.
    pub fg: Color32,
    /// Highlights, damage, loot.
    pub bright: Color32,
}

impl Palette {
    /// The palette for a theme from the data file.
    pub fn from_def(t: &ThemeDef) -> Self {
        let c = |h: &str| rgb(h).map_or(Color32::WHITE, |[r, g, b]| Color32::from_rgb(r, g, b));
        Self {
            bg: c(&t.colors.bg),
            panel: c(&t.colors.panel),
            faint: c(&t.colors.faint),
            dim: c(&t.colors.dim),
            fg: c(&t.colors.fg),
            bright: c(&t.colors.bright),
        }
    }
}

pub const FONT_BODY: f32 = 14.0;
pub const FONT_SMALL: f32 = 11.0;
pub const FONT_HEADING: f32 = 18.0;

/// Applies the palette and an all-monospace type scale to the whole app.
pub fn apply(ctx: &Context, p: &Palette) {
    let mut v = Visuals::dark();
    let zero = CornerRadius::ZERO;
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.bg;
    v.text_edit_bg_color = Some(p.bg);
    v.window_stroke = Stroke::new(1.0, p.dim);
    v.window_corner_radius = zero;
    v.menu_corner_radius = zero;
    v.window_shadow = Shadow::NONE;
    v.popup_shadow = Shadow::NONE;
    v.selection.bg_fill = p.faint;
    v.selection.stroke = Stroke::new(1.0, p.bright);
    v.hyperlink_color = p.bright;
    v.warn_fg_color = p.bright;
    v.error_fg_color = p.bright;
    v.text_cursor.stroke = Stroke::new(2.0, p.fg);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.faint);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.fg);
    w.inactive.bg_fill = p.bg;
    w.inactive.weak_bg_fill = p.bg;
    w.inactive.bg_stroke = Stroke::new(1.0, p.dim);
    w.inactive.fg_stroke = Stroke::new(1.0, p.fg);
    w.hovered.bg_fill = p.faint;
    w.hovered.weak_bg_fill = p.faint;
    w.hovered.bg_stroke = Stroke::new(1.0, p.bright);
    w.hovered.fg_stroke = Stroke::new(1.0, p.bright);
    // Pressed widgets go inverse video, like a selected terminal cell.
    w.active.bg_fill = p.fg;
    w.active.weak_bg_fill = p.fg;
    w.active.bg_stroke = Stroke::new(1.0, p.bright);
    w.active.fg_stroke = Stroke::new(1.0, p.bg);
    w.open.bg_fill = p.faint;
    w.open.weak_bg_fill = p.faint;
    w.open.bg_stroke = Stroke::new(1.0, p.dim);
    w.open.fg_stroke = Stroke::new(1.0, p.bright);
    for wv in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        wv.corner_radius = zero;
        wv.expansion = 0.0;
    }

    ctx.set_theme(Theme::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        style.visuals = v;
        let mono = |size| FontId::new(size, FontFamily::Monospace);
        style.text_styles = [
            (TextStyle::Small, mono(FONT_SMALL)),
            (TextStyle::Body, mono(FONT_BODY)),
            (TextStyle::Monospace, mono(FONT_BODY)),
            (TextStyle::Button, mono(FONT_BODY)),
            (TextStyle::Heading, mono(FONT_HEADING)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(6.0, 1.0);
    });
}

/// egui_dock styling to match: flat tabs, the active one inverse video.
pub fn dock_style(style: &egui::Style, p: &Palette) -> egui_dock::Style {
    let mut s = egui_dock::Style::from_egui(style);
    let zero = CornerRadius::ZERO;
    s.main_surface_border_stroke = Stroke::new(1.0, p.faint);
    s.main_surface_border_rounding = zero;
    s.separator.width = 1.0;
    s.separator.color_idle = p.faint;
    s.separator.color_hovered = p.dim;
    s.separator.color_dragged = p.fg;
    s.tab_bar.bg_fill = p.bg;
    s.tab_bar.hline_color = p.faint;
    s.tab_bar.corner_radius = zero;
    s.tab_bar.height = 22.0;
    s.tab.tab_body.bg_fill = p.panel;
    s.tab.tab_body.stroke = Stroke::new(1.0, p.faint);
    s.tab.tab_body.corner_radius = zero;
    s.tab.hline_below_active_tab_name = false;
    for (t, bg, text) in [
        (&mut s.tab.active, p.faint, p.bright),
        (&mut s.tab.focused, p.fg, p.bg),
        (&mut s.tab.active_with_kb_focus, p.faint, p.bright),
        (&mut s.tab.focused_with_kb_focus, p.fg, p.bg),
        (&mut s.tab.inactive, p.bg, p.dim),
        (&mut s.tab.inactive_with_kb_focus, p.bg, p.dim),
        (&mut s.tab.hovered, p.faint, p.fg),
    ] {
        t.bg_fill = bg;
        t.text_color = text;
        t.outline_color = p.faint;
        t.corner_radius = zero;
    }
    s.overlay.selection_color = p.fg.gamma_multiply(0.25);
    s.overlay.button_color = p.dim;
    s.overlay.button_border_stroke = Stroke::new(1.0, p.fg);
    s
}
