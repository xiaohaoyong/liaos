<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { loadNotes } from "./api";
import { useNotesStore } from "./stores/notes";
import { NOTE_COLORS, NOTE_COLOR_MAP } from "./types";
import type { Note, NoteColor } from "./types";

// 便利贴窗口本体：一张常驻桌面的小纸片。纯手写 UI（不引 Naive UI——
// 每条便签一个 WebView 进程，保持轻量），也不跟随深色模式（纸片恒为亮色）。
const notesStore = useNotesStore();
const win = getCurrentWindow();

// 自己的 id 与是否抢焦点（新建/定位补建时 focus=1，启动恢复不抢）
const params = new URLSearchParams(window.location.search);
const id = params.get("id") ?? "";
const wantFocus = params.get("focus") === "1";

const content = ref("");
const color = ref<NoteColor>("yellow");
const pinned = ref(false);
const ta = ref<HTMLTextAreaElement | null>(null);

const paperStyle = computed(() => {
  const c = NOTE_COLOR_MAP[color.value];
  return { background: c.bg, borderBottomColor: c.edge };
});

// ===== 高度自适应：量内容高度 → 调整窗口高度 =====
const GRIP_H = 26; // 顶部拖拽/工具条高度
// 默认窗口高度（与 Rust 建窗的 PLACEHOLDER_H 一致）：内容没写满默认高度时窗口
// 保持这个高度不缩；写满后内容每折一行才增高一行（scrollHeight 本身按行累进，
// 行内敲字不变，天然就是行粒度）。窗口高度始终 >= 内容所需，textarea 不出
// 滚动条；仅当内容超过屏幕可用高度时才 clamp，此时内容区内部滚动。
const DEFAULT_H = 160;

let lastW = 0; // 最近窗口逻辑宽（onResized 里维护）
let lastH = 0; // 最近窗口逻辑高（防 setSize 回环）
let measureQueued = false;

function measure() {
  const el = ta.value;
  if (!el || !lastW) return;
  // 必须先退出 flex 拉伸再归零：flex 子项的 inline height 会被 flex-grow 覆盖，
  // 只置 height:0 时 scrollHeight 读到的是"当前可视高"而非内容高，会形成
  // needed = 当前窗口高 + 4 的恒等式，每敲一个字窗口就长 4px。
  el.style.flex = "none";
  el.style.height = "0";
  const contentH = el.scrollHeight; // 此时才是真实内容高度
  el.style.flex = "";
  el.style.height = ""; // 恢复 flex 拉伸（同帧内改回，无闪烁）
  const maxH = window.screen.availHeight - 40; // 超屏则内容区内部滚动
  const needed = Math.min(Math.max(GRIP_H + contentH + 4, DEFAULT_H), maxH);
  if (Math.abs(needed - lastH) > 2) {
    lastH = needed;
    win.setSize(new LogicalSize(lastW, needed)).catch(() => {});
  }
}

function queueMeasure() {
  if (measureQueued) return;
  measureQueued = true;
  requestAnimationFrame(() => {
    measureQueued = false;
    measure();
  });
}

// ===== 编辑保存：800ms 防抖 + blur 即存 =====
let saveTimer: number | undefined;

function onInput() {
  queueMeasure();
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => notesStore.patch(id, { content: content.value }), 800);
}

function onBlur() {
  window.clearTimeout(saveTimer);
  notesStore.patch(id, { content: content.value });
}

// 正在编辑（textarea 聚焦）时跳过外部刷新，防止打断输入
const isEditing = () => document.activeElement === ta.value;

// ===== 位置/宽度持久化（store 内 600ms 防抖） =====
win.onMoved(async (e) => {
  const s = await win.scaleFactor();
  notesStore.updateGeometry(id, { x: e.payload.x / s, y: e.payload.y / s });
});

win.onResized(async (e) => {
  const s = await win.scaleFactor();
  const w = e.payload.width / s;
  lastH = e.payload.height / s;
  if (Math.abs(w - lastW) > 1) {
    lastW = w;
    notesStore.updateGeometry(id, { width: w });
    queueMeasure(); // 宽度变化 → 文本重排 → 高度可能变
  }
});

// 右缘拖宽手柄（高度由内容驱动，不提供纵向手柄；"East" 是 ResizeDirection 字面量联合）
function startResize() {
  win.startResizeDragging("East");
}

// ===== 工具条动作 =====
function pickColor(c: NoteColor) {
  color.value = c;
  notesStore.patch(id, { color: c });
}

async function togglePin() {
  pinned.value = !pinned.value;
  try {
    await win.setAlwaysOnTop(pinned.value);
    notesStore.patch(id, { pinned: pinned.value });
  } catch (e) {
    console.error("设置置顶失败", e);
    pinned.value = !pinned.value;
  }
}

// 删除：两段式确认（首击亮红「确认删除？」，3 秒内二击生效）
const confirmDelete = ref(false);
let confirmTimer: number | undefined;

function armDelete() {
  if (confirmDelete.value) {
    doDelete();
  } else {
    confirmDelete.value = true;
    window.clearTimeout(confirmTimer);
    confirmTimer = window.setTimeout(() => (confirmDelete.value = false), 3000);
  }
}

async function doDelete() {
  try {
    // Rust 端删数据后销毁本窗口；invoke 可能因窗口销毁不再返回，属正常
    await notesStore.remove(id);
  } catch (e) {
    console.error("删除便利贴失败", e);
  }
}

// ===== 初始化与跨窗口事件 =====
function applyRecord(note: Note) {
  content.value = note.content;
  color.value = note.color;
  pinned.value = note.pinned;
  queueMeasure();
}

async function reloadSelf(): Promise<Note | null> {
  const me = (await loadNotes()).find((n) => n.id === id) ?? null;
  if (me) applyRecord(me);
  return me;
}

onMounted(async () => {
  await notesStore.load();
  const me = notesStore.notes.find((n) => n.id === id);
  if (!me) {
    await win.destroy(); // 记录已不存在（删除竞态/同步删除）：自愈销毁
    return;
  }
  applyRecord(me);
  lastW = me.width;
  measure();
  await win.show(); // 量完高度才显示，避免闪变
  if (wantFocus) win.setFocus().catch(() => {});

  // 其他窗口改了这条便签（编辑弹窗/同步合并）：非编辑态时应用
  await listen<string>("notes-changed", async (e) => {
    if (e.payload !== id || isEditing()) return;
    if (!(await reloadSelf())) await win.destroy(); // 自己被删除（destroy 失败兜底）
  });

  // 自动同步拉到远程新内容：非编辑态时应用
  await listen("data-changed", async () => {
    if (isEditing()) return;
    await reloadSelf();
  });

  // Alt+F4（lib.rs 转发）：走两段式删除确认，不确认则窗口留在桌面
  await listen("note-close-requested", () => armDelete());
});
</script>

<template>
  <div class="note" :style="paperStyle">
    <div class="note-grip" data-tauri-drag-region>
      <div class="note-tools" data-tauri-drag-region="false">
        <span
          v-for="c in NOTE_COLORS"
          :key="c"
          class="color-dot"
          :class="{ active: color === c }"
          :style="{ background: NOTE_COLOR_MAP[c].bg, borderColor: NOTE_COLOR_MAP[c].edge }"
          title="换这个颜色"
          @click="pickColor(c)"
        />
        <button class="tool-btn" :class="{ on: pinned }" @click="togglePin">
          {{ pinned ? "已置顶" : "置顶" }}
        </button>
        <button class="tool-btn" :class="{ danger: confirmDelete }" @click="armDelete">
          {{ confirmDelete ? "确认删除？" : "删除" }}
        </button>
      </div>
    </div>
    <textarea
      ref="ta"
      v-model="content"
      class="note-input"
      placeholder="写点什么…"
      @input="onInput"
      @blur="onBlur"
    />
    <div class="resize-e" title="拖动调整宽度" @mousedown="startResize" />
  </div>
</template>

<style scoped>
.note {
  position: relative;
  box-sizing: border-box; /* 100vh 含 4px 底边框，避免溢出文档层 */
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: 3px;
  border-bottom: 4px solid; /* borderBottomColor 由 paperStyle 提供（纸片厚度感） */
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.16);
  color: #423f38; /* 固定墨色，不跟随深色模式 */
}

.note-grip {
  flex: 0 0 26px;
  display: flex;
  justify-content: flex-end;
  align-items: center;
  padding: 0 6px;
  cursor: grab;
  user-select: none;
}

.note-tools {
  display: none;
  gap: 4px;
  align-items: center;
}
.note:hover .note-tools {
  display: flex;
}

.color-dot {
  width: 13px;
  height: 13px;
  border-radius: 50%;
  cursor: pointer;
  border: 1px solid rgba(0, 0, 0, 0.15);
  transition: transform 0.1s ease;
}
.color-dot:hover {
  transform: scale(1.15);
}
.color-dot.active {
  outline: 2px solid rgba(0, 0, 0, 0.4);
  outline-offset: 1px;
}

.tool-btn {
  border: none;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 4px;
  font-size: 11px;
  line-height: 1;
  padding: 4px 6px;
  color: #423f38;
  cursor: pointer;
  white-space: nowrap;
}
.tool-btn:hover {
  background: rgba(0, 0, 0, 0.12);
}
.tool-btn.on {
  background: rgba(0, 0, 0, 0.18);
  font-weight: 600;
}
.tool-btn.danger {
  background: #c0392b;
  color: #fff;
}

.note-input {
  flex: 1;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 14px;
  line-height: 1.6;
  padding: 2px 12px 12px;
  overflow-y: auto;
}

.resize-e {
  position: absolute;
  top: 0;
  right: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
}
</style>
