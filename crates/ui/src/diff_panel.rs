use std::collections::HashSet;
use std::sync::Arc;

use egui::{RichText, Ui};

use crate::git_diff::{DiffLineKind, FileStatus, GitStatus};
use crate::theme as t;

pub struct DiffPanel {
    pub visible: bool,
    status: Option<Arc<GitStatus>>,
    expanded_files: HashSet<String>,
    reverted_hunks: HashSet<(String, usize)>,
}

pub enum DiffAction {
    None,
    RevertHunk { file: String, hunk_idx: usize },
}

impl DiffPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            status: None,
            expanded_files: HashSet::new(),
            reverted_hunks: HashSet::new(),
        }
    }

    pub fn update_status(&mut self, status: Arc<GitStatus>) {
        self.reverted_hunks.clear();
        self.status = Some(status);
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn show(&mut self, ui: &mut Ui) -> DiffAction {
        let mut action = DiffAction::None;

        let status = match &self.status {
            Some(s) => s.clone(),
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("No git repository detected")
                            .color(t::FG_DIM())
                            .size(13.0),
                    );
                });
                return action;
            }
        };

        ui.horizontal(|ui| {
            if let Some(ref branch) = status.branch {
                egui::Frame::new()
                    .fill(t::SURFACE())
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("⎇ {branch}"))
                                .strong()
                                .monospace()
                                .size(12.0)
                                .color(t::FG_SOFT()),
                        );
                    });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "+{} −{}",
                        status.total_insertions, status.total_deletions
                    ))
                    .size(11.0)
                    .color(t::FG_DIM()),
                );
                egui::Frame::new()
                    .fill(t::alpha(t::ACCENT(), 18))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(4, 1))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} files", status.file_count()))
                                .size(11.0)
                                .color(t::ACCENT_LIGHT()),
                        );
                    });
            });
        });

        ui.add_space(4.0);
        ui.separator();

        if status.changes.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(
                    RichText::new("Working tree clean")
                        .color(t::SUCCESS())
                        .size(13.0),
                );
            });
            return action;
        }

        egui::ScrollArea::vertical()
            .id_salt("diff_panel_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for change in &status.changes {
                    let (icon, icon_color) = match change.status {
                        FileStatus::Modified => ("M", t::WARNING()),
                        FileStatus::Added => ("A", t::SUCCESS()),
                        FileStatus::Deleted => ("D", t::ERROR()),
                        FileStatus::Renamed => ("R", t::ACCENT_CYAN()),
                    };

                    let short_name = std::path::Path::new(&change.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&change.path);

                    let expanded = self.expanded_files.contains(&change.path);
                    let header_id = ui.make_persistent_id(&change.path);

                    let resp = ui.horizontal(|ui| {
                        egui::Frame::new()
                            .fill(t::alpha(icon_color, 20))
                            .corner_radius(3.0)
                            .inner_margin(egui::Margin::symmetric(3, 0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(icon).color(icon_color).monospace().size(11.0),
                                );
                            });

                        let arrow = if expanded { "▼" } else { "▶" };
                        let resp = ui.selectable_label(
                            expanded,
                            RichText::new(format!("{arrow} {short_name}"))
                                .monospace()
                                .size(12.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if change.deletions > 0 {
                                ui.label(
                                    RichText::new(format!("−{}", change.deletions))
                                        .color(t::DIFF_DEL_FG())
                                        .size(11.0),
                                );
                            }
                            if change.insertions > 0 {
                                ui.label(
                                    RichText::new(format!("+{}", change.insertions))
                                        .color(t::DIFF_ADD_FG())
                                        .size(11.0),
                                );
                            }
                        });

                        resp
                    });

                    if resp.inner.clicked() {
                        if expanded {
                            self.expanded_files.remove(&change.path);
                        } else {
                            self.expanded_files.insert(change.path.clone());
                        }
                    }

                    resp.response.on_hover_text(&change.path);

                    if expanded {
                        if let Some(file_diff) = status.diffs.get(&change.path) {
                            ui.indent(header_id, |ui| {
                                for (hunk_idx, hunk) in file_diff.hunks.iter().enumerate() {
                                    let hunk_key = (change.path.clone(), hunk_idx);
                                    let reverted = self.reverted_hunks.contains(&hunk_key);

                                    ui.label(
                                        RichText::new(&hunk.header)
                                            .color(t::DIFF_HUNK_HEADER())
                                            .monospace()
                                            .size(11.0),
                                    );

                                    if reverted {
                                        ui.label(
                                            RichText::new("  (reverted)")
                                                .color(t::FG_DIM())
                                                .italics()
                                                .size(11.0),
                                        );
                                    } else {
                                        for line in &hunk.lines {
                                            let (prefix, fg, bg) = match line.kind {
                                                DiffLineKind::Add => {
                                                    ("+", t::DIFF_ADD_FG(), t::DIFF_ADD_BG())
                                                }
                                                DiffLineKind::Delete => {
                                                    ("-", t::DIFF_DEL_FG(), t::DIFF_DEL_BG())
                                                }
                                                DiffLineKind::Context => {
                                                    (" ", t::FG_DIM(), egui::Color32::TRANSPARENT)
                                                }
                                            };

                                            let text = format!(
                                                "{prefix} {}",
                                                line.content.trim_end_matches('\n')
                                            );

                                            ui.add(egui::Label::new(
                                                RichText::new(text)
                                                    .monospace()
                                                    .size(11.0)
                                                    .color(fg)
                                                    .background_color(bg),
                                            ));
                                        }

                                        ui.horizontal(|ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("Keep")
                                                            .size(10.0)
                                                            .color(t::FG_DIM()),
                                                    )
                                                    .fill(t::SURFACE())
                                                    .corner_radius(t::BUTTON_CORNER_RADIUS),
                                                )
                                                .clicked()
                                            {
                                                // Keep = acknowledge
                                            }
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("Revert")
                                                            .size(10.0)
                                                            .color(t::ERROR()),
                                                    )
                                                    .fill(t::alpha(t::ERROR(), 15))
                                                    .corner_radius(t::BUTTON_CORNER_RADIUS),
                                                )
                                                .clicked()
                                            {
                                                self.reverted_hunks.insert(hunk_key.clone());
                                                action = DiffAction::RevertHunk {
                                                    file: change.path.clone(),
                                                    hunk_idx,
                                                };
                                            }
                                        });
                                    }

                                    ui.add_space(4.0);
                                }
                            });
                        }
                    }
                }
            });

        action
    }
}

impl Default for DiffPanel {
    fn default() -> Self {
        Self::new()
    }
}
