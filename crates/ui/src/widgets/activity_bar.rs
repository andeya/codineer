use egui::{RichText, Ui};

use crate::icons;
use crate::theme::{self as t, font_size, radius, spacing};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ActivityItem {
    Terminal,
    Diff,
    Settings,
    Ssh,
}

impl ActivityItem {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Terminal => icons::TERMINAL,
            Self::Diff => icons::SOURCE_CONTROL,
            Self::Settings => icons::SETTINGS,
            Self::Ssh => icons::SSH,
        }
    }

    pub fn tooltip(&self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Diff => "Source Control",
            Self::Settings => "Settings",
            Self::Ssh => "SSH Connections",
        }
    }
}

pub const ACTIVITY_BAR_WIDTH: f32 = 40.0;

pub struct ActivityBar {
    active: Option<ActivityItem>,
}

impl ActivityBar {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn set_active(&mut self, item: Option<ActivityItem>) {
        self.active = item;
    }

    /// Returns the item that was clicked (to toggle).
    pub fn show(&self, ui: &mut Ui) -> Option<ActivityItem> {
        let mut clicked = None;

        ui.vertical_centered(|ui| {
            ui.add_space(spacing::SM);

            let items = [
                ActivityItem::Terminal,
                ActivityItem::Diff,
                ActivityItem::Settings,
                ActivityItem::Ssh,
            ];

            for item in &items {
                let is_active = self.active == Some(*item);
                let fg = if is_active {
                    t::ACCENT_LIGHT()
                } else {
                    t::FG_DIM()
                };

                ui.add_space(spacing::XXS);

                // Each item needs its own ID scope to prevent collisions
                // between the sibling label widgets inside the frames.
                let resp = ui.push_id(*item, |ui| {
                    egui::Frame::new()
                        .fill(if is_active {
                            t::alpha(t::ACCENT(), 15)
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .corner_radius(radius::MD)
                        .inner_margin(egui::Margin::same(spacing::SM as i8))
                        .show(ui, |ui| {
                            ui.set_min_width(28.0);
                            ui.set_min_height(28.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(item.icon())
                                        .size(font_size::TITLE)
                                        .color(fg),
                                );
                            });
                        })
                        .response
                })
                .inner;

                if is_active {
                    ui.painter().vline(
                        resp.rect.left(),
                        resp.rect.y_range(),
                        egui::Stroke::new(2.0, t::ACCENT()),
                    );
                }

                if resp
                    .on_hover_text(item.tooltip())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    clicked = Some(*item);
                }

                ui.add_space(spacing::XXS);
            }
        });

        clicked
    }
}

impl Default for ActivityBar {
    fn default() -> Self {
        Self::new()
    }
}
