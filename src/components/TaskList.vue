<script setup lang="ts">
import { NEmpty, NButton, NPopconfirm, NDropdown } from "naive-ui";
import type { Task, TaskStatus } from "../types";
import { STATUS_LABEL } from "../types";

defineProps<{ tasks: Task[] }>();

const emit = defineEmits<{
  (e: "edit", task: Task): void;
  (e: "delete", id: string): void;
  (e: "status", id: string, status: TaskStatus): void;
}>();

const statusOptions = (Object.keys(STATUS_LABEL) as TaskStatus[]).map((k) => ({
  label: STATUS_LABEL[k],
  key: k,
}));

function handleStatusSelect(task: Task, key: TaskStatus) {
  emit("status", task.id, key);
}
</script>

<template>
  <div v-if="tasks.length === 0" class="empty-wrap">
    <n-empty description="暂无待办事项，点击「新建」添加一条吧" />
  </div>

  <div v-else class="task-list">
    <div v-for="t in tasks" :key="t.id" class="task-card" :class="'s-' + t.status">
      <n-dropdown
        trigger="click"
        :options="statusOptions"
        @select="(key: TaskStatus) => handleStatusSelect(t, key)"
      >
        <button class="status-pill">
          <span class="dot"></span>
          {{ STATUS_LABEL[t.status] }}
        </button>
      </n-dropdown>

      <div class="task-body">
        <div
          class="task-title"
          :class="{ finished: t.status === 'done' || t.status === 'cancelled' }"
          @click="emit('edit', t)"
        >
          {{ t.title }}
        </div>
        <div class="task-meta">
          <span class="meta-cat">{{ t.category }}</span>
          <span v-if="t.plannedEndTime" class="meta-time">计划完成：{{ t.plannedEndTime }}</span>
        </div>
        <div v-if="t.status !== 'cancelled'" class="progress-track">
          <div class="progress-fill" :style="{ width: t.progress + '%' }"></div>
        </div>
      </div>

      <div class="task-actions">
        <n-button size="tiny" quaternary @click="emit('edit', t)">编辑</n-button>
        <n-popconfirm @positive-click="emit('delete', t.id)">
          <template #trigger>
            <n-button size="tiny" quaternary type="error">删除</n-button>
          </template>
          确认删除这条待办？
        </n-popconfirm>
      </div>
    </div>
  </div>
</template>

<style scoped>
.task-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px 24px;
}
.empty-wrap {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ===== 任务卡片 ===== */
.task-card {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: 16px 18px;
  margin-bottom: 10px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.task-card:hover {
  border-color: var(--primary);
  box-shadow: 0 4px 12px rgba(13, 148, 136, 0.1);
}

/* ===== 状态胶囊：圆点 + 文字，可点击切换 ===== */
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 11px;
  border-radius: 999px;
  border: none;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  flex-shrink: 0;
  white-space: nowrap;
  transition: filter 0.15s ease;
}
.status-pill:hover {
  filter: brightness(0.92);
}
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.s-todo .status-pill {
  color: var(--status-todo);
  background: color-mix(in srgb, var(--status-todo) 12%, transparent);
}
.s-doing .status-pill {
  color: var(--status-doing);
  background: color-mix(in srgb, var(--status-doing) 12%, transparent);
}
.s-done .status-pill {
  color: var(--status-done);
  background: color-mix(in srgb, var(--status-done) 12%, transparent);
}
.s-cancelled .status-pill {
  color: var(--status-cancelled);
  background: color-mix(in srgb, var(--status-cancelled) 12%, transparent);
}
.s-todo .dot {
  background: var(--status-todo);
}
.s-doing .dot {
  background: var(--status-doing);
}
.s-done .dot {
  background: var(--status-done);
}
.s-cancelled .dot {
  background: var(--status-cancelled);
}

/* ===== 内容区 ===== */
.task-body {
  flex: 1;
  min-width: 0;
}
.task-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-1);
  cursor: pointer;
  line-height: 1.5;
  word-break: break-all;
  transition: color 0.15s ease;
}
.task-title:hover {
  color: var(--primary);
}
.task-title.finished {
  color: var(--text-3);
  text-decoration: line-through;
}
.task-title.finished:hover {
  color: var(--primary);
}
.task-meta {
  display: flex;
  gap: 16px;
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-3);
}
.meta-cat {
  color: var(--text-2);
}

/* ===== 进度条（颜色跟随状态） ===== */
.progress-track {
  height: 4px;
  margin-top: 10px;
  border-radius: 2px;
  background: var(--border);
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.3s ease;
}
.s-todo .progress-fill {
  background: var(--status-todo);
}
.s-doing .progress-fill {
  background: var(--status-doing);
}
.s-done .progress-fill {
  background: var(--status-done);
}

.task-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}
</style>
