use egui::{RichText, Ui};

use crate::theme::{self as t, font_size, spacing};

pub struct SettingsRow<'a> {
    label: &'a str,
    description: Option<&'a str>,
}

impl<'a> SettingsRow<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            description: None,
        }
    }

    pub fn description(mut self, desc: &'a str) -> Self {
        self.description = Some(desc);
        self
    }

    pub fn show(self, ui: &mut Ui, control: impl FnOnce(&mut Ui) -> bool) -> bool {
        // Use the label text as stable ID salt so each row has a unique
        // widget namespace — prevents collisions between sibling rows.
        ui.push_id(self.label, |ui| {
            let mut changed = false;

            ui.add_space(spacing::SM);

            ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());

                ui.vertical(|ui| {
                    ui.set_min_width(160.0);
                    ui.label(
                        RichText::new(self.label)
                            .size(font_size::BODY)
                            .color(t::FG()),
                    );
                    if let Some(desc) = self.description {
                        ui.label(
                            RichText::new(desc)
                                .size(font_size::CAPTION)
                                .color(t::FG_MUTED()),
                        );
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    changed = control(ui);
                });
            });

            ui.add_space(spacing::SM);
            ui.separator();

            changed
        })
        .inner
    }
}
