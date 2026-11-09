//! The vis pane: what the speaker is playing, drawn from the samples it hands the run screen
//! (`Speaker::listen`), never from the game. Each chrome dresses it (`ChromeView::vis`); the
//! maths (spectrum, level) is core's `scope`. With nothing playing it lies flat and still.

use egui::{Sense, Stroke, Ui, pos2};
use ninsei_core::scope;

use crate::theme::Palette;

/// A scope's centre line: a line in a colour, or a dotted line (a 1-bit graticule).
#[derive(Clone, Copy, Debug)]
pub enum Centre {
    Line(egui::Color32),
    Dots(egui::Color32),
}

/// An oscilloscope trace of `samples` across the pane, in the text colour over the centre
/// line `centre`.
pub fn trace(ui: &mut Ui, p: &Palette, samples: &[f32], centre: Centre) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    // On whole pixels: one point of the trace per pixel column.
    let rect = egui::Rect::from_min_max(rect.min.round(), rect.max.round());
    let painter = ui.painter_at(rect);
    let mid = rect.center().y.round();
    match centre {
        Centre::Line(c) => {
            painter.line_segment(
                [pos2(rect.left(), mid), pos2(rect.right(), mid)],
                Stroke::new(1.0, c),
            );
        }
        Centre::Dots(c) => {
            let mut x = rect.left();
            while x < rect.right() {
                painter.rect_filled(
                    egui::Rect::from_min_size(pos2(x, mid), egui::vec2(1.0, 1.0)),
                    0.0,
                    c,
                );
                x += 4.0;
            }
        }
    }
    let points = scope::trace(samples, rect.width() as usize);
    if points.iter().all(|v| *v == 0.0) {
        return;
    }
    let half = rect.height() / 2.0 - 2.0;
    let line: Vec<egui::Pos2> = points
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(rect.left() + i as f32, mid - v.clamp(-1.0, 1.0) * half))
        .collect();
    painter.add(egui::Shape::line(line, Stroke::new(1.0, p.fg)));
}

/// Samples the pane is handed each frame: a power of two, for the spectrum.
pub const WINDOW: usize = 2048;
/// How often it redraws while it's on screen.
pub const FRAME_MS: u64 = 33;

/// Keeps the pane moving while it has something to show: asks for the next frame while any of
/// `samples` is sounding, and nothing in silence.
pub fn alive(ui: &Ui, samples: &[f32]) {
    if samples.iter().any(|s| *s != 0.0) {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(FRAME_MS));
    }
}

/// [`alive`] for a face drawn later in the frame: asks `ctx` for the next frame while any of
/// `samples` is sounding.
pub fn alive_ctx(ctx: &egui::Context, samples: &[f32]) {
    if samples.iter().any(|s| *s != 0.0) {
        ctx.request_repaint_after(std::time::Duration::from_millis(FRAME_MS));
    }
}

/// `n` spectrum bars of `samples` as a meter shows them: jumping up at once and falling slowly
/// (`scope::ballistics`), remembered under `id` between frames. While a bar is still falling
/// it asks for the next frame; once everything is still, it stops asking.
pub fn bars(ui: &Ui, id: &str, samples: &[f32], rate: u32, n: usize) -> Vec<f32> {
    let now = scope::spectrum(samples, rate, n);
    let key = egui::Id::new(("vis-bars", id));
    let ctx = ui.ctx();
    let dt = ctx.input(|i| i.stable_dt).min(0.1);
    let was: Vec<f32> = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
    let shown = scope::ballistics(&was, &now, dt);
    if shown != now {
        ctx.request_repaint_after(std::time::Duration::from_millis(FRAME_MS));
    }
    ctx.data_mut(|d| d.insert_temp(key, shown.clone()));
    shown
}

/// How long a numeric readout holds a value: a few readings a second, as a meter's display
/// does, so the digits can be read (and never strobe).
pub const HOLD_SECS: f64 = 0.25;

/// `value` as a readout shows it: held under `id`, taking a new reading only every
/// [`HOLD_SECS`].
pub fn held(ui: &Ui, id: &str, value: f32) -> f32 {
    let key = egui::Id::new(("vis-held", id));
    let ctx = ui.ctx();
    let now = ctx.input(|i| i.time);
    let shown = match ctx.data(|d| d.get_temp::<(f32, f64)>(key)) {
        Some((v, at)) if now - at < HOLD_SECS && now >= at => v,
        _ => {
            ctx.data_mut(|d| d.insert_temp(key, (value, now)));
            value
        }
    };
    if shown != value {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD_SECS));
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_readout_holds_its_number() {
        let mut h = crate::uitest::Harness::new(100.0, 100.0);
        let mut got = Vec::new();
        for v in [0.1, 0.2, 0.3] {
            h.frame(Vec::new(), |ui| got.push(held(ui, "r", v)));
        }
        assert_eq!(got, [0.1, 0.1, 0.1], "frames 0.1 s apart: held");
        h.wait(0.2);
        h.frame(Vec::new(), |ui| got.push(held(ui, "r", 0.4)));
        assert_eq!(got[3], 0.4, "a quarter second on: a new reading");
    }

    #[test]
    fn silence_is_still() {
        let mut h = crate::uitest::Harness::new(400.0, 300.0);
        let tone: Vec<f32> = (0..WINDOW).map(|i| 0.4 * (i as f32 * 0.13).sin()).collect();
        let mut shown = Vec::new();
        h.frame(Vec::new(), |ui| shown = bars(ui, "t", &tone, 48_000, 12));
        assert!(shown.iter().any(|v| *v > 0.5), "{shown:?}");
        // Silence: the bars fall over a second or so, then stop moving.
        for _ in 0..40 {
            h.frame(Vec::new(), |ui| {
                shown = bars(ui, "t", &[0.0; WINDOW], 48_000, 12)
            });
        }
        assert!(shown.iter().all(|v| *v == 0.0), "{shown:?}");
        let mut again = Vec::new();
        h.frame(Vec::new(), |ui| {
            again = bars(ui, "t", &[0.0; WINDOW], 48_000, 12)
        });
        assert_eq!(again, shown, "still");
    }

    /// Draws the vis pane on chrome `c` with `samples` playing, in a pane of 420 × 260.
    fn pane(c: ninsei_core::hardware::Chrome, samples: &[f32]) -> crate::uitest::Harness {
        let d = ninsei_core::Data::builtin().unwrap();
        let g = ninsei_core::Game::new(d.clone(), 3);
        let mut h = crate::uitest::Harness::new(420.0, 260.0);
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::new(10), false, None, Vec::new());
        let mut suite = crate::suite::State::default();
        for _ in 0..2 {
            h.frame(Vec::new(), |ui| {
                let mut panes = crate::panes::Panes {
                    game: &g,
                    pal: crate::chrome::view(c).palette(Palette::from_def(&d.themes[0])),
                    map: ninsei_core::netview::MapView::Iso,
                    deck: String::new(),
                    chrome: c,
                    corrupt: (0.0, 0),
                    term: &mut term,
                    focus_input: &mut focus,
                    bbs_open: false,
                    hex: &mut hex,
                    keep: &mut keep,
                    keep_room: 0,
                    suite: &mut suite,
                    hear: &|| samples.to_vec(),
                    scope_rate: 48_000,
                    out: Vec::new(),
                };
                crate::chrome::view(c).vis(&mut panes, ui);
            });
        }
        h
    }

    #[test]
    fn every_chrome_dresses_it_and_it_fits() {
        use ninsei_core::hardware::Chrome;
        let tone: Vec<f32> = (0..WINDOW).map(|i| 0.3 * (i as f32 * 0.2).sin()).collect();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 260.0));
        for c in [Chrome::Classic, Chrome::Btop, Chrome::Suite, Chrome::Stack] {
            for samples in [&[0.0; WINDOW][..], &tone[..]] {
                let h = pane(c, samples);
                assert!(h.outside(area).is_empty(), "{c:?}: {:?}", h.outside(area));
            }
        }
        // The suite reports the level on its form: dashes in silence, a reading with a tone.
        let quiet = pane(Chrome::Suite, &[0.0; WINDOW]);
        assert!(quiet.find("[   --- dB ]").is_some(), "{:?}", quiet.texts());
        let loud = pane(Chrome::Suite, &tone);
        assert!(loud.find("[ -13.5 dB ]").is_some(), "{:?}", loud.texts());
    }

    /// The run screen's dock with `extra` panes beside the terminal, drawn on chrome `c`, the
    /// vis pane's `hear` counting how often it's asked; `front` picks the tab shown.
    fn docked(front: crate::panes::Tab, samples: &[f32]) -> (usize, bool) {
        use crate::panes::Tab;
        use ninsei_core::hardware::Chrome;
        let d = ninsei_core::Data::builtin().unwrap();
        let g = ninsei_core::Game::new(d.clone(), 3);
        // The two secondary panes as one leaf, as the default layouts put them.
        let mut dock = egui_dock::DockState::new(vec![Tab::Net, Tab::Vis]);
        if let Some(path) = dock.find_tab(&front) {
            let _ = dock.set_active_tab(path);
        }
        let mut h = crate::uitest::Harness::new(1000.0, 700.0);
        let (mut term, mut focus, mut hex, mut keep) =
            (ninsei_term::LineEditor::new(10), false, None, Vec::new());
        let mut suite = crate::suite::State::default();
        let asked = std::cell::Cell::new(0);
        let hear = || {
            asked.set(asked.get() + 1);
            samples.to_vec()
        };
        let mut repaint = false;
        for _ in 0..40 {
            h.frame(Vec::new(), |ui| {
                let mut panes = crate::panes::Panes {
                    game: &g,
                    pal: Palette::from_def(&d.themes[0]),
                    map: ninsei_core::netview::MapView::Iso,
                    deck: String::new(),
                    chrome: Chrome::Btop,
                    corrupt: (0.0, 0),
                    term: &mut term,
                    focus_input: &mut focus,
                    bbs_open: false,
                    hex: &mut hex,
                    keep: &mut keep,
                    keep_room: 0,
                    suite: &mut suite,
                    hear: &hear,
                    scope_rate: 48_000,
                    out: Vec::new(),
                };
                egui_dock::DockArea::new(&mut dock).show_inside(ui, &mut panes);
            });
            repaint = h.ctx.has_requested_repaint();
        }
        (asked.get(), repaint)
    }

    #[test]
    fn a_hidden_vis_pane_costs_nothing_and_silence_asks_for_no_frames() {
        use crate::panes::Tab;
        let tone: Vec<f32> = (0..WINDOW).map(|i| 0.3 * (i as f32 * 0.2).sin()).collect();
        // Behind the net pane: never asked for samples.
        let (asked, _) = docked(Tab::Net, &tone);
        assert_eq!(asked, 0, "a hidden tab takes no samples");
        // In front: asked every frame, and it keeps moving while something plays...
        let (asked, repaint) = docked(Tab::Vis, &tone);
        assert!(asked >= 40, "{asked}");
        assert!(repaint, "playing: it wants the next frame");
        // ...and in silence it settles and asks for nothing.
        let (_, repaint) = docked(Tab::Vis, &[0.0; WINDOW]);
        assert!(!repaint, "silence is still");
    }
}
