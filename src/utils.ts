// 通用工具函数

// 生成唯一 ID（优先用 crypto.randomUUID）
export function newId(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  // 降级方案
  return "id-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 10);
}

// 当前时间（ISO 8601）
export function now(): string {
  return new Date().toISOString();
}

// 创建一条新的空任务（用于新建）
export function createEmptyTask(category: string): import("./types").Task {
  const t = now();
  return {
    id: newId(),
    title: "",
    category,
    status: "todo",
    progress: 0,
    createdAt: t,
    updatedAt: t,
  };
}
