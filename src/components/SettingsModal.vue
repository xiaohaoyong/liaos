<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import {
  NModal,
  NTabs,
  NTabPane,
  NForm,
  NFormItem,
  NInput,
  NSwitch,
  NButton,
  NSpace,
  NList,
  NListItem,
  NPopconfirm,
  NTag,
  NRadioGroup,
  NRadioButton,
  useMessage,
} from "naive-ui";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useSettingsStore, DEFAULT_NEWS_FEEDS } from "../stores/settings";
import { useCategoriesStore } from "../stores/categories";
import { syncNow, setHotkey, refreshNews } from "../api";
import type { NewsFeed, ThemeMode } from "../types";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const message = useMessage();
const settingsStore = useSettingsStore();
const categoriesStore = useCategoriesStore();

const remoteUrl = ref("");
const username = ref("");
const token = ref("");
const autoSync = ref(true);
const autostart = ref(false);
const syncing = ref(false);
const newCategoryName = ref("");
const hotkey = ref("");
// 新闻源：本地表单副本（打开弹窗时从 store 深拷贝，保存时整体写回）
const newsFeeds = ref<NewsFeed[]>([]);
const newFeedName = ref("");
const newFeedUrl = ref("");

const themeOptions = [
  { label: "跟随系统", value: "system" },
  { label: "亮色", value: "light" },
  { label: "暗色", value: "dark" },
];

function setTheme(v: string) {
  settingsStore.update({ theme: v as ThemeMode });
}

// 同步 settings store 到本地表单
watch(
  () => props.show,
  (show) => {
    if (show) {
      remoteUrl.value = settingsStore.settings.remoteUrl;
      username.value = settingsStore.settings.username;
      token.value = settingsStore.settings.token;
      autoSync.value = settingsStore.settings.autoSync;
      autostart.value = settingsStore.settings.autostart;
      hotkey.value = settingsStore.settings.hotkey;
      newsFeeds.value = JSON.parse(JSON.stringify(settingsStore.settings.newsFeeds));
    }
  }
);

onMounted(async () => {
  try {
    autostart.value = await isEnabled();
  } catch {
    /* 插件未初始化时忽略 */
  }
});

function saveGit() {
  settingsStore.update({
    remoteUrl: remoteUrl.value.trim(),
    username: username.value.trim(),
    token: token.value.trim(),
    autoSync: autoSync.value,
  });
  message.success("同步设置已保存");
}

async function toggleAutostart(v: boolean) {
  try {
    if (v) {
      await enable();
    } else {
      await disable();
    }
    settingsStore.update({ autostart: v });
  } catch (e) {
    message.error("设置开机自启失败：" + e);
  }
}

async function doSync() {
  syncing.value = true;
  try {
    const result = await syncNow();
    message.success(result || "同步完成");
  } catch (e) {
    message.error("同步失败：" + e);
  } finally {
    syncing.value = false;
  }
}

function addCategory() {
  const name = newCategoryName.value.trim();
  if (!name) {
    message.warning("请输入分类名");
    return;
  }
  if (categoriesStore.categories.some((c) => c.name === name)) {
    message.warning("该分类已存在");
    return;
  }
  categoriesStore.add(name);
  newCategoryName.value = "";
}

async function saveHotkey() {
  const h = hotkey.value.trim();
  if (!h) {
    message.warning("请输入快捷键");
    return;
  }
  try {
    await setHotkey(h);
    settingsStore.update({ hotkey: h });
    message.success("快捷键已保存");
  } catch (e) {
    message.error("快捷键设置失败：" + e);
  }
}

// ===== 新闻源管理 =====

function addFeed() {
  const name = newFeedName.value.trim();
  const url = newFeedUrl.value.trim();
  if (!name) {
    message.warning("请输入源名称");
    return;
  }
  if (!/^https?:\/\//.test(url)) {
    message.warning("请输入以 http(s):// 开头的 RSS 地址");
    return;
  }
  if (newsFeeds.value.some((f) => f.url === url)) {
    message.warning("该源已存在");
    return;
  }
  newsFeeds.value.push({ name, url, enabled: true });
  newFeedName.value = "";
  newFeedUrl.value = "";
}

function removeFeed(url: string) {
  newsFeeds.value = newsFeeds.value.filter((f) => f.url !== url);
}

function restoreDefaultFeeds() {
  newsFeeds.value = JSON.parse(JSON.stringify(DEFAULT_NEWS_FEEDS));
}

// 保存源列表并立即触发一次后台抓取，资讯窗口经 "news-updated" 事件自动刷新
function saveFeeds() {
  settingsStore.update({ newsFeeds: newsFeeds.value });
  refreshNews().catch(() => {
    /* 触发失败不影响保存结果 */
  });
  message.success("新闻源已保存，正在刷新");
}
</script>

<template>
  <n-modal
    :show="show"
    :mask-closable="false"
    preset="card"
    title="设置"
    style="width: 560px; max-width: 90vw"
    @update:show="emit('update:show', $event)"
  >
    <n-tabs type="line" animated>
      <!-- 同步设置 -->
      <n-tab-pane name="sync" tab="同步设置">
        <n-form label-placement="top">
          <n-form-item label="远程仓库地址（Gitee / GitHub / 自建）">
            <n-input v-model:value="remoteUrl" placeholder="https://gitee.com/用户名/仓库.git" />
          </n-form-item>
          <n-form-item label="用户名">
            <n-input v-model:value="username" placeholder="Gitee/GitHub 账号" />
          </n-form-item>
          <n-form-item label="Access Token">
            <n-input
              v-model:value="token"
              type="password"
              show-password-on="click"
              placeholder="Personal Access Token"
            />
          </n-form-item>
          <n-form-item label="自动同步">
            <n-switch v-model:value="autoSync" />
          </n-form-item>
          <n-form-item label="开机自启">
            <n-switch :value="autostart" @update:value="toggleAutostart" />
          </n-form-item>
          <n-space>
            <n-button type="primary" @click="saveGit">保存设置</n-button>
            <n-button :loading="syncing" @click="doSync">立即同步</n-button>
          </n-space>
        </n-form>
      </n-tab-pane>

      <!-- 外观 -->
      <n-tab-pane name="appearance" tab="外观">
        <n-form label-placement="top">
          <n-form-item label="主题">
            <n-radio-group
              :value="settingsStore.settings.theme"
              size="small"
              @update:value="setTheme"
            >
              <n-radio-button
                v-for="o in themeOptions"
                :key="o.value"
                :value="o.value"
              >
                {{ o.label }}
              </n-radio-button>
            </n-radio-group>
          </n-form-item>
          <n-form-item label="呼出/隐藏便签快捷键">
            <n-space>
              <n-input
                v-model:value="hotkey"
                placeholder="例如 Ctrl+Shift+Space"
                style="width: 220px"
              />
              <n-button type="primary" @click="saveHotkey">保存</n-button>
            </n-space>
          </n-form-item>
        </n-form>
      </n-tab-pane>

      <!-- 分类管理 -->
      <n-tab-pane name="category" tab="分类管理">
        <n-space vertical style="width: 100%">
          <n-space>
            <n-input
              v-model:value="newCategoryName"
              placeholder="新分类名"
              style="width: 200px"
              @keyup.enter="addCategory"
            />
            <n-button type="primary" @click="addCategory">添加分类</n-button>
          </n-space>
          <n-list bordered>
            <n-list-item v-for="c in categoriesStore.categories" :key="c.id">
              <div style="display: flex; justify-content: space-between; align-items: center">
                <span>{{ c.name }}<n-tag v-if="c.isPreset" size="small" style="margin-left: 8px">预设</n-tag></span>
                <n-popconfirm v-if="!c.isPreset" @positive-click="categoriesStore.remove(c.id)">
                  <template #trigger>
                    <n-button size="tiny" type="error" quaternary>删除</n-button>
                  </template>
                  确认删除该分类？
                </n-popconfirm>
              </div>
            </n-list-item>
          </n-list>
        </n-space>
      </n-tab-pane>

      <!-- 新闻源管理 -->
      <n-tab-pane name="news" tab="新闻源">
        <n-space vertical style="width: 100%">
          <n-space>
            <n-input
              v-model:value="newFeedName"
              placeholder="源名称"
              style="width: 130px"
            />
            <n-input
              v-model:value="newFeedUrl"
              placeholder="RSS 地址（https://…）"
              style="width: 240px"
              @keyup.enter="addFeed"
            />
            <n-button type="primary" @click="addFeed">添加</n-button>
          </n-space>
          <n-list bordered>
            <n-list-item v-for="f in newsFeeds" :key="f.url">
              <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px">
                <div style="min-width: 0; flex: 1">
                  <div>{{ f.name }}</div>
                  <div
                    style="
                      font-size: 12px;
                      color: var(--text-3);
                      overflow: hidden;
                      text-overflow: ellipsis;
                      white-space: nowrap;
                    "
                    :title="f.url"
                  >
                    {{ f.url }}
                  </div>
                </div>
                <n-switch v-model:value="f.enabled" size="small" />
                <n-popconfirm @positive-click="removeFeed(f.url)">
                  <template #trigger>
                    <n-button size="tiny" type="error" quaternary>删除</n-button>
                  </template>
                  确认删除该新闻源？
                </n-popconfirm>
              </div>
            </n-list-item>
          </n-list>
          <n-space>
            <n-button type="primary" @click="saveFeeds">保存并刷新</n-button>
            <n-button @click="restoreDefaultFeeds">恢复默认源</n-button>
          </n-space>
        </n-space>
      </n-tab-pane>
    </n-tabs>
  </n-modal>
</template>
