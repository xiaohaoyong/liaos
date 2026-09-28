<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NConfigProvider,
  NMessageProvider,
  NButton,
  NRadioGroup,
  NRadioButton,
  darkTheme,
  zhCN,
  dateZhCN,
  createDiscreteApi,
} from "naive-ui";
import TaskList from "./components/TaskList.vue";
import TaskEditModal from "./components/TaskEditModal.vue";
import SettingsModal from "./components/SettingsModal.vue";
import NotesPanel from "./components/NotesPanel.vue";
import { loadData, syncNow } from "./api";
import { listen } from "@tauri-apps/api/event";
import { useTasksStore } from "./stores/tasks";
import { useCategoriesStore } from "./stores/categories";
import { useSettingsStore } from "./stores/settings";
import { useNotesStore } from "./stores/notes";
import type { Task, TaskStatus } from "./types";
import { STATUS_LABEL } from "./types";
import { lightThemeOverrides, darkThemeOverrides } from "./theme";

const { message } = createDiscreteApi(["message"]);
const tasksStore = useTasksStore();
const categoriesStore = useCategoriesStore();
const settingsStore = useSettingsStore();
const notesStore = useNotesStore();

const showEdit = ref(false);
const editingTask = ref<Task | null>(null);
const showSettings = ref(false);

// 主内容区视图：任务列表 / 便利贴管理区（侧边栏切换）
const viewMode = ref<"tasks" | "notes">("tasks");

// 筛选条件：分类（侧边栏）+ 状态（顶部分段筛选，默认「未完成」）
const categoryFilter = ref<string>("all");
const statusFilter = ref<string>("unfinished");

// 某个分类（或全部，传 null）的未完成数量
function unfinishedCountOf(category: string | null): number {
  return tasksStore.tasks.filter((t) => {
    if (category !== null && t.category !== category) return false;
    return t.status === "todo" || t.status === "doing";
  }).length;
}

const navItems = computed(() => [
  { label: "全部分类", value: "all", count: unfinishedCountOf(null) },
  ...categoriesStore.categories.map((c) => ({
    label: c.name,
    value: c.name,
    count: unfinishedCountOf(c.name),
  })),
]);

const currentTitle = computed(() =>
  categoryFilter.value === "all" ? "全部分类" : categoryFilter.value
);

// 新建分类（侧边栏内联）
const addingCategory = ref(false);
const newCategoryName = ref("");

function startAddCategory() {
  addingCategory.value = true;
  newCategoryName.value = "";
}

function confirmAddCategory() {
  const name = newCategoryName.value.trim();
  if (!name) {
    addingCategory.value = false;
    return;
  }
  if (categoriesStore.categories.some((c) => c.name === name)) {
    message.warning("该分类已存在");
    return;
  }
  categoriesStore.add(name);
  categoryFilter.value = name;
  addingCategory.value = false;
}

const statusOptions = [
  { label: "未完成", value: "unfinished" },
  ...(Object.keys(STATUS_LABEL) as TaskStatus[]).map((k) => ({
    label: STATUS_LABEL[k],
    value: k,
  })),
];

const filteredTasks = computed(() =>
  tasksStore.tasks.filter((t) => {
    if (categoryFilter.value !== "all" && t.category !== categoryFilter.value) return false;
    if (statusFilter.value === "unfinished") {
      if (t.status !== "todo" && t.status !== "doing") return false;
    } else if (t.status !== statusFilter.value) {
      return false;
    }
    return true;
  })
);

// 主题：跟随系统 / 亮色 / 暗色
const media = window.matchMedia("(prefers-color-scheme: dark)");
const systemDark = ref(media.matches);
media.addEventListener("change", (e) => {
  systemDark.value = e.matches;
});

const isDark = computed(() => {
  const t = settingsStore.settings.theme;
  if (t === "dark") return true;
  if (t === "light") return false;
  return systemDark.value;
});

watch(
  isDark,
  (dark) => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  },
  { immediate: true }
);

async function reload() {
  try {
    const data = await loadData();
    tasksStore.setTasks(data.tasks);
    categoriesStore.setCategories(data.categories);
    settingsStore.setSettings(data.settings);
  } catch (e) {
    message.error("加载数据失败：" + e);
  }
  notesStore.load().catch(() => {});
}

onMounted(async () => {
  // 监听托盘菜单事件
  await listen("tray-create", () => openCreate());
  await listen("tray-sync", () => handleSync());
  await listen("tray-settings", () => {
    showSettings.value = true;
  });
  // 数据联动：便签窗口等其它窗口保存后广播 data-changed，这里重新加载
  await listen("data-changed", () => reload());
  // 便利贴联动：便签窗口/管理区改动后定向广播，刷新侧边栏角标与面板
  await listen("notes-changed", () => notesStore.load().catch(() => {}));

  await reload();
});

function openCreate() {
  editingTask.value = null;
  showEdit.value = true;
}

function openEdit(task: Task) {
  editingTask.value = task;
  showEdit.value = true;
}

function handleDelete(id: string) {
  tasksStore.remove(id);
}

function handleStatus(id: string, status: TaskStatus) {
  tasksStore.setStatus(id, status);
}

async function handleSync() {
  try {
    const result = await syncNow();
    message.success(result || "同步完成");
  } catch (e) {
    message.error("同步失败：" + e);
  }
}
</script>

<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="isDark ? darkTheme : null"
    :theme-overrides="isDark ? darkThemeOverrides : lightThemeOverrides"
  >
    <n-message-provider>
      <div class="app">
        <aside class="sidebar">
          <div class="brand">了事</div>

          <nav class="nav">
            <button
              class="nav-item"
              :class="{ active: viewMode === 'notes' }"
              @click="viewMode = 'notes'"
            >
              <span class="nav-label">便利贴</span>
              <span v-if="notesStore.notes.length > 0" class="nav-count">{{
                notesStore.notes.length
              }}</span>
            </button>
            <div class="nav-divider"></div>

            <button
              v-for="item in navItems"
              :key="item.value"
              class="nav-item"
              :class="{ active: viewMode === 'tasks' && categoryFilter === item.value }"
              @click="categoryFilter = item.value; viewMode = 'tasks'"
            >
              <span class="nav-label">{{ item.label }}</span>
              <span v-if="item.count > 0" class="nav-count">{{ item.count }}</span>
            </button>

            <div v-if="!addingCategory" class="nav-add" @click="startAddCategory">
              ＋ 新建分类
            </div>
            <div v-else class="nav-add-form">
              <input
                v-model="newCategoryName"
                class="nav-add-input"
                placeholder="分类名"
                autofocus
                @keyup.enter="confirmAddCategory"
                @keyup.esc="addingCategory = false"
                @blur="confirmAddCategory"
              />
            </div>
          </nav>

          <div class="sidebar-footer">
            <button class="foot-btn" @click="handleSync">同步</button>
            <button class="foot-btn" @click="showSettings = true">设置</button>
          </div>
        </aside>

        <main class="main">
          <template v-if="viewMode === 'tasks'">
            <header class="main-header">
              <h2 class="main-title">{{ currentTitle }}</h2>
              <n-radio-group v-model:value="statusFilter" size="small">
                <n-radio-button
                  v-for="o in statusOptions"
                  :key="o.value"
                  :value="o.value"
                >
                  {{ o.label }}
                </n-radio-button>
              </n-radio-group>
              <n-button type="primary" @click="openCreate">新建</n-button>
            </header>

            <TaskList
              :tasks="filteredTasks"
              @edit="openEdit"
              @delete="handleDelete"
              @status="handleStatus"
            />
          </template>

          <NotesPanel v-else />
        </main>

        <TaskEditModal v-model:show="showEdit" :task="editingTask" />
        <SettingsModal v-model:show="showSettings" />
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.app {
  height: 100vh;
  display: flex;
  background: var(--bg);
}

/* ===== 侧边栏 ===== */
.sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  border-right: 1px solid var(--border);
}

.brand {
  padding: 20px 20px 12px;
  font-size: 20px;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: var(--text-1);
}

.nav {
  flex: 1;
  overflow-y: auto;
  padding: 4px 12px;
}

.nav-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 8px 12px;
  margin: 2px 0;
  border: none;
  background: transparent;
  border-radius: 6px;
  font-size: 14px;
  color: var(--text-2);
  cursor: pointer;
  text-align: left;
  transition: background-color 0.15s ease, color 0.15s ease;
}
.nav-item:hover {
  background: var(--bg);
  color: var(--text-1);
}
.nav-item.active {
  background: var(--primary-soft);
  color: var(--primary);
  font-weight: 500;
}

.nav-count {
  font-size: 12px;
  color: var(--text-3);
}
.nav-item.active .nav-count {
  color: var(--primary);
}

.nav-divider {
  height: 1px;
  margin: 8px 4px;
  background: var(--border);
}

.nav-add {
  margin: 8px 4px 4px;
  padding: 8px 12px;
  font-size: 13px;
  color: var(--text-3);
  cursor: pointer;
  border-radius: 6px;
  transition: color 0.15s ease, background-color 0.15s ease;
}
.nav-add:hover {
  color: var(--primary);
  background: var(--bg);
}

.nav-add-form {
  margin: 8px 4px 4px;
}
.nav-add-input {
  width: 100%;
  padding: 7px 10px;
  box-sizing: border-box;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg);
  color: var(--text-1);
  font-size: 13px;
  outline: none;
}
.nav-add-input:focus {
  border-color: var(--primary);
}

.sidebar-footer {
  display: flex;
  gap: 8px;
  padding: 12px;
  border-top: 1px solid var(--border);
}
.foot-btn {
  flex: 1;
  padding: 8px;
  border: none;
  background: transparent;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-2);
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}
.foot-btn:hover {
  background: var(--bg);
  color: var(--text-1);
}

/* ===== 主内容区 ===== */
.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.main-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px 24px;
}

.main-title {
  margin: 0;
  margin-right: auto;
  font-size: 16px;
  font-weight: 600;
  color: var(--text-1);
}
</style>
