mod hosts;
mod news;
mod notes;
mod sticky;
mod storage;
mod sync;
mod tray;

use storage::{AppData, Category, Settings, Task, TaskStatus};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_notification::NotificationExt;

// ===== Commands =====

#[tauri::command]
fn load_data(app: tauri::AppHandle) -> Result<AppData, String> {
    storage::load_data(&app)
}

#[tauri::command]
fn save_tasks(app: tauri::AppHandle, tasks: Vec<Task>) -> Result<(), String> {
    storage::save_tasks(&app, &tasks)?;
    maybe_auto_sync(&app);
    let _ = app.emit("data-changed", ());
    Ok(())
}

#[tauri::command]
fn save_categories(app: tauri::AppHandle, categories: Vec<Category>) -> Result<(), String> {
    storage::save_categories(&app, &categories)?;
    maybe_auto_sync(&app);
    let _ = app.emit("data-changed", ());
    Ok(())
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    storage::save_settings(&app, &settings)?;
    // 广播给所有窗口：便签/新闻窗口及时拿到新设置（主题、新闻源等）
    let _ = app.emit("data-changed", ());
    Ok(())
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle) -> Result<String, String> {
    let dir = storage::data_dir(&app)?;
    let settings = storage::load_settings(&app)?;
    let outcome = sync::sync(&dir, &settings)?;
    // 拉到了远程新内容：tasks.json 等已被更新，广播让各窗口从磁盘重载
    if outcome.pulled {
        let _ = app.emit("data-changed", ());
    }
    Ok(outcome.message)
}

#[tauri::command]
fn focus_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

/// 注销全部后重注册便签 + hosts 两个槽位；两个都成功才返回 Ok。
/// 注意：失败后快捷键处于「全空」状态，调用方必须回滚。
fn reregister_all_shortcuts(
    app: &tauri::AppHandle,
    sticky_hotkey: &str,
    hosts_hotkey: &str,
) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    sticky::register_shortcut(app, sticky_hotkey)?;
    hosts::register_shortcut(app, hosts_hotkey)?;
    Ok(())
}

/// 尽力恢复旧的两个槽位（回滚再失败只能打日志，无法再退）
fn rollback_shortcuts(app: &tauri::AppHandle, old_sticky: &str, old_hosts: &str) {
    let _ = app.global_shortcut().unregister_all();
    if !old_sticky.trim().is_empty() {
        if let Err(e) = sticky::register_shortcut(app, old_sticky) {
            eprintln!("回滚便签快捷键失败: {e}");
        }
    }
    if !old_hosts.trim().is_empty() {
        if let Err(e) = hosts::register_shortcut(app, old_hosts) {
            eprintln!("回滚 Hosts 快捷键失败: {e}");
        }
    }
}

/// 读取当前生效的两个快捷键（settings 缺省/为空时回退各自默认键）
fn effective_hotkeys(app: &tauri::AppHandle) -> Result<(String, String), String> {
    let settings = storage::load_settings(app)?;
    let sticky_hk = if settings.hotkey.trim().is_empty() {
        "Ctrl+Shift+Space".to_string()
    } else {
        settings.hotkey.trim().to_string()
    };
    let hosts_hk = if settings.hosts_hotkey.trim().is_empty() {
        "Ctrl+Alt+H".to_string()
    } else {
        settings.hosts_hotkey.trim().to_string()
    };
    Ok((sticky_hk, hosts_hk))
}

#[tauri::command]
fn set_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    let trimmed = hotkey.trim().to_string();
    if trimmed.is_empty() {
        return Err("快捷键不能为空".into());
    }
    let (old_sticky, old_hosts) = effective_hotkeys(&app)?;
    if trimmed.eq_ignore_ascii_case(&old_hosts) {
        return Err("与 Hosts 编辑器快捷键冲突".into());
    }
    if let Err(e) = reregister_all_shortcuts(&app, &trimmed, &old_hosts) {
        rollback_shortcuts(&app, &old_sticky, &old_hosts);
        return Err(format!("快捷键格式无效或被占用：{e}"));
    }
    let mut settings = storage::load_settings(&app)?;
    settings.hotkey = trimmed;
    storage::save_settings(&app, &settings)?;
    Ok(())
}

#[tauri::command]
fn set_hosts_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    let trimmed = hotkey.trim().to_string();
    if trimmed.is_empty() {
        return Err("快捷键不能为空".into());
    }
    let (old_sticky, old_hosts) = effective_hotkeys(&app)?;
    if trimmed.eq_ignore_ascii_case(&old_sticky) {
        return Err("与便签快捷键冲突".into());
    }
    if let Err(e) = reregister_all_shortcuts(&app, &old_sticky, &trimmed) {
        rollback_shortcuts(&app, &old_sticky, &old_hosts);
        return Err(format!("快捷键格式无效或被占用：{e}"));
    }
    let mut settings = storage::load_settings(&app)?;
    settings.hosts_hotkey = trimmed;
    storage::save_settings(&app, &settings)?;
    Ok(())
}

// ===== 自动同步（后台异步，避免阻塞 UI） =====

fn maybe_auto_sync(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let settings = match storage::load_settings(&app) {
            Ok(s) => s,
            Err(_) => return,
        };
        if !settings.auto_sync || settings.remote_url.is_empty() {
            return;
        }
        let dir = match storage::data_dir(&app) {
            Ok(d) => d,
            Err(_) => return,
        };
        // 后台同步也可能拉到远程新内容（如另一台设备推的任务），同样广播重载
        if let Ok(outcome) = sync::sync(&dir, &settings) {
            if outcome.pulled {
                let _ = app.emit("data-changed", ());
            }
        }
    });
}

// ===== 开机通知：展示未完成事项 =====

fn notify_unfinished(app: &tauri::AppHandle) {
    if let Ok(data) = storage::load_data(app) {
        let unfinished: Vec<&Task> = data
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Todo || t.status == TaskStatus::Doing)
            .collect();
        if unfinished.is_empty() {
            return;
        }
        let count = unfinished.len();
        let preview: Vec<String> = unfinished
            .iter()
            .take(3)
            .map(|t| t.title.clone())
            .collect();
        let mut body = format!("您有 {count} 项未完成事项");
        if !preview.is_empty() {
            body.push_str("：");
            body.push_str(&preview.join("、"));
        }
        let _ = app
            .notification()
            .builder()
            .title("了事提醒")
            .body(&body)
            .show();
    }
}

// ===== 入口 =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // helper 模式拦截：被 runas 提权拉起的管理员子进程，写完 hosts 立即退出，
    // 绝不进入 Tauri 生命周期（否则会闪出第二套托盘和窗口）
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--apply-hosts" {
        std::process::exit(hosts::run_helper(&args[2]));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            tray::create_tray(app.handle())?;
            sticky::create_sticky_window(app.handle())?;
            // 新闻窗口与便签联动（toggle_sticky 一处控制两窗显隐），失败不阻断启动
            if let Err(e) = news::create_news_window(app.handle()) {
                eprintln!("创建新闻窗口失败: {e}");
            }
            // 便利贴窗口：常驻桌面，每条便签一个独立小窗（单窗失败内部容错不阻断）
            notes::create_note_windows(app.handle());
            if let Err(e) = sticky::register_hotkey(app.handle()) {
                eprintln!("注册全局快捷键失败: {e}");
            }
            if let Err(e) = hosts::register_hotkey(app.handle()) {
                eprintln!("注册 Hosts 快捷键失败: {e}");
            }
            notify_unfinished(app.handle());
            // 启动时后台抓取新闻（不阻塞启动），完成后经 "news-updated" 事件推送
            news::spawn_refresh(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label().starts_with("note-") {
                    // 便利贴无标题栏，CloseRequested 只会来自 Alt+F4：
                    // 转发给该窗口前端做删除确认（不确认则留在桌面），数据无损
                    let _ = window.emit_to(window.label(), "note-close-requested", ());
                } else {
                    let _ = window.hide();
                }
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            load_data,
            save_tasks,
            save_categories,
            save_settings,
            sync_now,
            focus_window,
            set_hotkey,
            set_hosts_hotkey,
            hosts::load_hosts,
            hosts::save_hosts,
            hosts::hosts_status,
            hosts::apply_hosts,
            news::get_news,
            news::refresh_news,
            notes::load_notes,
            notes::create_note,
            notes::update_note,
            notes::delete_note,
            notes::relocate_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
