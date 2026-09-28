<script setup lang="ts">
import { reactive, ref } from "vue";
import {
  NButton,
  NEmpty,
  NInput,
  NModal,
  NPopconfirm,
  NSpace,
  useMessage,
} from "naive-ui";
import type { Note } from "../types";
import { NOTE_COLOR_MAP } from "../types";
import { useNotesStore } from "../stores/notes";

// 便利贴管理区：主窗口侧边栏「便利贴」视图。
// 查看/新建/编辑（大段文本）/定位（找回屏幕外便签）/删除。
const message = useMessage();
const notesStore = useNotesStore();

// 编辑弹窗：在主窗口里编辑大段文本比小纸片上舒服
const showEdit = ref(false);
const editingId = ref<string | null>(null);
const editForm = reactive({ content: "" });

function openEdit(note: Note) {
  editingId.value = note.id;
  editForm.content = note.content;
  showEdit.value = true;
}

function submitEdit() {
  if (editingId.value) {
    notesStore.patch(editingId.value, { content: editForm.content });
  }
  showEdit.value = false;
}

async function handleCreate() {
  try {
    await notesStore.create();
  } catch (e) {
    message.error("新建便利贴失败：" + e);
  }
}

async function handleRelocate(id: string) {
  try {
    await notesStore.relocate(id);
    message.success("已定位到屏幕可见区域");
  } catch (e) {
    message.error("定位失败：" + e);
  }
}

async function handleDelete(id: string) {
  try {
    await notesStore.remove(id);
  } catch (e) {
    message.error("删除失败：" + e);
  }
}
</script>

<template>
  <div class="notes-panel">
    <header class="panel-header">
      <div class="panel-intro">常驻桌面的小纸片；置顶后不受快捷键收起影响</div>
      <n-button type="primary" @click="handleCreate">新建便利贴</n-button>
    </header>

    <div class="panel-body">
      <n-empty
        v-if="notesStore.notes.length === 0"
        description="还没有便利贴"
        class="panel-empty"
      />
      <div v-else class="note-cards">
        <div v-for="n in notesStore.notes" :key="n.id" class="note-card">
          <span
            class="card-color"
            :style="{ background: NOTE_COLOR_MAP[n.color]?.bg ?? '#fcf3c5' }"
          ></span>
          <div class="card-main">
            <div class="card-content">{{ n.content || "（空白）" }}</div>
            <div class="card-meta">
              {{ n.pinned ? "已置顶 · " : "" }}更新于
              {{ n.updatedAt.replace("T", " ").slice(0, 16) }}
            </div>
          </div>
          <n-space size="small" class="card-actions">
            <n-button size="tiny" quaternary @click="openEdit(n)">编辑</n-button>
            <n-button size="tiny" quaternary @click="handleRelocate(n.id)">定位</n-button>
            <n-popconfirm @positive-click="handleDelete(n.id)">
              <template #trigger>
                <n-button size="tiny" quaternary type="error">删除</n-button>
              </template>
              确定删除这条便利贴？
            </n-popconfirm>
          </n-space>
        </div>
      </div>
    </div>

    <n-modal
      v-model:show="showEdit"
      preset="card"
      title="编辑便利贴"
      style="width: 480px; max-width: 90vw"
    >
      <n-input
        v-model:value="editForm.content"
        type="textarea"
        placeholder="写点什么…"
        :autosize="{ minRows: 8, maxRows: 20 }"
      />
      <template #footer>
        <n-space justify="end">
          <n-button @click="showEdit = false">取消</n-button>
          <n-button type="primary" @click="submitEdit">保存</n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.notes-panel {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 24px;
}

.panel-intro {
  font-size: 13px;
  color: var(--text-3);
}

.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 0 24px 24px;
}

.panel-empty {
  margin-top: 80px;
}

.note-cards {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.note-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 8px;
}

.card-color {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  flex-shrink: 0;
  border: 1px solid var(--border);
}

.card-main {
  flex: 1;
  min-width: 0;
}

.card-content {
  font-size: 13px;
  color: var(--text-1);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-wrap;
  word-break: break-all;
}

.card-meta {
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-3);
}
</style>
