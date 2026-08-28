import { defineStore } from "pinia";
import { ref } from "vue";
import type { HostsConfig, HostsDomain } from "../types";
import { saveHosts } from "../api";

// Hosts 编辑器配置（域名 → 多候选 IP 单选生效），持久化到 Rust 端 hosts.json
export const useHostsStore = defineStore("hosts", () => {
  const config = ref<HostsConfig>({ domains: [] });

  function setConfig(c: HostsConfig) {
    config.value = c;
  }

  // 最近一次持久化的 Promise（apply/状态比对前经 flush() 等待，确保读到的配置是最新的）
  let lastPersist: Promise<void> = Promise.resolve();

  // 持久化到 Rust 端（写 hosts.json + 触发同步）
  function persist() {
    lastPersist = saveHosts(config.value).catch((e) => {
      console.error("保存 hosts 配置失败", e);
    });
  }

  // 等待未完成的持久化落盘（应用/状态比对前调用，避免竞态）
  function flush(): Promise<void> {
    return lastPersist;
  }

  // ===== 域名 =====

  function addDomain(domain: string) {
    config.value.domains.push({
      id: crypto.randomUUID(),
      domain,
      enabled: true,
      ips: [],
      activeIpId: null,
    });
    persist();
  }

  function removeDomain(id: string) {
    config.value.domains = config.value.domains.filter((d) => d.id !== id);
    persist();
  }

  function toggleDomain(id: string, enabled: boolean) {
    const d = config.value.domains.find((d) => d.id === id);
    if (!d) return;
    d.enabled = enabled;
    persist();
  }

  // 按域名查找（重名校验用）
  function findByDomain(domain: string): HostsDomain | undefined {
    return config.value.domains.find((d) => d.domain === domain);
  }

  // ===== 候选 IP =====

  // 添加候选 IP；若该域名当前没有生效 IP 且域名已启用，则自动设为生效
  function addIp(domainId: string, ip: string, note: string) {
    const d = config.value.domains.find((d) => d.id === domainId);
    if (!d) return;
    const item = { id: crypto.randomUUID(), ip, note };
    d.ips.push(item);
    if (d.activeIpId === null) {
      d.activeIpId = item.id;
    }
    persist();
  }

  function removeIp(domainId: string, ipId: string) {
    const d = config.value.domains.find((d) => d.id === domainId);
    if (!d) return;
    d.ips = d.ips.filter((i) => i.id !== ipId);
    if (d.activeIpId === ipId) {
      d.activeIpId = null; // 删的是生效 IP：回到未选择状态
    }
    persist();
  }

  // 单选切换生效 IP
  function setActiveIp(domainId: string, ipId: string) {
    const d = config.value.domains.find((d) => d.id === domainId);
    if (!d) return;
    d.activeIpId = ipId;
    persist();
  }

  return {
    config,
    setConfig,
    addDomain,
    removeDomain,
    toggleDomain,
    findByDomain,
    addIp,
    removeIp,
    setActiveIp,
    persist,
    flush,
  };
});
