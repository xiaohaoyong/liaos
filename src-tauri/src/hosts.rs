// Hosts 编辑器模块：域名分组管理 + 写入系统 hosts 文件（按需提权）+ 独立窗口 + 全局快捷键
//
// 写入策略：只替换系统 hosts 中的「标记块」（# >>> liaos hosts BEGIN 到 # <<< liaos hosts END），
// 块外内容字节级原样保留（用户 hosts 可能是 GBK 编码，禁止经 String 转换）。
// 权限策略：应用平时以普通权限运行，点「应用到系统」时用 ShellExecuteExW(runas) 拉起
// 自身的 helper 子进程（参数 --apply-hosts <临时文件>），由管理员身份完成备份+覆写+刷新 DNS。
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::storage::{self, read_json, write_json};

/// 默认快捷键（与 storage.rs 的 default_hosts_hotkey 保持一致）
const DEFAULT_HOSTS_HOTKEY: &str = "Ctrl+Alt+H";

/// 标记块边界（纯 ASCII，便于字节级查找）
const MARK_BEGIN: &[u8] = b"# >>> liaos hosts BEGIN";
const MARK_END: &[u8] = b"# <<< liaos hosts END";

// ===== 数据结构（与前端 src/types.ts 一一对应） =====

/// 域名下的一个候选 IP（可带备注，如「开发」「生产」）
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostsIp {
    pub id: String,
    pub ip: String,
    #[serde(default)]
    pub note: String,
}

/// 一个域名的完整配置：多个候选 IP，同一时刻仅 active_ip_id 指向的 IP 生效
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostsDomain {
    pub id: String,
    pub domain: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub ips: Vec<HostsIp>,
    #[serde(default)]
    pub active_ip_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostsConfig {
    /// 兼容旧版「分组」结构：缺失/无法识别时按空配置处理，不让整个文件读取失败
    #[serde(default)]
    pub domains: Vec<HostsDomain>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostsStatus {
    pub up_to_date: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub cancelled: bool,
    pub message: String,
}

fn default_true() -> bool {
    true
}

// ===== hosts.json 持久化 =====

fn hosts_config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage::data_dir(app)?.join("hosts.json"))
}

pub fn load_config(app: &tauri::AppHandle) -> Result<HostsConfig, String> {
    let path = hosts_config_path(app)?;
    Ok(read_json(&path, HostsConfig::default()))
}

pub fn save_config(app: &tauri::AppHandle, config: &HostsConfig) -> Result<(), String> {
    let path = hosts_config_path(app)?;
    write_json(&path, config)
}

// ===== 标记块读写（字节级，保护块外任意编码内容） =====

fn find_mark(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 配置 → 标记块完整字节（含首尾标记行）。
/// 每个启用且已选生效 IP 的域名渲染一行「{生效IP} {域名}」，候选 IP 与备注写入注释头。
pub fn render_block(config: &HostsConfig) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("# >>> liaos hosts BEGIN\r\n");
    s.push_str("# Managed by Liaos. 块外内容不会被修改。\r\n\r\n");
    for d in config.domains.iter().filter(|d| d.enabled) {
        let domain = d.domain.trim();
        if domain.is_empty() {
            continue;
        }
        let Some(active_ip_id) = d.active_ip_id.as_ref() else {
            continue; // 未选择生效 IP：该域名不写入
        };
        let Some(active) = d.ips.iter().find(|i| &i.id == active_ip_id) else {
            continue; // 生效 IP 已被删除：视为未选择
        };
        let ip = active.ip.trim();
        if ip.is_empty() {
            continue;
        }
        let note = active.note.trim();
        if note.is_empty() {
            s.push_str(&format!("# --- {domain} ---\r\n{ip} {domain}\r\n"));
        } else {
            s.push_str(&format!("# --- {domain}（{note}） ---\r\n{ip} {domain}\r\n"));
        }
    }
    s.push_str("\r\n# <<< liaos hosts END");
    s.into_bytes()
}

/// 把新标记块合入原 hosts 字节：块外内容原样保留。
/// 三种情况：双标记齐全 → 原位替换；只有 BEGIN（残缺）→ 起点到文件尾让位；无块 → 尾部追加。
pub fn merge_block(original: &[u8], block: &[u8]) -> Vec<u8> {
    match (find_mark(original, MARK_BEGIN), find_mark(original, MARK_END)) {
        (Some(begin), Some(end)) if begin < end => {
            let mut v = original[..begin].to_vec();
            v.extend_from_slice(block);
            v.extend_from_slice(&original[end + MARK_END.len()..]);
            v
        }
        (Some(begin), _) => {
            // END 缺失或错位：BEGIN 起到 EOF 均为 liaos 曾管辖内容，整体让位给新块
            let mut v = original[..begin].to_vec();
            v.extend_from_slice(block);
            v
        }
        (None, _) => {
            let mut v = original.to_vec();
            if !v.is_empty() && !v.ends_with(b"\n") {
                v.push(b'\n');
            }
            v.extend_from_slice(block);
            v
        }
    }
}

/// 系统 hosts 路径（x64 进程无 WoW64 重定向，System32 即真实目录）
pub fn system_hosts_path() -> PathBuf {
    let windir = std::env::var_os("windir").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(windir)
        .join("System32")
        .join("drivers")
        .join("etc")
        .join("hosts")
}

// ===== 已有条目纳入：块外冲突行注释化 =====
//
// 用户在用了事之前，hosts 里可能已有手工条目。若这些条目与 liaos 管理的域名同名，
// 直接保留会导致 Windows 解析命中块外旧行，liaos 标记块内的配置不生效。
// 因此每次应用时：把块外与 liaos 管理域名（含停用）同名的非注释行注释掉，让生效权交给标记块。

/// 去除字节切片首尾的空白（ASCII 空白）
fn trim_ws(b: &[u8]) -> &[u8] {
    match (b.iter().position(|c| !c.is_ascii_whitespace()), b.iter().rposition(|c| !c.is_ascii_whitespace())) {
        (Some(s), Some(e)) => &b[s..e + 1],
        _ => &[],
    }
}

/// 解析一行 hosts 条目，返回 (ip, hostname) 字节；空行/纯注释/格式错误返回 None。
/// 只取前两个 token（hosts 一行就是 `IP 主机名`），遇 `#` 起头的 token 停止。
fn parse_host_entry(line: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut tokens: Vec<&[u8]> = Vec::new();
    for t in line.split(|c| c.is_ascii_whitespace()) {
        let t = trim_ws(t);
        if t.is_empty() {
            continue;
        }
        if t[0] == b'#' {
            break;
        }
        tokens.push(t);
        if tokens.len() == 2 {
            break;
        }
    }
    if tokens.len() == 2 {
        Some((tokens[0].to_vec(), tokens[1].to_vec()))
    } else {
        None
    }
}

/// 把系统 hosts 块外与 liaos 管理域名冲突的条目行注释掉。
/// 仅处理标记块之外的字节（块内会被 render_block 整体替换），逐行字节级操作，
/// 只往行首插入 ASCII 前缀，行内容与行尾原样保留——兼容 GBK 等任意编码的块外内容。
pub fn neutralize_conflicts(original: &[u8], config: &HostsConfig) -> Vec<u8> {
    let managed: std::collections::HashSet<String> = config
        .domains
        .iter()
        .map(|d| d.domain.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if managed.is_empty() {
        return original.to_vec();
    }

    // 标记块区间（BEGIN..END+len）：整体跳过，交由 render_block 管辖
    let begin = find_mark(original, MARK_BEGIN);
    let end = find_mark(original, MARK_END);
    let (skip_start, skip_end) = match (begin, end) {
        (Some(b), Some(e)) if b < e => (b, e + MARK_END.len()),
        (Some(b), _) => (b, original.len()),
        (None, _) => (original.len(), original.len()),
    };

    let mut out = Vec::with_capacity(original.len() + 64);
    let mut i = 0;
    while i < original.len() {
        if i == skip_start {
            out.extend_from_slice(&original[skip_start..skip_end]);
            i = skip_end;
            continue;
        }
        let line_end = match original[i..].iter().position(|&b| b == b'\n') {
            Some(p) => i + p + 1, // 含 \n
            None => original.len(),
        };
        let line_bytes = &original[i..line_end];
        let content = line_bytes
            .strip_suffix(b"\n")
            .and_then(|c| c.strip_suffix(b"\r"))
            .unwrap_or(line_bytes);
        if let Some((_ip, host)) = parse_host_entry(content) {
            let host_lower = String::from_utf8_lossy(&host).to_ascii_lowercase();
            if managed.contains(&host_lower) {
                out.extend_from_slice(b"# liaos-managed: ");
            }
        }
        out.extend_from_slice(line_bytes);
        i = line_end;
    }
    out
}

/// 当前系统 hosts 的标记块是否与配置生成的期望内容一致（读 hosts 无需提权）
fn is_up_to_date(original: &[u8], expected: &[u8]) -> bool {
    match (find_mark(original, MARK_BEGIN), find_mark(original, MARK_END)) {
        (Some(begin), Some(end)) if begin < end => {
            &original[begin..end + MARK_END.len()] == expected
        }
        // 无块或残缺：空配置 + 空块视为最新，否则有未应用更改
        _ => {
            let empty_block = render_block(&HostsConfig::default());
            expected == empty_block.as_slice()
        }
    }
}

// ===== 提权 helper（管理员子进程主体，经 run() 首行拦截进入） =====

/// helper 退出码约定：0 成功 / 2 临时文件不可读 / 3 备份失败 / 4 写入失败
pub fn run_helper(tmp_path: &str) -> i32 {
    let hosts_path = system_hosts_path();
    let new_bytes = match std::fs::read(tmp_path) {
        Ok(b) => b,
        Err(_) => return 2,
    };

    // 备份：与 hosts 同目录，覆盖式单文件快照
    let bak_path = hosts_path.with_file_name("hosts.liaos.bak");
    match std::fs::read(&hosts_path) {
        Ok(old) => {
            if std::fs::write(&bak_path, &old).is_err() {
                return 3;
            }
        }
        Err(_) => return 3,
    }

    // 去只读属性（hosts 偶被用户/杀软设只读，管理员也绕不过属性位）
    if let Ok(meta) = std::fs::metadata(&hosts_path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(&hosts_path, perms);
        }
    }

    if std::fs::write(&hosts_path, &new_bytes).is_err() {
        return 4;
    }
    flush_dns();
    let _ = std::fs::remove_file(tmp_path);
    0
}

/// 刷新 DNS 缓存让解析立即生效；失败不影响写入结果
fn flush_dns() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("ipconfig")
            .arg("/flushdns")
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}

/// UAC 拉起自身 helper。Ok(Some(code))=helper 执行完；Ok(None)=用户取消授权；Err=其它失败
fn run_helper_elevated(exe: &str, tmp_path: &std::path::Path) -> Result<Option<i32>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, WAIT_FAILED};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let verb = wide("runas");
    let file = wide(exe);
    let parameters = wide(&format!("--apply-hosts \"{}\"", tmp_path.display()));

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0 as i32,
        ..Default::default()
    };

    unsafe {
        if let Err(e) = ShellExecuteExW(&mut info) {
            // HRESULT 低 16 位是 win32 错误码；1223 = ERROR_CANCELLED（用户在 UAC 点了「否」）
            let win32_code = (e.code().0 & 0xFFFF) as u32;
            if win32_code == 1223 {
                return Ok(None);
            }
            return Err(format!("提权启动失败（错误码 {win32_code}）"));
        }
        if info.hProcess.is_invalid() {
            return Ok(Some(0));
        }
        if WaitForSingleObject(info.hProcess, u32::MAX) == WAIT_FAILED {
            let _ = CloseHandle(info.hProcess);
            return Err("等待应用进程失败".into());
        }
        let mut exit_code: u32 = 0;
        let _ = GetExitCodeProcess(info.hProcess, &mut exit_code);
        let _ = CloseHandle(info.hProcess);
        Ok(Some(exit_code as i32))
    }
}

fn helper_exit_message(code: i32) -> String {
    match code {
        2 => "应用失败：临时文件不可读".into(),
        3 => "应用失败：备份原 hosts 文件失败".into(),
        4 => "应用失败：写入 hosts 被拒绝，请检查杀毒软件或文件属性".into(),
        _ => format!("应用失败（helper 退出码 {code}）"),
    }
}

// ===== 窗口与全局快捷键 =====

/// 打开（或唤醒）Hosts 编辑器窗口；托盘菜单与全局快捷键共用此入口
pub fn open_hosts_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("hosts") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    match WebviewWindowBuilder::new(app, "hosts", WebviewUrl::App("hosts.html".into()))
        .title("Hosts 编辑器")
        .inner_size(780.0, 560.0)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .build()
    {
        Ok(window) => {
            let _ = window.show();
        }
        Err(e) => eprintln!("创建 Hosts 编辑器窗口失败: {e}"),
    }
}

/// 注册指定快捷键，触发时呼出 Hosts 编辑器窗口
pub fn register_shortcut(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(hotkey, |app, _shortcut, event| {
            // 只在按下时触发一次，避免「按下 + 释放」重复呼出
            if event.state == ShortcutState::Pressed {
                open_hosts_window(app);
            }
        })
        .map_err(|e| e.to_string())
}

/// 启动时从 settings 读取快捷键并注册（空值回退默认）
pub fn register_hotkey(app: &tauri::AppHandle) -> Result<(), String> {
    let hotkey = crate::storage::load_settings(app)
        .map(|s| {
            if s.hosts_hotkey.trim().is_empty() {
                DEFAULT_HOSTS_HOTKEY.to_string()
            } else {
                s.hosts_hotkey
            }
        })
        .unwrap_or_else(|_| DEFAULT_HOSTS_HOTKEY.to_string());
    register_shortcut(app, &hotkey)
}

// ===== Commands =====

#[tauri::command]
pub fn load_hosts(app: tauri::AppHandle) -> Result<HostsConfig, String> {
    load_config(&app)
}

#[tauri::command]
pub fn save_hosts(app: tauri::AppHandle, config: HostsConfig) -> Result<(), String> {
    save_config(&app, &config)?;
    crate::maybe_auto_sync(&app);
    Ok(())
}

#[tauri::command]
pub fn hosts_status(app: tauri::AppHandle) -> Result<HostsStatus, String> {
    let config = load_config(&app)?;
    let original = std::fs::read(system_hosts_path()).unwrap_or_default();
    let expected = render_block(&config);
    Ok(HostsStatus {
        up_to_date: is_up_to_date(&original, &expected),
    })
}

#[tauri::command]
pub async fn apply_hosts(app: tauri::AppHandle) -> Result<ApplyOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let config = load_config(&app)?;
        let original = std::fs::read(system_hosts_path()).unwrap_or_default();
        // 先把块外与 liaos 管理域名冲突的已有条目注释掉，再合入标记块，
        // 避免 Windows 解析命中块外旧行导致 liaos 配置不生效
        let neutralized = neutralize_conflicts(&original, &config);
        let merged = merge_block(&neutralized, &render_block(&config));
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = std::env::temp_dir().join(format!("liaos-hosts-{stamp}.tmp"));
        std::fs::write(&tmp_path, &merged).map_err(|e| format!("写临时文件失败：{e}"))?;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let outcome = match run_helper_elevated(&exe.to_string_lossy(), &tmp_path) {
            Ok(Some(0)) => ApplyOutcome {
                cancelled: false,
                message: "已应用到系统".into(),
            },
            Ok(Some(code)) => ApplyOutcome {
                cancelled: false,
                message: helper_exit_message(code),
            },
            Ok(None) => ApplyOutcome {
                cancelled: true,
                message: "已取消授权，未做任何更改".into(),
            },
            Err(e) => return Err(e),
        };
        // helper 正常路径已删临时文件，这里兜底（UAC 取消等场景）
        let _ = std::fs::remove_file(&tmp_path);
        Ok(outcome)
    })
    .await
    .map_err(|e| e.to_string())?
}
