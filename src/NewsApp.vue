<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import {
  NConfigProvider,
  NMessageProvider,
  NEmpty,
  NButton,
  NDropdown,
  NTag,
  NSpin,
  darkTheme,
  zhCN,
  dateZhCN,
  createDiscreteApi,
  type DropdownOption,
} from "naive-ui";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { loadData, getNews, refreshNews } from "./api";
import { useTasksStore } from "./stores/tasks";
import { useCategoriesStore } from "./stores/categories";
import { useSettingsStore } from "./stores/settings";
import { createEmptyTask } from "./utils";
import type { NewsCache, NewsItem } from "./types";
import { lightThemeOverrides, darkThemeOverrides } from "./theme";

// 顶层消息提示（窗口根组件不能用 useMessage，与 App.vue / StickyApp.vue 同规范）
const { message } = createDiscreteApi(["message"]);
const tasksStore = useTasksStore();
const categoriesStore = useCategoriesStore();
const settingsStore = useSettingsStore();

// ---- 状态 ----
const news = ref<NewsCache>({ items: [], fetchedAt: null });
const loading = ref(true);

// 右键菜单（manual 模式手动定位）
const menuShow = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const activeItem = ref<NewsItem | null>(null);

// 是否所有新闻源都已停用（空态文案区分）
const allFeedsDisabled = computed(
  () => settingsStore.settings.newsFeeds.filter((f) => f.enabled).length === 0
);

// 主题：跟随系统 / 亮色 / 暗色（与 App.vue / StickyApp.vue 保持一致）
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

// ---- 数据装载 ----

// 装载任务/分类/设置（静默创建待办需要最新 tasks/categories，主题需要 settings）
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

// 手动刷新（⟳ 按钮）：触发后台抓取，loading 等 "news-updated" 事件解除
function manualRefresh() {
  if (loading.value) return;
  loading.value = true;
  refreshNews().catch((e) => {
    loading.value = false;
    message.error("刷新失败：" + e);
  });
}

onMounted(async () => {
  // 先注册监听再取缓存，防止启动抓取的 emit 早于监听注册而丢失
  await listen<NewsCache>("news-updated", (e) => {
    news.value = e.payload;
    loading.value = false;
  });
  // 数据联动：任一窗口保存后广播 data-changed，这里重新加载
  await listen("data-changed", () => reload());

  try {
    news.value = await getNews();
  } catch (e) {
    message.error("读取新闻缓存失败：" + e);
  }
  // 缓存为空或超过 5 分钟未更新时触发一次后台抓取；否则直接展示缓存
  const stale =
    !news.value.fetchedAt || Date.now() / 1000 - news.value.fetchedAt > 300;
  if (news.value.items.length === 0 || stale) {
    await refreshNews().catch(() => (loading.value = false));
  } else {
    loading.value = false;
  }
  await reload();
});

// ---- 交互 ----

// 左键：外部浏览器打开原文
function open(item: NewsItem) {
  openUrl(item.link).catch((e) => message.error("打开失败：" + e));
}

// 右键菜单：目前仅「加入任务列表」（manual 模式，事件坐标定位）
const menuOptions: DropdownOption[] = [{ label: "加入任务列表", key: "add" }];

function showMenu(e: MouseEvent, item: NewsItem) {
  activeItem.value = item;
  // 先关再开（nextTick），确保连续右键不同条目时坐标正确更新
  menuShow.value = false;
  nextTick(() => {
    menuX.value = e.clientX;
    menuY.value = e.clientY;
    menuShow.value = true;
  });
}

function onMenuSelect(key: string | number) {
  menuShow.value = false;
  if (key === "add" && activeItem.value) {
    addToTasks(activeItem.value);
  }
}

// 静默创建待办：标题 = 新闻标题，备注 = 原文链接，不弹编辑框
function addToTasks(item: NewsItem) {
  // 默认分类：优先预设「其他」，避免新闻默认归入「工作」
  const category =
    categoriesStore.categories.find((c) => c.id === "other")?.name ??
    categoriesStore.categories[0]?.name ??
    "其他";
  const task = createEmptyTask(category);
  task.title = item.title;
  task.note = item.link;
  tasksStore.add(task); // 落盘 + 广播 data-changed，主窗口/便签自动刷新
  message.success("已加入任务列表");
}

// 收起资讯窗（只隐藏自己，之后靠快捷键/托盘随便签一起唤回）
function hideSelf() {
  getCurrentWindow().hide();
}

// ---- 工具 ----

// 相对时间：刚刚 / x 分钟前 / x 小时前 / 昨天 / M月d日（跨年带年份）
function formatTime(ts: number): string {
  if (!ts) return "";
  const diffSec = Math.floor(Date.now() / 1000 - ts);
  if (diffSec < 60) return "刚刚";
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)} 分钟前`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} 小时前`;
  if (diffSec < 172800) return "昨天";
  const d = new Date(ts * 1000);
  const now = new Date();
  const sameYear = d.getFullYear() === now.getFullYear();
  const base = `${d.getMonth() + 1}月${d.getDate()}日`;
  return sameYear ? base : `${d.getFullYear()}年${base}`;
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
      <div class="news">
        <header class="news-header" data-tauri-drag-region>
          <span class="news-title">资讯</span>
          <span v-if="news.fetchedAt" class="news-time" data-tauri-drag-region>
            {{ formatTime(news.fetchedAt) }}更新
          </span>
          <div class="news-actions" data-tauri-drag-region="false">
            <n-button size="tiny" quaternary title="刷新" @click="manualRefresh">⟳</n-button>
            <n-button size="tiny" quaternary title="收起" @click="hideSelf">—</n-button>
          </div>
        </header>

        <div class="news-body">
          <div v-if="loading" class="news-centering">
            <n-spin size="small" />
            <span class="loading-text">正在抓取…</span>
          </div>
          <n-empty
            v-else-if="news.items.length === 0"
            class="news-centering"
            :description="allFeedsDisabled ? '所有新闻源均已停用' : '暂无新闻，可点击 ⟳ 刷新'"
            size="small"
          />
          <div v-else class="news-list">
            <div
              v-for="item in news.items"
              :key="item.id"
              class="news-item"
              @click="open(item)"
              @contextmenu.prevent="showMenu($event, item)"
            >
              <div class="item-head">
                <n-tag size="tiny" :bordered="false">{{ item.source }}</n-tag>
                <span class="item-time">{{ formatTime(item.publishedAt) }}</span>
              </div>
              <div class="item-title">{{ item.title }}</div>
              <div v-if="item.summary" class="item-summary">{{ item.summary }}</div>
            </div>
          </div>
        </div>

        <!-- 右键菜单：manual 模式，跟随鼠标坐标 -->
        <n-dropdown
          placement="bottom-start"
          trigger="manual"
          :show="menuShow"
          :x="menuX"
          :y="menuY"
          :options="menuOptions"
          :on-clickoutside="() => (menuShow = false)"
          @select="onMenuSelect"
          @update:show="menuShow = $event"
        />
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.news {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--sticky-bg);
  border: 1px solid var(--sticky-border);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
}

.news-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  flex-shrink: 0;
  cursor: grab;
  user-select: none;
}

.news-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-2);
}

.news-time {
  flex: 1;
  min-width: 0;
  font-size: 11px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.news-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}

.news-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 12px 12px;
}

.news-centering {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-3);
}

.loading-text {
  font-size: 12px;
}

.news-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.news-item {
  padding: 8px 10px;
  background: var(--sticky-surface);
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.15s ease;
}
.news-item:hover {
  background: var(--primary-soft);
}

.item-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

.item-time {
  font-size: 11px;
  color: var(--text-3);
  flex-shrink: 0;
}

.item-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-1);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
}

.item-summary {
  margin-top: 3px;
  font-size: 12px;
  color: var(--text-3);
  line-height: 1.45;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
}
</style>
