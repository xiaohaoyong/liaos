// 托盘模块：托盘图标 + 右键菜单 + 左键唤起主窗口
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
    let create_i = MenuItem::with_id(app, "create", "新建待办", true, None::<&str>)?;
    let sync_i = MenuItem::with_id(app, "sync", "立即同步", true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let toggle_sticky_i = MenuItem::with_id(app, "toggle-sticky", "显示/隐藏便签和资讯", true, None::<&str>)?;
    let new_note_i = MenuItem::with_id(app, "new-note", "新建便利贴", true, None::<&str>)?;
    let hosts_i = MenuItem::with_id(app, "hosts", "Hosts 编辑器", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&open_i, &create_i, &sync_i, &settings_i, &toggle_sticky_i, &new_note_i, &hosts_i, &quit_i],
    )?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false);

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "create" => {
                show_main_window(app);
                let _ = app.emit("tray-create", ());
            }
            "sync" => {
                show_main_window(app);
                let _ = app.emit("tray-sync", ());
            }
            "settings" => {
                show_main_window(app);
                let _ = app.emit("tray-settings", ());
            }
            "toggle-sticky" => crate::sticky::toggle_sticky(app),
            "new-note" => {
                if let Err(e) = crate::notes::create_note_internal(app) {
                    eprintln!("新建便利贴失败: {e}");
                }
            }
            "hosts" => crate::hosts::open_hosts_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
