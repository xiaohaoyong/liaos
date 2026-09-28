// 便签窗口模块：动态创建半透明无边框窗口 + 全局快捷键显示/隐藏切换
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// 默认快捷键（与 storage.rs 的 default_hotkey 保持一致）
const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";

/// 便签窗口的逻辑尺寸
const STICKY_WIDTH: f64 = 340.0;
const STICKY_HEIGHT: f64 = 520.0;
const STICKY_MARGIN: f64 = 20.0;

/// 计算便签窗口默认位置（屏幕右上角，留边距），返回逻辑坐标
/// （news.rs 的新闻窗口位置以此推导，保持紧贴便签左侧）
pub(crate) fn sticky_position(app: &tauri::AppHandle) -> (f64, f64) {
    match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let scale = monitor.scale_factor();
            let size = monitor.size(); // 物理像素
            let pos = monitor.position(); // 物理像素
            let sw = size.width as f64 / scale;
            let mx = pos.x as f64 / scale;
            let my = pos.y as f64 / scale;
            (mx + sw - STICKY_WIDTH - STICKY_MARGIN, my + STICKY_MARGIN)
        }
        _ => (100.0, 100.0),
    }
}

/// 创建便签窗口（启动时调用，创建后立即显示）
pub fn create_sticky_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let (x, y) = sticky_position(app);
    let window = WebviewWindowBuilder::new(app, "sticky", WebviewUrl::App("sticky.html".into()))
        .title("便签")
        .decorations(false)
        .transparent(true)
        .always_on_top(false)
        .skip_taskbar(true)
        .resizable(false)
        .inner_size(STICKY_WIDTH, STICKY_HEIGHT)
        .position(x, y)
        .build()?;
    let _ = window.show();
    Ok(())
}

/// 显示/隐藏切换（便签 + 新闻 + 非置顶便利贴联动）：
/// 隐藏→便签/新闻/非置顶便利贴同时取消置顶并隐藏；呼出→便签/新闻置顶显示
/// （焦点只给便签，新闻同屏不抢焦点），便利贴仅恢复显示【不置顶】（常驻桌面属性）；
/// 置顶便利贴两分支都不动（常驻提醒，永显且保持置顶）。
/// 全局快捷键与托盘菜单均走此入口，联动在此一处生效。
pub fn toggle_sticky(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("sticky") {
        let visible = window.is_visible().unwrap_or(false);
        let news = app.get_webview_window("news");
        let pinned = crate::notes::pinned_note_ids(app);
        let note_windows: Vec<_> = app
            .webview_windows()
            .into_iter()
            .filter(|(label, _)| label.starts_with("note-"))
            .map(|(label, w)| (label.trim_start_matches("note-").to_string(), w))
            .collect();
        if visible {
            let _ = window.set_always_on_top(false);
            let _ = window.hide();
            if let Some(n) = &news {
                let _ = n.set_always_on_top(false);
                let _ = n.hide();
            }
            for (id, w) in &note_windows {
                if !pinned.contains(id) {
                    let _ = w.set_always_on_top(false);
                    let _ = w.hide();
                }
            }
        } else {
            // 先显示新闻窗口，再呼出便签并聚焦
            if let Some(n) = &news {
                let _ = n.set_always_on_top(true);
                let _ = n.show();
                let _ = n.unminimize();
            }
            let _ = window.set_always_on_top(true);
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            for (id, w) in &note_windows {
                if !pinned.contains(id) {
                    // 恢复显示但不置顶：便利贴是常驻桌面属性，与便签「呼出即置顶」不同
                    let _ = w.show();
                    let _ = w.unminimize();
                }
            }
        }
    }
}

/// 注册指定快捷键，触发时切换便签窗口
pub fn register_shortcut(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(hotkey, |app, _shortcut, event| {
            // 只在按下时触发一次，避免「按下 + 释放」重复切换
            if event.state == ShortcutState::Pressed {
                toggle_sticky(app);
            }
        })
        .map_err(|e| e.to_string())
}

/// 启动时从 settings 读取快捷键并注册（空值回退默认）
pub fn register_hotkey(app: &tauri::AppHandle) -> Result<(), String> {
    let hotkey = crate::storage::load_settings(app)
        .map(|s| {
            if s.hotkey.trim().is_empty() {
                DEFAULT_HOTKEY.to_string()
            } else {
                s.hotkey
            }
        })
        .unwrap_or_else(|_| DEFAULT_HOTKEY.to_string());
    register_shortcut(app, &hotkey)
}
