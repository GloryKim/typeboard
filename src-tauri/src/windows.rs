use std::sync::atomic::{AtomicU32, Ordering};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle, Emitter, Manager, TitleBarStyle, WebviewUrl, WebviewWindowBuilder,
};

static WINDOW_SEQ: AtomicU32 = AtomicU32::new(1);

pub fn open_window(app: &AppHandle) -> Result<(), String> {
    let id = WINDOW_SEQ.fetch_add(1, Ordering::Relaxed);
    let label = format!("term-{id}");
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("typeboard")
        .inner_size(980.0, 640.0)
        .min_inner_size(400.0, 220.0)
        .resizable(true)
        .decorations(true)
        .focused(true)
        .theme(Some(tauri::Theme::Dark))
        .background_color(tauri::window::Color(0x1e, 0x1e, 0x2e, 0xff));

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .hidden_title(true)
            .title_bar_style(TitleBarStyle::Overlay)
            .traffic_light_position(tauri::LogicalPosition::new(14.0, 12.0));
    }

    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn emit_to_focused(app: &AppHandle, event: &str) {
    let windows = app.webview_windows();
    if let Some((_, win)) = windows.iter().find(|(_, w)| w.is_focused().unwrap_or(false)) {
        let _ = win.emit(event, ());
        return;
    }
    if let Some((_, win)) = windows.iter().next() {
        let _ = win.emit(event, ());
    }
}

pub fn emit_new_tab(app: &AppHandle) {
    emit_to_focused(app, "terminal://new-tab");
}

pub fn close_focused_window(app: &AppHandle) {
    let windows = app.webview_windows();
    if let Some((_, win)) = windows.iter().find(|(_, w)| w.is_focused().unwrap_or(false)) {
        let _ = win.close();
        return;
    }
}

#[tauri::command]
pub fn new_window(app: AppHandle) -> Result<(), String> {
    open_window(&app)
}

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let new_tab = MenuItem::with_id(app, "new-tab", "New Tab", true, Some("CmdOrCtrl+T"))?;
    let new_window = MenuItem::with_id(app, "new-window", "New Window", true, Some("CmdOrCtrl+N"))?;
    let new_window_in_window_menu =
        MenuItem::with_id(app, "new-window", "New Window", true, Some("CmdOrCtrl+N"))?;
    let close_tab = MenuItem::with_id(app, "close-tab", "Close Tab", true, Some("CmdOrCtrl+W"))?;
    let close_window =
        MenuItem::with_id(app, "close-window", "Close Window", true, Some("CmdOrCtrl+Shift+W"))?;
    let close_window_in_window_menu =
        MenuItem::with_id(app, "close-window", "Close Window", true, Some("CmdOrCtrl+Shift+W"))?;

    let pkg = app.package_info();
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &new_tab,
            &new_window,
            &PredefinedMenuItem::separator(app)?,
            &close_tab,
            &close_window,
            #[cfg(not(target_os = "macos"))]
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let window = Submenu::with_id_and_items(
        app,
        tauri::menu::WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &new_window_in_window_menu,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &close_window_in_window_menu,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let app_menu = Submenu::with_items(
        app,
        pkg.name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let zoom_in = MenuItem::with_id(app, "zoom-in", "Zoom In", true, Some("CmdOrCtrl+="))?;
    let zoom_out = MenuItem::with_id(app, "zoom-out", "Zoom Out", true, Some("CmdOrCtrl+-"))?;
    let zoom_reset =
        MenuItem::with_id(app, "zoom-reset", "Actual Size", true, Some("CmdOrCtrl+0"))?;

    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &zoom_in,
            &zoom_out,
            &zoom_reset,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::separator(app)?,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;

    #[cfg(target_os = "macos")]
    {
        Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window])
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pkg;
        Menu::with_items(app, &[&file, &edit, &view, &window])
    }
}
