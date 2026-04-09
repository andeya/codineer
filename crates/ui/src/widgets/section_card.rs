use egui::{RichText, Ui};

use crate::theme::{self as t, font_size, spacing};

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
        ui.add_space(spacing::LG);
        ui.push_id(self.title, |ui| {
            // Section header
            ui.horizontal(|ui| {
                if let Some(icon) = self.icon {
                    ui.label(
                        RichText::new(icon)
                            .size(font_size::BODY)
                            .color(t::ACCENT()),
                    );
                }
                ui.label(
                    RichText::new(self.title)
                        .size(font_size::BODY)
                        .strong()
                        .color(t::FG()),
                );
            });

            if let Some(desc) = self.description {
                ui.label(
                    RichText::new(desc)
                        .size(font_size::CAPTION)
                        .color(t::FG_DIM()),
                );
            }

            ui.add_space(spacing::SM);

            // Content area — single lightweight frame
            ui.push_id("content", |ui| {
                ui.set_min_width(ui.available_width());
                content(ui);
            });
        });
    }
}
