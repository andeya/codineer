use aineer_release_channel::ReleaseChannel;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, WebviewWindowBuilder,
};

/// Build and register the system-tray icon with Show / About / Quit actions.
///
/// Platform notes:
/// - **macOS**: Uses a dedicated 22pt (44px @2x) colored tray icon so it
///   renders crisply in the menu bar. `icon_as_template(false)` preserves the
///   original brand colors. Left-click shows the window; right-click opens the menu.
/// - **Windows**: Tray icon gets a stable `id` so that Windows correctly tracks
///   it across restarts. Left-click shows the window; right-click opens the menu.
/// - **Linux**: Behaviour depends on the DE's `libappindicator` / `StatusNotifier`
///   support. The menu is always available; left-click also shows the window.
pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let channel = ReleaseChannel::current();
    let display = channel.display_name();

    let show = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
    let about = MenuItemBuilder::with_id("about", format!("About {display}")).build(app)?;
    let quit_label = format!("Quit {display}");
    let quit = MenuItemBuilder::with_id("quit", &quit_label).build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&about)
        .separator()
        .item(&quit)
        .build()?;

    #[cfg(target_os = "macos")]
    let icon = {
        let png = include_bytes!("../icons/tray-icon@2x.png");
        tauri::image::Image::from_bytes(png)?
    };
    #[cfg(not(target_os = "macos"))]
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default window icon must be set in tauri.conf.json bundle.icon")?;

    let _tray = TrayIconBuilder::with_id("aineer-tray")
        .icon(icon)
        .icon_as_template(false)
        .tooltip(crate::version::display_title())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "about" => show_about_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Show (or restore) the main window.
///
/// Handles all three platform behaviours:
/// - `show()` — makes hidden window visible
/// - `unminimize()` — restores from taskbar/dock minimize
/// - `set_focus()` — brings to front
pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Open a custom About window (or focus the existing one).
///
/// Replaces the native macOS "About" dialog with a WebView window
/// that renders `index.html#about`.
pub fn show_about_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("about") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }

    let title = crate::version::about_title();

    let url = tauri::WebviewUrl::App("index.html#about".into());
    match WebviewWindowBuilder::new(app, "about", url)
        .title(&title)
        .inner_size(420.0, 400.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .center()
        .build()
    {
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to create about window: {e}"),
    }
}
