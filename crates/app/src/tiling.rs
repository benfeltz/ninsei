//! The Ono-Sendai's desktop: a keyboard-first tiling window manager, after Omarchy and
//! Hyprland. A slim top bar (workspaces, the focused tool, widgets), tools tiled
//! master-and-stack with gaps, and a launcher. A front-end on `home`; the tiling rules
//! (workspaces, focus, layout, search) are `ninsei_core::desktop`.
//!
//! Keys: **Esc** is the leader (it also lets go of the Shell), then a key:
//! `1`-`9` workspace, `Shift`+digit send the tile there, `Space` launcher, `h`/`l` or arrows
//! focus, `Enter` swap with master, `x` close, `?` help. The mouse works too: click a tile to
//! focus it, a workspace to switch. Browser-safe: no Ctrl or Alt shortcuts.

use egui::{
    Align2, Event, FontFamily, FontId, Id, Key, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use ninsei_core::desktop::{self, AddonEffect, Tiling};
use ninsei_core::hardware::Desktop;
use ninsei_core::hub::Tool;

use crate::home::{Cx, DesktopView, Home, WbOut, accent, backdrop, font, hint, ink, muted, shadow};

/// Sizes, in one place.
mod size {
    pub const BAR_H: f32 = 26.0;
    pub const GAP: f32 = 8.0;
    pub const TITLE_H: f32 = 18.0;
    pub const ROUND: f32 = 6.0;
    pub const LAUNCHER: [f32; 2] = [460.0, 320.0];
}

/// What the launcher lists.
#[derive(Clone, Debug)]
enum Entry {
    Tool(Tool, &'static str),
    WarDial,
    Command(&'static str),
}

impl Entry {
    fn label(&self) -> String {
        match self {
            Entry::Tool(_, name) => (*name).to_string(),
            Entry::WarDial => "WarDial".into(),
            Entry::Command(c) => format!("$ {c}"),
        }
    }
}

/// Shell commands the fzf launcher add-on offers.
const COMMANDS: [&str; 10] = [
    "phonebook",
    "ranges",
    "dir",
    "info",
    "monitor",
    "deck",
    "crt",
    "sound",
    "help",
    "clear",
];

/// The launcher's entries: the tools you have, and with fzf the Shell's commands.
fn entries(fuzzy: bool, save: &ninsei_core::hub::Save) -> Vec<Entry> {
    let mut v = vec![
        Entry::Tool(Tool::Terminal, "CarrierTerm"),
        Entry::WarDial,
        Entry::Tool(Tool::Rig, "CPU Shopper"),
        Entry::Tool(Tool::HexEd, "HexEd"),
        Entry::Tool(Tool::Qwk, "QWK Reader"),
        Entry::Tool(Tool::IceDb, "ICE DB"),
        Entry::Tool(Tool::NightComm, "NiGHTCOMM"),
        Entry::Tool(Tool::TigerTeam, "Tiger Team Pro"),
        Entry::Tool(Tool::Shell, "Shell"),
        Entry::Tool(Tool::System, "System"),
    ];
    v.retain(|e| match e {
        Entry::Tool(t, _) => save.opens(*t),
        Entry::WarDial => save.has_tool("wardial") && !save.has_dialer(),
        Entry::Command(_) => true,
    });
    if fuzzy {
        v.extend(COMMANDS.iter().map(|c| Entry::Command(c)));
    }
    v
}

#[derive(Default)]
struct Launcher {
    query: String,
    sel: usize,
    focused: bool,
}

pub struct TilingDesk {
    /// Esc was pressed: the next key is a desktop command.
    leader: bool,
    launcher: Option<Launcher>,
    help: bool,
}

impl TilingDesk {
    pub fn new() -> Self {
        Self {
            leader: false,
            launcher: None,
            help: false,
        }
    }

    /// Reads this frame's keys for the desktop. Returns true if something changed.
    /// `searching`: the Shell is in a `Ctrl+R` search, where Esc only ends the search.
    fn keys(&mut self, ui: &Ui, t: &mut Tiling, searching: bool) {
        let events = ui.input(|i| i.events.clone());
        for e in events {
            match e {
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } => {
                    if self.launcher.take().is_none() && !self.help && !searching {
                        self.leader = !self.leader;
                    }
                    self.help = false;
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if self.leader => {
                    let digit = [
                        Key::Num1,
                        Key::Num2,
                        Key::Num3,
                        Key::Num4,
                        Key::Num5,
                        Key::Num6,
                        Key::Num7,
                        Key::Num8,
                        Key::Num9,
                    ]
                    .iter()
                    .position(|k| *k == key);
                    let mut done = true;
                    match (key, digit) {
                        (_, Some(n)) if modifiers.shift => t.send(n),
                        (_, Some(n)) => t.switch(n),
                        (Key::Space | Key::P, _) => self.launcher = Some(Launcher::default()),
                        (Key::H | Key::ArrowLeft | Key::K | Key::ArrowUp, _) => t.step(-1),
                        (Key::L | Key::ArrowRight | Key::J | Key::ArrowDown, _) => t.step(1),
                        (Key::Enter, _) => t.swap_master(),
                        (Key::X, _) => {
                            t.close();
                        }
                        (Key::Questionmark | Key::Slash, _) => self.help = true,
                        _ => done = false,
                    }
                    if done {
                        self.leader = false;
                    }
                }
                _ => {}
            }
        }
    }
}

impl DesktopView for TilingDesk {
    fn show(&mut self, ui: &mut Ui, home: &mut Home, cx: &Cx, out: &mut Vec<WbOut>) {
        let (data, save, live) = (cx.data, cx.save, cx.live);
        home.prompt = "[runner@ono-sendai ~]$ ";
        let mut t = save.tiling.clone();
        t.ensure(save.workspaces(data));
        for tool in home.take_opens() {
            t.open(tool);
        }
        if live {
            self.keys(ui, &mut t, home.shell_searching());
        }
        let screen = ui.ctx().content_rect().translate(vec2(0.0, cx.offset));
        let painter = ui.painter_at(screen);
        painter.rect_filled(screen, 0.0, shadow());

        // The bar.
        let bar = Rect::from_min_size(screen.min, vec2(screen.width(), size::BAR_H));
        painter.rect_filled(bar, 0.0, shadow());
        painter.line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0, muted()),
        );
        let small = FontId::new(12.0, FontFamily::Monospace);
        let mut x = bar.left() + 8.0;
        for w in 0..t.workspaces.len() {
            let r = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(22.0, size::BAR_H - 8.0));
            let here = w == t.current;
            let used = !t.workspaces[w].is_empty();
            if here {
                painter.rect_filled(r, 4.0, accent());
            }
            let col = if here {
                shadow()
            } else if used {
                ink()
            } else {
                muted()
            };
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                (w + 1).to_string(),
                small.clone(),
                col,
            );
            if live
                && ui
                    .interact(r, Id::new(("tile-ws", w)), Sense::click())
                    .clicked()
            {
                t.switch(w);
            }
            x += 26.0;
        }
        let mode = if self.leader {
            " LEADER "
        } else {
            "  esc: keys"
        };
        painter.text(
            pos2(x + 8.0, bar.center().y),
            Align2::LEFT_CENTER,
            mode,
            small.clone(),
            if self.leader { accent() } else { muted() },
        );
        let focused = t.focused().map_or("empty", Tool::volume);
        painter.text(
            pos2(x + 110.0, bar.center().y),
            Align2::LEFT_CENTER,
            focused,
            small.clone(),
            ink(),
        );
        let deck = save.deck_def(data).title();
        let monitor = save.monitor_def(data).title();
        let up = ui.input(|i| i.time) as u64;
        let mut right = format!(
            "bank {}¢   heat {}   {}   {}   up {:02}:{:02}:{:02}",
            save.bank,
            save.telco_heat,
            deck,
            monitor,
            up / 3600,
            up / 60 % 60,
            up % 60
        );
        if save.has_effect(data, Desktop::Tiling, AddonEffect::Stats) {
            right = format!(
                "runs {} exits {} flat {} best {}¢   {right}",
                save.runs, save.exits, save.flatlines, save.best_haul
            );
        }
        painter.text(
            bar.right_center() - vec2(8.0, 0.0),
            Align2::RIGHT_CENTER,
            right,
            small.clone(),
            muted(),
        );
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));

        // The tiles.
        let area = Rect::from_min_max(pos2(screen.left(), bar.bottom()), screen.max);
        let layout = t.layout(
            [area.left(), area.top(), area.width(), area.height()],
            size::GAP,
        );
        if layout.is_empty() {
            hint(
                &painter,
                area,
                "empty workspace. esc space to launch a tool",
            );
        }
        let clicked = ui.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let mut focus_to = None;
        for (i, (tool, [x, y, w, h])) in layout.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
            let on = i == t.focus;
            painter.rect_filled(r, size::ROUND, backdrop());
            painter.rect_stroke(
                r,
                size::ROUND,
                Stroke::new(
                    if on { 2.0 } else { 1.0 },
                    if on { accent() } else { muted() },
                ),
                StrokeKind::Inside,
            );
            let label = format!(" {} ", tool.volume().to_lowercase());
            let g =
                painter.layout_no_wrap(label, small.clone(), if on { accent() } else { muted() });
            let tr = Rect::from_min_size(r.min + vec2(12.0, -7.0), g.size());
            painter.rect_filled(tr, 0.0, shadow());
            painter.galley(tr.min, g, ink());
            if live && clicked.is_some_and(|p| r.contains(p)) {
                focus_to = Some(i);
            }
            let content = r
                .shrink2(vec2(10.0, 8.0))
                .with_min_y(r.top() + size::TITLE_H - 4.0);
            let typing = live && on && self.launcher.is_none() && !self.leader;
            if tool == Tool::Shell {
                home.shell(ui, content, data, save, typing, out);
                continue;
            }
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content)
                    .id_salt(("tile", format!("{tool:?}")))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(content.intersect(screen));
            crate::hubtools::wb_visuals(child.visuals_mut());
            if !live {
                child.disable();
            }
            if tool == Tool::System {
                home.system.keys = typing;
            }
            crate::home::scrolled(&mut child, ("tile-scroll", tool), |ui| {
                home.tool_body(ui, tool, data, save, out)
            });
        }
        if let Some(i) = focus_to {
            t.focus = i;
            if t.focused() == Some(Tool::Shell) {
                home.focus();
            }
        }

        if live {
            self.launcher_ui(ui, home, cx, &mut t, out);
            let first = !save.tiling_help_seen;
            if (self.help || first) && help_ui(ui, screen) {
                self.help = false;
                if first {
                    out.push(WbOut::HelpSeen);
                }
            }
        }
        if t != save.tiling {
            out.push(WbOut::Tiling(t));
        }
    }
}

impl TilingDesk {
    /// The launcher: type to filter, arrows to pick, Enter to open or run, Esc to close.
    fn launcher_ui(
        &mut self,
        ui: &mut Ui,
        home: &mut Home,
        cx: &Cx,
        t: &mut Tiling,
        out: &mut Vec<WbOut>,
    ) {
        let Some(l) = &mut self.launcher else { return };
        let fuzzy = cx
            .save
            .has_effect(cx.data, Desktop::Tiling, AddonEffect::Fuzzy);
        let items = entries(fuzzy, cx.save);
        let labels: Vec<String> = items.iter().map(Entry::label).collect();
        let screen = ui.ctx().content_rect();
        let r = Rect::from_center_size(
            pos2(screen.center().x, screen.top() + screen.height() * 0.35),
            vec2(size::LAUNCHER[0], size::LAUNCHER[1]),
        );
        let mut run = None;
        // The box is painted inside the Area, on its own layer, so the widgets draw on top.
        egui::Area::new(Id::new("launcher-area"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .fixed_pos(r.min)
            .show(ui.ctx(), |ui| {
                let p = ui.painter();
                p.rect_filled(r.expand(3.0), size::ROUND + 2.0, shadow());
                p.rect_filled(r, size::ROUND, backdrop());
                p.rect_stroke(
                    r,
                    size::ROUND,
                    Stroke::new(2.0, accent()),
                    StrokeKind::Inside,
                );
                ui.allocate_rect(r, Sense::hover());
                let inner = r.shrink2(vec2(12.0, 10.0));
                let mut ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let ui = &mut ui;
                ui.set_width(inner.width());
                crate::hubtools::wb_visuals(ui.visuals_mut());
                let edit = egui::TextEdit::singleline(&mut l.query)
                    .hint_text(if fuzzy {
                        "open a tool or run a command"
                    } else {
                        "open a tool"
                    })
                    .font(font())
                    .desired_width(f32::INFINITY)
                    .id(Id::new("launcher-input"));
                let resp = ui.add(edit);
                if !l.focused {
                    resp.request_focus();
                    l.focused = true;
                }
                if resp.changed() {
                    l.sel = 0;
                }
                // Filtered after the edit, so the list matches what was just typed.
                let hits = desktop::search(&l.query, &labels, fuzzy);
                let (down, up, enter) = ui.input(|i| {
                    (
                        i.key_pressed(Key::ArrowDown),
                        i.key_pressed(Key::ArrowUp),
                        i.key_pressed(Key::Enter),
                    )
                });
                if !hits.is_empty() {
                    if down {
                        l.sel = (l.sel + 1) % hits.len();
                    }
                    if up {
                        l.sel = (l.sel + hits.len() - 1) % hits.len();
                    }
                    l.sel = l.sel.min(hits.len() - 1);
                }
                ui.add_space(6.0);
                for (k, &i) in hits.iter().take(12).enumerate() {
                    let on = k == l.sel;
                    let txt = egui::RichText::new(&labels[i]).font(font()).color(if on {
                        shadow()
                    } else {
                        ink()
                    });
                    let b = egui::Button::new(txt)
                        .fill(if on { accent() } else { backdrop() })
                        .frame(on);
                    if ui.add_sized([ui.available_width(), 20.0], b).clicked() {
                        run = Some(i);
                    }
                }
                if hits.is_empty() {
                    ui.label(
                        egui::RichText::new("nothing matches")
                            .font(font())
                            .color(muted()),
                    );
                }
                if enter && let Some(&i) = hits.get(l.sel) {
                    run = Some(i);
                }
            });
        if let Some(i) = run {
            match &items[i] {
                Entry::Tool(tool, _) => t.open(*tool),
                Entry::WarDial => {
                    home.term_tab = crate::home::TermTab::WarDial;
                    t.open(Tool::Terminal);
                }
                Entry::Command(c) => {
                    t.open(Tool::Shell);
                    home.say(format!("{}{c}", home.prompt));
                    home.run(c, cx.data, cx.save, out);
                }
            }
            if t.focused() == Some(Tool::Shell) {
                home.focus();
            }
            self.launcher = None;
        }
    }
}

/// The key help. Returns true when it's dismissed (a click, or any key but the leader).
fn help_ui(ui: &mut Ui, screen: Rect) -> bool {
    let lines = [
        ("AnarchyOS // ono-sendai cyberspace vii", ""),
        ("", ""),
        ("esc", "leader: then one of these"),
        ("space", "launcher: open a tool"),
        ("1-9", "go to workspace"),
        ("shift+1-9", "send the tile to a workspace"),
        ("h / l", "focus previous / next tile"),
        ("enter", "swap the tile with the master"),
        ("x", "close the tile"),
        ("?", "this help"),
        ("", ""),
        ("", "the mouse works too. click anywhere to start."),
    ];
    let r = Rect::from_center_size(
        screen.center(),
        vec2(580.0, 22.0 * lines.len() as f32 + 24.0),
    );
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("tiling-help"),
    ));
    painter.rect_filled(r.expand(3.0), size::ROUND + 2.0, shadow());
    painter.rect_filled(r, size::ROUND, backdrop());
    painter.rect_stroke(
        r,
        size::ROUND,
        Stroke::new(2.0, accent()),
        StrokeKind::Inside,
    );
    for (i, (k, v)) in lines.iter().enumerate() {
        let y = r.top() + 14.0 + i as f32 * 22.0;
        painter.text(
            pos2(r.left() + 18.0, y),
            Align2::LEFT_TOP,
            *k,
            font(),
            accent(),
        );
        painter.text(
            pos2(r.left() + 150.0, y),
            Align2::LEFT_TOP,
            *v,
            font(),
            ink(),
        );
    }
    ui.input(|i| {
        i.pointer.primary_clicked()
            || i.events.iter().any(|e| {
                matches!(e, Event::Key { pressed: true, key, .. } if *key != Key::Escape && *key != Key::Questionmark && *key != Key::Slash)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::{Cx, DesktopView};
    use egui::{Event, RawInput};

    #[test]
    fn the_shell_tile_takes_typing_once_enabled() {
        let data = ninsei_core::Data::builtin().unwrap();
        let save = ninsei_core::hub::Save {
            tiling_help_seen: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut desk = TilingDesk::new();
        let mut home = Home::new(Vec::new());
        // The first frame is drawn disabled, as while the desktop settles at boot: the Shell
        // must keep its focus request for the first frame it can take it.
        for (k, events) in [
            Vec::new(),
            Vec::new(),
            vec![Event::Text("ph".into())],
            Vec::new(),
        ]
        .into_iter()
        .enumerate()
        {
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                time: Some(k as f64 * 0.1),
                events,
                ..RawInput::default()
            };
            let mut o = ctx.run_ui(raw, |ui| {
                if k == 0 {
                    ui.disable();
                }
                let cx = Cx {
                    data: &data,
                    save: &save,
                    offset: 0.0,
                    live: true,
                };
                let mut out = Vec::new();
                desk.show(ui, &mut home, &cx, &mut out);
            });
            o.textures_delta.clear();
        }
        assert_eq!(home.shell_text(), "ph");
    }

    #[test]
    fn a_focused_system_tile_takes_its_keys_and_the_shell_keeps_its_own() {
        let data = ninsei_core::Data::builtin().unwrap();
        let mut save = ninsei_core::hub::Save {
            tiling_help_seen: true,
            tools: ninsei_core::data::TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect(),
            decks: vec!["onosendai".into()],
            deck: "onosendai".into(),
            ..Default::default()
        };
        save.ensure_hardware(&data);
        assert_eq!(
            save.firmware_def(&data).desktop,
            ninsei_core::hardware::Desktop::Tiling
        );
        let ctx = egui::Context::default();
        let mut desk = TilingDesk::new();
        let mut home = Home::new(Vec::new());
        let key = |k| {
            let e = |pressed| Event::Key {
                key: k,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            vec![e(true), e(false)]
        };
        let mut t = 0.0;
        let mut frame = |desk: &mut TilingDesk, home: &mut Home, events: Vec<Event>| {
            t += 0.1;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                time: Some(t),
                events,
                ..RawInput::default()
            };
            let mut out = Vec::new();
            let mut o = ctx.run_ui(raw, |ui| {
                let cx = Cx {
                    data: &data,
                    save: &save,
                    offset: 0.0,
                    live: true,
                };
                desk.show(ui, home, &cx, &mut out);
            });
            o.textures_delta.clear();
            // The layout is saved, as the app does.
            for w in out {
                if let crate::home::WbOut::Tiling(l) = w {
                    save.tiling = l;
                }
            }
        };
        frame(&mut desk, &mut home, Vec::new());
        frame(&mut desk, &mut home, Vec::new());
        home.open(Tool::System);
        frame(&mut desk, &mut home, Vec::new());
        frame(&mut desk, &mut home, Vec::new());
        assert!(home.system.keys, "the new System tile is focused");
        frame(&mut desk, &mut home, key(egui::Key::Num2));
        frame(&mut desk, &mut home, Vec::new());
        assert_eq!(home.system.panel, ninsei_core::system::Panel::Monitor);
        assert_eq!(home.shell_text(), "", "the Shell didn't get the key");
        // Focus back on the Shell (Esc, h): System's keys go quiet.
        frame(&mut desk, &mut home, key(egui::Key::Escape));
        frame(&mut desk, &mut home, key(egui::Key::H));
        frame(&mut desk, &mut home, Vec::new());
        assert!(!home.system.keys);
        frame(&mut desk, &mut home, key(egui::Key::Num3));
        frame(&mut desk, &mut home, Vec::new());
        assert_eq!(home.system.panel, ninsei_core::system::Panel::Monitor);
    }
}
