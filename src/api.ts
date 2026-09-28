// 调用 Tauri 后端 command 的统一封装
import { invoke } from "@tauri-apps/api/core";
import type { AppData, Task, Category, Settings, NewsCache, HostsConfig, HostsStatus, ApplyOutcome, Note, NotePatch } from "./types";

// 一次性加载完整应用数据（tasks + categories + settings）
export async function loadData(): Promise<AppData> {
  return invoke<AppData>("load_data");
}

// 保存任务列表（新增/更新/删除后调用，Rust 端负责写文件并触发同步）
export async function saveTasks(tasks: Task[]): Promise<void> {
  await invoke("save_tasks", { tasks });
}

// 保存分类列表
export async function saveCategories(categories: Category[]): Promise<void> {
  await invoke("save_categories", { categories });
}

// 保存应用配置
export async function saveSettings(settings: Settings): Promise<void> {
  await invoke("save_settings", { settings });
}

// 修改呼出/隐藏便签的全局快捷键（Rust 端注销旧键、注册新键并持久化）
export async function setHotkey(hotkey: string): Promise<void> {
  await invoke("set_hotkey", { hotkey });
}

// 手动触发一次 git 同步（pull + commit + push），返回结果信息
export async function syncNow(): Promise<string> {
  return invoke<string>("sync_now");
}

// 托盘菜单「新建待办」唤起主窗口并进入新建态
export async function focusWindow(): Promise<void> {
  await invoke("focus_window");
}

// 读取新闻本地缓存（窗口打开时调用，可能是空/旧数据）
export async function getNews(): Promise<NewsCache> {
  return invoke<NewsCache>("get_news");
}

// 触发一次后台新闻抓取（立即返回，结果经 "news-updated" 事件推送）
export async function refreshNews(): Promise<void> {
  await invoke("refresh_news");
}

// ===== Hosts 编辑器 =====

// 加载 hosts 分组配置（app_data_dir/hosts.json）
export async function loadHosts(): Promise<HostsConfig> {
  return invoke<HostsConfig>("load_hosts");
}

// 保存 hosts 分组配置（Rust 端写文件并触发同步）
export async function saveHosts(config: HostsConfig): Promise<void> {
  await invoke("save_hosts", { config });
}

// 查询系统 hosts 标记块是否与当前配置一致（用于「已是最新/有未应用更改」提示）
export async function hostsStatus(): Promise<HostsStatus> {
  return invoke<HostsStatus>("hosts_status");
}

// 应用到系统：提权子进程写 hosts + 刷新 DNS 缓存；UAC 取消走 cancelled 而非报错
export async function applyHosts(): Promise<ApplyOutcome> {
  return invoke<ApplyOutcome>("apply_hosts");
}

// 修改呼出 Hosts 编辑器的全局快捷键（Rust 端注销旧键、注册新键并持久化）
export async function setHostsHotkey(hotkey: string): Promise<void> {
  await invoke("set_hosts_hotkey", { hotkey });
}

// ===== 便利贴 =====

// 加载全部便利贴（app_data_dir/notes.json）
export async function loadNotes(): Promise<Note[]> {
  return invoke<Note[]>("load_notes");
}

// 新建便利贴（Rust 端写数据 + 创建桌面窗口）
export async function createNote(): Promise<Note> {
  return invoke<Note>("create_note");
}

// 局部更新一条便利贴（Rust 端在磁盘最新版上按 id 合并，避免多窗口覆盖竞态）
export async function updateNote(id: string, patch: NotePatch): Promise<void> {
  await invoke("update_note", { id, patch });
}

// 删除便利贴（Rust 端删数据 + 关闭对应窗口）
export async function deleteNote(id: string): Promise<void> {
  await invoke("delete_note", { id });
}

// 定位便利贴：拉回屏幕可见区域并显示聚焦（找回被拖到屏幕外的便签）
export async function relocateNote(id: string): Promise<void> {
  await invoke("relocate_note", { id });
}
