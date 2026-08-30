mod pty;
mod windows;

#[cfg(target_os = "macos")]
mod macos_dock;

use pty::PtyState;
use tauri::menu::MenuEvent;
use tauri::RunEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(PtyState::default())
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_close,
            windows::new_window,
        ])
        .setup(|app| {
            let menu = windows::build_menu(app.handle())?;
            app.set_menu(menu)?;
            #[cfg(target_os = "macos")]
            macos_dock::install(app.handle());
            Ok(())
        })
        .on_menu_event(|app, event| handle_menu(app, &event))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                let _ = windows::open_window(app);
            }
        });
}

fn handle_menu(app: &tauri::AppHandle, event: &MenuEvent) {
    match event.id().as_ref() {
        "new-window" => {
            let _ = windows::open_window(app);
        }
        "new-tab" => windows::emit_new_tab(app),
        "close-tab" => windows::emit_to_focused(app, "terminal://close-tab"),
        "close-window" => windows::close_focused_window(app),
        "zoom-in" => windows::emit_to_focused(app, "terminal://zoom-in"),
        "zoom-out" => windows::emit_to_focused(app, "terminal://zoom-out"),
        "zoom-reset" => windows::emit_to_focused(app, "terminal://zoom-reset"),
        _ => {}
    }
}
