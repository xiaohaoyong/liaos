# 了事

一款 Windows 桌面待办事项软件，常驻系统托盘，数据通过内嵌 git 库自动同步到你自己的远程仓库（Gitee / GitHub / 自建服务器），完全掌控自己的数据。

## 功能特性

- 逐条记录待办事项，包含 **实际开始 / 计划完成 / 实际完成** 时间
- 进度、分类、**四态状态**（待办 / 进行中 / 已完成 / 已取消）
- **桌面便利贴**：常驻桌面的小纸片，六色可选、自由定位、可单独置顶于所有窗口之上，随数据一起 git 同步
- 快捷待办列表（Ctrl+Shift+Space 呼出），与便利贴共用快捷键统一收起 / 恢复
- 常驻系统托盘，一键新建待办、新建便利贴、快捷同步、打开设置
- 数据存在本地 `app_data_dir`，通过内嵌 git 库同步，**不依赖系统 git**
- 关闭窗口即最小化到托盘，不打扰

## 下载安装

**最新版**：[前往 Releases 页面下载](https://github.com/xiaohaoyong/liaos/releases/latest)

> 安装包通过 GitHub Releases 发布。下载 `.exe` 双击安装即可，无需任何依赖环境。

历史版本见下方 [版本历史](#版本历史)。

## 版本历史

| 版本 | 说明 | 下载 |
|------|------|------|
| [v0.2.0](https://github.com/xiaohaoyong/liaos/releases/tag/v0.2.0) | 新增桌面便利贴 | [liaos_0.2.0_x64-setup.exe](https://github.com/xiaohaoyong/liaos/releases/download/v0.2.0/liaos_0.2.0_x64-setup.exe) |
| [v0.1.0](https://github.com/xiaohaoyong/liaos/releases/tag/v0.1.0) | 首个发布版 | [liaos_0.1.0_x64-setup.exe](https://github.com/xiaohaoyong/liaos/releases/download/v0.1.0/liaos_0.1.0_x64-setup.exe) |

## 使用说明

- 首次启动后可在设置里配置远程仓库地址与访问令牌，之后每次数据变更会自动同步
- 数据目录位于系统应用数据目录下（`tasks.json` / `categories.json` / `settings.json` / `notes.json`），内部维护一个 git 仓库用于同步与备份
- 便利贴：开机后自动恢复到桌面原位置；内容写满默认高度后每折一行自动长高；置顶的便利贴不受快捷键收起影响

## 开发与构建

环境要求：

- Node.js（项目使用 `D:\App\nodejs` 的 node）
- Rust 工具链 + MSVC（VS Build Tools，打包前需加载 vcvars64）
- npm

常用命令：

```bash
npm run tauri dev        # 开发模式（Vite dev server + Tauri 窗口，端口 1420）
npm run build            # 前端类型检查 + 构建
scripts/build-release.bat  # 打包 NSIS 安装包（须在 PowerShell / cmd 干净环境执行，勿走 Git Bash）
```

打包完成后安装包自动复制到项目根目录，文件名形如 `liaos_0.1.0_x64-setup.exe`。

## 技术栈

- 前端：Vue 3 + TypeScript + Vite + Naive UI + Pinia
- 后端：Rust + Tauri 2
- 数据同步：内嵌 git（`git2` crate），HTTPS / SSH + 令牌认证

## 开源许可

[MIT](LICENSE)（暂未添加 LICENSE 文件，后续补充）
