//! The `talk` window for MSG-node encounters: chat lines, numbered choices, a program picker.

use egui::{Align2, Context, Event, Key, RichText, Stroke, vec2};
use ninsei_core::{Action, ChoiceEffect, Game, LogKind};

use crate::theme::Palette;

/// UI-only state: which choice is waiting for you to pick a program.
#[derive(Default)]
pub struct EncounterView {
    pub picking: Option<usize>,
    /// The encounter (node, event) shown last frame. Keys typed in the frame a window opens
    /// belong to whatever opened it (e.g. the `1` in `jack 1`), so that frame ignores input.
    seen: Option<(usize, usize)>,
}

impl EncounterView {
    pub fn show(&mut self, ctx: &Context, game: &Game, p: &Palette) -> Vec<Action> {
        let mut out = Vec::new();
        let Some(e) = &game.encounter else {
            self.picking = None;
            self.seen = None;
            return out;
        };
        let ev = &game.data.events[e.event];
        let id = (e.node, e.event);
        let fresh = self.seen != Some(id);
        self.seen = Some(id);
        // The conversation owns the keyboard while it's open.
        ctx.memory_mut(|m| m.stop_text_input());

        // Keys: digits choose (or pick a slot), enter closes, escape backs out.
        let keys: Vec<char> = if fresh {
            Vec::new()
        } else {
            ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|ev| match ev {
                        Event::Text(t) => t.chars().next(),
                        Event::Key {
                            key: Key::Enter,
                            pressed: true,
                            ..
                        } => Some('\n'),
                        Event::Key {
                            key: Key::Escape,
                            pressed: true,
                            ..
                        } => Some('\x1b'),
                        _ => None,
                    })
                    .collect()
            })
        };
        for k in keys {
            if e.done {
                if k == '\n' || k == '\x1b' {
                    out.push(Action::EndEncounter);
                }
            } else if let Some(c) = self.picking {
                match k.to_digit(10) {
                    Some(d) => out.push(Action::Choose {
                        choice: c,
                        slot: Some(d as usize),
                    }),
                    None if k == '\x1b' => self.picking = None,
                    None => {}
                }
            } else if let Some(d) = k.to_digit(10).filter(|&d| d >= 1) {
                let c = d as usize - 1;
                if game.choice_needs_slot(c) {
                    self.picking = Some(c);
                } else {
                    out.push(Action::Choose {
                        choice: c,
                        slot: None,
                    });
                }
            }
        }

        egui::Window::new(format!("talk: runner@ninsei <-> {}", ev.from))
            .id(egui::Id::new("encounter"))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, -30.0))
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0, p.fg))
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.label(
                    RichText::new(format!("-- {} --", game.encounter_title().to_uppercase()))
                        .color(p.bright),
                );
                ui.add_space(4.0);
                for line in &ev.text {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("<{}>", ev.from)).color(p.dim));
                        ui.label(RichText::new(game.fill(line, None)).color(p.fg));
                    });
                }
                ui.add_space(8.0);

                if e.done {
                    for l in &e.result {
                        let col = match l.kind {
                            LogKind::Bad | LogKind::Loot => p.bright,
                            LogKind::Sys => p.dim,
                            _ => p.fg,
                        };
                        ui.label(RichText::new(format!("* {}", l.text)).color(col));
                    }
                    ui.add_space(8.0);
                    if ui.button(" [enter] close ").clicked() {
                        out.push(Action::EndEncounter);
                    }
                    return;
                }

                if let Some(c) = self.picking {
                    let effect = &ev.choices[c].effect;
                    ui.label(RichText::new("which program? [0-9]  [esc] back").color(p.bright));
                    for (i, s) in game.hdd.iter().enumerate() {
                        let note = match effect {
                            ChoiceEffect::Sell { per_ram } => {
                                format!("sells for {}¢", game.sale_price(i, *per_ram))
                            }
                            ChoiceEffect::Trade { .. } => {
                                let got = e.offer.map_or(String::new(), |w| {
                                    game.data.warez[w].name.to_ascii_lowercase()
                                });
                                format!("-> {got}.exe")
                            }
                            _ => String::new(),
                        };
                        let blocked = game.choice_blocker(c, Some(i));
                        let label = format!(
                            "[{i}] {:<16} r{} c{}  {note}",
                            s.spec.file(),
                            s.spec.ram,
                            s.spec.cpu
                        );
                        let btn = ui.add_enabled(
                            blocked.is_none(),
                            egui::Button::new(RichText::new(label).monospace()),
                        );
                        if let Some(why) = blocked {
                            btn.on_disabled_hover_text(why);
                        } else if btn.clicked() {
                            out.push(Action::Choose {
                                choice: c,
                                slot: Some(i),
                            });
                        }
                    }
                    if ui.small_button("back").clicked() {
                        self.picking = None;
                    }
                    return;
                }

                for (i, ch) in ev.choices.iter().enumerate() {
                    let label = format!("[{}] {}", i + 1, game.fill(&ch.label, Some(i)));
                    // A choice that needs a program is blocked only if no program would do.
                    let blocked = if game.choice_needs_slot(i) {
                        (0..game.hdd.len())
                            .all(|s| game.choice_blocker(i, Some(s)).is_some())
                            .then(|| game.choice_blocker(i, Some(0)).unwrap_or_default())
                    } else {
                        game.choice_blocker(i, None)
                    };
                    ui.horizontal(|ui| {
                        let btn = ui.add_enabled(blocked.is_none(), egui::Button::new(label));
                        if btn.clicked() {
                            if game.choice_needs_slot(i) {
                                self.picking = Some(i);
                            } else {
                                out.push(Action::Choose {
                                    choice: i,
                                    slot: None,
                                });
                            }
                        }
                        if let Some(why) = blocked {
                            ui.label(RichText::new(format!("({why})")).small().color(p.dim));
                        }
                    });
                }
            });
        if out
            .iter()
            .any(|a| matches!(a, Action::Choose { .. } | Action::EndEncounter))
        {
            self.picking = None;
        }
        out
    }
}
