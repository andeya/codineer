mod advanced;
mod bar;
mod gateway;
mod general;
mod json_editor;
mod model;
mod permissions;
mod plugins;
mod shell;

use std::path::PathBuf;

use egui::{RichText, Ui};

use crate::icons;
use crate::theme::{self as t, font_size, radius, spacing};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Shell,
    Model,
    Gateway,
    Permissions,
    Advanced,
    Json,
    Plugins,
}

impl SettingsTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Shell => "Shell",
            Self::Model => "Model",
            Self::Gateway => "Gateway",
            Self::Permissions => "Permissions",
            Self::Advanced => "Advanced",
            Self::Json => "JSON",
            Self::Plugins => "Plugins",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::General => icons::BEHAVIOR,
            Self::Shell => icons::SHELL,
            Self::Model => icons::MODEL,
            Self::Gateway => icons::GATEWAY,
            Self::Permissions => icons::PERMISSIONS,
            Self::Advanced => icons::ADVANCED,
            Self::Json => icons::JSON,
            Self::Plugins => icons::PLUGIN,
        }
    }

    pub fn all() -> &'static [SettingsTab] {
        &[
            Self::General,
            Self::Shell,
            Self::Model,
            Self::Gateway,
            Self::Permissions,
            Self::Advanced,
            Self::Json,
            Self::Plugins,
        ]
    }
}

pub struct SettingsPanel {
    pub open: bool,
    active_tab: SettingsTab,
    pub draft: SettingsDraft,
    dirty: bool,
    status_msg: Option<(String, bool)>,
    json_raw: String,
    json_sync_needed: bool,
    search_query: String,
}

/// Mutable draft of all settings being edited.
/// On Save, this gets applied to the runtime config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDraft {
    // General
    pub theme: String,
    pub font_size: f32,
    pub language: String,
    pub session_restore: bool,

    // Shell
    pub shell_path: String,
    pub shell_args: String,
    pub env_vars: Vec<(String, String)>,

    // Model
    pub default_model: String,
    pub fallback_models: String,
    pub thinking_mode: bool,

    // Gateway
    pub gateway_enabled: bool,
    pub gateway_addr: String,

    // Permissions
    pub default_permission_mode: String,
    pub permission_rules: Vec<(String, String)>,

    // Advanced
    pub sandbox_enabled: bool,
    pub auto_compact: bool,
    pub max_context_tokens: u32,
}

impl Default for SettingsDraft {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14.0,
            language: "en".to_string(),
            session_restore: false,

            shell_path: detect_default_shell(),
            shell_args: String::new(),
            env_vars: Vec::new(),

            default_model: "auto".to_string(),
            fallback_models: String::new(),
            thinking_mode: false,

            gateway_enabled: true,
            gateway_addr: "127.0.0.1:8090".to_string(),

            default_permission_mode: "ask".to_string(),
            permission_rules: Vec::new(),

            sandbox_enabled: false,
            auto_compact: true,
            max_context_tokens: 200_000,
        }
    }
}

fn detect_default_shell() -> String {
    #[cfg(unix)]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(any(unix, windows)))]
    {
        "sh".to_string()
    }
}

fn settings_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".aineer")
}

fn settings_file() -> PathBuf {
    settings_dir().join("settings.json")
}

pub fn save_settings(draft: &SettingsDraft) -> Result<(), String> {
    let dir = settings_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create settings dir: {e}"))?;
    let json =
        serde_json::to_string_pretty(draft).map_err(|e| format!("Failed to serialize: {e}"))?;
    std::fs::write(settings_file(), json).map_err(|e| format!("Failed to write settings: {e}"))
}

pub fn load_settings() -> Option<SettingsDraft> {
    let content = std::fs::read_to_string(settings_file()).ok()?;
    serde_json::from_str(&content).ok()
}

impl SettingsPanel {
    pub fn new() -> Self {
        let draft = load_settings().unwrap_or_default();
        let json_raw = serde_json::to_string_pretty(&draft).unwrap_or_else(|_| "{}".to_string());
        Self {
            open: false,
            active_tab: SettingsTab::General,
            draft,
            dirty: false,
            status_msg: None,
            json_raw,
            json_sync_needed: false,
            search_query: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn show(&mut self, ui: &mut Ui) {
        // Search bar
        ui.add_space(spacing::SM);
        egui::Frame::new()
            .fill(t::INPUT_BG())
            .corner_radius(radius::LG)
            .inner_margin(egui::Margin::symmetric(
                spacing::MD as i8,
                spacing::XS as i8,
            ))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(icons::SEARCH)
                            .size(font_size::BODY)
                            .color(t::FG_MUTED()),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .desired_width(ui.available_width())
                            .hint_text("Search settings...")
                            .frame(false)
                            .font(egui::FontId::proportional(font_size::BODY)),
                    );
                });
            });

        ui.add_space(spacing::MD);

        // Tab navigation with icons
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = spacing::XXS;
            for tab in SettingsTab::all() {
                let is_active = self.active_tab == *tab;
                let label = format!("{} {}", tab.icon(), tab.label());

                let text = RichText::new(label)
                    .size(font_size::SMALL)
                    .color(if is_active {
                        t::ACCENT_LIGHT()
                    } else {
                        t::FG_DIM()
                    });

                let fill = if is_active {
                    t::alpha(t::ACCENT(), 20)
                } else {
                    egui::Color32::TRANSPARENT
                };

                let stroke = if is_active {
                    egui::Stroke::new(1.0, t::alpha(t::ACCENT(), 80))
                } else {
                    egui::Stroke::NONE
                };

                let btn = egui::Button::new(if is_active { text.strong() } else { text })
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(radius::MD);

                if ui.add(btn).clicked() {
                    self.active_tab = *tab;
                }
            }
        });

        ui.add_space(spacing::XS);

        // Sync JSON text when switching to JSON tab
        if self.active_tab == SettingsTab::Json && self.json_sync_needed {
            self.json_raw =
                serde_json::to_string_pretty(&self.draft).unwrap_or_else(|_| "{}".to_string());
            self.json_sync_needed = false;
        }

        egui::ScrollArea::vertical()
            .id_salt("settings_panel_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let changed = match self.active_tab {
                    SettingsTab::General => general::show(ui, &mut self.draft),
                    SettingsTab::Shell => shell::show(ui, &mut self.draft),
                    SettingsTab::Model => model::show(ui, &mut self.draft),
                    SettingsTab::Gateway => gateway::show(ui, &mut self.draft),
                    SettingsTab::Permissions => permissions::show(ui, &mut self.draft),
                    SettingsTab::Advanced => advanced::show(ui, &mut self.draft),
                    SettingsTab::Json => json_editor::show(ui, &mut self.json_raw, &mut self.draft),
                    SettingsTab::Plugins => plugins::show(ui),
                };
                if changed {
                    self.dirty = true;
                    self.json_sync_needed = true;
                }
            });

        ui.add_space(spacing::SM);
        bar::show(ui, &mut self.dirty, &mut self.status_msg, &mut self.draft);
    }
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self::new()
    }
}
