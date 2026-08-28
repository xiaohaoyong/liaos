import { defineStore } from "pinia";
import { ref } from "vue";
import type { Task, TaskStatus } from "../types";
import { saveTasks } from "../api";
import { now } from "../utils";

export const useTasksStore = defineStore("tasks", () => {
  const tasks = ref<Task[]>([]);

  function setTasks(list: Task[]) {
    tasks.value = list;
  }

  // 持久化到 Rust 端（写 tasks.json + 触发同步）
  function persist() {
    saveTasks(tasks.value).catch((e) => console.error("保存任务失败", e));
  }

  function add(task: Task) {
    tasks.value.unshift(task);
    persist();
  }

  function update(task: Task) {
    const idx = tasks.value.findIndex((t) => t.id === task.id);
    if (idx >= 0) {
      tasks.value[idx] = { ...task, updatedAt: now() };
      persist();
    }
  }

  function remove(id: string) {
    tasks.value = tasks.value.filter((t) => t.id !== id);
    persist();
  }

  function setStatus(id: string, status: TaskStatus) {
    const t = tasks.value.find((t) => t.id === id);
    if (!t) return;
    t.status = status;
    if (status === "done") {
      t.progress = 100;
      t.actualEndTime = now(); // 完成时间始终取点击完成的时刻
    }
    update(t);
  }

  function setProgress(id: string, progress: number) {
    const t = tasks.value.find((t) => t.id === id);
    if (!t) return;
    t.progress = Math.max(0, Math.min(100, Math.round(progress)));
    if (t.progress === 100 && t.status !== "cancelled") {
      t.status = "done";
      t.actualEndTime = now(); // 拖满进度同样视为一次完成动作，取当前时刻
    }
    update(t);
  }

  // 未完成任务数量（用于开机通知）
  function unfinishedCount(): number {
    return tasks.value.filter((t) => t.status === "todo" || t.status === "doing").length;
  }

  return { tasks, setTasks, add, update, remove, setStatus, setProgress, unfinishedCount };
});
