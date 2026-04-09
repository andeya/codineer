use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

const FONT_INTER: &str = "Inter";
const FONT_INTER_BYTES: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");

const FONT_JETBRAINS: &str = "JetBrains Mono";
const FONT_JETBRAINS_BYTES: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

const FONT_CJK: &str = "LXGW WenKai Mono";
const FONT_CJK_BYTES: &[u8] = include_bytes!("../assets/fonts/LXGWWenKaiMonoTC-Regular.ttf");

const FONT_SYMBOLS: &str = "Noto Symbols";
const FONT_SYMBOLS_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf");

pub fn setup(ctx: &egui::Context, mode: ui::theme::ThemeMode) {
    setup_fonts(ctx);
    ui::theme::apply(ctx, mode);
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    insert_font(&mut fonts, FONT_INTER, FONT_INTER_BYTES);
    insert_font(&mut fonts, FONT_JETBRAINS, FONT_JETBRAINS_BYTES);
    insert_font(&mut fonts, FONT_CJK, FONT_CJK_BYTES);
    insert_font(&mut fonts, FONT_SYMBOLS, FONT_SYMBOLS_BYTES);

    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, FONT_SYMBOLS.to_owned());
    prop.insert(0, FONT_CJK.to_owned());
    prop.insert(0, FONT_INTER.to_owned());

    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    mono.insert(0, FONT_SYMBOLS.to_owned());
    mono.insert(0, FONT_CJK.to_owned());
    mono.insert(0, FONT_JETBRAINS.to_owned());

    ctx.set_fonts(fonts);
}

fn insert_font(defs: &mut FontDefinitions, name: &str, data: &'static [u8]) {
    defs.font_data
        .insert(name.to_owned(), Arc::new(FontData::from_static(data)));
}

pub fn aineer_terminal_theme() -> terminal::TerminalTheme {
    use ui::theme;
    // Use the same colour as CentralPanel so the terminal blends in
    // without a visible "shell frame" border effect.
    terminal::TerminalTheme::new(Box::new(terminal::ColorPalette {
        background: format!(
            "#{:02X}{:02X}{:02X}",
            theme::BG().r(),
            theme::BG().g(),
            theme::BG().b()
        ),
        foreground: format!(
            "#{:02X}{:02X}{:02X}",
            theme::FG().r(),
            theme::FG().g(),
            theme::FG().b()
        ),
        black: String::from("#1A1A2E"),
        red: String::from("#F38BA8"),
        green: String::from("#4ECB71"),
        yellow: String::from("#E9BE6D"),
        blue: String::from("#6C9BF5"),
        magenta: String::from("#C084FC"),
        cyan: String::from("#06B6D4"),
        white: String::from("#E0E4EF"),
        bright_black: String::from("#505070"),
        bright_red: String::from("#F2828D"),
        bright_green: String::from("#7DDA93"),
        bright_yellow: String::from("#F4CF85"),
        bright_blue: String::from("#93BBFF"),
        bright_magenta: String::from("#E0B2F7"),
        bright_cyan: String::from("#67E8F9"),
        bright_white: String::from("#FFFFFF"),
        ..Default::default()
    }))
}
