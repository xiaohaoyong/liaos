<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  NConfigProvider,
  NMessageProvider,
  NButton,
  NInput,
  NSwitch,
  NTag,
  NEmpty,
  NPopconfirm,
  NRadio,
  darkTheme,
  zhCN,
  dateZhCN,
  createDiscreteApi,
} from "naive-ui";
import { loadData, loadHosts, hostsStatus, applyHosts, setHostsHotkey } from "./api";
import { useSettingsStore } from "./stores/settings";
import { useHostsStore } from "./stores/hosts";
import { lightThemeOverrides, darkThemeOverrides } from "./theme";

const { message } = createDiscreteApi(["message"]);
const settingsStore = useSettingsStore();
const hostsStore = useHostsStore();

// ===== 主题：跟随系统 / 亮色 / 暗色（与 App.vue 保持一致） =====
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

// ===== 状态：系统 hosts 标记块是否与配置一致 =====
const upToDate = ref<boolean | null>(null); // null = 尚未加载
const applying = ref(false);

async function refreshStatus() {
  try {
    await hostsStore.flush(); // 先确保配置写盘完成，避免比对到旧配置
    const s = await hostsStatus();
    upToDate.value = s.upToDate;
  } catch (e) {
    console.error("查询 hosts 状态失败", e);
  }
}

// ===== 域名选择 =====
const activeDomainId = ref("");

const activeDomain = computed(() =>
  hostsStore.config.domains.find((d) => d.id === activeDomainId.value)
);

function selectDomain(id: string) {
  activeDomainId.value = id;
}

function ensureSelection() {
  if (!activeDomain.value) {
    activeDomainId.value = hostsStore.config.domains[0]?.id ?? "";
  }
}

// 左侧列表行：当前生效 IP（无则显示提示文案）
function activeIpOf(domainId: string): string {
  const d = hostsStore.config.domains.find((x) => x.id === domainId);
  if (!d) return "";
  const active = d.ips.find((i) => i.id === d.activeIpId);
  return active?.ip ?? "";
}

// ===== 域名增删 =====
const newDomainInput = ref("");

const DOMAIN_RE = /^[a-zA-Z0-9]([a-zA-Z0-9.-]*[a-zA-Z0-9])?$/;

function addDomain() {
  const domain = newDomainInput.value.trim();
  if (!domain) {
    message.warning("请输入域名");
    return;
  }
  if (!DOMAIN_RE.test(domain)) {
    message.warning("域名格式不正确（示例 api.example.com）");
    return;
  }
  if (hostsStore.findByDomain(domain)) {
    message.warning("该域名已存在");
    return;
  }
  hostsStore.addDomain(domain);
  newDomainInput.value = "";
  // 新建的域名自动选中
  const created = hostsStore.config.domains.find((d) => d.domain === domain);
  if (created) activeDomainId.value = created.id;
  refreshStatus();
}

function removeDomain(id: string) {
  hostsStore.removeDomain(id);
  ensureSelection();
  refreshStatus();
}

function toggleDomain(id: string, enabled: boolean) {
  hostsStore.toggleDomain(id, enabled);
  refreshStatus();
}

// ===== 候选 IP 增删与切换 =====
const newIpInput = ref("");
const newNoteInput = ref("");

const IPV4_RE = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;

function validIp(ip: string): boolean {
  if (ip.includes(":")) return true; // IPv6 字面量放行
  const m = ip.match(IPV4_RE);
  return !!m && m.slice(1).every((seg) => Number(seg) <= 255);
}

function addIp() {
  const ip = newIpInput.value.trim();
  if (!activeDomain.value) {
    message.warning("请先选择或添加一个域名");
    return;
  }
  if (!validIp(ip)) {
    message.warning("IP 格式不正确（示例 127.0.0.1）");
    return;
  }
  if (activeDomain.value.ips.some((i) => i.ip === ip)) {
    message.warning("该 IP 已在候选列表中");
    return;
  }
  hostsStore.addIp(activeDomain.value.id, ip, newNoteInput.value.trim());
  newIpInput.value = "";
  newNoteInput.value = "";
  refreshStatus();
}

function removeIp(domainId: string, ipId: string) {
  hostsStore.removeIp(domainId, ipId);
  refreshStatus();
}

function setActiveIp(domainId: string, ipId: string) {
  hostsStore.setActiveIp(domainId, ipId);
  refreshStatus();
}

// 备注编辑（@change 在失焦/回车时触发，避免每个按键都写盘）
function onNoteChange() {
  hostsStore.persist();
  refreshStatus();
}

// ===== 应用到系统（提权写 hosts + 刷新 DNS） =====
async function apply() {
  applying.value = true;
  try {
    await hostsStore.flush(); // 确保最新配置已写盘
    const out = await applyHosts();
    if (out.cancelled) {
      message.warning(out.message);
    } else if (out.message === "已应用到系统") {
      message.success(out.message);
    } else {
      message.error(out.message);
    }
    await refreshStatus();
  } catch (e) {
    message.error("应用失败：" + e);
  } finally {
    applying.value = false;
  }
}

// ===== 快捷键设置 =====
const hotkeyInput = ref("");

async function saveHotkey() {
  const h = hotkeyInput.value.trim();
  if (!h) {
    message.warning("请输入快捷键");
    return;
  }
  try {
    await setHostsHotkey(h);
    settingsStore.update({ hostsHotkey: h });
    message.success("快捷键已保存");
  } catch (e) {
    message.error("快捷键设置失败：" + e);
  }
}

// ===== 加载 =====
onMounted(async () => {
  try {
    const data = await loadData(); // 主题 + 快捷键显示需要 settings
    settingsStore.setSettings(data.settings);
    hotkeyInput.value = data.settings.hostsHotkey;
  } catch (e) {
    message.error("加载设置失败：" + e);
  }
  try {
    hostsStore.setConfig(await loadHosts());
    ensureSelection();
  } catch (e) {
    message.error("加载 hosts 配置失败：" + e);
  }
  await refreshStatus();
});
</script>

<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="isDark ? darkTheme : null"
    :theme-overrides="isDark ? darkThemeOverrides : lightThemeOverrides"
  >
    <n-message-provider>
      <div class="hosts-app">
        <!-- 工具栏 -->
        <header class="toolbar">
          <span class="title">Hosts 编辑器</span>
          <n-tag v-if="upToDate === true" type="success" size="small" round>已是最新</n-tag>
          <n-tag v-else-if="upToDate === false" type="warning" size="small" round>
            有未应用更改
          </n-tag>
          <div class="spacer"></div>
          <div class="hotkey-box" title="呼出本窗口的全局快捷键">
            <span class="hotkey-label">快捷键</span>
            <n-input
              v-model:value="hotkeyInput"
              placeholder="例如 Ctrl+Alt+H"
              size="small"
              style="width: 150px"
              @keyup.enter="saveHotkey"
            />
            <n-button size="small" @click="saveHotkey">保存</n-button>
          </div>
          <n-button
            type="primary"
            size="small"
            :loading="applying"
            @click="apply"
          >
            应用到系统（需管理员授权）
          </n-button>
        </header>

        <div class="body">
          <!-- 左：域名列表 -->
          <aside class="domains">
            <div
              v-for="d in hostsStore.config.domains"
              :key="d.id"
              class="domain-item"
              :class="{ active: d.id === activeDomainId }"
              @click="selectDomain(d.id)"
            >
              <n-switch
                size="small"
                :value="d.enabled"
                @update:value="(v: boolean) => toggleDomain(d.id, v)"
                @click.stop
              />
              <div class="domain-text">
                <span class="domain-name">{{ d.domain }}</span>
                <span class="domain-ip">
                  {{ d.enabled ? (activeIpOf(d.id) || "未选择生效 IP") : "已停用" }}
                </span>
              </div>
              <n-popconfirm @positive-click="removeDomain(d.id)">
                <template #trigger>
                  <n-button
                    size="tiny"
                    type="error"
                    quaternary
                    @click.stop
                  >
                    删
                  </n-button>
                </template>
                确认删除域名「{{ d.domain }}」（含 {{ d.ips.length }} 个候选 IP）？
              </n-popconfirm>
            </div>
            <div class="domain-add">
              <n-input
                v-model:value="newDomainInput"
                placeholder="新域名，如 api.example.com"
                size="small"
                @keyup.enter="addDomain"
              />
              <n-button size="small" @click="addDomain">添加</n-button>
            </div>
          </aside>

          <!-- 右：候选 IP 管理 -->
          <main class="ips">
            <template v-if="activeDomain">
              <div class="ips-head">
                <span class="ips-title">{{ activeDomain.domain }}</span>
                <n-tag size="small" :bordered="false">
                  {{ activeDomain.ips.length }} 个候选 IP
                </n-tag>
                <span class="ips-hint">单选切换生效 IP，停用开关在左侧</span>
              </div>

              <div class="ips-list">
                <div v-for="ip in activeDomain.ips" :key="ip.id" class="ip-row">
                  <n-radio
                    :checked="ip.id === activeDomain.activeIpId"
                    @update:checked="setActiveIp(activeDomainId, ip.id)"
                  >
                    生效
                  </n-radio>
                  <span class="ip-addr">{{ ip.ip }}</span>
                  <n-input
                    v-model:value="ip.note"
                    size="small"
                    placeholder="备注（如：开发 / 生产）"
                    class="ip-note"
                    @change="onNoteChange"
                  />
                  <n-popconfirm @positive-click="removeIp(activeDomainId, ip.id)">
                    <template #trigger>
                      <n-button size="tiny" type="error" quaternary>删</n-button>
                    </template>
                    确认删除候选 IP {{ ip.ip }}？
                  </n-popconfirm>
                </div>
                <n-empty
                  v-if="activeDomain.ips.length === 0"
                  description="该域名暂无候选 IP，在下方添加"
                  size="small"
                  style="margin: 24px 0"
                />
              </div>

              <div class="ip-add">
                <n-input
                  v-model:value="newIpInput"
                  placeholder="IP，如 127.0.0.1"
                  size="small"
                  style="width: 160px"
                  @keyup.enter="addIp"
                />
                <n-input
                  v-model:value="newNoteInput"
                  placeholder="备注（可选）"
                  size="small"
                  style="flex: 1"
                  @keyup.enter="addIp"
                />
                <n-button size="small" type="primary" @click="addIp">添加</n-button>
              </div>
            </template>
            <n-empty
              v-else
              description="先在左侧添加一个域名"
              style="margin: auto"
            />
          </main>
        </div>
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<style scoped>
.hosts-app {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg);
  overflow: hidden;
}

/* 工具栏 */
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

.title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-1);
}

.spacer {
  flex: 1;
}

.hotkey-box {
  display: flex;
  align-items: center;
  gap: 6px;
}

.hotkey-label {
  font-size: 12px;
  color: var(--text-3);
}

/* 主体两栏 */
.body {
  flex: 1;
  display: flex;
  min-height: 0;
}

/* 左栏：域名列表 */
.domains {
  width: 280px;
  flex-shrink: 0;
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: 8px;
  gap: 4px;
  overflow-y: auto;
}

.domain-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.domain-item:hover,
.domain-item.active {
  background: var(--primary-soft);
}

.domain-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.domain-name {
  font-size: 13px;
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.domain-ip {
  font-size: 11px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.domain-add {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}

/* 右栏：候选 IP */
.ips {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 12px 16px;
}

.ips-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.ips-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ips-hint {
  font-size: 12px;
  color: var(--text-3);
  margin-left: auto;
  flex-shrink: 0;
}

.ips-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.ip-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border-radius: 6px;
}

.ip-row:hover {
  background: var(--surface);
}

.ip-addr {
  font-size: 13px;
  color: var(--text-1);
  font-family: Consolas, Monaco, monospace;
  flex-shrink: 0;
  min-width: 120px;
}

.ip-note {
  flex: 1;
  min-width: 0;
}

.ip-add {
  display: flex;
  gap: 8px;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px dashed var(--border);
}
</style>
