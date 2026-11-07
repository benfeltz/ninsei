//! The sound pop-out on the run screen: a level and the sound switch, opened from the chrome's
//! own idiom (btop's bar, the suite's menu, Liza's Control Panel) and dressed by the chrome
//! (`ChromeView::sound`). The level and the switch are the save's (`Save::volume`, and
//! `Save::sound`, the System app's switch too); the pop-out only remembers that it's open and
//! what was set when it opened, so a Cancel can put it back. It pushes requests and never
//! writes the save while drawing, and while it's open it owns the keyboard.

use egui::{Context, Key};
use ninsei_core::hub::{Save, notch_volume, volume_notch};

use crate::panes::Request;

/// What the controls show: sound on or off, and the level in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub on: bool,
    pub volume: u32,
}

impl Level {
    pub fn of(save: &Save) -> Level {
        Level {
            on: save.sound,
            volume: u32::from(save.volume.min(100)),
        }
    }

    /// The level as a notch of a control with `steps` notches above silent.
    pub fn notch(&self, steps: u32) -> u32 {
        volume_notch(self.volume, steps)
    }

    /// The level `by` notches up (or down) a control with `steps` notches.
    pub fn nudged(self, by: i32, steps: u32) -> Level {
        let n = (self.notch(steps) as i32 + by).clamp(0, steps as i32) as u32;
        Level {
            volume: notch_volume(n, steps),
            ..self
        }
    }

    /// The requests that take `self` to `to`.
    pub fn to(self, to: Level) -> Vec<Request> {
        let mut out = Vec::new();
        if to.volume != self.volume {
            out.push(Request::Volume(to.volume));
        }
        if to.on != self.on {
            out.push(Request::SoundOn(to.on));
        }
        out
    }
}

/// Whether the pop-out is open, and what was set when it opened.
#[derive(Default, Debug)]
pub struct Popup {
    was: Option<Level>,
}

impl Popup {
    pub fn is_open(&self) -> bool {
        self.was.is_some()
    }

    pub fn open(&mut self, now: Level) {
        self.was.get_or_insert(now);
    }

    /// Opens it, or closes it if it's open (a bar's button).
    pub fn toggle(&mut self, now: Level) {
        if self.is_open() {
            self.close();
        } else {
            self.open(now);
        }
    }

    /// Closes it, keeping what's set.
    pub fn close(&mut self) {
        self.was = None;
    }

    /// Closes it and puts back what was set when it opened.
    pub fn cancel(&mut self, now: Level) -> Vec<Request> {
        self.was.take().map_or(Vec::new(), |was| now.to(was))
    }
}

/// The pop-out's keys this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    /// Notches up (+) or down (-).
    pub nudge: i32,
    /// Flip the sound switch.
    pub flip: bool,
    /// Enter: done.
    pub ok: bool,
    /// Esc: closes (a dialog with Cancel puts things back).
    pub esc: bool,
}

/// Reads the pop-out's keys, and takes the keyboard from anything that types: while it's open,
/// nothing else hears them. `flip` is the key that flips the sound switch.
pub fn keys(ctx: &Context, flip: Key) -> Keys {
    ctx.memory_mut(|m| m.stop_text_input());
    ctx.input(|i| {
        let up = [Key::ArrowRight, Key::ArrowUp, Key::Plus, Key::Equals];
        let down = [Key::ArrowLeft, Key::ArrowDown, Key::Minus];
        let count = |ks: &[Key]| ks.iter().filter(|k| i.key_pressed(**k)).count() as i32;
        Keys {
            nudge: count(&up) - count(&down),
            flip: i.key_pressed(flip),
            ok: i.key_pressed(Key::Enter),
            esc: i.key_pressed(Key::Escape),
        }
    })
}

/// The classic chrome's pop-out: a small window with a slider, the sound switch (the
/// System app's: it mutes everything) and close.
pub fn classic(ctx: &Context, cx: &mut crate::chrome::FrameCx) -> Vec<Request> {
    let (p, now) = (cx.pal, cx.level);
    let (mut to, k) = start(ctx, now, Key::M, CLASSIC_NOTCHES);
    let mut close = k.esc || k.ok;
    egui::Area::new(egui::Id::new("classic-volume"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(p.panel)
                .stroke(egui::Stroke::new(1.0, p.dim))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("volume").color(p.bright));
                    let mut v = to.volume;
                    let step = f64::from(100 / CLASSIC_NOTCHES);
                    if ui
                        .add(egui::Slider::new(&mut v, 0..=100).step_by(step).suffix("%"))
                        .changed()
                    {
                        to.volume = v;
                    }
                    let mut on = to.on;
                    if ui.checkbox(&mut on, "sound").changed() {
                        to.on = on;
                    }
                    if ui.button("close").clicked() {
                        close = true;
                    }
                });
        });
    if close {
        cx.popup.close();
    }
    now.to(to)
}

/// The classic slider's notches: 5% apiece.
const CLASSIC_NOTCHES: u32 = 20;

/// The start of every pop-out's frame: its keys, and the level they move to (`by` notches of
/// `notches`, the switch flipped by `flip`).
pub fn start(ctx: &Context, now: Level, flip: Key, notches: u32) -> (Level, Keys) {
    let k = keys(ctx, flip);
    let mut to = now.nudged(k.nudge, notches);
    if k.flip {
        to.on = !to.on;
    }
    (to, k)
}

/// Names a control for a screen reader (and the tests): what it says it is.
pub fn named(r: egui::Response, label: &str) -> egui::Response {
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    r
}

/// A control over drawn cells or pixels: `rect` takes `sense`, named `label`.
pub fn hit(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    salt: &str,
    label: &str,
    sense: egui::Sense,
) -> egui::Response {
    let r = ui
        .interact(rect, egui::Id::new((salt, label)), sense)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    named(r, label)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// What the app does with the pop-out's requests: the level they leave.
    pub fn apply(mut l: Level, reqs: &[Request]) -> Level {
        for r in reqs {
            match r {
                Request::Volume(v) => l.volume = *v,
                Request::SoundOn(on) => l.on = *on,
                _ => {}
            }
        }
        l
    }

    #[test]
    fn it_reads_the_save_and_steps_by_notches() {
        let s = Save {
            volume: 70,
            sound: false,
            ..Save::default()
        };
        let l = Level::of(&s);
        assert_eq!(
            l,
            Level {
                on: false,
                volume: 70
            }
        );
        assert_eq!(l.notch(10), 7);
        assert_eq!(l.nudged(1, 10).volume, 80);
        assert_eq!(l.nudged(-9, 10).volume, 0);
        assert_eq!(l.nudged(9, 7).volume, 100);
        assert!(!l.nudged(1, 10).on, "a nudge leaves the switch alone");
    }

    #[test]
    fn cancel_puts_back_what_was_there() {
        let mut p = Popup::default();
        let was = Level {
            on: true,
            volume: 60,
        };
        assert!(p.cancel(was).is_empty(), "nothing to undo when closed");
        p.open(was);
        p.open(Level {
            on: false,
            volume: 0,
        });
        assert!(p.is_open());
        let now = Level {
            on: false,
            volume: 20,
        };
        let back = p.cancel(now);
        assert!(!p.is_open());
        assert_eq!(apply(now, &back), was);
        // Close keeps what's set.
        p.toggle(was);
        p.toggle(was);
        assert!(!p.is_open());
    }

    use crate::chrome::{self, ChromeView, FrameCx};
    use crate::uitest::{Harness, click, key, text};
    use egui::{Event, Rect};
    use ninsei_core::hardware::Chrome;
    use ninsei_core::{Data, Game};

    /// A run screen in miniature: a command line that wants the keys, the chrome's status
    /// extras, and the pop-out on top while it's open. What it asked for comes back, and is
    /// applied to `level` as the app would.
    pub struct Screen {
        pub h: Harness,
        pub game: Game,
        pub pal: crate::theme::Palette,
        pub suite: crate::suite::State,
        pub popup: Popup,
        pub level: Level,
        pub typed: String,
        /// Focus the command line on the next frame, as the run screen does on arrival.
        focus: bool,
    }

    impl Screen {
        pub fn new(on: bool, volume: u32) -> Screen {
            let d = Data::builtin().unwrap();
            let pal = crate::theme::Palette::from_def(&d.themes[0]);
            Screen {
                h: Harness::new(1000.0, 700.0),
                game: Game::new(d, 3),
                pal,
                suite: crate::suite::State::default(),
                popup: Popup::default(),
                level: Level { on, volume },
                typed: String::new(),
                focus: true,
            }
        }

        pub fn frame(&mut self, c: Chrome, events: Vec<Event>) -> Vec<Request> {
            let view: &dyn ChromeView = chrome::view(c);
            let mut out = Vec::new();
            let (game, pal, level) = (&self.game, self.pal, self.level);
            let (suite, popup, typed) = (&mut self.suite, &mut self.popup, &mut self.typed);
            let focus = std::mem::take(&mut self.focus);
            self.h.frame(events, |ui| {
                let line = egui::Id::new("command-line");
                if focus {
                    ui.memory_mut(|m| m.request_focus(line));
                }
                ui.add(egui::TextEdit::singleline(typed).id(line));
                let mut cx = FrameCx {
                    game,
                    pal,
                    suite: &mut *suite,
                    keys_free: !popup.is_open(),
                    level,
                    popup: &mut *popup,
                };
                ui.horizontal(|ui| out.extend(view.status_extra(ui, &mut cx)));
                if cx.popup.is_open() {
                    out.extend(view.sound(ui.ctx(), &mut cx));
                }
            });
            self.level = apply(self.level, &out);
            out
        }

        /// Clicks the widget that says `what`.
        pub fn press(&mut self, c: Chrome, what: &str) -> Vec<Request> {
            let at = self
                .h
                .find(what)
                .unwrap_or_else(|| panic!("no {what:?} in {:?}", self.h.texts()))
                .center();
            self.frame(c, click(at))
        }

        pub fn screen(&self) -> Rect {
            Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))
        }
    }

    /// The sound requests, as words.
    pub fn said(reqs: &[Request]) -> Vec<String> {
        reqs.iter()
            .filter_map(|r| match r {
                Request::Volume(v) => Some(format!("volume {v}")),
                Request::SoundOn(on) => Some(format!("sound {}", if *on { "on" } else { "off" })),
                _ => None,
            })
            .collect()
    }

    fn keys_of(k: egui::Key, t: &str) -> Vec<Event> {
        let mut ev = text(t);
        ev.extend(key(k));
        ev
    }

    #[test]
    fn btop_s_speaker_opens_a_box_of_cells_that_steps_mutes_and_closes() {
        let c = Chrome::Btop;
        let mut s = Screen::new(true, 70);
        s.frame(c, Vec::new());
        assert!(s.h.find("🔊 70%").is_some(), "{:?}", s.h.texts());
        assert!(
            said(&s.press(c, "🔊 70%")).is_empty(),
            "opening asks for nothing"
        );
        assert!(s.popup.is_open());
        s.frame(c, Vec::new());
        // The box sits on screen, every control inside it.
        assert!(
            s.h.outside(s.screen()).is_empty(),
            "{:?}",
            s.h.outside(s.screen())
        );
        let meter = s.h.find("5%").unwrap();
        assert!(s.h.find("mute").is_some() && s.h.find("+").is_some());
        // It's laid out in whole cells: each meter cell is one cell, the meter starting two
        // cells in from the box, which starts on the speaker's left edge.
        let speaker = s.h.find("🔊 70%").unwrap();
        let cell0 = crate::cells::Cells::cell_size(&s.h.ctx);
        assert!((meter.left() - (speaker.left().round() + 2.0 * cell0.x)).abs() < 0.01);
        let cell = crate::cells::Cells::cell_size(&s.h.ctx);
        assert!((meter.width() - cell.x).abs() < 0.01, "{meter:?} {cell:?}");
        assert!((meter.height() - cell.y).abs() < 0.01, "{meter:?} {cell:?}");
        // Keys: → and + step 5%, ← and - back, a meter cell sets it, m mutes.
        assert_eq!(said(&s.frame(c, key(egui::Key::ArrowRight))), ["volume 75"]);
        assert_eq!(
            said(&s.frame(c, keys_of(egui::Key::Minus, "-"))),
            ["volume 70"]
        );
        assert_eq!(said(&s.press(c, "40%")), ["volume 40"]);
        assert_eq!(said(&s.press(c, "+")), ["volume 45"]);
        // `+` stays put whatever the number: at 5%, 10% and 100%.
        let plus = |s: &Screen| s.h.find("+").unwrap();
        let at = plus(&s);
        for v in [5, 10, 100] {
            s.level.volume = v;
            s.frame(c, Vec::new());
            assert_eq!(plus(&s), at, "at {v}%");
        }
        s.level.volume = 45;
        assert_eq!(said(&s.frame(c, keys_of(egui::Key::M, "m"))), ["sound off"]);
        assert_eq!(s.typed, "", "the command line never heard a key");
        s.frame(c, Vec::new());
        assert!(s.h.find("🔇 muted").is_some(), "{:?}", s.h.texts());
        assert_eq!(said(&s.press(c, "mute")), ["sound on"]);
        // Esc closes it and keeps what's set; the speaker toggles it too.
        s.frame(c, key(egui::Key::Escape));
        assert!(!s.popup.is_open());
        assert_eq!(
            s.level,
            Level {
                on: true,
                volume: 45
            }
        );
        s.frame(c, Vec::new());
        assert!(s.h.find("mute").is_none(), "closed");
        s.press(c, "🔉 45%");
        assert!(s.popup.is_open());
        s.frame(c, Vec::new());
        s.press(c, "🔉 45%");
        assert!(!s.popup.is_open());
    }

    /// The pop-out opened as its menu item does.
    fn opened(c: Chrome, on: bool, volume: u32) -> Screen {
        let mut s = Screen::new(on, volume);
        s.popup.open(s.level);
        // Two frames: an anchored window settles where it goes on its second.
        s.frame(c, Vec::new());
        s.frame(c, Vec::new());
        assert!(
            s.h.outside(s.screen()).is_empty(),
            "{:?}",
            s.h.outside(s.screen())
        );
        s
    }

    #[test]
    fn every_chrome_has_a_way_in() {
        for c in [Chrome::Classic, Chrome::Btop, Chrome::Suite, Chrome::Stack] {
            let v = chrome::view(c);
            let in_menus = v
                .menus()
                .iter()
                .any(|(_, parts)| parts.contains(&chrome::MenuPart::Sound));
            assert!(in_menus, "{c:?}");
        }
        assert_eq!(chrome::view(Chrome::Suite).sound_item(), "Sound...");
        assert_eq!(chrome::view(Chrome::Stack).sound_item(), "Control Panel...");
    }

    #[test]
    fn the_suite_s_sound_dialog_moves_mutes_and_cancels() {
        let c = Chrome::Suite;
        let mut s = opened(c, true, 50);
        assert!(s.h.find("[  OK  ]").is_some() && s.h.find("[Cancel]").is_some());
        assert_eq!(said(&s.frame(c, key(egui::Key::ArrowRight))), ["volume 60"]);
        assert_eq!(
            said(&s.frame(c, keys_of(egui::Key::Space, " "))),
            ["sound off"]
        );
        assert_eq!(said(&s.press(c, "30%")), ["volume 30"]);
        assert_eq!(said(&s.press(c, "Mute")), ["sound on"]);
        assert_eq!(said(&s.press(c, "Mute")), ["sound off"]);
        assert_eq!(s.typed, "", "the command line never heard a key");
        // Cancel puts back what was set when it opened.
        assert_eq!(said(&s.press(c, "[Cancel]")), ["volume 50", "sound on"]);
        assert!(!s.popup.is_open());
        assert_eq!(
            s.level,
            Level {
                on: true,
                volume: 50
            }
        );
        // So does Esc; Enter and OK keep it.
        let mut s = opened(c, true, 50);
        s.frame(c, key(egui::Key::ArrowLeft));
        assert_eq!(said(&s.frame(c, key(egui::Key::Escape))), ["volume 50"]);
        let mut s = opened(c, true, 50);
        s.frame(c, key(egui::Key::ArrowLeft));
        assert!(said(&s.frame(c, key(egui::Key::Enter))).is_empty());
        assert!(!s.popup.is_open());
        assert_eq!(s.level.volume, 40);
        let mut s = opened(c, true, 50);
        s.press(c, "[  OK  ]");
        assert!(!s.popup.is_open());
    }

    #[test]
    fn liza_s_control_panel_slides_notch_by_notch_and_closes() {
        let c = Chrome::Stack;
        let mut s = opened(c, true, 100);
        let track = s.h.find("Speaker Volume").expect("the slider");
        // GUI screens sit on whole pixels.
        for what in ["close", "Speaker Volume", "Sound"] {
            let r = s.h.find(what).unwrap();
            assert_eq!(
                (r.min.x.fract(), r.min.y.fract()),
                (0.0, 0.0),
                "{what} {r:?}"
            );
        }
        // A click on the track jumps the knob to the nearest notch: its left end is 0.
        let left = egui::pos2(track.left() + 2.0, track.center().y);
        assert_eq!(said(&s.frame(c, click(left))), ["volume 0"]);
        assert_eq!(said(&s.frame(c, key(egui::Key::ArrowRight))), ["volume 14"]);
        assert_eq!(said(&s.frame(c, key(egui::Key::ArrowRight))), ["volume 29"]);
        assert_eq!(said(&s.press(c, "Sound")), ["sound off"]);
        assert_eq!(
            said(&s.frame(c, keys_of(egui::Key::Space, " "))),
            ["sound on"]
        );
        assert_eq!(s.typed, "");
        // The close box puts it away, keeping what's set; so do Esc and Enter.
        s.press(c, "close");
        assert!(!s.popup.is_open());
        assert_eq!(
            s.level,
            Level {
                on: true,
                volume: 29
            }
        );
        for k in [egui::Key::Escape, egui::Key::Enter] {
            let mut s = opened(c, true, 100);
            s.frame(c, key(k));
            assert!(!s.popup.is_open(), "{k:?}");
        }
    }

    #[test]
    fn the_classic_window_has_a_slider_a_switch_and_close() {
        let c = Chrome::Classic;
        let mut s = opened(c, true, 50);
        assert!(s.h.find("sound").is_some(), "{:?}", s.h.texts());
        assert_eq!(said(&s.frame(c, key(egui::Key::ArrowRight))), ["volume 55"]);
        assert_eq!(said(&s.press(c, "sound")), ["sound off"]);
        s.press(c, "close");
        assert!(!s.popup.is_open());
    }

    #[test]
    fn the_system_app_and_the_pop_out_are_one_switch() {
        let d = Data::builtin().unwrap();
        let mut save = Save::default();
        save.ensure_hardware(&d);
        let mut sys = crate::system::SystemUi::default();
        let mut h = Harness::new(900.0, 700.0);
        // What System's box says, and what clicking it asks for.
        let mut system = |h: &mut Harness, save: &Save, events| {
            let mut out = Vec::new();
            h.frame(events, |ui| {
                crate::system::show(ui, &d, save, &mut sys, &mut out)
            });
            out
        };
        for on in [true, false, true] {
            save.sound = on;
            system(&mut h, &save, Vec::new());
            assert_eq!(h.ticked("Sound"), Some(on), "System");
            let mut run = Screen::new(true, 100);
            run.level = Level::of(&save);
            run.frame(Chrome::Btop, Vec::new());
            let speaker = if on { "🔊 100%" } else { "🔇 muted" };
            assert!(run.h.find(speaker).is_some(), "{:?}", run.h.texts());
        }
        // System's box flips the switch the pop-out shows (the app toggles `Save::sound`)...
        let at = h.find("Sound").unwrap().center();
        let out = system(&mut h, &save, click(at));
        assert!(matches!(out[..], [crate::home::WbOut::Sound]));
        save.sound = !save.sound;
        assert!(!Level::of(&save).on);
        // ...and the pop-out's switch is the one System shows.
        let mut run = Screen::new(true, 100);
        run.level = Level::of(&save);
        run.popup.open(run.level);
        run.frame(Chrome::Btop, Vec::new());
        run.frame(Chrome::Btop, Vec::new());
        let reqs = run.press(Chrome::Btop, "mute");
        assert_eq!(said(&reqs), ["sound on"]);
        save.sound = run.level.on;
        system(&mut h, &save, Vec::new());
        assert_eq!(h.ticked("Sound"), Some(true));
    }
}
