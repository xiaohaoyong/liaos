---
name: release
description: 发布「了事」新版本：更新版本号 → 打包 → git 提交打 tag → 推送 → 创建 GitHub Release 上传安装包 → 更新 README 下载链接。触发词：发布、发版、release、发新版本。
---

# 发布「了事」新版本

按顺序执行，每步成功后再进入下一步。发布全程需要网络（推送 + GitHub API）。

## 0. 前置检查

- 确认 gh CLI 已认证：`gh auth status`（未认证先 `gh auth login`）
- 确认 git 远程就绪：`git remote -v`（应为 `git@github.com:xiaohaoyong/liaos.git`）

## 1. 确定新版本号

- 用户指定版本号，或由 Claude 根据改动范围建议递增位（新增功能 → minor，修复 bug → patch，破坏性变更 → major），**提交用户确认后再执行**
- 版本号遵循 semver（`x.y.z`）

## 2. 更新版本号

```bash
node scripts/bump-version.mjs <新版本号 | major | minor | patch>
```

同步更新三处：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`。运行后 grep 三处确认版本号一致。

## 3. 打包

必须在干净环境（PowerShell / cmd），**不能走 Git Bash**（`/mingw64/bin` 会使 cl.exe/rustc 报 `0xc0000142`）：

```bash
scripts/build-release.bat
```

- 脚本内部先 `call vcvars64.bat` 加载 MSVC 环境，再执行 `npm run tauri build`
- 打包成功后在项目根目录生成 `liaos_<version>_x64-setup.exe`（posttauri 钩子复制并重命名为英文文件名）

## 4. 提交与打标签

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml README.md
git commit -m "release: v<version>"
git tag v<version>
git push origin main --tags
```

## 5. 创建 GitHub Release

```bash
gh release create v<version> \
  "liaos_<version>_x64-setup.exe" \
  --title "v<version>" \
  --notes "<本次主要变更摘要>"
```

## 6. 更新 README

在 [README.md](../../../README.md) 的「版本历史」表格**顶部**插入新版本行：

```markdown
| [v<version>](https://github.com/xiaohaoyong/liaos/releases/tag/v<version>) | <变更摘要> | [liaos_<version>_x64-setup.exe](https://github.com/xiaohaoyong/liaos/releases/download/v<version>/liaos_<version>_x64-setup.exe) |
```

并提交推送 README 变更。

## 7. 验证

- `gh release view v<version>` 确认 Release 与资产存在
- 确认 README 中的下载链接可访问

## 注意事项

- 安装包文件名是英文 `liaos_...`，但应用内产品名仍是中文「了事」，勿混用
- 版本号三处必须一致，以 `package.json` 为单一事实源
- 若 Release 创建失败（如已存在同名 tag），先 `gh release view v<version>` 排查
