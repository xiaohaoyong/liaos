// 数据层：数据结构定义 + tasks.json / categories.json / settings.json 读写
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

// ===== 数据结构（与前端 src/types.ts 对应） =====

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub category: String,
    pub status: TaskStatus,
    pub progress: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned_end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub is_preset: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub remote_url: String,
    pub username: String,
    pub token: String,
    pub auto_sync: bool,
    pub autostart: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_hosts_hotkey")]
    pub hosts_hotkey: String,
    #[serde(default = "default_news_feeds")]
    pub news_feeds: Vec<NewsFeed>,
}

fn default_theme() -> String {
    "system".into()
}

fn default_hotkey() -> String {
    "Ctrl+Shift+Space".into()
}

fn default_hosts_hotkey() -> String {
    "Ctrl+Alt+H".into()
}

// 新闻源（Settings.news_feeds 元素）
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewsFeed {
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

// 默认新闻源：中文综合 + 科技情报 + 英文社区（均已验证可直连）
fn default_news_feeds() -> Vec<NewsFeed> {
    vec![
        NewsFeed {
            name: "开源中国".into(),
            url: "https://www.oschina.net/news/rss".into(),
            enabled: true,
        },
        NewsFeed {
            name: "Solidot".into(),
            url: "https://www.solidot.org/index.rss".into(),
            enabled: true,
        },
        NewsFeed {
            name: "Hacker News".into(),
            url: "https://hnrss.org/frontpage".into(),
            enabled: true,
        },
    ]
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppData {
    pub tasks: Vec<Task>,
    pub categories: Vec<Category>,
    pub settings: Settings,
}

// ===== 预设数据 =====

fn default_categories() -> Vec<Category> {
    vec![
        Category {
            id: "work".into(),
            name: "工作".into(),
            is_preset: true,
        },
        Category {
            id: "life".into(),
            name: "生活".into(),
            is_preset: true,
        },
        Category {
            id: "study".into(),
            name: "学习".into(),
            is_preset: true,
        },
        Category {
            id: "other".into(),
            name: "其他".into(),
            is_preset: true,
        },
    ]
}

fn default_settings() -> Settings {
    Settings {
        remote_url: String::new(),
        username: String::new(),
        token: String::new(),
        auto_sync: true,
        autostart: false,
        theme: default_theme(),
        hotkey: default_hotkey(),
        hosts_hotkey: default_hosts_hotkey(),
        news_feeds: default_news_feeds(),
    }
}

// ===== 数据目录与文件路径 =====

pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn tasks_path(dir: &Path) -> PathBuf {
    dir.join("tasks.json")
}
fn categories_path(dir: &Path) -> PathBuf {
    dir.join("categories.json")
}
fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

// ===== 通用 JSON 读写 =====

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, default: T) -> T {
    match fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or(default),
        Err(_) => default,
    }
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let s = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

// ===== 对外接口 =====

/// 数据目录治理：确保 .gitignore 排除本地缓存文件（幂等，每次启动检查一次）。
/// sync.rs 的 add_all 使用 IndexAddOption::DEFAULT，遵循 .gitignore；
/// .gitignore 自身随 git 同步分发到其他设备是期望行为。
fn ensure_gitignore(dir: &Path) {
    let path = dir.join(".gitignore");
    if !path.exists() {
        let _ = fs::write(&path, "news_cache.json\n");
    }
}

/// 加载完整应用数据；首次运行时写入预设分类与默认设置
pub fn load_data(app: &tauri::AppHandle) -> Result<AppData, String> {
    let dir = data_dir(app)?;
    ensure_gitignore(&dir);

    // 分类：首次无文件时使用预设
    let cpath = categories_path(&dir);
    let categories = if cpath.exists() {
        read_json(&cpath, default_categories())
    } else {
        let d = default_categories();
        write_json(&cpath, &d)?;
        d
    };

    // 设置：首次无文件时使用默认
    let spath = settings_path(&dir);
    let settings = if spath.exists() {
        read_json(&spath, default_settings())
    } else {
        let d = default_settings();
        write_json(&spath, &d)?;
        d
    };

    let tasks = read_json(&tasks_path(&dir), Vec::new());

    Ok(AppData {
        tasks,
        categories,
        settings,
    })
}

pub fn save_tasks(app: &tauri::AppHandle, tasks: &[Task]) -> Result<(), String> {
    let dir = data_dir(app)?;
    write_json(&tasks_path(&dir), &tasks)
}

pub fn save_categories(app: &tauri::AppHandle, categories: &[Category]) -> Result<(), String> {
    let dir = data_dir(app)?;
    write_json(&categories_path(&dir), &categories)
}

pub fn save_settings(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    let dir = data_dir(app)?;
    write_json(&settings_path(&dir), &settings)
}

pub fn load_settings(app: &tauri::AppHandle) -> Result<Settings, String> {
    let dir = data_dir(app)?;
    Ok(read_json(&settings_path(&dir), default_settings()))
}
