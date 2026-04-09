use egui::{RichText, Ui};

use crate::theme::{self as t, font_size, radius, spacing};

pub struct SectionCard<'a> {
    title: &'a str,
    icon: Option<&'a str>,
    description: Option<&'a str>,
}

impl<'a> SectionCard<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            icon: None,
            description: None,
        }
    }

    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn description(mut self, desc: &'a str) -> Self {
        self.description = Some(desc);
        self
    }

    pub fn show(self, ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
        ui.add_space(spacing::MD);

        // Use the title as a stable ID salt so each SectionCard has its
        // own widget ID namespace — prevents collisions between siblings.
        ui.push_id(self.title, |ui| {
            egui::Frame::new()
                .fill(t::PANEL_BG())
                .corner_radius(radius::XL)
                .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                .inner_margin(egui::Margin::same(spacing::XL as i8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    ui.horizontal(|ui| {
                        if let Some(icon) = self.icon {
                            ui.label(
                                RichText::new(icon)
                                    .size(font_size::TITLE)
                                    .color(t::ACCENT_LIGHT()),
                            );
                            ui.add_space(spacing::SM);
                        }
                        ui.label(
                            RichText::new(self.title)
                                .size(font_size::SUBTITLE)
                                .strong()
                                .color(t::FG()),
                        );
                    });

                    if let Some(desc) = self.description {
                        ui.add_space(spacing::XXS);
                        ui.label(
                            RichText::new(desc)
                                .size(font_size::SMALL)
                                .color(t::FG_DIM()),
                        );
                    }

                    ui.add_space(spacing::LG);

                    // Inner content frame: use "content" as a sub-ID
                    ui.push_id("content", |ui| {
                        egui::Frame::new()
                            .fill(t::SURFACE())
                            .corner_radius(radius::LG)
                            .inner_margin(egui::Margin::same(spacing::LG as i8))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                content(ui);
                            });
                    });
                });
        });
    }
}
