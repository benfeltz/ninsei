//! The Ono-Sendai's desktop: a keyboard-first tiling window manager, after Omarchy and
//! Hyprland. A slim top bar (workspaces, the focused tool, widgets), tools tiled
//! master-and-stack with gaps, and a launcher. A front-end on `home`; the tiling rules
//! (workspaces, focus, layout, search) are `ninsei_core::desktop`.
//!
//! Keys: **Esc** is the leader (it also lets go of the Shell), then a key:
//! `1`-`9` workspace, `Shift`+digit send the tile there, `Space` launcher, `h`/`l` or arrows
//! focus, `Enter` swap with master, `f` full screen, `[`/`]` resize the master, `x` close,
//! `?` help. The mouse works too: click a tile to focus it, a workspace to switch.
//! Browser-safe: no Ctrl or Alt shortcuts.

use egui::{Event, FontFamily, FontId, Key, Rect, Ui, pos2, vec2};
use ninsei_core::desktop::{self, AddonEffect, Tiling};
use ninsei_core::hardware::Desktop;
use ninsei_core::hub::Tool;

use crate::home::{Cx, DesktopView, Home, WbOut, accent, backdrop, font, hint, ink, muted, shadow};
use crate::kit::anarchy;

use crate::kit::anarchy::size;

/// The desktop's look for the kit: the theme's colours and the app's type, and `ui`'s visuals
/// (what the theme set: the caret, square corners) dressed for the desk.
fn look(ui: &Ui) -> anarchy::Look {
    let mut visuals = ui.visuals().clone();
    crate::hubtools::wb_visuals(&mut visuals);
    anarchy::Look {
        backdrop: backdrop(),
        shadow: shadow(),
        ink: ink(),
        accent: accent(),
        muted: muted(),
        font: font(),
        small: FontId::new(12.0, FontFamily::Monospace),
        visuals,
    }
}

/// What the launcher lists.
#[derive(Clone, Debug)]
enum Entry {
    Tool(Tool, String),
    WarDial,
    Command(&'static str),
}

impl Entry {
    fn label(&self) -> String {
        match self {
            Entry::Tool(_, name) => name.clone(),
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

/// The launcher's entries: the tools you have, each copy of a game you own by its label, and
/// with fzf the Shell's commands.
fn entries(fuzzy: bool, save: &ninsei_core::hub::Save, data: &ninsei_core::Data) -> Vec<Entry> {
    let tool = |t, name: &str| Entry::Tool(t, name.to_string());
    let mut v = vec![
        tool(Tool::Terminal, "CarrierTerm"),
        Entry::WarDial,
        tool(Tool::Rig, "CPU Shopper"),
        tool(Tool::HexEd, "HexEd"),
        tool(Tool::Qwk, "QWK Reader"),
        tool(Tool::IceDb, "ICE DB"),
        tool(Tool::NightComm, "NiGHTCOMM"),
        tool(Tool::TigerTeam, "Tiger Team Pro"),
        tool(Tool::Shell, "Shell"),
        tool(Tool::System, "System"),
    ];
    v.extend(
        Tool::games(data, save)
            .into_iter()
            .map(|t| Entry::Tool(t, t.name(data))),
    );
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

pub struct TilingDesk {
    /// Esc was pressed: the next key is a desktop command.
    leader: bool,
    launcher: Option<anarchy::Launcher>,
    help: bool,
    /// Whether the Shell was taking the keys last frame.
    shell_typing: bool,
    /// The Shell has just started taking the keys: it asks for its focus next frame, after the
    /// key that gave them back has been read by someone else.
    focus_pending: bool,
}

impl TilingDesk {
    pub fn new() -> Self {
        Self {
            leader: false,
            launcher: None,
            help: false,
            shell_typing: false,
            focus_pending: false,
        }
    }

    /// Reads this frame's keys for the desktop. `searching`: the Shell is in a `Ctrl+R`
    /// search, where Esc only ends the search. A key the leader acts on is consumed (its
    /// press, release and the text it types), so no tile drawn later this frame sees it.
    fn keys(&mut self, ui: &Ui, t: &mut Tiling, searching: bool, help_open: bool) {
        let events = ui.input(|i| i.events.clone());
        let mut used: Vec<Key> = Vec::new();
        for e in events {
            match e {
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } => {
                    if self.launcher.take().is_none() && !help_open && !searching {
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
                        (Key::Space | Key::P, _) => {
                            self.launcher = Some(anarchy::Launcher::default())
                        }
                        (Key::H | Key::ArrowLeft | Key::K | Key::ArrowUp, _) => t.step(-1),
                        (Key::L | Key::ArrowRight | Key::J | Key::ArrowDown, _) => t.step(1),
                        (Key::Enter, _) => t.swap_master(),
                        (Key::F, _) => t.toggle_full(),
                        (Key::OpenBracket, _) => t.resize_master(-1),
                        (Key::CloseBracket, _) => t.resize_master(1),
                        (Key::X, _) => {
                            t.close();
                        }
                        (Key::Questionmark | Key::Slash, _) => self.help = true,
                        _ => done = false,
                    }
                    if done {
                        self.leader = false;
                        used.push(key);
                    }
                }
                _ => {}
            }
        }
        if !used.is_empty() {
            ui.ctx().input_mut(|i| {
                i.events.retain(|e| match e {
                    Event::Key { key, .. } => !used.contains(key),
                    Event::Text(_) => false,
                    _ => true,
                });
                for k in &used {
                    i.keys_down.remove(k);
                }
            });
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
            self.keys(
                ui,
                &mut t,
                home.shell_searching(),
                self.help || !save.tiling_help_seen,
            );
        }
        let screen = ui.ctx().content_rect().translate(vec2(0.0, cx.offset));
        let painter = ui.painter_at(screen);
        painter.rect_filled(screen, 0.0, shadow());

        let look = look(ui);
        // The bar: the workspaces, the mode, the focused tile and the widgets.
        let workspaces: Vec<anarchy::Workspace> = (0..t.workspaces.len())
            .map(|w| anarchy::Workspace {
                here: w == t.current,
                used: !t.workspaces[w].is_empty(),
            })
            .collect();
        let mut focused = t.focused().map_or("empty".into(), |t| t.name(data));
        if t.shape().full {
            focused.push_str("  [full]");
        }
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
        let bar = anarchy::Bar {
            workspaces: &workspaces,
            mode: if self.leader {
                " LEADER "
            } else {
                "  esc: keys"
            },
            mode_lit: self.leader,
            focused: &focused,
            right: &right,
        };
        if let Some(w) = anarchy::bar(ui, &painter, screen, &bar, &look, live) {
            t.switch(w);
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));

        // The tiles.
        let area = work_area(screen);
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
        // Esc drops egui's focus, and a leader, launcher or help key must not be typed into the
        // Shell: whenever the Shell goes from not taking the keys to taking them (a closed
        // leader, launcher or help, or focus moved onto it), it asks for its focus on the next
        // frame, once that key has been read.
        let help_open = self.help || !save.tiling_help_seen;
        let shell_typing = live
            && t.focused() == Some(Tool::Shell)
            && self.launcher.is_none()
            && !self.leader
            && !help_open;
        if std::mem::take(&mut self.focus_pending) && shell_typing {
            home.focus();
        } else if shell_typing && !self.shell_typing {
            self.focus_pending = true;
        }
        self.shell_typing = shell_typing;
        for (tool, [x, y, w, h]) in layout {
            // The tile's place on the workspace (full screen lays out only the focused one).
            let i = t.tiles().iter().position(|x| *x == tool).unwrap_or(t.focus);
            let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
            let on = i == t.focus;
            anarchy::tile(&painter, r, &tool.name(data).to_lowercase(), on, &look);
            if live && clicked.is_some_and(|p| r.contains(p)) {
                focus_to = Some(i);
            }
            let content = anarchy::tile_content(r);
            let typing = live && on && self.launcher.is_none() && !self.leader && !help_open;
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
            // The focused tile has the keyboard while nothing's over it.
            home.tool_in(
                &mut child,
                tool,
                typing && cx.keys,
                data,
                save,
                out,
                full_screen_hint(t.shape().full, t.tiles().len()),
                None,
                ("tile-scroll", tool),
            );
        }
        if let Some(i) = focus_to {
            t.focus = i;
            if t.focused() == Some(Tool::Shell) {
                home.focus();
            }
        }

        if live {
            self.launcher_ui(ui, home, cx, &mut t, out, &look);
            let first = !save.tiling_help_seen;
            if (self.help || first) && anarchy::help(ui, screen, &HELP, &look) {
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
        look: &anarchy::Look,
    ) {
        let Some(l) = &mut self.launcher else { return };
        let fuzzy = cx
            .save
            .has_effect(cx.data, Desktop::Tiling, AddonEffect::Fuzzy);
        let items = entries(fuzzy, cx.save, cx.data);
        let labels: Vec<String> = items.iter().map(Entry::label).collect();
        let hint = if fuzzy {
            "open a tool or run a command"
        } else {
            "open a tool"
        };
        let run = anarchy::launcher(
            ui.ctx(),
            ui.ctx().content_rect(),
            l,
            &labels,
            |q| desktop::search(q, &labels, fuzzy),
            hint,
            look,
        );
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

/// The "too small" notice's hint: full screen, when it would give the tile more room (not
/// when the workspace is full screen already, where Esc f would shrink it, nor for a tile
/// that's alone).
fn full_screen_hint(full: bool, tiles: usize) -> Option<&'static str> {
    (!full && tiles > 1).then_some("Esc f: full screen")
}

/// Where tiles go on `screen`: everything under the bar.
pub(crate) fn work_area(screen: Rect) -> Rect {
    Rect::from_min_max(pos2(screen.left(), screen.top() + size::BAR_H), screen.max)
}

/// The key help: a key (after the leader) and what it does.
const HELP: [(&str, &str); 14] = [
    ("AnarchyOS // ono-sendai cyberspace vii", ""),
    ("", ""),
    ("esc", "leader: then one of these"),
    ("space", "launcher: open a tool"),
    ("1-9", "go to workspace"),
    ("shift+1-9", "send the tile to a workspace"),
    ("h / l", "focus previous / next tile"),
    ("enter", "swap the tile with the master"),
    ("f", "full screen, and back"),
    ("[ / ]", "shrink / grow the master"),
    ("x", "close the tile"),
    ("?", "this help"),
    ("", ""),
    ("", "the mouse works too. click anywhere to start."),
];

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
                    keys: true,
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
        crate::uitest::themed(&ctx, &data);
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
                    keys: true,
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
        assert!(home.notices.is_empty(), "System fits the stack tile");
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

    /// The tiling desktop at 1280x800 on a `uitest::Harness`, with the save the app would keep.
    struct Rig {
        h: crate::uitest::Harness,
        data: ninsei_core::Data,
        save: ninsei_core::hub::Save,
        desk: TilingDesk,
        home: Home,
    }

    impl Rig {
        fn new(help_seen: bool) -> Self {
            let mut r = Rig {
                h: crate::uitest::Harness::new(1280.0, 800.0),
                data: ninsei_core::Data::builtin().unwrap(),
                save: ninsei_core::hub::Save {
                    tiling_help_seen: help_seen,
                    ..Default::default()
                },
                desk: TilingDesk::new(),
                home: Home::new(Vec::new()),
            };
            crate::uitest::themed(&r.h.ctx, &r.data);
            r.frame(Vec::new());
            r.frame(Vec::new());
            r
        }

        fn frame(&mut self, events: Vec<Event>) {
            let mut out = Vec::new();
            let (data, save, desk, home) = (&self.data, &self.save, &mut self.desk, &mut self.home);
            self.h.frame(events, |ui| {
                let cx = Cx {
                    data,
                    save,
                    offset: 0.0,
                    live: true,
                    keys: true,
                };
                desk.show(ui, home, &cx, &mut out);
            });
            for w in out {
                match w {
                    crate::home::WbOut::Tiling(l) => self.save.tiling = l,
                    crate::home::WbOut::HelpSeen => self.save.tiling_help_seen = true,
                    _ => {}
                }
            }
        }

        /// Runs `events`, then a quiet frame.
        fn send(&mut self, events: Vec<Event>) {
            self.frame(events);
            self.frame(Vec::new());
        }

        /// A key as a real keyboard sends it: the press, and the text it types.
        fn key_text(&mut self, k: egui::Key, s: &str) {
            let mut e = crate::uitest::key(k);
            e.extend(crate::uitest::text(s));
            self.send(e);
        }

        fn key(&mut self, k: egui::Key) {
            self.send(crate::uitest::key(k));
        }

        fn type_text(&mut self, s: &str) {
            self.send(crate::uitest::text(s));
        }
    }

    #[test]
    fn the_shell_takes_typing_after_the_first_boot_help_closes() {
        let mut r = Rig::new(false);
        // The key that closes the help isn't typed; what follows is.
        r.key_text(egui::Key::A, "a");
        assert!(r.save.tiling_help_seen, "the help closed");
        r.type_text("help");
        assert_eq!(r.home.shell_text(), "help");
        r.key(egui::Key::Enter);
        assert!(r.home.shell_has("dial <n>"), "help ran in the Shell");
    }

    #[test]
    fn the_shell_takes_typing_after_a_click_closes_the_help() {
        for p in [pos2(640.0, 400.0), pos2(640.0, 760.0), pos2(30.0, 400.0)] {
            let mut r = Rig::new(false);
            r.send(crate::uitest::click(p));
            assert!(r.save.tiling_help_seen, "the help closed at {p:?}");
            r.type_text("help");
            assert_eq!(r.home.shell_text(), "help", "typing after a click at {p:?}");
        }
    }

    #[test]
    fn esc_on_the_first_boot_help_closes_it_and_the_shell_takes_typing() {
        let mut r = Rig::new(false);
        r.key(egui::Key::Escape);
        assert!(r.save.tiling_help_seen, "Esc closed the help");
        assert!(!r.desk.leader, "and didn't open the leader");
        r.type_text("help");
        assert_eq!(r.home.shell_text(), "help");
    }

    #[test]
    fn leader_keys_reach_the_desktop_never_the_shell() {
        let mut r = Rig::new(true);
        r.type_text("ph");
        // Esc, Esc: the leader opens and closes, and the Shell types again.
        r.key(egui::Key::Escape);
        r.key(egui::Key::Escape);
        r.type_text("x");
        assert_eq!(
            r.home.shell_text(),
            "phx",
            "Esc, Esc leaves the Shell typing"
        );
        // Esc, h (focus previous, the Shell is alone): no h on the command line.
        r.key(egui::Key::Escape);
        r.key_text(egui::Key::H, "h");
        assert_eq!(r.home.shell_text(), "phx", "the leader's h isn't typed");
        // Esc, Enter (swap with master) doesn't submit the command line.
        r.key(egui::Key::Escape);
        r.key(egui::Key::Enter);
        assert_eq!(
            r.home.shell_text(),
            "phx",
            "the leader's Enter isn't submitted"
        );
        // Esc, ?, then the key that closes the help: neither is typed.
        r.key(egui::Key::Escape);
        r.key_text(egui::Key::Questionmark, "?");
        assert!(r.desk.help, "the help is open");
        r.key_text(egui::Key::Q, "q");
        assert!(!r.desk.help, "the help closed");
        r.type_text("y");
        assert_eq!(r.home.shell_text(), "phxy");
        // A click in the focused tile keeps it typing.
        r.send(crate::uitest::click(pos2(640.0, 400.0)));
        r.type_text("z");
        assert_eq!(r.home.shell_text(), "phxyz");
    }

    #[test]
    fn moving_the_focus_onto_the_shell_with_the_leader_gives_it_the_keys() {
        let mut r = Rig::new(true);
        r.save.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        r.home.open(Tool::IceDb);
        r.frame(Vec::new());
        r.frame(Vec::new());
        assert_ne!(
            r.save.tiling.focused(),
            Some(Tool::Shell),
            "the new tile is focused"
        );
        r.type_text("q");
        assert_eq!(r.home.shell_text(), "", "the Shell isn't typing");
        // Esc, h onto the Shell: the h isn't typed, what follows is.
        r.key(egui::Key::Escape);
        r.key_text(egui::Key::H, "h");
        assert_eq!(r.save.tiling.focused(), Some(Tool::Shell));
        r.type_text("ph");
        assert_eq!(r.home.shell_text(), "ph");
    }

    #[test]
    fn esc_f_full_screens_the_focused_tile_and_esc_brackets_resize_the_master() {
        let mut r = Rig::new(true);
        r.save.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        r.home.open(Tool::IceDb);
        r.frame(Vec::new());
        r.frame(Vec::new());
        assert_eq!(r.save.tiling.focused(), Some(Tool::IceDb), "the stack tile");
        let master = pos2(100.0, 400.0);
        r.key(egui::Key::Escape);
        r.key(egui::Key::F);
        assert!(r.save.tiling.shape().full);
        // The master isn't drawn: a click where it was lands on the full-screen tile.
        r.send(crate::uitest::click(master));
        assert_eq!(r.save.tiling.focused(), Some(Tool::IceDb));
        r.key(egui::Key::Escape);
        r.key(egui::Key::F);
        assert!(!r.save.tiling.shape().full, "and back");
        r.send(crate::uitest::click(master));
        assert_eq!(
            r.save.tiling.focused(),
            Some(Tool::Shell),
            "the master is back"
        );
        // Esc ] grows the master, Esc [ shrinks it.
        let pct = ninsei_core::desktop::MASTER_PCT;
        let step = ninsei_core::desktop::MASTER_STEP;
        r.key(egui::Key::Escape);
        r.key(egui::Key::CloseBracket);
        assert_eq!(r.save.tiling.shape().master, pct + step);
        for _ in 0..2 {
            r.key(egui::Key::Escape);
            r.key(egui::Key::OpenBracket);
        }
        assert_eq!(r.save.tiling.shape().master, pct - step);
        assert_eq!(r.home.shell_text(), "", "none of it was typed");
    }

    #[test]
    fn the_help_names_full_screen_and_the_resize_keys() {
        assert!(HELP.contains(&("f", "full screen, and back")));
        assert!(HELP.contains(&("[ / ]", "shrink / grow the master")));
    }

    #[test]
    fn the_leaders_keys_never_reach_a_focused_system_tile() {
        let mut r = Rig::new(true);
        r.save.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        r.save.decks = vec!["onosendai".into()];
        r.save.deck = "onosendai".into();
        r.save.ensure_hardware(&r.data);
        r.home.open(Tool::System);
        r.frame(Vec::new());
        r.frame(Vec::new());
        assert!(r.home.system.keys, "the System tile has the keys");
        let pct = ninsei_core::desktop::MASTER_PCT;
        let step = ninsei_core::desktop::MASTER_STEP;
        r.key(egui::Key::Escape);
        r.key_text(egui::Key::CloseBracket, "]");
        assert_eq!(r.save.tiling.shape().master, pct + step, "the master grew");
        r.key(egui::Key::Escape);
        r.key_text(egui::Key::OpenBracket, "[");
        assert_eq!(r.save.tiling.shape().master, pct);
        assert_eq!(
            r.home.system.panel,
            ninsei_core::system::Panel::Overview,
            "and System didn't turn its panel"
        );
        // Without the leader, [ and ] are System's again.
        r.key_text(egui::Key::CloseBracket, "]");
        assert_eq!(r.home.system.panel, ninsei_core::system::Panel::Monitor);
    }

    #[test]
    fn the_notice_offers_full_screen_only_when_it_would_help() {
        assert_eq!(full_screen_hint(false, 3), Some("Esc f: full screen"));
        assert_eq!(
            full_screen_hint(true, 3),
            None,
            "already full: Esc f would shrink it"
        );
        assert_eq!(
            full_screen_hint(false, 1),
            None,
            "alone, it has the whole workspace"
        );
    }

    #[test]
    fn the_launcher_scrolls_to_an_entry_past_its_first_screenful() {
        let mut r = Rig::new(true);
        r.save.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        let fzf = r
            .data
            .addons
            .iter()
            .find(|a| a.effect == AddonEffect::Fuzzy)
            .expect("the fzf add-on")
            .id
            .clone();
        r.save.addons.push(fzf);
        let labels: Vec<String> = entries(true, &r.save, &r.data)
            .iter()
            .map(Entry::label)
            .collect();
        assert!(labels.len() > 12, "more than a screenful: {labels:?}");
        r.key(egui::Key::Escape);
        r.key(egui::Key::Space);
        // `$ help`, past the first screenful: its list shows that it ran.
        let help = labels.iter().position(|l| l == "$ help").expect("help");
        assert!(help >= 12);
        for _ in 0..help {
            r.key(egui::Key::ArrowDown);
        }
        let l = r.desk.launcher.as_ref().expect("the launcher");
        assert_eq!(l.sel, help);
        assert!(l.list.shows(help), "the entry is in view");
        assert!(!l.list.shows(0), "the first has scrolled away");
        r.key(egui::Key::Enter);
        assert!(r.desk.launcher.is_none(), "it ran and closed");
        assert!(r.home.shell_has("dial <n>"), "help ran in the Shell");
    }

    #[test]
    fn the_launcher_is_dressed_by_the_theme() {
        let r = Rig::new(true);
        let p = crate::theme::Palette::from_def(&r.data.themes[0]);
        let mut seen = None;
        let mut h = crate::uitest::Harness::new(400.0, 300.0);
        crate::uitest::themed(&h.ctx, &r.data);
        h.frame(Vec::new(), |ui| seen = Some(look(ui).visuals));
        let v = seen.unwrap();
        assert_eq!(v.text_cursor.stroke.color, p.fg, "the theme's caret");
        assert_eq!(v.widgets.inactive.corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(v.widgets.hovered.expansion, 0.0);
    }

    /// A rig with every tool and the fzf add-on, so the launcher runs past a screenful.
    fn with_fzf() -> Rig {
        let mut r = Rig::new(true);
        r.save.tools = ninsei_core::data::TOOLS
            .iter()
            .map(|t| t.to_string())
            .collect();
        let fzf = r
            .data
            .addons
            .iter()
            .find(|a| a.effect == AddonEffect::Fuzzy)
            .unwrap()
            .id
            .clone();
        r.save.addons.push(fzf);
        r
    }

    #[test]
    fn a_workspace_pill_click_switches_to_it() {
        let mut r = Rig::new(true);
        // The second pill: 8px in, a pill and a step along, in the middle of the bar.
        let at = pos2(
            8.0 + size::PILL_STEP + size::PILL[0] / 2.0,
            size::BAR_H / 2.0,
        );
        r.send(crate::uitest::click(at));
        assert_eq!(r.save.tiling.current, 1);
    }

    #[test]
    fn typing_filters_the_launcher_and_a_click_opens_the_row() {
        let mut r = with_fzf();
        r.key(egui::Key::Escape);
        r.key(egui::Key::Space);
        r.key(egui::Key::ArrowDown);
        assert_eq!(r.desk.launcher.as_ref().unwrap().sel, 1);
        r.type_text("ice");
        assert_eq!(
            r.desk.launcher.as_ref().unwrap().sel,
            0,
            "typing starts the pick over"
        );
        let row =
            r.h.find("ICE DB")
                .unwrap_or_else(|| panic!("{:?}", r.h.texts()));
        assert!(r.h.find("CarrierTerm").is_none(), "filtered out");
        r.send(crate::uitest::click(row.center()));
        assert!(r.desk.launcher.is_none());
        assert_eq!(r.save.tiling.focused(), Some(Tool::IceDb));
    }

    #[test]
    fn esc_closes_the_launcher_without_the_leader() {
        let mut r = Rig::new(true);
        r.key(egui::Key::Escape);
        r.key(egui::Key::Space);
        assert!(r.desk.launcher.is_some());
        r.key(egui::Key::Escape);
        assert!(r.desk.launcher.is_none());
        assert!(!r.desk.leader, "closing it isn't the leader");
    }

    #[test]
    fn the_wheel_scrolls_the_launcher_and_its_mark_says_where() {
        let mut r = with_fzf();
        r.key(egui::Key::Escape);
        r.key(egui::Key::Space);
        let n = entries(true, &r.save, &r.data).len();
        let painted = |r: &Rig| -> String {
            r.h.painted()
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("|")
        };
        assert!(painted(&r).contains(&format!("1/{n}")), "{}", painted(&r));
        let over = r.h.find("CarrierTerm").unwrap().center();
        let wheel = vec![
            egui::Event::PointerMoved(over),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(0.0, -3.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        r.send(wheel);
        r.frame(Vec::new());
        assert!(
            r.desk.launcher.as_ref().unwrap().list.top > 0,
            "the wheel scrolled it"
        );
    }

    #[test]
    fn the_launcher_and_help_sit_on_whole_pixels() {
        let data = ninsei_core::Data::builtin().unwrap();
        let mut h = crate::uitest::Harness::new(1366.0, 768.0);
        crate::uitest::themed(&h.ctx, &data);
        let mut l = anarchy::Launcher::default();
        let labels = vec!["one".to_string(), "two".to_string()];
        h.frame(Vec::new(), |ui| {
            let look = look(ui);
            let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(1366.0, 768.0));
            anarchy::launcher(ui.ctx(), screen, &mut l, &labels, |_| vec![0, 1], "", &look);
            let odd = Rect::from_min_size(egui::Pos2::ZERO, vec2(1365.0, 767.0));
            anarchy::help(ui, odd, &HELP, &look);
        });
        h.frame(Vec::new(), |ui| {
            let look = look(ui);
            let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(1366.0, 768.0));
            anarchy::launcher(ui.ctx(), screen, &mut l, &labels, |_| vec![0, 1], "", &look);
            let odd = Rect::from_min_size(egui::Pos2::ZERO, vec2(1365.0, 767.0));
            anarchy::help(ui, odd, &HELP, &look);
        });
        // Every box (the frames and their fills) starts on a whole pixel.
        for (r, fill, stroke) in h.rects() {
            if r.width() > 300.0
                && (fill != egui::Color32::TRANSPARENT || stroke != egui::Color32::TRANSPARENT)
            {
                assert_eq!(r.min, r.min.round(), "{r:?}");
            }
        }
    }

    #[test]
    fn the_launcher_lists_each_copy_of_a_game_only_once_owned() {
        let data = ninsei_core::Data::builtin().unwrap();
        let mut save = ninsei_core::hub::Save::default();
        let games = |save: &ninsei_core::hub::Save| -> Vec<String> {
            entries(false, save, &data)
                .iter()
                .filter(|e| matches!(e, Entry::Tool(Tool::Game(..), _)))
                .map(Entry::label)
                .collect()
        };
        assert!(games(&save).is_empty(), "no game, no entry");
        save.learn_game("nibbler", ninsei_core::hub::GameCopy::Cracked);
        save.learn_game("nibbler", ninsei_core::hub::GameCopy::Boxed);
        assert_eq!(games(&save), ["NIBBLER [2K4]", "Nibbler"]);
    }

    #[test]
    fn a_game_from_the_launcher_tiles_and_takes_the_keys_but_never_the_leaders() {
        let mut r = Rig::new(true);
        r.save.intros = false;
        r.save
            .learn_game("minefield", ninsei_core::hub::GameCopy::Cracked);
        let mine = Tool::game("minefield", ninsei_core::hub::GameCopy::Cracked).unwrap();
        r.key(egui::Key::Escape);
        r.key(egui::Key::Space);
        r.type_text("minefield");
        r.key(egui::Key::Enter);
        assert_eq!(r.save.tiling.focused(), Some(mine));
        r.frame(Vec::new());
        let at = r.home.game(mine).unwrap().cursor().unwrap();
        r.key(egui::Key::ArrowRight);
        assert_eq!(r.home.game(mine).unwrap().cursor(), Some((at.0 + 1, at.1)));
        // The leader's keys are the desktop's: Esc, then an arrow moves the focus, never the
        // cursor, and the game waits while the leader's up.
        r.key(egui::Key::Escape);
        assert!(r.home.game(mine).unwrap().paused());
        r.key(egui::Key::ArrowLeft);
        assert_eq!(r.home.game(mine).unwrap().cursor(), Some((at.0 + 1, at.1)));
        assert_eq!(r.save.tiling.focused(), Some(Tool::Shell));
    }
}
