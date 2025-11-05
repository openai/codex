use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    AppHandle, Manager, Runtime,
};
use tracing::info;

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "📊 Dashboardを開く", true, None::<&str>)?;
    let separator1 = MenuItem::with_id(app, "sep1", "────────────────", false, None::<&str>)?;
    let watcher_status = MenuItem::with_id(
        app,
        "watcher_status",
        "✅ ファイル監視: ON",
        false,
        None::<&str>,
    )?;
    let core_status = MenuItem::with_id(
        app,
        "core_status",
        "🔄 Codex Core: 起動中",
        false,
        None::<&str>,
    )?;
    let separator2 = MenuItem::with_id(app, "sep2", "────────────────", false, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "⚙️ Settings", true, None::<&str>)?;
    let docs_item = MenuItem::with_id(app, "docs", "📖 Docs", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "❌ Quit", true, None::<&str>)?;

    // Build menu
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &separator1,
            &watcher_status,
            &core_status,
            &separator2,
            &settings_item,
            &docs_item,
            &quit_item,
        ],
    )?;

    // Create tray icon
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                info!("Showing main window");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "settings" => {
                info!("Opening settings");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    // TODO: Navigate to settings page via event (Tauri 2.0 API update needed)
                    // In v1.3.0: Implement proper navigation
                }
            }
            "docs" => {
                info!("Opening documentation");
                // TODO v1.3.0: Use tauri-plugin-opener instead of deprecated shell.open
                #[allow(deprecated)]
                let _ = tauri_plugin_shell::ShellExt::shell(app)
                    .open("https://github.com/zapabob/codex", None);
            }
            "quit" => {
                info!("Quitting application");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    info!("System tray created successfully");
    Ok(())
}

/// Update tray menu item status
#[allow(dead_code)]
pub fn update_tray_status<R: Runtime>(
    _app: &AppHandle<R>,
    _watcher_running: bool,
    _core_running: bool,
) -> tauri::Result<()> {
    // Note: Updating menu items dynamically requires holding a reference to the menu
    // This can be implemented with app state if needed
    Ok(())
}

