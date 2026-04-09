use egui::{Key, RichText, Sense, Ui};

use crate::cards::CardId;
use crate::completer::Completer;
use crate::theme as t;

#[derive(Debug)]
pub enum SubmitAction {
    Shell(String),
    Chat { text: String, refs: Vec<u64> },
    None,
}

/// Summary of a card for the @ picker.
#[derive(Clone)]
pub struct CardPickerItem {
    pub id: CardId,
    pub kind: &'static str,
    pub label: String,
}

/// Summary of a slash command for the / menu.
#[derive(Clone)]
pub struct SlashMenuItem {
    pub name: String,
    pub summary: String,
}

pub struct InputBar {
    pub text: String,
    pending_refs: Vec<u64>,
    shell_paused: bool,
    pub focus_enabled: bool,
    completer: Completer,
    completions: Vec<String>,
    completion_idx: Option<usize>,
    show_completions: bool,
    // @ picker state
    show_at_picker: bool,
    at_filter: String,
    at_selection: usize,
    // / slash menu state
    show_slash_menu: bool,
    slash_filter: String,
    slash_selection: usize,
    // Shell prompt context
    cwd_display: String,
    git_branch: Option<String>,
}

impl InputBar {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            pending_refs: Vec::new(),
            shell_paused: false,
            focus_enabled: true,
            completer: Completer::new(),
            completions: Vec::new(),
            completion_idx: None,
            show_completions: false,
            show_at_picker: false,
            at_filter: String::new(),
            at_selection: 0,
            show_slash_menu: false,
            slash_filter: String::new(),
            slash_selection: 0,
            cwd_display: "~".to_string(),
            git_branch: None,
        }
    }

    pub fn set_prompt_context(&mut self, cwd: &str, git_branch: Option<&str>) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        self.cwd_display = if !home.is_empty() && cwd.starts_with(&home) {
            format!("~{}", &cwd[home.len()..])
        } else {
            cwd.to_string()
        };
        self.git_branch = git_branch.map(String::from);
    }

    pub fn set_shell_paused(&mut self, paused: bool) {
        self.shell_paused = paused;
    }

    pub fn add_ref(&mut self, card_id: u64) {
        if !self.pending_refs.contains(&card_id) {
            self.pending_refs.push(card_id);
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        card_items: &[CardPickerItem],
        slash_items: &[SlashMenuItem],
    ) -> SubmitAction {
        let mut action = SubmitAction::None;

        egui::Frame::new()
            .fill(t::BG_ELEVATED())
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Ref chips
                if !self.pending_refs.is_empty() {
                    ui.horizontal(|ui| {
                        let mut to_remove = Vec::new();
                        for (i, r) in self.pending_refs.iter().enumerate() {
                            let label = ui.add(
                                egui::Label::new(
                                    RichText::new(format!("@#{r} ✕"))
                                        .small()
                                        .monospace()
                                        .color(t::ACCENT_LIGHT())
                                        .background_color(t::alpha(t::ACCENT(), 20)),
                                )
                                .sense(Sense::click()),
                            );
                            if label.clicked() {
                                to_remove.push(i);
                            }
                        }
                        for i in to_remove.into_iter().rev() {
                            self.pending_refs.remove(i);
                        }
                    });
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("$")
                            .monospace()
                            .color(t::SUCCESS())
                            .size(13.0),
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.text)
                            .desired_width((ui.available_width() - 180.0).max(100.0))
                            .font(egui::FontId::monospace(13.0))
                            .hint_text("Type command or message…")
                            .frame(false)
                            .text_color(t::FG()),
                    );

                    let shell_btn = ui.add_enabled(
                        !self.shell_paused,
                        egui::Button::new(RichText::new("⏎ Shell").size(11.0).color(
                            if self.shell_paused {
                                t::FG_MUTED()
                            } else {
                                t::SUCCESS()
                            },
                        ))
                        .fill(if self.shell_paused {
                            t::SURFACE()
                        } else {
                            t::blend(t::SURFACE(), t::SUCCESS(), 0.08)
                        })
                        .corner_radius(t::BUTTON_CORNER_RADIUS),
                    );

                    let has_refs = !self.pending_refs.is_empty();
                    let chat_shortcut = if cfg!(target_os = "macos") {
                        "⌘⏎ Chat"
                    } else {
                        "Ctrl+⏎ Chat"
                    };
                    let chat_btn = ui.add(
                        egui::Button::new(RichText::new(chat_shortcut).size(11.0).color(
                            if has_refs {
                                t::AMBER()
                            } else {
                                t::ACCENT_LIGHT()
                            },
                        ))
                        .fill(t::blend(t::SURFACE(), t::ACCENT(), 0.06))
                        .corner_radius(t::BUTTON_CORNER_RADIUS),
                    );

                    let text_has_content = !self.text.trim().is_empty();
                    let enter_pressed = response.lost_focus()
                        && ui.input(|i| {
                            i.key_pressed(Key::Enter) && !i.modifiers.command && !i.modifiers.shift
                        });
                    let cmd_enter = ui.input(|i| {
                        i.key_pressed(Key::Enter) && (i.modifiers.command || i.modifiers.ctrl)
                    });

                    // Detect @ and / triggers
                    if response.changed() {
                        self.show_completions = false;
                        self.completion_idx = None;
                        self.update_popup_state();
                    }

                    let esc = ui.input(|i| i.key_pressed(Key::Escape));

                    if esc {
                        self.show_at_picker = false;
                        self.show_slash_menu = false;
                    }

                    // Tab completion (only when no popup active)
                    let tab_pressed = ui.input(|i| i.key_pressed(Key::Tab) && !i.modifiers.shift);
                    if !self.show_at_picker
                        && !self.show_slash_menu
                        && tab_pressed
                        && text_has_content
                    {
                        if self.show_completions && !self.completions.is_empty() {
                            let idx = self
                                .completion_idx
                                .map(|i| (i + 1) % self.completions.len())
                                .unwrap_or(0);
                            self.completion_idx = Some(idx);
                            self.text = self.completions[idx].clone();
                        } else {
                            self.completions = self.completer.complete(&self.text);
                            self.show_completions = !self.completions.is_empty();
                            self.completion_idx = None;
                            if self.completions.len() == 1 {
                                self.text = self.completions[0].clone();
                                self.show_completions = false;
                            }
                        }
                    }

                    // Submit actions
                    if text_has_content
                        && !self.shell_paused
                        && !self.show_at_picker
                        && !self.show_slash_menu
                        && (shell_btn.clicked() || enter_pressed)
                    {
                        self.completer.add_to_history(&self.text);
                        self.show_completions = false;
                        action = SubmitAction::Shell(std::mem::take(&mut self.text));
                    }

                    if text_has_content
                        && !self.show_at_picker
                        && !self.show_slash_menu
                        && (chat_btn.clicked() || cmd_enter)
                    {
                        self.show_completions = false;
                        let refs = std::mem::take(&mut self.pending_refs);
                        action = SubmitAction::Chat {
                            text: std::mem::take(&mut self.text),
                            refs,
                        };
                    }

                    if matches!(action, SubmitAction::None) && self.focus_enabled {
                        response.request_focus();
                    }
                });

                // @ picker popup
                if self.show_at_picker {
                    let filtered: Vec<&CardPickerItem> = card_items
                        .iter()
                        .filter(|item| {
                            self.at_filter.is_empty()
                                || item
                                    .label
                                    .to_lowercase()
                                    .contains(&self.at_filter.to_lowercase())
                                || item.id.to_string().contains(&self.at_filter)
                        })
                        .take(8)
                        .collect();

                    if !filtered.is_empty() {
                        let up = ui.input(|i| i.key_pressed(Key::ArrowUp));
                        let down = ui.input(|i| i.key_pressed(Key::ArrowDown));
                        let confirm = ui.input(|i| {
                            i.key_pressed(Key::Enter) && !i.modifiers.command && !i.modifiers.ctrl
                        }) || ui.input(|i| i.key_pressed(Key::Tab));

                        if up && self.at_selection > 0 {
                            self.at_selection -= 1;
                        }
                        if down && self.at_selection + 1 < filtered.len() {
                            self.at_selection += 1;
                        }
                        self.at_selection = self.at_selection.min(filtered.len().saturating_sub(1));

                        ui.add_space(4.0);
                        let clicked = render_popup(ui, &filtered, self.at_selection, |item| {
                            format!("@#{} [{}] {}", item.id, item.kind, item.label)
                        });

                        let pick_idx = if confirm {
                            Some(self.at_selection)
                        } else {
                            clicked
                        };
                        if let Some(idx) = pick_idx {
                            if let Some(item) = filtered.get(idx) {
                                self.add_ref(item.id);
                                self.remove_at_token();
                                self.show_at_picker = false;
                            }
                        }
                    }
                }

                // / slash command popup
                if self.show_slash_menu {
                    let filtered: Vec<&SlashMenuItem> = slash_items
                        .iter()
                        .filter(|item| {
                            self.slash_filter.is_empty()
                                || item
                                    .name
                                    .to_lowercase()
                                    .contains(&self.slash_filter.to_lowercase())
                        })
                        .take(8)
                        .collect();

                    if !filtered.is_empty() {
                        let up = ui.input(|i| i.key_pressed(Key::ArrowUp));
                        let down = ui.input(|i| i.key_pressed(Key::ArrowDown));
                        let confirm = ui.input(|i| {
                            i.key_pressed(Key::Enter) && !i.modifiers.command && !i.modifiers.ctrl
                        }) || ui.input(|i| i.key_pressed(Key::Tab));

                        if up && self.slash_selection > 0 {
                            self.slash_selection -= 1;
                        }
                        if down && self.slash_selection + 1 < filtered.len() {
                            self.slash_selection += 1;
                        }
                        self.slash_selection =
                            self.slash_selection.min(filtered.len().saturating_sub(1));

                        ui.add_space(4.0);
                        let clicked = render_popup(ui, &filtered, self.slash_selection, |item| {
                            format!("/{} — {}", item.name, item.summary)
                        });

                        let pick_idx = if confirm {
                            Some(self.slash_selection)
                        } else {
                            clicked
                        };
                        if let Some(idx) = pick_idx {
                            if let Some(item) = filtered.get(idx) {
                                self.text = format!("/{}", item.name);
                                self.show_slash_menu = false;
                            }
                        }
                    }
                }

                // Shell tab completions
                if self.show_completions
                    && !self.completions.is_empty()
                    && !self.show_at_picker
                    && !self.show_slash_menu
                {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(t::SURFACE())
                        .corner_radius(t::BUTTON_CORNER_RADIUS)
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            for (i, comp) in self.completions.iter().enumerate() {
                                let is_selected = self.completion_idx == Some(i);
                                let text = RichText::new(comp)
                                    .monospace()
                                    .size(12.0)
                                    .color(if is_selected { t::FG() } else { t::FG_SOFT() });
                                let label = egui::Label::new(text).sense(Sense::click());
                                let resp = ui.add(label);
                                if resp.clicked() {
                                    self.text = comp.clone();
                                    self.show_completions = false;
                                }
                            }
                        });
                }

                if self.shell_paused {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("⚠ Interactive command running — Shell input paused")
                            .small()
                            .color(t::WARNING()),
                    );
                }
            });

        action
    }

    fn update_popup_state(&mut self) {
        // Detect @... token at end of text
        if let Some(at_pos) = self.text.rfind('@') {
            let after = &self.text[at_pos + 1..];
            if !after.contains(' ') {
                self.show_at_picker = true;
                self.at_filter = after.to_string();
                self.at_selection = 0;
                self.show_slash_menu = false;
                return;
            }
        }
        self.show_at_picker = false;

        // Detect /... at the beginning of text
        if self.text.starts_with('/') && !self.text.contains(' ') {
            self.show_slash_menu = true;
            self.slash_filter = self.text[1..].to_string();
            self.slash_selection = 0;
        } else {
            self.show_slash_menu = false;
        }
    }

    fn remove_at_token(&mut self) {
        if let Some(at_pos) = self.text.rfind('@') {
            self.text.truncate(at_pos);
            if self.text.ends_with(' ') || self.text.is_empty() {
                // clean
            } else {
                self.text.push(' ');
            }
        }
    }
}

fn render_popup<T, F>(ui: &mut Ui, items: &[&T], selection: usize, label_fn: F) -> Option<usize>
where
    F: Fn(&T) -> String,
{
    let mut clicked_idx = None;
    egui::Frame::new()
        .fill(t::SURFACE())
        .corner_radius(t::BUTTON_CORNER_RADIUS)
        .inner_margin(6.0)
        .stroke(egui::Stroke::new(0.5, t::BORDER_SUBTLE()))
        .show(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                let is_selected = i == selection;
                let label_text = label_fn(item);
                let text = RichText::new(label_text)
                    .monospace()
                    .size(12.0)
                    .color(if is_selected {
                        t::ACCENT_LIGHT()
                    } else {
                        t::FG_SOFT()
                    });
                let bg = if is_selected {
                    t::alpha(t::ACCENT(), 25)
                } else {
                    egui::Color32::TRANSPARENT
                };
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        let resp = ui.add(egui::Label::new(text).sense(Sense::click()));
                        if resp.clicked() {
                            clicked_idx = Some(i);
                        }
                    });
            }
        });
    clicked_idx
}

impl Default for InputBar {
    fn default() -> Self {
        Self::new()
    }
}
