import { defineStore } from "pinia";
import { ref } from "vue";
import type { Note, NotePatch } from "../types";
import { loadNotes, createNote, updateNote, deleteNote, relocateNote } from "../api";

// 便利贴（notes.json）。多个便签窗口与主窗口管理区各持一份实例，
// 写入一律走局部 patch（Rust 端按 id 合并到磁盘最新版），不做整表覆盖。
export const useNotesStore = defineStore("notes", () => {
  const notes = ref<Note[]>([]);

  // 最近一次持久化 Promise（模式同 stores/hosts.ts：需要读后写的地方先 flush）
  let lastPersist: Promise<void> = Promise.resolve();
  function flush(): Promise<void> {
    return lastPersist;
  }

  async function load() {
    notes.value = await loadNotes();
  }

  function setNotes(list: Note[]) {
    notes.value = list;
  }

  // 新建：Rust 端一条龙（写数据 + 建窗），成功后本地置顶展示
  async function create(): Promise<Note> {
    const note = await createNote();
    notes.value = [note, ...notes.value];
    return note;
  }

  // 内容/颜色/置顶变更：本地即时生效 + 落盘（输入防抖由调用方控制）
  function patch(id: string, p: NotePatch) {
    const n = notes.value.find((n) => n.id === id);
    if (n) Object.assign(n, p);
    lastPersist = updateNote(id, p).catch((e) => console.error("保存便利贴失败", e));
  }

  // ===== 几何（位置/宽度）：防抖合并落盘 =====
  // 拖动期间 onMoved 高频触发，若每次直存会触发 git commit+pull+push 风暴，
  // 静默 600ms 后每条便签只落一次盘。
  let geoTimer: number | undefined;
  const geoPending = new Map<string, NotePatch>();

  function updateGeometry(id: string, geo: NotePatch) {
    const n = notes.value.find((n) => n.id === id);
    if (n) Object.assign(n, geo);
    geoPending.set(id, { ...(geoPending.get(id) ?? {}), ...geo });
    window.clearTimeout(geoTimer);
    geoTimer = window.setTimeout(() => {
      for (const [nid, g] of geoPending) {
        lastPersist = updateNote(nid, g).catch((e) => console.error("保存便利贴位置失败", e));
      }
      geoPending.clear();
    }, 600);
  }

  // 删除：Rust 端删数据 + 关窗 + 广播
  async function remove(id: string) {
    await deleteNote(id);
    notes.value = notes.value.filter((n) => n.id !== id);
  }

  // 定位：位置以 Rust clamp 后的落盘值为准，回读刷新
  async function relocate(id: string) {
    await relocateNote(id);
    await load();
  }

  return { notes, load, setNotes, create, patch, updateGeometry, remove, relocate, flush };
});
