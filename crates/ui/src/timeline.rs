use egui::text::LayoutJob;
use egui::{FontId, RichText, Stroke, TextFormat, Ui, Vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::cards::{
    Card, CardId, ChatCard, OutputLine, ShellCard, SystemCard, ToolState, ToolTurn,
};
use crate::theme as t;

#[derive(Debug, Clone)]
pub enum TimelineAction {
    None,
    ToolApprove {
        card_id: CardId,
        tool_use_id: String,
    },
    ToolDeny {
        card_id: CardId,
        tool_use_id: String,
    },
    ToolApproveAll {
        card_id: CardId,
    },
    AddRef {
        card_id: CardId,
    },
}

const MAX_VISIBLE_OUTPUT_LINES: usize = 20;

pub struct Timeline {
    pub cards: Vec<Card>,
    next_id: CardId,
    auto_scroll: bool,
    md_cache: CommonMarkCache,
    /// Cached heights for virtualized rendering (card_id -> height).
    card_heights: std::collections::HashMap<CardId, f32>,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            cards: Vec::new(),
            next_id: 1,
            auto_scroll: true,
            md_cache: CommonMarkCache::default(),
            card_heights: std::collections::HashMap::new(),
        }
    }

    pub fn next_card_id(&mut self) -> CardId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn push_card(&mut self, card: Card) {
        self.next_id = self.next_id.max(card.id() + 1);
        self.cards.push(card);
        self.auto_scroll = true;
    }

    pub fn last_shell_card_mut(&mut self) -> Option<&mut ShellCard> {
        self.cards.iter_mut().rev().find_map(|c| match c {
            Card::Shell(sc) => Some(sc),
            _ => None,
        })
    }

    pub fn append_chat_response(&mut self, card_id: CardId, text: &str) {
        if let Some(Card::Chat(cc)) = self.cards.iter_mut().find(|c| c.id() == card_id) {
            if cc.response == "Thinking..." {
                cc.response.clear();
            }
            cc.response.push_str(text);
            self.card_heights.remove(&card_id);
            self.auto_scroll = true;
        }
    }

    pub fn finish_chat_streaming(&mut self, card_id: CardId) {
        if let Some(Card::Chat(cc)) = self.cards.iter_mut().find(|c| c.id() == card_id) {
            cc.streaming = false;
            self.card_heights.remove(&card_id);
        }
    }

    pub fn add_tool_pending(
        &mut self,
        card_id: CardId,
        tool_use_id: String,
        name: String,
        input: String,
    ) {
        if let Some(Card::Chat(cc)) = self.cards.iter_mut().find(|c| c.id() == card_id) {
            cc.tool_turns.push(ToolTurn {
                tool_use_id,
                name,
                input,
                state: ToolState::Pending,
            });
            self.card_heights.remove(&card_id);
            self.auto_scroll = true;
        }
    }

    pub fn set_tool_running(&mut self, card_id: CardId, tool_use_id: &str) {
        if let Some(Card::Chat(cc)) = self.cards.iter_mut().find(|c| c.id() == card_id) {
            if let Some(t) = cc
                .tool_turns
                .iter_mut()
                .find(|t| t.tool_use_id == tool_use_id)
            {
                t.state = ToolState::Running;
                self.card_heights.remove(&card_id);
            }
        }
    }

    pub fn set_tool_result(
        &mut self,
        card_id: CardId,
        tool_use_id: &str,
        output: String,
        is_error: bool,
    ) {
        if let Some(Card::Chat(cc)) = self.cards.iter_mut().find(|c| c.id() == card_id) {
            if let Some(t) = cc
                .tool_turns
                .iter_mut()
                .find(|t| t.tool_use_id == tool_use_id)
            {
                t.state = ToolState::Completed { output, is_error };
                self.card_heights.remove(&card_id);
            }
        }
    }

    pub fn set_tool_denied(&mut self, card_id: CardId, tool_use_id: &str) {
        if let Some(Card::Chat(cc)) = self.cards.iter_mut().find(|c| c.id() == card_id) {
            if let Some(t) = cc
                .tool_turns
                .iter_mut()
                .find(|t| t.tool_use_id == tool_use_id)
            {
                t.state = ToolState::Denied;
                self.card_heights.remove(&card_id);
            }
        }
    }

    pub fn card_summary(&self, card_id: CardId) -> Option<String> {
        self.cards
            .iter()
            .find(|c| c.id() == card_id)
            .map(|c| match c {
                Card::Shell(sc) => {
                    let out = sc.output_text();
                    let truncated = truncate_str(&out, 500);
                    format!("[Shell] $ {}\n{}", sc.command, truncated)
                }
                Card::Chat(cc) => {
                    let truncated = truncate_str(&cc.response, 500);
                    format!("[Chat] User: {}\nAI: {}", cc.prompt, truncated)
                }
                Card::System(sc) => format!("[System] {}", sc.message),
            })
    }

    pub fn show(&mut self, ui: &mut Ui) -> TimelineAction {
        let mut action = TimelineAction::None;
        let scroll = egui::ScrollArea::vertical()
            .id_salt("timeline_main_scroll")
            .auto_shrink([false; 2])
            .stick_to_bottom(self.auto_scroll);

        scroll.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(4.0);

            let viewport_top = ui.clip_rect().min.y;
            let viewport_bottom = ui.clip_rect().max.y;
            let md_cache = &mut self.md_cache;
            let card_heights = &mut self.card_heights;
            let spacing = 6.0;

            for card in &mut self.cards {
                let card_id = card.id();
                let cursor_y = ui.cursor().min.y;
                let estimated_h = card_heights.get(&card_id).copied().unwrap_or(60.0);

                // Skip rendering cards that are entirely above the viewport
                if cursor_y + estimated_h + spacing < viewport_top {
                    if let Some(&h) = card_heights.get(&card_id) {
                        ui.allocate_space(Vec2::new(ui.available_width(), h + spacing));
                        continue;
                    }
                }

                // Stop full rendering for cards below the viewport (but still allocate)
                if cursor_y > viewport_bottom + 100.0 {
                    if let Some(&h) = card_heights.get(&card_id) {
                        ui.allocate_space(Vec2::new(ui.available_width(), h + spacing));
                        continue;
                    }
                }

                let before_y = ui.cursor().min.y;
                // Each card gets its own ID scope to prevent widget ID collisions
                let card_action = ui.push_id(card_id, |ui| match card {
                    Card::Shell(sc) => render_shell_card(ui, sc),
                    Card::Chat(cc) => render_chat_card(ui, cc, md_cache),
                    Card::System(sys) => {
                        render_system_card(ui, sys);
                        TimelineAction::None
                    }
                }).inner;
                let after_y = ui.cursor().min.y;
                let rendered_h = after_y - before_y;
                if rendered_h > 0.0 {
                    card_heights.insert(card_id, rendered_h);
                }

                if !matches!(card_action, TimelineAction::None) {
                    action = card_action;
                }
                ui.add_space(spacing);
            }
        });

        action
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn render_shell_card(ui: &mut Ui, card: &mut ShellCard) -> TimelineAction {
    let mut action = TimelineAction::None;
    let (frame_color, left_accent) = if card.running {
        (t::SHELL_RUNNING_BG(), t::ACCENT())
    } else if card.exit_code == Some(0) {
        (t::SHELL_SUCCESS_BG(), t::SUCCESS())
    } else if card.exit_code.is_some() {
        (t::SHELL_ERROR_BG(), t::ERROR())
    } else {
        (t::SHELL_RUNNING_BG(), t::FG_MUTED())
    };

    egui::Frame::new()
        .fill(frame_color)
        .corner_radius(t::CARD_CORNER_RADIUS)
        .inner_margin(t::CARD_INNER_MARGIN)
        .stroke(Stroke::new(1.0, t::alpha(left_accent, 40)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("$")
                        .monospace()
                        .color(t::SUCCESS())
                        .size(13.0),
                );
                ui.label(RichText::new(&card.command).monospace().strong().size(13.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if card.running {
                        ui.spinner();
                    } else if let Some(code) = card.exit_code {
                        if code == 0 {
                            ui.label(RichText::new("✓").color(t::SUCCESS()).size(13.0));
                        } else {
                            ui.label(
                                RichText::new(format!("✗ {code}"))
                                    .color(t::ERROR())
                                    .size(13.0),
                            );
                        }
                    }

                    let collapse_label = if card.collapsed { "▶" } else { "▼" };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(collapse_label).color(t::FG_DIM()).size(11.0),
                            )
                            .fill(egui::Color32::TRANSPARENT),
                        )
                        .clicked()
                    {
                        card.collapsed = !card.collapsed;
                    }
                });
            });

            if !card.collapsed && !card.output_lines.is_empty() {
                ui.add_space(4.0);
                ui.painter().hline(
                    ui.available_rect_before_wrap().x_range(),
                    ui.cursor().top(),
                    Stroke::new(0.5, t::BORDER_SUBTLE()),
                );
                ui.add_space(4.0);

                if card.output_lines.len() > MAX_VISIBLE_OUTPUT_LINES {
                    ui.label(
                        RichText::new(format!(
                            "… {} lines hidden …",
                            card.output_lines.len() - MAX_VISIBLE_OUTPUT_LINES
                        ))
                        .small()
                        .color(t::FG_DIM()),
                    );
                }

                egui::ScrollArea::vertical()
                    .id_salt("shell_output")
                    .max_height(300.0)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let start = card
                            .output_lines
                            .len()
                            .saturating_sub(MAX_VISIBLE_OUTPUT_LINES);
                        let styled_available = !card.styled_output.is_empty();
                        for i in start..card.output_lines.len() {
                            if styled_available {
                                if let Some(styled_line) = card.styled_output.get(i) {
                                    ui.label(styled_line_to_layout(styled_line));
                                    continue;
                                }
                            }
                            ui.label(
                                RichText::new(&card.output_lines[i])
                                    .monospace()
                                    .size(12.0)
                                    .color(t::FG_SOFT()),
                            );
                        }
                    });
            }

            if !card.running {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("@AI").size(10.0).color(t::ACCENT_LIGHT()),
                                )
                                .fill(t::alpha(t::ACCENT(), 20))
                                .corner_radius(t::BUTTON_CORNER_RADIUS),
                            )
                            .on_hover_text("Reference this output in an AI chat")
                            .clicked()
                        {
                            action = TimelineAction::AddRef { card_id: card.id };
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Copy").size(10.0).color(t::FG_DIM()),
                                )
                                .fill(t::SURFACE())
                                .corner_radius(t::BUTTON_CORNER_RADIUS),
                            )
                            .clicked()
                        {
                            ui.ctx().copy_text(card.output_text());
                        }
                    });
                });
            }
        });

    action
}

fn render_chat_card(
    ui: &mut Ui,
    card: &ChatCard,
    md_cache: &mut CommonMarkCache,
) -> TimelineAction {
    let mut action = TimelineAction::None;
    let card_id = card.id;

    egui::Frame::new()
        .fill(t::CHAT_BG())
        .corner_radius(t::CARD_CORNER_RADIUS)
        .inner_margin(t::CARD_INNER_MARGIN)
        .stroke(Stroke::new(1.0, t::alpha(t::ACCENT(), 30)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("User").size(12.0).strong().color(t::FG()));
            });
            ui.label(RichText::new(&card.prompt).size(13.0).color(t::FG_SOFT()));

            if !card.context_refs.is_empty() {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    for r in &card.context_refs {
                        egui::Frame::new()
                            .fill(t::alpha(t::ACCENT(), 20))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("@#{r}"))
                                        .small()
                                        .monospace()
                                        .color(t::ACCENT_LIGHT()),
                                );
                            });
                    }
                });
            }

            if !card.response.is_empty() || card.streaming || !card.tool_turns.is_empty() {
                ui.add_space(6.0);
                ui.painter().hline(
                    ui.available_rect_before_wrap().x_range(),
                    ui.cursor().top(),
                    Stroke::new(0.5, t::BORDER_SUBTLE()),
                );
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Aineer")
                            .size(12.0)
                            .strong()
                            .color(t::ACCENT_LIGHT()),
                    );
                    if card.streaming {
                        ui.spinner();
                    }
                });

                if !card.response.is_empty() {
                    CommonMarkViewer::new().show(ui, md_cache, &card.response);
                }

                for (idx, turn) in card.tool_turns.iter().enumerate() {
                    let tool_action = render_tool_turn(ui, card_id, turn, idx);
                    if !matches!(tool_action, TimelineAction::None) {
                        action = tool_action;
                    }
                }
            }
        });

    action
}

fn render_tool_turn(ui: &mut Ui, card_id: CardId, turn: &ToolTurn, idx: usize) -> TimelineAction {
    let mut action = TimelineAction::None;

    ui.add_space(4.0);
    // Unique ID per tool turn to avoid widget collisions across multiple turns
    ui.push_id(idx, |ui| {
    egui::Frame::new()
        .fill(t::alpha(t::SURFACE(), 180))
        .corner_radius(6.0)
        .inner_margin(8.0)
        .stroke(Stroke::new(0.5, t::BORDER_SUBTLE()))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            let (icon, icon_color) = match &turn.state {
                ToolState::Pending => ("⏳", t::AMBER()),
                ToolState::Running => ("⚙", t::ACCENT_LIGHT()),
                ToolState::Completed { is_error, .. } => {
                    if *is_error {
                        ("✗", t::ERROR())
                    } else {
                        ("✓", t::SUCCESS())
                    }
                }
                ToolState::Denied => ("⊘", t::FG_DIM()),
            };

            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(12.0).color(icon_color));
                ui.label(
                    RichText::new(&turn.name)
                        .monospace()
                        .size(12.0)
                        .strong()
                        .color(t::FG()),
                );
                if matches!(turn.state, ToolState::Running) {
                    ui.spinner();
                }
            });

            if !turn.input.is_empty() {
                let input_preview = truncate_str(&turn.input, 200);
                ui.label(
                    RichText::new(input_preview)
                        .monospace()
                        .size(11.0)
                        .color(t::FG_DIM()),
                );
            }

            match &turn.state {
                ToolState::Pending => {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("✓ Allow").size(11.0).color(t::SUCCESS()),
                                )
                                .fill(t::blend(t::SURFACE(), t::SUCCESS(), 0.1))
                                .corner_radius(t::BUTTON_CORNER_RADIUS),
                            )
                            .clicked()
                        {
                            action = TimelineAction::ToolApprove {
                                card_id,
                                tool_use_id: turn.tool_use_id.clone(),
                            };
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("✗ Deny").size(11.0).color(t::ERROR()),
                                )
                                .fill(t::blend(t::SURFACE(), t::ERROR(), 0.1))
                                .corner_radius(t::BUTTON_CORNER_RADIUS),
                            )
                            .clicked()
                        {
                            action = TimelineAction::ToolDeny {
                                card_id,
                                tool_use_id: turn.tool_use_id.clone(),
                            };
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("⚡ Allow All").size(11.0).color(t::AMBER()),
                                )
                                .fill(t::blend(t::SURFACE(), t::AMBER(), 0.08))
                                .corner_radius(t::BUTTON_CORNER_RADIUS),
                            )
                            .clicked()
                        {
                            action = TimelineAction::ToolApproveAll { card_id };
                        }
                    });
                }
                ToolState::Completed { output, .. } if !output.is_empty() => {
                    let out_preview = truncate_str(output, 300);
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(out_preview)
                            .monospace()
                            .size(11.0)
                            .color(t::FG_SOFT()),
                    );
                }
                ToolState::Denied => {
                    ui.label(
                        RichText::new("Denied by user")
                            .italics()
                            .size(11.0)
                            .color(t::FG_DIM()),
                    );
                }
                _ => {}
            }
        });
    }); // push_id

    action
}

fn styled_line_to_layout(line: &OutputLine) -> LayoutJob {
    let mono = FontId::monospace(12.0);
    let mono_bold = FontId::new(12.0, egui::FontFamily::Monospace);
    let mut job = LayoutJob::default();
    for seg in &line.segments {
        let mut fmt = TextFormat {
            font_id: if seg.bold {
                mono_bold.clone()
            } else {
                mono.clone()
            },
            color: seg.fg,
            ..Default::default()
        };
        if seg.bold {
            fmt.extra_letter_spacing = 0.3;
        }
        job.append(&seg.text, 0.0, fmt);
    }
    if job.text.is_empty() {
        job.append(
            " ",
            0.0,
            TextFormat {
                font_id: mono,
                ..Default::default()
            },
        );
    }
    job
}

fn render_system_card(ui: &mut Ui, card: &SystemCard) {
    egui::Frame::new()
        .fill(t::SYSTEM_BG())
        .corner_radius(t::CARD_CORNER_RADIUS)
        .inner_margin(egui::Margin::symmetric(t::CARD_INNER_MARGIN as i8, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(&card.message)
                    .italics()
                    .size(12.0)
                    .color(t::FG_DIM()),
            );
        });
}
