// 便利贴模块：桌面常驻小纸片（每条便签一个独立无边框透明窗口）+ notes.json 持久化。
// 注意与 sticky.rs 的「便签」（快捷待办列表悬浮窗）是两个功能，UI 文案上叫「便利贴」。
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::storage::{self, read_json, write_json};

/// 色板（与前端 types.ts 的 NOTE_COLORS 一致；新建时按现存数量轮换）
pub(crate) const PALETTE: [&str; 6] = ["yellow", "pink", "blue", "green", "purple", "gray"];

/// 新建便利贴的默认逻辑宽度
const DEFAULT_WIDTH: f64 = 240.0;
/// 建窗时的占位高度（前端量完内容高度后会自行 setSize 校准）
const PLACEHOLDER_H: f64 = 160.0;
/// 窗口最小逻辑尺寸
const MIN_W: f64 = 180.0;
const MIN_H: f64 = 100.0;
/// 窗口宽度上限（防异常数据拉出巨型窗口）
const MAX_W: f64 = 800.0;

fn default_note_color() -> String {
    "yellow".into()
}

fn default_note_width() -> f64 {
    DEFAULT_WIDTH
}

/// 当前时间的 ISO 8601 字符串。必须与前端 toISOString() 逐字符同格式
/// （毫秒 + Z 后缀）：sync.rs 的按 id 合并对 updatedAt 做字符串比较择新，
/// 格式不一致会破坏字典序。
fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// 生成便利贴 id（纳秒时间戳 + pid；托盘新建时前端不在场，由 Rust 生成）
fn note_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("n{:x}-{:x}", nanos, std::process::id())
}

// ===== 数据结构（与前端 src/types.ts 的 Note 一一对应） =====

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default = "default_note_color")]
    pub color: String,
    #[serde(default)]
    pub x: f64, // 逻辑坐标
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_note_width")]
    pub width: f64, // 只存宽度；高度随内容自适应，不持久化
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// update_note 的局部字段补丁（前端只传要改的字段，在磁盘最新版上合并，
/// 避免多个便签窗口各持副本互相整表覆盖的丢更新竞态）
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NotePatch {
    pub content: Option<String>,
    pub color: Option<String>,
    pub pinned: Option<bool>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
}

// ===== notes.json 持久化（模板参照 hosts.rs） =====

fn notes_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage::data_dir(app)?.join("notes.json"))
}

/// 读取全部便利贴；文件缺失/损坏时静默回退空列表
pub fn load_notes_internal(app: &tauri::AppHandle) -> Result<Vec<Note>, String> {
    Ok(read_json(&notes_path(app)?, Vec::new()))
}

fn save_notes_internal(app: &tauri::AppHandle, notes: &[Note]) -> Result<(), String> {
    write_json(&notes_path(app)?, &notes)
}

// ===== 位置策略 =====

/// 显示器的逻辑矩形（物理像素 ÷ 缩放系数），返回 (x, y, w, h)
fn monitor_logical_rect(m: &tauri::Monitor) -> (f64, f64, f64, f64) {
    let s = m.scale_factor();
    let pos = m.position();
    let size = m.size();
    (
        pos.x as f64 / s,
        pos.y as f64 / s,
        size.width as f64 / s,
        size.height as f64 / s,
    )
}

/// 便利贴位置兜底：左上角向内 (min(width,60), 40) 的保护点必须落在某个显示器内，
/// 否则（分辨率变化/显示器被拔/数据异常）拉回主显示器左上角附近。
fn clamp_note_position(app: &tauri::AppHandle, x: f64, y: f64, width: f64) -> (f64, f64) {
    let guard_x = x + width.min(60.0);
    let guard_y = y + 40.0;
    let monitors = app.available_monitors().unwrap_or_default();
    let visible = monitors.iter().any(|m| {
        let (mx, my, mw, mh) = monitor_logical_rect(m);
        guard_x >= mx && guard_x < mx + mw && guard_y >= my && guard_y < my + mh
    });
    if visible {
        (x, y)
    } else {
        match app.primary_monitor() {
            Ok(Some(m)) => {
                let (mx, my, _, _) = monitor_logical_rect(&m);
                (mx + 60.0, my + 60.0)
            }
            _ => (60.0, 60.0),
        }
    }
}

/// 新建便利贴的级联位置：主显示器 (80,80) 起，按现存数量右下偏移 32px（模 10 折返），避免重叠
fn cascade_position(app: &tauri::AppHandle, index: usize) -> (f64, f64) {
    let (bx, by) = match app.primary_monitor() {
        Ok(Some(m)) => {
            let (mx, my, _, _) = monitor_logical_rect(&m);
            (mx + 80.0, my + 80.0)
        }
        _ => (80.0, 80.0),
    };
    let off = (index % 10) as f64 * 32.0;
    (bx + off, by + off)
}

// ===== 窗口管理 =====

/// 创建（或唤醒）一条便利贴的窗口。label 为 note-{id}，query 传 id（focus=1 时前端 show 后抢焦点）。
/// visible(false)：先不显示，前端量完内容高度后再 show()，避免启动闪变。
pub fn create_note_window(app: &tauri::AppHandle, note: &Note, focus: bool) -> tauri::Result<()> {
    let label = format!("note-{}", note.id);
    if let Some(w) = app.get_webview_window(&label) {
        // 同 id 窗口已存在：唤醒而非重建
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let (x, y) = clamp_note_position(app, note.x, note.y, note.width);
    let url = if focus {
        format!("note.html?id={}&focus=1", note.id)
    } else {
        format!("note.html?id={}", note.id)
    };
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title("便利贴")
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .resizable(true)
        .always_on_top(note.pinned)
        .visible(false)
        .min_inner_size(MIN_W, MIN_H)
        .inner_size(note.width, PLACEHOLDER_H)
        .position(x, y)
        .build()?;
    Ok(())
}

/// 启动时批量建窗（常驻桌面：开机后所有便利贴直接显示）。
/// 单窗失败仅打日志继续，不阻断启动。
pub fn create_note_windows(app: &tauri::AppHandle) {
    match load_notes_internal(app) {
        Ok(notes) => {
            for note in &notes {
                if let Err(e) = create_note_window(app, note, false) {
                    eprintln!("创建便利贴窗口失败（{}）: {e}", note.id);
                }
            }
        }
        Err(e) => eprintln!("读取 notes.json 失败: {e}"),
    }
}

/// 当前置顶便利贴的 id 集合（sticky.rs 的快捷键联动用；读取失败视为无置顶）
pub fn pinned_note_ids(app: &tauri::AppHandle) -> HashSet<String> {
    load_notes_internal(app)
        .map(|ns| ns.into_iter().filter(|n| n.pinned).map(|n| n.id).collect())
        .unwrap_or_default()
}

// ===== 业务入口（托盘直接调用；command 是给前端的薄封装） =====

/// 新建一条便利贴：写数据 → 建窗 → 触发同步 → 广播。色板按现存数量轮换。
pub fn create_note_internal(app: &tauri::AppHandle) -> Result<Note, String> {
    let mut notes = load_notes_internal(app)?;
    let t = now_iso();
    let (x, y) = cascade_position(app, notes.len());
    let note = Note {
        id: note_id(),
        content: String::new(),
        color: PALETTE[notes.len() % PALETTE.len()].to_string(),
        x,
        y,
        width: DEFAULT_WIDTH,
        pinned: false,
        created_at: t.clone(),
        updated_at: t,
    };
    notes.push(note.clone());
    save_notes_internal(app, &notes)?;
    if let Err(e) = create_note_window(app, &note, true) {
        // 数据已写入，窗口失败不影响（下次启动补建）
        eprintln!("创建便利贴窗口失败: {e}");
    }
    crate::maybe_auto_sync(app);
    let _ = app.emit("notes-changed", note.id.clone());
    Ok(note)
}

/// 删除便利贴：先落盘（数据是唯一事实源）→ 关窗 → 触发同步 → 广播。
/// destroy 不触发 CloseRequested，不会被 lib.rs 的关闭拦截捕获；
/// 万一 destroy 失败留下残窗，前端发现记录不存在会自愈销毁。
pub fn delete_note_internal(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let mut notes = load_notes_internal(app)?;
    notes.retain(|n| n.id != id);
    save_notes_internal(app, &notes)?;
    if let Some(w) = app.get_webview_window(&format!("note-{id}")) {
        let _ = w.destroy();
    }
    crate::maybe_auto_sync(app);
    let _ = app.emit("notes-changed", id.to_string());
    Ok(())
}

/// 定位/找回：clamp 到可见区域 → 持久化 → 窗口存在则移过去显示聚焦，不存在则补建
pub fn relocate_note_internal(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let mut notes = load_notes_internal(app)?;
    let Some(n) = notes.iter_mut().find(|n| n.id == id) else {
        return Ok(());
    };
    let (x, y) = clamp_note_position(app, n.x, n.y, n.width);
    n.x = x;
    n.y = y;
    n.updated_at = now_iso();
    let note = n.clone();
    save_notes_internal(app, &notes)?;
    let label = format!("note-{id}");
    match app.get_webview_window(&label) {
        Some(w) => {
            let _ = w.set_position(tauri::LogicalPosition::new(x, y));
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        None => {
            let _ = create_note_window(app, &note, true);
        }
    }
    crate::maybe_auto_sync(app);
    Ok(())
}

// ===== Commands =====

#[tauri::command]
pub fn load_notes(app: tauri::AppHandle) -> Result<Vec<Note>, String> {
    load_notes_internal(&app)
}

/// 注意必须 async：Tauri 2 的同步 command 在主线程执行，而 build() 需要等主线程
/// 事件循环建窗——同步 command 里建窗等于主线程自己等自己，直接死锁
/// （表现为点击新建无反应，且此后所有 invoke 都不再响应）。
/// async command 跑在线程池，不占主线程。
#[tauri::command]
pub async fn create_note(app: tauri::AppHandle) -> Result<Note, String> {
    create_note_internal(&app)
}

/// 局部字段更新：读磁盘最新版按 id 合并 patch。
/// 内容/颜色/置顶变更后广播 notes-changed；纯几何（x/y/width）变更不广播
/// （位置无需实时同步给其他窗口，且避免打断别窗正在编辑的文本）。
#[tauri::command]
pub fn update_note(app: tauri::AppHandle, id: String, patch: NotePatch) -> Result<(), String> {
    let mut notes = load_notes_internal(&app)?;
    let Some(n) = notes.iter_mut().find(|n| n.id == id) else {
        // 记录已被删除（删除竞态）：静默成功，发起方便签窗口随后自愈销毁
        return Ok(());
    };
    let mut broadcast = false;
    if let Some(v) = patch.content {
        n.content = v;
        broadcast = true;
    }
    if let Some(v) = patch.color {
        if PALETTE.contains(&v.as_str()) {
            n.color = v;
            broadcast = true;
        }
    }
    if let Some(v) = patch.pinned {
        n.pinned = v;
        broadcast = true;
    }
    if let Some(v) = patch.x {
        n.x = v;
    }
    if let Some(v) = patch.y {
        n.y = v;
    }
    if let Some(v) = patch.width {
        n.width = v.clamp(MIN_W, MAX_W);
    }
    // 几何变更也刷新时间戳：同步冲突时位置变更才能按 updatedAt 胜出
    n.updated_at = now_iso();
    save_notes_internal(&app, &notes)?;
    crate::maybe_auto_sync(&app);
    if broadcast {
        let _ = app.emit("notes-changed", id);
    }
    Ok(())
}

#[tauri::command]
pub fn delete_note(app: tauri::AppHandle, id: String) -> Result<(), String> {
    delete_note_internal(&app, &id)
}

/// 同 create_note：内部可能补建窗口（create_note_window），必须 async 避免主线程死锁
#[tauri::command]
pub async fn relocate_note(app: tauri::AppHandle, id: String) -> Result<(), String> {
    relocate_note_internal(&app, &id)
}
