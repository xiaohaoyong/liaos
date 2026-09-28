# 便利贴（桌面常驻便签）模块

## 业务概述

独立于任务体系的「实物便利贴」：每条便签是桌面上一个无边框透明小窗口（纸片样式），开机常驻显示，无开始/结束时间/进度/分类等任务属性，只有大段文字 + 颜色 + 位置 + 置顶。UI 文案叫「便利贴」（与快捷待办悬浮窗的「便签」区分），代码/数据模块叫 `notes`。

## 核心实体

`notes.json`（顶层数组），字段与 `src/types.ts` 的 `Note` 一一对应：

| 字段 | 类型 | 含义 |
|------|------|------|
| id | string | `n{纳秒hex}-{pid hex}`，Rust 生成（托盘新建时前端不在场） |
| content | string | 文字内容 |
| color | string | 6 色板：yellow/pink/blue/green/purple/gray |
| x / y | f64 | 窗口左上角**逻辑坐标**（物理像素 ÷ scaleFactor 换算） |
| width | f64 | 逻辑宽度，clamp [180, 800]；**高度不持久化**（内容自适应） |
| pinned | bool | 置顶于所有应用之上；置顶便签不受快捷键收起影响 |
| createdAt / updatedAt | string | ISO 8601 毫秒 + Z，Rust 与 JS `toISOString()` 逐字符同格式（同步合并按字符串择新） |

## 当前业务规则

- **新建**（三入口：托盘菜单 / 快捷待办列表底部按钮 / 主窗口侧边栏管理区）：色板按现存数量轮换、位置 (80,80) 起按数量 32px 级联（模 10 折返）；写盘 → 建窗（`visible(false)` + 前端量高后 `show()` 防闪变）→ auto_sync → 广播 `notes-changed`
- **高度自适应**：默认窗口高 160（内容不满时不缩不涨）；写满后每折一行长一行（`scrollHeight` 行粒度）；上限 `screen.availHeight - 40`（超屏则内容区内部滚动）
- **更新**：`update_note(id, patch)` 读磁盘最新版按 id 局部合并（多便签窗口各持副本，防整表覆盖竞态）；content/color/pinned 变更广播 `notes-changed`（payload=变更 id），**纯几何变更不广播**；updatedAt 一律刷新（同步冲突时位置变更才能胜出）
- **编辑保护**：textarea 聚焦（编辑中）的窗口跳过外部刷新事件；800ms 防抖 + blur 即存
- **几何持久化**：拖动/拖宽 → onMoved/onResized（物理像素÷scaleFactor）→ store 内 600ms 防抖落盘
- **删除**：两段式确认（首击亮红「确认删除？」3 秒内二击生效）；先落盘再 `destroy()`；Alt+F4 转发为 `note-close-requested` 事件同样走两段式确认
- **启动恢复**：`setup` 里 `create_note_windows` 批量建窗，单窗失败仅日志容错；启动时 `clamp_note_position` 保护点（左上角内缩 min(width,60)×40）不在任一显示器逻辑矩形内则拉回主屏 +60,+60
- **快捷键联动**（toggle_sticky）：收起分支隐藏非置顶便利贴；呼出分支 show + unminimize（**不置顶**，区别于便签悬浮窗）；置顶便利贴两分支都不动
- **git 同步**：notes.json 随 `add_all(["*"])` 自动纳入 commit/push；pull 合并走 `merge_array_file` 按 id 合并、updatedAt 字符串比较择新；远程没有 notes.json 时 restore_local_files 恢复本地版
- **自愈**：便签窗口发现自己的 id 不在数据里（删除竞态/同步删除）→ 自行 `destroy()`

## 历史变更记录

- **2026-09-28 v0.2.0**：模块随便利贴功能新增。调试期三处行为修正：①高度自适应改为「默认 160 内不动、写满后按行增长」（原 MIN_H=100 下限导致窗口先缩后频繁增长）；②测量时先 `flex:none` 再归零（flex 子项 inline height 被 flex-grow 覆盖，scrollHeight 读到的是可视高，形成每敲一字长 4px 的恒等式）；③文档层 `overflow:hidden` + body margin 清零 + `.note` border-box（body 8px margin + 100vh/4px 边框溢出文档，滚动条常驻）
- **2026-09-27**：新建便利贴无反应的根因——Tauri 2 同步 command 在主线程执行，`build()` 等主线程 = 自死锁，且死锁后所有 invoke 不再响应；`create_note` / `relocate_note` 改 async command 修复

## 常用查询场景

- 启动恢复全部便签：`notes::create_note_windows(app)`（读 `load_notes_internal`）
- 取置顶集合（快捷键联动过滤）：`notes::pinned_note_ids(app) -> HashSet<String>`
- 管理区列表/编辑/定位：前端 `stores/notes.ts` 的 `load/patch/relocate/remove`

## 已知坑点

- **command 里建窗必须 async**：同步 command 跑主线程，`WebviewWindowBuilder::build()` 需要主线程事件循环，同步调用 = 死锁（本次 v0.2.0 的核心教训；唤醒已有窗口的 show/hide/set_focus 不受影响）
- **隐藏的 WebView2 会被系统挂起**：不执行 JS、收不到 Vite HMR reload；取证/驱动隐藏窗口前端时不能依赖其自发性
- **taskkill 强杀 app 会留孤儿 msedgewebview2 进程**（父进程死、browser 组存活），调试重启后建议清理，否则环境判断会被干扰
- **待查（未解决）**：2026-09-27 调试中观察到一次「旧便利贴记录在自动同步后从 notes.json 消失、HEAD 提交内容为 `[]`」的数据丢失现象，根因未定位（疑与 sync.rs 的 pull/checkout 重置时序有关）；在多设备同步场景使用便利贴需注意，单机 auto_sync 正常场景未复现
- 便签窗口的 `document.title` 不会同步到 Win32 窗口标题，别用改 title 的方式做前端取证

## 相关文件

- Rust：`src-tauri/src/notes.rs`（数据/窗口/commands）、`src-tauri/src/sticky.rs`（快捷键联动）、`src-tauri/src/tray.rs`（托盘入口）、`src-tauri/src/lib.rs`（setup 恢复 + CloseRequested 分流）
- 前端：`note.html` / `src/note.ts` / `src/NoteApp.vue`（窗口本体）、`src/components/NotesPanel.vue`（管理区）、`src/stores/notes.ts`、`src/types.ts`（Note/NoteColor/NOTE_COLOR_MAP）
- 配置：`vite.config.ts`（MPA 入口）、`src-tauri/capabilities/default.json`（`note-*` glob + set-size/start-resize-dragging/destroy 权限）、`src/styles.css`（透明 + 文档层钳制）
