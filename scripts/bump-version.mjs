// 版本号管理脚本：以命令行传入的 semver 版本号为单一事实来源，
// 同步更新 package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml 三处版本号。
// 用法：node scripts/bump-version.mjs 0.2.0
//       node scripts/bump-version.mjs patch   （major | minor | patch 递增当前版本）
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const SEMVER = /^\d+\.\d+\.\d+$/;

const arg = process.argv[2];
if (!arg) {
  console.error('用法：node scripts/bump-version.mjs <版本号>  或  major | minor | patch');
  process.exit(1);
}

// 读取当前 package.json 版本号（单一事实源）
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

/** 计算新版本号：显式 semver 或按 major/minor/patch 递增 */
function resolveNext(raw, current) {
  if (SEMVER.test(raw)) return raw;
  const [major, minor, patch] = current.split('.').map(Number);
  if (raw === 'major') return `${major + 1}.0.0`;
  if (raw === 'minor') return `${major}.${minor + 1}.0`;
  if (raw === 'patch') return `${major}.${minor}.${patch + 1}`;
  throw new Error(`无效版本参数：${raw}（需要 semver 或 major/minor/patch）`);
}

const version = resolveNext(arg, pkg.version);

/** 只替换 version 字段文本（保持原文件格式，避免 JSON.stringify 重排），并校验仍为合法 JSON */
function syncVersionText(relPath, version) {
  const file = join(root, relPath);
  const raw = readFileSync(file, 'utf8');
  const next = raw.replace(/"version": "[\d.]+"/, `"version": "${version}"`);
  JSON.parse(next); // 校验仍是合法 JSON，防止误替换
  writeFileSync(file, next, 'utf8');
  console.log(`✅ 已更新 ${relPath} → ${version}`);
}

// 1. package.json（单一事实源）
syncVersionText('package.json', version);

// 2. src-tauri/tauri.conf.json
syncVersionText('src-tauri/tauri.conf.json', version);

// 3. src-tauri/Cargo.toml（tauri-app 与 tauri_app_lib 两个 package 的 version）
const cargoPath = join(root, 'src-tauri', 'Cargo.toml');
const cargo = readFileSync(cargoPath, 'utf8').replace(
  /^version = "[\d.]+"$/gm,
  `version = "${version}"`,
);
writeFileSync(cargoPath, cargo, 'utf8');
console.log(`✅ 已更新 src-tauri/Cargo.toml → ${version}`);
