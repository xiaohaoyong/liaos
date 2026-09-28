<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NConfigProvider,
  NMessageProvider,
  NEmpty,
  NButton,
  darkTheme,
  zhCN,
  dateZhCN,
  createDiscreteApi,
} from "naive-ui";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { loadData, focusWindow, createNote } from "./api";
import { useTasksStore } from "./stores/tasks";
import { useCategoriesStore } from "./stores/categories";
import { useSettingsStore } from "./stores/settings";
import { UNFINISHED_STATUSES } from "./types";
import { lightThemeOverrides, darkThemeOverrides } from "./theme";
import StickyQuickAdd from "./components/StickyQuickAdd.vue";

const { message } = createDiscreteApi(["message"]);
const tasksStore = useTasksStore();
const categoriesStore = useCategoriesStore();
const settingsStore = useSettingsStore();

const showQuickAdd = ref(false);

// 未完成事项（todo + doing，即排除「已完成/已取消」）
const unfinishedTasks = computed(() =>
  tasksStore.tasks.filter((t) => UNFINISHED_STATUSES.includes(t.status))
);

// 主题：跟随系统 / 亮色 / 暗色（与 App.vue 保持一致）
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
}

onMounted(async () => {
  // 数据联动：任一窗口保存后广播 data-changed，这里重新加载
  await listen("data-changed", () => reload());
  await reload();
});

// 点击事项唤起主界面
function openMain() {
  focusWindow();
}

// 收起便签（隐藏，之后靠快捷键/托盘唤回）
function hideSelf() {
  getCurrentWindow().hide();
}

// 新建便利贴（Rust 端写数据并在桌面级联位置弹出小纸片）
async function addNote() {
  try {
    await createNote();
  } catch (e) {
    message.error("新建便利贴失败：" + e);
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
      <div class="sticky">
        <header class="sticky-header" data-tauri-drag-region>
          <span class="sticky-title">了事</span>
          <div class="sticky-actions" data-tauri-drag-region="false">
            <n-button size="tiny" quaternary title="添加待办" @click="showQuickAdd = true">＋</n-button>
            <n-button size="tiny" quaternary title="收起" @click="hideSelf">—</n-button>
          </div>
        </header>

        <div class="sticky-body">
          <n-empty
            v-if="unfinishedTasks.length === 0"
            description="暂无未完成事项"
            size="small"
          />
          <div v-else class="sticky-list">
            <div
              v-for="t in unfinishedTasks"
              :key="t.id"
              class="sticky-item"
              @click="openMain"
            >
              <span class="dot" :class="'s-' + t.status"></span>
              <span class="item-title">{{ t.title }}</span>
              <span class="item-cat">{{ t.category }}</span>
            </div>
          </div>
        </div>

        <div class="sticky-footer">
          <n-button size="tiny" quaternary class="add-note-btn" @click="addNote">＋ 便利贴</n-button>
        </div>

        <StickyQuickAdd v-model:show="showQuickAdd" />
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.sticky {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--sticky-bg);
  border: 1px solid var(--sticky-border);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
}

.sticky-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  flex-shrink: 0;
  cursor: grab;
  user-select: none;
}

.sticky-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}

.sticky-actions {
  display: flex;
  gap: 2px;
}

.sticky-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 12px 12px;
}

.sticky-footer {
  flex-shrink: 0;
  padding: 0 12px 8px;
  display: flex;
  justify-content: center;
}

.sticky-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.sticky-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  background: var(--sticky-surface);
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.15s ease;
}
.sticky-item:hover {
  background: var(--primary-soft);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.s-todo {
  background: var(--status-todo);
}
.s-doing {
  background: var(--status-doing);
}

.item-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-cat {
  font-size: 11px;
  color: var(--text-3);
  flex-shrink: 0;
}
</style>
