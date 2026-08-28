// 打包完成后，把生成的 NSIS 安装包复制到项目根目录，并改用英文文件名对外分发。
// 由 package.json 的 posttauri 钩子触发：npm run tauri build 跑完后自动执行。
// npm run tauri dev 也会触发本脚本，但此时安装包不存在，会静默跳过。
import { copyFileSync, existsSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// 从 tauri.conf.json 读取产品名和版本号
const conf = JSON.parse(
  readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'),
);
// Tauri 生成的安装包文件名（中文产品名，如 了事_0.1.0_x64-setup.exe）
const srcName = `${conf.productName}_${conf.version}_x64-setup.exe`;
// 对外分发的文件名用英文，GitHub Release 下载链接无需 URL 编码（如 liaos_0.1.0_x64-setup.exe）
const destName = `liaos_${conf.version}_x64-setup.exe`;

const src = join(
  root,
  'src-tauri',
  'target',
  'release',
  'bundle',
  'nsis',
  srcName,
);
const dest = join(root, destName);

if (existsSync(src)) {
  copyFileSync(src, dest);
  console.log(`✅ 已复制安装包到项目根目录：${destName}`);
} else {
  // dev 或 build 失败时安装包不存在，静默跳过，不影响原有流程
  process.exit(0);
}
