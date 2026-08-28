import { defineStore } from "pinia";
import { ref } from "vue";
import type { NewsFeed, Settings } from "../types";
import { saveSettings } from "../api";

// 默认新闻源（与 Rust 端 storage.rs 的 default_news_feeds 保持一致）
export const DEFAULT_NEWS_FEEDS: NewsFeed[] = [
  { name: "开源中国", url: "https://www.oschina.net/news/rss", enabled: true },
  { name: "Solidot", url: "https://www.solidot.org/index.rss", enabled: true },
  { name: "Hacker News", url: "https://hnrss.org/frontpage", enabled: true },
];

const DEFAULT_SETTINGS: Settings = {
  remoteUrl: "",
  username: "",
  token: "",
  autoSync: true,
  autostart: false,
  theme: "system",
  hotkey: "Ctrl+Shift+Space",
  hostsHotkey: "Ctrl+Alt+H",
  newsFeeds: DEFAULT_NEWS_FEEDS,
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<Settings>({ ...DEFAULT_SETTINGS });

  function setSettings(s: Settings) {
    settings.value = { ...DEFAULT_SETTINGS, ...s };
  }

  function update(patch: Partial<Settings>) {
    settings.value = { ...settings.value, ...patch };
    saveSettings(settings.value).catch((e) => console.error("保存设置失败", e));
  }

  // 是否已配置 git 远程仓库
  function isSyncConfigured(): boolean {
    return !!settings.value.remoteUrl && !!settings.value.token;
  }

  return { settings, setSettings, update, isSyncConfigured };
});
