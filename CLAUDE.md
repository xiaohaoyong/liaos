# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

「了事」是一个 Windows 桌面待办事项软件，基于 Tauri 2.x。逐条记录待办（含实际开始/计划完成/实际完成时间、进度、分类、四态状态），常驻系统托盘，数据通过内嵌 git 库（`git2`）同步到用户自配的远程仓库（Gitee / GitHub / 自建服务器）。

- 前端：Vue 3 + TypeScript + Vite + Naive UI + Pinia
- 后端：Rust（`src-tauri/`），数据存于 `app_data_dir` 下的 `tasks.json` / `categories.json` / `settings.json`
- 应用内嵌 git 仓库（数据目录下的 `.git`），**不依赖系统 git**，HTTPS + Personal Access Token 认证

## 常用命令

```bash
npm run tauri dev      # 开发模式（Vite dev server + Tauri 窗口，端口 1420）
npm run build          # 前端类型检查 + 构建（vue-tsc --noEmit && vite build）
npm run tauri build    # 打包为 NSIS 安装包，完成后自动复制一份到项目根目录（posttauri 钩子 → scripts/copy-setup.js）
cd src-tauri && cargo check   # 仅编译检查 Rust 后端
```

项目当前没有单元测试框架；验证靠 `npm run tauri dev` 手动跑通 + `npm run build` 类型检查。

## 打包环境约束（务必遵守）

1. **打包必须在干净环境（PowerShell/cmd）**，不能走 Git Bash。Git Bash 的 `/mingw64/bin` 会导致 `cl.exe`/`rustc` 报 `0xc0000142`（STATUS_DLL_INIT_FAILED）。且重装后必须先用 vcvars64 加载 MSVC 环境（缺 INCLUDE/LIB 和 link.exe 会编译失败）。直接用现成脚本 `scripts/build-release.bat`：
   ```bat
   call scripts/build-release.bat
   ```
   脚本内部：设干净 PATH（`C:\Users\Administrator\.cargo\bin;D:\App\nodejs;C:\Windows\System32;C:\Windows`，nodejs 重装后在 D:\App 不在 E 盘）→ `call vcvars64.bat` → `npm run tauri build`。若手写 PowerShell 注意：cmd 的 `set` 输出解析 vcvars 环境易被引号拆坏，用 `.bat` 文件最稳。
   `npm run tauri dev` 有时能侥幸通过（libz-sys 命中缓存），但 release 打包首次真正调用 `cl.exe` 就会暴露，不要据此误判环境没问题。

2. **产品名是中文「了事」，只能打 NSIS 包**。MSI 无法把中文产品名写入代码页 1252，会报 `LGHT0311`。`tauri.conf.json` 里 `bundle.targets` 已固定为 `["nsis"]`，不要改回 `"all"` 或加 msi。

3. `vite.config.ts` 里 `base: './'` 是打包必需（Tauri 加载 `frontendDist` 需相对路径，缺了会白屏），不要删。

## 架构与数据流

**前后端数据契约是核心，改数据结构必须两边同步改**：
- `src/types.ts`（TS 类型）与 `src-tauri/src/storage.rs`（Rust struct）一一对应。Rust 侧靠 `#[serde(rename_all = "camelCase")]` 对齐 TS 的 camelCase；`TaskStatus` 枚举额外用 `#[serde(rename_all = "lowercase")]` 映射 `"todo"/"doing"/"done"/"cancelled"`。
- Tauri command 名是 snake_case（`load_data`、`save_tasks`、`sync_now`…），前端 `src/api.ts` 用 `invoke("load_data")` 统一封装；invoke 的参数 key 是 camelCase。

**一次变更的完整闭环**：前端 Pinia store（内存）变更 → `api.ts` 的 `invoke` → Rust command → `storage.rs` 写 JSON → `maybe_auto_sync`（`lib.rs` 里 `async_runtime::spawn` 后台异步，不阻塞 UI）→ `sync.rs` 的 `sync()`（pull → commit → push）。

**启动流程**：`lib.rs` 的 `setup` 里创建托盘 + `notify_unfinished` 弹开机通知；前端 `App.vue` 的 `onMounted` 一次性 `load_data` 填充 tasks/categories/settings 三个 store。

**托盘与前端通信靠事件，不走 command**：`tray.rs` 菜单点击后 `app.emit("tray-create" / "tray-sync" / "tray-settings")`，`App.vue` 用 `@tauri-apps/api/event` 的 `listen` 监听。"打开主界面"和"新建待办"都是先 `show_main_window` 再发事件。

**窗口关闭 = 最小化到托盘**：`lib.rs` 的 `on_window_event` 拦截 `CloseRequested`，执行 `window.hide()` + `api.prevent_close()`。程序真正退出只能通过托盘菜单「退出程序」（`app.exit(0)`）。

**git 同步策略**（`sync.rs`，单人使用场景）：先 commit 再 pull（pull 的 checkout 会重置工作区）。pull 时通过 `remote.default_branch()` 兼容 master/main。三种情形：①本地空库或本地历史与远程**无共同祖先**（新机器/数据目录重建）→ 以远程为基准接管（tasks/categories 按 id 合并两边条目、同 id 任务取 updatedAt 较新者，settings.json 始终用本机版，远程没有的本地文件如 hosts.json 恢复保留，再补一个提交保证 push 为 fast-forward）；②有共同祖先且能合并 → 正常合并（merge commit 必须双亲，单亲会截断历史导致 push 被拒）；③合并冲突 → 以本地为准（`reset --hard`），下次 push 覆盖远程。拉到远程新内容后 `sync` 返回 `pulled` 标志，`lib.rs` 据此 emit `data-changed` 让前端重载。commit message 用时间戳。

## 已知易踩的坑

- **Naive UI 消息组件**：`App.vue` 自身 setup 里调用 `useMessage()` 会抛 `No outer <n-message-provider /> founded`——因为 `n-message-provider` 是"未来才渲染的子组件"，provide 尚未执行。所以 `App.vue` 用 `createDiscreteApi(["message"])`，而 `n-message-provider` 仍保留包裹（供 `TaskEditModal` / `SettingsModal` 等子组件用 `useMessage()`）。新增顶层逻辑不要回到 `useMessage`。
- **残留 Vite 进程会导致白屏**：若 dev 时页面全白且报 `esbuild: The service is no longer running`，多半是旧的 Vite node 子进程残留。先 `taskkill` 残留的 node 进程再重启 dev，只杀 Tauri 进程不够。
